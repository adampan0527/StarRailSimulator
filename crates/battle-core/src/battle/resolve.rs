use crate::{
    Action, DamageKind, EventKind, StatusKind,
    damage::{self, DamageTarget},
    fixture::{
        Ability, BREAK_DELAY_BP, Effect, Element, EnemyKit, EnemyTarget, GAUGE_FULL,
        KILL_ENERGY_MILLI, Target,
    },
};

use super::{
    Battle, DummyDamage, ExtraToughness, Kit, Status,
    combat_status::DebuffKind,
    periodic::{DotEvent, DotScale},
    support::{OffenseBuffs, OffenseContext, percentage_of},
};

struct Hit {
    element: Element,
    multiplier_bp: u32,
    toughness: u16,
    origin: Option<Action>,
    scale: DamageScale,
}

#[derive(Clone, Copy)]
struct DamageRecipient {
    unit: usize,
    share_bp: u16,
    reduction_bp: u32,
}

struct DamageView {
    target: DamageTarget,
    attack: u32,
    defense: u32,
    bonus: OffenseBuffs,
    critical: bool,
}

struct DamageEvent {
    kind: DamageKind,
    critical: DamageCritical,
    origin: Option<Action>,
    assisted_by: Option<usize>,
    from_break: bool,
    defense_ignore_bp: u16,
}

#[derive(Clone, Copy)]
enum DamageCritical {
    Never,
    Chance(u32),
    Guaranteed,
}

#[derive(Clone)]
pub(super) struct SuperBreakTarget {
    target: usize,
    element: Element,
    toughness_segments: Vec<SuperBreakSegment>,
}

#[derive(Clone, Copy)]
struct SuperBreakSegment {
    base: u16,
    personal_efficiency_bp: u32,
    full_toughness: bool,
}

impl SuperBreakSegment {
    fn scaled(self, team_bp: u64, off_element_bp: u16) -> u16 {
        let efficiency_bp = 10_000 + u64::from(self.personal_efficiency_bp) + team_bp;
        let weakness_bp = if self.full_toughness {
            10_000
        } else {
            off_element_bp
        };
        (u64::from(self.base) * efficiency_bp * u64::from(weakness_bp) / 100_000_000)
            .min(u64::from(u16::MAX)) as u16
    }
}

#[derive(Clone, Copy)]
enum DamageScale {
    Attack,
    AttackWithElation(u16),
    BreakScaledAttack {
        break_ratio_bp: u16,
        break_cap_bp: u16,
    },
    Defense,
    StackedAttack {
        multiplier_bp: u32,
        defense_ignore_bp: u16,
    },
    MaxHp,
    AttackAndDefense(u16),
    AttackAndMaxHp {
        max_hp_bp: u16,
        flat_base_bp: u64,
    },
    TargetHpCapped(u16),
    Elation {
        punchline: u32,
        factor_bp: u16,
    },
}

impl DamageScale {
    fn base_bp(
        self,
        attack: u32,
        defense: u32,
        source_hp: u32,
        target_hp: u32,
        multiplier_bp: u32,
    ) -> u64 {
        let ratio = u64::from(multiplier_bp);
        match self {
            Self::Attack | Self::AttackWithElation(_) | Self::BreakScaledAttack { .. } => {
                u64::from(attack) * ratio
            }
            Self::Defense => u64::from(defense) * ratio,
            Self::StackedAttack { multiplier_bp, .. } => {
                u64::from(attack) * u64::from(multiplier_bp)
            }
            Self::MaxHp => u64::from(source_hp) * ratio,
            Self::AttackAndDefense(defense_bp) => {
                u64::from(attack) * ratio + u64::from(defense) * u64::from(defense_bp)
            }
            Self::AttackAndMaxHp {
                max_hp_bp,
                flat_base_bp,
            } => {
                u64::from(attack) * ratio
                    + u64::from(source_hp) * u64::from(max_hp_bp)
                    + flat_base_bp
            }
            Self::TargetHpCapped(cap_bp) => {
                (u64::from(target_hp) * ratio).min(u64::from(attack) * u64::from(cap_bp))
            }
            Self::Elation { .. } => 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PeriodicHit {
    pub element: Element,
    pub damage: DotScale,
    pub stacks: u8,
    pub ratio_bp: u16,
    pub origin: Action,
}

pub(super) struct ElationHit {
    pub element: Element,
    pub multiplier_bp: u32,
    pub punchline: u32,
    pub toughness: u16,
    pub origin: Action,
    pub factor_bp: u16,
    pub assisted_by: Option<usize>,
}

pub(super) struct CounterAttack {
    pub element: Element,
    pub multiplier_bp: u16,
    pub defense_multiplier_bp: u16,
    pub toughness: u16,
    pub splash: Option<(u16, u16)>,
    pub energy_milli: u32,
}

#[derive(Clone)]
pub(super) struct FollowUpAttack {
    pub element: Element,
    pub multiplier_bp: u16,
    pub toughness: u16,
    pub energy_milli: u32,
    pub first_hit: Option<(u16, u16)>,
}

#[derive(Clone, Copy)]
pub(super) struct FollowUpBounces {
    pub hit_count: u8,
    pub multiplier_bp: u16,
    pub toughness_half_units: u16,
}

#[derive(Clone)]
pub(super) struct FollowUp {
    actor: usize,
    targets: FollowUpTargets,
    origin: Action,
    attack: FollowUpAttack,
}

#[derive(Clone)]
pub(super) enum PendingAttack {
    FollowUp(FollowUp),
    TriggeredSkill { actor: usize, target: usize },
}

#[derive(Clone, Copy)]
enum FollowUpTargets {
    All,
    Single(usize),
    SingleBlast {
        target: usize,
        adjacent: (u16, u16),
    },
    CounterBlast {
        target: Option<usize>,
        adjacent: (u16, u16),
        bounces: Option<FollowUpBounces>,
    },
    Random {
        hit_count: u8,
        adjacent_multiplier_bp: Option<u16>,
    },
    SingleOrRandom(Option<usize>),
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DamageTargetAura {
    pub(super) vulnerability_bp: u32,
    // The damage formula combines DEF reduction and ignore, and RES reduction and penetration.
    pub(super) defense_down_bp: u32,
    pub(super) resistance_down_bp: u32,
}

impl Battle<'_> {
    fn is_ultimate_damage(&self, actor: usize, event: &DamageEvent) -> bool {
        match (event.kind, &self.plan.units[actor].kit) {
            (DamageKind::Direct, Kit::Official(kit)) => event
                .origin
                .is_some_and(|action| kit.mechanics.primary_attack_counts_as_ultimate(action)),
            (DamageKind::Direct, _) => event.origin.is_some_and(Action::is_ultimate),
            (DamageKind::FollowUp, Kit::Official(kit)) => {
                kit.mechanics.follow_up_counts_as_ultimate(event.origin)
            }
            _ => false,
        }
    }

    pub(super) fn is_follow_up_damage(
        &self,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        match (kind, action, &self.plan.units[actor].kit) {
            (DamageKind::FollowUp, _, _) => true,
            (DamageKind::Direct, Some(action), Kit::Official(kit)) => {
                kit.mechanics.primary_attack_counts_as_follow_up(action)
            }
            _ => false,
        }
    }

    pub(super) fn resolve_ability(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        for effect in ability.effects {
            if matches!(effect, Effect::RandomHits { .. }) {
                self.apply_effect(actor, actor, *effect);
            } else if let Some(target) = target {
                self.apply_effect(actor, target, *effect);
            } else {
                let targets = match ability.target {
                    Target::SelfOnly => actor..actor + 1,
                    Target::AllAllies => {
                        for target in self.party_units() {
                            if self.is_targetable(target) {
                                self.apply_effect(actor, target, *effect);
                            }
                        }
                        continue;
                    }
                    Target::AllEnemies => self.enemies(),
                    Target::Ally | Target::OtherAlly | Target::Enemy => continue,
                };
                for target in targets {
                    self.apply_effect(actor, target, *effect);
                }
            }
        }
    }

    fn apply_effect(&mut self, actor: usize, target: usize, effect: Effect) {
        if self.units[target].hp == 0
            && !matches!(
                effect,
                Effect::BlastAdjacent { .. }
                    | Effect::MaxHpBlastAdjacent { .. }
                    | Effect::RandomHits { .. }
            )
        {
            return;
        }
        match effect {
            Effect::Hit {
                element,
                multiplier_bp,
                toughness,
            } => self.attack_hit(actor, target, element, multiplier_bp, toughness),
            Effect::WideHit {
                element,
                multiplier_bp,
                toughness,
            } => self.attack_hit_scaled(
                actor,
                target,
                element,
                multiplier_bp,
                toughness,
                DamageScale::Attack,
            ),
            Effect::MaxHpHit {
                element,
                multiplier_bp,
                toughness,
            } => self.attack_hit_scaled(
                actor,
                target,
                element,
                u32::from(multiplier_bp),
                toughness,
                DamageScale::MaxHp,
            ),
            Effect::BlastAdjacent {
                element,
                multiplier_bp,
                toughness,
            } => {
                let multiplier_bp = self.mechanics(actor).map_or(multiplier_bp, |mechanics| {
                    mechanics.blast_multiplier_bp(self, actor, self.action, multiplier_bp)
                });
                for neighbor in self.adjacent_enemies(target) {
                    self.attack_hit(actor, neighbor, element, multiplier_bp, toughness);
                }
            }
            Effect::MaxHpBlastAdjacent {
                element,
                multiplier_bp,
                toughness,
            } => {
                for neighbor in self.adjacent_enemies(target) {
                    self.attack_hit_scaled(
                        actor,
                        neighbor,
                        element,
                        u32::from(multiplier_bp),
                        toughness,
                        DamageScale::MaxHp,
                    );
                }
            }
            Effect::RandomHits {
                element,
                multiplier_bp,
                toughness,
                count,
            } => {
                let count = self.mechanics(actor).map_or(count, |mechanics| {
                    mechanics.random_hit_count(self, actor, count)
                });
                for _ in 0..count {
                    let Some(selected) = self.random_living_enemy() else {
                        break;
                    };
                    self.attack_hit(actor, selected, element, multiplier_bp, toughness);
                }
            }
            Effect::RepeatingSingleHits {
                element,
                multiplier_bp,
                toughness,
                initial_hits,
                extra_hits,
                extra_chance_bp,
            } => {
                for hit in 0..initial_hits + extra_hits {
                    if !self.is_targetable(target)
                        || (hit >= initial_hits && !self.roll_fixed(extra_chance_bp))
                    {
                        break;
                    }
                    self.attack_hit(actor, target, element, multiplier_bp, toughness);
                }
            }
            Effect::DirectedAttack { .. }
            | Effect::DirectedRadiusAttack { .. }
            | Effect::DirectedAreaAttack => {
                if let Some(mechanics) = self.mechanics(actor) {
                    mechanics.resolve_special_attack(self, actor, target);
                }
            }
            Effect::Shield { amount, turns } => {
                self.apply_status(actor, target, StatusKind::Shield, amount, turns)
            }
            Effect::Vulnerability { bp, turns } => {
                self.apply_status(actor, target, StatusKind::Vulnerability, bp, turns)
            }
            Effect::SpeedUp { bp, turns } => {
                self.apply_status(actor, target, StatusKind::SpeedUp, bp, turns)
            }
            Effect::Heal(amount) => self.heal_unit(actor, target, amount),
            Effect::Advance(bp) => {
                let gauge = self.units[target]
                    .gauge_quanta
                    .saturating_sub(gauge_for(bp));
                self.shift_gauge(actor, target, gauge, gauge == 0);
            }
            Effect::ImmediateTurn => self.shift_gauge(actor, target, 0, true),
        }
    }

    pub(super) fn attack_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
    ) {
        self.attack_hit_scaled(
            actor,
            target,
            element,
            u32::from(multiplier_bp),
            toughness,
            DamageScale::Attack,
        );
    }

    pub(super) fn memosprite_hit(
        &mut self,
        sprite: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
        origin: Action,
    ) {
        if !self.is_targetable(sprite) {
            return;
        }
        self.strike(
            sprite,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
        );
    }

    pub(super) fn memosprite_hp_hit(
        &mut self,
        sprite: usize,
        target: usize,
        element: Element,
        multiplier_bp: u32,
        toughness: u16,
    ) {
        if self.is_targetable(sprite) {
            self.strike(
                sprite,
                target,
                DamageKind::Direct,
                Hit {
                    element,
                    multiplier_bp,
                    toughness,
                    origin: Some(Action::Skill),
                    scale: DamageScale::MaxHp,
                },
            );
        }
    }

    pub(super) fn attack_hit_with_elation(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: (u16, u16),
        toughness: u16,
    ) {
        self.attack_hit_scaled(
            actor,
            target,
            element,
            u32::from(multiplier_bp.0),
            toughness,
            DamageScale::AttackWithElation(multiplier_bp.1),
        );
    }

    pub(super) fn attack_hit_with_break_effect(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
        break_scale: (u16, u16),
    ) {
        self.attack_hit_scaled(
            actor,
            target,
            element,
            u32::from(multiplier_bp),
            toughness,
            DamageScale::BreakScaledAttack {
                break_ratio_bp: break_scale.0,
                break_cap_bp: break_scale.1,
            },
        );
    }

    fn attack_hit_scaled(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u32,
        toughness: u16,
        scale: DamageScale,
    ) {
        let hp_before = self.units[target].hp;
        if hp_before == 0 {
            return;
        }
        let mechanics = self.action.zip(self.mechanics(actor));
        if let Some((action, mechanics)) = &mechanics {
            mechanics.before_attack_hit(self, actor, target, *action);
        }
        let critical = self.strike(
            actor,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp,
                toughness,
                origin: self.action,
                scale,
            },
        );
        if let Some((action, mechanics)) = mechanics {
            if critical {
                mechanics.after_critical_hit(self, actor, target, action);
            }
            mechanics.after_attack_hit(self, actor, target, action, hp_before);
        }
    }

    pub(super) fn adjacent_enemies(&self, target: usize) -> impl Iterator<Item = usize> + use<> {
        let enemies = self.enemies();
        [target.checked_sub(1), target.checked_add(1)]
            .map(|neighbor| {
                neighbor.filter(|&unit| enemies.contains(&unit) && self.units[unit].hp > 0)
            })
            .into_iter()
            .flatten()
    }

    pub(super) fn attack_hit_with_max_hp(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multipliers_bp: (u16, u16),
        flat_base_bp: u64,
        toughness: u16,
    ) {
        let (attack_bp, max_hp_bp) = multipliers_bp;
        self.attack_hit_scaled(
            actor,
            target,
            element,
            u32::from(attack_bp),
            toughness,
            DamageScale::AttackAndMaxHp {
                max_hp_bp,
                flat_base_bp,
            },
        );
    }

    pub(super) fn heal_unit(&mut self, actor: usize, target: usize, amount: u32) {
        let amount = self.incoming_healing_amount(target, amount);
        let restored = self.restore_hp_unmodified(actor, target, amount);
        if restored > 0
            && let Some(rules) = self.mechanics(target)
        {
            rules.after_healing(self, target, restored);
        }
        if amount > 0 && self.is_targetable(target) && self.is_party_unit(target) {
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(rules) = self.mechanics(owner)
                {
                    rules.on_party_healed(self, owner, target, amount);
                }
            }
        }
    }

