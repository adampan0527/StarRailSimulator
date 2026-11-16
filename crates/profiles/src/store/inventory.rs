use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;

use super::{Archive, light_cones, put_preset, read_profile};
use crate::{Error, LightConeEquipment, Loadout, Relic, RelicSlot, Relics};

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
}

impl Archive {
    pub fn inventory(&self) -> Result<Vec<InventoryRelic>, Error> {
        let mut query = self.db.prepare(
            "SELECT i.id, i.relic, e.character_id FROM relic_inventory i
             LEFT JOIN equipped_relics e ON i.id = e.relic_id ORDER BY i.id DESC",
        )?;
        let rows = query.query_map([], |row| {
            Ok((row.get(0)?, row.get::<_, String>(1)?, row.get(2)?))
        })?;
        rows.map(|row| {
            let (id, json, owner_id) = row?;
            Ok(InventoryRelic {
                id,
                relic: serde_json::from_str(&json)?,
                owner_id,
            })
        })
        .collect()
    }

    pub fn save_inventory_relic(&mut self, id: Option<u32>, relic: &Relic) -> Result<(), Error> {
        let data = self
            .catalog
            .relics
            .get(&relic.data_id)
            .ok_or_else(|| Error::missing("relic", &relic.data_id))?;
        self.catalog.validate_relic(data.slot, relic)?;
        let slot = slot_number(data.slot);
        let json = serde_json::to_string(relic)?;
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
