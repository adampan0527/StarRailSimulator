use crate::{
    Action, DamageKind, EventKind, StatusKind,
    damage::{self, DamageTarget},
    fixture::{
        Ability, BREAK_DELAY_BP, Effect, Element, EnemyKit, EnemyTarget, GAUGE_FULL,
        KILL_ENERGY_MILLI, Target,
    },
};

use super::{
    Battle, DummyDamage, Kit, Status,
    combat_status::DebuffKind,
    periodic::{DotEvent, DotScale},
    support::{OffenseBuffs, OffenseContext},
};

struct Hit {
    element: Element,
    multiplier_bp: u16,
    toughness: u16,
    origin: Option<Action>,
    scale: DamageScale,
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
    crit_rate_bp: Option<u32>,
    origin: Option<Action>,
    assisted_by: Option<usize>,
    from_break: bool,
}

impl DamageEvent {
    fn is_ultimate_hit(&self) -> bool {
        self.kind == DamageKind::Direct && self.origin == Some(Action::Ultimate)
    }
}

#[derive(Clone, Copy)]
enum DamageScale {
    Attack,
    MaxHp,
    AttackAndDefense(u16),
    TargetHpCapped(u16),
}

impl DamageScale {
    fn base_bp(
        self,
        attack: u32,
        defense: u32,
        source_hp: u32,
        target_hp: u32,
        multiplier_bp: u16,
    ) -> u64 {
        let ratio = u64::from(multiplier_bp);
        match self {
            Self::Attack => u64::from(attack) * ratio,
            Self::MaxHp => u64::from(source_hp) * ratio,
            Self::AttackAndDefense(defense_bp) => {
                u64::from(attack) * ratio + u64::from(defense) * u64::from(defense_bp)
            }
            Self::TargetHpCapped(cap_bp) => {
                (u64::from(target_hp) * ratio).min(u64::from(attack) * u64::from(cap_bp))
            }
        }
    }
}