    /// Effective incoming heal before capping to missing HP.
    pub(super) fn incoming_healing_amount(&self, target: usize, amount: u32) -> u32 {
        let personal_bp = self
            .mechanics(target)
            .map_or(0, |rules| rules.incoming_healing_bonus_bp(self, target));
        let bonus_bp = if self.is_party_unit(target) {
            self.allies()
                .filter(|&owner| self.is_targetable(owner))
                .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
                .fold(personal_bp, |total, (owner, rules)| {
                    total.saturating_add(rules.ally_incoming_healing_bonus_bp(self, owner))
                })
        } else {
            personal_bp
        };
        amount.saturating_add(percentage_of(amount, bonus_bp))
    }

    /// Used when an effect sets current HP independently of incoming healing modifiers.
    pub(super) fn restore_hp_unmodified(
        &mut self,
        actor: usize,
        target: usize,
        amount: u32,
    ) -> u32 {
        // A zero-HP target can be rescued, but a departed unit is off the field.
        if self.units[target].departed {
            return 0;
        }
        let (hp, max_hp) = self.hp(target);
        let restored = amount.min(max_hp - hp);
        self.units[target].hp += restored;
        self.emit(
            EventKind::Heal { amount: restored },
            Some(actor),
            Some(target),
        );
        restored
    }

    pub(super) fn spend_hp(&mut self, actor: usize, amount: u32, minimum_remaining: u32) {
        self.spend_ally_hp(actor, actor, amount, minimum_remaining);
    }

    /// Consumes the target's HP directly and returns the actual loss.
    pub(super) fn spend_ally_hp(
        &mut self,
        source: usize,
        target: usize,
        amount: u32,
        minimum_remaining: u32,
    ) -> u32 {
        if minimum_remaining == 1 {
            self.protect_from_fatal_hp_overflow(target, amount);
        }
        let hp = &mut self.units[target].hp;
        let amount = amount.min(hp.saturating_sub(minimum_remaining));
        *hp -= amount;
        if amount > 0 {
            self.emit(EventKind::HpSpent { amount }, Some(source), Some(target));
            if let Some(rules) = self.mechanics(target) {
                rules.after_hp_spent(self, target, amount);
            }
            self.record_team_hp_loss(target, amount);
            self.notify_ally_health_event(target, None);
            self.notify_ally_hp_loss(target);
        }
        amount
    }

