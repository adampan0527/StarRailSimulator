use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1220";
const BASIC: &str = "skill.122001";
const SKILL: &str = "skill.122002";
const ULTIMATE: &str = "skill.122003";
const TALENT: &str = "skill.122004";
const BOLTSUNDER_BLITZ: &str = "skill.122008";
const WARAXE_SKYWARD: &str = "skill.122009";
const HEAVENPATH: &str = "trace.1220101";
const FORMSHIFT: &str = "trace.1220102";
const BOLTING_GRASP: &str = "trace.1220103";

const ULTIMATE_COST_HALVES: u8 = 12;
const AUREUS_CAP_HALVES: u8 = 24;
const ULTIMATE_STRIKES: u8 = 6;
const E1_CAP: u8 = 5;
const E2_CAP: u8 = 6;

#[derive(Clone, Default)]
pub(in crate::battle) struct FeixiaoState {
    aureus_halves: u8,
    talent_triggered: bool,
    eidolon_follow_ups: u8,
    ultimate_strikes_resolved: u8,
}

struct Feixiao {
    eidolon: u8,
    heavenpath: bool,
    formshift: bool,
    bolting_grasp: bool,
    talent_multiplier_bp: u16,
    talent_damage_bp: u32,
    unbroken_multiplier_bp: u16,
    broken_multiplier_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 0
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Hunt
        || input.combat_type.element != Element::Wind
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 1)?[0];
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 6)?;
    let boltsunder = input.parameters(BOLTSUNDER_BLITZ, 2)?;
    let waraxe = input.parameters(WARAXE_SKYWARD, 2)?;
    if ultimate[1] != 1.0
        || ultimate[2] != f64::from(ULTIMATE_STRIKES)
        || talent[1..4] != [2.0, 6.0, 12.0]
        || talent[5] != 2.0
    {
        return Err(invalid("Flying Aureus, Ultimate or Talent parameters"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let waraxe_bp = ratio_bp(WARAXE_SKYWARD, waraxe[0])?;
    let talent_multiplier_bp = ratio_bp(
        TALENT,
        talent[0] + if input.eidolon == 6 { 1.4 } else { 0.0 },
    )?;
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: wind_attack(
            SkillPoints::Generate,
            vec![hit(ratio_bp(BASIC, basic)?, 10)],
        ),
        skill: wind_attack(SkillPoints::Spend, vec![hit(ratio_bp(SKILL, skill)?, 20)]),
        ultimate: wind_attack(
            SkillPoints::Unchanged,
            vec![
                Effect::RepeatingSingleHits {
                    element: Element::Wind,
                    multiplier_bp: waraxe_bp,
                    toughness: 5,
                    initial_hits: ULTIMATE_STRIKES,
                    extra_hits: 0,
                    extra_chance_bp: 0,
                },
                // The finisher has a separate row (122014) with zero toughness.
                hit(ratio_bp(ULTIMATE, ultimate[0])?, 0),
            ],
        ),
        mechanics: Arc::new(Feixiao {
            eidolon: input.eidolon,
            heavenpath: trace(HEAVENPATH),
            formshift: trace(FORMSHIFT),
            bolting_grasp: trace(BOLTING_GRASP),
            talent_multiplier_bp,
            talent_damage_bp: u32::from(ratio_bp(TALENT, talent[4])?),
            unbroken_multiplier_bp: ratio_bp(WARAXE_SKYWARD, waraxe[0] + waraxe[1])?,
            broken_multiplier_bp: ratio_bp(BOLTSUNDER_BLITZ, boltsunder[0] + boltsunder[1])?,
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

fn wind_attack(skill_points: SkillPoints, effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli: 0,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Wind,
        multiplier_bp,
        toughness,
    }
}

impl Feixiao {
    fn change_aureus(battle: &mut Battle<'_>, source: usize, owner: usize, halves: i8) {
        let state = &mut battle.units[owner].feixiao_state;
        let before_halves = state.aureus_halves;
        let after_halves = before_halves
            .saturating_add_signed(halves)
            .min(AUREUS_CAP_HALVES);
        if before_halves != after_halves {
            state.aureus_halves = after_halves;
            battle.emit(
                EventKind::FlyingAureus {
                    before_halves,
                    after_halves,
                },
                Some(source),
                Some(owner),
            );
        }
    }

    fn follow_up_attack(&self) -> FollowUpAttack {
        FollowUpAttack {
            element: Element::Wind,
            multiplier_bp: self.talent_multiplier_bp,
            toughness: if self.eidolon >= 4 { 10 } else { 5 },
            energy_milli: 0,
            first_hit: None,
        }
    }
}

impl Mechanics for Feixiao {
    fn activate_special_ultimate(&self, battle: &mut Battle<'_>, owner: usize) {
        let missing =
            ULTIMATE_COST_HALVES.saturating_sub(battle.units[owner].feixiao_state.aureus_halves);
        Self::change_aureus(battle, owner, owner, missing as i8);
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        Some(battle.units[actor].feixiao_state.aureus_halves >= ULTIMATE_COST_HALVES)
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.heavenpath {
            Self::change_aureus(battle, owner, owner, 6);
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner != actor {
            return;
        }
        let state = &mut battle.units[owner].feixiao_state;
        let triggered = std::mem::take(&mut state.talent_triggered);
        state.eidolon_follow_ups = 0;
        if self.heavenpath && !triggered && battle.units[owner].normal_turns > 1 {
            Self::change_aureus(battle, owner, owner, 1);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Ultimate => {
                Self::change_aureus(battle, actor, actor, -(ULTIMATE_COST_HALVES as i8));
                battle.units[actor].feixiao_state.ultimate_strikes_resolved = 0;
            }
            Action::Skill if self.bolting_grasp => battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::AttackPercent,
                4_800,
                3,
                Some(BOLTING_GRASP),
            ),
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Skill
            && let Some(target) = target.filter(|&target| battle.is_targetable(target))
        {
            battle.follow_up_single(actor, target, action, self.follow_up_attack());
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
        primary_target: Option<usize>,
    ) {
        let kind = if battle.resolving_follow_up {
            DamageKind::FollowUp
        } else {
            DamageKind::Direct
        };
        if source == owner && origin == Action::Ultimate && kind == DamageKind::Direct {
            return;
        }
        let follow_up = battle.is_follow_up_damage(source, Some(origin), kind);
        let state = &mut battle.units[owner].feixiao_state;
        let halves = if self.eidolon >= 2 && follow_up && state.eidolon_follow_ups < E2_CAP {
            state.eidolon_follow_ups += 1;
            2
        } else {
            1
        };
        Self::change_aureus(battle, source, owner, halves);
        if source != owner
            && !battle.units[owner].feixiao_state.talent_triggered
            && battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            battle.units[owner].feixiao_state.talent_triggered = true;
            battle.follow_up_single_or_random(
                owner,
                primary_target,
                origin,
                self.follow_up_attack(),
            );
        }
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.apply_ally_buff_with_id(
            actor,
            actor,
            AllyBuffKind::Damage,
            self.talent_damage_bp,
            2,
            Some(TALENT),
        );
        if self.eidolon >= 4 {
            battle.apply_speed_up(actor, actor, 800, 2);
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if action == Action::Ultimate {
            let strikes = &mut battle.units[actor].feixiao_state.ultimate_strikes_resolved;
            *strikes += 1;
        }
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
        multiplier_bp: u32,
    ) -> u32 {
        if !is_primary_ultimate(action, kind) {
            return multiplier_bp;
        }
        let strikes_resolved = battle.units[actor].feixiao_state.ultimate_strikes_resolved;
        let multiplier_bp = if strikes_resolved >= ULTIMATE_STRIKES {
            multiplier_bp
        } else if battle.units[target].toughness == 0 {
            u32::from(self.broken_multiplier_bp)
        } else {
            u32::from(self.unbroken_multiplier_bp)
        };
        let e1_stacks = if self.eidolon >= 1 {
            strikes_resolved.min(E1_CAP)
        } else {
            0
        };
        (multiplier_bp.saturating_mul(10 + u32::from(e1_stacks)) / 10).min(u32::from(u16::MAX))
    }

    fn ignores_weakness(&self, action: Option<Action>, kind: DamageKind) -> bool {
        is_primary_ultimate(action, kind)
    }

    fn toughness_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        10_000 * u32::from(is_primary_ultimate(action, kind))
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        3_600 * u32::from(self.formshift && battle.is_follow_up_damage(actor, action, kind))
    }

    fn ultimate_counts_as_follow_up(&self) -> bool {
        self.formshift
    }

    fn follow_up_counts_as_ultimate(&self, _origin: Option<Action>) -> bool {
        self.eidolon == 6
    }

    fn ultimate_resistance_penetration_bp(&self) -> u32 {
        2_000 * u32::from(self.eidolon == 6)
    }
}

fn is_primary_ultimate(action: Option<Action>, kind: DamageKind) -> bool {
    action == Some(Action::Ultimate) && kind == DamageKind::Direct
}
