use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Affix, Catalog, CharacterVariant, Config, Error, LightCone, Relic, RelicSlot, Relics,
    catalog::check_level,
};

const DEFAULT_CHARACTER_LEVEL: u16 = 80;
const DEFAULT_CHARACTER_ASCENSION: u8 = 6;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Loadout {
    pub variant_id: String,
    pub affinitive_eidolon: bool,
    pub config: Config,
}

impl Catalog {
    pub fn default_loadout(&self, character_id: &str) -> Result<Loadout, Error> {
        let character = self.character(character_id)?;
        self.default_loadout_at(
            character_id,
            &character.default_variant,
            DEFAULT_CHARACTER_LEVEL,
            DEFAULT_CHARACTER_ASCENSION,
        )
    }

    pub fn default_loadout_at(
        &self,
        character_id: &str,
        variant_id: &str,
        level: u16,
        ascension: u8,
    ) -> Result<Loadout, Error> {
        check_level(level, ascension)?;
        let character = self.character(character_id)?;
        let variant = character
            .variants
            .get(variant_id)
            .ok_or_else(|| Error::invalid("variant_id", variant_id))?;
        let skill_levels = variant
            .skills
            .iter()
            .filter(|(_, skill)| !skill.upgrade_requirements.is_empty())
            .map(|(id, skill)| (id.clone(), skill.base_cap(level, ascension)))
            .filter(|(_, cap)| *cap > 0)
            .collect();

        let mut unlocked = BTreeSet::new();
        loop {
            let eligible: Vec<_> = variant
                .traces
                .iter()
                .filter(|(_, trace)| {
                    !trace.automatic
                        && trace.requirement.met(level, ascension)
                        && trace
                            .prerequisites
                            .iter()
                            .all(|parent| unlocked.contains(parent))
                })
                .map(|(id, _)| id.clone())
                .collect();
            let previous = unlocked.len();
            unlocked.extend(eligible);
            if previous == unlocked.len() {
                break;
            }
        }
        Ok(Loadout {
            variant_id: variant_id.into(),
            affinitive_eidolon: false,
            config: Config {
                level,
                ascension,
                eidolon: 0,
                skill_levels,
                unlocked_trace_ids: unlocked.into_iter().collect(),
                light_cone: None,
                relics: Relics::default(),
            },
        })
    }

    pub fn default_light_cone(&self, id: &str) -> Result<LightCone, Error> {
        self.light_cone(id)?;
        Ok(LightCone {
            data_id: id.into(),
            level: 80,
            ascension: 6,
            superimposition: 1,
        })
    }

    pub(crate) fn validate_light_cone(&self, cone: &LightCone) -> Result<(), Error> {
        check_level(cone.level, cone.ascension)
            .map_err(|error| Error::invalid("light_cone.level", error.to_string()))?;
        self.light_cone(&cone.data_id)?.rank(cone.superimposition)?;
        Ok(())
    }

    pub fn validate_loadout(&self, character_id: &str, loadout: &Loadout) -> Result<(), Error> {
        let character = self.character(character_id)?;
        let config = &loadout.config;
        check_level(config.level, config.ascension)?;
        let variant = character
            .variants
            .get(&loadout.variant_id)
            .ok_or_else(|| Error::invalid("variant_id", &loadout.variant_id))?;
        if config.eidolon > 6 {
            return Err(Error::invalid("eidolon", config.eidolon.to_string()));
        }
        if loadout.affinitive_eidolon && character.affinitive_eidolon.is_none() {
            return Err(Error::invalid("affinitive_eidolon", character_id));
        }
        validate_progression(config, variant)?;
        if let Some(cone) = &config.light_cone {
            self.validate_light_cone(cone)?;
        }
        let relics = &config.relics;
        for (slot, relic) in [
            (RelicSlot::Head, &relics.head),
            (RelicSlot::Hands, &relics.hands),
            (RelicSlot::Body, &relics.body),
            (RelicSlot::Feet, &relics.feet),
            (RelicSlot::PlanarSphere, &relics.planar_sphere),
            (RelicSlot::LinkRope, &relics.link_rope),
        ] {
            if let Some(relic) = relic {
                self.validate_relic(slot, relic)?;
            }
        }
        Ok(())
    }

