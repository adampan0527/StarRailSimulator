use std::{collections::BTreeMap, path::Path};

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;

use super::{
    Archive, INITIAL_PRESET, ImportedRelic, RelicImportSummary, insert_profile_if_missing,
    inventory, put_preset,
};
use crate::{Catalog, Error, LightCone, Loadout};

pub(super) const SCHEMA: &str = "
CREATE TABLE archiver_account (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    uid INTEGER NOT NULL
) STRICT;
CREATE TABLE archiver_progression (
    character_id TEXT PRIMARY KEY REFERENCES profiles(character_id),
    loadout TEXT NOT NULL
) STRICT;
CREATE TABLE archiver_light_cones (
    account_uid INTEGER NOT NULL,
    source_uid TEXT NOT NULL,
    cone_id INTEGER NOT NULL UNIQUE REFERENCES light_cone_inventory(id) ON DELETE CASCADE,
    source_cone TEXT NOT NULL,
    PRIMARY KEY (account_uid, source_uid)
) STRICT;
CREATE TABLE archiver_relics (
    source_uid TEXT PRIMARY KEY,
    relic_id INTEGER NOT NULL UNIQUE REFERENCES imported_relics(relic_id) ON DELETE CASCADE
) STRICT;
";

#[derive(Default)]
pub struct AccountInventory {
    pub uid: Option<u32>,
    pub characters: BTreeMap<String, Loadout>,
    pub light_cones: Vec<ImportedLightCone>,
    pub relics: Vec<ImportedRelic>,
    pub skipped_characters: usize,
    pub skipped_light_cones: usize,
}

pub struct ImportedLightCone {
    pub source_uid: String,
    pub owner_id: Option<String>,
    pub cone: LightCone,
}

#[derive(Default, Serialize)]
pub struct AccountImportSummary {
    pub characters_applied: usize,
    pub characters_preserved: usize,
    pub characters_skipped: usize,
    pub light_cones_added: usize,
    pub light_cones_updated: usize,
    pub light_cones_equipped: usize,
    pub light_cones_skipped: usize,
    pub relics: RelicImportSummary,
    pub relics_updated: usize,
}

impl Archive {
    /// Reads the binding of an existing archive without opening its catalog or migrating it.
    pub fn stored_account_uid(path: &Path) -> Result<Option<u32>, Error> {
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let has_binding: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'archiver_account' AND type = 'table')",
            [],
            |row| row.get(0),
        )?;
        if has_binding {
            bound_account(&db)
        } else {
            Ok(None)
        }
    }

    /// Applies the account snapshot together with its inventory in one transaction.
    pub fn import_account(
        &mut self,
        account: &AccountInventory,
    ) -> Result<AccountImportSummary, Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(uid) = account.uid {
            bind_account(&transaction, uid)?;
        }
        let mut summary = AccountImportSummary {
            characters_skipped: account.skipped_characters,
            light_cones_skipped: account.skipped_light_cones,
            ..AccountImportSummary::default()
        };
        for (id, loadout) in &account.characters {
            match import_progression(&transaction, &self.catalog, id, loadout)? {
                Some(true) => summary.characters_applied += 1,
                Some(false) => {}
                None => summary.characters_preserved += 1,
            }
        }
        for item in &account.light_cones {
            import_light_cone(
                &transaction,
                &self.catalog,
                account.uid.unwrap_or_default(),
                item,
                &mut summary,
            )?;
        }
        (summary.relics, summary.relics_updated) = inventory::import_relics(
            &transaction,
            &self.catalog,
            &account.relics,
            account.uid.is_some(),
        )?;
        transaction.commit()?;
        Ok(summary)
    }
}

fn bind_account(db: &Connection, uid: u32) -> Result<(), Error> {
    match bound_account(db)? {
        Some(previous) if previous != uid => Err(Error::invalid("archiver.uid", uid.to_string())),
        Some(_) => Ok(()),
        None => {
            db.execute("INSERT INTO archiver_account VALUES (1, ?1)", [uid])?;
            Ok(())
        }
    }
}

