use crate::{Action, DamageKind, EventKind, StatusKind, fixture::Element};

use super::Battle;

#[derive(Clone, Copy)]
pub(super) struct Shield {
    source: usize,
    origin: Option<Action>,
    amount: u32,
    expires_after_turn: u32,
}

#[derive(Clone, Copy)]
enum FreezeDamage {
    Break,
    Skill(u16),
}

#[derive(Clone, Copy)]
pub(super) struct Freeze {
    source: usize,
    origin: Action,
    turns: u32,
    damage: FreezeDamage,
}

#[derive(Clone, Default)]
pub(super) struct CounterState {
    pub enhanced_charges: u8,
    pub ult_expires_after_turn: u32,
    pub recently_hit: bool,
    pub marked: Vec<usize>,
    uses: u8,
    used_on_turn: u32,
}

impl Battle<'_> {
    pub(super) fn apply_official_shield(
        &mut self,
        source: usize,
        target: usize,
        amount: u32,
        turns: u32,
        origin: Option<Action>,
    ) {
        if amount == 0 || turns == 0 || !self.is_targetable(target) {
            return;
        }
        let state = &mut self.units[target];
        let shield = Shield {
            source,
            origin,
            amount,
            expires_after_turn: state.normal_turns + turns,
        };
        let existing = state
            .shields
            .iter_mut()
            .find(|item| item.source == source && item.origin == origin);
        if let Some(existing) = existing {
            *existing = shield;
        } else {
            state.shields.push(shield);
        }
        self.emit(
            EventKind::StatusApplied(StatusKind::Shield),
            Some(source),
            Some(target),
        );
    }

    pub(super) fn has_official_shield(&self, source: usize, target: usize, origin: Action) -> bool {
        self.units[target]
            .shields
            .iter()
            .any(|item| item.source == source && item.origin == Some(origin))
    }

    pub(super) fn shield_amount(&self, target: usize) -> u32 {
        let state = &self.units[target];
        let fixture = state.magnitude(StatusKind::Shield);
        state
            .shields
            .iter()
            .map(|shield| shield.amount)
            .fold(fixture, u32::max)
    }

    pub(super) fn deplete_shields(&mut self, target: usize, amount: u32) {
        let state = &mut self.units[target];
        let mut depleted = Vec::new();
        if let Some(shield) = &mut state.statuses[StatusKind::Shield.index()] {
            shield.magnitude = shield.magnitude.saturating_sub(amount);
            if shield.magnitude == 0 {
                depleted.push(shield.source);
                state.statuses[StatusKind::Shield.index()] = None;
            }
        }
        state.shields.retain_mut(|shield| {
            shield.amount = shield.amount.saturating_sub(amount);
            if shield.amount == 0 {
                depleted.push(shield.source);
            }
            shield.amount > 0
        });
        for source in depleted {
            self.emit(EventKind::ShieldDepleted, Some(source), Some(target));
        }
    }

    pub(super) fn expire_shields(&mut self, target: usize) {
        let state = &mut self.units[target];
        let now = state.normal_turns;
        let ended: Vec<_> = state
            .shields
            .extract_if(.., |item| item.expires_after_turn <= now)
            .collect();
        for item in ended {
            self.emit(
                EventKind::StatusExpired(StatusKind::Shield),
                Some(item.source),
                Some(target),
            );
        }
    }

    pub(super) fn apply_break_freeze(&mut self, source: usize, target: usize, origin: Action) {
        self.apply_freeze(
            target,
            Freeze {
                source,
                origin,
                damage: FreezeDamage::Break,
                turns: 1,
            },
        );
    }

    pub(super) fn apply_skill_freeze(
        &mut self,
        source: usize,
        target: usize,
        chance_bp: u32,
        turns: u32,
        multiplier_bp: u16,
        origin: Action,
    ) -> bool {
        if self.units[target].hp == 0
            || turns == 0
            || !self.roll_status(source, target, chance_bp, true)
        {
            return false;
        }
        self.apply_freeze(
            target,
            Freeze {
                source,
                origin,
                damage: FreezeDamage::Skill(multiplier_bp),
                turns,
            },
        )
    }

    fn apply_freeze(&mut self, target: usize, status: Freeze) -> bool {
        if self.resists_enemy_control(status.source, target) || self.resists_debuff(target) {
            return false;
        }
        self.units[target].frozen = Some(status);
        self.emit(
            EventKind::StatusApplied(StatusKind::Frozen),
            Some(status.source),
            Some(target),
        );
        true
    }

    pub(super) fn clear_freeze(&mut self, target: usize) -> bool {
        let Some(freeze) = self.units[target].frozen.take() else {
            return false;
        };
        self.emit(
            EventKind::StatusExpired(StatusKind::Frozen),
            Some(freeze.source),
            Some(target),
        );
        true
    }

    pub(super) fn thaw(&mut self, target: usize) -> bool {
        let Some(mut freeze) = self.units[target].frozen else {
            return false;
        };
        let source = freeze.source;
        // The target is still frozen while its turn-start damage resolves.
        match freeze.damage {
            FreezeDamage::Break => self.break_status_hit(
                source,
                target,
                Element::Ice,
                freeze.origin,
                DamageKind::Additional,
            ),
            FreezeDamage::Skill(multiplier) => self.secondary_hit(
                source,
                target,
                Element::Ice,
                multiplier,
                DamageKind::Additional,
                freeze.origin,
            ),
        }
        self.units[target].frozen = None;
        if self.units[target].hp == 0 {
            return true;
        }
        freeze.turns -= 1;
        if freeze.turns > 0 {
            self.units[target].frozen = Some(freeze);
        } else {
            self.emit(
                EventKind::StatusExpired(StatusKind::Frozen),
                Some(source),
                Some(target),
            );
            if matches!(freeze.damage, FreezeDamage::Skill(_))
                && let Some(mechanics) = self.mechanics(source)
            {
                mechanics.after_skill_freeze_ended(self, source, target);
            }
        }
        self.skip_frozen_turn(source, target, freeze.turns == 0);
        true
    }

    pub(super) fn spend_counter_charge(&mut self, actor: usize, limit: u8) -> bool {
        let state = &mut self.units[actor];
        let counters = &mut state.counter_state;
        if counters.used_on_turn != state.normal_turns {
            counters.uses = 0;
            counters.used_on_turn = state.normal_turns;
        }
        if counters.uses >= limit {
            return false;
        }
        counters.uses += 1;
        true
    }
}
