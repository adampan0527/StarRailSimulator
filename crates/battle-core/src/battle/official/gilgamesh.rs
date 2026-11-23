use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        resolve::{DamageTargetAura, JointFollowUpAttack},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints, saber,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1509";
const BASIC: &str = "skill.150909";
const SKILL: &str = "skill.150902";
const ULTIMATE: &str = "skill.150903";
const TALENT: &str = "skill.150904";
const JOINT: &str = "skill.150905";
const EPICS_OPENING: &str = "trace.1509101";
const HEROS_HAUTEUR: &str = "trace.1509102";
const HEGEMONS_STRIFE: &str = "trace.1509103";
const ENERGY_CAP: u32 = 360;
const THUNDER: Element = Element::Thunder;
const INTEREST_THRESHOLD: u16 = 10;
const SPEED_PER_INTEREST_BP: u32 = 1_000;
const BOUNCES: u8 = 10;
const JOINT_THRESHOLD: u8 = 8;
const GOLDEN_RULE_CAP: u8 = 3;

#[derive(Clone, Default)]
pub(in crate::battle) struct GilgameshState {
    interest: u16,
    lifetime_interest: u8,
    piqued: bool,
    basic_target: Option<usize>,
    acknowledgement_expires: Option<u32>,
    burden_expires: Option<u32>,
    joint_count: u8,
    joint_pending: bool,
    golden_rule: u8,
    spent_golden_rule: u8,
}

struct Gilgamesh {
    acknowledgement_bp: u32,
    burden_bp: u32,
    gilgamesh_joint: JointFollowUpAttack,
    saber_joint: JointFollowUpAttack,
    saber_ultimate_factor_bp: u16,
    epics_opening: bool,
    heros_hauteur: bool,
    hegemons_strife: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != THUNDER
    {
        return Err(invalid("energy, combat type or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 7)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 4)?;
    let joint = input.parameters(JOINT, 6)?;
    if skill[3] != f64::from(INTEREST_THRESHOLD) || skill[5] != 3.0 {
        return Err(invalid(SKILL));
    }
    if ultimate[2] != f64::from(BOUNCES) {
        return Err(invalid(ULTIMATE));
    }
    if talent[0] != 3.0
        || talent[1] != f64::from(INTEREST_THRESHOLD)
        || u32::from(ratio_bp(TALENT, talent[3])?) != SPEED_PER_INTEREST_BP
    {
        return Err(invalid(TALENT));
    }
    if joint[2..5] != [3.0, 120.0, f64::from(JOINT_THRESHOLD)] {
        return Err(invalid(JOINT));
    }
    let second = input.eidolon >= 2;
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: ability(
            Target::Enemy,
            20_000,
            vec![hit(ratio_bp(BASIC, basic)?, 10)],
        ),
        skill: ability(
            Target::Enemy,
            30_000,
            vec![
                hit(ratio_bp(SKILL, skill[0] + f64::from(second))?, 20),
                Effect::BlastAdjacent {
                    element: THUNDER,
                    multiplier_bp: ratio_bp(SKILL, skill[1] + 0.5 * f64::from(second))?,
                    toughness: 10,
                },
            ],
        ),
        ultimate: ability(
            Target::AllEnemies,
            5_000,
            vec![
                hit(ratio_bp(ULTIMATE, ultimate[0])?, 40),
                Effect::RandomHits {
                    element: THUNDER,
                    multiplier_bp: ratio_bp(
                        ULTIMATE,
                        ultimate[1] + 0.8 * f64::from(input.eidolon >= 6),
                    )?,
                    toughness: 2,
                    count: BOUNCES,
                },
            ],
        ),
        mechanics: Arc::new(Gilgamesh {
            acknowledgement_bp: u32::from(ratio_bp(SKILL, skill[4])?),
            burden_bp: u32::from(ratio_bp(TALENT, talent[2])?),
            gilgamesh_joint: JointFollowUpAttack {
                element: THUNDER,
                multiplier_bp: joint_ratio_bp(joint[0])?,
                toughness: 10,
                energy_milli: 10_000,
            },
            saber_joint: JointFollowUpAttack {
                element: Element::Wind,
                multiplier_bp: joint_ratio_bp(joint[1])?,
                toughness: 10,
                energy_milli: 0,
            },
            saber_ultimate_factor_bp: ratio_bp(JOINT, joint[5])?,
            epics_opening: trace(EPICS_OPENING),
            heros_hauteur: trace(HEROS_HAUTEUR),
            hegemons_strife: trace(HEGEMONS_STRIFE),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(0.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn ability(target: Target, energy_milli: u32, effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects,
        skill_points: SkillPoints::Unchanged,
    }
}

fn joint_ratio_bp(ratio: f64) -> Result<u32, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(0.0..=100_000.0).contains(&bp) {
        return Err(invalid(JOINT));
    }
    Ok(bp as u32)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: THUNDER,
        multiplier_bp,
        toughness,
    }
}

fn is_saber(battle: &Battle<'_>, actor: usize) -> bool {
    battle.plan.units[actor].official_form.as_deref() == Some(saber::FORM)
}

fn change_interest(battle: &mut Battle<'_>, owner: usize, after: u16) {
    let state = &mut battle.units[owner].gilgamesh_state;
    let before = std::mem::replace(&mut state.interest, after);
    let gained = after.saturating_sub(before).min(6) as u8;
    state.lifetime_interest = state.lifetime_interest.saturating_add(gained).min(6);
    state.piqued |= after >= INTEREST_THRESHOLD;
    if before != after {
        battle.emit(
            EventKind::GilgameshInterest { before, after },
            Some(owner),
            None,
        );
    }
}

fn gain_interest(battle: &mut Battle<'_>, owner: usize, amount: u16) {
    let after = battle.units[owner]
        .gilgamesh_state
        .interest
        .saturating_add(amount);
    change_interest(battle, owner, after);
}

fn change_golden_rule(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].gilgamesh_state.golden_rule, after);
    if before != after {
        battle.emit(
            EventKind::GilgameshGoldenRule { before, after },
            Some(owner),
            None,
        );
    }
}

impl Gilgamesh {
    fn team_offense_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if !self.hegemons_strife || !battle.is_targetable(owner) || !battle.is_party_unit(target) {
            return 0;
        }
        let energy_cap = battle.plan.units[target].kit.max_energy_milli() / 1_000;
        2_000 + 100 * energy_cap.saturating_sub(140).min(100)
    }
}

impl Mechanics for Gilgamesh {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 2 {
            gain_interest(battle, owner, 5);
        }
    }

