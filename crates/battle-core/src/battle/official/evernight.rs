use std::sync::Arc;

use crate::{
    Action, BattleError, EventKind, StatusKind,
    fixture::{Ability, Effect, Element, Stats, Target},
};

use super::{
    super::{Battle, SummonTurn, resolve::DamageTargetAura, support::percentage_of},
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1413";
const BASIC: &str = "skill.141301";
const SKILL: &str = "skill.141302";
const TIME_SKILL: &str = "skill.141309";
const ULTIMATE: &str = "skill.141303";
const TALENT: &str = "skill.141304";
const EVEY_SKILL: &str = "skill.1141301";
const EVEY_TALENT: &str = "skill.1141303";
const EVEY_DEPARTURE: &str = "skill.1141306";
const EVEY_BURST: &str = "skill.1141307";
const DARK_THE_NIGHT: &str = "trace.1413101";
const ROUSE_THE_FLAME: &str = "trace.1413102";
const KINDLE_THE_MORN: &str = "trace.1413103";
const EVEY: &str = "evey";
const EVEY_SPEED: u64 = 160_000_000;
const MEMORIA_THRESHOLD: u32 = 16;
const EVEY_HP_BP: u32 = 5_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct EvernightState {
    memoria: u32,
    darkest_riddle: Option<u32>,
    burst_advance_used: bool,
    skill_until: u32,
    skill_crit_bp: u32,
    talent_crit_until: u32,
    trace_crit_until: u32,
    departure_speed_bp: u32,
    burst_consumed: u32,
    last_target: Option<usize>,
    received_attacks: [Option<u32>; 2],
}

struct Evernight {
    eidolon: u8,
    dark_the_night: bool,
    rouse_the_flame: bool,
    kindle_the_morn: bool,
    skill_crit_ratio_bp: u16,
    ult_damage_bp: u16,
    riddle_damage_bp: u16,
    riddle_vulnerability_bp: u16,
    talent_crit_bp: u16,
    evey_field_damage_bp: u16,
    evey_damage_bp: u16,
    evey_bonus_per_four_bp: u16,
    burst_primary_bp: u16,
    burst_secondary_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != Element::Ice
        || input.energy_capacity != 240
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, combat type or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 6)?;
    let time_skill = input
        .skill_parameters
        .get(TIME_SKILL)
        .map_or(Ok(skill), |_| input.parameters(TIME_SKILL, 6))?;
    let ult = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 7)?;
    let evey = input.parameters(EVEY_SKILL, 4)?;
    let evey_talent = input.parameters(EVEY_TALENT, 2)?;
    let departure = input.parameters(EVEY_DEPARTURE, 3)?;
    let burst = input.parameters(EVEY_BURST, 3)?;
    if skill[1..] != [2.0, 2.0, 0.5, 12.0, 0.1]
        || time_skill[1..] != skill[1..]
        || ult[1] != 2.0
        || talent[0] != 2.0
        || talent[2..6] != [2.0, 160.0, 0.5, f64::from(MEMORIA_THRESHOLD)]
        || evey[2..] != [4.0, 1.0]
        || evey_talent[1] != 3.0
        || departure[..3] != [0.1, 0.01, 40.0]
        || burst[2] != f64::from(MEMORIA_THRESHOLD)
    {
        return Err(invalid("unexpected frozen skill conditions"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|entry| entry == id);
    Ok(OfficialKit {
        max_energy_milli: 240_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Ice,
                multiplier_bp: ratio_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: vec![Effect::DirectedAreaAttack],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Evernight {
            eidolon: input.eidolon,
            dark_the_night: trace(DARK_THE_NIGHT),
            rouse_the_flame: trace(ROUSE_THE_FLAME),
            kindle_the_morn: trace(KINDLE_THE_MORN),
            skill_crit_ratio_bp: ratio_bp(SKILL, skill[0])?,
            ult_damage_bp: ratio_bp(ULTIMATE, ult[0])?,
            riddle_damage_bp: ratio_bp(ULTIMATE, ult[2])?,
            riddle_vulnerability_bp: ratio_bp(ULTIMATE, ult[3])?,
            talent_crit_bp: ratio_bp(TALENT, talent[1])?,
            evey_field_damage_bp: ratio_bp(EVEY_TALENT, evey_talent[0])?,
            evey_damage_bp: ratio_bp(EVEY_SKILL, evey[0])?,
            evey_bonus_per_four_bp: ratio_bp(EVEY_SKILL, evey[1])?,
            burst_primary_bp: ratio_bp(EVEY_BURST, burst[0])?,
            burst_secondary_bp: ratio_bp(EVEY_BURST, burst[1])?,
        }),
    })
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id}"))
}

