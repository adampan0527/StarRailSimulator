use crate::{
    Action, DamageKind, StatusKind,
    fixture::{Ability, Element},
};

use super::{Battle, combat_status::DebuffKind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AllyBuffKind {
    AttackFlat,
    AttackPercent,
    BreakEffect,
    CritRate,
    CritDamage,
    CritOverflowRatio,
    Damage,
    ResistancePenetration,
    DefenseIgnore {
        summon: bool,
    },
    DefensePercent,
    EffectResistance,
    MaxHpFlat,
    /// Flat speed in millionths of one speed point.
    SpeedFlat,
    SpeedPercent,
}

#[derive(Clone, Copy)]
pub(super) struct AllyBuff {
    source: usize,
    kind: AllyBuffKind,
    value: u32,
    expires_after_turn: u32,
    effect_id: Option<&'static str>,
}

#[derive(Clone, Default)]
pub(super) struct AstaState {
    pub charge_stacks: u8,
    pub hit_targets: Vec<usize>,
    pub preserve_charge_on_next_turn: bool,
}

#[derive(Clone, Default)]
pub(super) struct BronyaState {
    pub skill_point_ready_at: u32,
    pub follow_up_ready_at: u32,
    pub pending_speed_target: Option<usize>,
}

#[derive(Clone, Default)]
pub(super) struct TingyunState {
    pub benediction_target: Option<usize>,
    pub benediction_expires_after_turn: u32,
    pub benediction_attack_cap: u32,
    pub e2_refund_turn: Option<u32>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct OffenseBuffs {
    pub attack_bp: u32,
    pub attack_flat: u32,
    pub break_effect_bp: u32,
    pub toughness_efficiency_bp: u32,
    pub off_element_toughness_bp: u16,
    pub break_damage_bonus_bp: u32,
    pub max_hp_flat: u32,
    pub defense_bp: u32,
    pub defense_ignore_bp: u32,
    pub resistance_penetration_bp: u32,
    pub elation_bp: u32,
    pub merrymake_bp: u32,
    pub damage_bp: u32,
    pub crit_rate_bp: u32,
    pub crit_damage_bp: u32,
    pub crit_overflow_ratio_bp: u32,
}

#[derive(Clone, Copy)]
pub(super) struct OffenseContext {
    pub actor: usize,
    pub max_hp_source: usize,
    pub enemy: Option<usize>,
    pub element: Element,
    pub kind: DamageKind,
    pub break_damage: bool,
    pub follow_up: bool,
    pub summon: bool,
    pub direct_skill: bool,
}

impl OffenseBuffs {
    fn add(&mut self, buff: Self) {
        self.attack_bp = self.attack_bp.saturating_add(buff.attack_bp);
        self.attack_flat = self.attack_flat.saturating_add(buff.attack_flat);
        self.break_effect_bp = self.break_effect_bp.saturating_add(buff.break_effect_bp);
        self.toughness_efficiency_bp = self
            .toughness_efficiency_bp
            .saturating_add(buff.toughness_efficiency_bp);
        self.off_element_toughness_bp = self
            .off_element_toughness_bp
            .max(buff.off_element_toughness_bp);
        self.break_damage_bonus_bp = self
            .break_damage_bonus_bp
            .saturating_add(buff.break_damage_bonus_bp);
        self.max_hp_flat = self.max_hp_flat.saturating_add(buff.max_hp_flat);
        self.defense_bp = self.defense_bp.saturating_add(buff.defense_bp);
        self.defense_ignore_bp = self
            .defense_ignore_bp
            .saturating_add(buff.defense_ignore_bp);
        self.resistance_penetration_bp = self
            .resistance_penetration_bp
            .saturating_add(buff.resistance_penetration_bp);
        self.elation_bp = self.elation_bp.saturating_add(buff.elation_bp);
        self.merrymake_bp = self.merrymake_bp.saturating_add(buff.merrymake_bp);
        self.damage_bp = self.damage_bp.saturating_add(buff.damage_bp);
        self.crit_rate_bp = self.crit_rate_bp.saturating_add(buff.crit_rate_bp);
        self.crit_damage_bp = self.crit_damage_bp.saturating_add(buff.crit_damage_bp);
        self.crit_overflow_ratio_bp = self
            .crit_overflow_ratio_bp
            .saturating_add(buff.crit_overflow_ratio_bp);
    }
}

impl Battle<'_> {
    pub(super) fn notify_ally_sustain(&mut self, source: usize, target: usize) {
        if source != target
            && self.is_party_unit(source)
            && let Some(rules) = self.mechanics(target)
        {
            rules.on_ally_sustain(self, target);
        }
    }

    pub(super) fn extend_active_buffs_by_one_turn(&mut self, target: usize) {
        let state = &mut self.units[target];
        for kind in [StatusKind::Shield, StatusKind::SpeedUp] {
            if let Some(status) = &mut state.statuses[kind.index()] {
                status.expires_after_turn = status.expires_after_turn.saturating_add(1);
            }
        }
        for buff in &mut state.ally_buffs {
            buff.expires_after_turn = buff.expires_after_turn.saturating_add(1);
        }
        for shield in &mut state.shields {
            shield.expires_after_turn = shield.expires_after_turn.saturating_add(1);
        }
        if let Some(invigorated) = &mut state.invigoration {
            invigorated.expires_after_turn = invigorated.expires_after_turn.saturating_add(1);
        }
        if let Some(response) = &mut state.lynx_response {
            response.expires_after_turn = response.expires_after_turn.saturating_add(1);
        }
        for heal in &mut state.heals_over_time {
            heal.turns = heal.turns.saturating_add(1);
        }
    }

    pub(super) fn apply_ally_buff(
        &mut self,
        source: usize,
        target: usize,
        kind: AllyBuffKind,
        value: u32,
        turns: u32,
    ) {
        self.apply_ally_buff_with_id(source, target, kind, value, turns, None);
    }

    pub(super) fn apply_ally_buff_with_id(
        &mut self,
        source: usize,
        target: usize,
        kind: AllyBuffKind,
        value: u32,
        turns: u32,
        effect_id: Option<&'static str>,
    ) {
        if self.units[target].hp == 0
            || (source != target && !self.is_targetable(target))
            || value == 0
            || turns == 0
        {
            return;
        }
        let state = &mut self.units[target];
        let buff = AllyBuff {
            source,
            kind,
            value,
            expires_after_turn: state.normal_turns + turns,
            effect_id,
        };
        if let Some(slot) = state
            .ally_buffs
            .iter_mut()
            .find(|item| item.source == source && item.kind == kind && item.effect_id == effect_id)
        {
            *slot = buff;
        } else {
            state.ally_buffs.push(buff);
        }
    }

    pub(super) fn has_ally_buff(
        &self,
        source: usize,
        target: usize,
        kind: AllyBuffKind,
        effect_id: &'static str,
    ) -> bool {
        self.units[target].ally_buffs.iter().any(|buff| {
            buff.source == source && buff.kind == kind && buff.effect_id == Some(effect_id)
        })
    }

    pub(super) fn expire_ally_buffs(&mut self, actor: usize) {
        let state = &mut self.units[actor];
        let completed = state.normal_turns;
        state
            .ally_buffs
            .retain(|buff| buff.expires_after_turn > completed);
        self.units[actor].hp = self.units[actor].hp.min(self.effective_max_hp(actor));
    }

    pub(super) fn ally_buff_value(&self, source: usize, target: usize, kind: AllyBuffKind) -> u32 {
        self.units[target]
            .ally_buffs
            .iter()
            .find(|buff| buff.source == source && buff.kind == kind)
            .map_or(0, |buff| buff.value)
    }

    pub(super) fn apply_max_hp_increase(
        &mut self,
        source: usize,
        target: usize,
        amount: u32,
        turns: u32,
    ) {
        let before = self.effective_max_hp(target);
        self.apply_ally_buff(source, target, AllyBuffKind::MaxHpFlat, amount, turns);
        let after = self.effective_max_hp(target);
        self.units[target].hp = self.units[target]
            .hp
            .saturating_add(after.saturating_sub(before))
            .min(after);
    }

    pub(super) fn effective_max_hp(&self, actor: usize) -> u32 {
        self.effective_max_hp_excluding(actor, None)
    }

    pub(super) fn effective_max_hp_excluding(&self, actor: usize, source: Option<usize>) -> u32 {
        let max_hp = self.units[actor]
            .memosprite_max_hp
            .unwrap_or(self.plan.units[actor].stats.hp)
            .saturating_add(self.ally_buff_total(actor, AllyBuffKind::MaxHpFlat));
        if !self.is_party_unit(actor) {
            return max_hp;
        }
        let total = self
            .allies()
            .filter(|&owner| Some(owner) != source)
            .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
            .fold(max_hp, |hp, (owner, rules)| {
                let bonus = if self.memosprites().contains(&actor) {
                    rules.memosprite_max_hp_bonus(self, owner, actor)
                } else {
                    rules.ally_max_hp_bonus(self, owner, actor)
                };
                hp.saturating_add(bonus)
            });
        percentage_of(total, self.max_hp_multiplier_bp(actor))
    }

    pub(super) fn max_hp_multiplier_bp(&self, actor: usize) -> u32 {
        self.mechanics(actor)
            .map_or(10_000, |rules| rules.max_hp_multiplier_bp(self, actor))
    }

    fn ally_buff_total(&self, actor: usize, kind: AllyBuffKind) -> u32 {
        self.units[actor]
            .ally_buffs
            .iter()
            .filter(|buff| buff.kind == kind)
            .map(|buff| buff.value)
            .fold(0, u32::saturating_add)
    }

    pub(super) fn support_speed(&self, actor: usize) -> (u64, u64) {
        let buff_speed = (
            u64::from(self.ally_buff_total(actor, AllyBuffKind::SpeedFlat)),
            u64::from(self.ally_buff_total(actor, AllyBuffKind::SpeedPercent)),
        );
        self.allies()
            .filter(|&owner| self.units[owner].hp > 0)
            .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
            .fold(buff_speed, |(flat, ratio), (owner, rules)| {
                (
                    flat + rules.ally_flat_speed_micros(self, owner, actor),
                    ratio + u64::from(rules.ally_speed_bonus_bp(self, owner, actor)),
                )
            })
    }

    pub(super) fn support_break_effect(&self, actor: usize) -> u32 {
        let buffs = self.ally_buff_total(actor, AllyBuffKind::BreakEffect);
        if !self.allies().contains(&actor) {
            return buffs;
        }
        self.allies()
            .filter(|&owner| self.is_targetable(owner))
            .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
            .fold(buffs, |total, (owner, rules)| {
                total.saturating_add(rules.ally_break_effect_bonus_bp(self, owner, actor))
            })
    }

    pub(super) fn effective_break_effect(&self, actor: usize) -> u32 {
        let own_bonus = self.mechanics(actor).map_or(0, |rules| {
            rules
                .break_effect_bonus_bp(self, actor)
                .saturating_add(rules.break_effect_from_attack_bp(self, actor, None))
        });
        self.plan.units[actor]
            .stats
            .break_effect_bp
            .saturating_add(own_bonus)
            .saturating_add(self.support_break_effect(actor))
    }

    pub(super) fn support_effect_resistance(&self, actor: usize) -> u32 {
        self.ally_buff_total(actor, AllyBuffKind::EffectResistance)
    }

    pub(super) fn offense_buffs_from(
        &self,
        source: usize,
        context: OffenseContext,
    ) -> OffenseBuffs {
        let OffenseContext {
            actor: target,
            max_hp_source,
            enemy,
            element,
            kind,
            break_damage,
            follow_up,
            summon,
            direct_skill,
        } = context;
        let mut bonuses = OffenseBuffs::default();
        for buff in self.units[target]
            .ally_buffs
            .iter()
            .filter(|buff| buff.source == source)
        {
            let total = match buff.kind {
                AllyBuffKind::AttackFlat => &mut bonuses.attack_flat,
                AllyBuffKind::AttackPercent => &mut bonuses.attack_bp,
                AllyBuffKind::BreakEffect => &mut bonuses.break_effect_bp,
                AllyBuffKind::CritRate => &mut bonuses.crit_rate_bp,
                AllyBuffKind::CritDamage => &mut bonuses.crit_damage_bp,
                AllyBuffKind::CritOverflowRatio => &mut bonuses.crit_overflow_ratio_bp,
                AllyBuffKind::Damage => &mut bonuses.damage_bp,
                AllyBuffKind::ResistancePenetration => &mut bonuses.resistance_penetration_bp,
                AllyBuffKind::DefenseIgnore {
                    summon: applies_to_summon,
                } if summon == applies_to_summon => &mut bonuses.defense_ignore_bp,
                AllyBuffKind::DefensePercent => &mut bonuses.defense_bp,
                AllyBuffKind::SpeedFlat
                | AllyBuffKind::SpeedPercent
                | AllyBuffKind::EffectResistance
                | AllyBuffKind::MaxHpFlat
                | AllyBuffKind::DefenseIgnore { .. } => continue,
            };
            *total = total.saturating_add(buff.value);
        }
        bonuses.max_hp_flat = self.units[max_hp_source]
            .ally_buffs
            .iter()
            .filter(|buff| buff.source == source && buff.kind == AllyBuffKind::MaxHpFlat)
            .map(|buff| buff.value)
            .fold(0, u32::saturating_add);
        if let Some(enemy) = enemy {
            for debuff in self.units[enemy]
                .debuffs
                .iter()
                .filter(|debuff| debuff.source == source && debuff.kind == DebuffKind::Unnerved)
            {
                bonuses.crit_damage_bp = bonuses.crit_damage_bp.saturating_add(debuff.bp);
            }
        }
        if self.units[source].hp > 0
            && let Some(mechanics) = self.mechanics(source)
        {
            if self.is_party_unit(target) && self.is_targetable(source) {
                if self.aha.is_some() {
                    bonuses.elation_bp = mechanics.ally_elation_bp(self, source, target);
                }
                if kind == DamageKind::Elation {
                    bonuses.merrymake_bp = mechanics.ally_merrymake_bp(self, source, target);
                    bonuses.defense_ignore_bp = bonuses
                        .defense_ignore_bp
                        .saturating_add(mechanics.ally_elation_defense_ignore_bp(self, source));
                }
                bonuses.break_effect_bp = bonuses
                    .break_effect_bp
                    .saturating_add(mechanics.ally_break_effect_bonus_bp(self, source, target));
                if kind == DamageKind::SuperBreak {
                    bonuses.toughness_efficiency_bp =
                        mechanics.ally_toughness_bonus_bp(self, source, target);
                    bonuses.off_element_toughness_bp =
                        mechanics.off_element_toughness_bp(self, source, target);
                }
                if break_damage {
                    bonuses.break_damage_bonus_bp =
                        mechanics.ally_break_damage_bonus_bp(self, source, target);
                }
            }
            bonuses.max_hp_flat = bonuses.max_hp_flat.saturating_add(
                if self.memosprite_owner(max_hp_source).is_some() {
                    mechanics.memosprite_max_hp_bonus(self, source, max_hp_source)
                } else {
                    mechanics.ally_max_hp_bonus(self, source, max_hp_source)
                },
            );
            bonuses.attack_bp = bonuses
                .attack_bp
                .saturating_add(mechanics.ally_attack_bonus_bp(self, source, target));
            bonuses.attack_flat = bonuses
                .attack_flat
                .saturating_add(mechanics.ally_attack_bonus_flat(self, source, target));
            bonuses.crit_rate_bp = bonuses
                .crit_rate_bp
                .saturating_add(mechanics.ally_crit_rate_bonus_bp(self, source, target));
            bonuses.crit_damage_bp = bonuses
                .crit_damage_bp
                .saturating_add(mechanics.ally_crit_damage_bonus_bp(self, source, target));
            if direct_skill {
                bonuses.damage_bp = bonuses
                    .damage_bp
                    .saturating_add(mechanics.ally_skill_damage_bonus_bp(self, source, target));
                bonuses.crit_damage_bp = bonuses
                    .crit_damage_bp
                    .saturating_add(mechanics.ally_skill_crit_damage_bp(self, source, target));
                bonuses.defense_ignore_bp = bonuses
                    .defense_ignore_bp
                    .saturating_add(mechanics.ally_skill_defense_ignore_bp(self, source, target));
            }
            bonuses.damage_bp = bonuses
                .damage_bp
                .saturating_add(mechanics.ally_damage_bonus_bp(self, source, target, element));
            if kind == DamageKind::DamageOverTime {
                bonuses.damage_bp = bonuses
                    .damage_bp
                    .saturating_add(mechanics.ally_dot_damage_bonus_bp(self, source, target));
            }
            if let Some(enemy) = enemy {
                bonuses.attack_bp = bonuses
                    .attack_bp
                    .saturating_add(mechanics.ally_target_attack_bonus_bp(self, source, enemy));
                bonuses.damage_bp = bonuses.damage_bp.saturating_add(
                    mechanics.ally_target_damage_bonus_bp(self, source, target, enemy, kind),
                );
                bonuses.crit_damage_bp = bonuses
                    .crit_damage_bp
                    .saturating_add(mechanics.target_crit_damage_bonus_bp(self, source, enemy));
                if follow_up {
                    bonuses.crit_damage_bp = bonuses.crit_damage_bp.saturating_add(
                        mechanics.follow_up_crit_damage_bonus_bp(self, source, enemy),
                    );
                }
            }
        }
        bonuses
    }

    pub(super) fn offense_buffs(
        &self,
        context: OffenseContext,
        allow: impl Fn(usize) -> bool,
    ) -> OffenseBuffs {
        if !self.is_party_unit(context.actor) {
            return OffenseBuffs::default();
        }
        let mut total = OffenseBuffs::default();
        for source in self.allies().filter(|&source| allow(source)) {
            total.add(self.offense_buffs_from(source, context));
        }
        total
    }

    pub(super) fn effective_crit_damage_bp(&self, actor: usize) -> u32 {
        self.effective_crit_damage_bp_without(actor, None)
    }

    pub(super) fn effective_crit_damage_bp_without(
        &self,
        actor: usize,
        excluded_owner: Option<usize>,
    ) -> u32 {
        self.plan.units[actor].stats.crit_damage_bp.saturating_add(
            self.offense_buffs(
                OffenseContext {
                    actor,
                    max_hp_source: actor,
                    enemy: None,
                    element: Element::Physical,
                    kind: DamageKind::Direct,
                    break_damage: false,
                    follow_up: false,
                    summon: self.memosprite_owner(actor).is_some(),
                    direct_skill: false,
                },
                |source| Some(source) != excluded_owner,
            )
            .crit_damage_bp,
        )
    }

    pub(super) fn effective_attack(&self, actor: usize) -> u32 {
        self.effective_attack_excluding(actor, None)
    }

    pub(super) fn effective_attack_excluding(&self, actor: usize, excluded: Option<usize>) -> u32 {
        let buffs = self.offense_buffs(
            OffenseContext {
                actor,
                max_hp_source: actor,
                enemy: None,
                element: Element::Physical,
                kind: DamageKind::Direct,
                break_damage: false,
                follow_up: false,
                summon: false,
                direct_skill: false,
            },
            |source| Some(source) != excluded,
        );
        self.attack_with_buffs(actor, buffs)
    }

    pub(super) fn attack_with_buffs(&self, actor: usize, buffs: OffenseBuffs) -> u32 {
        let stats = self.plan.units[actor].stats;
        let personal_bp = self.units[actor].attack_bonus.map_or(0, |(bp, _)| bp);
        let mechanics = self.mechanics(actor);
        let flat = mechanics
            .as_ref()
            .map_or(0, |rules| rules.bonus_attack_flat(self, actor));
        let conversion_bp = mechanics
            .as_ref()
            .map_or(0, |rules| rules.attack_conversion_from_defense_bp());
        let break_conversion_bp = mechanics.as_ref().map_or(0, |rules| {
            let break_effect = stats
                .break_effect_bp
                .saturating_add(rules.break_effect_bonus_bp(self, actor))
                .saturating_add(buffs.break_effect_bp);
            rules.attack_bonus_from_break_effect_bp(break_effect)
        });
        let defense = self.defense_with_buff(actor, buffs.defense_bp);
        let attack = stats
            .attack
            .saturating_add(percentage_of(
                stats.base_attack,
                personal_bp
                    .saturating_add(buffs.attack_bp)
                    .saturating_add(break_conversion_bp),
            ))
            .saturating_add(percentage_of(defense, conversion_bp))
            .saturating_add(flat)
            .saturating_add(buffs.attack_flat);
        if actor < self.plan.ally_count {
            return attack;
        }
        let reduction = self
            .allies()
            .filter(|&owner| self.units[owner].hp > 0)
            .filter_map(|owner| {
                self.mechanics(owner)
                    .map(|rules| rules.attack_reduction_bp(self, owner, actor))
            })
            .fold(self.attack_down_bp(actor), u32::saturating_add);
        attack.saturating_sub(percentage_of(stats.base_attack, reduction))
    }

    pub(super) fn defense_with_buff(&self, actor: usize, buff_bp: u32) -> u32 {
        let stats = self.plan.units[actor].stats;
        let mechanics = self.mechanics(actor);
        if mechanics
            .as_ref()
            .is_some_and(|rules| rules.defense_is_zero(self, actor))
        {
            return 0;
        }
        let personal_bp = mechanics.map_or(0, |rules| rules.defense_bonus_bp(self, actor));
        stats.defense.saturating_add(percentage_of(
            stats.base_defense,
            buff_bp.saturating_add(personal_bp),
        ))
    }

    pub(super) fn effective_defense(&self, actor: usize) -> u32 {
        self.defense_with_buff(
            actor,
            self.ally_buff_total(actor, AllyBuffKind::DefensePercent),
        )
    }

    pub(super) fn restore_energy_flat(&mut self, source: usize, target: usize, amount_milli: u32) {
        let cap = self.plan.units[target].kit.max_energy_milli();
        if cap == 0 {
            self.notify_external_energy(source, target);
            return;
        }
        let energy = self.units[target]
            .energy_milli
            .saturating_add(amount_milli)
            .min(cap);
        self.set_energy(source, target, energy);
    }

    pub(super) fn notify_external_energy(&mut self, source: usize, target: usize) {
        if source != target
            && self.is_party_unit(source)
            && self.is_targetable(target)
            && let Some(rules) = self.mechanics(target)
        {
            rules.on_external_energy(self, target);
        }
    }

    pub(super) fn notify_ally_turn_opening(&mut self, actor: usize, extra: bool) {
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_ally_turn_opening(self, owner, actor, extra);
            }
        }
    }

    pub(super) fn notify_active_ability(&mut self, actor: usize) {
        if !self.tracks_active_abilities {
            return;
        }
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
                && rules.tracks_active_abilities()
            {
                rules.on_active_ability(self, owner, actor);
            }
        }
    }

    pub(super) fn prepare_skill_copy(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        if !self.tracks_skill_copies
            || !matches!(self.action, Some(Action::Skill | Action::EnhancedSkill))
        {
            return;
        }
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(rules) = self.mechanics(owner)
                && rules.tracks_skill_copies()
            {
                rules.before_ally_skill_copy(self, owner, actor, target, ability);
            }
        }
    }

    pub(super) fn notify_ally_action_started(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(rules) = self.mechanics(owner)
            {
                rules.before_ally_action(self, owner, actor, target, ability);
            }
        }
    }

    pub(super) fn notify_ally_action_finished(&mut self, actor: usize, action: Action) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_ally_action_finished(self, owner, actor, action);
            }
        }
    }

    pub(super) fn notify_ordinary_turn_finished(&mut self, actor: usize) {
        if actor < self.plan.ally_count {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(rules) = self.mechanics(owner)
                {
                    rules.after_ordinary_turn(self, owner, actor);
                }
            }
        }
    }

    pub(super) fn notify_unit_action_finished(&mut self, actor: usize) {
        if self.tracks_action_end_reactions {
            // Resolve new reactions before the owners clear any per-action counters.
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(rules) = self.mechanics(owner)
                    && rules.tracks_action_end_reactions()
                {
                    rules.react_to_action_end(self, owner, actor);
                }
            }
        }
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_unit_action_finished(self, owner, actor);
            }
        }
    }

    pub(super) fn notify_battle_turn_finished(&mut self, actor: usize, extra: bool) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(rules) = self.mechanics(owner)
            {
                rules.after_battle_turn(self, owner, actor, extra);
            }
        }
    }
}

pub(super) fn percentage_of(value: u32, bp: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(bp) / 10_000;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}
