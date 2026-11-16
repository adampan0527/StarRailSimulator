#![forbid(unsafe_code)]

mod catalog;
mod loadout;
mod store;
mod training;

pub use catalog::{
    Affix, Catalog, Character, CharacterVariant, Eidolon, Growth, LightConeData, PropertyBonus,
    RelicData, RelicSet, RelicSlot, Requirement, Resource, Servant, ServantStat, Skill,
    SkillActivation, Superimposition, Trace,
};
pub use loadout::{FrozenCharacter, FrozenTeam, Loadout};
pub use star_rail_battle_core::{Config, LightCone, Relic, Relics};
pub use store::{Archive, Profile, Team};
pub use training::training_catalog;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid {field}: {detail}")]
    Invalid { field: &'static str, detail: String },
    #[error("{kind} not found: {id}")]
    NotFound { kind: &'static str, id: String },
    #[error("unsupported {kind} version: {version}")]
    Version { kind: &'static str, version: String },
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cannot prepare the fixture battle: {0}")]
    Battle(#[from] star_rail_battle_core::BattleError),
}

impl Error {
    fn invalid(field: &'static str, detail: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            detail: detail.into(),
        }
    }

    fn missing(kind: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            kind,
            id: id.into(),
        }
    }
}

fn valid_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && id.split('.').all(|part| {
            part.as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
                && part.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_')
                })
        })
}
