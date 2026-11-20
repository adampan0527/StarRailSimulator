use crate::{Action, DamageKind, EventKind, StatusKind, fixture::Element};

use super::Battle;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum DebuffKind {
    Slow,
    DefenseDown,
    Vulnerability,
    DotVulnerability,
    EffectResistanceDown,
    ResistanceDown(Element),
}

impl DebuffKind {
    fn status(self) -> StatusKind {
        match self {
            Self::Slow => StatusKind::Slow,
            Self::DefenseDown => StatusKind::DefenseDown,
            Self::Vulnerability => StatusKind::Vulnerability,
            Self::DotVulnerability => StatusKind::DotVulnerability,
            Self::EffectResistanceDown => StatusKind::EffectResistanceDown,
            Self::ResistanceDown(_) => StatusKind::ResistanceDown,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Debuff {
    pub source: usize,
    pub kind: DebuffKind,
    pub bp: u32,
    expires_after_turn: u32,
}

#[derive(Clone, Copy)]
pub(super) struct HealOverTime {
    source: usize,
    origin: Action,
    turns: u32,
}

#[derive(Clone, Copy)]
pub(super) struct BreakDot {
    pub source: usize,
    origin: Action,
    pub element: Element,
    turns: u32,
}

impl Battle<'_> {
    fn apply_debuff(
        &mut self,
        source: usize,
        target: usize,
        kind: DebuffKind,
        bp: u32,
        turns: u32,
    ) {
        let state = &mut self.units[target];
        if state.hp == 0 {
            return;
        }
        let updated = Debuff {
            source,
            kind,
            bp,
            expires_after_turn: state.normal_turns + turns,
        };
        if let Some(current) = state
            .debuffs
            .iter_mut()
            .find(|status| status.kind == kind && status.source == source)
        {
            *current = updated;
        } else {
            state.debuffs.push(updated);
        }
        self.emit(
            EventKind::StatusApplied(kind.status()),
            Some(source),
            Some(target),
        );
    }

    pub(super) fn apply_speed_down(&mut self, actor: usize, target: usize, bp: u32, turns: u32) {
        self.apply_debuff(actor, target, DebuffKind::Slow, bp, turns);
    }

    pub(super) fn is_slowed(&self, target: usize) -> bool {
        self.units[target]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::Slow)
    }

    pub(super) fn apply_defense_down(&mut self, actor: usize, target: usize, bp: u32, turns: u32) {
        self.apply_debuff(actor, target, DebuffKind::DefenseDown, bp, turns);
    }

    pub(super) fn apply_vulnerability(&mut self, actor: usize, target: usize, bp: u32, turns: u32) {
        self.apply_debuff(actor, target, DebuffKind::Vulnerability, bp, turns);
    }

    pub(super) fn vulnerability_from(&self, source: usize, target: usize) -> u32 {
        self.units[target]
            .debuffs
            .iter()
            .find(|debuff| debuff.kind == DebuffKind::Vulnerability && debuff.source == source)
            .map_or(0, |debuff| debuff.bp)
    }

    pub(super) fn apply_effect_resistance_down(
        &mut self,
        actor: usize,
        target: usize,
        bp: u32,
        turns: u32,
    ) {
        self.apply_debuff(actor, target, DebuffKind::EffectResistanceDown, bp, turns);
    }

    pub(super) fn apply_dot_vulnerability(
        &mut self,
        actor: usize,
        target: usize,
        bp: u32,
        turns: u32,
    ) {
        self.apply_debuff(actor, target, DebuffKind::DotVulnerability, bp, turns);
    }

    pub(super) fn apply_resistance_down(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        bp: u32,
        turns: u32,
    ) {
        self.apply_debuff(
            actor,
            target,
            DebuffKind::ResistanceDown(element),
            bp,
            turns,
        );
    }

    pub(super) fn is_debuffed(&self, target: usize) -> bool {
        let state = &self.units[target];
        !state.debuffs.is_empty()
            || state.magnitude(StatusKind::Vulnerability) > 0
            || self.is_frozen(target)
            || self.has_shock(target)
            || !state.break_dots.is_empty()
            || !state.skill_dots.is_empty()
    }

    pub(super) fn cleanse_one(&mut self, actor: usize, target: usize) {
        let state = &mut self.units[target];
        let (kind, source) = if let Some(status) = state.debuffs.first().copied() {
            state.debuffs.remove(0);
            (status.kind.status(), status.source)
        } else if let Some(status) = state.statuses[StatusKind::Vulnerability.index()].take() {
            (StatusKind::Vulnerability, status.source)
        } else {
            return;
        };
        self.emit(EventKind::StatusExpired(kind), Some(source), Some(target));
        self.emit(EventKind::StatusCleared, Some(actor), Some(target));
    }

    pub(super) fn dispel_one(&mut self, actor: usize, target: usize) -> bool {
        let state = &mut self.units[target];
        for kind in [StatusKind::Shield, StatusKind::SpeedUp] {
            if let Some(status) = state.statuses[kind.index()].take() {
                self.emit(
                    EventKind::StatusExpired(kind),
                    Some(status.source),
                    Some(target),
                );
                self.emit(EventKind::StatusCleared, Some(actor), Some(target));
                return true;
            }
        }
        false
    }

    pub(super) fn expire_debuffs(&mut self, target: usize) {
        let state = &mut self.units[target];
        let turns = state.normal_turns;
        let expired: Vec<_> = state
            .debuffs
            .extract_if(.., |status| status.expires_after_turn <= turns)
            .collect();
        for status in expired {
            self.emit(
                EventKind::StatusExpired(status.kind.status()),
                Some(status.source),
                Some(target),
            );
        }
    }

    pub(super) fn apply_heal_over_time(
        &mut self,
        source: usize,
        target: usize,
        turns: u32,
        origin: Action,
    ) {
        let heals = &mut self.units[target].heals_over_time;
        let status = HealOverTime {
            source,
            origin,
            turns,
        };
        if let Some(existing) = heals
            .iter_mut()
            .find(|heal| heal.source == source && heal.origin == origin)
        {
            *existing = status;
        } else if turns > 0 {
            heals.push(status);
        }
    }

    pub(super) fn tick_heals_over_time(&mut self, target: usize) {
        let heals = self.units[target].heals_over_time.clone();
        for heal in heals {
            if let Some(mechanics) = self.mechanics(heal.source) {
                mechanics.heal_over_time(self, heal.source, target, heal.origin);
            }
        }
        let heals = &mut self.units[target].heals_over_time;
        for heal in heals.iter_mut() {
            heal.turns = heal.turns.saturating_sub(1);
        }
        heals.retain(|heal| heal.turns > 0);
    }

    pub(super) fn apply_break_dot(
        &mut self,
        source: usize,
        target: usize,
        element: Element,
        origin: Action,
    ) {
        let status = BreakDot {
            source,
            origin,
            element,
            turns: 2,
        };
        let dots = &mut self.units[target].break_dots;
        if let Some(dot) = dots.iter_mut().find(|dot| dot.element == element) {
            *dot = status;
        } else {
            dots.push(status);
        }
    }

    pub(super) fn tick_break_dots(&mut self, target: usize) {
        let dots = self.units[target].break_dots.clone();
        for dot in dots {
            if self.units[target].hp == 0 {
                return;
            }
            self.break_status_hit(
                dot.source,
                target,
                dot.element,
                dot.origin,
                DamageKind::DamageOverTime,
            );
        }
        self.units[target].break_dots.retain_mut(|dot| {
            dot.turns = dot.turns.saturating_sub(1);
            dot.turns > 0
        });
    }
}
