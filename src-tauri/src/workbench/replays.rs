use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use star_rail_battle_core::{Outcome, Session};

use super::Error;

#[derive(Serialize)]
pub struct ReplaySummary {
    pub id: u32,
    pub started_at: i64,
    pub finished: bool,
}

pub(super) struct ReplayStore(Connection);

impl ReplayStore {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let mut db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(3))?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match tx.pragma_query_value(None, "user_version", |row| row.get(0))? {
            0 => {
                tx.execute_batch(
                    "CREATE TABLE replays (
                        id INTEGER PRIMARY KEY CHECK (id BETWEEN 1 AND 4294967295),
                        started_at INTEGER NOT NULL DEFAULT (unixepoch()),
                        finished INTEGER NOT NULL CHECK (finished IN (0, 1)),
                        session TEXT NOT NULL
                    ) STRICT;",
                )?;
                tx.pragma_update(None, "user_version", 1)?;
            }
            1 => {}
            version => return Err(Error::ReplayVersion(version)),
        }
        tx.commit()?;
        Ok(Self(db))
    }

    pub fn insert(&mut self, session: &Session, outcome: Outcome) -> Result<u32, Error> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = tx.query_row(
            "INSERT INTO replays (session, finished) VALUES (?1, ?2) RETURNING id",
            params![serde_json::to_string(session)?, outcome != Outcome::Ongoing],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn session(&self, id: u32) -> Result<Session, Error> {
        read(&self.0, id)
    }

    pub fn update<T>(
        &mut self,
        id: u32,
        apply: impl FnOnce(&mut Session) -> Result<(T, Outcome), Error>,
    ) -> Result<T, Error> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = read(&tx, id)?;
        let (result, outcome) = apply(&mut session)?;
        tx.execute(
            "UPDATE replays SET session = ?1, finished = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&session)?,
                outcome != Outcome::Ongoing,
                id
            ],
        )?;
        tx.commit()?;
        Ok(result)
    }

    pub fn list(&self) -> Result<Vec<ReplaySummary>, Error> {
        let mut query = self
            .0
            .prepare("SELECT id, started_at, finished FROM replays ORDER BY id DESC")?;
        Ok(query
            .query_map([], |row| {
                Ok(ReplaySummary {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    finished: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }
}

fn read(db: &Connection, id: u32) -> Result<Session, Error> {
    let json: String = db
        .query_row("SELECT session FROM replays WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or(Error::ReplayNotFound(id))?;
    Ok(Session::from_json(&json)?)
}
