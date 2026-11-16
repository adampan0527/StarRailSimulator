//! Offline battle simulation.
#![forbid(unsafe_code)]

mod battle;
mod contract;
mod damage;
mod fixture;
mod report;
mod rng;

pub use battle::{Battle, PreparedBattle};
pub use contract::{
    Action, AllyInput, BattleError, Command, Config, EnemyInput, InitialInput, LightCone, Relic,
    Relics, Session,
};
pub use report::{
    DamageKind, Event, EventKind, LogMode, Outcome, Snapshot, StatusKind, UnitSnapshot,
};
