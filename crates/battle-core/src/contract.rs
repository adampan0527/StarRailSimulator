use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Basic,
    Skill,
    EnhancedSkill,
    Ultimate,
    EnhancedUltimate,
    #[serde(skip_deserializing)]
    ElationSkill,
}

impl Action {
    pub const fn is_ultimate(self) -> bool {
        matches!(self, Self::Ultimate | Self::EnhancedUltimate)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub decision_index: u32,
    pub actor_id: String,
    pub action: Action,
    #[serde(deserialize_with = "required_option")]
    pub target_id: Option<String>,
}

/// The envelope and frozen input shared with the profile layer.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    schema_version: u32,
    pub input: InitialInput,
    pub commands: Vec<Command>,
}

impl Session {
    pub fn new(input: InitialInput) -> Self {
        Self {
            schema_version: 1,
            input,
            commands: Vec::new(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, BattleError> {
        let session: Self = serde_json::from_str(json)
            .map_err(|error| BattleError::InvalidJson(error.to_string()))?;
        if session.schema_version != 1 {
            return Err(BattleError::UnsupportedSchema(session.schema_version));
        }
        Ok(session)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InitialInput {
    pub game_version: String,
    pub rules_version: String,
    pub data_version: String,
    pub rng_version: String,
    pub seed: u32,
    pub stage_id: String,
    pub starting_skill_points: u32,
    pub team: Vec<AllyInput>,
    pub waves: Vec<Vec<EnemyInput>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllyInput {
    pub unit_id: String,
    pub character_form_id: String,
    pub starting_energy_milli: u32,
    pub config: Config,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyInput {
    pub unit_id: String,
    pub enemy_id: String,
    pub level: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub level: u16,
    pub ascension: u8,
    pub eidolon: u8,
    pub skill_levels: BTreeMap<String, u16>,
    pub unlocked_trace_ids: Vec<String>,
    #[serde(deserialize_with = "required_option")]
    pub light_cone: Option<LightCone>,
    pub relics: Relics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LightCone {
    pub data_id: String,
    pub level: u16,
    pub ascension: u8,
    pub superimposition: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relic {
    pub data_id: String,
    pub level: u16,
    pub main_stat: String,
    pub substats: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relics {
    #[serde(deserialize_with = "required_option")]
    pub head: Option<Relic>,
    #[serde(deserialize_with = "required_option")]
    pub hands: Option<Relic>,
    #[serde(deserialize_with = "required_option")]
    pub body: Option<Relic>,
    #[serde(deserialize_with = "required_option")]
    pub feet: Option<Relic>,
    #[serde(deserialize_with = "required_option")]
    pub planar_sphere: Option<Relic>,
    #[serde(deserialize_with = "required_option")]
    pub link_rope: Option<Relic>,
}

fn required_option<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(deserializer)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleError {
    InvalidJson(String),
    UnsupportedSchema(u32),
    UnsupportedVersion { field: &'static str, value: String },
    InvalidInput(String),
    UnsupportedData(String),
    StaleDecision,
    BattleEnded,
    InvalidActor,
    IllegalTiming,
    InsufficientSkillPoints,
    InsufficientEnergy,
    InsufficientUltimateResource,
    InvalidTarget,
}

impl fmt::Display for BattleError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(output, "invalid session JSON: {detail}"),
            Self::UnsupportedSchema(schema) => {
                write!(output, "unsupported schema version: {schema}")
            }
            Self::UnsupportedVersion { field, value } => {
                write!(output, "unsupported {field}: {value}")
            }
            Self::InvalidInput(detail) => write!(output, "invalid input: {detail}"),
            Self::UnsupportedData(detail) => write!(output, "unsupported fixture data: {detail}"),
            Self::StaleDecision => {
                output.write_str("decision index does not match the current decision")
            }
            Self::BattleEnded => output.write_str("battle has ended"),
            Self::InvalidActor => output.write_str("actor is not a living ally"),
            Self::IllegalTiming => output.write_str("action is unavailable at this time"),
            Self::InsufficientSkillPoints => output.write_str("not enough skill points"),
            Self::InsufficientEnergy => output.write_str("not enough energy"),
            Self::InsufficientUltimateResource => output.write_str("not enough Ultimate resource"),
            Self::InvalidTarget => output.write_str("target does not match the action"),
        }
    }
}

impl Error for BattleError {}
