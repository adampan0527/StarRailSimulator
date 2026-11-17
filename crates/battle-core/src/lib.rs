//! Offline battle simulation.
#![forbid(unsafe_code)]

mod battle;
mod contract;
mod damage;
mod fixture;
mod report;
mod rng;

pub use battle::{
    Battle, DprResult, PreparedBattle, VictorySearch, VictorySolution, calculate_dpr,
    search_fastest_victory,
};
pub use contract::{
    Action, AllyInput, BattleError, Command, Config, EnemyInput, InitialInput, LightCone, Relic,
    Relics, Session,
};
pub use fixture::{TrainingAttributes, training_attributes};
pub use report::{
    ActionOrderEntry, DamageKind, Event, EventKind, LogMode, Outcome, Snapshot, StatusKind,
    UnitSnapshot,
};
