use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::{
    Archive, INITIAL_PRESET, insert_profile_if_missing, light_cones, put_preset, read_profile,
};
use crate::{Catalog, Error, LightConeEquipment, Loadout, Relic, RelicSlot, Relics};

pub(super) const SCHEMA: &str = "
CREATE TABLE relic_inventory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slot INTEGER NOT NULL CHECK (slot BETWEEN 0 AND 5),
    relic TEXT NOT NULL,
    UNIQUE (id, slot)
) STRICT;
CREATE TABLE equipped_relics (
    character_id TEXT NOT NULL REFERENCES profiles(character_id),
    slot INTEGER NOT NULL,
    relic_id INTEGER NOT NULL UNIQUE,
    PRIMARY KEY (character_id, slot),
    FOREIGN KEY (relic_id, slot) REFERENCES relic_inventory(id, slot)
) STRICT;
";

pub(super) const IMPORT_SCHEMA: &str = "
CREATE TABLE imported_relics (
    source_uid TEXT NOT NULL,
    source_relic TEXT NOT NULL,
    relic_id INTEGER NOT NULL UNIQUE REFERENCES relic_inventory(id) ON DELETE CASCADE,
    PRIMARY KEY (source_uid, source_relic)
) STRICT;
";

pub(super) const IMPORT_DETAILS_SCHEMA: &str =
    "ALTER TABLE imported_relics ADD COLUMN source_details TEXT;";

const SLOTS: [RelicSlot; 6] = [
    RelicSlot::Head,
    RelicSlot::Hands,
    RelicSlot::Body,
    RelicSlot::Feet,
    RelicSlot::PlanarSphere,
    RelicSlot::LinkRope,
];

fn slot_number(slot: RelicSlot) -> u8 {
    match slot {
        RelicSlot::Head => 0,
        RelicSlot::Hands => 1,
        RelicSlot::Body => 2,
        RelicSlot::Feet => 3,
        RelicSlot::PlanarSphere => 4,
        RelicSlot::LinkRope => 5,
    }
}

#[derive(Serialize)]
pub struct InventoryRelic {
    pub id: u32,
    pub relic: Relic,
    pub owner_id: Option<String>,
    pub source: Option<RelicSource>,
}

