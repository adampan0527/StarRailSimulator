use std::{collections::BTreeSet, path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{Catalog, Error, FrozenCharacter, FrozenTeam, Loadout, valid_id};

const SCHEMA_VERSION: u32 = 1;
const INITIAL_PRESET: &str = "默认配装";

const SCHEMA: &str = "
CREATE TABLE archive_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    game_version TEXT NOT NULL,
    data_version TEXT NOT NULL
) STRICT;
CREATE TABLE profiles (
    character_id TEXT PRIMARY KEY,
    current_preset TEXT NOT NULL,
    FOREIGN KEY (character_id, current_preset)
        REFERENCES presets(character_id, name) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE presets (
    character_id TEXT NOT NULL REFERENCES profiles(character_id),
    name TEXT NOT NULL,
    loadout TEXT NOT NULL,
    PRIMARY KEY (character_id, name)
) STRICT;
CREATE TABLE teams (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL
) STRICT;
CREATE TABLE team_members (
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 3),
    character_id TEXT NOT NULL REFERENCES profiles(character_id),
    PRIMARY KEY (team_id, position),
    UNIQUE (team_id, character_id)
) STRICT;
";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub character_form_id: String,
    pub current_preset: String,
    pub presets: std::collections::BTreeMap<String, Loadout>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub character_form_ids: Vec<String>,
}

pub struct Archive {
    db: Connection,
    catalog: Catalog,
}

impl Archive {
    pub fn open(path: impl AsRef<Path>, catalog: Catalog) -> Result<Self, Error> {
        catalog.validate()?;
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.busy_timeout(Duration::from_secs(3))?;
        let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: u32 =
            transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                transaction.execute_batch(SCHEMA)?;
                transaction.execute(
                    "INSERT INTO archive_metadata VALUES (1, ?1, ?2)",
                    params![catalog.game_version, catalog.data_version],
                )?;
                transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }
            SCHEMA_VERSION => {
                let (game, data): (String, String) = transaction.query_row(
                    "SELECT game_version, data_version FROM archive_metadata WHERE singleton = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if (game.as_str(), data.as_str()) != (&catalog.game_version, &catalog.data_version)
                {
                    return Err(Error::Version {
                        kind: "archive catalog",
                        version: data,
                    });
                }
            }
            version => {
                return Err(Error::Version {
                    kind: "archive schema",
                    version: version.to_string(),
                });
            }
        }
        transaction.commit()?;
        Ok(Self { db, catalog })
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn ensure_profile(&mut self, character_id: &str) -> Result<Profile, Error> {
        let default = self.catalog.default_loadout(character_id)?;
        self.ensure_profile_with_loadout(character_id, INITIAL_PRESET, &default)
    }

    /// Creates a missing profile with the caller's initial preset, leaving existing profiles intact.
    pub fn ensure_profile_with_loadout(
        &mut self,
        character_id: &str,
        preset: &str,
        loadout: &Loadout,
    ) -> Result<Profile, Error> {
        validate_name("preset.name", preset)?;
        self.catalog.validate_loadout(character_id, loadout)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.execute(
            "INSERT INTO profiles (character_id, current_preset) VALUES (?1, ?2)
             ON CONFLICT(character_id) DO NOTHING",
            params![character_id, preset],
        )? == 1
        {
            put_preset(&transaction, character_id, preset, loadout)?;
        }
        let profile = read_profile(&transaction, character_id)?;
        transaction.commit()?;
        Ok(profile)
    }

    pub fn profile(&self, character_id: &str) -> Result<Profile, Error> {
        let transaction = self.db.unchecked_transaction()?;
        let profile = read_profile(&transaction, character_id)?;
        transaction.commit()?;
        Ok(profile)
    }

