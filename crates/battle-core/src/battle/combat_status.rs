use crate::{Action, DamageKind, EventKind, StatusKind, fixture::Element};

use super::{Battle, resolve::gauge_for};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum DebuffKind {
    Slow,
    BreakImprisonment,
    SkillImprisonment,
    Weightless {
        defense_down_bp: u16,
        resistance_down_bp: u16,
    },
    DefenseDown,
    DefenseBug,
    Vulnerability,
    DotVulnerability,
    EffectResistanceDown,
    Besotted,
    AttackDown,
    AllResistanceDown,
    ResistanceDown(Element),
    Epiphany {
        only_on_turn: bool,
        effect_resistance_down_bp: u16,
    },
    ImplantedWeakness(Element),
}

impl DebuffKind {
    pub(super) fn is_speed_down(self) -> bool {
        matches!(
            self,
            Self::Slow
                | Self::BreakImprisonment
                | Self::SkillImprisonment
                | Self::Weightless { .. }
        )
    }

    fn status(self) -> StatusKind {
        match self {
            Self::Slow | Self::BreakImprisonment => StatusKind::Slow,
            Self::SkillImprisonment => StatusKind::Imprisoned,
            Self::Weightless { .. } => StatusKind::Weightless,
            Self::DefenseDown | Self::DefenseBug => StatusKind::DefenseDown,
            Self::Vulnerability => StatusKind::Vulnerability,
            Self::DotVulnerability => StatusKind::DotVulnerability,
            Self::EffectResistanceDown => StatusKind::EffectResistanceDown,
            Self::Besotted => StatusKind::Besotted,
            Self::AttackDown => StatusKind::AttackDown,
            Self::AllResistanceDown | Self::ResistanceDown(_) => StatusKind::ResistanceDown,
            Self::Epiphany { .. } => StatusKind::Epiphany,
            Self::ImplantedWeakness(_) => StatusKind::WeaknessImplanted,
        }
    }