    fn can_use_basic(&self, battle: &Battle<'_>, owner: usize) -> bool {
        !battle.units[owner].gilgamesh_state.piqued
    }

    fn can_use_skill(&self, battle: &Battle<'_>, owner: usize) -> bool {
        battle.units[owner].gilgamesh_state.piqued
    }

    fn forced_action_target(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<usize> {
        if action == Action::Basic {
            battle.units[owner].gilgamesh_state.basic_target
        } else {
            None
        }
    }

    fn prepare_automatic_action(&self, battle: &mut Battle<'_>, owner: usize) -> Option<Action> {
        if !self.can_use_basic(battle, owner) {
            return None;
        }
        let target = battle.units[owner].gilgamesh_state.basic_target;
        if !target.is_some_and(|target| battle.is_targetable(target)) {
            battle.units[owner].gilgamesh_state.basic_target = battle.random_living_enemy();
        }
        battle.units[owner]
            .gilgamesh_state
            .basic_target
            .map(|_| Action::Basic)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            let unit = &mut battle.units[owner];
            unit.gilgamesh_state.acknowledgement_expires = Some(unit.normal_turns + 3);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Basic => battle.units[owner].gilgamesh_state.basic_target = None,
            Action::Skill => {
                change_interest(battle, owner, 0);
                if self.eidolon >= 1 {
                    battle.restore_energy_flat(owner, owner, 40_000);
                }
            }
            Action::Ultimate => battle.units[owner].gilgamesh_state.spent_golden_rule = 0,
            _ => {}
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            let unit = &mut battle.units[owner];
            let state = &mut unit.gilgamesh_state;
            if state
                .acknowledgement_expires
                .is_some_and(|end| unit.normal_turns >= end)
            {
                state.acknowledgement_expires = None;
            }
            if state
                .burden_expires
                .is_some_and(|end| unit.normal_turns >= end)
            {
                state.burden_expires = None;
            }
        }
    }

