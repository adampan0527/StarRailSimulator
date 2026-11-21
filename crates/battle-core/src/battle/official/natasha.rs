use std::sync::Arc;

use super::{
    super::Battle,
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::{Healing, MICROS, healing_micros},
};
use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110501";
const SKILL: &str = "skill.110502";
const ULTIMATE: &str = "skill.110503";
const TALENT: &str = "skill.110504";
const BP: u32 = 10_000;
const EIDOLON_HP_THRESHOLD_BP: u32 = 3_000;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 90 {
        return Err(BattleError::InvalidInput("Natasha energy capacity".into()));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    let multiplier = (basic * f64::from(BP)).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&multiplier)
        || !(1.0..=u8::MAX.into()).contains(&skill[2])
        || skill[2].fract() != 0.0
        || talent[0] > 1.0
    {
        return Err(BattleError::InvalidInput("Natasha skill parameters".into()));
    }
    let has_trace = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let mechanics = Natasha {
        skill: Healing::from_data("Natasha", SKILL, skill[0], skill[3])?,
        skill_hot: Healing::from_data("Natasha", SKILL, skill[1], skill[4])?,
        skill_hot_turns: skill[2] as u32 + u32::from(has_trace("trace.1105103")),
        ultimate: Healing::from_data("Natasha", ULTIMATE, ultimate[0], ultimate[1])?,
        emergency_threshold_bp: (talent[0] * f64::from(BP)).round() as u32,
        emergency_boost_micros: healing_micros("Natasha", TALENT, talent[1], 10.0)?,
        outgoing_boost_micros: u64::from(input.stats.healing_bp) * (MICROS / u64::from(BP))
            + if has_trace("trace.1105102") {
                100_000
            } else {
                0
            },
        cleanse: has_trace("trace.1105101"),
        eidolon: input.eidolon,
    };
    Ok(OfficialKit {
        max_energy_milli: 90_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Physical,
                multiplier_bp: multiplier as u16,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
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

const E1_HEAL: Healing = Healing {
    hp_ratio_micros: 150_000,
    flat_micros: 400 * MICROS,
};
const E2_HOT: Healing = Healing {
    hp_ratio_micros: 60_000,
    flat_micros: 160 * MICROS,
};

fn low_hp(battle: &Battle<'_>, unit: usize, threshold_bp: u32) -> bool {
    let (hp, max_hp) = battle.hp(unit);
    hp > 0 && u64::from(hp) * u64::from(BP) <= u64::from(max_hp) * u64::from(threshold_bp)
}

struct Natasha {
    skill: Healing,
    skill_hot: Healing,
    skill_hot_turns: u32,
    ultimate: Healing,
    emergency_threshold_bp: u32,
    emergency_boost_micros: u64,
    outgoing_boost_micros: u64,
    cleanse: bool,
    eidolon: u8,
}

impl Natasha {
    fn heal(&self, battle: &mut Battle<'_>, actor: usize, target: usize, healing: Healing) {
        if battle.hp(target).0 == 0 {
            return;
        }
        let mut bonus = self.outgoing_boost_micros;
        if low_hp(battle, target, self.emergency_threshold_bp) {
            bonus += self.emergency_boost_micros;
        }
        battle.heal_unit(actor, target, healing.amount(battle.hp(actor).1, bonus));
    }
}

impl Mechanics for Natasha {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Ultimate && self.eidolon >= 2 {
            for ally in battle.allies() {
                if low_hp(battle, ally, EIDOLON_HP_THRESHOLD_BP) {
                    battle.apply_heal_over_time(actor, ally, 1, Action::Ultimate);
                }
            }
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => {
                if self.cleanse {
                    battle.cleanse_one(actor, target);
                }
                self.heal(battle, actor, target, self.skill);
                battle.apply_heal_over_time(actor, target, self.skill_hot_turns, Action::Skill);
            }
            (Action::Ultimate, _) => {
                for ally in battle.allies() {
                    self.heal(battle, actor, ally, self.ultimate);
                }
            }
            _ => {}
        }
    }

    fn heal_over_time(&self, battle: &mut Battle<'_>, actor: usize, target: usize, origin: Action) {
        let healing = match origin {
            Action::Skill => self.skill_hot,
            Action::Ultimate | Action::EnhancedUltimate => E2_HOT,
            Action::Basic => return,
        };
        self.heal(battle, actor, target, healing);
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if self.eidolon >= 6 && action == Action::Basic {
            battle.secondary_hit_from_hp(actor, target, Element::Physical, 4_000, action);
        }
    }

    fn after_enemy_attack(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1
            && low_hp(battle, actor, EIDOLON_HP_THRESHOLD_BP)
            && battle.take_once_per_battle(actor)
        {
            self.heal(battle, actor, actor, E1_HEAL);
        }
        if self.eidolon >= 4 {
            battle.gain_energy(actor, actor, 5_000);
        }
    }
}
