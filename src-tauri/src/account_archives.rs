use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use star_rail_profiles::AccountImportSummary;

use crate::{
    character_archive::{CharacterArchive, Error, Index},
    desktop_relic_export::{ExportOrigin, account::AccountSnapshot},
};

#[derive(Serialize)]
pub(crate) struct AccountList {
    active_uid: Option<u32>,
    accounts: Vec<Account>,
}

#[derive(Serialize)]
struct Account {
    uid: u32,
    remark: String,
}

#[derive(Serialize)]
pub(crate) struct ImportedAccount {
    uid: Option<u32>,
    created: bool,
    summary: AccountImportSummary,
    selection: AccountList,
}

pub(crate) struct AccountArchives {
    root: PathBuf,
    preferences: Connection,
    legacy_uid: Option<u32>,
    active_uid: Option<u32>,
    accounts: BTreeMap<u32, String>,
    loaded: Option<CharacterArchive>,
}

impl AccountArchives {
    pub(crate) fn open(root: &Path) -> Result<Self, Error> {
        let directory = root.join("accounts");
        fs::create_dir_all(&directory)?;
        let legacy_uid = CharacterArchive::stored_account_uid(root)?;
        let mut accounts: BTreeMap<_, _> = legacy_uid
            .into_iter()
            .map(|uid| (uid, uid.to_string()))
            .collect();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(uid) = name.to_str().and_then(|name| name.parse::<u32>().ok())
                && name.to_str() == Some(uid.to_string().as_str())
                && entry.file_type()?.is_dir()
                && CharacterArchive::stored_account_uid(&entry.path())? == Some(uid)
            {
                accounts.insert(uid, uid.to_string());
            }
        }

        let preferences = open_preferences(root)?;
        let saved_uid: Option<u32> = preferences
            .query_row(
                "SELECT uid FROM active_account WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        {
            let mut query = preferences.prepare("SELECT uid, remark FROM account_remarks")?;
            let remarks = query.query_map([], |row| Ok((row.get::<_, u32>(0)?, row.get(1)?)))?;
            for remark in remarks {
                let (uid, remark) = remark?;
                if let Some(entry) = accounts.get_mut(&uid) {
                    *entry = remark;
                }
            }
        }
        Ok(Self {
            root: root.to_owned(),
            preferences,
            legacy_uid,
            active_uid: saved_uid
                .filter(|uid| accounts.contains_key(uid))
                .or(legacy_uid)
                .or_else(|| accounts.first_key_value().map(|(&uid, _)| uid)),
            accounts,
            loaded: None,
        })
    }

    pub(crate) fn list(&self) -> AccountList {
        AccountList {
            active_uid: self.active_uid,
            accounts: self
                .accounts
                .iter()
                .map(|(&uid, remark)| Account {
                    uid,
                    remark: remark.clone(),
                })
                .collect(),
        }
    }

    pub(crate) fn select(&mut self, uid: u32) -> Result<AccountList, Error> {
        if !self.accounts.contains_key(&uid) {
            return Err(Error::AccountSelection);
        }
        if self.active_uid != Some(uid) {
            self.preferences.execute(
                "INSERT OR REPLACE INTO active_account VALUES (1, ?1)",
                [uid],
            )?;
            self.active_uid = Some(uid);
            self.loaded = None;
        }
        Ok(self.list())
    }

    pub(crate) fn rename(&mut self, uid: u32, remark: &str) -> Result<AccountList, Error> {
        let entry = self.accounts.get_mut(&uid).ok_or(Error::AccountSelection)?;
        let remark = remark.trim();
        if remark.chars().count() > 48 || remark.chars().any(char::is_control) {
            return Err(Error::AccountRemark);
        }
        if remark.is_empty() {
            self.preferences
                .execute("DELETE FROM account_remarks WHERE uid = ?1", [uid])?;
            *entry = uid.to_string();
        } else {
            self.preferences.execute(
                "INSERT OR REPLACE INTO account_remarks VALUES (?1, ?2)",
                params![uid, remark],
            )?;
            *entry = remark.to_owned();
        }
        Ok(self.list())
    }

    pub(crate) fn check_scope(&self, scope: Option<&str>) -> Result<(), Error> {
        if let Some(scope) = scope {
            let matches = match self.active_uid {
                Some(uid) => scope == uid.to_string(),
                None => scope == "local",
            };
            if !matches {
                return Err(Error::AccountSelection);
            }
        }
        Ok(())
    }

    pub(crate) fn active_archive(&mut self) -> Result<&mut CharacterArchive, Error> {
        let path = self.archive_path(self.active_uid);
        match &mut self.loaded {
            Some(archive) => Ok(archive),
            loaded => Ok(loaded.insert(CharacterArchive::installed_at(&path, &self.root)?)),
        }
    }

    pub(crate) fn import_catalog(&mut self, json: &str) -> Result<Index, Error> {
        let path = self.archive_path(self.active_uid);
        let archive = CharacterArchive::import_catalog(&path, &self.root, json)?;
        let index = archive.index()?;
        self.loaded = Some(archive);
        Ok(index)
    }

    pub(crate) fn import(
        &mut self,
        snapshot: &str,
        origin: ExportOrigin,
    ) -> Result<ImportedAccount, Error> {
        let snapshot = AccountSnapshot::parse(snapshot, origin)?;
        let uid = snapshot.import_uid()?;
        // The first account inherits the original local archive, including existing manual builds.
        let binds_legacy = uid.is_some() && self.active_uid.is_none();
        let created = uid.is_some_and(|uid| !self.accounts.contains_key(&uid));
        let summary = if uid.is_none() || uid == self.active_uid || binds_legacy {
            self.active_archive()?.apply_snapshot(snapshot)?
        } else {
            let path = self.archive_path(uid);
            CharacterArchive::installed_at(&path, &self.root)?.apply_snapshot(snapshot)?
        };
        // The binding and inventory commit together. A failed first import leaves no listed account.
        if let Some(uid) = uid {
            if binds_legacy {
                self.legacy_uid = Some(uid);
                self.active_uid = Some(uid);
            }
            self.accounts.entry(uid).or_insert_with(|| uid.to_string());
        }
        Ok(ImportedAccount {
            uid,
            created,
            summary,
            selection: self.list(),
        })
    }

    fn archive_path(&self, uid: Option<u32>) -> PathBuf {
        match uid {
            Some(uid) if Some(uid) != self.legacy_uid => {
                self.root.join("accounts").join(uid.to_string())
            }
            _ => self.root.clone(),
        }
    }
}