fn ratio_bp(id: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn pair_member(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
    target == owner || battle.memosprite_owner(target) == Some(owner)
}

impl Evernight {
    fn set_memoria(battle: &mut Battle<'_>, owner: usize, after: u32) {
        let before = std::mem::replace(&mut battle.units[owner].evernight_state.memoria, after);
        if before != after {
            battle.emit(EventKind::Memoria { before, after }, Some(owner), None);
        }
    }

    fn gain_memoria(&self, battle: &mut Battle<'_>, owner: usize, amount: u32) {
        if amount == 0 {
            return;
        }
        let before = battle.units[owner].evernight_state.memoria;
        let after = before
            .saturating_add(amount)
            .saturating_add(2 * u32::from(self.eidolon >= 2));
        Self::set_memoria(battle, owner, after);
        if after >= MEMORIA_THRESHOLD && battle.is_targetable(owner) {
            battle.cleanse_crowd_control(owner, owner);
            if !battle.units[owner].evernight_state.burst_advance_used
                && let Some(sprite) = battle.active_memosprite(owner)
            {
                battle.units[owner].evernight_state.burst_advance_used = true;
                battle.advance_ally(owner, sprite, 10_000);
            }
        }
    }

    fn summon(battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        if let Some(sprite) = battle.active_memosprite(owner) {
            battle.cleanse_crowd_control(owner, sprite);
            return Some(sprite);
        }
        let sprite = battle.memosprite(owner)?;
        let hp = percentage_of(battle.effective_max_hp(owner), EVEY_HP_BP).max(1);
        battle.units[sprite].memosprite_max_hp = Some(hp);
        battle.units[sprite].memosprite_generation += 1;
        battle.units[sprite].hp = battle.effective_max_hp(sprite);
        battle.units[owner].summon_turn = Some(SummonTurn::new(EVEY, EVEY_SPEED));
        battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
        battle.advance_ally(owner, sprite, 10_000);
        Some(sprite)
    }

    fn target(battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        battle.units[owner]
            .evernight_state
            .last_target
            .filter(|&target| battle.enemies().contains(&target) && battle.is_targetable(target))
            .or_else(|| battle.random_living_enemy())
    }

    fn cast_skill(&self, battle: &mut Battle<'_>, owner: usize) {
        let before_summon = battle.active_memosprite(owner);
        let cost = percentage_of(battle.units[owner].hp, 1_000);
        battle.spend_hp(owner, cost, 1);
        Self::summon(battle, owner);
        if let Some(sprite) = before_summon {
            let healing = percentage_of(battle.effective_max_hp(sprite), 5_000);
            battle.heal_unit(owner, sprite, healing);
        }
        let turn = battle.units[owner].normal_turns;
        let ode = super::cyrene::time_ode(battle, owner);
        let ratio = ode.map_or(u32::from(self.skill_crit_ratio_bp), |ode| {
            u32::from(self.skill_crit_ratio_bp) + ode.crit_ratio_bp
        });
        let crit = percentage_of(battle.effective_crit_damage_bp(owner), ratio);
        battle.units[owner].evernight_state.skill_until = turn + 2;
        battle.units[owner].evernight_state.skill_crit_bp = crit;
        self.gain_memoria(battle, owner, 2 + ode.map_or(0, |ode| ode.memoria));
        if battle.units[owner].evernight_state.darkest_riddle.is_some() {
            self.gain_memoria(battle, owner, 12);
        }
    }

    fn enter_riddle(&self, battle: &mut Battle<'_>, owner: usize) {
        let charge = &mut battle.units[owner].evernight_state.darkest_riddle;
        let already_active = charge.is_some();
        *charge = Some(
            charge
                .unwrap_or_default()
                .saturating_add(2 + 2 * u32::from(self.eidolon >= 2)),
        );
        if !already_active {
            battle.emit(
                EventKind::StatusApplied(StatusKind::DarkestRiddle),
                Some(owner),
                Some(owner),
            );
        }
        battle.cleanse_crowd_control(owner, owner);
    }

    fn evey_turn(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize, target: usize) {
        let memoria = battle.units[owner].evernight_state.memoria;
        let burst = memoria >= MEMORIA_THRESHOLD && battle.is_targetable(owner);
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), Some(target));
        if burst {
            for enemy in
                std::iter::once(target).chain(battle.enemies().filter(|&enemy| enemy != target))
            {
                let (per_point, toughness) = if enemy == target {
                    (self.burst_primary_bp, 30)
                } else {
                    (self.burst_secondary_bp, 20)
                };
                battle.memosprite_hp_hit(
                    sprite,
                    enemy,
                    Element::Ice,
                    memoria.saturating_mul(u32::from(per_point)),
                    toughness,
                    Action::Skill,
                );
            }
        } else {
            let multiplier = u32::from(self.evey_damage_bp)
                + memoria / 4 * u32::from(self.evey_bonus_per_four_bp);
            battle.memosprite_hp_hit(sprite, target, Element::Ice, multiplier, 10, Action::Skill);
            self.gain_memoria(battle, owner, 1);
        }
        battle.notify_attack(sprite, Action::Skill, Some(target));
        battle.gain_energy(sprite, owner, if burst { 10_000 } else { 20_000 });
        if burst {
            self.finish_burst(battle, owner, sprite, memoria);
        }
        battle.resolve_follow_ups();
    }

    fn finish_burst(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize, consumed: u32) {
        Self::set_memoria(battle, owner, 0);
        let state = &mut battle.units[owner].evernight_state;
        state.burst_advance_used = false;
        state.burst_consumed = consumed;
        if let Some(charge) = &mut state.darkest_riddle {
            *charge = charge.saturating_sub(1);
        }
        battle.spend_hp(sprite, battle.units[sprite].hp, 0);
        battle.despawn_memosprite(owner, sprite);
        if self.dark_the_night {
            battle.restore_skill_points(owner, 1);
        }
        if self.eidolon == 6 {
            self.gain_memoria(battle, owner, percentage_of(consumed, 3_000));
        }
    }
}

