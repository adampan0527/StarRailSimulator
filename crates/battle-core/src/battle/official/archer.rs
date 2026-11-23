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

pub(super) const FORM: &str = "character.1015";
const BASIC: &str = "skill.101501";
const SKILL: &str = "skill.101502";
const ULTIMATE: &str = "skill.101503";
const TALENT: &str = "skill.101504";
const SKILL_POINT_CAP_TRACE: &str = "trace.1015101";
const INITIAL_CHARGE_TRACE: &str = "trace.1015102";
const CRIT_DAMAGE_TRACE: &str = "trace.1015103";
const ENERGY_CAP: u32 = 220;
const SKILL_COST: u8 = 2;
const CIRCUIT_LIMIT: u8 = 5;
const SKILL_DAMAGE_STACK_CAP: u8 = 2;
const ULTIMATE_CHARGE: u8 = 2;
const CHARGE_CAP: u8 = 4;
const CRIT_DAMAGE_MIN_SKILL_POINTS: u8 = 4;
const QUANTUM: Element = Element::Quantum;

#[derive(Clone, Default)]
pub(in crate::battle) struct ArcherState {
    charge: u8,
    circuit_skills: u8,
}

impl ArcherState {
    pub(super) fn circuit_is_full(&self) -> bool {
        self.circuit_skills >= CIRCUIT_LIMIT
    }
}

struct Archer {
    end: OfficialAbility,
    follow_up: FollowUpAttack,
    skill_damage_per_stack_bp: u32,
    raises_skill_point_cap: bool,
    initial_charge: bool,
    crit_damage_trace: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Hunt
        || input.combat_type.element != QUANTUM
    {
        return Err(invalid("energy, combat type or eidolon"));
    }

    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 1)?[0];
    if skill[2] != f64::from(SKILL_DAMAGE_STACK_CAP)
        || skill[3] != 1.0
        || skill[4] != f64::from(CIRCUIT_LIMIT)
    {
        return Err(invalid(SKILL));
    }
    if ultimate[1] != f64::from(ULTIMATE_CHARGE) || ultimate[2] != f64::from(CHARGE_CAP) {
        return Err(invalid(ULTIMATE));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(BASIC, basic, 10)?],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![hit(SKILL, skill[0], 20)?],
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 5_000,
            effects: vec![hit(ULTIMATE, ultimate[0], 30)?],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Archer {
            end: OfficialAbility {
                target: Target::SelfOnly,
                energy_milli: 0,
                effects: Vec::new(),
                skill_points: SkillPoints::Unchanged,
            },
            follow_up: FollowUpAttack {
                element: QUANTUM,
                multiplier_bp: basis_points(TALENT, talent)?,
                toughness: 10,
                energy_milli: 5_000,
                first_hit: None,
            },
            skill_damage_per_stack_bp: u32::from(basis_points(SKILL, skill[1])?),
            raises_skill_point_cap: trace(SKILL_POINT_CAP_TRACE),
            initial_charge: trace(INITIAL_CHARGE_TRACE),
            crit_damage_trace: trace(CRIT_DAMAGE_TRACE),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("Archer invalid {detail}"))
}