    /// Returns the selected or inherited skill level with unlocked Eidolon bonuses applied.
    pub fn effective_skill_level(
        &self,
        character_id: &str,
        loadout: &Loadout,
        skill_id: &str,
    ) -> Result<u16, Error> {
        let character = self.character(character_id)?;
        let variant = character
            .variants
            .get(&loadout.variant_id)
            .ok_or_else(|| Error::invalid("variant_id", &loadout.variant_id))?;
        let skill = variant
            .skills
            .get(skill_id)
            .ok_or_else(|| Error::missing("skill", skill_id))?;
        if !skill.conditional_levels.is_empty() {
            return Err(Error::invalid(
                "skill.level",
                "requires the source character's activation state",
            ));
        }
        let source = skill.level_source.as_deref().unwrap_or(skill_id);
        let base = match loadout.config.skill_levels.get(source).copied() {
            Some(level) => level,
            None if skill.parameters.len() == 1 => 1,
            None => {
                return Err(Error::invalid(
                    "skill.level",
                    format!("{skill_id} has no selected level"),
                ));
            }
        };
        let unlocked = variant
            .eidolons
            .get(..usize::from(loadout.config.eidolon))
            .ok_or_else(|| Error::invalid("eidolon", loadout.config.eidolon.to_string()))?;
        let special = character
            .affinitive_eidolon
            .as_ref()
            .filter(|_| loadout.affinitive_eidolon);
        let bonus = unlocked
            .iter()
            .chain(special)
            .map(|eidolon| {
                eidolon
                    .skill_bonuses
                    .get(skill_id)
                    .or_else(|| eidolon.skill_bonuses.get(source))
                    .copied()
                    .unwrap_or_default()
            })
            .fold(0_u16, u16::saturating_add);
        Ok(base
            .saturating_add(bonus)
            .min(skill.parameters.len() as u16))
    }

    pub(crate) fn validate_relic(&self, slot: RelicSlot, item: &Relic) -> Result<(), Error> {
        let data = self
            .relics
            .get(&item.data_id)
            .ok_or_else(|| Error::missing("relic", &item.data_id))?;
        if data.slot != slot {
            return Err(Error::invalid(
                "relic.slot",
                format!("{} in {slot:?}", item.data_id),
            ));
        }
        self.relic_main_value(&item.data_id, &item.main_stat, item.level)?;
        let group = &self.sub_affixes[&data.sub_group];
        let [min_total, max_total] = relic_roll_bounds(data, item.level);
        let mut possible_totals = BTreeSet::from([0]);
        for (property, &value) in &item.substats {
            let affix = group
                .get(property)
                .filter(|_| property != &item.main_stat && value != 0)
                .ok_or_else(|| Error::invalid("relic.substats", property))?;
            let counts: Vec<_> = (1..=max_total)
                .filter(|&count| attainable(*affix, count, value))
                .collect();
            if counts.is_empty() {
                return Err(Error::invalid(
                    "relic.substats",
                    format!("{property}: {value}"),
                ));
            }
            possible_totals = possible_totals
                .iter()
                .flat_map(|total| counts.iter().map(move |count| total + count))
                .filter(|&total| total <= max_total)
                .collect();
        }
        if !possible_totals
            .into_iter()
            .any(|total| total >= min_total && item.substats.len() == usize::from(total.min(4)))
        {
            return Err(Error::invalid(
                "relic.substats",
                "roll count incompatible with level and rarity",
            ));
        }
        Ok(())
    }

    /// Lists individually attainable values in millionths. Saving still checks
    /// whether all selected substats are attainable together.
    pub fn relic_substat_values(&self, id: &str, level: u16) -> Result<RelicSubstatValues, Error> {
        let relic = self
            .relics
            .get(id)
            .ok_or_else(|| Error::missing("relic", id))?;
        if level > relic.max_level {
            return Err(Error::invalid("relic.level", level.to_string()));
        }
        let [min_total, max_total] = relic_roll_bounds(relic, level);
        let max_lines = max_total.min(4);
        let max_rolls = max_total - max_lines.saturating_sub(1);
        let values = self.sub_affixes[&relic.sub_group]
            .iter()
            .map(|(property, &affix)| {
                let tiers = (1..=max_rolls)
                    .flat_map(|count| {
                        (0..=u16::from(affix.step_count) * count)
                            .map(move |tier| affix_micros(affix, count, tier))
                    })
                    .filter(|value| *value > 0.0 && *value <= f64::from(u32::MAX))
                    .map(|value| value as u32)
                    .collect::<BTreeSet<_>>();
                (property.clone(), tiers.into_iter().collect())
            })
            .collect();
        Ok(RelicSubstatValues {
            min_lines: min_total.min(4),
            max_lines,
            values,
        })
    }
}

pub struct RelicSubstatValues {
    pub min_lines: u16,
    pub max_lines: u16,
    pub values: BTreeMap<String, Vec<u32>>,
}