fn bound_account(db: &Connection) -> Result<Option<u32>, Error> {
    Ok(db
        .query_row(
            "SELECT uid FROM archiver_account WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()?)
}

fn active_preset(db: &Connection, id: &str) -> Result<Option<(String, Loadout)>, Error> {
    let row = db
        .query_row(
            "SELECT p.current_preset, b.loadout FROM profiles p JOIN presets b
         ON b.character_id = p.character_id AND b.name = p.current_preset
         WHERE p.character_id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    row.map(|(name, json)| Ok((name, serde_json::from_str(&json)?)))
        .transpose()
}

// Selection and inline equipment belong to the local profile. Game progression can refresh
// while the initial preset still matches its catalog defaults or last applied game values.
fn import_progression(
    db: &Connection,
    catalog: &Catalog,
    id: &str,
    source: &Loadout,
) -> Result<Option<bool>, Error> {
    catalog.validate_loadout(id, source)?;
    let applied = if let Some((name, mut current)) = active_preset(db, id)? {
        if name != INITIAL_PRESET {
            return Ok(None);
        }
        let previous: Option<String> = db
            .query_row(
                "SELECT loadout FROM archiver_progression WHERE character_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        let previous = match previous {
            Some(json) => serde_json::from_str(&json)?,
            None => catalog.default_loadout(id)?,
        };
        let cone = current.config.light_cone.take();
        let relics = std::mem::take(&mut current.config.relics);
        if current != previous {
            return Ok(None);
        }
        if &current == source {
            false
        } else {
            let mut imported = source.clone();
            imported.config.light_cone = cone;
            imported.config.relics = relics;
            put_preset(db, id, INITIAL_PRESET, &imported)?;
            true
        }
    } else {
        insert_profile_if_missing(db, id, INITIAL_PRESET, source)?;
        true
    };
    db.execute(
        "INSERT INTO archiver_progression VALUES (?1, ?2)
         ON CONFLICT(character_id) DO UPDATE SET loadout = excluded.loadout",
        params![id, serde_json::to_string(source)?],
    )?;
    Ok(Some(applied))
}

fn import_light_cone(
    db: &Connection,
    catalog: &Catalog,
    uid: u32,
    item: &ImportedLightCone,
    summary: &mut AccountImportSummary,
) -> Result<(), Error> {
    catalog.validate_light_cone(&item.cone)?;
    let json = serde_json::to_string(&item.cone)?;
    let previous = db.query_row(
        "SELECT cone_id, source_cone FROM archiver_light_cones WHERE account_uid = ?1 AND source_uid = ?2",
        params![uid, item.source_uid],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    ).optional()?;
    if let Some((id, previous)) = previous {
        if previous != json {
            summary.light_cones_updated += db.execute(
                "UPDATE light_cone_inventory SET cone = ?2 WHERE id = ?1 AND cone = ?3",
                params![id, json, previous],
            )?;
            db.execute(
                "UPDATE archiver_light_cones SET source_cone = ?2 WHERE cone_id = ?1",
                params![id, json],
            )?;
        }
        return Ok(());
    }
    db.execute(
        "INSERT INTO light_cone_inventory (cone) VALUES (?1)",
        [&json],
    )?;
    let id = db.last_insert_rowid();
    db.execute(
        "INSERT INTO archiver_light_cones VALUES (?1, ?2, ?3, ?4)",
        params![uid, item.source_uid, id, json],
    )?;
    summary.light_cones_added += 1;
    if let Some(owner) = &item.owner_id {
        let inline_equipped = if let Some((_, preset)) = active_preset(db, owner)? {
            preset.config.light_cone.is_some()
        } else {
            insert_profile_if_missing(db, owner, INITIAL_PRESET, &catalog.default_loadout(owner)?)?;
            false
        };
        if !inline_equipped {
            summary.light_cones_equipped += db.execute(
                "INSERT INTO equipped_light_cones VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                params![owner, id],
            )?;
        }
    }
    Ok(())
}
