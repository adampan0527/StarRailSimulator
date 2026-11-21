use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1312";
const BASIC: &str = "skill.131201";
const SKILL: &str = "skill.131202";
const ULTIMATE: &str = "skill.131203";
const TALENT: &str = "skill.131204";
const RELEASE: &str = "trace.1312101";
const INTERLOCK: &str = "trace.1312102";
const TRANSMISSION: &str = "trace.1312103";

const BASE_ULTIMATE_HITS: u8 = 3;
const MAX_ACCUMULATED_HITS: u8 = 10;
const MAX_E1_HITS: usize = 5;
const E2_CHANCE_BP: u32 = 2_400;
const E2_DEFENSE_DOWN_BP: u32 = 1_600;
const E2_TURNS: u32 = 3;
const E4_MULTIPLIER_BP: u16 = 600;
const E6_DAMAGE_BP: u32 = 3_000;
const RELEASE_CHANCE_BP: u32 = 8_000;
const INTERLOCK_HIT_RATE_BP: u32 = 6_000;
const TRANSMISSION_CRIT_DAMAGE_BP: u32 = 3_000;

#[derive(Clone, Copy)]
struct UltimateCast {
    extra_bounces: u8,
    first_hit: bool,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct MishaState {
    accumulated_hits: u8,
    active_ultimate: Option<UltimateCast>,
    e6_damage_active: bool,
    e6_refund_ready: bool,
}

struct Misha {
    eidolon: u8,
    release: bool,
    interlock: bool,
    transmission: bool,
    freeze_chance_bp: u32,
    freeze_damage_bp: u16,
    talent_energy_milli: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let basic = ratio_bp(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 2)?;
    if skill[2] != 1.0 {
        return Err(invalid(SKILL));
    }
    if ultimate[0] != f64::from(BASE_ULTIMATE_HITS)
        || ultimate[4] != f64::from(MAX_ACCUMULATED_HITS)
        || ultimate[2] > 1.0
    {
        return Err(invalid(ULTIMATE));
    }
    if !(1.0..=2.5).contains(&talent[0]) || talent[1] != 1.0 {
        return Err(invalid(TALENT));
    }

    let ultimate_multiplier_bp = ratio_bp(ULTIMATE, ultimate[1])?
        .checked_add(E4_MULTIPLIER_BP * u16::from(input.eidolon >= 4))
        .ok_or_else(|| invalid(ULTIMATE))?;
    let trace_enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: ice_attack(20_000, SkillPoints::Generate, vec![ice_hit(basic, 10)]),
        skill: ice_attack(
            30_000,
            SkillPoints::Spend,
            vec![
                ice_hit(ratio_bp(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Ice,
                    multiplier_bp: ratio_bp(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
        ),
        ultimate: ice_attack(
            5_000,
            SkillPoints::Unchanged,
            vec![
                ice_hit(ultimate_multiplier_bp, 10),
                Effect::RandomHits {
                    element: Element::Ice,
                    multiplier_bp: ultimate_multiplier_bp,
                    toughness: 5,
                    count: BASE_ULTIMATE_HITS - 1,
                },
            ],
        ),
        mechanics: Arc::new(Misha {
            eidolon: input.eidolon,
            release: trace_enabled(RELEASE),
            interlock: trace_enabled(INTERLOCK),
            transmission: trace_enabled(TRANSMISSION),
            freeze_chance_bp: u32::from(ratio_bp(ULTIMATE, ultimate[2])?),
            freeze_damage_bp: ratio_bp(ULTIMATE, ultimate[3])?,
            talent_energy_milli: (talent[0] * 1_000.0).round() as u32,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn ice_attack(
    energy_milli: u32,
    skill_points: SkillPoints,
    effects: Vec<Effect>,
) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn ice_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Ice,
        multiplier_bp,
        toughness,
    }
}

impl MishaState {
    fn add_hits(&mut self, hits: u8) {
        self.accumulated_hits = self
            .accumulated_hits
            .saturating_add(hits)
            .min(MAX_ACCUMULATED_HITS - BASE_ULTIMATE_HITS);
    }
}

impl Mechanics for Misha {
    fn on_ally_skill_points_spent(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        points: u8,
    ) {
        battle.units[owner].misha_state.add_hits(points);
        battle.gain_energy(owner, owner, self.talent_energy_milli * u32::from(points));
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Basic => {}
            Action::Skill => battle.units[actor].misha_state.add_hits(1),
            Action::Ultimate | Action::EnhancedUltimate => {
                let eidolon_hits = if self.eidolon >= 1 {
                    battle
                        .enemies()
                        .filter(|&target| battle.hp(target).0 > 0)
                        .count()
                        .min(MAX_E1_HITS) as u8
                } else {
                    0
                };
                let state = &mut battle.units[actor].misha_state;
                state.active_ultimate = Some(UltimateCast {
                    extra_bounces: state.accumulated_hits + eidolon_hits,
                    first_hit: true,
                });
                if self.eidolon == 6 {
                    state.e6_damage_active = true;
                    state.e6_refund_ready = true;
                }
            }
        }
    }

    fn random_hit_count(&self, battle: &Battle<'_>, actor: usize, base: u8) -> u8 {
        base + battle.units[actor]
            .misha_state
            .active_ultimate
            .map_or(0, |cast| cast.extra_bounces)
    }

    fn before_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
    ) {
        let Some(cast) = &mut battle.units[actor].misha_state.active_ultimate else {
            return;
        };
        let first_hit = std::mem::take(&mut cast.first_hit);
        let chance_bp =
            self.freeze_chance_bp + RELEASE_CHANCE_BP * u32::from(first_hit && self.release);
        battle.apply_skill_freeze(actor, target, chance_bp, 1, self.freeze_damage_bp, action);
        if self.eidolon >= 2 && battle.roll_effect(actor, target, E2_CHANCE_BP) {
            battle.apply_defense_down(actor, target, E2_DEFENSE_DOWN_BP, E2_TURNS);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let state = &mut battle.units[actor].misha_state;
        match action {
            Action::Basic => {}
            Action::Skill => {
                if std::mem::take(&mut state.e6_refund_ready) {
                    battle.restore_skill_points(actor, 1);
                }
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                state.active_ultimate = None;
                state.accumulated_hits = 0;
            }
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            battle.units[owner].misha_state.e6_damage_active = false;
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        E6_DAMAGE_BP * u32::from(battle.units[actor].misha_state.e6_damage_active)
    }

    fn effect_hit_rate_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        INTERLOCK_HIT_RATE_BP
            * u32::from(self.interlock && battle.units[actor].misha_state.active_ultimate.is_some())
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        TRANSMISSION_CRIT_DAMAGE_BP * u32::from(self.transmission && battle.is_frozen(target))
    }
}
