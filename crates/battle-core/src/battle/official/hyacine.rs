use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Stats, Target},
};

use super::{
    super::{
        Battle,
        resolve::DamageTargetAura,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
    healing::{Healing, MICROS},
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1409";
const ICA: &str = "little-ica";
const BASIC: &str = "skill.140901";
const SKILL: &str = "skill.140902";
const ULTIMATE: &str = "skill.140903";
const TALENT: &str = "skill.140904";
const MEM_SKILL: &str = "skill.1140901";
const MEM_TALENT: &str = "skill.1140903";
const MEM_ARRIVAL: &str = "skill.1140905";
const MEM_DEPARTURE: &str = "skill.1140906";
const GLOOMY_GRIN: &str = "trace.1409101";
const STORMY_CARESS: &str = "trace.1409102";
const TEMPESTUOUS_HALT: &str = "trace.1409103";
const ENERGY_CAPACITY: u32 = 140;
const SPEED_THRESHOLD: u64 = 200;
const SPEED_BONUS_CAP: u64 = 200;

#[derive(Clone, Default)]
pub(in crate::battle) struct HyacineState {
    healing_tally: u64,
    after_rain_until: Option<u32>,
    pending_heals: Vec<usize>,
    damage_stack_expiries: Vec<u32>,
}

struct Hyacine {
    eidolon: u8,
    gloomy_grin: bool,
    stormy_caress: bool,
    tempestuous_halt: bool,
    healing_bonus_micros: u64,
    skill_heal: Healing,
    skill_heal_ica: Healing,
    ultimate_heal: Healing,
    ultimate_heal_ica: Healing,
    rain_hp: Healing,
    rain_turns: u32,
    ica_hp_bp: u32,
    talent_damage_bp: u32,
    talent_turns: u32,
    talent_stacks: usize,
    ica_damage_bp: u16,
    tally_cleared_bp: u16,
    ica_hp_cost_bp: u32,
    talent_heal: Healing,
    talent_team_heal: Healing,
    first_summon_energy_milli: u32,
    summon_energy_milli: u32,
    departure_advance_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != Element::Wind
        || input.eidolon > 6
    {
        return Err(invalid("combat type, energy or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 7)?;
    let talent = input.parameters(TALENT, 5)?;
    let ica_skill = input.parameters(MEM_SKILL, 2)?;
    let ica_talent = input.parameters(MEM_TALENT, 5)?;
    let arrival = input.parameters(MEM_ARRIVAL, 2)?;
    let departure = input.parameters(MEM_DEPARTURE, 1)?;
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let heal = |id, hp, flat| Healing::from_data(FORM, id, hp, flat);
    if talent[1] != 1.0 || ica_talent[0] != 0.04 || ica_skill[1] != 0.5 {
        return Err(invalid("unexpected frozen Ica conditions"));
    }
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Wind,
                multiplier_bp: ratio_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: restore_ability(30_000, SkillPoints::Spend),
        ultimate: restore_ability(5_000, SkillPoints::Unchanged),
        mechanics: Arc::new(Hyacine {
            eidolon: input.eidolon,
            gloomy_grin: trace(GLOOMY_GRIN),
            stormy_caress: trace(STORMY_CARESS),
            tempestuous_halt: trace(TEMPESTUOUS_HALT),
            healing_bonus_micros: u64::from(input.stats.healing_bp) * 100,
            skill_heal: heal(SKILL, skill[0], skill[1])?,
            skill_heal_ica: heal(SKILL, skill[2], skill[3])?,
            ultimate_heal: heal(ULTIMATE, ultimate[0], ultimate[1])?,
            ultimate_heal_ica: heal(ULTIMATE, ultimate[5], ultimate[6])?,
            rain_hp: heal(ULTIMATE, ultimate[2], ultimate[3])?,
            rain_turns: count(ULTIMATE, ultimate[4])?,
            ica_hp_bp: u32::from(ratio_bp(TALENT, talent[0])?),
            talent_damage_bp: u32::from(ratio_bp(TALENT, talent[2])?),
            talent_turns: count(TALENT, talent[3])?,
            talent_stacks: count(TALENT, talent[4])? as usize,
            ica_damage_bp: ratio_bp(MEM_SKILL, ica_skill[0])?,
            tally_cleared_bp: if input.eidolon >= 6 {
                1_200
            } else {
                ratio_bp(MEM_SKILL, ica_skill[1])?
            },
            ica_hp_cost_bp: u32::from(ratio_bp(MEM_TALENT, ica_talent[0])?),
            talent_heal: heal(MEM_TALENT, ica_talent[1], ica_talent[2])?,
            talent_team_heal: heal(MEM_TALENT, ica_talent[3], ica_talent[4])?,
            summon_energy_milli: count(MEM_ARRIVAL, arrival[0])? * 1_000,
            first_summon_energy_milli: count(MEM_ARRIVAL, arrival[1])? * 1_000,
            departure_advance_bp: ratio_bp(MEM_DEPARTURE, departure[0])?,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn count(id: &str, value: f64) -> Result<u32, BattleError> {
    if !(1.0..=140.0).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u32)
}

fn ratio_bp(id: &str, value: f64) -> Result<u16, BattleError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(invalid(id));
    }
    Ok((value * 10_000.0).round() as u16)
}

fn restore_ability(energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::AllAllies,
        energy_milli,
        effects: Vec::new(),
        skill_points,
    }
}

fn rain_active(battle: &Battle<'_>, owner: usize) -> bool {
    battle.is_targetable(owner)
        && battle.units[owner]
            .hyacine_state
            .after_rain_until
            .is_some_and(|end| battle.units[owner].normal_turns < end)
}

impl Hyacine {
    fn excess_speed(&self, battle: &Battle<'_>, owner: usize) -> u64 {
        if !self.tempestuous_halt {
            return 0;
        }
        (battle.speed_micros(owner) / MICROS)
            .saturating_sub(SPEED_THRESHOLD)
            .min(SPEED_BONUS_CAP)
    }

    fn max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let base_hp = battle.plan.units[target].stats.base_hp;
        let is_ica = battle.memosprite_owner(target) == Some(owner);
        let halt = self.tempestuous_halt
            && (owner == target || is_ica)
            && battle.speed_micros(owner) > SPEED_THRESHOLD * MICROS;
        let rain = if rain_active(battle, owner) {
            self.rain_hp
                .amount(base_hp, 0)
                .saturating_add(percentage_of(base_hp, 5_000 * u32::from(self.eidolon >= 1)))
        } else {
            0
        };
        rain.saturating_add(percentage_of(base_hp, 2_000 * u32::from(halt)))
    }

    fn set_rain(&self, battle: &mut Battle<'_>, owner: usize, active: bool) {
        // At turn opening the owner counter has already crossed the expiration threshold.
        let previous = active.then(|| {
            battle
                .party_units()
                .map(|unit| (unit, battle.effective_max_hp(unit)))
                .collect::<Vec<_>>()
        });
        battle.units[owner].hyacine_state.after_rain_until =
            active.then_some(battle.units[owner].normal_turns + self.rain_turns);
        if let Some(previous) = previous {
            for (unit, max_before) in previous {
                if battle.is_targetable(unit) {
                    let max_after = battle.effective_max_hp(unit);
                    let hp = &mut battle.units[unit].hp;
                    *hp = hp
                        .saturating_add(max_after.saturating_sub(max_before))
                        .min(max_after);
                }
            }
        } else {
            for unit in battle.party_units() {
                battle.units[unit].hp = battle.units[unit].hp.min(battle.effective_max_hp(unit));
            }
        }
        let event = if active {
            EventKind::StatusApplied(StatusKind::AfterRain)
        } else {
            EventKind::StatusExpired(StatusKind::AfterRain)
        };
        battle.emit(event, Some(owner), Some(owner));
    }

    fn summon(&self, battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        let sprite = battle.memosprite(owner)?;
        if !battle.is_targetable(sprite) {
            // The active team aura is applied independently to the new sprite.
            let hp = battle.effective_max_hp_excluding(owner, Some(owner));
            battle.units[sprite].memosprite_max_hp = Some(percentage_of(hp, self.ica_hp_bp));
            let first = battle.units[sprite].memosprite_generation == 0;
            battle.units[sprite].memosprite_generation += 1;
            battle.units[sprite].hp = battle.effective_max_hp(sprite);
            battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
            battle.gain_energy(
                sprite,
                owner,
                self.summon_energy_milli + self.first_summon_energy_milli * u32::from(first),
            );
        }
        Some(sprite)
    }

    fn heal(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
        heal: Healing,
    ) {
        if !battle.is_targetable(target) {
            return;
        }
        let (hp, max_hp) = battle.hp(target);
        let low_hp_bonus = 250_000 * u64::from(self.gloomy_grin && hp <= max_hp / 2);
        let boost =
            self.healing_bonus_micros + low_hp_bonus + 10_000 * self.excess_speed(battle, owner);
        let amount = heal.amount(battle.effective_max_hp(owner), boost);
        let amount = battle.heal_unit_with_overflow(source, target, amount);
        if amount == 0 {
            return;
        }
        let expiry = battle
            .active_memosprite(owner)
            .map(|sprite| battle.units[sprite].normal_turns + self.talent_turns);
        let state = &mut battle.units[owner].hyacine_state;
        state.healing_tally = state.healing_tally.saturating_add(u64::from(amount));
        if let Some(expiry) = expiry {
            if state.damage_stack_expiries.len() == self.talent_stacks {
                state.damage_stack_expiries.remove(0);
            }
            state.damage_stack_expiries.push(expiry);
        }
    }

    fn heal_party(&self, battle: &mut Battle<'_>, owner: usize, action: Action) {
        let Some(sprite) = self.summon(battle, owner) else {
            return;
        };
        let (party_heal, sprite_heal) = if action == Action::Ultimate {
            (self.ultimate_heal, self.ultimate_heal_ica)
        } else {
            (self.skill_heal, self.skill_heal_ica)
        };
        for target in battle.party_units() {
            if self.stormy_caress && battle.is_targetable(target) {
                battle.cleanse_one(owner, target);
            }
            let heal = if target == sprite {
                sprite_heal
            } else {
                party_heal
            };
            self.heal(battle, owner, owner, target, heal);
        }
        if action == Action::Ultimate {
            self.set_rain(battle, owner, true);
        }
    }

    fn consume_pending_heals(&self, battle: &mut Battle<'_>, owner: usize) {
        let pending = std::mem::take(&mut battle.units[owner].hyacine_state.pending_heals);
        for target in pending {
            let Some(sprite) = battle.active_memosprite(owner) else {
                break;
            };
            if !battle.is_targetable(target) {
                continue;
            }
            let cost = percentage_of(battle.effective_max_hp(sprite), self.ica_hp_cost_bp).max(1);
            let lethal = battle.units[sprite].hp <= cost;
            battle.spend_ally_hp(sprite, sprite, cost, 0);
            battle.emit(EventKind::TriggeredSkill, Some(sprite), Some(target));
            self.heal(battle, owner, sprite, target, self.talent_heal);
            if rain_active(battle, owner) {
                for ally in battle.party_units() {
                    self.heal(battle, owner, sprite, ally, self.talent_team_heal);
                }
            }
            if lethal {
                battle.despawn_memosprite(owner, sprite);
            }
        }
    }

    fn ica_attack(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        if !battle.enemies().any(|target| battle.is_targetable(target)) {
            return;
        }
        battle.units[sprite].normal_turns += 1;
        battle.emit(EventKind::ExtraTurnStarted, Some(sprite), None);
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        battle.notify_active_ability(sprite);
        let tally = &mut battle.units[owner].hyacine_state.healing_tally;
        // Match the damage engine's integer input range without truncating ordinary tallies.
        let base_bp = (*tally).min(u64::from(u32::MAX)) * u64::from(self.ica_damage_bp);
        *tally -= *tally / 10_000 * u64::from(self.tally_cleared_bp)
            + (*tally % 10_000) * u64::from(self.tally_cleared_bp) / 10_000;
        let parent_debuff_targets = battle.begin_debuff_action();
        battle.begin_attack();
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                battle.memosprite_fixed_base_hit(sprite, target, Element::Wind, base_bp, 10);
            }
        }
        battle.notify_attack(sprite, Action::Skill, None);
        battle.finish_debuff_action(sprite, parent_debuff_targets);
        battle.gain_energy(sprite, owner, 5_000);
        battle.resolve_follow_ups();
        battle.notify_memosprite_turn_finished(sprite);
        battle.finish_turn(sprite);
        let elapsed = battle.units[sprite].normal_turns;
        battle.units[owner]
            .hyacine_state
            .damage_stack_expiries
            .retain(|&end| end > elapsed);
    }
}

impl Mechanics for Hyacine {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].hp = battle.effective_max_hp(owner);
    }

    fn memosprite_profile(&self, mut master: Stats) -> Option<MemospriteProfile> {
        master.hp = percentage_of(master.hp, self.ica_hp_bp);
        master.base_hp = percentage_of(master.base_hp, self.ica_hp_bp);
        master.base_aggro = 100;
        master.base_speed_micros = 0;
        master.flat_speed_micros = 0;
        Some(MemospriteProfile {
            name: ICA,
            stats: master,
        })
    }

    fn memosprite_has_fixed_zero_speed(&self) -> bool {
        true
    }

    fn memosprite_immune_to_debuffs(&self) -> bool {
        true
    }

    fn memosprite_guarantees_critical(&self, _: Option<Action>) -> bool {
        self.gloomy_grin
    }

    fn on_memosprite_departed(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        battle.units[owner]
            .hyacine_state
            .damage_stack_expiries
            .clear();
        if battle.is_targetable(owner) {
            battle.advance_ally(owner, owner, self.departure_advance_bp);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        if battle.units[owner].hyacine_state.after_rain_until.is_some() {
            self.set_rain(battle, owner, false);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if matches!(action, Action::Skill | Action::Ultimate) {
            self.heal_party(battle, owner, action);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if !battle.is_targetable(owner) {
            return;
        }
        if owner == actor
            && !extra
            && battle.units[owner].hyacine_state.after_rain_until.is_some()
            && !rain_active(battle, owner)
        {
            self.set_rain(battle, owner, false);
        }
        self.consume_pending_heals(battle, owner);
    }

    fn on_enemy_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if battle.is_targetable(owner) {
            self.consume_pending_heals(battle, owner);
        }
    }

    fn tracks_action_end_reactions(&self) -> bool {
        true
    }

    fn react_to_action_end(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        self.consume_pending_heals(battle, owner);
        if actor == owner
            && rain_active(battle, owner)
            && let Some(sprite) = battle.active_memosprite(owner)
        {
            self.ica_attack(battle, owner, sprite);
        }
    }

    fn on_party_hp_lost(&self, battle: &mut Battle<'_>, owner: usize, target: usize, _: u32) {
        if !battle.is_targetable(owner) || !battle.is_targetable(target) {
            return;
        }
        if self.eidolon >= 2 {
            battle.apply_ally_buff(owner, target, AllyBuffKind::SpeedPercent, 3_000, 2);
        }
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        if sprite != target {
            let pending = &mut battle.units[owner].hyacine_state.pending_heals;
            if !pending.contains(&target) {
                pending.push(target);
            }
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: Action,
        _: Option<usize>,
    ) {
        if self.eidolon >= 1 && rain_active(battle, owner) {
            self.heal(
                battle,
                owner,
                owner,
                actor,
                Healing {
                    hp_ratio_micros: 80_000,
                    flat_micros: 0,
                },
            );
        }
    }

    fn ally_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        self.max_hp_bonus(battle, owner, target)
    }

    fn memosprite_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, sprite: usize) -> u32 {
        self.max_hp_bonus(battle, owner, sprite)
    }

    fn effect_resistance_bonus_bp(&self, _: &Battle<'_>, _: usize) -> u32 {
        5_000 * u32::from(self.stormy_caress)
    }

    fn ally_crit_rate_bonus_bp(&self, _: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        10_000 * u32::from(self.gloomy_grin && owner == actor)
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        if self.eidolon >= 4 && (owner == actor || battle.memosprite_owner(actor) == Some(owner)) {
            200 * self.excess_speed(battle, owner) as u32
        } else {
            0
        }
    }

    fn memosprite_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        sprite: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if kind != DamageKind::Direct {
            return 0;
        }
        battle.memosprite_owner(sprite).map_or(0, |owner| {
            self.talent_damage_bp
                * battle.units[owner]
                    .hyacine_state
                    .damage_stack_expiries
                    .len() as u32
        })
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        DamageTargetAura {
            resistance_down_bp: 2_000
                * u32::from(
                    self.eidolon >= 6
                        && battle.is_party_unit(actor)
                        && battle.active_memosprite(owner).is_some(),
                ),
            ..DamageTargetAura::default()
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill
                if rain_active(battle, owner) || battle.active_memosprite(owner).is_none() =>
            {
                Some(1)
            }
            _ => None,
        }
    }
}