    pub(super) fn reduces_resistance_to(self, element: Element) -> bool {
        match self {
            Self::AllResistanceDown => true,
            Self::ResistanceDown(affected) | Self::ImplantedWeakness(affected) => {
                affected == element
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Debuff {
    pub source: usize,
    pub kind: DebuffKind,
    pub bp: u32,
    expires_after_turn: Option<u32>,
}

impl Debuff {
    pub(super) fn remaining_turns(self, turn: u32) -> Option<u32> {
        self.expires_after_turn.map(|end| end.saturating_sub(turn))
    }
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

#[derive(Clone, Copy)]
pub(super) struct Entanglement {
    source: usize,
    origin: Action,
    stacks: u16,
}

impl Battle<'_> {
    pub(super) fn is_crowd_controlled(&self, unit: usize) -> bool {
        self.is_frozen(unit)
            || self.units[unit].entanglement.is_some()
            || self.units[unit].debuffs.iter().any(|status| {
                matches!(
                    status.kind,
                    DebuffKind::BreakImprisonment | DebuffKind::SkillImprisonment
                )
            })
    }

    pub(super) fn apply_debuff(
        &mut self,
        source: usize,
        target: usize,
        kind: DebuffKind,
        bp: u32,
        turns: u32,
    ) -> bool {
        if self.units[target].hp == 0 || self.resists_debuff(target) {
            return false;
        }
        self.store_debuff(source, target, kind, bp, Some(turns))
    }

    pub(super) fn apply_permanent_debuff(
        &mut self,
        source: usize,
        target: usize,
        kind: DebuffKind,
        bp: u32,
    ) {
        self.store_debuff(source, target, kind, bp, None);
    }

    fn store_debuff(
        &mut self,
        source: usize,
        target: usize,
        kind: DebuffKind,
        bp: u32,
        turns: Option<u32>,
    ) -> bool {
        let state = &mut self.units[target];
        if state.hp == 0 {
            return false;
        }
        let updated = Debuff {
            source,
            kind,
            bp,
            expires_after_turn: turns.map(|turns| state.normal_turns.saturating_add(turns)),
        };
        if let Some(current) = state.debuffs.iter_mut().find(|status| {
            status.source == source
                && (status.kind == kind
                    || matches!(
                        (status.kind, kind),
                        (
                            DebuffKind::ImplantedWeakness(_),
                            DebuffKind::ImplantedWeakness(_)
                        )
                    ))
        }) {
            *current = updated;
        } else {
            state.debuffs.push(updated);
        }
        self.emit(
            EventKind::StatusApplied(kind.status()),
            Some(source),
            Some(target),
        );
        true
    }

    pub(super) fn apply_break_slow(&mut self, actor: usize, target: usize) -> bool {
        self.apply_debuff(actor, target, DebuffKind::BreakImprisonment, 1_000, 1)
    }

    pub(super) fn apply_speed_down(&mut self, actor: usize, target: usize, bp: u32, turns: u32) {
        self.apply_debuff(actor, target, DebuffKind::Slow, bp, turns);
    }

    pub(super) fn apply_skill_imprisonment(
        &mut self,
        source: usize,
        target: usize,
        slow_bp: u32,
        delay_bp: u16,
    ) {
        if self.apply_debuff(source, target, DebuffKind::SkillImprisonment, slow_bp, 1) {
            self.delay_action(source, target, delay_bp);
        }
    }

    pub(super) fn has_weightless(&self, source: usize, target: usize) -> bool {
        self.units[target].debuffs.iter().any(|status| {
            status.source == source && matches!(status.kind, DebuffKind::Weightless { .. })
        })
    }

    pub(super) fn is_slowed(&self, target: usize) -> bool {
        self.units[target]
            .debuffs
            .iter()
            .any(|status| status.kind.is_speed_down())
    }

    pub(super) fn has_besotted_from(&self, target: usize, source: usize) -> bool {
        self.units[target]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::Besotted && status.source == source)
    }

    pub(super) fn attack_down_bp(&self, unit: usize) -> u32 {
        self.units[unit]
            .debuffs
            .iter()
            .filter(|status| status.kind == DebuffKind::AttackDown)
            .map(|status| status.bp)
            .fold(0, u32::saturating_add)
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
        self.debuff_count(target) > 0
    }

    pub(super) fn has_epiphany_from(&self, source: usize, target: usize) -> bool {
        self.units[target].debuffs.iter().any(|status| {
            status.source == source && matches!(status.kind, DebuffKind::Epiphany { .. })
        })
    }

    pub(super) fn debuff_count(&self, target: usize) -> usize {
        let state = &self.units[target];
        let special = self
            .allies()
            .filter(|&owner| self.units[owner].hp > 0)
            .filter_map(|owner| self.mechanics(owner).map(|rules| (owner, rules)))
            .map(|(owner, rules)| rules.additional_debuff_count(self, owner, target))
            .sum::<usize>();
        state.debuffs.len()
            + usize::from(state.magnitude(StatusKind::Vulnerability) > 0)
            + usize::from(self.is_frozen(target))
            + state.shocks.len()
            + usize::from(state.entanglement.is_some())
            + state.break_dots.len()
            + usize::from(state.black_swan.arcana.is_some())
            + self.standalone_skill_dot_count(target)
            + special
    }

    pub(super) fn cleanse_one(&mut self, actor: usize, target: usize) {
        let state = &mut self.units[target];
        let expired = if let Some(index) = state
            .debuffs
            .iter()
            .position(|status| status.expires_after_turn.is_some())
        {
            let status = state.debuffs.remove(index);
            Some((status.kind.status(), status.source))
        } else {
            state.statuses[StatusKind::Vulnerability.index()]
                .take()
                .map(|status| (StatusKind::Vulnerability, status.source))
        };
        if let Some((kind, source)) = expired {
            self.emit(EventKind::StatusExpired(kind), Some(source), Some(target));
        } else if !self.clear_freeze(target)
            && !self.clear_shock(target)
            && !self.clear_periodic_debuff(target)
        {
            let Some(status) = self.units[target].entanglement.take() else {
                return;
            };
            self.emit(
                EventKind::StatusExpired(StatusKind::Entangled),
                Some(status.source),
                Some(target),
            );
        }
        self.emit(EventKind::StatusCleared, Some(actor), Some(target));
    }

    fn clear_periodic_debuff(&mut self, target: usize) -> bool {
        let state = &mut self.units[target];
        if !state.skill_dots.is_empty() {
            state.skill_dots.remove(0);
        } else if !state.break_dots.is_empty() {
            state.break_dots.remove(0);
        } else if let Some(arcana) = state.black_swan.arcana.take() {
            self.emit(
                EventKind::StatusExpired(StatusKind::Arcana),
                Some(arcana.source),
                Some(target),
            );
        } else {
            return false;
        }
        true
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
            .extract_if(.., |status| {
                status.expires_after_turn.is_some_and(|end| end <= turns)
            })
            .collect();
        for status in expired {
            self.emit(
                EventKind::StatusExpired(status.kind.status()),
                Some(status.source),
                Some(target),
            );
        }
    }

    pub(super) fn remove_debuff(&mut self, source: usize, target: usize, kind: DebuffKind) {
        let debuffs = &mut self.units[target].debuffs;
        let previous_len = debuffs.len();
        debuffs.retain(|status| status.source != source || status.kind != kind);
        if debuffs.len() < previous_len {
            self.emit(
                EventKind::StatusExpired(kind.status()),
                Some(source),
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
        self.refresh_heal_over_time(source, target, turns, origin, false);
    }

    pub(super) fn apply_shared_heal_over_time(
        &mut self,
        source: usize,
        target: usize,
        turns: u32,
        origin: Action,
    ) {
        self.refresh_heal_over_time(source, target, turns, origin, true);
    }

    fn refresh_heal_over_time(
        &mut self,
        source: usize,
        target: usize,
        turns: u32,
        origin: Action,
        shared: bool,
    ) {
        if !self.is_targetable(target) {
            return;
        }
        let heals = &mut self.units[target].heals_over_time;
        let status = HealOverTime {
            source,
            origin,
            turns,
        };
        if let Some(existing) = heals
            .iter_mut()
            .find(|heal| heal.source == source && (shared || heal.origin == origin))
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
        if self.resists_debuff(target) {
            return;
        }
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

    pub(super) fn apply_entanglement(
        &mut self,
        source: usize,
        target: usize,
        origin: Action,
        break_effect_bp: u32,
    ) {
        if self.resists_debuff(target) {
            return;
        }
        self.units[target].entanglement = Some(Entanglement {
            source,
            origin,
            stacks: 1,
        });
        self.emit(
            EventKind::StatusApplied(StatusKind::Entangled),
            Some(source),
            Some(target),
        );
        let delay = gauge_for(2_000) / 10_000 * (10_000 + u64::from(break_effect_bp));
        self.shift_gauge(
            source,
            target,
            self.units[target].gauge_quanta + delay,
            false,
        );
    }

    pub(super) fn stack_entanglement(&mut self, target: usize) {
        if let Some(status) = &mut self.units[target].entanglement {
            status.stacks = (status.stacks + 1).min(5);
        }
    }

    pub(super) fn tick_entanglement(&mut self, target: usize) {
        let Some(status) = self.units[target].entanglement.take() else {
            return;
        };
        self.break_status_hit_scaled(
            status.source,
            target,
            Element::Quantum,
            status.origin,
            DamageKind::Additional,
            6_000 * status.stacks,
        );
        self.emit(
            EventKind::StatusExpired(StatusKind::Entangled),
            Some(status.source),
            Some(target),
        );
    }
}
