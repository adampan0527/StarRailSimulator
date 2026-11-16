use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::{Error, valid_id};

const MIN_LEVELS: [u16; 7] = [1, 20, 30, 40, 50, 60, 70];
const MAX_LEVELS: [u16; 7] = [20, 30, 40, 50, 60, 70, 80];

pub(crate) fn check_level(level: u16, ascension: u8) -> Result<usize, Error> {
    let phase = usize::from(ascension);
    if phase >= MIN_LEVELS.len() || !(MIN_LEVELS[phase]..=MAX_LEVELS[phase]).contains(&level) {
        return Err(Error::invalid(
            "level",
            format!("level {level}, ascension {ascension}"),
        ));
    }
    Ok(phase)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Growth {
    pub base: f64,
    pub step: f64,
}

type PromotionStats = [BTreeMap<String, Growth>; 7];

fn attributes(
    stats: &PromotionStats,
    level: u16,
    ascension: u8,
) -> Result<BTreeMap<String, f64>, Error> {
    let phase = check_level(level, ascension)?;
    Ok(stats[phase]
        .iter()
        .map(|(id, growth)| (id.clone(), growth.base + growth.step * f64::from(level - 1)))
        .collect())
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Requirement {
    pub level: u16,
    pub ascension: u8,
}

impl Requirement {
    pub(crate) fn met(self, level: u16, ascension: u8) -> bool {
        level >= self.level && ascension >= self.ascension
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PropertyBonus {
    pub property: String,
    pub value: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillActivation {
    pub level: u16,
    pub kind: String,
    pub character_id: String,
    pub minimum: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Skill {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub parameters: Vec<Vec<f64>>,
    pub upgrade_requirements: Vec<Requirement>,
    pub level_source: Option<String>,
    pub conditional_levels: Vec<SkillActivation>,
    pub hidden: bool,
    pub display_skill_id: Option<String>,
}

impl Skill {
    pub(crate) fn base_cap(&self, level: u16, ascension: u8) -> u16 {
        self.upgrade_requirements
            .iter()
            .take_while(|requirement| requirement.met(level, ascension))
            .count() as u16
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trace {
    pub name: String,
    pub description: String,
    pub requirement: Requirement,
    pub prerequisites: Vec<String>,
    pub properties: Vec<PropertyBonus>,
    pub parameters: Vec<Vec<f64>>,
    pub automatic: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Eidolon {
    pub id: String,
    pub name: String,
    pub description: String,
    pub skill_bonuses: BTreeMap<String, u16>,
    pub parameters: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServantStat {
    pub base: String,
    pub inheritance: String,
    pub skill_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Servant {
    pub name: String,
    pub skill_ids: Vec<String>,
    pub hp: ServantStat,
    pub speed: ServantStat,
    pub aggro: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterVariant {
    pub skills: BTreeMap<String, Skill>,
    pub traces: BTreeMap<String, Trace>,
    pub eidolons: [Eidolon; 6],
    pub servant: Option<Servant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Resource {
    pub kind: String,
    pub capacity: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Character {
    pub name: String,
    pub source_ids: Vec<String>,
    pub rarity: u8,
    pub path: String,
    pub element: String,
    pub resource: Resource,
    pub promotions: PromotionStats,
    pub default_variant: String,
    pub variants: BTreeMap<String, CharacterVariant>,
    pub affinitive_eidolon: Option<Eidolon>,
}

impl Character {
    pub fn base_attributes(
        &self,
        level: u16,
        ascension: u8,
    ) -> Result<BTreeMap<String, f64>, Error> {
        attributes(&self.promotions, level, ascension)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Superimposition {
    pub parameters: Vec<f64>,
    pub properties: Vec<PropertyBonus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LightConeData {
    pub name: String,
    pub rarity: u8,
    pub path: String,
    pub promotions: PromotionStats,
    pub effect_name: String,
    pub description: String,
    pub ranks: [Superimposition; 5],
}

impl LightConeData {
    pub fn base_attributes(
        &self,
        level: u16,
        ascension: u8,
    ) -> Result<BTreeMap<String, f64>, Error> {
        attributes(&self.promotions, level, ascension)
    }

    pub fn rank(&self, superimposition: u8) -> Result<&Superimposition, Error> {
        match superimposition {
            1..=5 => Ok(&self.ranks[usize::from(superimposition - 1)]),
            _ => Err(Error::invalid(
                "light_cone.superimposition",
                superimposition.to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelicSlot {
    Head,
    Hands,
    Body,
    Feet,
    PlanarSphere,
    LinkRope,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Affix {
    pub base: f64,
    pub step: f64,
    pub step_count: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelicData {
    pub name: String,
    pub slot: RelicSlot,
    pub rarity: u8,
    pub set_id: String,
    pub max_level: u16,
    pub main_group: String,
    pub sub_group: String,
    pub initial_substats: [u8; 2],
    pub upgrade_interval: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelicSet {
    pub name: String,
    pub effects: BTreeMap<u8, String>,
    pub properties: BTreeMap<u8, Vec<PropertyBonus>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub game_version: String,
    #[serde(skip)]
    pub data_version: String,
    pub characters: BTreeMap<String, Character>,
    pub light_cones: BTreeMap<String, LightConeData>,
    pub relics: BTreeMap<String, RelicData>,
    pub relic_sets: BTreeMap<String, RelicSet>,
    pub main_affixes: BTreeMap<String, BTreeMap<String, Affix>>,
    pub sub_affixes: BTreeMap<String, BTreeMap<String, Affix>>,
    pub properties: BTreeMap<String, String>,
}

impl Catalog {
    pub fn from_json(json: &str) -> Result<Self, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document<'a> {
            data_version: String,
            #[serde(borrow)]
            content: &'a RawValue,
        }

        let document: Document<'_> = serde_json::from_str(json)?;
        let content = document.content.get();
        let mut catalog: Self = serde_json::from_str(content)?;
        let digest = Sha256::digest(content.as_bytes());
        let expected = format!("sr.{}.m03.{digest:x}", catalog.game_version);
        if document.data_version != expected {
            return Err(Error::invalid(
                "catalog.data_version",
                "content digest mismatch",
            ));
        }
        catalog.data_version = document.data_version;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn character(&self, id: &str) -> Result<&Character, Error> {
        self.characters
            .get(id)
            .ok_or_else(|| Error::missing("character", id))
    }

    pub fn light_cone(&self, id: &str) -> Result<&LightConeData, Error> {
        self.light_cones
            .get(id)
            .ok_or_else(|| Error::missing("light cone", id))
    }

    pub fn relic_main_value(&self, id: &str, property: &str, level: u16) -> Result<f64, Error> {
        let relic = self
            .relics
            .get(id)
            .ok_or_else(|| Error::missing("relic", id))?;
        if level > relic.max_level {
            return Err(Error::invalid("relic.level", level.to_string()));
        }
        let affix = self
            .main_affixes
            .get(&relic.main_group)
            .and_then(|group| group.get(property))
            .ok_or_else(|| Error::invalid("relic.main_stat", property))?;
        Ok(affix.base + affix.step * f64::from(level))
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 {
            return Err(Error::Version {
                kind: "catalog schema",
                version: self.schema_version.to_string(),
            });
        }
        if self.game_version.is_empty()
            || !valid_id(&self.data_version)
            || self.characters.is_empty()
        {
            return Err(Error::invalid(
                "catalog",
                "missing version or character data",
            ));
        }
        let mut source_ids = BTreeSet::new();
        for (id, character) in &self.characters {
            if !valid_id(id)
                || character.name.is_empty()
                || character.source_ids.is_empty()
                || !character
                    .source_ids
                    .iter()
                    .all(|source| source_ids.insert(source))
                || !character.variants.contains_key(&character.default_variant)
            {
                return Err(Error::invalid("catalog.character", id));
            }
            for variant in character.variants.values() {
                if variant.skills.is_empty() {
                    return Err(Error::invalid("catalog.skills", id));
                }
                for (skill_id, skill) in &variant.skills {
                    if !valid_id(skill_id)
                        || skill.parameters.is_empty()
                        || skill.parameters.len() > usize::from(u16::MAX)
                        || skill.upgrade_requirements.len() > skill.parameters.len()
                        || skill.level_source.as_ref().is_some_and(|source| {
                            !variant.skills.get(source).is_some_and(|parent| {
                                !parent.upgrade_requirements.is_empty()
                                    && parent.level_source.is_none()
                            })
                        })
                        || skill.conditional_levels.iter().any(|condition| {
                            condition.level == 0
                                || usize::from(condition.level) > skill.parameters.len()
                                || !self.characters.contains_key(&condition.character_id)
                        })
                        || skill
                            .display_skill_id
                            .as_ref()
                            .is_some_and(|id| !variant.skills.contains_key(id))
                    {
                        return Err(Error::invalid("catalog.skill", format!("{id}/{skill_id}")));
                    }
                }
                for (trace_id, trace) in &variant.traces {
                    if !valid_id(trace_id)
                        || !trace
                            .prerequisites
                            .iter()
                            .all(|key| variant.traces.contains_key(key))
                    {
                        return Err(Error::invalid("catalog.trace", format!("{id}/{trace_id}")));
                    }
                }
                if let Some(servant) = &variant.servant
                    && (servant.name.is_empty()
                        || servant.skill_ids.is_empty()
                        || !servant
                            .skill_ids
                            .iter()
                            .all(|id| variant.skills.contains_key(id))
                        || [&servant.hp, &servant.speed].iter().any(|stat| {
                            stat.skill_id
                                .as_ref()
                                .is_some_and(|id| !variant.skills.contains_key(id))
                        }))
                {
                    return Err(Error::invalid("catalog.servant", id));
                }
            }
        }
        for (id, relic) in &self.relics {
            if !valid_id(id)
                || !self.relic_sets.contains_key(&relic.set_id)
                || !self.main_affixes.contains_key(&relic.main_group)
                || !self.sub_affixes.contains_key(&relic.sub_group)
                || relic.upgrade_interval == 0
                || relic.initial_substats[0] > relic.initial_substats[1]
                || relic.initial_substats[1] > 4
            {
                return Err(Error::invalid("catalog.relic", id));
            }
        }
        Ok(())
    }
}
