use std::sync::Arc;

use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::percentage_of},
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "trailblazer.harmony";
const BASIC: &str = "skill.800501";
const SKILL: &str = "skill.800502";
const ULTIMATE: &str = "skill.800503";
const TALENT: &str = "skill.800504";
const DANCE: &str = "trace.8005101";
const SHUFFLE: &str = "trace.8005102";
const HAT: &str = "trace.8005103";
const IMAGINARY: Element = Element::Imaginary;
const E2_ENERGY_REGEN_BP: u32 = 2_500;
const E2_TURNS: u32 = 3;
const E4_SHARE_BP: u32 = 1_500;

#[derive(Clone, Default)]
pub(in crate::battle) struct HarmonyTrailblazerState {
    backup_dancer_until_turn: u32,
    refunded_skill: bool,
}

struct HarmonyTrailblazer {
    eidolon: u8,
    dancer_turns: u32,
    dancer_break_effect_bp: u32,
    talent_energy_milli: u32,
    dance: bool,
    hat: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != IMAGINARY
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }

    // The frozen catalog maps both 8005 and 8006 to the 8005 skill and trace IDs.
    let basic_bp = ratio_bp(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = input.parameters(SKILL, 2)?;
    let skill_bp = ratio_bp(SKILL, skill[0])?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 1)?[0];
    if skill[1] != 2.0 || ultimate[0] != 3.0 || ultimate[1] != 1.0 {
        return Err(invalid("skill or ultimate mechanics"));
    }
    let energy_milli = (talent * 1_000.0).round();
    if !(5_000.0..=12_500.0).contains(&energy_milli) {
        return Err(invalid(TALENT));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let first_toughness = if has_trace(SHUFFLE) { 20 } else { 10 };
    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basic_bp, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![
                hit(skill_bp, first_toughness),
                Effect::RandomHits {
                    element: IMAGINARY,
                    multiplier_bp: skill_bp,
                    toughness: 5,
                    count: 4 + 2 * u8::from(input.eidolon >= 6),
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(HarmonyTrailblazer {
            eidolon: input.eidolon,
            dancer_turns: 3,
            dancer_break_effect_bp: u32::from(ratio_bp(ULTIMATE, ultimate[2])?),
            talent_energy_milli: energy_milli as u32,
            dance: has_trace(DANCE),
            hat: has_trace(HAT),
        }),
    })
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: IMAGINARY,
        multiplier_bp,
        toughness,
    }
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(skill: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn backup_dancer_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    unit.normal_turns < unit.harmony_trailblazer_state.backup_dancer_until_turn
}

impl Mechanics for HarmonyTrailblazer {
    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if action == Action::Skill {
            battle.gain_energy(actor, actor, 6_000);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let unit = &mut battle.units[owner];
        let state = &mut unit.harmony_trailblazer_state;
        match action {
            Action::Ultimate => {
                state.backup_dancer_until_turn =
                    unit.normal_turns.saturating_add(self.dancer_turns);
            }
            Action::Skill | Action::EnhancedSkill if self.eidolon >= 1 && !state.refunded_skill => {
                state.refunded_skill = true;
                battle.restore_skill_points(owner, 1);
            }
            Action::Basic | Action::Skill | Action::EnhancedSkill | Action::EnhancedUltimate => {}
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: usize,
        _action: Action,
    ) {
        if !battle.allies().contains(&actor) || !battle.is_targetable(owner) {
            return;
        }
        battle.gain_energy(owner, owner, self.talent_energy_milli);
        if self.hat && battle.is_targetable(target) {
            battle.delay_action(owner, target, 3_000);
        }
    }

    fn energy_regeneration_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        E2_ENERGY_REGEN_BP
            * u32::from(self.eidolon >= 2 && battle.units[actor].finished_turns < E2_TURNS)
    }

    fn ally_break_effect_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let dancer = self.dancer_break_effect_bp * u32::from(backup_dancer_active(battle, owner));
        let share = if self.eidolon >= 4 && target != owner {
            percentage_of(battle.effective_break_effect(owner), E4_SHARE_BP)
        } else {
            0
        };
        dancer.saturating_add(share)
    }

    fn provides_super_break(&self) -> bool {
        true
    }

    fn super_break_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _actor: usize,
        _target: usize,
    ) -> u16 {
        if !backup_dancer_active(battle, owner) {
            return 0;
        }
        let enemies = battle
            .enemies()
            .filter(|&target| battle.is_targetable(target))
            .count();
        match (self.dance, enemies) {
            (false, _) => 10_000,
            (true, 0 | 1) => 16_000,
            (true, 2) => 15_000,
            (true, 3) => 14_000,
            (true, 4) => 13_000,
            (true, _) => 12_000,
        }
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Ultimate).then_some(0)
    }
}