impl Mechanics for Evernight {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        let hp = percentage_of(master.hp, EVEY_HP_BP).max(1);
        Some(MemospriteProfile {
            name: EVEY,
            stats: Stats {
                hp,
                base_hp: percentage_of(master.base_hp, EVEY_HP_BP).max(1),
                base_speed_micros: EVEY_SPEED,
                flat_speed_micros: 0,
                base_aggro: 125,
                ..master
            },
        })
    }

    fn memosprite_immune_to_enemy_control(&self, _: &Battle<'_>, _: usize) -> bool {
        true
    }

    fn memosprite_aggro_multiplier_bp(&self) -> u32 {
        40_000
    }

    fn tracks_active_abilities(&self) -> bool {
        self.rouse_the_flame || self.dark_the_night
    }

    fn on_active_ability(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.dark_the_night && pair_member(battle, owner, actor) {
            let cost = percentage_of(battle.units[actor].hp, 500);
            battle.spend_hp(actor, cost, 1);
            battle.units[owner].evernight_state.trace_crit_until =
                battle.units[owner].normal_turns + 2;
        }
        if self.rouse_the_flame && (owner == actor || battle.memosprite_owner(actor).is_some()) {
            battle.gain_energy(actor, owner, 5_000);
            self.gain_memoria(battle, owner, 1);
        }
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        Self::summon(battle, owner);
        if self.rouse_the_flame {
            battle.gain_energy(owner, owner, 70_000);
            self.gain_memoria(battle, owner, 1);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Basic {
            battle.units[owner].evernight_state.last_target = target;
        }
        if action == Action::Skill {
            self.cast_skill(battle, owner);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate
            && let Some(ode) = super::cyrene::time_ode(battle, owner)
        {
            self.gain_memoria(battle, owner, ode.memoria);
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if let Some(sprite) = Self::summon(battle, owner) {
            for target in battle.enemies() {
                battle.memosprite_hp_hit(
                    sprite,
                    target,
                    Element::Ice,
                    u32::from(self.ult_damage_bp),
                    30,
                    Action::Ultimate,
                );
            }
        }
        self.enter_riddle(battle, owner);
    }

    fn attack_source(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Ability<'_>,
    ) -> usize {
        if action == Action::Ultimate {
            battle.active_memosprite(actor).unwrap_or(actor)
        } else {
            actor
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        if let Some(sprite) = battle.active_memosprite(owner)
            && let Some(target) = Self::target(battle, owner)
        {
            self.evey_turn(battle, owner, sprite, target);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if actor == owner && !extra {
            let state = &mut battle.units[owner].evernight_state;
            state.departure_speed_bp = 0;
            if state.darkest_riddle == Some(0) {
                state.darkest_riddle = None;
                battle.emit(
                    EventKind::StatusExpired(StatusKind::DarkestRiddle),
                    Some(owner),
                    Some(owner),
                );
            }
        }
    }

    fn on_memosprite_departed(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        let state = &mut battle.units[owner].evernight_state;
        state.departure_speed_bp = 1_000 + std::mem::take(&mut state.burst_consumed).min(40) * 100;
    }

    fn on_party_hp_lost(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        _: u32,
        received_attack: Option<u32>,
    ) {
        if !pair_member(battle, owner, target) {
            return;
        }
        let turn = battle.units[owner].normal_turns;
        let state = &mut battle.units[owner].evernight_state;
        if let Some(id) = received_attack {
            let last = &mut state.received_attacks[usize::from(target != owner)];
            if last.replace(id) == Some(id) {
                return;
            }
        }
        state.talent_crit_until = turn + 2;
        self.gain_memoria(battle, owner, 2);
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, owner: usize) -> bool {
        let state = &battle.units[owner].evernight_state;
        state.darkest_riddle.is_some() || state.memoria >= MEMORIA_THRESHOLD
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        battle.units[owner].evernight_state.departure_speed_bp
    }

    fn ally_crit_rate_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        3_500 * u32::from(self.dark_the_night && pair_member(battle, owner, target))
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let unit = &battle.units[owner];
        let state = &unit.evernight_state;
        let personal = if pair_member(battle, owner, target) {
            u32::from(self.talent_crit_bp) * u32::from(unit.normal_turns < state.talent_crit_until)
                + 1_500 * u32::from(unit.normal_turns < state.trace_crit_until)
                + 4_000 * u32::from(self.eidolon >= 2)
        } else {
            0
        };
        if !battle.is_targetable(owner)
            || battle.memosprite_owner(target).is_none()
            || unit.normal_turns >= state.skill_until
        {
            return personal;
        }
        let trace_bonus = if self.kindle_the_morn {
            let count = battle
                .allies()
                .filter(|&ally| {
                    battle.plan.units[ally]
                        .combat_type
                        .is_some_and(|kind| kind.path == CombatPath::Remembrance)
                })
                .count()
                .clamp(1, 4);
            [500, 1_500, 5_000, 6_500][count - 1]
        } else {
            0
        };
        personal + state.skill_crit_bp + trace_bonus
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _: Element,
    ) -> u32 {
        if !pair_member(battle, owner, target) {
            return 0;
        }
        u32::from(self.evey_field_damage_bp) * u32::from(battle.active_memosprite(owner).is_some())
            + u32::from(self.riddle_damage_bp)
                * u32::from(battle.units[owner].evernight_state.darkest_riddle.is_some())
    }

    fn ally_memosprite_damage_factor_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.eidolon == 0 || !battle.is_targetable(owner) {
            return 10_000;
        }
        match battle
            .enemies()
            .filter(|&enemy| battle.is_targetable(enemy))
            .count()
        {
            1 => 15_000,
            2 => 13_000,
            3 => 12_500,
            _ => 12_000,
        }
    }

    fn ally_toughness_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        if self.eidolon < 4 || !battle.is_targetable(owner) {
            return 0;
        }
        battle
            .memosprite_owner(actor)
            .map_or(0, |master| 2_500 + 2_500 * u32::from(master == owner))
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
        DamageTargetAura {
            vulnerability_bp: u32::from(self.riddle_vulnerability_bp)
                * u32::from(battle.units[owner].evernight_state.darkest_riddle.is_some()),
            resistance_down_bp: 2_000 * u32::from(self.eidolon == 6),
            ..DamageTargetAura::default()
        }
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill => Some(2),
            _ => None,
        }
    }
}
