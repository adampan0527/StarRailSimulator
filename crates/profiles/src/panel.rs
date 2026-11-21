use std::collections::BTreeMap;

use serde::Serialize;
use star_rail_battle_core::{Element, TrainingAttributes};

use crate::{Catalog, Error, Loadout, PropertyBonus};

#[derive(Clone, Copy, Serialize)]
pub struct ScaledStat {
    pub base: f64,
    pub total: f64,
}

/// Static character sheet. Ratio values are represented as fractions.
#[derive(Default, Serialize)]
pub struct CharacterStats {
    pub hp: Option<ScaledStat>,
    pub attack: Option<ScaledStat>,
    pub defense: Option<ScaledStat>,
    pub speed: Option<ScaledStat>,
    pub crit_rate: Option<f64>,
    pub crit_damage: Option<f64>,
    pub element_damage: Option<f64>,
    #[serde(skip)]
    pub(crate) elemental_damage: [f64; Element::COUNT],
    pub break_effect: Option<f64>,
    pub elation: Option<f64>,
    pub healing_boost: Option<f64>,
    pub energy_regeneration: Option<f64>,
    pub effect_hit_rate: Option<f64>,
    pub effect_resistance: Option<f64>,
}

impl Catalog {
    pub fn trace_bonuses(&self, id: &str, loadout: &Loadout) -> Result<BTreeMap<&str, f64>, Error> {
        let character = self.character(id)?;
        let config = &loadout.config;
        let variant = character
            .variants
            .get(&loadout.variant_id)
            .ok_or_else(|| Error::invalid("variant_id", &loadout.variant_id))?;
        let mut bonuses = BTreeMap::new();
        for (trace_id, trace) in &variant.traces {
            if config.unlocked_trace_ids.contains(trace_id)
                || (trace.automatic && trace.requirement.met(config.level, config.ascension))
            {
                add_bonuses(&mut bonuses, &trace.properties);
            }
        }
        Ok(bonuses)
    }

    /// Applies current equipment and unconditional data properties to the character sheet.
    pub fn character_stats(&self, id: &str, loadout: &Loadout) -> Result<CharacterStats, Error> {
        let character = self.character(id)?;
        let config = &loadout.config;
        let mut base = character.base_attributes(config.level, config.ascension)?;
        let mut bonuses = self.trace_bonuses(id, loadout)?;
        if let Some(cone) = &config.light_cone {
            let data = self.light_cone(&cone.data_id)?;
            for (key, value) in data.base_attributes(cone.level, cone.ascension)? {
                *base.entry(key).or_default() += value;
            }
            if data.path == character.path {
                add_bonuses(&mut bonuses, &data.rank(cone.superimposition)?.properties);
            }
        }

        let relics = &config.relics;
        let mut sets = BTreeMap::<&str, u8>::new();
        for relic in [
            &relics.head,
            &relics.hands,
            &relics.body,
            &relics.feet,
            &relics.planar_sphere,
            &relics.link_rope,
        ]
        .into_iter()
        .flatten()
        {
            let main = self.relic_main_value(&relic.data_id, &relic.main_stat, relic.level)?;
            *bonuses.entry(&relic.main_stat).or_default() += main;
            for (key, value) in &relic.substats {
                *bonuses.entry(key).or_default() += f64::from(*value) / 1_000_000.0;
            }
            *sets.entry(&self.relics[&relic.data_id].set_id).or_default() += 1;
        }
        for (set, count) in sets {
            for (_, properties) in self.relic_sets[set].properties.range(..=count) {
                add_bonuses(&mut bonuses, properties);
            }
        }

        let bonus = |key: &str| bonuses.get(key).copied().unwrap_or_default();
        let scaled = |key, ratio, flat| {
            base.get(key).map(|&base| ScaledStat {
                base,
                total: base * (1.0 + bonus(ratio)) + bonus(flat),
            })
        };
        let element_property = format!("{}addedratio", character.element);
        Ok(CharacterStats {
            hp: scaled("hp", "hp_ratio", "hp_flat"),
            attack: scaled("atk", "attack_ratio", "attack_flat"),
            defense: scaled("def", "defense_ratio", "defense_flat"),
            speed: scaled("spd", "speed_ratio", "speed"),
            crit_rate: base
                .get("crit_rate")
                .map(|value| value + bonus("crit_rate")),
            crit_damage: base
                .get("crit_dmg")
                .map(|value| value + bonus("crit_damage")),
            element_damage: Some(
                bonus(element_property.as_str()) + bonus("alldamagetypeaddedratio"),
            ),
            elemental_damage: Element::ALL.map(|element| {
                bonus(format!("{}addedratio", element.id()).as_str())
                    + bonus("alldamagetypeaddedratio")
            }),
            break_effect: Some(bonus("breakdamageaddedratiobase")),
            elation: Some(bonus("elationdamageaddedratiobase")),
            healing_boost: Some(bonus("healratiobase")),
            energy_regeneration: Some(1.0 + bonus("spratiobase")),
            effect_hit_rate: Some(bonus("statusprobabilitybase")),
            effect_resistance: Some(bonus("statusresistancebase")),
        })
    }
}

fn add_bonuses<'a>(totals: &mut BTreeMap<&'a str, f64>, bonuses: &'a [PropertyBonus]) {
    for bonus in bonuses {
        *totals.entry(&bonus.property).or_default() += bonus.value;
    }
}

impl From<TrainingAttributes> for CharacterStats {
    fn from(stats: TrainingAttributes) -> Self {
        let stat = |total| Some(ScaledStat { base: total, total });
        Self {
            hp: stat(f64::from(stats.hp)),
            attack: stat(f64::from(stats.attack)),
            defense: stat(f64::from(stats.defense)),
            speed: stat(stats.speed_micros as f64 / 1_000_000.0),
            ..Self::default()
        }
    }
}
