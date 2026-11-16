use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::Archive;
use crate::{Error, LightCone, Loadout};

pub(super) const SCHEMA: &str = "
CREATE TABLE light_cone_inventory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cone TEXT NOT NULL
) STRICT;
CREATE TABLE equipped_light_cones (
    character_id TEXT PRIMARY KEY REFERENCES profiles(character_id),
    cone_id INTEGER NOT NULL UNIQUE REFERENCES light_cone_inventory(id)
) STRICT;
";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "source",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LightConeEquipment {
    Custom(LightCone),
    Inventory(u32),
}

#[derive(Serialize)]
pub struct InventoryLightCone {
    pub id: u32,
    pub cone: LightCone,
    pub owner_id: Option<String>,
}

impl Archive {
    pub fn light_cone_inventory(&self) -> Result<Vec<InventoryLightCone>, Error> {
        let mut query = self.db.prepare(
            "SELECT i.id, i.cone, e.character_id FROM light_cone_inventory i
             LEFT JOIN equipped_light_cones e ON i.id = e.cone_id ORDER BY i.id DESC",
        )?;
        let rows = query.query_map([], |row| {
            Ok((row.get(0)?, row.get::<_, String>(1)?, row.get(2)?))
        })?;
        rows.map(|row| {
            let (id, json, owner_id) = row?;
            Ok(InventoryLightCone {
                id,
                cone: serde_json::from_str(&json)?,
                owner_id,
            })
        })
        .collect()
    }

    pub fn save_inventory_light_cone(
        &mut self,
        id: Option<u32>,
        cone: &LightCone,
    ) -> Result<(), Error> {
        self.catalog.validate_light_cone(cone)?;
        let json = serde_json::to_string(cone)?;
        if let Some(id) = id {
            if self.db.execute(
                "UPDATE light_cone_inventory SET cone = ?2 WHERE id = ?1",
                params![id, json],
            )? == 0
            {
                return Err(Error::missing("inventory light cone", id.to_string()));
            }
        } else {
            self.db.execute(
                "INSERT INTO light_cone_inventory (cone) VALUES (?1)",
                [json],
            )?;
        }
        Ok(())
    }

    pub fn delete_inventory_light_cone(&mut self, id: u32) -> Result<(), Error> {
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if owner(&transaction, id)?.is_some() {
            return Err(Error::invalid("light_cone.equipped", id.to_string()));
        }
        if transaction.execute("DELETE FROM light_cone_inventory WHERE id = ?1", [id])? == 0 {
            return Err(Error::missing("inventory light cone", id.to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn equipped_light_cone_id(&self, character_id: &str) -> Result<Option<u32>, Error> {
        Ok(self
            .db
            .query_row(
                "SELECT cone_id FROM equipped_light_cones WHERE character_id = ?1",
                [character_id],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn owner(db: &Connection, id: u32) -> Result<Option<String>, Error> {
    Ok(db
        .query_row(
            "SELECT character_id FROM equipped_light_cones WHERE cone_id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

pub(super) fn write_equipment(
    db: &Connection,
    character_id: &str,
    equipment: Option<LightConeEquipment>,
) -> Result<Option<LightCone>, Error> {
    db.execute(
        "DELETE FROM equipped_light_cones WHERE character_id = ?1",
        [character_id],
    )?;
    match equipment {
        None => Ok(None),
        Some(LightConeEquipment::Custom(cone)) => Ok(Some(cone)),
        Some(LightConeEquipment::Inventory(id)) => {
            if owner(db, id)?.is_some() {
                return Err(Error::invalid("light_cone.occupied", id.to_string()));
            }
            let exists = db
                .query_row(
                    "SELECT 1 FROM light_cone_inventory WHERE id = ?1",
                    [id],
                    |_| Ok(()),
                )
                .optional()?;
            if exists.is_none() {
                return Err(Error::missing("inventory light cone", id.to_string()));
            }
            db.execute(
                "INSERT INTO equipped_light_cones VALUES (?1, ?2)",
                params![character_id, id],
            )?;
            Ok(None)
        }
    }
}

pub(super) fn apply_equipment(
    db: &Connection,
    id: &str,
    loadout: &mut Loadout,
) -> Result<(), Error> {
    let json: Option<String> = db.query_row(
        "SELECT i.cone FROM equipped_light_cones e JOIN light_cone_inventory i ON i.id = e.cone_id
         WHERE e.character_id = ?1", [id], |row| row.get(0),
    ).optional()?;
    if let Some(json) = json {
        loadout.config.light_cone = Some(serde_json::from_str(&json)?);
    }
    Ok(())
}