fn basis_points(id: &str, multiplier: f64) -> Result<u16, BattleError> {
    let bp = (multiplier * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit(id: &str, multiplier: f64, toughness: u16) -> Result<Effect, BattleError> {
    let multiplier_bp = (multiplier * 10_000.0).round();
    if !(1.0..=u32::MAX as f64).contains(&multiplier_bp) {
        return Err(invalid(id));
    }

    // Archer's Ultimate can exceed the multiplier of a single standard Hit.
    Ok(Effect::WideHit {
        element: QUANTUM,
        multiplier_bp: multiplier_bp as u32,
        toughness,
    })
}

fn set_charge(battle: &mut Battle<'_>, actor: usize, charge: u8) {
    let charge = charge.min(CHARGE_CAP);
    let before = std::mem::replace(&mut battle.units[actor].archer_state.charge, charge);
    if before != charge {
        battle.emit(
            EventKind::ArcherCharge {
                before,
                after: charge,
            },
            Some(actor),
            None,
        );
    }
}

fn circuit_active(battle: &Battle<'_>, actor: usize) -> bool {
    battle.units[actor].archer_state.circuit_skills > 0
}

impl Mechanics for Archer {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.initial_charge {
            set_charge(battle, actor, 1);
        }
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::EnhancedSkill && circuit_active(battle, actor)).then_some(&self.end)
    }

    fn can_use_basic(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !circuit_active(battle, actor)
    }

    fn shared_skill_point_cost(&self, _: &Battle<'_>, _: usize, action: Action) -> u8 {
        SKILL_COST * u8::from(action == Action::Skill)
    }

    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn ends_turn_without_ability(&self, action: Action) -> bool {
        action == Action::EnhancedSkill
    }

    fn ends_retained_turn(&self, battle: &Battle<'_>, actor: usize) -> bool {
        circuit_active(battle, actor)
            && (battle.units[actor].archer_state.circuit_is_full()
                || battle.skill_points < SKILL_COST
                || !battle.enemies().any(|enemy| battle.is_targetable(enemy)))
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        if owner == actor {
            battle.units[actor].archer_state.circuit_skills = 0;
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if circuit_active(battle, owner) {
            battle.end_retained_turn(owner);
            battle.units[owner].archer_state.circuit_skills = 0;
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].archer_state.circuit_skills = 0;
        set_charge(battle, actor, 0);
        battle.set_skill_points(actor, battle.skill_points.min(battle.skill_point_cap()));
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Skill {
            battle.spend_skill_points(actor, SKILL_COST);
        }
        if action == Action::Ultimate
            && self.eidolon >= 2
            && let Some(target) = target
        {
            battle.implant_weakness(actor, target, QUANTUM, 2);
            battle.apply_resistance_down(actor, target, QUANTUM, 2_000, 2);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            let state = &mut battle.units[actor].archer_state;
            state.circuit_skills += 1;
            if self.eidolon >= 1 && state.circuit_skills == 3 {
                battle.restore_skill_points(actor, 2);
            }
        } else if action == Action::Ultimate {
            let charge = battle.units[actor].archer_state.charge;
            set_charge(battle, actor, charge + ULTIMATE_CHARGE);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if kind != DamageKind::Direct {
            return 0;
        }
        match origin {
            Some(Action::Skill) => {
                let cap = SKILL_DAMAGE_STACK_CAP + u8::from(self.eidolon >= 6);
                let stacks = battle.units[actor].archer_state.circuit_skills.min(cap);
                self.skill_damage_per_stack_bp * u32::from(stacks)
            }
            Some(Action::Ultimate) if self.eidolon >= 4 => 15_000,
            _ => 0,
        }
    }

    fn ally_skill_defense_ignore_bp(&self, _: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        2_000 * u32::from(owner == actor && self.eidolon >= 6)
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        primary_target: Option<usize>,
    ) {
        if !battle.is_party_unit(actor)
            || battle.memosprite_owner(actor).unwrap_or(actor) == owner
            || !self.can_execute_follow_up(battle, owner)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return;
        }
        let target = primary_target.or_else(|| battle.attack_targets().first().copied());
        battle.follow_up_single_or_random(owner, target, origin, self.follow_up.clone());
    }

    fn can_execute_follow_up(&self, battle: &Battle<'_>, actor: usize) -> bool {
        battle.units[actor].archer_state.charge > 0
            && battle.is_targetable(actor)
            && !battle.is_crowd_controlled(actor)
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        set_charge(battle, actor, battle.units[actor].archer_state.charge - 1);
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _: Action) {
        battle.restore_skill_points(actor, 1);
    }

    fn skill_point_cap_bonus(&self) -> u8 {
        2 * u8::from(self.raises_skill_point_cap)
    }

    fn on_ally_skill_points_restored(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        _: u8,
    ) {
        if self.crit_damage_trace && battle.skill_points >= CRIT_DAMAGE_MIN_SKILL_POINTS {
            battle.apply_ally_buff_with_id(
                owner,
                owner,
                AllyBuffKind::CritDamage,
                12_000,
                1,
                Some(CRIT_DAMAGE_TRACE),
            );
        }
    }

    fn on_ally_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        if owner == actor && self.eidolon >= 6 {
            battle.restore_skill_points(owner, 1);
        }
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        (action == Action::EnhancedSkill).then_some(5)
    }
}
