use crate::{
    Action, DamageKind, EventKind, StatusKind,
    damage::{self, DamageTarget},
    fixture::{
        Ability, BREAK_DELAY_BP, Effect, Element, EnemyKit, EnemyTarget, GAUGE_FULL,
        KILL_ENERGY_MILLI, Stats, Target,
    },
};

use super::{
    Battle, DummyDamage, ExtraToughness, Kit, Status,
    combat_status::DebuffKind,
    enemy::{EnemyDebuffKind, EnemyDotScale, EnemyOnHit, EnemySkillAction, OfficialEnemyKit},
    periodic::{DotEvent, DotScale, DotTiming, SkillDot},
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
    unbroken: bool,
    fixed_provider: Option<usize>,
    precise_conversion: bool,
}

impl SuperBreakSegment {
    fn scaled(self, team_bp: u64, off_element_bp: u16) -> u16 {
        (self.scaled_hundredths(team_bp, off_element_bp) / 100) as u16
    }

    fn scaled_hundredths(self, team_bp: u64, off_element_bp: u16) -> u32 {
        if self.fixed_provider.is_some() {
            return u32::from(self.base);
        }
        let efficiency_bp = 10_000 + u64::from(self.personal_efficiency_bp) + team_bp;
        let weakness_bp = if self.full_toughness {
            10_000
        } else {
            off_element_bp
        };
        (u64::from(self.base) * efficiency_bp * u64::from(weakness_bp) / 1_000_000)
            .min(u64::from(u16::MAX) * 100) as u32
    }

    fn converted_hundredths(self, buffs: OffenseBuffs) -> u32 {
        if self.unbroken && !buffs.unbroken_super_break {
            return 0;
        }
        if let Some(owner) = self.fixed_provider
            && buffs.fixed_toughness_providers & (1 << owner) == 0
        {
            return 0;
        }
        let value = self.scaled_hundredths(
            u64::from(buffs.toughness_efficiency_bp),
            buffs.off_element_toughness_bp,
        );
        if self.precise_conversion || self.unbroken {
            value
        } else {
            value / 100 * 100
        }
    }
}

#[derive(Clone, Copy)]
enum SuperBreakConversion {
    None,
    Attack,
    FollowUp(u16),
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
        allow_provider_elation: bool,
    },
    FixedBase(u64),
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
            Self::FixedBase(base_bp) => base_bp,
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

#[derive(Clone, Copy)]
pub(super) struct JointFollowUpAttack {
    pub element: Element,
    pub multiplier_bp: u32,
    pub toughness: u16,
    pub energy_milli: u32,
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
    ElationSkillFromBanger {
        actor: usize,
    },
    TriggeredSkill {
        actor: usize,
        target: usize,
    },
    TriggeredElationSkill {
        actor: usize,
        punchline: u32,
    },
    JointFollowUp {
        participants: [(usize, JointFollowUpAttack); 2],
        origin: Action,
        restores_skill_points: u8,
    },
}