    pub fn profile_ids(&self) -> Result<Vec<String>, Error> {
        let mut query = self
            .db
            .prepare("SELECT character_id FROM profiles ORDER BY character_id")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Atomically saves the draft and optionally selects it as the current preset.
    pub fn save_preset(
        &mut self,
        character_id: &str,
        name: &str,
        draft: &Loadout,
        make_current: bool,
    ) -> Result<(), Error> {
        validate_name("preset.name", name)?;
        self.catalog.validate_loadout(character_id, draft)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        current_preset(&transaction, character_id)?;
        put_preset(&transaction, character_id, name, draft)?;
        if make_current {
            transaction.execute(
                "UPDATE profiles SET current_preset = ?2 WHERE character_id = ?1",
                params![character_id, name],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn select_preset(&mut self, character_id: &str, name: &str) -> Result<(), Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        current_preset(&transaction, character_id)?;
        if transaction.execute(
            "UPDATE profiles SET current_preset = ?2 WHERE character_id = ?1
             AND EXISTS (SELECT 1 FROM presets WHERE character_id = ?1 AND name = ?2)",
            params![character_id, name],
        )? == 0
        {
            return Err(Error::missing("preset", name));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn rename_preset(
        &mut self,
        character_id: &str,
        name: &str,
        new_name: &str,
    ) -> Result<(), Error> {
        validate_name("preset.name", new_name)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_preset(&transaction, character_id)?;
        if name != new_name
            && transaction
                .query_row(
                    "SELECT 1 FROM presets WHERE character_id = ?1 AND name = ?2",
                    params![character_id, new_name],
                    |row| row.get::<_, u32>(0),
                )
                .optional()?
                .is_some()
        {
            return Err(Error::invalid("preset.name", "name already exists"));
        }
        if transaction.execute(
            "UPDATE presets SET name = ?3 WHERE character_id = ?1 AND name = ?2",
            params![character_id, name, new_name],
        )? == 0
        {
            return Err(Error::missing("preset", name));
        }
        if current == name {
            transaction.execute(
                "UPDATE profiles SET current_preset = ?2 WHERE character_id = ?1",
                params![character_id, new_name],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_preset(&mut self, character_id: &str, name: &str) -> Result<(), Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if current_preset(&transaction, character_id)? == name {
            return Err(Error::invalid("preset.name", "select another preset first"));
        }
        if transaction.execute(
            "DELETE FROM presets WHERE character_id = ?1 AND name = ?2",
            params![character_id, name],
        )? == 0
        {
            return Err(Error::missing("preset", name));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn save_team(&mut self, team: &Team) -> Result<(), Error> {
        self.write_team(team, false)
    }

    /// Seeds a named team if it is missing, even if two processes initialize at once.
    pub fn ensure_team(&mut self, team: &Team) -> Result<(), Error> {
        self.write_team(team, true)
    }

    fn write_team(&mut self, team: &Team, preserve_existing: bool) -> Result<(), Error> {
        validate_name("team.name", &team.name)?;
        let member_ids = &team.character_form_ids;
        let distinct: BTreeSet<_> = member_ids.iter().collect();
        if !valid_id(&team.id) {
            return Err(Error::invalid("team.id", &team.id));
        }
        if !(1..=4).contains(&member_ids.len()) || distinct.len() != member_ids.len() {
            return Err(Error::invalid(
                "team.members",
                "expected one to four distinct characters",
            ));
        }
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if preserve_existing
            && transaction
                .query_row("SELECT 1 FROM teams WHERE id = ?1", [&team.id], |row| {
                    row.get::<_, u8>(0)
                })
                .optional()?
                .is_some()
        {
            return Ok(());
        }
        for id in member_ids {
            current_preset(&transaction, id)?;
        }
        transaction.execute(
            "INSERT INTO teams (id, name) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET name = excluded.name",
            params![team.id, team.name],
        )?;
        transaction.execute("DELETE FROM team_members WHERE team_id = ?1", [&team.id])?;
        for (position, id) in (0_u16..).zip(member_ids) {
            transaction.execute(
                "INSERT INTO team_members (team_id, position, character_id) VALUES (?1, ?2, ?3)",
                params![team.id, position, id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn team(&self, id: &str) -> Result<Team, Error> {
        let transaction = self.db.unchecked_transaction()?;
        let name = team_name(&transaction, id)?;
        let members = {
            let mut query = transaction.prepare(
                "SELECT character_id FROM team_members WHERE team_id = ?1 ORDER BY position",
            )?;
            query
                .query_map([id], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        transaction.commit()?;
        Ok(Team {
            id: id.into(),
            name,
            character_form_ids: members,
        })
    }

    pub fn team_ids(&self) -> Result<Vec<String>, Error> {
        let mut query = self.db.prepare("SELECT id FROM teams ORDER BY id")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    pub fn delete_team(&mut self, id: &str) -> Result<(), Error> {
        if self.db.execute("DELETE FROM teams WHERE id = ?1", [id])? == 0 {
            return Err(Error::missing("team", id));
        }
        Ok(())
    }

    pub fn freeze_team(&self, id: &str) -> Result<FrozenTeam, Error> {
        let transaction = self.db.unchecked_transaction()?;
        team_name(&transaction, id)?;
        let members = {
            let mut query = transaction.prepare(
                "SELECT m.character_id, b.loadout
                 FROM team_members m JOIN profiles p ON p.character_id = m.character_id
                 JOIN presets b ON b.character_id = p.character_id AND b.name = p.current_preset
                 WHERE m.team_id = ?1 ORDER BY m.position",
            )?;
            let rows = query.query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.map(|row| {
                let (character_form_id, json) = row?;
                let loadout = serde_json::from_str(&json)?;
                self.catalog
                    .validate_loadout(&character_form_id, &loadout)?;
                Ok(FrozenCharacter {
                    character_form_id,
                    loadout,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?
        };
        transaction.commit()?;
        if members.is_empty() {
            return Err(Error::invalid("team.members", "team has no members"));
        }
        Ok(FrozenTeam::new(&self.catalog, members))
    }
}

fn validate_name(field: &'static str, name: &str) -> Result<(), Error> {
    if name.trim().is_empty() || name.trim() != name || name.chars().any(char::is_control) {
        return Err(Error::invalid(field, name));
    }
    Ok(())
}

fn current_preset(db: &Connection, id: &str) -> Result<String, Error> {
    db.query_row(
        "SELECT current_preset FROM profiles WHERE character_id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| Error::missing("profile", id))
}

fn team_name(db: &Connection, id: &str) -> Result<String, Error> {
    db.query_row("SELECT name FROM teams WHERE id = ?1", [id], |row| {
        row.get(0)
    })
    .optional()?
    .ok_or_else(|| Error::missing("team", id))
}

fn put_preset(db: &Connection, id: &str, name: &str, loadout: &Loadout) -> Result<(), Error> {
    db.execute(
        "INSERT INTO presets (character_id, name, loadout) VALUES (?1, ?2, ?3)
         ON CONFLICT(character_id, name) DO UPDATE SET loadout = excluded.loadout",
        params![id, name, serde_json::to_string(loadout)?],
    )?;
    Ok(())
}

fn read_profile(db: &Connection, id: &str) -> Result<Profile, Error> {
    let current_preset = current_preset(db, id)?;
    let mut query = db.prepare("SELECT name, loadout FROM presets WHERE character_id = ?1")?;
    let rows = query.query_map([id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let presets = rows
        .map(|row| {
            let (name, json) = row?;
            Ok((name, serde_json::from_str(&json)?))
        })
        .collect::<Result<_, Error>>()?;
    Ok(Profile {
        character_form_id: id.into(),
        current_preset,
        presets,
    })
}
