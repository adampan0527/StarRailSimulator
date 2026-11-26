//! Offline battle simulation.
#![forbid(unsafe_code)]

mod battle;
mod contract;
mod damage;
mod fixture;
mod report;
mod rng;

pub use battle::{
    Battle, CombatPath, CombatType, DprCharacter, DprCredit, DprHit, DprResult, DprScenario,
    DprTarget, OfficialCharacter, OfficialEnemy, OfficialEnemyStats, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, VictorySearch, VictorySolution, calculate_dpr, official_enemy_stats,
    search_fastest_victory, supports_official_dpr, supports_official_dpr_variant,
    supports_official_enemy,
};
pub use contract::{
    Action, AllyInput, BattleError, Command, Config, EnemyInput, InitialInput, LightCone, Relic,
    Relics, Session,
};
pub use fixture::{Element, TrainingAttributes, training_attributes};
pub use report::{
    ActionOrderEntry, DamageKind, Event, EventKind, LogMode, Outcome, Snapshot, StatusKind,
    UnitSnapshot,
};