#[derive(Clone, Copy)]
enum FollowUpTargets {
    All,
    Single(usize),
    Special,
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
        first_target: Option<usize>,
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
    pub(super) fn queue_elation_skill_from_banger(&mut self, actor: usize) {
        self.pending_follow_ups
            .push_front(PendingAttack::ElationSkillFromBanger { actor });
    }

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
            (DamageKind::Direct | DamageKind::Elation, Some(action), Kit::Official(kit)) => {
                kit.mechanics
                    .primary_attack_counts_as_follow_up(self, actor, action)
                    || (kind == DamageKind::Direct
                        && action.is_ultimate()
                        && self.is_party_unit(actor)
                        && self.allies().any(|owner| {
                            self.is_targetable(owner)
                                && self
                                    .mechanics(owner)
                                    .is_some_and(|rules| rules.grants_ultimate_follow_up())
                        }))
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

    pub(super) fn attack_hit_with_defense(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        [attack_bp, defense_bp]: [u16; 2],
        toughness: u16,
    ) {
        self.attack_hit_scaled(
            actor,
            target,
            element,
            u32::from(attack_bp),
            toughness,
            DamageScale::AttackAndDefense(defense_bp),
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
        origin: Action,
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
                    origin: Some(origin),
                    scale: DamageScale::MaxHp,
                },
            );
        }
    }

    pub(super) fn final_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u32,
        toughness: u16,
    ) {
        self.strike(
            actor,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp,
                toughness,
                origin: Some(Action::Ultimate),
                scale: DamageScale::Attack,
            },
        );
    }

    pub(super) fn memosprite_fixed_base_hit(
        &mut self,
        sprite: usize,
        target: usize,
        element: Element,
        base_bp: u64,
        toughness: u16,
    ) {
        self.strike(
            sprite,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp: 0,
                toughness,
                origin: Some(Action::Skill),
                scale: DamageScale::FixedBase(base_bp),
            },
        );
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
        self.heal_unit_with_overflow(actor, target, amount);
    }

    pub(super) fn heal_unit_with_overflow(
        &mut self,
        actor: usize,
        target: usize,
        amount: u32,
    ) -> u32 {
        let amount = self.incoming_healing_amount(target, amount);
        let restored = self.restore_hp_unmodified(actor, target, amount);
        if restored > 0
            && let Some(rules) = self.mechanics(target)
        {
            rules.after_healing(self, target, restored);
        }
        if amount > 0 && self.is_targetable(target) && self.is_party_unit(target) {
            self.notify_ally_sustain(actor, target);
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(rules) = self.mechanics(owner)
                {
                    rules.on_party_healed(self, owner, target, amount);
                }
            }
        }
        amount
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
        if self.units[target].departed || !self.in_territory(target) {
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
            self.record_team_hp_loss(target, amount, None);
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

    fn record_team_hp_loss(&mut self, target: usize, amount: u32, received_attack: Option<u32>) {
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
                mechanics.on_party_hp_lost(self, owner, target, amount, received_attack);
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
        self.immune_to_debuffs(target)
            || self.use_survival_response_guard(target)
            || self
                .mechanics(target)
                .is_some_and(|mechanics| mechanics.prevent_debuff(self, target))
    }

    pub(super) fn resists_enemy_control(&mut self, actor: usize, target: usize) -> bool {
        if !self.enemies().contains(&actor) || !self.is_party_unit(target) {
            return false;
        }
        if self.immune_to_debuffs(target) {
            return true;
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

    pub(super) fn queue_special_follow_up(
        &mut self,
        actor: usize,
        origin: Action,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::Special,
                origin,
                attack,
            }));
    }

    pub(super) fn has_pending_special_follow_up(&self, actor: usize) -> bool {
        self.pending_follow_ups.iter().any(|pending| {
            matches!(pending, PendingAttack::FollowUp(queued)
                if queued.actor == actor && matches!(queued.targets, FollowUpTargets::Special))
        })
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
                    first_target: None,
                    hit_count,
                    adjacent_multiplier_bp,
                },
                origin,
                attack,
            }));
    }

    pub(super) fn follow_up_bouncing_from(
        &mut self,
        actor: usize,
        origin: Action,
        first_target: Option<usize>,
        hit_count: u8,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::FollowUp(FollowUp {
                actor,
                targets: FollowUpTargets::Random {
                    first_target,
                    hit_count,
                    adjacent_multiplier_bp: None,
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

    pub(super) fn queue_joint_follow_up(
        &mut self,
        participants: [(usize, JointFollowUpAttack); 2],
        origin: Action,
        restores_skill_points: u8,
    ) {
        self.pending_follow_ups
            .push_back(PendingAttack::JointFollowUp {
                participants,
                origin,
                restores_skill_points,
            });
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
        self.perform_nested_skill(actor, Some(target), ability, Action::Skill, None);
    }

    pub(super) fn perform_skill_copy(
        &mut self,
        owner: usize,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
        action: Action,
    ) {
        self.perform_nested_skill(actor, target, ability, action, Some(owner));
        // The main cast continues with its own per-attack identity.
        self.attack_index += 1;
    }

    fn perform_nested_skill(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
        action: Action,
        copy_source: Option<usize>,
    ) {
        let previous_action = self.action.replace(action);
        let previous_target = std::mem::replace(&mut self.action_target, target);
        let previous_nested = self.nested_skill.replace(super::NestedSkill {
            actor,
            copied_by: copy_source,
        });
        let previous_follow_up = std::mem::take(&mut self.resolving_follow_up);
        let previous_attack_targets = std::mem::take(&mut self.attack_targets);
        let previous_attack_implants = std::mem::take(&mut self.attack_implants);
        let previous_attack_damage = self.attack_damage.take();
        let previous_attack_spent_points = self.attack_spent_skill_points;
        let previous_super_break_targets = std::mem::take(&mut self.super_break_targets);
        let parent_debuff_targets = self.begin_debuff_action();
        self.begin_attack();
        self.emit(EventKind::TriggeredSkill, Some(actor), target);
        self.notify_ally_action_started(actor, target, ability);
        let handled = copy_source.is_some()
            && self
                .mechanics(actor)
                .is_some_and(|mechanics| mechanics.resolve_copied_skill(self, actor, target));
        if !handled {
            self.resolve_ability(actor, target, ability);
            self.notify_attack(actor, action, target);
        }
        if copy_source.is_some() {
            self.gain_energy(actor, actor, ability.energy_milli);
        }
        self.notify_ally_action_finished(actor, action);
        self.finish_debuff_action(actor, parent_debuff_targets);
        self.attack_targets = previous_attack_targets;
        self.attack_implants = previous_attack_implants;
        self.attack_damage = previous_attack_damage;
        self.attack_spent_skill_points = previous_attack_spent_points;
        self.super_break_targets = previous_super_break_targets;
        self.resolving_follow_up = previous_follow_up;
        self.nested_skill = previous_nested;
        self.action_target = previous_target;
        self.action = previous_action;
    }

    fn copied_skill_provider(&self, actor: usize) -> Option<usize> {
        self.nested_skill
            .filter(|skill| skill.actor == actor)
            .and_then(|skill| skill.copied_by)
            .filter(|&owner| owner != actor)
    }

    pub(super) fn resolve_follow_ups(&mut self) {
        while let Some(attack) = self.pending_follow_ups.pop_front() {
            let follow_up = match attack {
                PendingAttack::FollowUp(follow_up) => follow_up,
                PendingAttack::ElationSkillFromBanger { actor } => {
                    if self.is_targetable(actor)
                        && self.enemies().any(|target| self.is_targetable(target))
                    {
                        self.resolve_elation_skill(
                            actor,
                            super::elation::AhaInstant {
                                punchline: self.certified_banger(actor).unwrap_or(0),
                                skill_damage_factor_bp: 10_000,
                            },
                        );
                    }
                    continue;
                }
                PendingAttack::TriggeredSkill { actor, target } => {
                    if self.is_targetable(actor)
                        && let Some(rules) = self.mechanics(actor)
                    {
                        rules.on_triggered_skill(self, actor, target);
                    }
                    continue;
                }
                PendingAttack::TriggeredElationSkill { actor, punchline } => {
                    use super::elation::{AhaInstant, ElationSkillSource};

                    if self.wave_index + 1 < self.plan.wave_ends.len()
                        && self.enemies().all(|target| self.units[target].hp == 0)
                    {
                        self.pending_follow_ups
                            .push_front(PendingAttack::TriggeredElationSkill { actor, punchline });
                        break;
                    }
                    if self.is_targetable(actor)
                        && self.enemies().any(|target| self.is_targetable(target))
                    {
                        self.resolve_elation_skill_from(
                            actor,
                            AhaInstant {
                                punchline,
                                skill_damage_factor_bp: 10_000,
                            },
                            ElationSkillSource::Triggered,
                        );
                    }
                    continue;
                }
                PendingAttack::JointFollowUp {
                    participants,
                    origin,
                    restores_skill_points,
                } => {
                    self.resolve_joint_follow_up(participants, origin, restores_skill_points);
                    continue;
                }
            };
            let FollowUp {
                actor,
                targets,
                origin,
                attack,
            } = follow_up;
            if self.units[actor].hp == 0 || !self.in_territory(actor) {
                continue;
            }
            let mechanics = self.mechanics(actor);
            let special = matches!(targets, FollowUpTargets::Special);
            if !special
                && mechanics
                    .as_ref()
                    .is_some_and(|rules| !rules.can_execute_follow_up(self, actor))
            {
                continue;
            }
            let single = match targets {
                FollowUpTargets::All | FollowUpTargets::Random { .. } => None,
                FollowUpTargets::Special => {
                    let Some(unit) = mechanics
                        .as_ref()
                        .and_then(|rules| rules.special_follow_up_target(self, actor))
                    else {
                        continue;
                    };
                    Some(unit)
                }
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
            self.emit(EventKind::FollowUp, Some(actor), single);
            if !special && let Some(mechanics) = &mechanics {
                mechanics.before_follow_up(self, actor);
            }
            if special {
                if let (Some(mechanics), Some(target)) = (&mechanics, single) {
                    mechanics.resolve_special_follow_up(self, actor, target, origin);
                    self.gain_energy(actor, actor, attack.energy_milli);
                }
            } else if let FollowUpTargets::Random {
                mut first_target,
                hit_count,
                adjacent_multiplier_bp,
            } = targets
            {
                for _ in 0..hit_count {
                    let Some(target) = first_target
                        .take()
                        .filter(|&target| self.is_targetable(target))
                        .or_else(|| self.random_living_enemy())
                    else {
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
            if !special && let Some(mechanics) = mechanics {
                mechanics.after_follow_up(self, actor, origin);
            }
            self.notify_attack(actor, origin, single);
            self.finish_debuff_action(actor, parent_debuff_targets);
            self.resolving_follow_up = was_follow_up;
        }
    }

    fn resolve_joint_follow_up(
        &mut self,
        participants: [(usize, JointFollowUpAttack); 2],
        origin: Action,
        restores_skill_points: u8,
    ) {
        let [(source, first), (partner, second)] = participants;
        if participants
            .iter()
            .any(|(actor, _)| !self.is_targetable(*actor) || self.is_crowd_controlled(*actor))
            || !self.enemies().any(|target| self.is_targetable(target))
        {
            if let Some(rules) = self.mechanics(source) {
                rules.on_joint_follow_up_finished(self, source, partner, false);
            }
            return;
        }
        let was_follow_up = std::mem::replace(&mut self.resolving_follow_up, true);
        for (actor, other, attack) in [(source, partner, first), (partner, source, second)] {
            if !self.is_targetable(actor)
                || !self.enemies().any(|target| self.is_targetable(target))
            {
                continue;
            }
            let parent_debuff_targets = self.begin_debuff_action();
            self.begin_attack();
            self.emit(EventKind::FollowUp, Some(actor), None);
            for target in self.enemies() {
                self.follow_up_hit_with_multiplier(
                    actor,
                    target,
                    origin,
                    attack.element,
                    attack.multiplier_bp,
                    attack.toughness,
                );
            }
            self.gain_energy(actor, actor, attack.energy_milli);
            self.notify_attack_except_partner(actor, origin, None, Some(other));
            self.finish_debuff_action(actor, parent_debuff_targets);
        }
        self.resolving_follow_up = was_follow_up;
        self.restore_skill_points(source, restores_skill_points);
        if let Some(rules) = self.mechanics(source) {
            rules.on_joint_follow_up_finished(self, source, partner, true);
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
        self.follow_up_hit_with_multiplier(
            actor,
            target,
            origin,
            element,
            u32::from(multiplier_bp),
            toughness,
        );
    }

    pub(super) fn follow_up_hit_with_max_hp(
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
                scale: DamageScale::MaxHp,
            },
        );
    }

    fn follow_up_hit_with_multiplier(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        element: Element,
        multiplier_bp: u32,
        toughness: u16,
    ) {
        self.strike(
            actor,
            target,
            DamageKind::FollowUp,
            Hit {
                element,
                multiplier_bp,
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
        self.notify_attack_except_partner(actor, origin, primary_target, None);
    }

    fn notify_attack_except_partner(
        &mut self,
        actor: usize,
        origin: Action,
        primary_target: Option<usize>,
        joint_partner: Option<usize>,
    ) {
        let landed = !self.attack_targets.is_empty();
        if self.tracks_fixed_attack_toughness && landed {
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(mechanics) = self.mechanics(owner)
                    && mechanics.provides_fixed_attack_toughness()
                {
                    mechanics.before_super_breaks(self, owner, actor, origin);
                }
            }
        }
        let super_break_targets = std::mem::take(&mut self.super_break_targets);
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_attack_finished(self, owner, actor, origin);
                if landed && joint_partner != Some(owner) {
                    mechanics.on_ally_attack(self, owner, actor, origin, primary_target);
                }
            }
        }
        self.resolve_super_breaks(actor, origin, &super_break_targets);
        self.finish_dot_batch(DotTiming::Detonation);
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
        if !crossed_half.is_empty() {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.after_attack(self, owner, actor, &crossed_half, origin);
                }
            }
        }
        if landed && self.is_party_unit(actor) {
            let attacked_enemies: Vec<_> = self
                .attack_targets
                .iter()
                .copied()
                .filter(|&target| self.enemies().contains(&target))
                .collect();
            self.trigger_enemy_counter_stances(actor, &attacked_enemies);
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
            if let Some(rules) = self.mechanics(target) {
                rules.on_enemy_targeted(self, target);
            }
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

    pub(super) fn resolve_official_enemy(&mut self, actor: usize, kit: &OfficialEnemyKit) -> u8 {
        let living_teammates = self
            .enemies()
            .filter(|&unit| unit != actor && self.units[unit].hp > 0)
            .count();
        let Some(skill_index) =
            kit.select_next_skill(&mut self.units[actor].enemy_state, living_teammates)
        else {
            return 0;
        };
        let Some(skill) = kit.skills.get(skill_index) else {
            return 0;
        };
        match skill.action {
            EnemySkillAction::SingleAttack {
                element,
                multiplier_bp,
                sp_hit_milli,
                on_hit,
            } => {
                if let Some(target) = self.select_official_enemy_target(actor) {
                    self.resolve_official_enemy_single_hit(
                        actor,
                        target,
                        element,
                        multiplier_bp,
                        sp_hit_milli,
                        on_hit,
                    );
                }
                0
            }
            EnemySkillAction::SingleAttackPreferFrozen {
                element,
                multiplier_bp,
                frozen_bonus_bp,
                sp_hit_milli,
            } => {
                let enhanced = self.units[actor].enemy_state.enhanced_state;
                let frozen_target = if enhanced {
                    self.party_units()
                        .find(|&unit| self.can_enemy_target(unit) && self.is_frozen(unit))
                } else {
                    None
                };
                if let Some(target) =
                    frozen_target.or_else(|| self.select_official_enemy_target(actor))
                {
                    let was_frozen = enhanced && self.is_frozen(target);
                    self.resolve_official_enemy_single_hit(
                        actor,
                        target,
                        element,
                        multiplier_bp,
                        sp_hit_milli,
                        None,
                    );
                    if was_frozen && self.units[target].hp > 0 && frozen_bonus_bp > 0 {
                        self.strike(
                            actor,
                            target,
                            DamageKind::Additional,
                            Hit {
                                element,
                                multiplier_bp: frozen_bonus_bp,
                                toughness: 0,
                                origin: None,
                                scale: DamageScale::Attack,
                            },
                        );
                    }
                }
                0
            }
            EnemySkillAction::SelfBarrierAndSingleAttack {
                element,
                multiplier_bp,
                sp_hit_milli,
            } => {
                if !self.units[actor].enemy_state.barrier_active {
                    self.units[actor].enemy_state.barrier_active = true;
                    self.emit(
                        EventKind::StatusApplied(StatusKind::Barrier),
                        Some(actor),
                        Some(actor),
                    );
                }
                if let Some(target) = self.select_official_enemy_target(actor) {
                    self.resolve_official_enemy_single_hit(
                        actor,
                        target,
                        element,
                        multiplier_bp,
                        sp_hit_milli,
                        None,
                    );
                }
                0
            }
            EnemySkillAction::BlastAttack {
                element,
                main_multiplier_bp,
                adjacent_multiplier_bp,
                sp_hit_milli,
                on_hit,
            } => {
                if let Some(primary) = self.select_official_enemy_target(actor) {
                    let mut targets = vec![(primary, main_multiplier_bp)];
                    if primary < self.plan.ally_count {
                        if let Some(left) = primary.checked_sub(1)
                            && self.can_enemy_target(left)
                        {
                            targets.push((left, adjacent_multiplier_bp));
                        }
                        let right = primary + 1;
                        if right < self.plan.ally_count && self.can_enemy_target(right) {
                            targets.push((right, adjacent_multiplier_bp));
                        }
                    }
                    self.resolve_official_enemy_multi_hit(
                        actor,
                        primary,
                        &targets,
                        element,
                        sp_hit_milli,
                        on_hit,
                    );
                }
                0
            }
            EnemySkillAction::AllAttack {
                element,
                multiplier_bp,
                sp_hit_milli,
                on_hit,
            } => {
                let targets: Vec<_> = self
                    .party_units()
                    .filter(|&unit| self.can_enemy_target(unit))
                    .map(|unit| (unit, multiplier_bp))
                    .collect();
                if let Some(&(primary, _)) = targets.first() {
                    self.resolve_official_enemy_multi_hit(
                        actor,
                        primary,
                        &targets,
                        element,
                        sp_hit_milli,
                        on_hit,
                    );
                }
                0
            }
            EnemySkillAction::BounceAttack {
                element,
                primary_multiplier_bp,
                primary_sp_hit_milli,
                bounce_count,
                bounce_multiplier_bp,
                bounce_sp_hit_milli,
                on_hit,
            } => {
                if let Some(primary) = self.select_official_enemy_target(actor) {
                    self.resolve_official_enemy_bounce(
                        actor,
                        primary,
                        element,
                        primary_multiplier_bp,
                        primary_sp_hit_milli,
                        bounce_count,
                        bounce_multiplier_bp,
                        bounce_sp_hit_milli,
                        on_hit,
                    );
                }
                0
            }
            EnemySkillAction::HealAllEnemies { max_hp_ratio_bp } => {
                self.notify_active_ability(actor);
                let enemies: Vec<_> = self
                    .enemies()
                    .filter(|&unit| self.units[unit].hp > 0)
                    .collect();
                for ally_enemy in enemies {
                    let amount = percentage_of(self.effective_max_hp(ally_enemy), max_hp_ratio_bp);
                    self.heal_unit(actor, ally_enemy, amount);
                }
                self.finish_enemy_action();
                self.resolve_follow_ups();
                0
            }
            EnemySkillAction::EnterEnhancedState {
                target_phase,
                next_flag,
            } => {
                self.notify_active_ability(actor);
                let state = &mut self.units[actor].enemy_state;
                state.ai_phase = target_phase;
                state.ai_flag = next_flag;
                if !state.enhanced_state {
                    state.enhanced_state = true;
                    self.emit(
                        EventKind::StatusApplied(StatusKind::EnemyEnhanced),
                        Some(actor),
                        Some(actor),
                    );
                }
                self.finish_enemy_action();
                0
            }
            EnemySkillAction::SelfAttackBuff { attack_bonus_bp } => {
                self.notify_active_ability(actor);
                self.units[actor].enemy_state.attack_bonus_bp = self.units[actor]
                    .enemy_state
                    .attack_bonus_bp
                    .saturating_add(attack_bonus_bp);
                self.emit(
                    EventKind::StatusApplied(StatusKind::EnemyEnhanced),
                    Some(actor),
                    Some(actor),
                );
                self.finish_enemy_action();
                0
            }
            EnemySkillAction::LockSingleTarget => {
                self.notify_active_ability(actor);
                if let Some(target) = self.select_official_enemy_target(actor) {
                    self.units[actor].enemy_state.locked_target = Some(target);
                    self.emit(
                        EventKind::StatusApplied(StatusKind::TargetLock),
                        Some(actor),
                        Some(target),
                    );
                }
                self.finish_enemy_action();
                0
            }
            EnemySkillAction::ClearLockAndSingleAttack {
                element,
                multiplier_bp,
                sp_hit_milli,
                on_hit,
            } => {
                let target = self.select_official_enemy_target(actor);
                if let Some(locked) = self.units[actor].enemy_state.locked_target.take() {
                    self.emit(
                        EventKind::StatusExpired(StatusKind::TargetLock),
                        Some(actor),
                        Some(locked),
                    );
                }
                if let Some(target) = target {
                    self.resolve_official_enemy_single_hit(
                        actor,
                        target,
                        element,
                        multiplier_bp,
                        sp_hit_milli,
                        on_hit,
                    );
                }
                0
            }
            EnemySkillAction::EnterCounterStance { counter } => {
                self.notify_active_ability(actor);
                let state = &mut self.units[actor].enemy_state;
                state.counter_stance = true;
                state.counter_attack = Some(counter);
                self.emit(
                    EventKind::StatusApplied(StatusKind::CounterStance),
                    Some(actor),
                    Some(actor),
                );
                self.finish_enemy_action();
                0
            }
            EnemySkillAction::ClearCounterAndSingleAttack {
                element,
                multiplier_bp,
                sp_hit_milli,
            } => {
                if self.units[actor].enemy_state.counter_stance {
                    self.units[actor].enemy_state.counter_stance = false;
                    self.units[actor].enemy_state.counter_attack = None;
                    self.emit(
                        EventKind::StatusExpired(StatusKind::CounterStance),
                        Some(actor),
                        Some(actor),
                    );
                }
                if let Some(target) = self.select_official_enemy_target(actor) {
                    self.resolve_official_enemy_single_hit(
                        actor,
                        target,
                        element,
                        multiplier_bp,
                        sp_hit_milli,
                        None,
                    );
                }
                0
            }
            EnemySkillAction::SummonAllies {
                count,
                grant_extra_action,
            } => {
                self.notify_active_ability(actor);
                self.summon_official_enemies(actor, count);
                self.finish_enemy_action();
                u8::from(grant_extra_action)
            }
        }
    }

    fn summon_official_enemies(&mut self, actor: usize, count: u8) {
        let living = self
            .enemies()
            .filter(|&unit| self.units[unit].hp > 0)
            .count();
        let to_spawn = usize::from(count).min(5_usize.saturating_sub(living));
        let candidates: Vec<_> = self
            .enemies()
            .filter(|&unit| {
                unit != actor
                    && self.units[unit].enemy_state.dormant_summon
                    && self.units[unit].hp == 0
            })
            .take(to_spawn)
            .collect();
        for slot in candidates {
            let plan = &self.plan.units[slot];
            let max_hp = plan.stats.hp;
            let max_toughness = plan.kit.max_toughness();
            let initial_gauge = u64::try_from(
                u128::from(GAUGE_FULL) * u128::from(plan.kit.initial_delay_ratio_bp()) / 10_000,
            )
            .unwrap_or(GAUGE_FULL);
            let mut runtime = match &plan.kit {
                Kit::OfficialEnemy(kit) => kit.init_runtime_state(),
                _ => Default::default(),
            };
            runtime.dormant_summon = true;
            let state = &mut self.units[slot];
            state.hp = max_hp;
            state.toughness = max_toughness;
            state.gauge_quanta = initial_gauge;
            state.departed = false;
            state.advanced_to_front = false;
            state.frozen = None;
            state.debuffs.clear();
            state.skill_dots.clear();
            state.break_dots.clear();
            state.shocks.clear();
            state.entanglement = None;
            state.enemy_state = runtime;
        }
    }

    fn trigger_enemy_counter_stances(&mut self, attacker: usize, attacked_enemies: &[usize]) {
        if self.units[attacker].hp == 0 || !self.can_enemy_target(attacker) {
            return;
        }
        for &enemy in attacked_enemies {
            if self.units[attacker].hp == 0 || !self.can_enemy_target(attacker) {
                break;
            }
            if self.units[enemy].hp == 0
                || self.units[enemy].toughness == 0
                || self.is_crowd_controlled(enemy)
                || !self.units[enemy].enemy_state.counter_stance
            {
                continue;
            }
            let Some(counter) = self.units[enemy].enemy_state.counter_attack else {
                continue;
            };
            self.resolve_official_enemy_single_hit(
                enemy,
                attacker,
                counter.element,
                counter.multiplier_bp,
                counter.sp_hit_milli,
                None,
            );
        }
    }

    fn on_enemy_weakness_broken(&mut self, actor: usize, target: usize) {
        if !matches!(self.plan.units[target].kit, Kit::OfficialEnemy(_)) {
            return;
        }
        let state = &mut self.units[target].enemy_state;
        state.ai_phase = 1;
        state.ai_flag = 1;
        let had_enhanced = state.enhanced_state || state.attack_bonus_bp > 0;
        state.enhanced_state = false;
        state.attack_bonus_bp = 0;
        let locked = state.locked_target.take();
        let had_counter = state.counter_stance;
        state.counter_stance = false;
        state.counter_attack = None;
        if had_enhanced {
            self.emit(
                EventKind::StatusExpired(StatusKind::EnemyEnhanced),
                Some(actor),
                Some(target),
            );
        }
        if let Some(locked_unit) = locked {
            self.emit(
                EventKind::StatusExpired(StatusKind::TargetLock),
                Some(actor),
                Some(locked_unit),
            );
        }
        if had_counter {
            self.emit(
                EventKind::StatusExpired(StatusKind::CounterStance),
                Some(actor),
                Some(target),
            );
        }
    }

    fn select_official_enemy_target(&mut self, actor: usize) -> Option<usize> {
        let taunt = self.allies().find(|&owner| {
            self.is_targetable(owner)
                && self
                    .mechanics(owner)
                    .is_some_and(|rules| rules.taunts_enemy(self, owner, actor))
        });
        if taunt.is_some() {
            return taunt;
        }
        if let Some(locked) = self.units[actor].enemy_state.locked_target
            && self.can_enemy_target(locked)
        {
            return Some(locked);
        }
        self.random_enemy_target()
    }

    fn resolve_official_enemy_single_hit(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u32,
        sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    ) {
        let was_shielded = self.shield_amount(target) > 0;
        if let Some(rules) = self.mechanics(target) {
            rules.on_enemy_targeted(self, target);
        }
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
        self.strike(
            actor,
            target,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp,
                toughness: 0,
                origin: None,
                scale: DamageScale::Attack,
            },
        );
        self.apply_enemy_post_hit_effects(actor, target, on_hit);
        self.finish_enemy_action();
        let energy_recipient = self.memosprite_owner(target).unwrap_or(target);
        if self.units[energy_recipient].hp > 0 && sp_hit_milli > 0 {
            self.gain_energy(actor, energy_recipient, sp_hit_milli);
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

    fn resolve_official_enemy_multi_hit(
        &mut self,
        actor: usize,
        primary: usize,
        targets: &[(usize, u32)],
        element: Element,
        sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    ) {
        let shielded: Vec<_> = targets
            .iter()
            .map(|&(target, _)| (target, self.shield_amount(target) > 0))
            .collect();
        for &(target, _) in targets {
            if let Some(rules) = self.mechanics(target) {
                rules.on_enemy_targeted(self, target);
            }
        }
        self.notify_active_ability(actor);
        self.begin_attack();
        self.emit(EventKind::EnemyAttack, Some(actor), Some(primary));
        self.enemy_action = Some(actor);
        for &(target, _) in targets {
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.before_enemy_attack_seen(self, owner, target);
                }
            }
        }
        for &(target, multiplier_bp) in targets {
            if self.units[target].hp == 0 {
                continue;
            }
            self.strike(
                actor,
                target,
                DamageKind::Direct,
                Hit {
                    element,
                    multiplier_bp,
                    toughness: 0,
                    origin: None,
                    scale: DamageScale::Attack,
                },
            );
            self.apply_enemy_post_hit_effects(actor, target, on_hit);
        }
        self.finish_enemy_action();
        for &(target, was_shielded) in &shielded {
            let energy_recipient = self.memosprite_owner(target).unwrap_or(target);
            if self.units[energy_recipient].hp > 0 && sp_hit_milli > 0 {
                self.gain_energy(actor, energy_recipient, sp_hit_milli);
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
        }
        self.resolve_follow_ups();
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_official_enemy_bounce(
        &mut self,
        actor: usize,
        primary: usize,
        element: Element,
        primary_multiplier_bp: u32,
        primary_sp_hit_milli: u32,
        bounce_count: u8,
        bounce_multiplier_bp: u32,
        bounce_sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    ) {
        let mut hit_records: Vec<(usize, bool, u32)> = Vec::new();
        let mut record_hit = |battle: &mut Self, target: usize, sp_milli: u32| {
            if let Some((_, _, total_sp)) = hit_records
                .iter_mut()
                .find(|(recorded, _, _)| *recorded == target)
            {
                *total_sp = total_sp.saturating_add(sp_milli);
            } else {
                hit_records.push((target, battle.shield_amount(target) > 0, sp_milli));
            }
        };

        if let Some(rules) = self.mechanics(primary) {
            rules.on_enemy_targeted(self, primary);
        }
        record_hit(self, primary, primary_sp_hit_milli);
        self.notify_active_ability(actor);
        self.begin_attack();
        self.emit(EventKind::EnemyAttack, Some(actor), Some(primary));
        self.enemy_action = Some(actor);
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.before_enemy_attack_seen(self, owner, primary);
            }
        }
        self.strike(
            actor,
            primary,
            DamageKind::Direct,
            Hit {
                element,
                multiplier_bp: primary_multiplier_bp,
                toughness: 0,
                origin: None,
                scale: DamageScale::Attack,
            },
        );
        self.apply_enemy_post_hit_effects(actor, primary, on_hit);

        for _ in 0..bounce_count {
            let candidates: Vec<_> = self
                .party_units()
                .filter(|&unit| self.can_enemy_target(unit))
                .collect();
            if candidates.is_empty() {
                break;
            }
            let target = candidates[self.random_index(candidates.len())];
            record_hit(self, target, bounce_sp_hit_milli);
            self.strike(
                actor,
                target,
                DamageKind::Direct,
                Hit {
                    element,
                    multiplier_bp: bounce_multiplier_bp,
                    toughness: 0,
                    origin: None,
                    scale: DamageScale::Attack,
                },
            );
            self.apply_enemy_post_hit_effects(actor, target, on_hit);
        }
        self.finish_enemy_action();
        for (target, was_shielded, total_sp) in hit_records {
            let energy_recipient = self.memosprite_owner(target).unwrap_or(target);
            if self.units[energy_recipient].hp > 0 && total_sp > 0 {
                self.gain_energy(actor, energy_recipient, total_sp);
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
        }
        self.resolve_follow_ups();
    }

    fn apply_enemy_post_hit_effects(
        &mut self,
        actor: usize,
        target: usize,
        on_hit: Option<EnemyOnHit>,
    ) {
        if self.units[target].hp == 0 {
            return;
        }
        let upgraded_reverberation = self.trigger_reverberation_on_hit(actor, target);
        let Some(effect) = on_hit else {
            return;
        };
        match effect {
            EnemyOnHit::Debuff {
                kind,
                base_chance_bp,
                magnitude_bp,
                turns,
            } => {
                if self.roll_status(actor, target, base_chance_bp, false) {
                    let debuff_kind = match kind {
                        EnemyDebuffKind::DefenseDown => DebuffKind::DefenseDown,
                        EnemyDebuffKind::Slow => DebuffKind::Slow,
                        EnemyDebuffKind::Weaken => DebuffKind::Weaken,
                        EnemyDebuffKind::EffectResistanceDown => DebuffKind::EffectResistanceDown,
                    };
                    self.apply_debuff(actor, target, debuff_kind, magnitude_bp, turns);
                }
            }
            EnemyOnHit::ActionDelay {
                base_chance_bp,
                delay_bp,
            } => {
                if self.roll_status(actor, target, base_chance_bp, false) {
                    self.delay_action(actor, target, delay_bp);
                }
            }
            EnemyOnHit::Dot {
                element,
                scale,
                base_chance_bp,
                turns,
            } => {
                let damage = match scale {
                    EnemyDotScale::Attack(bp) => DotScale::Attack(bp),
                    EnemyDotScale::TargetMaxHp(hp_bp) => DotScale::TargetHp {
                        hp_bp,
                        attack_cap_bp: u16::MAX,
                    },
                };
                self.apply_skill_dot(
                    actor,
                    target,
                    base_chance_bp,
                    SkillDot {
                        element,
                        damage,
                        origin: Action::Basic,
                        turns,
                        max_stacks: 5,
                    },
                );
            }
            EnemyOnHit::Reverberation {
                base_chance_bp,
                turns,
            } => {
                if !upgraded_reverberation && self.roll_status(actor, target, base_chance_bp, false)
                {
                    self.apply_debuff(actor, target, DebuffKind::Reverberation, 0, turns);
                }
            }
            EnemyOnHit::Freeze {
                base_chance_bp,
                damage_multiplier_bp,
                turns,
            } => {
                self.apply_skill_freeze(
                    actor,
                    target,
                    base_chance_bp,
                    turns,
                    damage_multiplier_bp,
                    Action::Basic,
                );
            }
            EnemyOnHit::Entanglement {
                base_chance_bp,
                delay_bp,
                damage_multiplier_bp,
            } => {
                if self.roll_status(actor, target, base_chance_bp, true) {
                    self.apply_enemy_entanglement(actor, target, delay_bp, damage_multiplier_bp);
                }
            }
        }
    }

    pub(super) fn enemy_targeting_weight(&self, unit: usize) -> u64 {
        let weight =
            u32::from(self.plan.units[unit].stats.base_aggro) + self.lynx_aggro_bonus(unit);
        let ratio = if let Some(owner) = self.memosprite_owner(unit) {
            self.mechanics(owner)
                .map_or(10_000, |rules| rules.memosprite_aggro_multiplier_bp())
        } else {
            self.mechanics(unit)
                .map_or(10_000, |rules| rules.aggro_multiplier_bp(self, unit))
        };
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
                aura.vulnerability_bp = aura
                    .vulnerability_bp
                    .saturating_add(mechanics.target_vulnerability_bp(self, owner, target));
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

    fn ally_damage_factor_from(&self, owner: usize, actor: usize, event: &DamageEvent) -> u32 {
        let is_dot = event.kind == DamageKind::DamageOverTime && self.is_party_unit(actor);
        let is_memosprite = self.memosprite_owner(actor).is_some();
        let is_ultimate = event.kind == DamageKind::Direct && self.is_ultimate_damage(actor, event);
        if !is_dot && !is_memosprite && !is_ultimate {
            return 10_000;
        }
        let Some(rules) = self.mechanics(owner) else {
            return 10_000;
        };
        let dot_factor = if is_dot {
            rules.ally_dot_damage_factor_bp(self, owner)
        } else {
            10_000
        };
        let sprite_factor = if is_memosprite {
            rules.ally_memosprite_damage_factor_bp(self, owner)
        } else {
            10_000
        };
        let ultimate_factor = if is_ultimate {
            rules.ally_ultimate_damage_factor_bp(self, actor)
        } else {
            10_000
        };
        combine_damage_factors(
            combine_damage_factors(dot_factor, sprite_factor),
            ultimate_factor,
        )
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
        for status in state
            .debuffs
            .iter()
            .filter(|status| allow_source(status.source))
        {
            if let DebuffKind::BalefireBind { vulnerability_bp } = status.kind {
                vulnerability = vulnerability.saturating_add(u32::from(vulnerability_bp));
            }
        }
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
        let restricted_resistance_down = state
            .debuffs
            .iter()
            .find(|status| {
                status.kind == DebuffKind::RestrictedWeakness && allow_source(status.source)
            })
            .map_or(0, |status| {
                let base_resistance = u32::from(plan.stats.resist_bp[element.index()]);
                if base_resistance == 0 {
                    status.bp
                } else {
                    base_resistance
                }
            });
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
        let mut outgoing_damage_factor_bp = if event.kind == DamageKind::DamageOverTime
            || self.memosprite_owner(actor).is_some()
            || (event.kind == DamageKind::Direct && self.is_ultimate_damage(actor, event))
        {
            self.allies().filter(|&owner| allow_source(owner)).fold(
                outgoing_damage_factor_bp,
                |factor, owner| {
                    let support = self.ally_damage_factor_from(owner, actor, event);
                    combine_damage_factors(factor, support)
                },
            )
        } else {
            outgoing_damage_factor_bp
        };
        if !self.enemies().contains(&actor) {
            for status in &self.units[actor].debuffs {
                if status.kind == DebuffKind::Weaken {
                    outgoing_damage_factor_bp = combine_damage_factors(
                        outgoing_damage_factor_bp,
                        10_000_u32.saturating_sub(status.bp),
                    );
                }
            }
        }
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
            defense_down_bp: sum_matching(&DebuffKind::reduces_defense)
                .saturating_add(weightless_defense_down)
                .saturating_add(field_defense_down)
                .saturating_add(personal_defense_ignore)
                .saturating_add(u32::from(event.defense_ignore_bp)),
            resistance_down_bp: sum_matching(&|kind| kind.reduces_resistance_to(element))
                .saturating_add(restricted_resistance_down)
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

    pub(super) fn secondary_hit_from_fixed_base(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        base_bp: u64,
        origin: Action,
        provider: usize,
    ) {
        self.strike_scaled(
            actor,
            target,
            DamageKind::Additional,
            Hit {
                element,
                multiplier_bp: 10_000,
                toughness: 0,
                origin: Some(origin),
                scale: DamageScale::FixedBase(base_bp),
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
        self.resolve_elation_hit(actor, target, hit, true);
    }

    pub(super) fn elation_hit_using_actor_stats(
        &mut self,
        actor: usize,
        target: usize,
        hit: ElationHit,
    ) {
        self.resolve_elation_hit(actor, target, hit, false);
    }

    fn resolve_elation_hit(
        &mut self,
        actor: usize,
        target: usize,
        hit: ElationHit,
        allow_provider_elation: bool,
    ) {
        let bonus = self
            .mechanics(actor)
            .map_or(0, |rules| rules.elation_punchline_bonus(self, actor));
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
                    punchline: hit.punchline.saturating_add(bonus),
                    factor_bp: hit.factor_bp,
                    allow_provider_elation,
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
        let base_hp = self.units[hp_source]
            .memosprite_max_hp
            .unwrap_or(self.plan.units[hp_source].stats.hp);
        let max_hp_multiplier_bp = self.max_hp_multiplier_bp(hp_source);
        let uses_elation = matches!(
            scale,
            DamageScale::Elation { .. } | DamageScale::AttackWithElation(_)
        );
        let intrinsic_elation = if uses_elation {
            self.base_elation_bp(actor)
        } else {
            0
        };
        let fallback_elation = assisted_by
            .filter(|_| {
                uses_elation
                    && !matches!(
                        scale,
                        DamageScale::Elation {
                            allow_provider_elation: false,
                            ..
                        }
                    )
            })
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
                    let converted = mechanics.as_ref().map_or(0, |rules| {
                        rules.elation_from_attack_bp(view.attack).saturating_add(
                            rules.elation_from_crit_damage_bp(adjusted.crit_damage_bp),
                        )
                    });
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
                    ..
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
            let per_hit = if kind == DamageKind::FollowUp {
                mechanics
                    .as_ref()
                    .map_or(0, |rules| rules.follow_up_hit_super_break_bp())
            } else {
                0
            };
            let segment = SuperBreakSegment {
                base: toughness,
                personal_efficiency_bp: personal_toughness_bp,
                full_toughness: hit_weakness,
                unbroken: false,
                fixed_provider: None,
                precise_conversion: per_hit > 0,
            };
            self.apply_attack_toughness(
                actor,
                target,
                (element, segment),
                origin,
                source,
                match (is_attack, per_hit) {
                    (false, _) => SuperBreakConversion::None,
                    (true, 0) => SuperBreakConversion::Attack,
                    (true, bp) => SuperBreakConversion::FollowUp(bp),
                },
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
        source: Stats,
        conversion: SuperBreakConversion,
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
        let reduction = if segment.precise_conversion {
            let precision = segment.scaled_hundredths(team_toughness_bp, off_element_bp);
            let carried = precision + u32::from(self.units[target].toughness_reduction_remainder);
            let reduction = (carried / 100).min(u32::from(u16::MAX)) as u16;
            let remaining =
                u32::from(normal_before) + extra.map_or(0, |pool| u32::from(pool.remaining));
            // Carry fractional reductions between hits while a toughness pool is still live.
            self.units[target].toughness_reduction_remainder = if u32::from(reduction) < remaining {
                (carried % 100) as u8
            } else {
                0
            };
            reduction
        } else {
            segment.scaled(team_toughness_bp, off_element_bp)
        };
        if reduction == 0 {
            return;
        }
        let convert = if self.tracks_super_break
            && !matches!(conversion, SuperBreakConversion::None)
            && self.is_party_unit(actor)
            && self.enemies().contains(&target)
            && self.is_targetable(target)
            && (normal_before == 0
                || self.allies().any(|owner| {
                    self.is_targetable(owner)
                        && self
                            .mechanics(owner)
                            .is_some_and(|rules| rules.allows_unbroken_super_break(self, owner))
                }))
            && let Some(origin) = origin
        {
            segment.unbroken = normal_before > 0;
            self.record_super_break_segment(target, element, segment);
            Some(origin)
        } else {
            None
        };
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
        if self.units[target].toughness_reduction_remainder > 0
            && self.units[target].toughness == 0
            && self
                .live_extra_toughness(target)
                .is_none_or(|pool| pool.remaining == 0)
        {
            self.units[target].toughness_reduction_remainder = 0;
        }
        if let (Some(origin), SuperBreakConversion::FollowUp(bp)) = (convert, conversion)
            && self.is_targetable(target)
        {
            self.resolve_super_break_segments(
                (actor, source),
                origin,
                (target, element),
                &[segment],
                (actor, bp),
            );
        }
    }

    /// Adds fixed, weakness-ignoring toughness measured in hundredths of the combat scale.
    pub(super) fn fixed_attack_toughness(
        &mut self,
        (provider, actor): (usize, usize),
        target: usize,
        element: Element,
        origin: Action,
        toughness_hundredths: u16,
    ) {
        let mut source = self.plan.units[actor].stats;
        if let Some(rules) = self.mechanics(actor) {
            source.break_effect_bp = source
                .break_effect_bp
                .saturating_add(rules.break_effect_bonus_bp(self, actor));
        }
        self.apply_attack_toughness(
            actor,
            target,
            (
                element,
                SuperBreakSegment {
                    base: toughness_hundredths,
                    personal_efficiency_bp: 0,
                    full_toughness: true,
                    unbroken: false,
                    fixed_provider: Some(provider),
                    precise_conversion: true,
                },
            ),
            Some(origin),
            source,
            SuperBreakConversion::Attack,
        );
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
        self.on_enemy_weakness_broken(actor, target);
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
                self.resolve_super_break_segments(
                    (actor, source),
                    origin,
                    (*target, *element),
                    toughness_segments,
                    (owner, multiplier),
                );
            }
        }
    }

    fn resolve_super_break_segments(
        &mut self,
        (actor, source): (usize, Stats),
        origin: Action,
        (target, element): (usize, Element),
        segments: &[SuperBreakSegment],
        (owner, multiplier): (usize, u16),
    ) {
        let provider = self.mechanics(owner);
        let hp_before = self.units[target].hp;
        let mut event = Self::break_damage_event(origin, DamageKind::SuperBreak);
        event.assisted_by = Some(owner);
        self.resolve_damage(actor, target, element, event, |view| {
            let mut adjusted = source;
            adjusted.break_effect_bp = adjusted
                .break_effect_bp
                .saturating_add(view.bonus.break_effect_bp);
            let toughness = segments
                .iter()
                .map(|segment| segment.converted_hundredths(view.bonus))
                .fold(0, u32::saturating_add);
            let toughness = if view.target.toughness > 0 {
                (u64::from(toughness) * 9 / 10) as u32
            } else {
                toughness
            };
            let multiplier = provider.as_ref().map_or(multiplier, |rules| {
                rules.super_break_multiplier_for_effect(multiplier, adjusted.break_effect_bp)
            });
            damage::super_break(adjusted, view.target, element, toughness, multiplier)
        });
        self.record_half_hp_crossing(target, hp_before);
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
            unbroken: false,
            fixed_provider: None,
            precise_conversion: false,
        };
        self.apply_attack_toughness(
            actor,
            target,
            (element, segment),
            Some(origin),
            source,
            SuperBreakConversion::Attack,
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
        let assisted_by = assisted_by.or_else(|| self.copied_skill_provider(actor));
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
            ultimate: self.is_ultimate_damage(actor, &event),
            direct_skill: kind == DamageKind::Direct
                && matches!(origin, Some(Action::Skill | Action::EnhancedSkill)),
            summon: self.memosprite_owner(actor).is_some()
                || self
                    .mechanics(actor)
                    .is_some_and(|rules| rules.damage_comes_from_summon(self, actor, origin, kind))
                || assisted_by.is_some_and(|provider| {
                    kind == DamageKind::Additional
                        && self.mechanics(provider).is_some_and(|rules| {
                            rules.assisted_damage_comes_from_summon(self, provider)
                        })
                }),
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
            defender.outgoing_damage_factor_bp = combine_damage_factors(
                defender.outgoing_damage_factor_bp,
                10_000 + bonus.skill_damage_factor_bonus_bp,
            );
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
            if rolled_critical && let Some(rules) = &actor_mechanics {
                bonus.crit_damage_bp =
                    bonus
                        .crit_damage_bp
                        .saturating_add(rules.crit_rate_overflow_damage_bp(
                            self,
                            actor,
                            total_crit_rate,
                        ));
            }
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
                    status.kind.reduces_defense()
                        || matches!(
                            status.kind,
                            DebuffKind::SeamStitch
                                | DebuffKind::Vulnerability
                                | DebuffKind::WoesWhisper
                                | DebuffKind::Weightless { .. }
                                | DebuffKind::RestrictedWeakness
                        )
                        || self.epiphany_applies(status.kind, target)
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
                        || self.ally_damage_factor_from(owner, actor, &event) != 10_000
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
            self.record_aha_damage(actor, target, kind, amount);
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
                is_echo: self.resolving_dot_echoes,
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
        mut assists: Vec<(usize, u32)>,
    ) {
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            if let Some(owner) = self.copied_skill_provider(actor) {
                let remainder =
                    amount.saturating_sub(assists.iter().map(|&(_, amount)| amount).sum());
                if let Some((_, credited)) = assists.iter_mut().find(|entry| entry.0 == owner) {
                    *credited += remainder;
                } else if remainder > 0 {
                    assists.push((owner, remainder));
                }
            }
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
        remaining_bp = remaining_bp
            * 10_000_u32.saturating_sub(self.support_damage_reduction_bp(target))
            / 10_000;
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
                    let bp = mechanics.ally_damage_reduction_bp_for_target(self, owner, target);
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
        if self.units[target].enemy_state.barrier_active
            && !matches!(kind, DamageKind::DamageOverTime)
        {
            self.units[target].enemy_state.barrier_active = false;
            self.emit(
                EventKind::Damage {
                    kind,
                    amount: 0,
                    hp_loss: 0,
                    absorbed: amount,
                    critical,
                },
                Some(actor),
                Some(target),
            );
            self.emit(
                EventKind::StatusExpired(StatusKind::Barrier),
                Some(actor),
                Some(target),
            );
            return;
        }
        let shield_absorbed = self.shield_amount(target).min(amount);
        let mut absorbed = shield_absorbed;
        let mut incoming = amount - absorbed;
        if incoming > 0 && self.is_party_unit(target) {
            for owner in self.allies() {
                if self.is_targetable(owner)
                    && let Some(rules) = self.mechanics(owner)
                {
                    let blocked = rules
                        .absorb_ally_damage(self, owner, incoming)
                        .min(incoming);
                    incoming -= blocked;
                    absorbed += blocked;
                }
            }
        }
        let preserved_hp = u32::from(self.protect_from_fatal_hp_overflow(target, incoming));
        let state = &mut self.units[target];
        let hp_loss = state.hp.saturating_sub(preserved_hp).min(incoming);
        state.hp -= hp_loss;
        if matches!(
            self.plan.units[target].kit,
            Kit::Enemy(_) | Kit::OfficialEnemy(_)
        ) {
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
        self.record_aha_damage(actor, target, kind, amount);
        self.notify_ally_damage(actor, target, kind, hp_loss);
        self.accrue_elation_debt(actor, target, hp_loss);
        if shield_absorbed > 0 {
            self.deplete_shields(target, amount);
        }
        let attack_index = (self.enemy_action == Some(actor)
            && matches!(kind, DamageKind::Direct | DamageKind::FollowUp))
        .then_some(self.attack_index);
        self.record_team_hp_loss(target, hp_loss, attack_index);
        if amount > 0 {
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
        if self.has_fixed_zero_speed(target)
            || (advanced_to_front || after_quanta < self.gauge_quanta(target))
                && self.ally_advance_blocked(actor, target)
        {
            return;
        }
        let before_quanta = if self.territory_owner == Some(target)
            && let Some(countdown) = &mut self.units[target].countdown
        {
            countdown.advanced_to_front = advanced_to_front;
            std::mem::replace(&mut countdown.gauge_quanta, after_quanta)
        } else if let Some(owner) = self.memosprite_owner(target)
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
        if after > before {
            state.toughness_reduction_remainder = 0;
        }
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
            if amount_milli > 0 {
                self.notify_external_energy(actor, target);
            }
            return;
        }
        let gain = self.adjusted_energy_gain(target, amount_milli);
        self.restore_energy_flat(actor, target, gain);
    }

    pub(super) fn adjusted_energy_gain(&self, target: usize, amount_milli: u32) -> u32 {
        let bonus_bp = self.mechanics(target).map_or(0, |mechanics| {
            mechanics.energy_regeneration_bonus_bp(self, target)
        });
        let ratio = self.plan.units[target]
            .stats
            .energy_regeneration_bp
            .saturating_add(bonus_bp);
        saturating_stat(u64::from(amount_milli) * u64::from(ratio) / 10_000)
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
                if self.is_targetable(target)
                    && let Some(mechanics) = self.mechanics(target)
                {
                    mechanics.on_energy_gained(self, target, after_milli - before_milli);
                }
                for owner in self.allies() {
                    if self.is_targetable(owner)
                        && let Some(mechanics) = self.mechanics(owner)
                    {
                        mechanics.on_ally_energy_restored(self, owner, after_milli - before_milli);
                    }
                }
            }
            if target < self.plan.ally_count && before_milli < cap && after_milli >= cap {
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

fn combine_damage_factors(first_bp: u32, second_bp: u32) -> u32 {
    (u64::from(first_bp) * u64::from(second_bp) / 10_000).min(u64::from(u32::MAX)) as u32
}

fn saturating_stat(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub(super) fn gauge_for(basis_points: u16) -> u64 {
    GAUGE_FULL / 10_000 * u64::from(basis_points)
}
