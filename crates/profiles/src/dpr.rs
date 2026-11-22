use star_rail_battle_core::{
    CombatPath, CombatType, Element, OfficialCharacter, OfficialStats,
    supports_official_dpr_variant,
};

use crate::{Catalog, Error, FrozenCharacter, ScaledStat};

const DATA_VERSION: &str =
    "sr.4.7.0.m03.dce40f8d031ea2b3272537b2dd4041510e65e163db773a9d1a7cda7a509b1ff9";

impl Catalog {
    /// Resolves a frozen loadout against the exact data snapshot accepted by the DPR rules.
    pub fn dpr_character(
        &self,
        member: &FrozenCharacter,
    ) -> Result<Option<OfficialCharacter>, Error> {
        let id = &member.character_form_id;
        let loadout = &member.loadout;
        let config = &loadout.config;
        if self.data_version != DATA_VERSION
            || !supports_official_dpr_variant(id, &loadout.variant_id)
            || config.level != 80
            || loadout.affinitive_eidolon
        {
            return Ok(None);
        }
        self.validate_loadout(id, loadout)?;
        let character = self.character(id)?;
        let path = CombatPath::from_id(&character.path)
            .ok_or_else(|| Error::invalid("character.path", id))?;
        let element = Element::from_id(&character.element)
            .ok_or_else(|| Error::invalid("character.element", id))?;
        let energy_capacity = match (
            id.as_str(),
            character.resource.kind.as_str(),
            character.resource.capacity,
        ) {
            ("character.1220", "flying_aureus", Some(12)) => 0,
            ("character.1308", "slashed_dream", Some(9)) => 0,
            ("character.1407", "newbud", None) => 0,
            ("character.1220" | "character.1308" | "character.1407", _, _) => {
                return Err(Error::invalid("character.resource", id));
            }
            (_, "energy", Some(capacity)) => capacity,
            _ => return Err(Error::invalid("character.resource", id)),
        };
        let variant = &character.variants[&loadout.variant_id];
        let mut skill_parameters = std::collections::BTreeMap::new();
        for (skill_id, skill) in &variant.skills {
            if !skill.conditional_levels.is_empty() {
                continue;
            }
            let level = self.effective_skill_level(id, loadout, skill_id)?;
            skill_parameters.insert(
                skill_id.clone(),
                skill.parameters[usize::from(level - 1)].clone(),
            );
        }
        let base_aggro = character
            .base_attributes(config.level, config.ascension)?
            .get("taunt")
            .copied()
            .filter(|value| (1.0..=1_000.0).contains(value) && value.fract() == 0.0)
            .ok_or_else(|| Error::invalid("character.taunt", id))? as u16;
        let panel = self.character_stats(id, loadout)?;
        let stat = |name, value: Option<ScaledStat>| value.ok_or_else(|| Error::invalid(name, id));
        let hp = stat("character.hp", panel.hp)?;
        let attack = stat("character.attack", panel.attack)?;
        let defense = stat("character.defense", panel.defense)?;
        let speed = stat("character.speed", panel.speed)?;
        let ratio = |value: Option<f64>| (value.unwrap_or_default() * 10_000.0).round() as u32;
        let stats = OfficialStats {
            hp: hp.total.round() as u32,
            base_hp: hp.base.round() as u32,
            attack: attack.total.round() as u32,
            base_attack: attack.base.round() as u32,
            defense: defense.total.round() as u32,
            base_defense: defense.base.round() as u32,
            base_aggro,
            base_speed_micros: (speed.base * 1_000_000.0).round() as u64,
            speed_micros: (speed.total * 1_000_000.0).round() as u64,
            crit_rate_bp: ratio(panel.crit_rate),
            crit_damage_bp: ratio(panel.crit_damage),
            element_damage_bp: panel.elemental_damage.map(|value| ratio(Some(value))),
            effect_hit_rate_bp: ratio(panel.effect_hit_rate),
            effect_resistance_bp: ratio(panel.effect_resistance),
            break_effect_bp: ratio(panel.break_effect),
            energy_regeneration_bp: ratio(panel.energy_regeneration),
            healing_bp: ratio(panel.healing_boost),
        };
        let enabled_trace_ids = variant
            .traces
            .iter()
            .filter(|(id, trace)| {
                config.unlocked_trace_ids.contains(id)
                    || (trace.automatic && trace.requirement.met(config.level, config.ascension))
            })
            .map(|(id, _)| id.clone())
            .collect();
        Ok(Some(OfficialCharacter {
            unit_id: id.clone(),
            form_id: id.clone(),
            combat_type: CombatType { path, element },
            energy_capacity,
            level: config.level,
            stats,
            skill_parameters,
            eidolon: config.eidolon,
            enabled_trace_ids,
        }))
    }
}
