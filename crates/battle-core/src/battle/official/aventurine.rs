use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1304";
const BASIC: &str = "skill.130401";
const SKILL: &str = "skill.130402";
const ULTIMATE: &str = "skill.130403";
const TALENT: &str = "skill.130404";
const A2: &str = "trace.1304101";
const A4: &str = "trace.1304102";
const A6: &str = "trace.1304103";
const E4: &str = "eidolon.130404";

const BLIND_BET_COST: u8 = 7;
const BLIND_BET_CAP: u8 = 10;
const BASE_FOLLOW_UP_HITS: u8 = 7;
const A6_TRIGGER_CAP: u8 = 3;

#[derive(Clone, Default)]
pub(in crate::battle) struct AventurineState {
    blind_bets: u8,
    follow_up_origin: Option<Action>,
    shielded_attack_target: bool,
    a6_triggers: u8,
    a6_trigger_turn: u32,
    control_guard_ready_on_turn: u32,
}

struct Aventurine {
    shield_defense_bp: u16,
    shield_flat: u32,
    shield_turns: u32,
    unnerved_crit_damage_bp: u32,
    unnerved_turns: u32,
    follow_up_multiplier_bp: u16,
    effect_resistance_bp: u32,
    control_guard_cooldown: u32,
    eidolon: u8,
    leverage: bool,
    hot_hand: bool,
    bingo: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Preservation
        || input.combat_type.element != Element::Imaginary
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let [basic] = input.parameters(BASIC, 1)? else {
        return Err(invalid(BASIC));
    };
    let [shield_defense, shield_flat, shield_turns, shield_cap] = input.parameters(SKILL, 4)?
    else {
        return Err(invalid(SKILL));
    };
    let [ult_max_bets, ultimate, unnerved, unnerved_turns] = input.parameters(ULTIMATE, 4)? else {
        return Err(invalid(ULTIMATE));
    };
    let [
        extra_self_bet,
        hits,
        follow_up,
        effect_resistance,
        control_cooldown,
    ] = input.parameters(TALENT, 5)?
    else {
        return Err(invalid(TALENT));
    };
    if *shield_cap != 2.0 || *shield_turns != 3.0 {
        return Err(invalid(SKILL));
    }
    if *ult_max_bets != f64::from(BLIND_BET_COST) || *unnerved_turns != 3.0 {
        return Err(invalid(ULTIMATE));
    }
    if *extra_self_bet != 1.0
        || *hits != f64::from(BASE_FOLLOW_UP_HITS)
        || *control_cooldown != 2.0
        || *effect_resistance > 1.0
    {
        return Err(invalid(TALENT));
    }
    if !(1.0..=f64::from(u32::MAX)).contains(shield_flat) || shield_flat.fract() != 0.0 {
        return Err(invalid(SKILL));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 110_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![imaginary_hit(ratio_bp(BASIC, *basic)?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 5_000,
            effects: vec![imaginary_hit(ratio_bp(ULTIMATE, *ultimate)?, 30)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Aventurine {
            shield_defense_bp: ratio_bp(SKILL, *shield_defense)?,
            shield_flat: *shield_flat as u32,
            shield_turns: *shield_turns as u32,
            unnerved_crit_damage_bp: u32::from(ratio_bp(ULTIMATE, *unnerved)?),
            unnerved_turns: *unnerved_turns as u32,
            follow_up_multiplier_bp: ratio_bp(TALENT, *follow_up)?,
            effect_resistance_bp: u32::from(ratio_bp(TALENT, *effect_resistance)?),
            control_guard_cooldown: *control_cooldown as u32,
            eidolon: input.eidolon,
            leverage: trace(A2),
            hot_hand: trace(A4),
            bingo: trace(A6),
        }),
    })
}

fn invalid(parameter: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {parameter}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn imaginary_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Imaginary,
        multiplier_bp,
        toughness,
    }
}

fn shield_value(defense: u32, ratio_bp: u16, flat: u32) -> u32 {
    let amount = u64::from(defense) * u64::from(ratio_bp) / 10_000 + u64::from(flat);
    u32::try_from(amount).unwrap_or(u32::MAX)
}

fn has_fortified_wager(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
    battle.has_official_shield(owner, target, Action::Skill)
}

impl Aventurine {
    fn grant_fortified_wager(&self, battle: &mut Battle<'_>, actor: usize, from_follow_up: bool) {
        let defense = battle.defense(actor);
        let skill_amount = shield_value(defense, self.shield_defense_bp, self.shield_flat);
        let cap = skill_amount.saturating_mul(2);
        let active = battle.allies().filter(|&ally| battle.is_targetable(ally));
        let weakest = from_follow_up
            .then(|| {
                active
                    .clone()
                    .min_by_key(|&ally| battle.shield_amount(ally))
            })
            .flatten();
        let added = if from_follow_up {
            shield_value(defense, 720, 96)
        } else {
            skill_amount
        };
        for ally in battle.allies() {
            let current = battle
                .official_shield(actor, ally, Action::Skill)
                .map_or(0, |shield| shield.amount);
            let grant = if weakest == Some(ally) {
                added.saturating_mul(2)
            } else {
                added
            };
            battle.apply_official_shield(
                actor,
                ally,
                current.saturating_add(grant).min(cap),
                self.shield_turns,
                Some(Action::Skill),
            );
        }
    }

