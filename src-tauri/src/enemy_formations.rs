use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const CATALOG: &str = include_str!("../../src/team/enemy-preview.json");

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    enemy_id: String,
    level: u8,
}

type Slots = [Option<Slot>; 5];

#[derive(Serialize)]
pub(crate) struct EnemyFormation {
    id: u32,
    name: String,
    slots: Slots,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FormationDraft {
    name: String,
    slots: Slots,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("invalid formation")]
    Invalid,
    #[error("formation not found")]
    Missing,
    #[error("unsupported enemy formation database version {0}")]
    Version(u32),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize)]
pub(crate) struct CommandError {
    code: &'static str,
}

fn connect(path: &Path) -> Result<Connection, Error> {
    let mut db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(3))?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match tx.pragma_query_value(None, "user_version", |row| row.get(0))? {
        0 => {
            tx.execute_batch(
                "CREATE TABLE enemy_formations (
                    id INTEGER PRIMARY KEY CHECK (id BETWEEN 1 AND 4294967295),
                    name TEXT NOT NULL,
                    slots TEXT NOT NULL
                ) STRICT;",
            )?;
            tx.pragma_update(None, "user_version", 1)?;
        }
        1 => {}
        version => return Err(Error::Version(version)),
    }
    tx.commit()?;
    Ok(db)
}

fn list(db: &Connection) -> Result<Vec<EnemyFormation>, Error> {
    let mut query = db.prepare("SELECT id, name, slots FROM enemy_formations ORDER BY id")?;
    let mut rows = query.query([])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(EnemyFormation {
            id: row.get(0)?,
            name: row.get(1)?,
            slots: serde_json::from_str(&row.get::<_, String>(2)?)?,
        });
    }
    Ok(result)
}

fn save(db: &Connection, id: Option<u32>, draft: FormationDraft) -> Result<EnemyFormation, Error> {
    #[derive(Deserialize)]
    struct Enemy {
        id: String,
    }
    #[derive(Deserialize)]
    struct Catalog {
        levels: Vec<u8>,
        enemies: Vec<Enemy>,
    }

    let name = draft.name.trim();
    if name.is_empty() || name.chars().count() > 48 || name.chars().any(char::is_control) {
        return Err(Error::Invalid);
    }
    let catalog: Catalog = serde_json::from_str(CATALOG)?;
    if draft.slots.iter().all(Option::is_none)
        || draft.slots.iter().flatten().any(|slot| {
            !catalog.levels.contains(&slot.level)
                || !catalog
                    .enemies
                    .iter()
                    .any(|enemy| enemy.id == slot.enemy_id)
        })
    {
        return Err(Error::Invalid);
    }
    let json = serde_json::to_string(&draft.slots)?;
    let id = match id {
        Some(id) => db
            .query_row(
                "UPDATE enemy_formations SET name = ?1, slots = ?2 WHERE id = ?3 RETURNING id",
                params![name, json, id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(Error::Missing)?,
        None => db.query_row(
            "INSERT INTO enemy_formations (name, slots) VALUES (?1, ?2) RETURNING id",
            params![name, json],
            |row| row.get(0),
        )?,
    };
    Ok(EnemyFormation {
        id,
        name: name.into(),
        slots: draft.slots,
    })
}

async fn with_store<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&Connection) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    let unavailable = || CommandError {
        code: "storage_failed",
    };
    tauri::async_runtime::spawn_blocking(move || {
        let path = app.path().app_local_data_dir().map_err(|_| unavailable())?;
        std::fs::create_dir_all(&path).map_err(|_| unavailable())?;
        connect(&path.join("enemy-formations.sqlite"))
            .and_then(|db| operation(&db))
            .map_err(|error| {
                let code = match error {
                    Error::Invalid => "invalid_enemy_formation",
                    Error::Missing => "enemy_formation_missing",
                    Error::Version(_) => "incompatible_version",
                    _ => {
                        eprintln!("敌方编队存档操作失败：{error}");
                        "storage_failed"
                    }
                };
                CommandError { code }
            })
    })
    .await
    .map_err(|_| unavailable())?
}

#[tauri::command]
pub(crate) async fn enemy_formations(app: AppHandle) -> Result<Vec<EnemyFormation>, CommandError> {
    with_store(app, list).await
}

#[tauri::command]
pub(crate) async fn enemy_formation_save(
    app: AppHandle,
    id: Option<u32>,
    draft: FormationDraft,
) -> Result<EnemyFormation, CommandError> {
    with_store(app, move |db| save(db, id, draft)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_formations_preserve_empty_positions_and_validate_before_updates() {
        let directory = crate::workbench_tests::TestData::new();
        let database = directory.path().join("formations.sqlite");
        let db = connect(&database).unwrap();
        let slot = Slot {
            enemy_id: "1003010".into(),
            level: 80,
        };
        let draft = |name: &str, slot: Slot| FormationDraft {
            name: name.into(),
            slots: [None, Some(slot), None, None, None],
        };
        let first = save(&db, None, draft(" 精英 ", slot.clone())).unwrap();
        let second = save(
            &db,
            None,
            draft(
                "等级对照",
                Slot {
                    level: 90,
                    ..slot.clone()
                },
            ),
        )
        .unwrap();
        assert_ne!(first.id, second.id);
        assert!(matches!(
            save(
                &db,
                Some(first.id),
                draft(
                    "无效",
                    Slot {
                        level: 0,
                        ..slot.clone()
                    }
                )
            ),
            Err(Error::Invalid)
        ));
        assert!(matches!(
            save(&db, Some(99), draft("不存在", slot.clone())),
            Err(Error::Missing)
        ));
        drop(db);
        let saved = list(&connect(&database).unwrap()).unwrap();
        assert_eq!(saved[0].name, "精英");
        assert_eq!(saved[0].slots, [None, Some(slot), None, None, None]);
        assert_eq!(saved[1].slots[1].as_ref().unwrap().level, 90);
    }
}
