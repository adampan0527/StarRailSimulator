use std::sync::Arc;

use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

use super::{
    super::Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1104";
const BASIC: &str = "skill.110401";
const SKILL: &str = "skill.110402";
const ULTIMATE: &str = "skill.110403";
const TALENT: &str = "skill.110404";
const COMMANDER: &str = "trace.1104102";
const FIGHTING_SPIRIT: &str = "trace.1104103";

const BASIS_POINTS: u64 = 10_000;
const E1_FREEZE_CHANCE_BP: u32 = 3_500;
const E2_SLOW_BP: u32 = 2_000;
const E4_EFFECT_RESISTANCE_BP: u32 = 2_000;
const E6_EXTRA_RESTORE_BP: u32 = 5_000;
const FIGHTING_SPIRIT_BP: u16 = 3_500;

struct Gepard {
    freeze_chance_bp: u32,
    freeze_turns: u32,
    freeze_damage_bp: u16,
    shield_defense_bp: u16,
    shield_flat_x_bp: u64,
    shield_turns: u32,
    restore_hp_bp: u32,
    eidolon: u8,
    commander: bool,
    fighting_spirit: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }

    let [basic] = input.parameters(BASIC, 1)? else {
        return Err(invalid(BASIC));
    };
    let [skill, freeze_chance, freeze_turns, freeze_damage] = input.parameters(SKILL, 4)? else {
        return Err(invalid(SKILL));
    };
    let [shield_defense, shield_turns, shield_flat] = input.parameters(ULTIMATE, 3)? else {
        return Err(invalid(ULTIMATE));
    };
    let [restore_hp] = input.parameters(TALENT, 1)? else {
        return Err(invalid(TALENT));
    };
    let trace_enabled = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let mut freeze_chance_bp = percent_bp(SKILL, *freeze_chance)?;
    let mut restore_hp_bp = percent_bp(TALENT, *restore_hp)?;
    let shield_flat_x_bp = (*shield_flat * BASIS_POINTS as f64).round();
    if shield_flat_x_bp > f64::from(u32::MAX) * BASIS_POINTS as f64 {
        return Err(invalid(ULTIMATE));
    }
    if input.eidolon >= 1 {
        freeze_chance_bp += E1_FREEZE_CHANCE_BP;
    }
    if input.eidolon >= 6 {
        restore_hp_bp += E6_EXTRA_RESTORE_BP;
    }

    let mechanics = Gepard {
        freeze_chance_bp,
        freeze_turns: turns(SKILL, *freeze_turns)?,
        freeze_damage_bp: ratio_bp(SKILL, *freeze_damage)?,
        shield_defense_bp: ratio_bp(ULTIMATE, *shield_defense)?,
        shield_flat_x_bp: shield_flat_x_bp as u64,
        shield_turns: turns(ULTIMATE, *shield_turns)?,
        restore_hp_bp,
        eidolon: input.eidolon,
        commander: trace_enabled(COMMANDER),
        fighting_spirit: trace_enabled(FIGHTING_SPIRIT),
    };
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![ice_hit(ratio_bp(BASIC, *basic)?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![ice_hit(ratio_bp(SKILL, *skill)?, 20)],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM} {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * BASIS_POINTS as f64).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn percent_bp(id: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio > 1.0 {
        return Err(invalid(id));
    }
    ratio_bp(id, ratio).map(u32::from)
}

fn turns(id: &str, count: f64) -> Result<u32, BattleError> {
    if !(1.0..=f64::from(u16::MAX)).contains(&count) || count.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(count as u32)
}

fn ice_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Ice,
        multiplier_bp,
        toughness,
    }
}

impl Mechanics for Gepard {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => {
                battle.apply_skill_freeze(
                    actor,
                    target,
                    self.freeze_chance_bp,
                    self.freeze_turns,
                    self.freeze_damage_bp,
                    action,
                );
            }
            (Action::Ultimate, None) => {
                let defense = u64::from(battle.defense(actor));
                let shield = (defense * u64::from(self.shield_defense_bp) + self.shield_flat_x_bp)
                    / BASIS_POINTS;
                let shield = u32::try_from(shield).unwrap_or(u32::MAX);
                for ally in battle.allies() {
                    if battle.hp(ally).0 > 0 {
                        battle.apply_official_shield(
                            actor,
                            ally,
                            shield,
                            self.shield_turns,
                            Some(action),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn after_skill_freeze_ended(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if self.eidolon >= 2 {
            battle.apply_speed_down(owner, target, E2_SLOW_BP, 1);
        }
    }

    fn prevent_defeat(&self, battle: &mut Battle<'_>, owner: usize) -> bool {
        if !battle.take_once_per_battle(owner) {
            return false;
        }
        let (_, max_hp) = battle.hp(owner);
        let restore = u64::from(max_hp) * u64::from(self.restore_hp_bp) / BASIS_POINTS;
        let restore = u32::try_from(restore).unwrap_or(u32::MAX).max(1);
        battle.heal_unit(owner, owner, restore);
        if self.commander {
            battle.fill_energy(owner);
        }
        if self.eidolon >= 6 {
            battle.advance_immediate(owner);
        }
        true
    }

    fn attack_conversion_from_defense_bp(&self) -> u32 {
        if self.fighting_spirit {
            u32::from(FIGHTING_SPIRIT_BP)
        } else {
            0
        }
    }

    fn effect_resistance_aura_bp(&self) -> u32 {
        if self.eidolon >= 4 {
            E4_EFFECT_RESISTANCE_BP
        } else {
            0
        }
    }
}
