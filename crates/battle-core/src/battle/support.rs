use crate::{
    Action, DamageKind,
    fixture::{Ability, Element},
};

use super::Battle;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AllyBuffKind {
    AttackPercent,
    CritDamage,
    Damage,
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
    pub max_hp_flat: u32,
    pub defense_bp: u32,
    pub damage_bp: u32,
    pub crit_rate_bp: u32,
    pub crit_damage_bp: u32,
}

#[derive(Clone, Copy)]
pub(super) struct OffenseContext {
    pub actor: usize,
    pub enemy: Option<usize>,
    pub element: Element,
    pub kind: DamageKind,
}

impl OffenseBuffs {
    fn add(&mut self, buff: Self) {
        self.attack_bp = self.attack_bp.saturating_add(buff.attack_bp);
        self.attack_flat = self.attack_flat.saturating_add(buff.attack_flat);
        self.max_hp_flat = self.max_hp_flat.saturating_add(buff.max_hp_flat);
        self.defense_bp = self.defense_bp.saturating_add(buff.defense_bp);
        self.damage_bp = self.damage_bp.saturating_add(buff.damage_bp);
        self.crit_rate_bp = self.crit_rate_bp.saturating_add(buff.crit_rate_bp);
        self.crit_damage_bp = self.crit_damage_bp.saturating_add(buff.crit_damage_bp);
    }
}

impl Battle<'_> {
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
        let state = &mut self.units[target];
        if state.hp == 0 || value == 0 || turns == 0 {
            return;
        }
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
        self.plan.units[actor]
            .stats
            .hp
            .saturating_add(self.ally_buff_total(actor, AllyBuffKind::MaxHpFlat))
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
        (
            u64::from(self.ally_buff_total(actor, AllyBuffKind::SpeedFlat)),
            u64::from(self.ally_buff_total(actor, AllyBuffKind::SpeedPercent)),
        )
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
            enemy,
            element,
            kind,
        } = context;
        let mut bonuses = OffenseBuffs::default();
        for buff in self.units[target]
            .ally_buffs
            .iter()
            .filter(|buff| buff.source == source)
        {
            let total = match buff.kind {
                AllyBuffKind::AttackPercent => &mut bonuses.attack_bp,
                AllyBuffKind::CritDamage => &mut bonuses.crit_damage_bp,
                AllyBuffKind::Damage => &mut bonuses.damage_bp,
                AllyBuffKind::DefensePercent => &mut bonuses.defense_bp,
                AllyBuffKind::MaxHpFlat => &mut bonuses.max_hp_flat,
                AllyBuffKind::SpeedFlat
                | AllyBuffKind::SpeedPercent
                | AllyBuffKind::EffectResistance => continue,
            };
            *total = total.saturating_add(buff.value);
        }
        if self.units[source].hp > 0
            && let Some(mechanics) = self.mechanics(source)
        {
            bonuses.attack_bp = bonuses
                .attack_bp
                .saturating_add(mechanics.ally_attack_bonus_bp(self, source, target));
            bonuses.attack_flat = mechanics.ally_attack_bonus_flat(self, source, target);
            bonuses.crit_rate_bp = mechanics.ally_crit_rate_bonus_bp(self, source, target);
            bonuses.crit_damage_bp = bonuses
                .crit_damage_bp
                .saturating_add(mechanics.ally_crit_damage_bonus_bp(self, source, target));
            bonuses.damage_bp = bonuses
                .damage_bp
                .saturating_add(mechanics.ally_damage_bonus_bp(self, source, target, element));
            if kind == DamageKind::DamageOverTime {
                bonuses.damage_bp = bonuses
                    .damage_bp
                    .saturating_add(mechanics.ally_dot_damage_bonus_bp(self, source, target));
            }
            if let Some(enemy) = enemy {
                bonuses.damage_bp = bonuses.damage_bp.saturating_add(
                    mechanics.ally_target_damage_bonus_bp(self, source, target, enemy, kind),
                );
            }
        }
        bonuses
    }

    pub(super) fn offense_buffs(
        &self,
        context: OffenseContext,
        allow: impl Fn(usize) -> bool,
    ) -> OffenseBuffs {
        if context.actor >= self.plan.ally_count {
            return OffenseBuffs::default();
        }
        let mut total = OffenseBuffs::default();
        for source in self.allies().filter(|&source| allow(source)) {
            total.add(self.offense_buffs_from(source, context));
        }
        total
    }

    pub(super) fn effective_crit_damage_bp(&self, actor: usize) -> u32 {
        self.plan.units[actor].stats.crit_damage_bp.saturating_add(
            self.offense_buffs(
                OffenseContext {
                    actor,
                    enemy: None,
                    element: Element::Physical,
                    kind: DamageKind::Direct,
                },
                |_| true,
            )
            .crit_damage_bp,
        )
    }

    pub(super) fn effective_attack(&self, actor: usize) -> u32 {
        let buffs = self.offense_buffs(
            OffenseContext {
                actor,
                enemy: None,
                element: Element::Physical,
                kind: DamageKind::Direct,
            },
            |_| true,
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
        let defense = self.defense_with_buff(actor, buffs.defense_bp);
        let attack = stats
            .attack
            .saturating_add(percentage_of(
                stats.base_attack,
                personal_bp.saturating_add(buffs.attack_bp),
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
        let personal_bp = self
            .mechanics(actor)
            .map_or(0, |rules| rules.defense_bonus_bp(self, actor));
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
        let energy = self.units[target]
            .energy_milli
            .saturating_add(amount_milli)
            .min(cap);
        self.set_energy(source, target, energy);
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
}

fn percentage_of(value: u32, bp: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(bp) / 10_000;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}
