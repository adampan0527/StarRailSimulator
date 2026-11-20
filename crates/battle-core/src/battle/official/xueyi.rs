use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1214";
const BASIC: &str = "skill.121401";
const SKILL: &str = "skill.121402";
const ULTIMATE: &str = "skill.121403";
const TALENT: &str = "skill.121404";
const CLAIRVOYANT_LOOM: &str = "trace.1214101";
const INTREPID_ROLLERBEARINGS: &str = "trace.1214102";
const PERSPICACIOUS_MAINFRAME: &str = "trace.1214103";
const TOUGHNESS_PER_KARMA: u16 = 10;
const ULTIMATE_TOUGHNESS: u16 = 40;

#[derive(Clone, Default)]
pub(in crate::battle) struct XueyiState {
    karma: u8,
    attack_toughness_reduced: u16,
    break_effect_expires_after_turn: u32,
}

struct Xueyi {
    eidolon: u8,
    clairvoyant_loom: bool,
    intrepid_rollerbearings: bool,
    perspicacious_mainframe: bool,
    ultimate_bonus_per_ten_bp: u16,
    ultimate_bonus_cap_bp: u16,
    follow_up_multiplier_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity or eidolon"
        )));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 3)?;
    if talent[0] != 8.0 || talent[2] != 1.0 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: Karma parameters"
        )));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![quantum_hit(basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                quantum_hit(basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Quantum,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 5_000,
            effects: vec![quantum_hit(
                basis_points(ULTIMATE, ultimate[0])?,
                ULTIMATE_TOUGHNESS,
            )],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Xueyi {
            eidolon: input.eidolon,
            clairvoyant_loom: trace(CLAIRVOYANT_LOOM),
            intrepid_rollerbearings: trace(INTREPID_ROLLERBEARINGS),
            perspicacious_mainframe: trace(PERSPICACIOUS_MAINFRAME),
            ultimate_bonus_per_ten_bp: basis_points(ULTIMATE, ultimate[1])?,
            ultimate_bonus_cap_bp: basis_points(ULTIMATE, ultimate[2])?,
            follow_up_multiplier_bp: basis_points(TALENT, talent[1])?,
        }),
    })
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {id} multiplier"
        )));
    }
    Ok(bp as u16)
}

fn quantum_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Quantum,
        multiplier_bp,
        toughness,
    }
}

impl Mechanics for Xueyi {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if self.eidolon >= 4 && action == Action::Ultimate {
            let state = &mut battle.units[actor];
            state.xueyi_state.break_effect_expires_after_turn = state.finished_turns + 2;
        }
    }

    fn break_effect_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let state = &battle.units[actor];
        4_000 * u32::from(state.finished_turns < state.xueyi_state.break_effect_expires_after_turn)
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let mut bonus = 0;
        if self.clairvoyant_loom {
            let break_effect = battle.plan.units[actor].stats.break_effect_bp
                + self.break_effect_bonus_bp(battle, actor);
            bonus += break_effect.min(24_000);
        }
        if kind == DamageKind::FollowUp && self.eidolon >= 1 {
            bonus += 4_000;
        }
        if kind == DamageKind::Direct && action == Some(Action::Ultimate) {
            let toughness = battle.units[target].toughness;
            let reduced = u32::from(toughness.min(ULTIMATE_TOUGHNESS));
            bonus += (reduced * u32::from(self.ultimate_bonus_per_ten_bp)
                / u32::from(TOUGHNESS_PER_KARMA))
            .min(u32::from(self.ultimate_bonus_cap_bp));
            let max_toughness = battle.plan.units[target].kit.max_toughness();
            if self.intrepid_rollerbearings
                && max_toughness > 0
                && u32::from(toughness) * 2 >= u32::from(max_toughness)
            {
                bonus += 1_000;
            }
        }
        bonus
    }

    fn ignores_weakness(&self, action: Option<Action>, kind: DamageKind) -> bool {
        (kind == DamageKind::Direct && action == Some(Action::Ultimate))
            || (kind == DamageKind::FollowUp && self.eidolon >= 2)
    }

    fn on_toughness_reduced(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        amount: u16,
    ) {
        if owner == source && battle.in_follow_up() {
            return;
        }
        let pending = &mut battle.units[owner].xueyi_state.attack_toughness_reduced;
        *pending = pending.saturating_add(amount);
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        let state = &mut battle.units[owner].xueyi_state;
        let reduced = std::mem::take(&mut state.attack_toughness_reduced);
        let capacity = if self.eidolon >= 6 { 6 } else { 8 };
        let overflow_capacity = 6 * u8::from(self.perspicacious_mainframe);
        let earned = if owner == source {
            reduced / TOUGHNESS_PER_KARMA
        } else {
            u16::from(reduced > 0)
        };
        state.karma =
            (u16::from(state.karma) + earned).min(u16::from(capacity + overflow_capacity)) as u8;
        if state.karma < capacity
            || battle.is_frozen(owner)
            || !battle.enemies().any(|target| battle.units[target].hp > 0)
        {
            return;
        }
        battle.units[owner].xueyi_state.karma -= capacity;
        battle.follow_up_random(
            owner,
            origin,
            3,
            FollowUpAttack {
                element: Element::Quantum,
                multiplier_bp: self.follow_up_multiplier_bp,
                toughness: 5,
                energy_milli: 2_000,
                first_hit: None,
            },
        );
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _: Action) {
        let (hp, max_hp) = battle.hp(actor);
        if self.eidolon >= 2 && hp < max_hp {
            battle.heal_unit(actor, actor, max_hp / 20);
        }
    }
}