    fn change_blind_bets(
        battle: &mut Battle<'_>,
        source: usize,
        actor: usize,
        amount: i8,
        origin: Action,
    ) {
        let state = &mut battle.units[actor].aventurine_state;
        let before = state.blind_bets;
        let after = before.saturating_add_signed(amount).min(BLIND_BET_CAP);
        if before == after {
            return;
        }
        state.blind_bets = after;
        if before < BLIND_BET_COST && after >= BLIND_BET_COST {
            state.follow_up_origin = Some(origin);
        }
        battle.emit(
            EventKind::BlindBet { before, after },
            Some(source),
            Some(actor),
        );
    }

    fn queue_ready_follow_up(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        if !battle.is_targetable(actor)
            || battle.is_crowd_controlled(actor)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return false;
        }
        let Some(origin) = battle.units[actor].aventurine_state.follow_up_origin.take() else {
            return false;
        };
        Self::change_blind_bets(battle, actor, actor, -(BLIND_BET_COST as i8), origin);
        battle.follow_up_random(
            actor,
            origin,
            BASE_FOLLOW_UP_HITS + if self.eidolon >= 4 { 3 } else { 0 },
            FollowUpAttack {
                element: Element::Imaginary,
                multiplier_bp: self.follow_up_multiplier_bp,
                toughness: 3,
                energy_milli: 1_000,
                first_hit: None,
            },
        );
        true
    }
}

impl Mechanics for Aventurine {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.hot_hand {
            self.grant_fortified_wager(battle, actor, false);
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor && self.queue_ready_follow_up(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn on_ally_action_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize, _: Action) {
        if self.queue_ready_follow_up(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Ultimate, Some(target)) => {
                let gained = 1 + battle.random_index(usize::from(BLIND_BET_COST)) as i8;
                Self::change_blind_bets(battle, actor, actor, gained, action);
                battle.apply_debuff(
                    actor,
                    target,
                    DebuffKind::Unnerved,
                    self.unnerved_crit_damage_bp,
                    self.unnerved_turns,
                );
            }
            (Action::Basic, Some(target)) if self.eidolon >= 2 => {
                battle.apply_debuff(actor, target, DebuffKind::AllResistanceDown, 1_200, 3);
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::AssistSkill | Action::ElationSkill => {}
            Action::Skill | Action::EnhancedSkill => {
                self.grant_fortified_wager(battle, actor, false)
            }
            Action::Ultimate => {
                if self.eidolon >= 1 {
                    self.grant_fortified_wager(battle, actor, false);
                }
                self.queue_ready_follow_up(battle, actor);
            }
            Action::Basic | Action::EnhancedUltimate => {}
        }
    }

    fn before_enemy_attack_seen(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let shielded = has_fortified_wager(battle, owner, target);
        battle.units[owner].aventurine_state.shielded_attack_target = shielded;
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
        _: bool,
    ) {
        if std::mem::take(&mut battle.units[owner].aventurine_state.shielded_attack_target) {
            let gained = if owner == target { 2 } else { 1 };
            Self::change_blind_bets(battle, attacker, owner, gained, Action::Basic);
            self.queue_ready_follow_up(battle, owner);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        let kind = if battle.in_follow_up() {
            DamageKind::FollowUp
        } else {
            DamageKind::Direct
        };
        if self.bingo
            && source != owner
            && battle.is_follow_up_damage(source, Some(origin), kind)
            && has_fortified_wager(battle, owner, source)
        {
            let unit = &mut battle.units[owner];
            let state = &mut unit.aventurine_state;
            if state.a6_trigger_turn != unit.normal_turns {
                state.a6_trigger_turn = unit.normal_turns;
                state.a6_triggers = 0;
            }
            if state.a6_triggers < A6_TRIGGER_CAP {
                state.a6_triggers += 1;
                Self::change_blind_bets(battle, source, owner, 1, origin);
            }
        }
        self.queue_ready_follow_up(battle, owner);
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 4 {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::DefensePercent,
                4_000,
                2,
                Some(E4),
            );
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _: Action) {
        if self.bingo {
            self.grant_fortified_wager(battle, actor, true);
        }
    }

    fn effect_resistance_aura_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        self.effect_resistance_bp * u32::from(has_fortified_wager(battle, owner, target))
    }

    fn block_enemy_control(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        target: usize,
    ) -> bool {
        if owner != target || !has_fortified_wager(battle, owner, owner) {
            return false;
        }
        let unit = &mut battle.units[owner];
        if unit.normal_turns < unit.aventurine_state.control_guard_ready_on_turn {
            return false;
        }
        unit.aventurine_state.control_guard_ready_on_turn = unit
            .normal_turns
            .saturating_add(self.control_guard_cooldown);
        true
    }

    fn scales_with_defense(&self) -> bool {
        true
    }

    fn crit_rate_from_defense_bp(&self, defense: u32) -> u32 {
        if !self.leverage {
            return 0;
        }
        (defense.saturating_sub(1_600) / 100 * 200).min(4_800)
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 1 && has_fortified_wager(battle, owner, target))
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.eidolon < 6 || !matches!(kind, DamageKind::Direct | DamageKind::FollowUp) {
            return 0;
        }
        let shielded_teammates = battle
            .allies()
            .filter(|&ally| {
                ally != actor && battle.is_targetable(ally) && battle.shield_amount(ally) > 0
            })
            .count();
        5_000 * shielded_teammates.min(3) as u32
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        let has_offensive_shield_effect = self.bingo || self.eidolon >= 1;
        (action == Action::Skill
            && has_offensive_shield_effect
            && battle.allies().any(|ally| {
                battle.is_targetable(ally)
                    && battle
                        .official_shield(actor, ally, Action::Skill)
                        .is_none_or(|shield| {
                            shield.expires_after_turn <= battle.units[ally].normal_turns
                        })
            }))
        .then_some(2)
    }
}
