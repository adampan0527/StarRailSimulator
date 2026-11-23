use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, SummonTurn, resolve::DamageTargetAura, support::AllyBuffKind},
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1414";
const BASIC: &str = "skill.141401";
const SKILL: &str = "skill.141402";
const ULTIMATE: &str = "skill.141403";
const TALENT: &str = "skill.141404";
const BOND_ATTACK: &str = "trace.1414101";
const SYLVANITY: &str = "trace.1414102";
const DRAGON_BLESSING: &str = "trace.1414103";
const SOULDRAGON: &str = "souldragon";
const PHYSICAL: Element = Element::Physical;
const BP: u64 = 10_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct PermansorState {
    bondmate: Option<usize>,
    bond_attack_flat: u32,
    empowered_actions: u8,
    dragon_attacking: bool,
}

struct ShieldAmount {
    attack_ratio_bp: u16,
    flat_bp: u32,
}

impl ShieldAmount {
    fn from_frozen(id: &str, attack_ratio: f64, flat: f64) -> Result<Self, BattleError> {
        let flat_bp = (flat * BP as f64).round();
        if !(1.0..=100_000_000.0).contains(&flat_bp) {
            return Err(invalid(id));
        }
        Ok(Self {
            attack_ratio_bp: ratio_bp(id, attack_ratio)?,
            flat_bp: flat_bp as u32,
        })
    }

    fn scaled_bp(&self, attack: u32) -> u64 {
        u64::from(attack) * u64::from(self.attack_ratio_bp) + u64::from(self.flat_bp)
    }
}