fn relic_roll_bounds(data: &crate::RelicData, level: u16) -> [u16; 2] {
    data.initial_substats
        .map(|lines| u16::from(lines) + level / data.upgrade_interval)
}

fn attainable(affix: Affix, count: u16, value: u32) -> bool {
    (0..=u16::from(affix.step_count) * count)
        .any(|tier| affix_micros(affix, count, tier) == f64::from(value))
}

fn affix_micros(affix: Affix, count: u16, tier: u16) -> f64 {
    let roll = affix.base * f64::from(count) + affix.step * f64::from(tier);
    (roll * 1_000_000.0).round()
}

fn validate_progression(config: &Config, variant: &CharacterVariant) -> Result<(), Error> {
    let caps: BTreeMap<_, _> = variant
        .skills
        .iter()
        .filter(|(_, skill)| !skill.upgrade_requirements.is_empty())
        .map(|(id, skill)| (id, skill.base_cap(config.level, config.ascension)))
        .filter(|(_, cap)| *cap > 0)
        .collect();
    if caps.len() != config.skill_levels.len() {
        return Err(Error::invalid(
            "skill_levels",
            "expected all trainable skills exactly once",
        ));
    }
    for (id, &level) in &config.skill_levels {
        if !caps.get(id).is_some_and(|&cap| (1..=cap).contains(&level)) {
            return Err(Error::invalid("skill_levels", format!("{id}: {level}")));
        }
    }
    let unlocked: BTreeSet<_> = config.unlocked_trace_ids.iter().collect();
    if unlocked.len() != config.unlocked_trace_ids.len() {
        return Err(Error::invalid("unlocked_trace_ids", "duplicate trace"));
    }
    for id in &unlocked {
        let trace = variant
            .traces
            .get(*id)
            .ok_or_else(|| Error::invalid("unlocked_trace_ids", *id))?;
        if trace.automatic
            || !trace.requirement.met(config.level, config.ascension)
            || !trace
                .prerequisites
                .iter()
                .all(|parent| unlocked.contains(parent))
        {
            return Err(Error::invalid("unlocked_trace_ids", *id));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenCharacter {
    pub character_form_id: String,
    pub loadout: Loadout,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenTeam {
    schema_version: u32,
    pub game_version: String,
    pub data_version: String,
    pub members: Vec<FrozenCharacter>,
}

impl FrozenTeam {
    pub(crate) fn new(catalog: &Catalog, members: Vec<FrozenCharacter>) -> Self {
        Self {
            schema_version: 1,
            game_version: catalog.game_version.clone(),
            data_version: catalog.data_version.clone(),
            members,
        }
    }

    pub fn from_json(json: &str, catalog: &Catalog) -> Result<Self, Error> {
        let snapshot: Self = serde_json::from_str(json)?;
        if snapshot.schema_version != 1 {
            return Err(Error::Version {
                kind: "snapshot schema",
                version: snapshot.schema_version.to_string(),
            });
        }
        if snapshot.game_version != catalog.game_version
            || snapshot.data_version != catalog.data_version
        {
            return Err(Error::Version {
                kind: "snapshot catalog",
                version: snapshot.data_version,
            });
        }
        let mut seen = BTreeSet::new();
        if !(1..=4).contains(&snapshot.members.len())
            || !snapshot
                .members
                .iter()
                .all(|member| seen.insert(&member.character_form_id))
        {
            return Err(Error::invalid(
                "snapshot.members",
                "expected one to four distinct characters",
            ));
        }
        for member in &snapshot.members {
            catalog.validate_loadout(&member.character_form_id, &member.loadout)?;
        }
        Ok(snapshot)
    }

    /// Applies a fixture-only snapshot to the shared M02 session template.
    pub fn fixture_session(
        &self,
        template: &star_rail_battle_core::Session,
    ) -> Result<star_rail_battle_core::Session, Error> {
        use star_rail_battle_core::{PreparedBattle, Session};
        let mut input = template.input.clone();
        if self.data_version != "fixture.data.v1"
            || self.data_version != input.data_version
            || self.game_version != input.game_version
            || self.members.len() != input.team.len()
        {
            return Err(Error::invalid(
                "fixture_session",
                "fixture version or team size",
            ));
        }
        for (slot, member) in input.team.iter_mut().zip(&self.members) {
            if slot.character_form_id != member.character_form_id
                || member.loadout.variant_id != "base"
                || member.loadout.affinitive_eidolon
            {
                return Err(Error::invalid("fixture_session", &member.character_form_id));
            }
            slot.config = member.loadout.config.clone();
        }
        PreparedBattle::new(&input)?;
        Ok(Session::new(input))
    }
}