pub struct ImportedRelic {
    pub source_uid: String,
    pub relic: Relic,
    pub source: RelicSource,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct RelicSource {
    pub location: String,
    pub rolls: BTreeMap<String, RelicRolls>,
    /// Unlocked by future enhancements. Values are in millionths.
    pub preview_substats: Option<BTreeMap<String, u32>>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct RelicRolls {
    pub count: u32,
    pub step: u32,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct RelicImportSummary {
    pub added: usize,
    pub equipped: usize,
}

impl Archive {
    pub fn inventory(&self) -> Result<Vec<InventoryRelic>, Error> {
        let mut query = self.db.prepare(
            "SELECT i.id, i.relic, e.character_id, s.source_details, i.relic = s.source_relic
             FROM relic_inventory i
             LEFT JOIN equipped_relics e ON i.id = e.relic_id
             LEFT JOIN imported_relics s ON i.id = s.relic_id ORDER BY i.id DESC",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<bool>>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, json, owner_id, details, matches_source) = row?;
            let source = details
                .map(|details| serde_json::from_str::<RelicSource>(&details))
                .transpose()?
                .map(|mut source| {
                    if matches_source == Some(false) {
                        source.rolls.clear();
                        source.preview_substats = None;
                    }
                    source
                });
            Ok(InventoryRelic {
                id,
                relic: serde_json::from_str(&json)?,
                owner_id,
                source,
            })
        })
        .collect()
    }

    pub fn save_inventory_relic(&mut self, id: Option<u32>, relic: &Relic) -> Result<(), Error> {
        let (slot, json) = self.inventory_entry(relic)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(id) = id {
            if owner(&transaction, id)?.is_some() && inventory_slot(&transaction, id)? != slot {
                return Err(Error::invalid("inventory.equipped", id.to_string()));
            }
            if transaction.execute(
                "UPDATE relic_inventory SET slot = ?2, relic = ?3 WHERE id = ?1",
                params![id, slot, json],
            )? == 0
            {
                return Err(Error::missing("inventory relic", id.to_string()));
            }
        } else {
            transaction.execute(
                "INSERT INTO relic_inventory (slot, relic) VALUES (?1, ?2)",
                params![slot, json],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Refreshes source details and fills empty equipment slots when first recording a source
    /// snapshot. Matching instances retain their inventory IDs and local edits.
    pub fn import_inventory_relics(
        &mut self,
        relics: &[ImportedRelic],
    ) -> Result<RelicImportSummary, Error> {
        let characters_by_source: BTreeMap<_, _> = self
            .catalog
            .characters
            .iter()
            .flat_map(|(id, character)| {
                character
                    .source_ids
                    .iter()
                    .map(move |source| (source.as_str(), id.as_str()))
            })
            .collect();
        let entries = relics
            .iter()
            .map(|item| self.inventory_entry(&item.relic).map(|entry| (item, entry)))
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut summary = RelicImportSummary::default();
        {
            let mut imported = transaction.prepare(
                "SELECT s.relic_id, s.source_details, i.relic = s.source_relic
                 FROM imported_relics s JOIN relic_inventory i ON i.id = s.relic_id
                 WHERE s.source_uid = ?1 AND s.source_relic = ?2",
            )?;
            let mut inventory =
                transaction.prepare("INSERT INTO relic_inventory (slot, relic) VALUES (?1, ?2)")?;
            let mut source = transaction.prepare(
                "INSERT INTO imported_relics (source_uid, source_relic, relic_id) VALUES (?1, ?2, ?3)",
            )?;
            let mut update_source = transaction
                .prepare("UPDATE imported_relics SET source_details = ?2 WHERE relic_id = ?1")?;
            for (item, (slot, json)) in entries {
                let uid = &item.source_uid;
                let row = imported
                    .query_row(params![uid, json], |row| {
                        Ok((
                            row.get::<_, u32>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, bool>(2)?,
                        ))
                    })
                    .optional()?;
                let (id, previous, matches_source) = if let Some(row) = row {
                    row
                } else {
                    inventory.execute(params![slot, json])?;
                    let id = transaction.last_insert_rowid();
                    source.execute(params![uid, json, id])?;
                    summary.added += 1;
                    (
                        u32::try_from(id)
                            .map_err(|_| Error::invalid("inventory.id", id.to_string()))?,
                        None,
                        true,
                    )
                };

                if previous.is_none()
                    && matches_source
                    && let Some(&character_id) =
                        characters_by_source.get(item.source.location.as_str())
                {
                    summary.equipped +=
                        equip_imported_relic(&transaction, &self.catalog, character_id, slot, id)?;
                }

                let mut details = item.source.clone();
                if details.preview_substats.is_none()
                    && let Some(previous) = previous
                {
                    details.preview_substats =
                        serde_json::from_str::<RelicSource>(&previous)?.preview_substats;
                }
                update_source.execute(params![id, serde_json::to_string(&details)?])?;
            }
        }
        transaction.commit()?;
        Ok(summary)
    }

    fn inventory_entry(&self, relic: &Relic) -> Result<(u8, String), Error> {
        let data = self
            .catalog
            .relics
            .get(&relic.data_id)
            .ok_or_else(|| Error::missing("relic", &relic.data_id))?;
        self.catalog.validate_relic(data.slot, relic)?;
        Ok((slot_number(data.slot), serde_json::to_string(relic)?))
    }

    pub fn delete_inventory_relic(&mut self, id: u32) -> Result<(), Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if owner(&transaction, id)?.is_some() {
            return Err(Error::invalid("inventory.equipped", id.to_string()));
        }
        if transaction.execute("DELETE FROM relic_inventory WHERE id = ?1", [id])? == 0 {
            return Err(Error::missing("inventory relic", id.to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn equipped_relic_ids(&self, id: &str) -> Result<BTreeMap<RelicSlot, u32>, Error> {
        let mut query = self
            .db
            .prepare("SELECT slot, relic_id FROM equipped_relics WHERE character_id = ?1")?;
        Ok(query
            .query_map([id], |row| {
                Ok((SLOTS[usize::from(row.get::<_, u8>(0)?)], row.get(1)?))
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Saves the active progression, light cone and all six inventory assignments together.
    pub fn save_character_equipment(
        &mut self,
        id: &str,
        eidolon: u8,
        affinitive: bool,
        light_cone: Option<LightConeEquipment>,
        equipment: &BTreeMap<RelicSlot, u32>,
    ) -> Result<(), Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let profile = read_profile(&transaction, id)?;
        let mut active = profile.presets[&profile.current_preset].clone();
        active.config.eidolon = eidolon;
        active.affinitive_eidolon = affinitive;
        active.config.light_cone = light_cones::write_equipment(&transaction, id, light_cone)?;
        active.config.relics = Relics::default();
        self.catalog.validate_loadout(id, &active)?;

        for (&slot, &relic_id) in equipment {
            if slot_number(slot) != inventory_slot(&transaction, relic_id)? {
                return Err(Error::invalid("relic.slot", relic_id.to_string()));
            }
            if owner(&transaction, relic_id)?.is_some_and(|owner_id| owner_id != id) {
                return Err(Error::invalid("inventory.occupied", relic_id.to_string()));
            }
        }
        transaction.execute("DELETE FROM equipped_relics WHERE character_id = ?1", [id])?;
        for (&slot, &relic_id) in equipment {
            transaction.execute(
                "INSERT INTO equipped_relics VALUES (?1, ?2, ?3)",
                params![id, slot_number(slot), relic_id],
            )?;
        }
        put_preset(&transaction, id, &profile.current_preset, &active)?;
        transaction.commit()?;
        Ok(())
    }
}

fn equip_imported_relic(
    db: &Connection,
    catalog: &Catalog,
    character_id: &str,
    slot: u8,
    relic_id: u32,
) -> Result<usize, Error> {
    let current = db
        .query_row(
            "SELECT b.loadout FROM profiles p JOIN presets b
         ON b.character_id = p.character_id AND b.name = p.current_preset
         WHERE p.character_id = ?1",
            [character_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let active = if let Some(current) = current {
        serde_json::from_str::<Loadout>(&current)?
    } else {
        let default = catalog.default_loadout(character_id)?;
        insert_profile_if_missing(db, character_id, INITIAL_PRESET, &default)?;
        default
    };
    // Equipping an inventory item would replace all inline relics in a legacy preset.
    if active.config.relics != Relics::default() {
        return Ok(0);
    }
    Ok(db.execute(
        "INSERT INTO equipped_relics VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
        params![character_id, slot, relic_id],
    )?)
}

fn owner(db: &Connection, id: u32) -> Result<Option<String>, Error> {
    Ok(db
        .query_row(
            "SELECT character_id FROM equipped_relics WHERE relic_id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

fn inventory_slot(db: &Connection, id: u32) -> Result<u8, Error> {
    db.query_row(
        "SELECT slot FROM relic_inventory WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| Error::missing("inventory relic", id.to_string()))
}

pub(super) fn apply_equipment(
    db: &Connection,
    id: &str,
    loadout: &mut Loadout,
) -> Result<(), Error> {
    light_cones::apply_equipment(db, id, loadout)?;
    let mut query = db.prepare(
        "SELECT e.slot, i.relic FROM equipped_relics e JOIN relic_inventory i ON i.id = e.relic_id
         WHERE e.character_id = ?1",
    )?;
    let mut rows = query.query([id])?;
    let mut relics = Relics::default();
    let mut has_inventory = false;
    while let Some(row) = rows.next()? {
        has_inventory = true;
        let item = serde_json::from_str(&row.get::<_, String>(1)?)?;
        match SLOTS[usize::from(row.get::<_, u8>(0)?)] {
            RelicSlot::Head => relics.head = Some(item),
            RelicSlot::Hands => relics.hands = Some(item),
            RelicSlot::Body => relics.body = Some(item),
            RelicSlot::Feet => relics.feet = Some(item),
            RelicSlot::PlanarSphere => relics.planar_sphere = Some(item),
            RelicSlot::LinkRope => relics.link_rope = Some(item),
        }
    }
    if has_inventory {
        loadout.config.relics = relics;
    }
    Ok(())
}
