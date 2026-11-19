use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{Catalog, Error, FrozenCharacter, FrozenTeam, Loadout, valid_id};

mod archiver;
mod inventory;
mod light_cones;
pub use archiver::{AccountImportSummary, AccountInventory, ImportedLightCone};
pub use inventory::{ImportedRelic, InventoryRelic, RelicImportSummary, RelicRolls, RelicSource};
pub use light_cones::{InventoryLightCone, LightConeEquipment};

const SCHEMA_VERSION: u32 = 6;
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
            }
            1..=SCHEMA_VERSION => {
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
        if version < 2 {
            transaction.execute_batch(inventory::SCHEMA)?;
        }
        if version < 3 {
            transaction.execute_batch(light_cones::SCHEMA)?;
        }
        if version < 4 {
            transaction.execute_batch(inventory::IMPORT_SCHEMA)?;
        }
        if version < 5 {
            transaction.execute_batch(inventory::IMPORT_DETAILS_SCHEMA)?;
        }
        if version < 6 {
            transaction.execute_batch(archiver::SCHEMA)?;
        }
        if version < SCHEMA_VERSION {
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
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
        insert_profile_if_missing(&transaction, character_id, preset, loadout)?;
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

    /// Resolves the current presets of saved characters and their equipped inventory values.
    pub fn active_loadouts(&self) -> Result<BTreeMap<String, Loadout>, Error> {
        let transaction = self.db.unchecked_transaction()?;
        let loadouts = {
            let mut query = transaction.prepare(
                "SELECT p.character_id, b.loadout FROM profiles p
                 JOIN presets b ON b.character_id = p.character_id AND b.name = p.current_preset",
            )?;
            let rows = query.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.map(|row| {
                let (id, json) = row?;
                let mut loadout = serde_json::from_str(&json)?;
                inventory::apply_equipment(&transaction, &id, &mut loadout)?;
                Ok((id, loadout))
            })
            .collect::<Result<_, Error>>()?
        };
        transaction.commit()?;
        Ok(loadouts)
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
        self.write_dense_team(team, false)
    }

    /// Seeds a named team if it is missing, even if two processes initialize at once.
    pub fn ensure_team(&mut self, team: &Team) -> Result<(), Error> {
        self.write_dense_team(team, true)
    }

    fn write_dense_team(&mut self, team: &Team, preserve_existing: bool) -> Result<(), Error> {
        if !(1..=4).contains(&team.character_form_ids.len()) {
            return Err(Error::invalid(
                "team.members",
                "expected one to four distinct characters",
            ));
        }
        let slots = std::array::from_fn(|i| team.character_form_ids.get(i).cloned());
        let mode = if preserve_existing {
            TeamWrite::Ensure
        } else {
            TeamWrite::Upsert
        };
        self.write_slots(&team.id, &team.name, &slots, mode)
    }

    pub fn save_team_slots(
        &mut self,
        id: &str,
        name: &str,
        slots: &TeamSlots,
    ) -> Result<(), Error> {
        self.write_slots(id, name, slots, TeamWrite::Upsert)
    }

    pub fn save_named_team(
        &mut self,
        id: Option<&str>,
        name: &str,
        slots: &TeamSlots,
    ) -> Result<SavedTeam, Error> {
        let generated;
        let (id, mode) = match id {
            Some(id) => (id, TeamWrite::Update),
            None => {
                generated = self.db.query_row(
                    "SELECT 'team.' || lower(hex(randomblob(16)))",
                    [],
                    |row| row.get::<_, String>(0),
                )?;
                (generated.as_str(), TeamWrite::Create)
            }
        };
        self.write_slots(id, name, slots, mode)?;
        Ok(SavedTeam {
            id: id.into(),
            name: name.into(),
            slots: slots.clone(),
        })
    }

    pub fn saved_teams(&self) -> Result<Vec<SavedTeam>, Error> {
        let mut query = self.db.prepare(
            "SELECT t.id, t.name, m.position, m.character_id
             FROM teams t LEFT JOIN team_members m ON t.id = m.team_id
             ORDER BY t.rowid, m.position",
        )?;
        let mut rows = query.query([])?;
        let mut teams: Vec<SavedTeam> = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            if teams.last().is_none_or(|team| team.id != id) {
                teams.push(SavedTeam {
                    id,
                    name: row.get(1)?,
                    slots: TeamSlots::default(),
                });
            }
            if let (Some(team), Some(position)) = (teams.last_mut(), row.get::<_, Option<u8>>(2)?) {
                team.slots[usize::from(position)] = row.get(3)?;
            }
        }
        Ok(teams)
    }

    fn write_slots(
        &mut self,
        id: &str,
        name: &str,
        slots: &TeamSlots,
        mode: TeamWrite,
    ) -> Result<(), Error> {
        validate_name("team.name", name)?;
        if !valid_id(id) {
            return Err(Error::invalid("team.id", id));
        }
        let mut distinct = BTreeSet::new();
        if !slots.iter().flatten().all(|member| distinct.insert(member)) {
            return Err(Error::invalid("team.members", "duplicate character"));
        }
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        match mode {
            TeamWrite::Ensure => match team_name(&transaction, id) {
                Ok(_) => return Ok(()),
                Err(Error::NotFound { kind: "team", .. }) => {}
                Err(error) => return Err(error),
            },
            TeamWrite::Update => {
                team_name(&transaction, id)?;
            }
            TeamWrite::Create | TeamWrite::Upsert => {}
        }
        for member in slots.iter().flatten() {
            match current_preset(&transaction, member) {
                Ok(_) => {}
                Err(Error::NotFound {
                    kind: "profile", ..
                }) if matches!(mode, TeamWrite::Create | TeamWrite::Update) => {
                    let loadout = self.catalog.default_loadout(member)?;
                    insert_profile_if_missing(&transaction, member, INITIAL_PRESET, &loadout)?;
                }
                Err(error) => return Err(error),
            }
        }
        let statement = match mode {
            TeamWrite::Create | TeamWrite::Ensure => "INSERT INTO teams (id, name) VALUES (?1, ?2)",
            TeamWrite::Update => "UPDATE teams SET name = ?2 WHERE id = ?1",
            TeamWrite::Upsert => {
                "INSERT INTO teams (id, name) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET name = excluded.name"
            }
        };
        transaction.execute(statement, params![id, name])?;
        transaction.execute("DELETE FROM team_members WHERE team_id = ?1", [id])?;
        for (position, member) in slots.iter().enumerate() {
            if let Some(member) = member {
                transaction.execute(
                    "INSERT INTO team_members (team_id, position, character_id) VALUES (?1, ?2, ?3)",
                    params![id, position as u8, member],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn team_slots(&self, id: &str) -> Result<TeamSlots, Error> {
        let transaction = self.db.unchecked_transaction()?;
        team_name(&transaction, id)?;
        let mut slots = TeamSlots::default();
        {
            let mut query = transaction
                .prepare("SELECT position, character_id FROM team_members WHERE team_id = ?1")?;
            let mut rows = query.query([id])?;
            while let Some(row) = rows.next()? {
                let position: u8 = row.get(0)?;
                slots[usize::from(position)] = row.get(1)?;
            }
        }
        transaction.commit()?;
        Ok(slots)
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
                let mut loadout: Loadout = serde_json::from_str(&json)?;
                inventory::apply_equipment(&transaction, &character_form_id, &mut loadout)?;
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

pub type TeamSlots = [Option<String>; 4];

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SavedTeam {
    pub id: String,
    pub name: String,
    pub slots: TeamSlots,
}

enum TeamWrite {
    Create,
    Update,
    Ensure,
    Upsert,
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

fn insert_profile_if_missing(
    db: &Connection,
    character_id: &str,
    preset: &str,
    loadout: &Loadout,
) -> Result<(), Error> {
    if db.execute(
        "INSERT INTO profiles (character_id, current_preset) VALUES (?1, ?2)
         ON CONFLICT(character_id) DO NOTHING",
        params![character_id, preset],
    )? == 1
    {
        put_preset(db, character_id, preset, loadout)?;
    }
    Ok(())
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
    let mut presets: BTreeMap<String, Loadout> = rows
        .map(|row| {
            let (name, json) = row?;
            Ok((name, serde_json::from_str(&json)?))
        })
        .collect::<Result<_, Error>>()?;
    if let Some(active) = presets.get_mut(&current_preset) {
        inventory::apply_equipment(db, id, active)?;
    }
    Ok(Profile {
        character_form_id: id.into(),
        current_preset,
        presets,
    })
}