struct Permansor {
    eidolon: u8,
    attack_trace: bool,
    sylvanity: bool,
    dragon_blessing: bool,
    skill_shield: ShieldAmount,
    ultimate_shield: ShieldAmount,
    dragon_shield: ShieldAmount,
    dragon_damage_bp: u16,
    bond_damage_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 135
        || input.combat_type.path != CombatPath::Preservation
        || input.combat_type.element != PHYSICAL
        || input.eidolon > 6
    {
        return Err(invalid("combat path, element, energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 8)?;
    let talent = input.parameters(TALENT, 6)?;
    if skill[2..] != [3.0, 3.0]
        || ultimate[2] != 2.0
        || ultimate[5..7] != [3.0, 3.0]
        || talent[2..] != [3.0, 3.0, 165.0, 1.0]
    {
        return Err(invalid("shield timing, summon speed or cleanse count"));
    }

    Ok(OfficialKit {
        max_energy_milli: 135_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![attack(BASIC, basic[0], 10)?],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: vec![],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![attack(ULTIMATE, ultimate[0], 20)?],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Permansor {
            eidolon: input.eidolon,
            attack_trace: input.enabled_trace_ids.iter().any(|id| id == BOND_ATTACK),
            sylvanity: input.enabled_trace_ids.iter().any(|id| id == SYLVANITY),
            dragon_blessing: input
                .enabled_trace_ids
                .iter()
                .any(|id| id == DRAGON_BLESSING),
            skill_shield: ShieldAmount::from_frozen(SKILL, skill[0], skill[1])?,
            ultimate_shield: ShieldAmount::from_frozen(ULTIMATE, ultimate[3], ultimate[4])?,
            dragon_shield: ShieldAmount::from_frozen(TALENT, talent[0], talent[1])?,
            dragon_damage_bp: ratio_bp(ULTIMATE, ultimate[1])?,
            bond_damage_bp: ratio_bp(ULTIMATE, ultimate[7])?,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * BP as f64).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn attack(id: &str, multiplier: f64, toughness: u16) -> Result<Effect, BattleError> {
    Ok(Effect::Hit {
        element: PHYSICAL,
        multiplier_bp: ratio_bp(id, multiplier)?,
        toughness,
    })
}

fn bondmate(battle: &Battle<'_>, owner: usize) -> Option<usize> {
    battle.units[owner]
        .permansor_state
        .bondmate
        .filter(|&target| battle.units[owner].hp > 0 && battle.units[target].hp > 0)
}

fn clear_bond(battle: &mut Battle<'_>, owner: usize) {
    if let Some(old) = battle.units[owner].permansor_state.bondmate.take() {
        battle.emit(
            EventKind::StatusExpired(StatusKind::Bondmate),
            Some(owner),
            Some(old),
        );
    }
    battle.units[owner].summon_turn = None;
    battle.units[owner].permansor_state.empowered_actions = 0;
}

impl Permansor {
    fn bind(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let attack = battle.effective_attack_excluding(owner, Some(owner));
        let state = &mut battle.units[owner].permansor_state;
        state.bond_attack_flat = (u64::from(attack) * 1_500 / BP) as u32;
        let previous = state.bondmate.replace(target);
        if let Some(old) = previous.filter(|&old| old != target) {
            battle.emit(
                EventKind::StatusExpired(StatusKind::Bondmate),
                Some(owner),
                Some(old),
            );
        }
        if previous != Some(target) {
            battle.emit(
                EventKind::StatusApplied(StatusKind::Bondmate),
                Some(owner),
                Some(target),
            );
        }
        battle.units[owner]
            .summon_turn
            .get_or_insert_with(|| SummonTurn::new(SOULDRAGON, 165_000_000));
    }

    fn shield(battle: &mut Battle<'_>, owner: usize, target: usize, addition: u32, cap: u32) {
        let previous = battle
            .official_shield(owner, target, Action::Skill)
            .map_or(0, |shield| shield.amount);
        battle.apply_official_shield(
            owner,
            target,
            previous.saturating_add(addition).min(cap),
            3,
            Some(Action::Skill),
        );
    }

    fn team_shields(&self, battle: &mut Battle<'_>, owner: usize, shield: &ShieldAmount) {
        let attack = battle.effective_attack(owner);
        let amount = (shield.scaled_bp(attack) / BP) as u32;
        let cap = (self.skill_shield.scaled_bp(attack) * 3 / BP) as u32;
        for target in battle.party_units() {
            Self::shield(battle, owner, target, amount, cap);
        }
    }

    fn dragon_protection(&self, battle: &mut Battle<'_>, owner: usize, multiplier: u64) {
        let attack = battle.effective_attack(owner);
        let amount = (self.dragon_shield.scaled_bp(attack) * multiplier / BP) as u32;
        let cap = (self.skill_shield.scaled_bp(attack) * 3 / BP) as u32;
        for target in battle.party_units() {
            if battle.is_targetable(target) {
                battle.cleanse_one(owner, target);
                Self::shield(battle, owner, target, amount, cap);
            }
        }
        if self.dragon_blessing
            && let Some(target) = battle
                .party_units()
                .filter(|&target| battle.is_targetable(target))
                .min_by_key(|&target| battle.shield_amount(target))
        {
            let extra = (u64::from(attack) * 500 + 100 * BP) * multiplier / BP;
            Self::shield(battle, owner, target, extra as u32, cap);
        }
    }

    fn empower_dragon(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].permansor_state.empowered_actions = 2 + 2 * u8::from(self.eidolon >= 2);
        if self.eidolon >= 2
            && let Some(dragon) = &mut battle.units[owner].summon_turn
        {
            dragon.advance(10_000);
        }
    }

    fn dragon_attack(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let Some(combat_type) = battle.plan.units[target].combat_type else {
            return;
        };
        let bond_multiplier = 1 + u16::from(self.eidolon >= 2);
        battle.begin_attack();
        battle.emit(EventKind::FollowUp, Some(owner), None);
        battle.units[owner].permansor_state.dragon_attacking = true;
        for enemy in battle.enemies() {
            battle.follow_up_hit(
                owner,
                enemy,
                Action::Skill,
                PHYSICAL,
                self.dragon_damage_bp,
                20,
            );
        }
        for enemy in battle.enemies() {
            battle.secondary_hit_with_credit(
                target,
                enemy,
                combat_type.element,
                self.bond_damage_bp.saturating_mul(bond_multiplier),
                Action::Skill,
                owner,
            );
        }
        if self.dragon_blessing
            && let Some(enemy) = battle
                .enemies()
                .filter(|&enemy| battle.is_targetable(enemy))
                .max_by_key(|&enemy| (battle.units[enemy].hp, Reverse(enemy)))
        {
            battle.secondary_hit_with_credit(
                target,
                enemy,
                combat_type.element,
                4_000 * bond_multiplier,
                Action::Skill,
                owner,
            );
        }
        battle.notify_attack(owner, Action::Skill, None);
        battle.units[owner].permansor_state.dragon_attacking = false;
        battle.resolve_follow_ups();
    }
}

impl Mechanics for Permansor {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.sylvanity {
            battle.advance(owner, 4_000);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Skill if let Some(target) = target => {
                self.bind(battle, owner, target);
                self.team_shields(battle, owner, &self.skill_shield);
            }
            Action::Ultimate => {
                self.team_shields(battle, owner, &self.ultimate_shield);
                self.empower_dragon(battle, owner);
                if self.eidolon >= 1 {
                    battle.restore_skill_points(owner, 1);
                    if let Some(target) = bondmate(battle, owner) {
                        battle.apply_ally_buff_with_id(
                            owner,
                            target,
                            AllyBuffKind::ResistancePenetration,
                            1_800,
                            3,
                            Some(ULTIMATE),
                        );
                    }
                }
                if self.eidolon >= 6
                    && let Some(target) = bondmate(battle, owner)
                    && battle.is_targetable(target)
                    && let Some(combat_type) = battle.plan.units[target].combat_type
                {
                    for enemy in battle.enemies() {
                        battle.secondary_hit_with_credit(
                            target,
                            enemy,
                            combat_type.element,
                            33_000,
                            Action::Ultimate,
                            owner,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn summon_holder(&self, battle: &Battle<'_>, owner: usize) -> usize {
        bondmate(battle, owner).unwrap_or(owner)
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(target) = bondmate(battle, owner) else {
            clear_bond(battle, owner);
            return;
        };
        let state = &mut battle.units[owner].permansor_state;
        let empowered = state.empowered_actions > 0;
        state.empowered_actions = state.empowered_actions.saturating_sub(1);
        self.dragon_protection(battle, owner, 1 + u64::from(empowered && self.eidolon >= 2));
        if empowered && battle.is_targetable(target) {
            self.dragon_attack(battle, owner, target);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) {
        if self.sylvanity
            && bondmate(battle, owner) == Some(actor)
            && battle.is_targetable(owner)
            && !battle.units[owner].permansor_state.dragon_attacking
        {
            battle.gain_energy(actor, owner, 6_000);
            if let Some(dragon) = &mut battle.units[owner].summon_turn {
                dragon.advance(1_500);
            }
        }
    }

    fn on_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize) {
        if bondmate(battle, owner).is_none() {
            clear_bond(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        clear_bond(battle, owner);
    }

    fn damage_comes_from_summon(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        kind == DamageKind::FollowUp && battle.units[actor].permansor_state.dragon_attacking
    }

    fn assisted_damage_comes_from_summon(&self, battle: &Battle<'_>, owner: usize) -> bool {
        battle.units[owner].permansor_state.dragon_attacking
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.attack_trace && bondmate(battle, owner) == Some(target) {
            battle.units[owner].permansor_state.bond_attack_flat
        } else {
            0
        }
    }

    fn ally_damage_reduction_bp_for_target(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
    ) -> u32 {
        2_000 * u32::from(self.eidolon >= 4 && bondmate(battle, owner) == Some(target))
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        if self.eidolon < 6 || bondmate(battle, owner).is_none() {
            return DamageTargetAura::default();
        }
        DamageTargetAura {
            vulnerability_bp: 2_000,
            defense_down_bp: 1_200 * u32::from(bondmate(battle, owner) == Some(actor)),
            ..DamageTargetAura::default()
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if action != Action::Skill || bondmate(battle, owner).is_some() {
            return None;
        }
        let preferred = battle
            .allies()
            .filter(|&ally| battle.is_targetable(ally))
            .max_by_key(|&ally| {
                (
                    ally != owner,
                    battle.plan.units[ally].stats.attack,
                    Reverse(ally),
                )
            });
        (target == preferred).then_some(2)
    }
}