    fn notify_ally_health_event(&mut self, target: usize, attack_index: Option<u32>) {
        if !self.allies().contains(&target) {
            return;
        }
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_ally_health_event(self, owner, target, attack_index);
            }
        }
    }

    fn record_team_hp_loss(&mut self, target: usize, amount: u32) {
        if !self.is_party_unit(target) || amount == 0 {
            return;
        }
        for owner in self.allies() {
            // Record a lethal loss before rescue or removal of its owner.
            if (owner == target || self.units[owner].hp > 0)
                && let Some(mechanics) = self.mechanics(owner)
            {
                if self.allies().contains(&target) {
                    mechanics.record_team_hp_loss(self, owner, amount);
                }
                mechanics.on_party_hp_lost(self, owner, target, amount);
            }
        }
    }

    fn protect_from_fatal_hp_overflow(&mut self, target: usize, incoming: u32) -> bool {
        let hp = self.units[target].hp;
        if hp == 0 || incoming < hp || !self.is_party_unit(target) || !self.can_enemy_target(target)
        {
            return false;
        }
        let overflow = incoming - (hp - 1);
        self.allies().any(|owner| {
            self.is_targetable(owner)
                && self.mechanics(owner).is_some_and(|mechanics| {
                    mechanics.absorb_fatal_hp_overflow(self, owner, target, overflow)
                })
        })
    }

    fn notify_ally_hp_loss(&mut self, target: usize) {
        if target >= self.plan.ally_count || self.units[target].hp == 0 {
            return;
        }
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.after_ally_hp_loss(self, owner, target);
            }
        }
    }

    pub(super) fn apply_attack_bonus(&mut self, actor: usize, magnitude_bp: u32, turns: u32) {
        let state = &mut self.units[actor];
        state.attack_bonus = Some((magnitude_bp, state.normal_turns + turns));
    }

    pub(super) fn add_crit_rate_stacks(&mut self, actor: usize, stack_bp: u32, max_stacks: u8) {
        let rate = &mut self.units[actor].crit_rate_bonus_bp;
        *rate = (*rate + stack_bp).min(stack_bp * u32::from(max_stacks));
    }

    pub(super) fn resists_debuff(&mut self, target: usize) -> bool {
        self.use_survival_response_guard(target)
            || self
                .mechanics(target)
                .is_some_and(|mechanics| mechanics.prevent_debuff(self, target))
    }

    pub(super) fn resists_enemy_control(&mut self, actor: usize, target: usize) -> bool {
        if !self.enemies().contains(&actor) || !self.is_party_unit(target) {
            return false;
        }
        if let Some(owner) = self.memosprite_owner(target)
            && self
                .mechanics(owner)
                .is_some_and(|rules| rules.memosprite_immune_to_enemy_control(self, owner))
        {
            return true;
        }
        if self
            .mechanics(target)
            .is_some_and(|rules| rules.immune_to_enemy_control(self, target))
        {
            return true;
        }
        self.allies().any(|owner| {
            self.is_targetable(owner)
                && self.mechanics(owner).is_some_and(|mechanics| {
                    mechanics.block_enemy_control(self, owner, actor, target)
                })
        })
    }

    pub(super) fn roll_effect(&mut self, actor: usize, target: usize, base_chance_bp: u32) -> bool {
        self.roll_status(actor, target, base_chance_bp, false)
    }

    pub(super) fn effective_effect_hit_rate_bp(&self, actor: usize) -> u32 {
        let aura = self
            .allies()
            .filter(|&ally| actor < self.plan.ally_count && self.units[ally].hp > 0)
            .filter_map(|ally| self.mechanics(ally))
            .map(|mechanics| mechanics.effect_hit_rate_aura_bp())
            .fold(0_u32, u32::saturating_add);
        let personal = self.mechanics(actor).map_or(0, |mechanics| {
            mechanics.effect_hit_rate_bonus_bp(self, actor)
        });
        self.plan.units[actor]
            .stats
            .effect_hit_rate_bp
            .saturating_add(aura)
            .saturating_add(personal)
    }

    pub(super) fn roll_status(
        &mut self,
        actor: usize,
        target: usize,
        base_chance_bp: u32,
        control: bool,
    ) -> bool {
        if control && base_chance_bp > 0 && self.resists_enemy_control(actor, target) {
            return false;
        }
        let hit_rate = self.effective_effect_hit_rate_bp(actor);
        let resist_aura_bp = self
            .allies()
            .filter(|&ally| target < self.plan.ally_count && self.units[ally].hp > 0)
            .filter_map(|ally| self.mechanics(ally).map(|mechanics| (ally, mechanics)))
            .map(|(ally, mechanics)| mechanics.effect_resistance_aura_bp(self, ally, target))
            .fold(0_u32, u32::saturating_add);
        let resistance_down = self.units[target]
            .debuffs
            .iter()
            .filter_map(|status| match status.kind {
                DebuffKind::EffectResistanceDown => Some(status.bp),
                DebuffKind::Epiphany {
                    effect_resistance_down_bp,
                    ..
                } => Some(u32::from(effect_resistance_down_bp)),
                _ => None,
            })
            .fold(0_u32, u32::saturating_add);
        let personal_resistance = self.mechanics(target).map_or(0, |mechanics| {
            mechanics.effect_resistance_bonus_bp(self, target)
        });
        let resistance = self.plan.units[target]
            .stats
            .effect_resistance_bp
            .saturating_add(resist_aura_bp)
            .saturating_add(personal_resistance)
            .saturating_add(self.support_effect_resistance(target))
            .saturating_sub(resistance_down);
        let control_resistance = if control {
            self.mechanics(target)
                .map_or(0, |mechanics| mechanics.control_resistance_bp())
        } else {
            0
        };
        let chance = u128::from(base_chance_bp)
            * (10_000 + u128::from(hit_rate))
            * u128::from(10_000_u32.saturating_sub(resistance))
            * u128::from(10_000_u32.saturating_sub(control_resistance))
            / 10_000_u128.pow(3);
        self.roll_fixed(u32::try_from(chance).unwrap_or(u32::MAX))
    }

    pub(super) fn roll_fixed(&mut self, chance: u32) -> bool {
        let chance = u64::from(chance);
        chance >= 10_000 || (chance > 0 && self.rng.below(10_000) < chance)
    }

    pub(super) fn apply_speed_up(&mut self, actor: usize, target: usize, bp: u32, turns: u32) {
        self.apply_status(actor, target, StatusKind::SpeedUp, bp, turns);
    }

    pub(super) fn advance_immediate(&mut self, actor: usize) {
        self.advance_ally_immediate(actor, actor);
    }

    pub(super) fn advance_ally_immediate(&mut self, actor: usize, target: usize) {
        self.shift_gauge(actor, target, 0, true);
    }

    pub(super) fn advance(&mut self, actor: usize, bp: u16) {
        self.advance_ally(actor, actor, bp);
    }

    pub(super) fn advance_ally(&mut self, source: usize, target: usize, bp: u16) {
        let remaining = self.gauge_quanta(target).saturating_sub(gauge_for(bp));
        self.shift_gauge(source, target, remaining, remaining == 0);
    }

    pub(super) fn random_index(&mut self, contenders: usize) -> usize {
        if contenders < 2 {
            0
        } else {
            self.rng.below(contenders as u64) as usize
        }
    }

    pub(super) fn random_living_enemy(&mut self) -> Option<usize> {
        let live: Vec<_> = self
            .enemies()
            .filter(|&unit| self.units[unit].hp > 0)
            .collect();
        let index = self.random_index(live.len());
        live.get(index).copied()
    }

    pub(super) fn follow_up(
        &mut self,
        actor: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
        origin: Action,
        energy_milli: u32,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::All,
                origin,
                attack: FollowUpAttack {
                    element,
                    multiplier_bp,
                    toughness,
                    energy_milli,
                    first_hit: None,
                },
            }));
    }

    pub(super) fn follow_up_single(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::Single(target),
                origin,
                attack,
            }));
    }

    /// Hit the original target and its neighbors while the primary target is still living.
    pub(super) fn follow_up_blast(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        attack: FollowUpAttack,
        adjacent: (u16, u16),
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::SingleBlast { target, adjacent },
                origin,
                attack,
            }));
    }

    /// Each hit draws a living enemy and grants the specified energy for that hit.
    pub(super) fn follow_up_random(
        &mut self,
        actor: usize,
        origin: Action,
        hit_count: u8,
        attack: FollowUpAttack,
    ) {
        self.follow_up_random_blast(actor, origin, hit_count, None, attack);
    }

    /// Each bounce resolves primary damage and then optional adjacent damage with no toughness.
    pub(super) fn follow_up_random_blast(
        &mut self,
        actor: usize,
        origin: Action,
        hit_count: u8,
        adjacent_multiplier_bp: Option<u16>,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::Random {
                    hit_count,
                    adjacent_multiplier_bp,
                },
                origin,
                attack,
            }));
    }

    pub(super) fn follow_up_single_or_random(
        &mut self,
        actor: usize,
        target: Option<usize>,
        origin: Action,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::SingleOrRandom(target),
                origin,
                attack,
            }));
    }

    pub(super) fn queue_triggered_skill(&mut self, actor: usize, target: usize) {
        self.pending_follow_ups
            .push_back(PendingAttack::TriggeredSkill { actor, target });
    }

    pub(super) fn queue_counter_blast(
        &mut self,
        actor: usize,
        target: Option<usize>,
        origin: Action,
        attack: FollowUpAttack,
        adjacent: (u16, u16),
        bounces: Option<FollowUpBounces>,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::CounterBlast {
                    target,
                    adjacent,
                    bounces,
                },
                origin,
                attack,
            }));
    }

    pub(super) fn perform_triggered_skill(
        &mut self,
        actor: usize,
        target: usize,
        ability: Ability<'_>,
    ) {
        let previous_action = self.action.replace(Action::Skill);
        let previous_target = self.action_target.replace(target);
        let previous_follow_up = std::mem::take(&mut self.resolving_follow_up);
        let previous_attack_targets = std::mem::take(&mut self.attack_targets);
        let previous_attack_damage = self.attack_damage.take();
        let previous_attack_spent_points = self.attack_spent_skill_points;
        let previous_super_break_targets = std::mem::take(&mut self.super_break_targets);
        let parent_debuff_targets = self.begin_debuff_action();
        self.begin_attack();
        self.emit(EventKind::TriggeredSkill, Some(actor), Some(target));
        self.notify_ally_action_started(actor, Some(target), ability);
        self.resolve_ability(actor, Some(target), ability);
        self.notify_attack(actor, Action::Skill, Some(target));
        self.notify_ally_action_finished(actor, Action::Skill);
        self.finish_debuff_action(actor, parent_debuff_targets);
        self.attack_targets = previous_attack_targets;
        self.attack_damage = previous_attack_damage;
        self.attack_spent_skill_points = previous_attack_spent_points;
        self.super_break_targets = previous_super_break_targets;
        self.resolving_follow_up = previous_follow_up;
        self.action_target = previous_target;
        self.action = previous_action;
    }

    pub(super) fn resolve_follow_ups(&mut self) {
        while let Some(attack) = self.pending_follow_ups.pop_front() {
            let follow_up = match attack {
                PendingAttack::FollowUp(follow_up) => follow_up,
                PendingAttack::TriggeredSkill { actor, target } => {
                    if self.is_targetable(actor)
                        && let Some(rules) = self.mechanics(actor)
                    {
                        rules.on_triggered_skill(self, actor, target);
                    }
                    continue;
                }
            };
            let FollowUp {
                actor,
                targets,
                origin,
                attack,
            } = follow_up;
            if self.units[actor].hp == 0 {
                continue;
            }
            let single = match targets {
                FollowUpTargets::All | FollowUpTargets::Random { .. } => None,
                FollowUpTargets::Single(unit)
                | FollowUpTargets::SingleBlast { target: unit, .. } => Some(unit),
                FollowUpTargets::SingleOrRandom(target)
                | FollowUpTargets::CounterBlast { target, .. } => {
                    let Some(unit) = target
                        .filter(|&unit| self.is_targetable(unit))
                        .or_else(|| self.random_living_enemy())
                    else {
                        continue;
                    };
                    Some(unit)
                }
            };
            let candidates = single.map_or_else(|| self.enemies(), |unit| unit..unit + 1);
            if !candidates.clone().any(|unit| self.is_targetable(unit)) {
                continue;
            }
            let was_follow_up = std::mem::replace(&mut self.resolving_follow_up, true);
            let parent_debuff_targets = self.begin_debuff_action();
            self.begin_attack();
            let mechanics = self.mechanics(actor);
            self.emit(EventKind::FollowUp, Some(actor), single);
            if let Some(mechanics) = &mechanics {
                mechanics.before_follow_up(self, actor);
            }
            if let FollowUpTargets::Random {
                hit_count,
                adjacent_multiplier_bp,
            } = targets
            {
                for _ in 0..hit_count {
                    let Some(target) = self.random_living_enemy() else {
                        break;
                    };
                    self.follow_up_hit(
                        actor,
                        target,
                        origin,
                        attack.element,
                        attack.multiplier_bp,
                        attack.toughness,
                    );
                    if let Some(multiplier_bp) = adjacent_multiplier_bp {
                        for neighbor in self.adjacent_enemies(target) {
                            self.follow_up_hit(
                                actor,
                                neighbor,
                                origin,
                                attack.element,
                                multiplier_bp,
                                0,
                            );
                        }
                    }
                    if let Some(mechanics) = &mechanics {
                        mechanics.after_follow_up_hit(self, actor, target, origin);
                    }
                    self.gain_energy(actor, actor, attack.energy_milli);
                }
            } else {
                let (first_bp, first_toughness) = attack.first_hit.unwrap_or_default();
                let remaining = (
                    attack.multiplier_bp.saturating_sub(first_bp),
                    attack.toughness.saturating_sub(first_toughness),
                );
                for target in candidates {
                    if !self.is_targetable(target) {
                        continue;
                    }
                    for (multiplier_bp, toughness) in
                        attack.first_hit.into_iter().chain([remaining])
                    {
                        self.follow_up_hit(
                            actor,
                            target,
                            origin,
                            attack.element,
                            multiplier_bp,
                            toughness,
                        );
                    }
                    if let FollowUpTargets::SingleBlast { adjacent, .. }
                    | FollowUpTargets::CounterBlast { adjacent, .. } = targets
                    {
                        for neighbor in self.adjacent_enemies(target) {
                            self.follow_up_hit(
                                actor,
                                neighbor,
                                origin,
                                attack.element,
                                adjacent.0,
                                adjacent.1,
                            );
                        }
                    }
                    if let Some(mechanics) = &mechanics {
                        mechanics.after_follow_up_hit(self, actor, target, origin);
                    }
                }
                if let FollowUpTargets::CounterBlast {
                    bounces: Some(bounces),
                    ..
                } = targets
                {
                    // Carry fractional toughness between hits on each enemy within this attack.
                    let mut fractional_targets = Vec::new();
                    for _ in 0..bounces.hit_count {
                        let Some(target) = self.random_living_enemy() else {
                            break;
                        };
                        let mut toughness = bounces.toughness_half_units / 2;
                        if bounces.toughness_half_units % 2 != 0 {
                            if let Some(slot) =
                                fractional_targets.iter().position(|&unit| unit == target)
                            {
                                fractional_targets.swap_remove(slot);
                                toughness += 1;
                            } else {
                                fractional_targets.push(target);
                            }
                        }
                        self.follow_up_hit(
                            actor,
                            target,
                            origin,
                            attack.element,
                            bounces.multiplier_bp,
                            toughness,
                        );
                        if let Some(mechanics) = &mechanics {
                            mechanics.after_follow_up_hit(self, actor, target, origin);
                        }
                    }
                }
                self.gain_energy(actor, actor, attack.energy_milli);
            }
            if let Some(mechanics) = mechanics {
                mechanics.after_follow_up(self, actor, origin);
            }
            self.notify_attack(actor, origin, single);
            self.finish_debuff_action(actor, parent_debuff_targets);
            self.resolving_follow_up = was_follow_up;
        }
    }

    pub(super) fn follow_up_hit(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
    ) {
        self.strike(
            actor,
            target,
            DamageKind::FollowUp,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
        );
    }

    pub(super) fn notify_attack(
        &mut self,
        actor: usize,
        origin: Action,
        primary_target: Option<usize>,
    ) {
        let landed = !self.attack_targets.is_empty();
        let super_break_targets = std::mem::take(&mut self.super_break_targets);
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_attack_finished(self, owner, actor, origin);
                if landed {
                    mechanics.on_ally_attack(self, owner, actor, origin, primary_target);
                }
            }
        }
        self.resolve_super_breaks(actor, origin, &super_break_targets);
        if let Some(damage) = self.attack_damage.take()
            && landed
        {
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(mechanics) = self.mechanics(owner)
                    && mechanics.tracks_attack_damage()
                {
                    mechanics.after_ally_attack_damage(self, owner, origin, damage);
                }
            }
        }
        let crossed_half = std::mem::take(&mut self.crossed_half);
        if crossed_half.is_empty() {
            return;
        }
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.after_attack(self, owner, actor, &crossed_half, origin);
            }
        }
    }

    pub(super) fn resolve_enemy(&mut self, actor: usize, kit: &EnemyKit) {
        let taunt = self.allies().find(|&owner| {
            self.is_targetable(owner)
                && self
                    .mechanics(owner)
                    .is_some_and(|rules| rules.taunts_enemy(self, owner, actor))
        });
        let target = taunt.or_else(|| match kit.target {
            EnemyTarget::FirstLiving => {
                self.party_units().find(|&unit| self.can_enemy_target(unit))
            }
            EnemyTarget::RandomLiving => self.random_enemy_target(),
        });
        if let Some(target) = target {
            let was_shielded = self.shield_amount(target) > 0;
            self.notify_active_ability(actor);
            self.begin_attack();
            self.emit(EventKind::EnemyAttack, Some(actor), Some(target));
            self.enemy_action = Some(actor);
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.before_enemy_attack_seen(self, owner, target);
                }
            }
            self.hit(actor, target, Element::Physical, 10_000, 0);
            self.finish_enemy_action();
            let energy_recipient = self.memosprite_owner(target).unwrap_or(target);
            if self.units[energy_recipient].hp > 0 {
                self.gain_energy(actor, energy_recipient, kit.hit_energy_milli);
            }
            if self.units[target].hp > 0
                && let Some(mechanics) = self.mechanics(target)
            {
                mechanics.after_enemy_attack(self, target);
            }
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.after_enemy_attack_seen(self, owner, actor, target, was_shielded);
                }
            }
            self.resolve_follow_ups();
        }
    }

    pub(super) fn enemy_targeting_weight(&self, unit: usize) -> u64 {
        let weight =
            u32::from(self.plan.units[unit].stats.base_aggro) + self.lynx_aggro_bonus(unit);
        let ratio = self
            .mechanics(unit)
            .map_or(10_000, |rules| rules.aggro_multiplier_bp(self, unit));
        (u64::from(weight) * u64::from(ratio)).max(1)
    }

    pub(super) fn finish_enemy_action(&mut self) {
        self.enemy_action = None;
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.after_enemy_action(self, owner);
            }
        }
    }

    fn random_enemy_target(&mut self) -> Option<usize> {
        let mut weighted = self
            .party_units()
            .filter(|&unit| self.can_enemy_target(unit))
            .map(|unit| (unit, self.enemy_targeting_weight(unit)))
            .collect::<Vec<_>>();
        let &(_, first_weight) = weighted.first()?;
        if weighted.iter().all(|&(_, weight)| weight == first_weight) {
            let slot = self.rng.below(weighted.len() as u64) as usize;
            return Some(weighted[slot].0);
        }
        // Keep the original random range for fixtures with integer aggro weights.
        if weighted.iter().all(|&(_, weight)| weight % 10_000 == 0) {
            for (_, weight) in &mut weighted {
                *weight /= 10_000;
            }
        }
        let total = weighted.iter().map(|&(_, weight)| weight).sum();
        let mut roll = self.rng.below(total);
        weighted.into_iter().find_map(|(unit, weight)| {
            if roll < weight {
                return Some(unit);
            }
            roll -= weight;
            None
        })
    }

    fn can_enemy_target(&self, unit: usize) -> bool {
        self.is_targetable(unit)
            && !self
                .memosprite_owner(unit)
                .and_then(|owner| self.mechanics(owner))
                .is_some_and(|rules| rules.memosprite_is_backup())
    }

    fn target_auras_from(
        &self,
        owner: usize,
        actor: usize,
        target: usize,
        element: Element,
        event: &DamageEvent,
    ) -> DamageTargetAura {
        if self.units[owner].hp == 0 || !self.enemies().contains(&target) {
            return DamageTargetAura::default();
        }
        self.mechanics(owner)
            .map_or(DamageTargetAura::default(), |mechanics| {
                let mut aura = mechanics.damage_target_aura(self, owner, actor);
                if self.is_ultimate_damage(actor, event) {
                    aura.vulnerability_bp = aura
                        .vulnerability_bp
                        .saturating_add(mechanics.ultimate_vulnerability_bp(self, owner, target));
                }
                if self.is_follow_up_damage(actor, event.origin, event.kind) {
                    aura.vulnerability_bp = aura
                        .vulnerability_bp
                        .saturating_add(mechanics.follow_up_vulnerability_bp(self, owner, target));
                }
                aura.resistance_down_bp = aura
                    .resistance_down_bp
                    .saturating_add(mechanics.all_resistance_down_bp(self, owner, target))
                    .saturating_add(
                        mechanics.elemental_resistance_down_bp(self, owner, target, element),
                    );
                aura.defense_down_bp = aura
                    .defense_down_bp
                    .saturating_add(mechanics.target_defense_down_bp(self, owner, target));
                aura
            })
    }

    fn damage_target(
        &self,
        actor: usize,
        target: usize,
        element: Element,
        event: &DamageEvent,
        allow_source: impl Fn(usize) -> bool,
    ) -> DamageTarget {
        let plan = &self.plan.units[target];
        let state = &self.units[target];
        let sum_matching = |matches_kind: &dyn Fn(DebuffKind) -> bool| {
            state
                .debuffs
                .iter()
                .filter(|status| matches_kind(status.kind) && allow_source(status.source))
                .map(|status| status.bp)
                .fold(0_u32, u32::saturating_add)
        };
        let sum = |kind| sum_matching(&|other| other == kind);
        let legacy_vulnerability = state.statuses[StatusKind::Vulnerability.index()]
            .filter(|status| allow_source(status.source))
            .map_or(0, |status| status.magnitude);
        let mut vulnerability = legacy_vulnerability
            .saturating_add(sum(DebuffKind::Vulnerability))
            .saturating_add(sum(DebuffKind::SeamStitch))
            .saturating_add(sum(DebuffKind::WoesWhisper));
        if event.kind != DamageKind::DamageOverTime
            && let Some(standoff) = state
                .debuffs
                .iter()
                .find(|status| status.source == actor && status.kind == DebuffKind::Standoff)
            && allow_source(actor)
        {
            vulnerability = vulnerability.saturating_add(standoff.bp);
        }
        if let Some(rules) = self.mechanics(target) {
            vulnerability = vulnerability
                .saturating_add(rules.incoming_vulnerability_bp(self, target, actor, event.kind));
        }
        vulnerability = vulnerability.saturating_add(
            state
                .debuffs
                .iter()
                .filter(|status| {
                    allow_source(status.source) && self.epiphany_applies(status.kind, target)
                })
                .map(|status| status.bp)
                .fold(0_u32, u32::saturating_add),
        );
        if self.is_ultimate_damage(actor, event) {
            vulnerability = vulnerability.saturating_add(sum(DebuffKind::UltimateVulnerability));
        }
        if event.kind == DamageKind::DamageOverTime {
            vulnerability = vulnerability.saturating_add(sum(DebuffKind::DotVulnerability));
        }
        if event.kind.is_break() {
            vulnerability =
                vulnerability.saturating_add(sum_matching(&DebuffKind::is_break_vulnerability));
            if let Some(rules) = self.mechanics(actor) {
                vulnerability =
                    vulnerability.saturating_add(rules.own_break_vulnerability_bp(self, actor));
            }
        }
        let mut field_defense_down = 0_u32;
        let mut field_resistance_down = 0_u32;
        for owner in self.allies().filter(|&owner| allow_source(owner)) {
            let aura = self.target_auras_from(owner, actor, target, element, event);
            vulnerability = vulnerability.saturating_add(aura.vulnerability_bp);
            field_defense_down = field_defense_down.saturating_add(aura.defense_down_bp);
            field_resistance_down = field_resistance_down.saturating_add(aura.resistance_down_bp);
        }
        let mut weightless_defense_down = 0_u32;
        let mut weightless_resistance_down = 0_u32;
        for status in state
            .debuffs
            .iter()
            .filter(|status| allow_source(status.source))
        {
            if let DebuffKind::Weightless {
                defense_down_bp,
                resistance_down_bp,
            } = status.kind
            {
                weightless_defense_down =
                    weightless_defense_down.saturating_add(u32::from(defense_down_bp));
                weightless_resistance_down =
                    weightless_resistance_down.saturating_add(u32::from(resistance_down_bp));
            }
        }
        let (personal_penetration, personal_defense_ignore, outgoing_damage_factor_bp) =
            self.mechanics(actor).map_or((0, 0, 10_000), |mechanics| {
                let (ultimate_penetration, ultimate_defense_ignore) =
                    if self.is_ultimate_damage(actor, event) {
                        (
                            mechanics.ultimate_resistance_penetration_bp(),
                            mechanics.ultimate_defense_ignore_bp(),
                        )
                    } else {
                        (0, 0)
                    };
                (
                    mechanics
                        .resistance_penetration_bp(self, actor, element)
                        .saturating_add(ultimate_penetration),
                    mechanics
                        .defense_ignore_bp(self, actor, target)
                        .saturating_add(ultimate_defense_ignore),
                    mechanics.outgoing_damage_factor_bp(),
                )
            });
        let sprite_penetration = self.memosprite_owner(actor).map_or(0, |owner| {
            self.mechanics(owner).map_or(0, |rules| {
                rules.memosprite_resistance_penetration_bp(self, owner, element)
            })
        });
        let personal_penetration = personal_penetration.saturating_add(sprite_penetration);
        let mut stats = plan.stats;
        stats.hp = self.effective_max_hp(target);
        stats.defense = self.defense(target);
        DamageTarget {
            stats,
            damage_reduction_bp: 0,
            break_damage_bonus_bp: 0,
            damage_share_bp: 10_000,
            outgoing_damage_factor_bp,
            vulnerability_bp: vulnerability,
            defense_down_bp: sum(DebuffKind::DefenseDown)
                .saturating_add(sum(DebuffKind::DefenseBug))
                .saturating_add(weightless_defense_down)
                .saturating_add(field_defense_down)
                .saturating_add(personal_defense_ignore)
                .saturating_add(u32::from(event.defense_ignore_bp)),
            resistance_down_bp: sum_matching(&|kind| kind.reduces_resistance_to(element))
                .saturating_add(weightless_resistance_down)
                .saturating_add(field_resistance_down)
                .saturating_add(personal_penetration)
                .saturating_add(if element == Element::Wind {
                    self.units[actor].active_penetration_bp
                } else {
                    0
                }),
            toughness: state.toughness,
            max_toughness: plan.kit.max_toughness(),
        }
    }

    fn epiphany_applies(&self, kind: DebuffKind, target: usize) -> bool {
        matches!(
            kind,
            DebuffKind::Epiphany { only_on_turn, .. }
                if !only_on_turn || self.enemy_turn == Some(target)
        )
    }

    fn hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
    ) -> bool {
        self.strike(
            actor,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness,
                origin: self.action,
                scale: DamageScale::Attack,
            },
        )
    }

    pub(super) fn secondary_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        kind: DamageKind,
        origin: Action,
    ) {
        self.strike(
            actor,
            target,
            kind,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness: 0,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
        );
    }

    pub(super) fn secondary_hit_with_credit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        origin: Action,
        provider: usize,
    ) {
        self.strike_scaled(
            actor,
            target,
            DamageKind::Additional,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness: 0,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
            10_000,
            Some(provider),
        );
    }

    pub(super) fn fixed_critical_additional_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        crit_damage_bp: u32,
        origin: Action,
    ) {
        let mut source = self.plan.units[actor].stats;
        source.crit_damage_bp = crit_damage_bp;
        let index = element.index();
        self.resolve_damage(
            actor,
            target,
            element,
            DamageEvent {
                kind: DamageKind::Additional,
                critical: DamageCritical::Guaranteed,
                origin: Some(origin),
                assisted_by: None,
                from_break: false,
                defense_ignore_bp: 0,
            },
            |view| {
                let mut adjusted = source;
                adjusted.element_damage_bp[index] =
                    adjusted.element_damage_bp[index].saturating_add(view.bonus.damage_bp);
                let base_bp = u64::from(view.attack) * u64::from(multiplier_bp);
                damage::direct(adjusted, view.target, element, base_bp, view.critical)
            },
        );
    }

    pub(super) fn secondary_hit_from_hp(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        origin: Action,
    ) {
        self.strike(
            actor,
            target,
            DamageKind::Additional,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness: 0,
                origin: Some(origin),
                scale: DamageScale::MaxHp,
            },
        );
    }

    pub(super) fn attack_extra_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
        origin: Action,
    ) {
        self.strike(
            actor,
            target,
            DamageKind::Additional,
            Hit {
                element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
        );
    }

    pub(super) fn counter_attack(&mut self, actor: usize, target: usize, counter: CounterAttack) {
        if self.units[actor].hp == 0 || self.units[target].hp == 0 {
            return;
        }
        let origin = Action::Basic;
        let was_follow_up = std::mem::replace(&mut self.resolving_follow_up, true);
        let parent_debuff_targets = self.begin_debuff_action();
        self.begin_attack();
        self.emit(EventKind::FollowUp, Some(actor), Some(target));
        let neighbors: Vec<_> = self.adjacent_enemies(target).collect();
        let mut hit = |target, multiplier_bp, toughness, defense_multiplier_bp| {
            self.strike(
                actor,
                target,
                DamageKind::FollowUp,
                Hit {
                    element: counter.element,
                    multiplier_bp: u32::from(multiplier_bp),
                    toughness,
                    origin: Some(origin),
                    scale: DamageScale::AttackAndDefense(defense_multiplier_bp),
                },
            );
        };
        hit(
            target,
            counter.multiplier_bp,
            counter.toughness,
            counter.defense_multiplier_bp,
        );
        if let Some((multiplier, toughness)) = counter.splash {
            for neighbor in neighbors {
                hit(neighbor, multiplier, toughness, 0);
            }
        }
        self.gain_energy(actor, actor, counter.energy_milli);
        self.notify_attack(actor, origin, Some(target));
        self.finish_debuff_action(actor, parent_debuff_targets);
        self.resolve_follow_ups();
        self.resolving_follow_up = was_follow_up;
    }

    fn strike(&mut self, actor: usize, target: usize, kind: DamageKind, impact: Hit) -> bool {
        self.strike_scaled(actor, target, kind, impact, 10_000, None)
    }

    pub(super) fn elation_hit(&mut self, actor: usize, target: usize, hit: ElationHit) {
        self.strike_scaled(
            actor,
            target,
            DamageKind::Elation,
            Hit {
                element: hit.element,
                multiplier_bp: hit.multiplier_bp,
                toughness: hit.toughness,
                origin: Some(hit.origin),
                scale: DamageScale::Elation {
                    punchline: hit.punchline,
                    factor_bp: hit.factor_bp,
                },
            },
            10_000,
            hit.assisted_by,
        );
    }

    pub(super) fn periodic_hit(&mut self, actor: usize, target: usize, dot: PeriodicHit) {
        let (multiplier_bp, scale) = match dot.damage {
            DotScale::Attack(bp) => (
                bp.saturating_mul(u16::from(dot.stacks)),
                DamageScale::Attack,
            ),
            DotScale::StackedAttack {
                multiplier_bp,
                defense_ignore_bp,
            } => (
                10_000,
                DamageScale::StackedAttack {
                    multiplier_bp,
                    defense_ignore_bp,
                },
            ),
            DotScale::TargetHp {
                hp_bp,
                attack_cap_bp,
            } => (hp_bp, DamageScale::TargetHpCapped(attack_cap_bp)),
        };
        self.strike_scaled(
            actor,
            target,
            DamageKind::DamageOverTime,
            Hit {
                element: dot.element,
                multiplier_bp: u32::from(multiplier_bp),
                toughness: 0,
                origin: Some(dot.origin),
                scale,
            },
            dot.ratio_bp,
            None,
        );
    }

    fn strike_scaled(
        &mut self,
        actor: usize,
        target: usize,
        kind: DamageKind,
        impact: Hit,
        ratio_bp: u16,
        assisted_by: Option<usize>,
    ) -> bool {
        let Hit {
            element,
            mut multiplier_bp,
            toughness,
            origin,
            scale,
        } = impact;
        let hp_before = self.units[target].hp;
        if hp_before == 0 {
            return false;
        }
        let is_attack = matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
            || (kind == DamageKind::Elation && assisted_by.is_none());
        if is_attack
            && origin.is_some()
            && matches!(
                self.plan.units[actor].kit,
                Kit::Official(_) | Kit::Memosprite { .. }
            )
            && self.enemies().contains(&target)
            && !self.attack_targets.contains(&target)
        {
            self.attack_targets.push(target);
            self.stack_entanglement(target);
            let follow_up = self.is_follow_up_damage(actor, origin, kind);
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_first_attack_hit(
                        self,
                        owner,
                        target,
                        kind == DamageKind::Direct,
                        follow_up,
                    );
                    mechanics.on_attack_target(self, owner, target);
                    mechanics.on_ally_first_attack_hit(self, owner, actor, target);
                }
            }
        }
        let mut source = self.plan.units[actor].stats;
        let element_index = element.index();
        source.element_damage_bp[element_index] = source.element_damage_bp[element_index]
            .saturating_add(self.units[actor].active_attack_bonus_bp);
        let mechanics = self.mechanics(actor);
        let follow_up_max_hp_bp = if kind == DamageKind::FollowUp {
            mechanics
                .as_ref()
                .map_or(0, |rules| rules.follow_up_max_hp_multiplier_bp())
        } else {
            0
        };
        let scale = if follow_up_max_hp_bp > 0 && matches!(scale, DamageScale::Attack) {
            DamageScale::AttackAndMaxHp {
                max_hp_bp: follow_up_max_hp_bp,
                flat_base_bp: 0,
            }
        } else if matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
            && matches!(scale, DamageScale::Attack)
            && mechanics
                .as_ref()
                .is_some_and(|rules| rules.scales_with_defense())
        {
            DamageScale::Defense
        } else {
            scale
        };
        let mut hit_weakness = self.has_weakness(target, element)
            || self
                .memosprite_owner(actor)
                .and_then(|owner| self.mechanics(owner))
                .is_some_and(|rules| rules.memosprite_ignores_weakness());
        let mut personal_toughness_bp = 0;
        if let Some(mechanics) = &mechanics {
            if toughness > 0 {
                hit_weakness |= mechanics.ignores_weakness(origin, kind)
                    || mechanics
                        .alternate_weakness(self, actor)
                        .is_some_and(|weakness| self.has_weakness(target, weakness));
                personal_toughness_bp = mechanics.toughness_bonus_bp(self, actor, origin, kind);
            }
            source.element_damage_bp[element_index] = source.element_damage_bp[element_index]
                .saturating_add(mechanics.damage_bonus_bp(self, actor, target, origin, kind));
            source.crit_rate_bp = source
                .crit_rate_bp
                .saturating_add(mechanics.crit_rate_bonus_bp(self, actor, target, origin, kind));
            source.crit_damage_bp = source
                .crit_damage_bp
                .saturating_add(mechanics.crit_damage_bonus_bp(self, actor, target, origin, kind));
            source.break_effect_bp = source
                .break_effect_bp
                .saturating_add(mechanics.break_effect_bonus_bp(self, actor));
            multiplier_bp =
                mechanics.damage_multiplier_bp(self, actor, target, origin, kind, multiplier_bp);
        }
        if let Some(owner) = self.memosprite_owner(actor)
            && let Some(rules) = self.mechanics(owner)
        {
            source.element_damage_bp[element_index] = source.element_damage_bp[element_index]
                .saturating_add(rules.memosprite_damage_bonus_bp(self, actor, origin, kind));
        }
        let flat_damage_bp = mechanics.as_ref().map_or(0, |mechanics| {
            mechanics.flat_damage_base_bp(self, actor, origin, kind)
        });
        let action_factor_bp = mechanics.as_ref().map_or(10_000, |mechanics| {
            mechanics.action_damage_factor_bp(self, actor, origin, kind)
        });
        let crit_rate = source
            .crit_rate_bp
            .saturating_add(self.units[actor].crit_rate_bonus_bp);
        let hp_source = self.max_hp_damage_source(actor);
        let base_hp = self.plan.units[hp_source].stats.hp;
        let max_hp_multiplier_bp = self.max_hp_multiplier_bp(hp_source);
        let uses_elation = matches!(
            scale,
            DamageScale::Elation { .. } | DamageScale::AttackWithElation(_)
        );
        let intrinsic_elation = if uses_elation {
            self.intrinsic_elation_bp(actor)
        } else {
            0
        };
        let fallback_elation = assisted_by
            .filter(|_| uses_elation)
            .map_or(0, |owner| self.intrinsic_elation_bp(owner));
        let crit = self.resolve_damage(
            actor,
            target,
            element,
            DamageEvent {
                kind,
                critical: if kind == DamageKind::DamageOverTime {
                    DamageCritical::Never
                } else if kind == DamageKind::Direct
                    && let Some(rules) = self
                        .memosprite_owner(actor)
                        .and_then(|owner| self.mechanics(owner))
                    && rules.memosprite_guarantees_critical(origin)
                {
                    DamageCritical::Guaranteed
                } else {
                    DamageCritical::Chance(crit_rate)
                },
                origin,
                assisted_by,
                from_break: false,
                defense_ignore_bp: match scale {
                    DamageScale::StackedAttack {
                        defense_ignore_bp, ..
                    } => defense_ignore_bp,
                    _ => 0,
                },
            },
            |view| {
                let mut adjusted = source;
                let converted_damage = mechanics.as_ref().map_or(0, |mechanics| {
                    mechanics.damage_bonus_from_break_effect_bp(
                        source
                            .break_effect_bp
                            .saturating_add(view.bonus.break_effect_bp),
                    )
                });
                adjusted.element_damage_bp[element_index] = adjusted.element_damage_bp
                    [element_index]
                    .saturating_add(view.bonus.damage_bp)
                    .saturating_add(converted_damage);
                adjusted.crit_damage_bp = adjusted
                    .crit_damage_bp
                    .saturating_add(view.bonus.crit_damage_bp);
                let elation_bp = if uses_elation {
                    let converted = mechanics
                        .as_ref()
                        .map_or(0, |rules| rules.elation_from_attack_bp(view.attack));
                    intrinsic_elation
                        .saturating_add(converted)
                        .max(fallback_elation)
                        .saturating_add(view.bonus.elation_bp)
                } else {
                    0
                };
                if let DamageScale::Elation {
                    punchline,
                    factor_bp,
                } = scale
                {
                    return damage::elation(
                        adjusted,
                        view.target,
                        element,
                        damage::ElationDamage {
                            multiplier_bp,
                            elation_bp,
                            merrymake_bp: view.bonus.merrymake_bp,
                            punchline,
                            factor_bp,
                        },
                        view.critical,
                    );
                }
                let multiplier_bp = match scale {
                    DamageScale::AttackWithElation(factor_bp) => multiplier_bp
                        .saturating_add(percentage_of(elation_bp, u32::from(factor_bp))),
                    DamageScale::BreakScaledAttack {
                        break_ratio_bp,
                        break_cap_bp,
                    } => multiplier_bp.saturating_add(percentage_of(
                        source
                            .break_effect_bp
                            .saturating_add(view.bonus.break_effect_bp)
                            .min(u32::from(break_cap_bp)),
                        u32::from(break_ratio_bp),
                    )),
                    _ => multiplier_bp,
                };
                let base_bp = scale
                    .base_bp(
                        view.attack,
                        view.defense,
                        percentage_of(
                            base_hp.saturating_add(view.bonus.max_hp_flat),
                            max_hp_multiplier_bp,
                        ),
                        view.target.stats.hp,
                        multiplier_bp,
                    )
                    .saturating_add(flat_damage_bp);
                let base_bp = base_bp * u64::from(ratio_bp) / 10_000;
                let base_bp = if action_factor_bp == 10_000 {
                    base_bp
                } else {
                    u64::try_from(u128::from(base_bp) * u128::from(action_factor_bp) / 10_000)
                        .unwrap_or(u64::MAX)
                };
                damage::direct(adjusted, view.target, element, base_bp, view.critical)
            },
        );

        if toughness > 0 {
            let segment = SuperBreakSegment {
                base: toughness,
                personal_efficiency_bp: personal_toughness_bp,
                full_toughness: hit_weakness,
            };
            self.apply_attack_toughness(
                actor,
                target,
                (element, segment),
                origin,
                source,
                is_attack,
            );
        }
        if crit
            && kind != DamageKind::Direct
            && let Some(action) = origin
            && let Some(mechanics) = self.mechanics(actor)
        {
            mechanics.after_non_primary_critical_hit(self, actor, target, action, kind);
        }
        if origin.is_some()
            && (self.action.is_some() || kind == DamageKind::FollowUp)
            && kind != DamageKind::DamageOverTime
        {
            self.record_half_hp_crossing(target, hp_before);
        }
        crit
    }

    fn toughness_support(&self, actor: usize) -> (u64, u16) {
        if !self.is_party_unit(actor) {
            return (0, 0);
        }
        self.allies()
            .filter(|&owner| self.is_targetable(owner))
            .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
            .fold((0, 0), |(efficiency, off_element), (owner, rules)| {
                (
                    efficiency + u64::from(rules.ally_toughness_bonus_bp(self, owner, actor)),
                    off_element.max(rules.off_element_toughness_bp(self, owner, actor)),
                )
            })
    }

    fn apply_attack_toughness(
        &mut self,
        actor: usize,
        target: usize,
        impact: (Element, SuperBreakSegment),
        origin: Option<Action>,
        source: crate::fixture::Stats,
        collect_super_break: bool,
    ) {
        if self.units[target].hp == 0 {
            return;
        }
        let (element, mut segment) = impact;
        let normal_before = self.units[target].toughness;
        let extra = self
            .live_extra_toughness(target)
            .filter(|pool| pool.remaining > 0);
        segment.full_toughness |= normal_before == 0 && extra.is_some();
        let (team_toughness_bp, off_element_bp) = self.toughness_support(actor);
        let reduction = segment.scaled(team_toughness_bp, off_element_bp);
        if reduction == 0 {
            return;
        }
        if self.tracks_super_break
            && collect_super_break
            && normal_before == 0
            && origin.is_some()
            && self.is_party_unit(actor)
            && self.enemies().contains(&target)
            && self.is_targetable(target)
        {
            self.record_super_break_segment(target, element, segment);
        }
        if normal_before > 0 {
            let after = normal_before.saturating_sub(reduction);
            self.set_toughness(actor, target, after);
            self.notify_toughness_reduced(actor, target, origin, normal_before - after);
            if after == 0 {
                self.resolve_weakness_break(actor, target, element, origin, source);
            }
        }
        let overflow = reduction.saturating_sub(normal_before);
        if self.is_targetable(target)
            && overflow > 0
            && let Some(pool) = extra
        {
            let after = pool.remaining.saturating_sub(overflow);
            self.set_extra_toughness(actor, target, after);
            self.notify_toughness_reduced(actor, target, origin, pool.remaining - after);
            if after == 0 {
                self.resolve_weakness_break(actor, target, element, origin, source);
            }
        }
    }

    fn notify_toughness_reduced(
        &mut self,
        actor: usize,
        target: usize,
        origin: Option<Action>,
        reduced: u16,
    ) {
        if origin.is_none() || !self.is_party_unit(actor) || !self.enemies().contains(&target) {
            return;
        }
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_toughness_reduced(self, owner, actor, reduced);
            }
        }
    }

    fn resolve_weakness_break(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Option<Action>,
        source: crate::fixture::Stats,
    ) {
        self.emit(EventKind::WeaknessBroken, Some(actor), Some(target));
        self.resolve_damage(
            actor,
            target,
            element,
            DamageEvent {
                kind: DamageKind::WeaknessBreak,
                critical: DamageCritical::Never,
                origin,
                assisted_by: None,
                from_break: true,
                defense_ignore_bp: 0,
            },
            |view| {
                let mut adjusted = source;
                adjusted.break_effect_bp = adjusted
                    .break_effect_bp
                    .saturating_add(view.bonus.break_effect_bp);
                damage::weakness_break(adjusted, view.target, element, 10_000)
            },
        );
        if self.units[target].hp > 0 {
            let delayed = self.units[target].gauge_quanta + gauge_for(BREAK_DELAY_BP);
            self.shift_gauge(actor, target, delayed, false);
            if source.level == 80
                && let Some(action) = origin
            {
                let break_effect = self.effective_break_effect(actor);
                match element {
                    Element::Ice if self.roll_status(actor, target, 15_000, true) => {
                        self.apply_break_freeze(actor, target, action);
                    }
                    Element::Thunder => self.apply_break_shock(actor, target, action),
                    Element::Quantum if self.roll_status(actor, target, 15_000, true) => {
                        self.apply_entanglement(actor, target, action, break_effect);
                    }
                    Element::Imaginary if self.roll_status(actor, target, 15_000, true) => {
                        self.apply_imprisonment(actor, target, break_effect);
                    }
                    Element::Wind | Element::Physical | Element::Fire
                        if self.roll_effect(actor, target, 15_000) =>
                    {
                        self.apply_break_dot(actor, target, element, action)
                    }
                    _ => {}
                }
            }
        }
        if let Some(action) = origin {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_weakness_break(self, owner, actor, target, action);
                }
            }
        }
    }

    fn record_half_hp_crossing(&mut self, target: usize, hp_before: u32) {
        let (hp, max_hp) = self.hp(target);
        if self.enemies().contains(&target)
            && hp_before > max_hp / 2
            && hp <= max_hp / 2
            && !self.crossed_half.contains(&target)
        {
            self.crossed_half.push(target);
        }
    }

    fn record_super_break_segment(
        &mut self,
        target: usize,
        element: Element,
        segment: SuperBreakSegment,
    ) {
        if let Some(group) = self
            .super_break_targets
            .iter_mut()
            .find(|group| group.target == target && group.element == element)
        {
            group.toughness_segments.push(segment);
        } else {
            self.super_break_targets.push(SuperBreakTarget {
                target,
                element,
                toughness_segments: vec![segment],
            });
        }
    }

    fn resolve_super_breaks(&mut self, actor: usize, origin: Action, targets: &[SuperBreakTarget]) {
        if targets.is_empty() {
            return;
        }
        let mut source = self.plan.units[actor].stats;
        if let Some(rules) = self.mechanics(actor) {
            source.break_effect_bp = source
                .break_effect_bp
                .saturating_add(rules.break_effect_bonus_bp(self, actor));
        }
        for group in targets {
            let SuperBreakTarget {
                target,
                element,
                toughness_segments,
            } = group;
            for owner in self.allies() {
                if !self.is_targetable(*target) || !self.is_targetable(owner) {
                    continue;
                }
                let Some(rules) = self.mechanics(owner) else {
                    continue;
                };
                let multiplier = rules.super_break_multiplier_bp(self, owner, actor, *target);
                if multiplier == 0 {
                    continue;
                }
                let hp_before = self.units[*target].hp;
                let mut event = Self::break_damage_event(origin, DamageKind::SuperBreak);
                event.assisted_by = Some(owner);
                self.resolve_damage(actor, *target, *element, event, |view| {
                    let mut adjusted = source;
                    adjusted.break_effect_bp = adjusted
                        .break_effect_bp
                        .saturating_add(view.bonus.break_effect_bp);
                    let toughness = toughness_segments
                        .iter()
                        .map(|segment| {
                            u32::from(segment.scaled(
                                u64::from(view.bonus.toughness_efficiency_bp),
                                view.bonus.off_element_toughness_bp,
                            ))
                        })
                        .sum();
                    let multiplier = rules
                        .super_break_multiplier_for_effect(multiplier, adjusted.break_effect_bp);
                    damage::super_break(adjusted, view.target, *element, toughness, multiplier)
                });
                self.record_half_hp_crossing(*target, hp_before);
            }
        }
    }

    pub(super) fn skip_frozen_turn(&mut self, actor: usize, target: usize, thawed: bool) {
        self.shift_gauge(
            actor,
            target,
            if thawed { GAUGE_FULL / 2 } else { GAUGE_FULL },
            false,
        );
    }

    pub(super) fn break_shock_hit(&mut self, actor: usize, target: usize, origin: Action) {
        self.break_status_hit(
            actor,
            target,
            Element::Thunder,
            origin,
            DamageKind::DamageOverTime,
        );
    }

    pub(super) fn break_status_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        kind: DamageKind,
    ) {
        self.break_status_hit_scaled(actor, target, element, origin, kind, 10_000);
    }

    pub(super) fn break_dot_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        ratio_bp: u16,
    ) {
        self.break_status_hit_scaled(
            actor,
            target,
            element,
            origin,
            DamageKind::DamageOverTime,
            ratio_bp,
        );
    }

    /// Deals damage from a break effect without causing another break or applying its statuses.
    pub(super) fn break_status_hit_scaled(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        kind: DamageKind,
        ratio_bp: u16,
    ) {
        let event = Self::break_damage_event(origin, kind);
        self.resolve_break_status_hit(actor, target, element, event, u32::from(ratio_bp), u16::MAX);
    }

    pub(super) fn break_hit_capped(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        ratio_bp: u16,
        max_toughness: u16,
    ) {
        let event = Self::break_damage_event(origin, DamageKind::WeaknessBreak);
        self.resolve_break_status_hit(
            actor,
            target,
            element,
            event,
            u32::from(ratio_bp),
            max_toughness,
        );
    }

    /// Resolves an added Break hit and its weakness-ignoring Toughness as part of the current attack.
    pub(super) fn attack_break_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        ratio_bp: u32,
        toughness: u16,
    ) {
        let hp_before = self.units[target].hp;
        let event = Self::break_damage_event(origin, DamageKind::WeaknessBreak);
        self.resolve_break_status_hit(actor, target, element, event, ratio_bp, u16::MAX);
        let mut source = self.plan.units[actor].stats;
        let mut personal_efficiency_bp = 0;
        if let Some(rules) = self.mechanics(actor) {
            source.break_effect_bp = source
                .break_effect_bp
                .saturating_add(rules.break_effect_bonus_bp(self, actor));
            personal_efficiency_bp =
                rules.toughness_bonus_bp(self, actor, Some(origin), DamageKind::WeaknessBreak);
        }
        let segment = SuperBreakSegment {
            base: toughness,
            personal_efficiency_bp,
            full_toughness: true,
        };
        self.apply_attack_toughness(
            actor,
            target,
            (element, segment),
            Some(origin),
            source,
            true,
        );
        self.record_half_hp_crossing(target, hp_before);
    }

    fn break_damage_event(origin: Action, kind: DamageKind) -> DamageEvent {
        DamageEvent {
            kind,
            critical: DamageCritical::Never,
            origin: Some(origin),
            assisted_by: None,
            from_break: true,
            defense_ignore_bp: 0,
        }
    }

    fn resolve_break_status_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        event: DamageEvent,
        ratio_bp: u32,
        max_toughness: u16,
    ) {
        if self.units[target].hp == 0 {
            return;
        }
        let mut source = self.plan.units[actor].stats;
        if let Some(mechanics) = self.mechanics(actor) {
            source.break_effect_bp = source
                .break_effect_bp
                .saturating_add(mechanics.break_effect_bonus_bp(self, actor));
        }
        let initial_break = event.kind == DamageKind::WeaknessBreak;
        self.resolve_damage(actor, target, element, event, |view| {
            let mut adjusted = source;
            adjusted.break_effect_bp = adjusted
                .break_effect_bp
                .saturating_add(view.bonus.break_effect_bp);
            if initial_break {
                let mut defender = view.target;
                defender.max_toughness = defender.max_toughness.min(max_toughness);
                damage::weakness_break(adjusted, defender, element, ratio_bp)
            } else {
                damage::break_status(adjusted, view.target, element, ratio_bp)
            }
        });
    }

    fn resolve_damage(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        event: DamageEvent,
        calculate: impl Fn(DamageView) -> u32,
    ) -> bool {
        let DamageEvent {
            kind,
            critical,
            origin,
            assisted_by,
            from_break,
            ..
        } = event;
        let crit_rate_bp = match critical {
            DamageCritical::Chance(rate) => Some(rate),
            DamageCritical::Never | DamageCritical::Guaranteed => None,
        };
        let offense = OffenseContext {
            actor,
            max_hp_source: self.max_hp_damage_source(actor),
            enemy: self.enemies().contains(&target).then_some(target),
            element,
            kind,
            break_damage: from_break || kind.is_break(),
            follow_up: self.is_follow_up_damage(actor, origin, kind),
            summon: self.memosprite_owner(actor).is_some()
                || self
                    .mechanics(actor)
                    .is_some_and(|rules| rules.damage_comes_from_summon(self, actor, origin, kind)),
        };
        let distribution = self.damage_distribution(actor, target);
        let recipient = |unit, share_bp| DamageRecipient {
            unit,
            share_bp,
            reduction_bp: self.incoming_reduction_bp(actor, unit, kind),
        };
        let main_recipient = recipient(target, distribution.map_or(10_000, |(_, bp)| 10_000 - bp));
        let distributed_recipient = distribution.map(|(unit, bp)| recipient(unit, bp));
        let applied_buffs = self.offense_buffs(offense, |_| true);
        let actor_mechanics = self.mechanics(actor);
        let derived_critical = crit_rate_bp.and(actor_mechanics.as_ref());
        let crit_rate_from_defense = |defense| {
            derived_critical
                .as_ref()
                .map_or(0, |rules| rules.crit_rate_from_defense_bp(defense))
        };
        let break_effect = self.plan.units[actor].stats.break_effect_bp.saturating_add(
            derived_critical
                .as_ref()
                .map_or(0, |rules| rules.break_effect_bonus_bp(self, actor)),
        );
        let crit_from_break = |bonus: OffenseBuffs| {
            derived_critical.as_ref().map_or((0, 0), |rules| {
                rules.crit_from_break_effect_bp(break_effect.saturating_add(bonus.break_effect_bp))
            })
        };
        // Compare the same roll with each counterfactual crit rate during attribution.
        let crit_roll = crit_rate_bp
            .filter(|rate| {
                rate.saturating_add(applied_buffs.crit_rate_bp)
                    .saturating_add(crit_from_break(applied_buffs).0)
                    .saturating_add(crit_rate_from_defense(
                        self.defense_with_buff(actor, applied_buffs.defense_bp),
                    ))
                    > 0
            })
            .map(|_| self.rng.below(10_000));
        let damage_with = |recipient: DamageRecipient,
                           mut bonus: OffenseBuffs,
                           allow: &dyn Fn(usize) -> bool| {
            let mut defender = self.damage_target(actor, recipient.unit, element, &event, allow);
            defender.damage_share_bp = recipient.share_bp;
            defender.damage_reduction_bp = recipient.reduction_bp;
            defender.break_damage_bonus_bp = bonus.break_damage_bonus_bp;
            defender.defense_down_bp = defender
                .defense_down_bp
                .saturating_add(bonus.defense_ignore_bp);
            defender.resistance_down_bp = defender
                .resistance_down_bp
                .saturating_add(bonus.resistance_penetration_bp);
            let defense = self.defense_with_buff(actor, bonus.defense_bp);
            let (break_crit_rate, break_crit_damage) = crit_from_break(bonus);
            let total_crit_rate = crit_rate_bp
                .unwrap_or_default()
                .saturating_add(bonus.crit_rate_bp)
                .saturating_add(break_crit_rate)
                .saturating_add(crit_rate_from_defense(defense));
            let rolled_critical = crit_roll.is_some_and(|roll| roll < u64::from(total_crit_rate));
            let critical = matches!(critical, DamageCritical::Guaranteed) || rolled_critical;
            bonus.crit_damage_bp = bonus.crit_damage_bp.saturating_add(break_crit_damage);
            if rolled_critical && bonus.crit_overflow_ratio_bp > 0 {
                let overflow = total_crit_rate.saturating_sub(10_000);
                bonus.crit_damage_bp = bonus
                    .crit_damage_bp
                    .saturating_add(percentage_of(overflow, bonus.crit_overflow_ratio_bp));
            }
            let attack = self.attack_with_buffs(actor, bonus);
            if let Some(rules) = &actor_mechanics {
                bonus.break_effect_bp = bonus
                    .break_effect_bp
                    .saturating_add(rules.break_effect_from_attack_bp(self, actor, Some(attack)));
            }
            let amount = calculate(DamageView {
                target: defender,
                bonus,
                attack,
                defense,
                critical,
            });
            (amount, critical)
        };
        let ultimate = (kind == DamageKind::Direct && origin.is_some_and(Action::is_ultimate))
            .then(|| self.mechanics(actor))
            .flatten();
        let (amount, critical) = damage_with(main_recipient, applied_buffs, &|_| true);
        let distributed_damage = distributed_recipient.map(|recipient| {
            let (amount, _) = damage_with(recipient, applied_buffs, &|_| true);
            (recipient.unit, amount)
        });
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            let state = &self.units[target];
            let assists: std::collections::BTreeSet<_> = state
                .debuffs
                .iter()
                .filter(|status| {
                    matches!(
                        status.kind,
                        DebuffKind::DefenseDown
                            | DebuffKind::SeamStitch
                            | DebuffKind::DefenseBug
                            | DebuffKind::Vulnerability
                            | DebuffKind::WoesWhisper
                            | DebuffKind::Weightless { .. }
                    ) || self.epiphany_applies(status.kind, target)
                        || status.kind.reduces_resistance_to(element)
                        || (status.kind == DebuffKind::UltimateVulnerability
                            && self.is_ultimate_damage(actor, &event))
                        || (status.kind == DebuffKind::DotVulnerability
                            && kind == DamageKind::DamageOverTime)
                        || (status.kind.is_break_vulnerability() && kind.is_break())
                })
                .map(|status| status.source)
                .chain(
                    state.statuses[StatusKind::Vulnerability.index()].map(|status| status.source),
                )
                .chain(self.allies().filter(|&owner| {
                    self.offense_buffs_from(owner, offense) != OffenseBuffs::default()
                        || self.target_auras_from(owner, actor, target, element, &event)
                            != DamageTargetAura::default()
                }))
                .filter(|&source| {
                    source != self.memosprite_owner(actor).unwrap_or(actor)
                        && source < self.plan.ally_count
                })
                .collect();
            let mut credits = Vec::new();
            if !assists.is_empty() {
                let damage_by = |allow: &dyn Fn(usize) -> bool| {
                    damage_with(main_recipient, self.offense_buffs(offense, allow), allow).0
                };
                let mut damage = damage_by(&|source| !assists.contains(&source));
                for &owner in &assists {
                    let adjusted =
                        damage_by(&|source| source <= owner || !assists.contains(&source));
                    if adjusted > damage {
                        credits.push((owner, adjusted - damage));
                    }
                    damage = adjusted;
                }
            }
            if let Some(owner) =
                assisted_by.filter(|&owner| owner != actor && owner < self.plan.ally_count)
            {
                let remainder = amount - credits.iter().map(|&(_, value)| value).sum::<u32>();
                if let Some((_, value)) = credits.iter_mut().find(|entry| entry.0 == owner) {
                    *value += remainder;
                } else if remainder > 0 {
                    credits.push((owner, remainder));
                }
            }
            if let Some(rules) = ultimate {
                rules.capture_ultimate_damage(self, actor, target, amount, &credits);
            }
            self.notify_ally_damage(actor, target, kind, amount);
            self.dummy_damage.push(DummyDamage {
                av_micros: self.av_micros,
                actor,
                target,
                kind,
                critical,
                amount,
                assists: credits,
                action: origin.unwrap_or(Action::Basic),
            });
        } else {
            if let Some(rules) = ultimate {
                rules.capture_ultimate_damage(self, actor, target, amount, &[]);
            }
            self.deal_damage(actor, target, kind, amount, critical);
            if let Some((recipient, amount)) = distributed_damage {
                self.deal_damage(actor, recipient, kind, amount, critical);
            }
        }
        if offense.enemy.is_some()
            && self.is_party_unit(actor)
            && matches!(
                kind,
                DamageKind::Direct
                    | DamageKind::FollowUp
                    | DamageKind::Additional
                    | DamageKind::Elation
                    | DamageKind::WeaknessBreak
                    | DamageKind::SuperBreak
            )
            && let Some(total) = &mut self.attack_damage
        {
            *total = total.saturating_add(u64::from(amount));
        }
        if amount > 0
            && kind != DamageKind::TrueDamage
            && offense.enemy.is_some()
            && self.is_party_unit(actor)
        {
            let damage_owner = self.memosprite_owner(actor).unwrap_or(actor);
            for owner in self.allies() {
                let bp = self.mechanics(owner).map_or(0, |rules| {
                    rules.memosprite_true_damage_bp(self, owner, actor)
                });
                let extra = percentage_of(amount, bp);
                if extra > 0 {
                    let assists = if damage_owner == owner {
                        Vec::new()
                    } else {
                        vec![(owner, extra)]
                    };
                    self.true_damage(
                        actor,
                        target,
                        origin.unwrap_or(Action::Basic),
                        extra,
                        assists,
                    );
                }
            }
        }
        if amount > 0
            && kind == DamageKind::DamageOverTime
            && offense.enemy.is_some()
            && let Some(origin) = origin
        {
            let dot = DotEvent {
                source: actor,
                target,
                element,
                from_break,
                origin,
                timing: self.dot_timing,
            };
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_enemy_dot_damage(self, owner, dot);
                }
            }
        }
        critical
    }

    pub(super) fn true_damage(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        amount: u32,
        assists: Vec<(usize, u32)>,
    ) {
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            self.dummy_damage.push(DummyDamage {
                av_micros: self.av_micros,
                actor,
                target,
                kind: DamageKind::TrueDamage,
                critical: false,
                amount,
                assists,
                action: origin,
            });
        } else if self.is_targetable(target) {
            let hp_before = self.units[target].hp;
            self.deal_damage(actor, target, DamageKind::TrueDamage, amount, false);
            self.record_half_hp_crossing(target, hp_before);
        }
    }

    fn notify_ally_damage(&mut self, actor: usize, target: usize, kind: DamageKind, amount: u32) {
        if !self.tracks_ally_damage
            || amount == 0
            || kind == DamageKind::TrueDamage
            || !self.enemies().contains(&target)
            || !self.is_party_unit(actor)
        {
            return;
        }
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(mechanics) = self.mechanics(owner)
                && mechanics.tracks_ally_damage()
            {
                // Observe the hit before a defeat can move an enemy mark.
                mechanics.on_ally_damage(self, owner, actor, target, kind, amount);
            }
        }
    }

    fn incoming_reduction_bp(&self, actor: usize, target: usize, kind: DamageKind) -> u32 {
        let personal_bp = self.mechanics(target).map_or(0, |mechanics| {
            mechanics.incoming_damage_reduction_bp(self, target, actor, kind)
        });
        let invigoration_bp = self.invigoration_damage_reduction_bp(target);
        let mut remaining_bp =
            10_000_u32.saturating_sub(personal_bp) * (10_000 - invigoration_bp) / 10_000;
        let enemy_source = self.enemies().contains(&actor);
        let ally_target = self.is_party_unit(target);
        if !enemy_source && !ally_target {
            return 10_000 - remaining_bp;
        }
        if enemy_source {
            for status in &self.units[actor].debuffs {
                if status.kind == DebuffKind::Weaken {
                    remaining_bp = remaining_bp * 10_000_u32.saturating_sub(status.bp) / 10_000;
                }
            }
        }
        for owner in self.allies().filter(|&owner| self.units[owner].hp > 0) {
            if let Some(mechanics) = self.mechanics(owner) {
                if enemy_source {
                    let bp = mechanics.enemy_damage_reduction_bp(self, owner);
                    remaining_bp = remaining_bp * 10_000_u32.saturating_sub(bp) / 10_000;
                }
                if ally_target {
                    let bp = mechanics.ally_damage_reduction_bp(self, owner);
                    remaining_bp = remaining_bp * 10_000_u32.saturating_sub(bp) / 10_000;
                }
            }
        }
        10_000 - remaining_bp
    }

    fn damage_distribution(&self, actor: usize, target: usize) -> Option<(usize, u16)> {
        if !self.enemies().contains(&actor) || !self.is_party_unit(target) {
            return None;
        }
        self.allies()
            .filter(|&owner| owner != target && self.is_targetable(owner))
            .find_map(|owner| {
                let bp = self.mechanics(owner)?.damage_distribution_bp(self, owner);
                (bp > 0).then_some((owner, bp.min(10_000)))
            })
    }

    fn deal_damage(
        &mut self,
        actor: usize,
        target: usize,
        kind: DamageKind,
        amount: u32,
        critical: bool,
    ) {
        let absorbed = self.shield_amount(target).min(amount);
        let incoming = amount - absorbed;
        let preserved_hp = u32::from(self.protect_from_fatal_hp_overflow(target, incoming));
        let state = &mut self.units[target];
        let hp_loss = state.hp.saturating_sub(preserved_hp).min(incoming);
        state.hp -= hp_loss;
        if matches!(self.plan.units[target].kit, Kit::Enemy(_)) {
            self.enemy_hp_lost += hp_loss;
        }
        self.emit(
            EventKind::Damage {
                kind,
                amount,
                hp_loss,
                absorbed,
                critical,
            },
            Some(actor),
            Some(target),
        );
        self.notify_ally_damage(actor, target, kind, hp_loss);
        if absorbed > 0 {
            self.deplete_shields(target, amount);
        }
        self.record_team_hp_loss(target, hp_loss);
        if amount > 0 {
            let attack_index = (self.enemy_action == Some(actor)
                && matches!(kind, DamageKind::Direct | DamageKind::FollowUp))
            .then_some(self.attack_index);
            self.notify_ally_health_event(target, attack_index);
        }
        if hp_loss > 0
            && let Some(mechanics) = self.mechanics(target)
        {
            mechanics.after_hp_loss(self, target, actor, hp_loss);
        }
        if hp_loss > 0 {
            self.notify_ally_hp_loss(target);
        }
        if self.units[target].hp == 0 {
            if let Some(mechanics) = self.mechanics(target)
                && mechanics.prevent_defeat(self, target)
                && self.units[target].hp > 0
            {
                return;
            }
            if self.enemies().contains(&actor)
                && self.allies().contains(&target)
                && (self.try_team_rescue(actor, target)
                    || (matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
                        && self.try_ally_rescue(target)))
            {
                return;
            }
            self.confirm_defeat(actor, target);
        }
    }

    pub(super) fn confirm_defeat(&mut self, actor: usize, target: usize) {
        self.units[target].hp = 0;
        self.emit(EventKind::UnitDefeated, Some(actor), Some(target));
        if let Some(mechanics) = self.mechanics(target) {
            mechanics.on_defeated(self, target);
        }
        let attack_owner = self.memosprite_owner(actor).unwrap_or(actor);
        if self.allies().contains(&attack_owner) {
            self.gain_energy(actor, attack_owner, KILL_ENERGY_MILLI);
            if let Some(mechanics) = self.mechanics(attack_owner) {
                mechanics.after_defeat(self, attack_owner, target);
            }
        }
        if self.enemies().contains(&target) {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_enemy_defeat(self, owner, actor, target);
                }
            }
        } else if target < self.plan.ally_count {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_ally_defeat(self, owner);
                }
            }
        }
        self.clear_defeated_effects(target);
        self.remove_memosprite_of(target);
        self.notify_memosprite_departed(target);
    }

    pub(super) fn clear_defeated_effects(&mut self, target: usize) {
        let state = &mut self.units[target];
        state.statuses.fill(None);
        state.summon_turn = None;
        state.countdown = None;
        state.shields.clear();
        state.shocks.clear();
        state.frozen = None;
        state.debuffs.clear();
        state.break_dots.clear();
        state.entanglement = None;
        state.lynx_response = None;
        state.skill_dots.clear();
        state.heals_over_time.clear();
        state.invigoration = None;
        state.ally_buffs.clear();
    }

    fn try_team_rescue(&mut self, attacker: usize, target: usize) -> bool {
        self.allies().any(|owner| {
            (owner == target || self.is_targetable(owner))
                && self.mechanics(owner).is_some_and(|mechanics| {
                    mechanics.prevent_team_defeat(self, owner, attacker, target)
                })
        })
    }

    fn try_ally_rescue(&mut self, target: usize) -> bool {
        for owner in self.allies() {
            if owner != target
                && self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
                && mechanics.prevent_ally_defeat(self, owner, target)
                && self.units[target].hp > 0
            {
                return true;
            }
        }
        false
    }

    pub(super) fn shift_gauge(
        &mut self,
        actor: usize,
        target: usize,
        after_quanta: u64,
        advanced_to_front: bool,
    ) {
        let before_quanta = if let Some(owner) = self.memosprite_owner(target)
            && let Some(summon) = &mut self.units[owner].summon_turn
        {
            summon.advanced_to_front = advanced_to_front;
            std::mem::replace(&mut summon.gauge_quanta, after_quanta)
        } else {
            self.units[target].advanced_to_front = advanced_to_front;
            std::mem::replace(&mut self.units[target].gauge_quanta, after_quanta)
        };
        self.emit(
            EventKind::GaugeShifted {
                before_quanta,
                after_quanta,
            },
            Some(actor),
            Some(target),
        );
    }

    pub(super) fn delay_action(&mut self, source: usize, target: usize, delay_bp: u16) {
        let delayed = self
            .gauge_quanta(target)
            .saturating_add(gauge_for(delay_bp));
        self.shift_gauge(source, target, delayed, false);
    }

    fn apply_imprisonment(&mut self, actor: usize, target: usize, break_effect_bp: u32) {
        if self.apply_break_slow(actor, target) {
            let extra_delay = gauge_for(3_000) / 10_000 * (10_000 + u64::from(break_effect_bp));
            let delayed = self.gauge_quanta(target) + extra_delay;
            self.shift_gauge(actor, target, delayed, false);
        }
    }

    pub(super) fn set_toughness(&mut self, actor: usize, target: usize, after: u16) {
        let state = &mut self.units[target];
        let before = std::mem::replace(&mut state.toughness, after);
        if before == 0 && after > 0 {
            state.rebloom_spent = false;
        }
        self.emit(
            EventKind::Toughness { before, after },
            Some(actor),
            Some(target),
        );
        if before == 0
            && after > 0
            && let Some(extra) = self.live_extra_toughness(target)
        {
            self.set_extra_toughness(extra.source, target, extra.maximum);
        }
    }

    pub(super) fn grant_extra_toughness(&mut self, source: usize, target: usize, maximum: u16) {
        if maximum == 0 {
            return;
        }
        self.units[target].extra_toughness = Some(ExtraToughness {
            source,
            remaining: 0,
            maximum,
        });
        self.set_extra_toughness(source, target, maximum);
    }

    fn live_extra_toughness(&self, target: usize) -> Option<ExtraToughness> {
        self.units[target]
            .extra_toughness
            .filter(|extra| self.is_targetable(extra.source))
    }

    fn set_extra_toughness(&mut self, actor: usize, target: usize, after: u16) {
        if let Some(pool) = &mut self.units[target].extra_toughness {
            let before = std::mem::replace(&mut pool.remaining, after);
            if before != after {
                self.emit(
                    EventKind::CloudflameLuster { before, after },
                    Some(actor),
                    Some(target),
                );
            }
        }
    }

    pub(super) fn apply_status(
        &mut self,
        actor: usize,
        target: usize,
        status: StatusKind,
        magnitude: u32,
        turns: u32,
    ) {
        if status == StatusKind::Vulnerability && self.resists_debuff(target) {
            return;
        }
        let state = &mut self.units[target];
        state.statuses[status.index()] = Some(Status {
            source: actor,
            magnitude,
            expires_after_turn: state.normal_turns + turns,
        });
        if status == StatusKind::Vulnerability {
            self.record_enemy_debuff(target);
        }
        self.emit(EventKind::StatusApplied(status), Some(actor), Some(target));
    }

    pub(super) fn finish_turn(&mut self, actor: usize) {
        self.expire_certified_banger(actor);
        self.expire_debuffs(actor);
        self.expire_shields(actor);
        self.expire_ally_buffs(actor);
        self.expire_invigoration(actor);
        self.expire_survival_response(actor);
        let state = &mut self.units[actor];
        state.finished_turns = state.normal_turns;
        if state
            .attack_bonus
            .is_some_and(|(_, expires)| expires <= state.normal_turns)
        {
            state.attack_bonus = None;
        }
        for kind in StatusKind::ALL {
            let state = &mut self.units[actor];
            if let Some(status) = state.statuses[kind.index()]
                && status.expires_after_turn <= state.normal_turns
            {
                state.statuses[kind.index()] = None;
                self.emit(
                    EventKind::StatusExpired(kind),
                    Some(status.source),
                    Some(actor),
                );
            }
        }
        self.notify_ordinary_turn_finished(actor);
        self.notify_battle_turn_finished(actor, false);
    }

    pub(super) fn gain_energy(&mut self, actor: usize, target: usize, amount_milli: u32) {
        let cap = self.plan.units[target].kit.max_energy_milli();
        if cap == 0 {
            return;
        }
        let bonus_bp = self.mechanics(target).map_or(0, |mechanics| {
            mechanics.energy_regeneration_bonus_bp(self, target)
        });
        let ratio = self.plan.units[target]
            .stats
            .energy_regeneration_bp
            .saturating_add(bonus_bp);
        let gain = u64::from(amount_milli) * u64::from(ratio) / 10_000;
        let after = cap.min(
            self.units[target]
                .energy_milli
                .saturating_add(saturating_stat(gain)),
        );
        self.set_energy(actor, target, after);
    }

    pub(super) fn set_energy(&mut self, actor: usize, target: usize, after_milli: u32) {
        let before_milli = std::mem::replace(&mut self.units[target].energy_milli, after_milli);
        if before_milli != after_milli {
            self.emit(
                EventKind::Energy {
                    before_milli,
                    after_milli,
                },
                Some(actor),
                Some(target),
            );
            let cap = self.plan.units[target].kit.max_energy_milli();
            if target < self.plan.ally_count && after_milli > before_milli {
                for owner in self.allies() {
                    if self.is_targetable(owner)
                        && let Some(mechanics) = self.mechanics(owner)
                    {
                        mechanics.on_ally_energy_restored(self, owner, after_milli - before_milli);
                    }
                }
            }
            if target < self.plan.ally_count && before_milli < cap && after_milli == cap {
                for owner in self.allies() {
                    if self.units[owner].hp > 0
                        && let Some(mechanics) = self.mechanics(owner)
                    {
                        mechanics.on_ally_energy_full(self, owner, target);
                    }
                }
            }
        }
    }

    pub(super) fn fill_energy(&mut self, actor: usize) {
        self.set_energy(actor, actor, self.plan.units[actor].kit.max_energy_milli());
    }
}

fn saturating_stat(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub(super) fn gauge_for(basis_points: u16) -> u64 {
    GAUGE_FULL / 10_000 * u64::from(basis_points)
}
