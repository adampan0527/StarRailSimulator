use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1404";
const IMAGINARY: Element = Element::Imaginary;
const ENERGY: u32 = 160;
const BASIC: &str = "skill.140401";
const SKILL: &str = "skill.140402";
const ULTIMATE: &str = "skill.140403";
const TALENT: &str = "skill.140404";
const KINGSLAYER: &str = "skill.140409";
const GODSLAYER: &str = "skill.140411";

const CHARGE_PER_FULL_HP: u32 = 100_000;
const CHARGE_CAP: u32 = 200_000;
const E2_CHARGE_CAP: u32 = 40_000;
const E2_HEAL_CONVERSION_BP: u32 = 4_000;
const E4_HEAL_BP: u32 = 1_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct MydeiState {
    vendetta: bool,
    charge_milli: u32,
    chiton_steps: u32,
    preserved_vendettas: u8,
    healing_charge_milli: u32,
    godslayer_queued: bool,
    godslayer_running: bool,
    cyrene_godslayer_crit_bp: u32,
    kingslayer_target: Option<usize>,
    godslayer_target: Option<usize>,
    marked_target: Option<usize>,
}

struct Mydei {
    eidolon: u8,
    water_and_earth: bool,
    thirty_tyrants: bool,
    bloodied_chiton: bool,
    skill_cost_bp: u32,
    kingslayer_cost_bp: u32,
    kingslayer: OfficialAbility,
    godslayer: OfficialAbility,
    godslayer_main_bp: u16,
    godslayer_charge_milli: u32,
    vendetta_hp_bp: u32,
    entry_heal_bp: u32,
    rescue_heal_bp: u32,
    ultimate_heal_bp: u32,
    ultimate_charge_milli: u32,
    taunt_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != IMAGINARY
    {
        return Err(invalid("energy, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 5)?;
    let kingslayer = input.parameters(KINGSLAYER, 3)?;
    let godslayer = input.parameters(GODSLAYER, 3)?;
    if skill[2] != 0.5
        || kingslayer[2] != 0.35
        || ultimate[3] != 2.0
        || ultimate[4] != 20.0
        || talent[2] != 150.0
        || godslayer[2] != talent[2]
        || talent[3] != 0.5
        || talent[4] != 0.5
    {
        return Err(invalid("fixed skill parameters"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let godslayer_main_bp = ratio_bp(GODSLAYER, godslayer[0])?
        .checked_add(if input.eidolon >= 1 { 3_000 } else { 0 })
        .ok_or_else(|| invalid(GODSLAYER))?;
    let godslayer_splash_bp = ratio_bp(GODSLAYER, godslayer[1])?;
    let godslayer_effects = if input.eidolon >= 1 {
        vec![Effect::DirectedAreaAttack]
    } else {
        blast(godslayer_main_bp, godslayer_splash_bp, (30, 20))
    };

    Ok(OfficialKit {
        max_energy_milli: ENERGY * 1_000,
        basic: attack(
            vec![hit(ratio_bp(BASIC, basic[0])?, 10)],
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            blast(
                ratio_bp(SKILL, skill[0])?,
                ratio_bp(SKILL, skill[1])?,
                (20, 10),
            ),
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(
            blast(
                ratio_bp(ULTIMATE, ultimate[0])?,
                ratio_bp(ULTIMATE, ultimate[1])?,
                (20, 20),
            ),
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Mydei {
            eidolon: input.eidolon,
            water_and_earth: trace("trace.1404101"),
            thirty_tyrants: trace("trace.1404102"),
            bloodied_chiton: trace("trace.1404103"),
            skill_cost_bp: u32::from(ratio_bp(SKILL, skill[2])?),
            kingslayer_cost_bp: u32::from(ratio_bp(KINGSLAYER, kingslayer[2])?),
            kingslayer: attack(
                blast(
                    ratio_bp(KINGSLAYER, kingslayer[0])?,
                    ratio_bp(KINGSLAYER, kingslayer[1])?,
                    (20, 10),
                ),
                30_000,
                SkillPoints::Unchanged,
            ),
            godslayer: attack(godslayer_effects, 10_000, SkillPoints::Unchanged),
            godslayer_main_bp,
            godslayer_charge_milli: if input.eidolon == 6 { 100_000 } else { 150_000 },
            vendetta_hp_bp: u32::from(ratio_bp(TALENT, talent[4])?),
            entry_heal_bp: u32::from(ratio_bp(TALENT, talent[0])?),
            rescue_heal_bp: u32::from(ratio_bp(TALENT, talent[3])?),
            ultimate_heal_bp: u32::from(ratio_bp(ULTIMATE, ultimate[2])?),
            ultimate_charge_milli: (ultimate[4] as u32) * 1_000,
            taunt_turns: ultimate[3] as u32,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::MaxHpHit {
        element: IMAGINARY,
        multiplier_bp,
        toughness,
    }
}

fn blast(main_bp: u16, splash_bp: u16, toughness: (u16, u16)) -> Vec<Effect> {
    vec![
        hit(main_bp, toughness.0),
        Effect::MaxHpBlastAdjacent {
            element: IMAGINARY,
            multiplier_bp: splash_bp,
            toughness: toughness.1,
        },
    ]
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn charge_milli(amount: u32, max_hp: u32, ratio_bp: u32) -> u32 {
    let scaled = u64::from(amount) * u64::from(ratio_bp) * u64::from(CHARGE_PER_FULL_HP)
        / (u64::from(max_hp) * 10_000);
    scaled.min(u64::from(CHARGE_CAP)) as u32
}

impl Mydei {
    fn set_charge(battle: &mut Battle<'_>, actor: usize, after_milli: u32) {
        let before_milli = std::mem::replace(
            &mut battle.units[actor].mydei_state.charge_milli,
            after_milli,
        );
        if before_milli != after_milli {
            battle.emit(
                EventKind::MydeiCharge {
                    before_milli,
                    after_milli,
                },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn gain_charge(battle: &mut Battle<'_>, actor: usize, amount_milli: u32) {
        let state = &battle.units[actor].mydei_state;
        if !state.godslayer_running {
            let after = state
                .charge_milli
                .saturating_add(amount_milli)
                .min(CHARGE_CAP);
            Self::set_charge(battle, actor, after);
        }
    }

    fn set_vendetta(battle: &mut Battle<'_>, actor: usize, active: bool) {
        let before = battle.effective_max_hp(actor);
        battle.units[actor].mydei_state.vendetta = active;
        let after = battle.effective_max_hp(actor);
        let unit = &mut battle.units[actor];
        unit.hp = unit
            .hp
            .saturating_add(after.saturating_sub(before))
            .min(after);
        let event = if active {
            EventKind::StatusApplied(StatusKind::Vendetta)
        } else {
            EventKind::StatusExpired(StatusKind::Vendetta)
        };
        battle.emit(event, Some(actor), Some(actor));
    }

    fn heal(battle: &mut Battle<'_>, actor: usize, ratio_bp: u32) {
        let max_hp = battle.effective_max_hp(actor);
        battle.heal_unit(actor, actor, percentage_of(max_hp, ratio_bp));
    }

    fn settle_charge(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &battle.units[actor].mydei_state;
        if !state.vendetta && state.charge_milli >= CHARGE_PER_FULL_HP {
            Self::set_charge(battle, actor, state.charge_milli - CHARGE_PER_FULL_HP);
            Self::set_vendetta(battle, actor, true);
            Self::heal(battle, actor, self.entry_heal_bp);
            battle.advance_immediate(actor);
        }
        let state = &battle.units[actor].mydei_state;
        if state.vendetta
            && state.charge_milli >= self.godslayer_charge_milli
            && !state.godslayer_running
            && !state.godslayer_queued
            && !battle.is_crowd_controlled(actor)
        {
            battle.units[actor].mydei_state.godslayer_queued = true;
            battle.queue_extra_turn(actor);
        }
    }

    fn godslayer_ready(battle: &Battle<'_>, actor: usize) -> bool {
        battle.in_extra_turn(actor) && battle.units[actor].mydei_state.godslayer_queued
    }

    fn forced_target(battle: &Battle<'_>, actor: usize) -> Option<usize> {
        let state = &battle.units[actor].mydei_state;
        let alive =
            |&target: &usize| battle.enemies().contains(&target) && battle.is_targetable(target);
        if Self::godslayer_ready(battle, actor) {
            return state
                .marked_target
                .filter(alive)
                .or(state.godslayer_target.filter(alive));
        }
        state.kingslayer_target.filter(alive)
    }
}

impl Mechanics for Mydei {
    fn cyrene_ode_applied(
        &self,
        battle: &mut Battle<'_>,
        _: usize,
        actor: usize,
        parameters: [u32; 4],
    ) {
        if !battle.units[actor].mydei_state.vendetta {
            battle.advance_ally_immediate(actor, actor);
            return;
        }
        let target = battle.units[actor]
            .mydei_state
            .marked_target
            .filter(|&target| battle.is_targetable(target))
            .or_else(|| battle.random_living_enemy());
        if let Some(target) = target {
            battle.units[actor].mydei_state.cyrene_godslayer_crit_bp = parameters[0];
            battle.emit(
                EventKind::MydeiEnhancedSkill { godslayer: true },
                Some(actor),
                Some(target),
            );
            battle.perform_triggered_skill(actor, target, self.godslayer.resolve().0);
            battle.units[actor].mydei_state.cyrene_godslayer_crit_bp = 0;
        }
    }
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.bloodied_chiton {
            battle.units[actor].mydei_state.chiton_steps = battle
                .effective_max_hp(actor)
                .saturating_sub(4_000)
                .min(4_000)
                / 100;
        }
        if self.eidolon == 6 {
            Self::set_vendetta(battle, actor, true);
        }
    }

    fn can_use_basic(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !battle.units[actor].mydei_state.vendetta
    }

    fn forced_action_target(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<usize> {
        (action == Action::Skill && battle.units[actor].mydei_state.vendetta)
            .then(|| Self::forced_target(battle, actor))
            .flatten()
    }

    fn prepare_automatic_action(&self, battle: &mut Battle<'_>, actor: usize) -> Option<Action> {
        if !battle.units[actor].mydei_state.vendetta {
            return None;
        }
        if Self::forced_target(battle, actor).is_some() {
            return Some(Action::Skill);
        }
        let target = battle.random_living_enemy();
        let godslayer = Self::godslayer_ready(battle, actor);
        let state = &mut battle.units[actor].mydei_state;
        if godslayer {
            state.godslayer_target = target;
        } else {
            state.kingslayer_target = target;
        }
        target.map(|_| Action::Skill)
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if action != Action::Skill || !battle.units[actor].mydei_state.vendetta {
            return None;
        }
        Some(if Self::godslayer_ready(battle, actor) {
            &self.godslayer
        } else {
            &self.kingslayer
        })
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let godslayer = Self::godslayer_ready(battle, actor);
                let vendetta = battle.units[actor].mydei_state.vendetta;
                if vendetta {
                    battle.emit(
                        EventKind::MydeiEnhancedSkill { godslayer },
                        Some(actor),
                        target,
                    );
                }
                if godslayer {
                    let state = &mut battle.units[actor].mydei_state;
                    state.godslayer_running = true;
                    state.godslayer_queued = false;
                    state.marked_target = None;
                    state.godslayer_target = None;
                    let remaining = state.charge_milli - self.godslayer_charge_milli;
                    Self::set_charge(battle, actor, remaining);
                } else {
                    battle.units[actor].mydei_state.kingslayer_target = None;
                    let hp = battle.units[actor].hp;
                    let cost = if vendetta {
                        self.kingslayer_cost_bp
                    } else {
                        self.skill_cost_bp
                    };
                    battle.spend_hp(actor, percentage_of(hp, cost), 1);
                }
            }
            Action::Ultimate => {
                Self::heal(battle, actor, self.ultimate_heal_bp);
                Self::gain_charge(battle, actor, self.ultimate_charge_milli);
                battle.units[actor].mydei_state.marked_target = target;
            }
            _ => {}
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        for enemy in
            std::iter::once(target).chain(battle.enemies().filter(|&enemy| enemy != target))
        {
            if battle.is_targetable(enemy) {
                battle.attack_hit_with_max_hp(
                    actor,
                    enemy,
                    IMAGINARY,
                    (0, self.godslayer_main_bp),
                    0,
                    if enemy == target { 30 } else { 20 },
                );
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
        if action == Action::Ultimate
            && let Some(target) = target
        {
            for enemy in std::iter::once(target).chain(battle.adjacent_enemies(target)) {
                battle.apply_debuff(actor, enemy, DebuffKind::Taunt, 0, self.taunt_turns);
            }
        }
    }

    fn taunts_enemy(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> bool {
        battle.units[enemy]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::Taunt && status.source == owner)
    }

    fn after_hp_spent(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        let max_hp = battle.effective_max_hp(actor);
        Self::gain_charge(battle, actor, charge_milli(amount, max_hp, 10_000));
    }

    fn after_hp_loss(&self, battle: &mut Battle<'_>, actor: usize, source: usize, amount: u32) {
        let max_hp = battle.effective_max_hp(actor);
        let chiton = battle.units[actor].mydei_state.chiton_steps;
        let bonus = if battle.enemies().contains(&source) {
            chiton * 250
        } else {
            0
        };
        Self::gain_charge(battle, actor, charge_milli(amount, max_hp, 10_000 + bonus));
    }

    fn after_healing(&self, battle: &mut Battle<'_>, target: usize, restored: u32) {
        let state = &battle.units[target].mydei_state;
        if self.eidolon < 2 || !state.vendetta || state.godslayer_running {
            return;
        }
        let max_hp = battle.effective_max_hp(target);
        let amount = charge_milli(restored, max_hp, E2_HEAL_CONVERSION_BP)
            .min(E2_CHARGE_CAP - state.healing_charge_milli);
        battle.units[target].mydei_state.healing_charge_milli += amount;
        Self::gain_charge(battle, target, amount);
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            battle.units[owner].mydei_state.godslayer_running = false;
        }
        self.settle_charge(battle, owner);
        battle.units[owner].mydei_state.healing_charge_milli = 0;
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        self.settle_charge(battle, owner);
    }

    fn after_enemy_attack(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 4 && battle.units[actor].mydei_state.vendetta {
            Self::heal(battle, actor, E4_HEAL_BP);
        }
    }

    fn prevent_defeat(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        let state = &mut battle.units[actor].mydei_state;
        if !state.vendetta {
            return false;
        }
        if self.water_and_earth && state.preserved_vendettas < 3 {
            state.preserved_vendettas += 1;
        } else {
            state.godslayer_queued = false;
            state.kingslayer_target = None;
            state.godslayer_target = None;
            battle.cancel_queued_extra_turn(actor);
            Self::set_charge(battle, actor, 0);
            Self::set_vendetta(battle, actor, false);
        }
        Self::heal(battle, actor, self.rescue_heal_bp);
        true
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::set_charge(battle, actor, 0);
        battle.units[actor].mydei_state.marked_target = None;
    }

    fn max_hp_multiplier_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        10_000 + self.vendetta_hp_bp * u32::from(battle.units[actor].mydei_state.vendetta)
    }

    fn defense_is_zero(&self, battle: &Battle<'_>, actor: usize) -> bool {
        battle.units[actor].mydei_state.vendetta
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, actor: usize) -> bool {
        self.thirty_tyrants && battle.units[actor].mydei_state.vendetta
    }

    fn incoming_healing_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        battle.units[actor].mydei_state.chiton_steps * 75
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        battle.units[actor].mydei_state.chiton_steps * 120
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].mydei_state;
        u32::from(self.eidolon >= 4 && state.vendetta) * 3_000 + state.cyrene_godslayer_crit_bp
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _: usize) -> u32 {
        u32::from(self.eidolon >= 2 && battle.units[actor].mydei_state.vendetta) * 1_500
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Skill).then_some(1)
    }
}