    fn on_ally_ultimate_started(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        cost_milli: u32,
    ) {
        if actor == owner {
            gain_interest(
                battle,
                owner,
                2 * u16::from(self.epics_opening) + 5 * u16::from(self.eidolon >= 2),
            );
            let golden_rule = battle.units[owner].gilgamesh_state.golden_rule;
            battle.units[owner].gilgamesh_state.spent_golden_rule = golden_rule;
            change_golden_rule(battle, owner, 0);
            return;
        }
        let unit = &mut battle.units[owner];
        unit.gilgamesh_state.burden_expires = Some(unit.normal_turns + 3);
        if self.epics_opening {
            gain_interest(battle, owner, 2);
            if cost_milli > 0 {
                battle.restore_energy_flat(actor, owner, cost_milli * 30 / 100);
            }
        }
        if self.eidolon >= 6 {
            let after = (battle.units[owner].gilgamesh_state.golden_rule + 1).min(GOLDEN_RULE_CAP);
            change_golden_rule(battle, owner, after);
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: Action,
    ) {
        if actor != owner && battle.is_targetable(owner) {
            gain_interest(battle, owner, 1);
        }
    }

    fn on_memosprite_turn_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if battle.is_targetable(owner) {
            gain_interest(battle, owner, 1);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        if !battle.is_targetable(owner) {
            return;
        }
        if battle.in_follow_up() && actor != owner && battle.is_party_unit(actor) {
            gain_interest(battle, owner, 1);
        }
        let Some(saber) = battle
            .allies()
            .find(|&unit| is_saber(battle, unit) && battle.units[unit].hp > 0)
        else {
            return;
        };
        let state = &mut battle.units[owner].gilgamesh_state;
        if state.joint_pending {
            return;
        }
        if actor == owner || actor == saber {
            state.joint_count = (state.joint_count + 1).min(JOINT_THRESHOLD);
        }
        if state.joint_count != JOINT_THRESHOLD
            || !battle.is_targetable(saber)
            || battle.is_crowd_controlled(owner)
            || battle.is_crowd_controlled(saber)
        {
            return;
        }
        battle.units[owner].gilgamesh_state.joint_pending = true;
        battle.queue_joint_follow_up(
            [(owner, self.gilgamesh_joint), (saber, self.saber_joint)],
            origin,
            0,
        );
    }

    fn on_joint_follow_up_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        partner: usize,
        resolved: bool,
    ) {
        let state = &mut battle.units[owner].gilgamesh_state;
        state.joint_pending = false;
        if resolved {
            state.joint_count = 0;
            gain_interest(battle, owner, 3);
            battle.units[partner]
                .saber_state
                .arm_gilgamesh_ultimate(self.saber_ultimate_factor_bp);
            battle.restore_energy_flat(owner, partner, 120_000);
        }
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        u32::from(battle.units[owner].gilgamesh_state.interest) * SPEED_PER_INTEREST_BP
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if action == Some(Action::Ultimate)
            && kind == DamageKind::Direct
            && battle.units[owner].gilgamesh_state.burden_expires.is_some()
        {
            self.burden_bp
        } else {
            0
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[owner].gilgamesh_state;
        let lifetime = u32::from(state.lifetime_interest) * 2_500 * u32::from(self.heros_hauteur);
        let spent = if action == Some(Action::Ultimate) && kind == DamageKind::Direct {
            u32::from(state.spent_golden_rule) * 10_000
        } else {
            0
        };
        lifetime + spent
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let own_acknowledgement = owner == target
            && self.eidolon >= 1
            && battle.units[owner]
                .gilgamesh_state
                .acknowledgement_expires
                .is_some();
        self.team_offense_bp(battle, owner, target) + 6_000 * u32::from(own_acknowledgement)
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        self.team_offense_bp(battle, owner, target)
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        if !battle.is_targetable(owner) || !battle.is_party_unit(actor) {
            return DamageTargetAura::default();
        }
        let acknowledgement = (owner == actor || self.eidolon >= 1)
            && battle.units[owner]
                .gilgamesh_state
                .acknowledgement_expires
                .is_some();
        DamageTargetAura {
            defense_down_bp: self.acknowledgement_bp * u32::from(acknowledgement),
            resistance_down_bp: 2_000
                * u32::from(self.eidolon >= 6 && battle.allies().contains(&actor)),
            ..DamageTargetAura::default()
        }
    }

    fn energy_regeneration_bonus_bp(&self, _: &Battle<'_>, _: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 4)
    }

    fn ally_ultimate_damage_factor_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if is_saber(battle, actor) {
            battle.units[actor]
                .saber_state
                .gilgamesh_ultimate_factor_bp()
        } else {
            10_000
        }
    }
}
