use crate::{
    Action, DamageKind, EventKind, StatusKind,
    damage::{self, DamageTarget},
    fixture::{
        Ability, BREAK_DELAY_BP, Effect, Element, EnemyKit, EnemyTarget, GAUGE_FULL,
        KILL_ENERGY_MILLI, Target,
    },
};

use super::{Battle, DummyDamage, Kit, Status, combat_status::DebuffKind};

struct Hit {
    element: Element,
    multiplier_bp: u16,
    toughness: u16,
    origin: Option<Action>,
    scales_with_hp: bool,
}

#[derive(Clone)]
pub(super) struct FollowUp {
    actor: usize,
    element: Element,
    multiplier_bp: u16,
    toughness: u16,
    origin: Action,
    energy_milli: u32,
}

impl Battle<'_> {
    pub(super) fn resolve_ability(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        for effect in ability.effects {
            if let Some(target) = target {
                self.apply_effect(actor, target, *effect);
            } else {
                let targets = match ability.target {
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
        if self.units[target].hp == 0 && !matches!(effect, Effect::BlastAdjacent { .. }) {
            return;
        }
        match effect {
            Effect::Hit {
                element,
                multiplier_bp,
                toughness,
            } => {
                let hp_before = self.units[target].hp;
                let critical = self.hit(actor, target, element, multiplier_bp, toughness);
                if let Some(action) = self.action
                    && let Some(mechanics) = self.mechanics(actor)
                {
                    if critical {
                        mechanics.after_critical_hit(self, actor, target, action);
                    }
                    mechanics.after_attack_hit(self, actor, target, action, hp_before);
                }
            }
            Effect::BlastAdjacent {
                element,
                multiplier_bp,
                toughness,
            } => {
                let multiplier_bp = self.mechanics(actor).map_or(multiplier_bp, |mechanics| {
                    mechanics.blast_multiplier_bp(self, actor, self.action, multiplier_bp)
                });
                for neighbor in self.adjacent_enemies(target) {
                    self.hit(actor, neighbor, element, multiplier_bp, toughness);
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

    pub(super) fn roll_effect(
        &mut self,
        actor: usize,
        _target: usize,
        base_chance_bp: u32,
    ) -> bool {
        let aura: u32 = self
            .allies()
            .filter(|&ally| self.units[ally].hp > 0)
            .filter_map(|ally| self.mechanics(ally))
            .map(|mechanics| mechanics.effect_hit_rate_aura_bp())
            .sum();
        let effect_hit_rate = self.plan.units[actor]
            .stats
            .effect_hit_rate_bp
            .saturating_add(aura);
        let chance = u64::from(base_chance_bp) * (10_000 + u64::from(effect_hit_rate)) / 10_000;
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
        self.shift_gauge(actor, actor, 0, true);
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
            element,
            multiplier_bp,
            toughness,
            origin,
            energy_milli,
        });
    }

    pub(super) fn finish_attack(&mut self, actor: usize, origin: Action) {
        self.notify_attack(actor, origin);
        while let Some(follow_up) = self.pending_follow_ups.pop_front() {
            let FollowUp {
                actor,
                element,
                multiplier_bp,
                toughness,
                origin,
                energy_milli,
            } = follow_up;
            if self.units[actor].hp == 0 || !self.enemies().any(|unit| self.units[unit].hp > 0) {
                continue;
            }
            self.emit(EventKind::FollowUp, Some(actor), None);
            for target in self.enemies() {
                self.strike(
                    actor,
                    target,
                    DamageKind::FollowUp,
                    Hit {
                        element,
                        multiplier_bp,
                        toughness,
                        origin: Some(origin),
                        scales_with_hp: false,
                    },
                );
            }
            self.gain_energy(actor, actor, energy_milli);
            self.notify_attack(actor, origin);
        }
    }

    fn notify_attack(&mut self, actor: usize, origin: Action) {
        if self.crossed_half.is_empty() {
            return;
        }
        let crossed_half = std::mem::take(&mut self.crossed_half);
        for owner in self
            .allies()
            .filter(|&owner| self.units[owner].hp > 0)
            .collect::<Vec<_>>()
        {
            if let Some(mechanics) = self.mechanics(owner) {
                mechanics.after_attack(self, owner, actor, &crossed_half, origin);
            }
        }
    }

    pub(super) fn resolve_enemy(&mut self, actor: usize, kit: &EnemyKit) {
        let count = self
            .allies()
            .filter(|&unit| self.units[unit].hp > 0)
            .count();
        if count == 0 {
            return;
        }
        let slot = match kit.target {
            EnemyTarget::FirstLiving => 0,
            EnemyTarget::RandomLiving => self.rng.below(count as u64) as usize,
        };
        if let Some(target) = self
            .allies()
            .filter(|&unit| self.units[unit].hp > 0)
            .nth(slot)
        {
            self.emit(EventKind::EnemyAttack, Some(actor), Some(target));
            self.hit(actor, target, Element::Physical, 10_000, 0);
            if self.units[target].hp > 0 {
                self.gain_energy(actor, target, kit.hit_energy_milli);
                if let Some(mechanics) = self.mechanics(target) {
                    mechanics.after_enemy_attack(self, target);
                }
            }
        }
    }

    fn damage_target(
        &self,
        actor: usize,
        target: usize,
        element: Element,
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
        let vulnerability = state.statuses[StatusKind::Vulnerability.index()]
            .filter(|status| allow_source(status.source))
            .map_or(0, |status| status.magnitude);
        DamageTarget {
            stats: plan.stats,
            vulnerability_bp: vulnerability,
            defense_down_bp: sum(DebuffKind::DefenseDown),
            resistance_down_bp: sum(DebuffKind::ResistanceDown(element)).saturating_add(
                if element == Element::Wind {
                    self.units[actor].active_penetration_bp
                } else {
                    0
                },
            ),
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
                scales_with_hp: false,
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
                scales_with_hp: false,
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
                multiplier_bp,
                toughness: 0,
                origin: Some(origin),
                scales_with_hp: true,
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
                scales_with_hp: false,
            },
        );
    }

    fn strike(&mut self, actor: usize, target: usize, kind: DamageKind, impact: Hit) -> bool {
        let Hit {
            element,
            mut multiplier_bp,
            toughness,
            origin,
            scales_with_hp,
        } = impact;
        let hp_before = self.units[target].hp;
        if hp_before == 0 {
            return false;
        }
        let mut source = self.plan.units[actor].stats;
        if let Some((bp, _)) = self.units[actor].attack_bonus {
            let boost = u64::from(source.base_attack) * u64::from(bp) / 10_000;
            source.attack = source.attack.saturating_add(saturating_stat(boost));
        }
        if scales_with_hp {
            source.attack = source.hp;
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
            multiplier_bp =
                mechanics.damage_multiplier_bp(self, actor, target, origin, kind, multiplier_bp);
        }
        let crit_rate = source
            .crit_rate_bp
            .saturating_add(self.units[actor].crit_rate_bonus_bp);
        let crit = kind != DamageKind::DamageOverTime
            && crit_rate > 0
            && self.rng.below(10_000) < u64::from(crit_rate);
        self.resolve_damage(actor, target, element, (kind, crit, origin), |defender| {
            damage::direct(source, defender, element, multiplier_bp, crit)
        });

        let hit_weakness = self.plan.units[target].kit.matches_weakness(element);
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
                    (DamageKind::WeaknessBreak, false, origin),
                    |defender| damage::weakness_break(source, defender, element),
                );
                if self.units[target].hp > 0 {
                    let delayed = self.units[target].gauge_quanta + gauge_for(BREAK_DELAY_BP);
                    self.shift_gauge(actor, target, delayed, false);
                    if source.level == 80
                        && let Some(action) = origin
                    {
                        match element {
                            Element::Ice if self.roll_effect(actor, target, 15_000) => {
                                self.units[target].frozen = Some((actor, action));
                            }
                            Element::Thunder => self.apply_break_shock(actor, target, action),
                            Element::Wind | Element::Physical
                                if self.roll_effect(actor, target, 15_000) =>
                            {
                                self.apply_break_dot(actor, target, element, action)
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        let (hp, max_hp) = self.hp(target);
        if self.action.is_some()
            && kind != DamageKind::DamageOverTime
            && self.enemies().contains(&target)
            && hp_before > max_hp / 2
            && hp <= max_hp / 2
        {
            self.crossed_half.push(target);
        }
        crit
    }

    pub(super) fn thaw(&mut self, target: usize) -> bool {
        let Some((actor, origin)) = self.units[target].frozen.take() else {
            return false;
        };
        self.break_status_hit(actor, target, Element::Ice, origin, DamageKind::Additional);
        self.shift_gauge(actor, target, GAUGE_FULL / 2, false);
        true
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
        let source = self.plan.units[actor].stats;
        self.resolve_damage(
            actor,
            target,
            element,
            (kind, false, Some(origin)),
            |defender| damage::break_status(source, defender, element),
        );
    }

    fn resolve_damage(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        (kind, critical, origin): (DamageKind, bool, Option<Action>),
        calculate: impl Fn(DamageTarget) -> u32,
    ) {
        let defender = self.damage_target(actor, target, element, |_| true);
        let amount = calculate(defender);
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            let state = &self.units[target];
            let assists: std::collections::BTreeSet<_> = state
                .debuffs
                .iter()
                .filter(|status| {
                    matches!(status.kind, DebuffKind::DefenseDown)
                        || status.kind == DebuffKind::ResistanceDown(element)
                })
                .map(|status| status.source)
                .chain(
                    state.statuses[StatusKind::Vulnerability.index()].map(|status| status.source),
                )
                .filter(|&source| source != actor && source < self.plan.ally_count)
                .collect();
            let mut credits = Vec::new();
            if !assists.is_empty() {
                let mut damage =
                    calculate(self.damage_target(actor, target, element, |source| {
                        !assists.contains(&source)
                    }));
                for &owner in &assists {
                    let adjusted =
                        calculate(self.damage_target(actor, target, element, |source| {
                            source <= owner || !assists.contains(&source)
                        }));
                    if adjusted > damage {
                        credits.push((owner, adjusted - damage));
                    }
                    damage = adjusted;
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
    }

    fn deal_damage(
        &mut self,
        actor: usize,
        target: usize,
        kind: DamageKind,
        amount: u32,
        critical: bool,
    ) {
        let state = &mut self.units[target];
        let shield = &mut state.statuses[StatusKind::Shield.index()];
        let mut absorbed = 0;
        let mut shield_source = None;
        if let Some(status) = shield {
            absorbed = status.magnitude.min(amount);
            status.magnitude -= absorbed;
            if status.magnitude == 0 {
                shield_source = Some(status.source);
                *shield = None;
            }
        }
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
        if let Some(source) = shield_source {
            self.emit(EventKind::ShieldDepleted, Some(source), Some(target));
        }
        if self.units[target].hp == 0 {
            self.units[target].statuses = [None; 6];
            self.units[target].shocks.clear();
            self.units[target].frozen = None;
            self.units[target].debuffs.clear();
            self.units[target].break_dots.clear();
            self.units[target].heals_over_time.clear();
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
        }
    }

    fn shift_gauge(
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
    }

    pub(super) fn gain_energy(&mut self, actor: usize, target: usize, amount_milli: u32) {
        let cap = self.plan.units[target].kit.max_energy_milli();
        let ratio = self.plan.units[target].stats.energy_regeneration_bp;
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
        }
    }
}

fn saturating_stat(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn gauge_for(basis_points: u16) -> u64 {
    GAUGE_FULL / 10_000 * u64::from(basis_points)
}
