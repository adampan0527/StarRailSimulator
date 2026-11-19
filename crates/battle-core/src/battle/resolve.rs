use crate::{
    DamageKind, EventKind, StatusKind,
    damage::{self, DamageTarget},
    fixture::{
        Ability, BREAK_DELAY_BP, Effect, Element, EnemyKit, EnemyTarget, GAUGE_FULL,
        KILL_ENERGY_MILLI, Target,
    },
};

use super::{Battle, DummyDamage, Kit, Status};

impl Battle<'_> {
    pub(super) fn resolve_ability(
        &mut self,
        actor: usize,
        target: Option<usize>,
        ability: Ability,
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
        if self.units[target].hp == 0 {
            return;
        }
        match effect {
            Effect::Hit {
                element,
                multiplier_bp,
                toughness,
            } => self.hit(actor, target, element, multiplier_bp, toughness),
            Effect::Shield { amount, turns } => {
                self.apply_status(actor, target, StatusKind::Shield, amount, turns)
            }
            Effect::Vulnerability { bp, turns } => {
                self.apply_status(actor, target, StatusKind::Vulnerability, bp, turns)
            }
            Effect::SpeedUp { bp, turns } => {
                self.apply_status(actor, target, StatusKind::SpeedUp, bp, turns)
            }
            Effect::Heal(amount) => {
                let max_hp = self.plan.units[target].stats.hp;
                let hp = &mut self.units[target].hp;
                let restored = amount.min(max_hp - *hp);
                *hp += restored;
                self.emit(
                    EventKind::Heal { amount: restored },
                    Some(actor),
                    Some(target),
                );
            }
            Effect::Advance(bp) => {
                let gauge = self.units[target]
                    .gauge_quanta
                    .saturating_sub(gauge_for(bp));
                self.shift_gauge(actor, target, gauge, gauge == 0);
            }
            Effect::ImmediateTurn => self.shift_gauge(actor, target, 0, true),
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
            }
        }
    }

    fn damage_target(&self, target: usize) -> DamageTarget {
        let plan = &self.plan.units[target];
        DamageTarget {
            stats: plan.stats,
            vulnerability_bp: self.units[target].magnitude(StatusKind::Vulnerability),
            toughness: self.units[target].toughness,
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
    ) {
        let source = self.plan.units[actor].stats;
        let crit =
            source.crit_rate_bp > 0 && self.rng.below(10_000) < u64::from(source.crit_rate_bp);
        self.resolve_damage(actor, target, DamageKind::Direct, crit, |defender| {
            damage::direct(source, defender, element, multiplier_bp, crit)
        });

        let hit_weakness = self.plan.units[target].kit.matches_weakness(element);
        let before = self.units[target].toughness;
        if self.units[target].hp == 0 || !hit_weakness || before == 0 || toughness == 0 {
            return;
        }
        let after = before.saturating_sub(toughness);
        self.set_toughness(actor, target, after);
        if after == 0 {
            self.emit(EventKind::WeaknessBroken, Some(actor), Some(target));
            self.resolve_damage(
                actor,
                target,
                DamageKind::WeaknessBreak,
                false,
                |defender| damage::weakness_break(source, defender, element),
            );
            if self.units[target].hp > 0 {
                let delayed = self.units[target].gauge_quanta + gauge_for(BREAK_DELAY_BP);
                self.shift_gauge(actor, target, delayed, false);
            }
        }
    }

    fn resolve_damage(
        &mut self,
        actor: usize,
        target: usize,
        kind: DamageKind,
        critical: bool,
        calculate: impl Fn(DamageTarget) -> u32,
    ) {
        let defender = self.damage_target(target);
        let amount = calculate(defender);
        if matches!(self.plan.units[target].kit, Kit::Dummy { .. }) {
            let assist = self.units[target].statuses[StatusKind::Vulnerability.index()]
                .filter(|status| status.source != actor && status.source < self.plan.ally_count)
                .map(|status| {
                    let without_assist = calculate(DamageTarget {
                        vulnerability_bp: 0,
                        ..defender
                    });
                    (status.source, amount - without_assist)
                });
            self.dummy_damage.push(DummyDamage {
                av_micros: self.av_micros,
                actor,
                target,
                kind,
                critical,
                amount,
                assist,
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
            self.units[target].statuses = [None; 3];
            self.emit(EventKind::UnitDefeated, Some(actor), Some(target));
            if matches!(self.plan.units[actor].kit, Kit::Ally(_)) {
                self.gain_energy(actor, actor, KILL_ENERGY_MILLI);
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
        let after = cap.min(self.units[target].energy_milli + amount_milli);
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

fn gauge_for(basis_points: u16) -> u64 {
    GAUGE_FULL / 10_000 * u64::from(basis_points)
}