fn open_preferences(root: &Path) -> Result<Connection, Error> {
    let mut db = Connection::open(root.join("account-settings.sqlite"))?;
    db.busy_timeout(Duration::from_secs(3))?;
    let transaction = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => transaction.execute_batch(
            "CREATE TABLE active_account (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), uid INTEGER NOT NULL) STRICT;
             CREATE TABLE account_remarks (uid INTEGER PRIMARY KEY, remark TEXT NOT NULL) STRICT;
             PRAGMA user_version = 1;",
        )?,
        1 => {},
        version => return Err(star_rail_profiles::Error::Version {
            kind: "account settings schema",
            version: version.to_string(),
        }.into()),
    }
    transaction.commit()?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use star_rail_battle_core::Session;
    use star_rail_profiles::training_catalog;

    use super::*;
    use crate::workbench_tests::TestData;

    fn prepare_catalog(root: &Path) {
        let session = Session::from_json(include_str!(
            "../../contracts/battle-session.v1.example.json"
        ))
        .unwrap();
        let mut catalog = training_catalog(&session);
        let cone = catalog
            .light_cones
            .remove("fixture.destruction_cone")
            .unwrap();
        catalog.light_cones.insert("cone.20000".into(), cone);
        let boots = catalog.relics.remove("fixture.training_boots").unwrap();
        catalog.relics.insert("relic.31024".into(), boots);
        let content = serde_json::to_string(&catalog).unwrap();
        let digest = Sha256::digest(content.as_bytes());
        let document = format!(
            "{{\"data_version\":\"sr.{}.m03.{digest:x}\",\"content\":{content}}}",
            catalog.game_version,
        );
        fs::write(root.join("catalog.json"), document).unwrap();
    }

    fn snapshot(uid: u32, level: u32) -> Value {
        json!({
            "source": "reliquary_archiver", "version": 4,
            "metadata": { "uid": uid },
            "characters": [{
                "id": "1001", "level": level.min(20), "ascension": 0,
                "eidolon": (level % 10).min(6), "skills": {}, "traces": {}
            }],
            "light_cones": [{
                "id": "20000", "level": level, "ascension": 0,
                "superimposition": 1, "location": "", "_uid": "same-cone"
            }],
            "relics": [{
                "id": "31024", "name": "Test boots", "slot": "Feet", "rarity": 2,
                "level": 3, "mainstat": "ATK",
                "substats": [{"key": "SPD", "value": 2.3, "count": 1, "step": 1}],
                "location": "", "lock": false, "discard": false, "_uid": "same-relic"
            }]
        })
    }

    fn has_saved_team(archives: &mut AccountArchives, id: &str) -> bool {
        let index = archives.active_archive().unwrap().index().unwrap();
        let index = serde_json::to_value(index).unwrap();
        index["teams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|team| team["id"] == id)
    }

    #[test]
    fn routes_imports_by_uid_and_persists_switching_and_remarks() {
        let dir = TestData::new();
        prepare_catalog(dir.path());
        let mut legacy = CharacterArchive::installed(dir.path()).unwrap();
        let local = legacy
            .save_team(None, "旧配队", &Default::default())
            .unwrap();
        legacy
            .import_account(&snapshot(123, 10).to_string(), ExportOrigin::File)
            .unwrap();
        drop(legacy);

        let mut archives = AccountArchives::open(dir.path()).unwrap();
        assert_eq!(archives.list().active_uid, Some(123));
        assert_eq!(archives.list().accounts[0].remark, "123");
        let live = json!({ "event": "InitialScan", "data": snapshot(456, 12) });
        let new = archives
            .import(&live.to_string(), ExportOrigin::Live)
            .unwrap();
        assert!(new.created);
        assert_eq!(new.uid, Some(456));
        assert_eq!(new.summary.characters_applied, 1);
        assert_eq!(new.summary.light_cones_added, 1);
        assert_eq!(new.summary.relics.added, 1);
        assert_eq!(new.selection.active_uid, Some(123));
        assert_eq!(new.selection.accounts[1].remark, "456");

        archives.rename(456, " 小号 ").unwrap();
        let update = archives
            .import(&snapshot(456, 14).to_string(), ExportOrigin::File)
            .unwrap();
        assert!(!update.created);
        assert_eq!(update.summary.characters_applied, 1);
        assert_eq!(update.summary.light_cones_added, 0);
        assert_eq!(update.summary.light_cones_updated, 1);
        assert_eq!(update.summary.relics.added, 0);
        assert_eq!(update.selection.accounts[1].remark, "小号");
        assert_eq!(
            archives
                .active_archive()
                .unwrap()
                .light_cone_inventory()
                .unwrap()[0]
                .cone
                .level,
            10
        );
        assert!(has_saved_team(&mut archives, &local.id));
        assert_eq!(
            archives
                .active_archive()
                .unwrap()
                .profile("march_7th.preservation")
                .unwrap()
                .edit
                .eidolon,
            0
        );

        archives.select(456).unwrap();
        assert!(matches!(
            archives.check_scope(Some("123")),
            Err(Error::AccountSelection)
        ));
        assert!(!has_saved_team(&mut archives, &local.id));
        assert_eq!(
            archives
                .active_archive()
                .unwrap()
                .profile("march_7th.preservation")
                .unwrap()
                .edit
                .eidolon,
            4
        );
        assert_eq!(
            archives
                .active_archive()
                .unwrap()
                .light_cone_inventory()
                .unwrap()[0]
                .cone
                .level,
            14
        );
        assert_eq!(
            archives
                .active_archive()
                .unwrap()
                .inventory()
                .unwrap()
                .len(),
            1
        );
        drop(archives);

        let mut reopened = AccountArchives::open(dir.path()).unwrap();
        assert_eq!(reopened.list().active_uid, Some(456));
        assert_eq!(reopened.list().accounts[1].remark, "小号");
        assert_eq!(
            reopened.rename(456, "  ").unwrap().accounts[1].remark,
            "456"
        );
        assert!(matches!(reopened.select(789), Err(Error::AccountSelection)));
        reopened.select(123).unwrap();
        assert!(has_saved_team(&mut reopened, &local.id));
    }

    #[test]
    fn binds_first_uid_and_ignores_failed_or_uidless_account_imports() {
        let dir = TestData::new();
        prepare_catalog(dir.path());
        let mut archives = AccountArchives::open(dir.path()).unwrap();
        let local = archives
            .active_archive()
            .unwrap()
            .save_team(None, "本地配队", &Default::default())
            .unwrap();
        let mut invalid = snapshot(123, 10);
        invalid["metadata"]["uid"] = Value::Null;
        assert!(matches!(
            archives.import(&invalid.to_string(), ExportOrigin::File),
            Err(Error::AccountUidMissing)
        ));
        assert!(archives.list().accounts.is_empty());

        let first = archives
            .import(&snapshot(123, 10).to_string(), ExportOrigin::File)
            .unwrap();
        assert!(first.created);
        assert_eq!(first.selection.active_uid, Some(123));
        assert!(has_saved_team(&mut archives, &local.id));

        let mut broken = snapshot(456, 999);
        assert!(
            archives
                .import(&broken.to_string(), ExportOrigin::File)
                .is_err()
        );
        assert_eq!(archives.list().accounts.len(), 1);
        drop(archives);

        let mut reopened = AccountArchives::open(dir.path()).unwrap();
        assert_eq!(reopened.list().accounts.len(), 1);
        broken.as_object_mut().unwrap().remove("light_cones");
        broken.as_object_mut().unwrap().remove("characters");
        broken.as_object_mut().unwrap().remove("metadata");
        let relics = reopened
            .import(&broken.to_string(), ExportOrigin::File)
            .unwrap();
        assert_eq!(relics.uid, None);
        assert_eq!(relics.selection.active_uid, Some(123));
        assert_eq!(relics.selection.accounts.len(), 1);
    }
}