pub(super) struct PeriodicHit {
    pub element: Element,
    pub damage: DotScale,
    pub stacks: u8,
    pub ratio_bp: u16,
    pub origin: Action,
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

#[derive(Clone)]
pub(super) struct FollowUp {
    actor: usize,
    target: Option<usize>,
    origin: Action,
    attack: FollowUpAttack,
}

impl Battle<'_> {
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
                    Target::AllAllies => self.allies(),
                    Target::AllEnemies => self.enemies(),
                    Target::Ally | Target::Enemy => continue,
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
                Effect::BlastAdjacent { .. } | Effect::RandomHits { .. }
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
            Effect::MaxHpHit {
                element,
                multiplier_bp,
                toughness,
            } => self.attack_hit_scaled(
                actor,
                target,
                element,
                multiplier_bp,
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
                    let live: Vec<_> = self
                        .enemies()
                        .filter(|&unit| self.units[unit].hp > 0)
                        .collect();
                    if live.is_empty() {
                        break;
                    }
                    let selected = live[self.random_index(live.len())];
                    self.attack_hit(actor, selected, element, multiplier_bp, toughness);
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
            multiplier_bp,
            toughness,
            DamageScale::Attack,
        );
    }

    fn attack_hit_scaled(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        multiplier_bp: u16,
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

    pub(super) fn heal_unit(&mut self, actor: usize, target: usize, amount: u32) {
        let (hp, max_hp) = self.hp(target);
        let restored = amount.min(max_hp - hp);
        self.units[target].hp += restored;
        self.emit(
            EventKind::Heal { amount: restored },
            Some(actor),
            Some(target),
        );
    }

    pub(super) fn spend_hp(&mut self, actor: usize, amount: u32, minimum_remaining: u32) {
        let hp = &mut self.units[actor].hp;
        let amount = amount.min(hp.saturating_sub(minimum_remaining));
        *hp -= amount;
        if amount > 0 {
            self.emit(EventKind::HpSpent { amount }, Some(actor), Some(actor));
            self.notify_ally_hp_loss(actor);
        }
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
        let hit_rate = self.effective_effect_hit_rate_bp(actor);
        let resist_aura_bp = self
            .allies()
            .filter(|&ally| target < self.plan.ally_count && self.units[ally].hp > 0)
            .filter_map(|ally| self.mechanics(ally))
            .map(|mechanics| mechanics.effect_resistance_aura_bp())
            .fold(0_u32, u32::saturating_add);
        let resistance_down = self.units[target]
            .debuffs
            .iter()
            .filter(|status| status.kind == DebuffKind::EffectResistanceDown)
            .map(|status| status.bp)
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
        let remaining = self.units[actor].gauge_quanta.saturating_sub(gauge_for(bp));
        self.shift_gauge(actor, actor, remaining, remaining == 0);
    }

    pub(super) fn random_index(&mut self, contenders: usize) -> usize {
        if contenders < 2 {
            0
        } else {
            self.rng.below(contenders as u64) as usize
        }
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
        self.pending_follow_ups.push_back(FollowUp {
            actor,
            target: None,
            origin,
            attack: FollowUpAttack {
                element,
                multiplier_bp,
                toughness,
                energy_milli,
                first_hit: None,
            },
        });
    }

    pub(super) fn follow_up_single(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        attack: FollowUpAttack,
    ) {
        self.pending_follow_ups.push_back(FollowUp {
            actor,
            target: Some(target),
            origin,
            attack,
        });
    }

    pub(super) fn finish_attack(&mut self, actor: usize, origin: Action) {
        self.notify_attack(actor, origin);
        while let Some(follow_up) = self.pending_follow_ups.pop_front() {
            let FollowUp {
                actor,
                target,
                origin,
                attack,
            } = follow_up;
            let targets = target.map_or_else(|| self.enemies(), |target| target..target + 1);
            if self.units[actor].hp == 0 || !targets.clone().any(|unit| self.units[unit].hp > 0) {
                continue;
            }
            let was_follow_up = std::mem::replace(&mut self.resolving_follow_up, true);
            self.attack_targets.clear();
            let mechanics = self.mechanics(actor);
            self.emit(EventKind::FollowUp, Some(actor), target);
            for target in targets {
                if self.units[target].hp == 0 {
                    continue;
                }
                let (first_bp, first_toughness) = attack.first_hit.unwrap_or_default();
                let remaining = (
                    attack.multiplier_bp.saturating_sub(first_bp),
                    attack.toughness.saturating_sub(first_toughness),
                );
                for (multiplier_bp, toughness) in attack.first_hit.into_iter().chain([remaining]) {
                    self.strike(
                        actor,
                        target,
                        DamageKind::FollowUp,
                        Hit {
                            element: attack.element,
                            multiplier_bp,
                            toughness,
                            origin: Some(origin),
                            scale: DamageScale::Attack,
                        },
                    );
                }
                if let Some(mechanics) = &mechanics {
                    mechanics.after_follow_up_hit(self, actor, target, origin);
                }
            }
            self.gain_energy(actor, actor, attack.energy_milli);
            if let Some(mechanics) = mechanics {
                mechanics.after_follow_up(self, actor, origin);
            }
            self.notify_attack(actor, origin);
            self.resolving_follow_up = was_follow_up;
        }
    }

    fn notify_attack(&mut self, actor: usize, origin: Action) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_attack_finished(self, owner, actor, origin);
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
        let target = match kit.target {
            EnemyTarget::FirstLiving => self.allies().find(|&unit| self.units[unit].hp > 0),
            EnemyTarget::RandomLiving => self.random_enemy_target(),
        };
        if let Some(target) = target {
            let was_shielded = self.shield_amount(target) > 0;
            self.emit(EventKind::EnemyAttack, Some(actor), Some(target));
            self.hit(actor, target, Element::Physical, 10_000, 0);
            if self.units[target].hp > 0 {
                self.gain_energy(actor, target, kit.hit_energy_milli);
                if let Some(mechanics) = self.mechanics(target) {
                    mechanics.after_enemy_attack(self, target);
                }
            }
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.after_enemy_attack_seen(self, owner, actor, target, was_shielded);
                }
            }
        }
    }

    fn random_enemy_target(&mut self) -> Option<usize> {
        let weighted = self
            .allies()
            .filter(|&unit| self.units[unit].hp > 0)
            .map(|unit| {
                let weight =
                    u32::from(self.plan.units[unit].stats.base_aggro) + self.lynx_aggro_bonus(unit);
                (unit, u64::from(weight))
            })
            .collect::<Vec<_>>();
        let &(_, first_weight) = weighted.first()?;
        if weighted.iter().all(|&(_, weight)| weight == first_weight) {
            let slot = self.rng.below(weighted.len() as u64) as usize;
            return Some(weighted[slot].0);
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

    fn target_auras_from(&self, owner: usize, target: usize, event: &DamageEvent) -> (u32, u32) {
        if self.units[owner].hp == 0 || !self.enemies().contains(&target) {
            return (0, 0);
        }
        self.mechanics(owner).map_or((0, 0), |mechanics| {
            let vulnerability = if event.is_ultimate_hit() {
                mechanics.ultimate_vulnerability_bp(self, owner, target)
            } else {
                0
            };
            (
                vulnerability,
                mechanics.all_resistance_down_bp(self, owner, target),
            )
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
        let sum = |kind| {
            state
                .debuffs
                .iter()
                .filter(|status| status.kind == kind && allow_source(status.source))
                .map(|status| status.bp)
                .fold(0_u32, u32::saturating_add)
        };
        let legacy_vulnerability = state.statuses[StatusKind::Vulnerability.index()]
            .filter(|status| allow_source(status.source))
            .map_or(0, |status| status.magnitude);
        let mut vulnerability = legacy_vulnerability.saturating_add(sum(DebuffKind::Vulnerability));
        if event.kind == DamageKind::DamageOverTime {
            vulnerability = vulnerability.saturating_add(sum(DebuffKind::DotVulnerability));
        }
        if event.kind == DamageKind::WeaknessBreak {
            vulnerability = vulnerability.saturating_add(sum(DebuffKind::Besotted));
        }
        let mut field_resistance_down = 0_u32;
        for owner in self.allies().filter(|&owner| allow_source(owner)) {
            let (ultimate_vulnerability, resistance_down) =
                self.target_auras_from(owner, target, event);
            vulnerability = vulnerability.saturating_add(ultimate_vulnerability);
            field_resistance_down = field_resistance_down.saturating_add(resistance_down);
        }
        let personal_penetration = self.mechanics(actor).map_or(0, |mechanics| {
            mechanics.resistance_penetration_bp(self, actor, element)
        });
        let mut stats = plan.stats;
        stats.hp = self.effective_max_hp(target);
        stats.defense = self.defense(target);
        DamageTarget {
            stats,
            damage_reduction_bp: 0,
            vulnerability_bp: vulnerability,
            defense_down_bp: sum(DebuffKind::DefenseDown),
            resistance_down_bp: sum(DebuffKind::ResistanceDown(element))
                .saturating_add(sum(DebuffKind::AllResistanceDown))
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
                multiplier_bp,
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
                multiplier_bp,
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
                multiplier_bp,
                toughness: 0,
                origin: Some(origin),
                scale: DamageScale::Attack,
            },
            10_000,
            Some(provider),
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
                multiplier_bp,
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
                multiplier_bp,
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
        self.attack_targets.clear();
        self.emit(EventKind::FollowUp, Some(actor), Some(target));
        let neighbors: Vec<_> = self.adjacent_enemies(target).collect();
        let mut hit = |target, multiplier_bp, toughness, defense_multiplier_bp| {
            self.strike(
                actor,
                target,
                DamageKind::FollowUp,
                Hit {
                    element: counter.element,
                    multiplier_bp,
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
        self.finish_attack(actor, origin);
        self.resolving_follow_up = was_follow_up;
    }

    fn strike(&mut self, actor: usize, target: usize, kind: DamageKind, impact: Hit) -> bool {
        self.strike_scaled(actor, target, kind, impact, 10_000, None)
    }

    pub(super) fn periodic_hit(&mut self, actor: usize, target: usize, dot: PeriodicHit) {
        let (multiplier_bp, scale) = match dot.damage {
            DotScale::Attack(bp) => (
                bp.saturating_mul(u16::from(dot.stacks)),
                DamageScale::Attack,
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
                multiplier_bp,
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
        if matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
            && origin.is_some()
            && matches!(self.plan.units[actor].kit, Kit::Official(_))
            && self.enemies().contains(&target)
            && !self.attack_targets.contains(&target)
        {
            self.attack_targets.push(target);
            self.stack_entanglement(target);
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_attack_target(self, owner, target);
                }
            }
        }
        let mut source = self.plan.units[actor].stats;
        if element == Element::Thunder
            && let Some(bp) = source.thunder_damage_bp
        {
            source.element_damage_bp = bp;
        }
        source.element_damage_bp = source
            .element_damage_bp
            .saturating_add(self.units[actor].active_attack_bonus_bp);
        if let Some(mechanics) = self.mechanics(actor) {
            source.element_damage_bp = source
                .element_damage_bp
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
        let crit_rate = source
            .crit_rate_bp
            .saturating_add(self.units[actor].crit_rate_bonus_bp);
        let crit = self.resolve_damage(
            actor,
            target,
            element,
            DamageEvent {
                kind,
                crit_rate_bp: (kind != DamageKind::DamageOverTime).then_some(crit_rate),
                origin,
                assisted_by,
                from_break: false,
            },
            |view| {
                let mut adjusted = source;
                adjusted.element_damage_bp = adjusted
                    .element_damage_bp
                    .saturating_add(view.bonus.damage_bp);
                adjusted.crit_damage_bp = adjusted
                    .crit_damage_bp
                    .saturating_add(view.bonus.crit_damage_bp);
                let base_bp = scale.base_bp(
                    view.attack,
                    view.defense,
                    source.hp.saturating_add(view.bonus.max_hp_flat),
                    view.target.stats.hp,
                    multiplier_bp,
                ) * u64::from(ratio_bp)
                    / 10_000;
                damage::direct(adjusted, view.target, element, base_bp, view.critical)
            },
        );

        let hit_weakness = self.has_weakness(target, element);
        let before = self.units[target].toughness;
        if self.units[target].hp > 0 && hit_weakness && before > 0 && toughness > 0 {
            let after = before.saturating_sub(toughness);
            self.set_toughness(actor, target, after);
            if after == 0 {
                self.emit(EventKind::WeaknessBroken, Some(actor), Some(target));
                self.resolve_damage(
                    actor,
                    target,
                    element,
                    DamageEvent {
                        kind: DamageKind::WeaknessBreak,
                        crit_rate_bp: None,
                        origin,
                        assisted_by: None,
                        from_break: true,
                    },
                    |view| damage::weakness_break(source, view.target, element),
                );
                if self.units[target].hp > 0 {
                    let delayed = self.units[target].gauge_quanta + gauge_for(BREAK_DELAY_BP);
                    self.shift_gauge(actor, target, delayed, false);
                    if source.level == 80
                        && let Some(action) = origin
                    {
                        match element {
                            Element::Ice if self.roll_status(actor, target, 15_000, true) => {
                                self.apply_break_freeze(actor, target, action);
                            }
                            Element::Thunder => self.apply_break_shock(actor, target, action),
                            Element::Quantum if self.roll_status(actor, target, 15_000, true) => {
                                self.apply_entanglement(
                                    actor,
                                    target,
                                    action,
                                    source.break_effect_bp,
                                );
                            }
                            Element::Imaginary if self.roll_status(actor, target, 15_000, true) => {
                                self.apply_imprisonment(actor, target, source.break_effect_bp);
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
        }
        if crit
            && kind != DamageKind::Direct
            && let Some(action) = origin
            && let Some(mechanics) = self.mechanics(actor)
        {
            mechanics.after_non_primary_critical_hit(self, actor, target, action, kind);
        }
        let (hp, max_hp) = self.hp(target);
        if origin.is_some()
            && (self.action.is_some() || kind == DamageKind::FollowUp)
            && kind != DamageKind::DamageOverTime
            && self.enemies().contains(&target)
            && hp_before > max_hp / 2
            && hp <= max_hp / 2
        {
            self.crossed_half.push(target);
        }
        crit
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

    pub(super) fn break_status_hit_scaled(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        origin: Action,
        kind: DamageKind,
        ratio_bp: u16,
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
        self.resolve_damage(
            actor,
            target,
            element,
            DamageEvent {
                kind,
                crit_rate_bp: None,
                origin: Some(origin),
                assisted_by: None,
                from_break: true,
            },
            |view| damage::break_status(source, view.target, element, ratio_bp),
        );
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
            crit_rate_bp,
            origin,
            assisted_by,
            from_break,
        } = event;
        let offense = OffenseContext {
            actor,
            enemy: self.enemies().contains(&target).then_some(target),
            element,
            kind,
        };
        let incoming_reduction_bp = self.mechanics(target).map_or(0, |mechanics| {
            mechanics.incoming_damage_reduction_bp(self, target, actor, kind)
        });
        let invigoration_bp = self.invigoration_damage_reduction_bp(target);
        let base_remaining_bp =
            (10_000 - incoming_reduction_bp.min(10_000)) * (10_000 - invigoration_bp) / 10_000;
        let remaining_bp = if self.enemies().contains(&actor) {
            self.allies()
                .filter(|&owner| self.units[owner].hp > 0)
                .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
                .fold(base_remaining_bp, |bp, (owner, rules)| {
                    bp * 10_000_u32.saturating_sub(rules.enemy_damage_reduction_bp(self, owner))
                        / 10_000
                })
        } else {
            base_remaining_bp
        };
        let reduction_bp = 10_000 - remaining_bp;
        let applied_buffs = self.offense_buffs(offense, |_| true);
        // Compare the same roll with each counterfactual crit rate during attribution.
        let crit_roll = crit_rate_bp
            .filter(|rate| rate.saturating_add(applied_buffs.crit_rate_bp) > 0)
            .map(|rate| (rate, self.rng.below(10_000)));
        let damage_with = |bonus: OffenseBuffs, allow: &dyn Fn(usize) -> bool| {
            let mut defender = self.damage_target(actor, target, element, &event, allow);
            defender.damage_reduction_bp = reduction_bp;
            let critical = crit_roll.is_some_and(|(rate, roll)| {
                roll < u64::from(rate.saturating_add(bonus.crit_rate_bp))
            });
            let amount = calculate(DamageView {
                target: defender,
                bonus,
                attack: self.attack_with_buffs(actor, bonus),
                defense: self.defense_with_buff(actor, bonus.defense_bp),
                critical,
            });
            (amount, critical)
        };
        let (amount, critical) = damage_with(applied_buffs, &|_| true);
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            let state = &self.units[target];
            let assists: std::collections::BTreeSet<_> = state
                .debuffs
                .iter()
                .filter(|status| {
                    matches!(
                        status.kind,
                        DebuffKind::DefenseDown
                            | DebuffKind::Vulnerability
                            | DebuffKind::AllResistanceDown
                    ) || status.kind == DebuffKind::ResistanceDown(element)
                        || (status.kind == DebuffKind::DotVulnerability
                            && kind == DamageKind::DamageOverTime)
                        || (status.kind == DebuffKind::Besotted
                            && kind == DamageKind::WeaknessBreak)
                })
                .map(|status| status.source)
                .chain(
                    state.statuses[StatusKind::Vulnerability.index()].map(|status| status.source),
                )
                .chain(self.allies().filter(|&owner| {
                    self.offense_buffs_from(owner, offense) != OffenseBuffs::default()
                        || self.target_auras_from(owner, target, &event) != (0, 0)
                }))
                .filter(|&source| source != actor && source < self.plan.ally_count)
                .collect();
            let mut credits = Vec::new();
            if !assists.is_empty() {
                let damage_by = |allow: &dyn Fn(usize) -> bool| {
                    damage_with(self.offense_buffs(offense, allow), allow).0
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
            self.deal_damage(actor, target, kind, amount, critical);
        }
        if amount > 0 && kind == DamageKind::DamageOverTime && offense.enemy.is_some() {
            let dot = DotEvent {
                source: actor,
                target,
                element,
                from_break,
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

    fn deal_damage(
        &mut self,
        actor: usize,
        target: usize,
        kind: DamageKind,
        amount: u32,
        critical: bool,
    ) {
        let absorbed = self.shield_amount(target).min(amount);
        let state = &mut self.units[target];
        let hp_loss = state.hp.min(amount - absorbed);
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
        if absorbed > 0 {
            self.deplete_shields(target, amount);
        }
        if hp_loss > 0
            && let Some(mechanics) = self.mechanics(target)
        {
            mechanics.after_hp_loss(self, target, hp_loss);
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
            if actor >= self.plan.ally_count
                && target < self.plan.ally_count
                && matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
                && self.try_ally_rescue(target)
            {
                return;
            }
            self.emit(EventKind::UnitDefeated, Some(actor), Some(target));
            if actor < self.plan.ally_count {
                self.gain_energy(actor, actor, KILL_ENERGY_MILLI);
                if let Some(mechanics) = self.mechanics(actor) {
                    mechanics.after_defeat(self, actor, target);
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
            }
            self.units[target].statuses.fill(None);
            self.units[target].shields.clear();
            self.units[target].shocks.clear();
            self.units[target].frozen = None;
            self.units[target].debuffs.clear();
            self.units[target].break_dots.clear();
            self.units[target].entanglement = None;
            self.units[target].lynx_response = None;
            self.units[target].skill_dots.clear();
            self.units[target].heals_over_time.clear();
            self.units[target].invigoration = None;
            self.units[target].ally_buffs.clear();
        }
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
        let before_quanta = std::mem::replace(&mut self.units[target].gauge_quanta, after_quanta);
        self.units[target].advanced_to_front = advanced_to_front;
        self.emit(
            EventKind::GaugeShifted {
                before_quanta,
                after_quanta,
            },
            Some(actor),
            Some(target),
        );
    }

    fn apply_imprisonment(&mut self, actor: usize, target: usize, break_effect_bp: u32) {
        if self.apply_break_slow(actor, target) {
            let extra_delay = gauge_for(3_000) / 10_000 * (10_000 + u64::from(break_effect_bp));
            let delayed = self.units[target].gauge_quanta + extra_delay;
            self.shift_gauge(actor, target, delayed, false);
        }
    }

    pub(super) fn set_toughness(&mut self, actor: usize, target: usize, after: u16) {
        let before = std::mem::replace(&mut self.units[target].toughness, after);
        self.emit(
            EventKind::Toughness { before, after },
            Some(actor),
            Some(target),
        );
    }

    fn apply_status(
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
        self.emit(EventKind::StatusApplied(status), Some(actor), Some(target));
    }

    pub(super) fn finish_turn(&mut self, actor: usize) {
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
    }

    pub(super) fn gain_energy(&mut self, actor: usize, target: usize, amount_milli: u32) {
        let cap = self.plan.units[target].kit.max_energy_milli();
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
