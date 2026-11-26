use crate::{Action, fixture::Element};

use super::{Battle, combat_status::DebuffKind, resolve::PeriodicHit};

#[derive(Clone, Copy)]
pub(super) struct DotEvent {
    pub source: usize,
    pub target: usize,
    pub element: Element,
    pub from_break: bool,
    pub origin: Action,
    pub timing: DotTiming,
    pub is_echo: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum DotTiming {
    TurnStart(usize),
    Detonation,
    Other,
}

#[derive(Clone, Copy)]
pub(super) enum SpecialDotTrigger {
    TurnStart,
    Detonation {
        matching_element: Option<Element>,
        ratio_bp: u16,
        origin: Action,
    },
}

#[derive(Clone, Copy)]
pub(super) enum DotScale {
    Attack(u16),
    StackedAttack {
        multiplier_bp: u32,
        defense_ignore_bp: u16,
    },
    TargetHp {
        hp_bp: u16,
        attack_cap_bp: u16,
    },
}

#[derive(Clone, Copy)]
pub(super) struct SkillDot {
    pub element: Element,
    pub damage: DotScale,
    pub origin: Action,
    pub turns: u32,
    pub max_stacks: u8,
}

#[derive(Clone, Copy)]
pub(super) struct ActiveSkillDot {
    source: usize,
    rule: SkillDot,
    stacks: u8,
    shares_vulnerability: bool,
    separate: bool,
}

#[derive(Clone, Default)]
pub(super) struct LukaState {
    pub fighting_will: u8,
    pub attack_stacks: u8,
    pub dmg_bonus_expires_after_turn: u32,
    pub enhanced_basic: bool,
    pub jabs: u8,
    pub extra_jabs: u8,
}

impl Battle<'_> {
    pub(super) fn apply_skill_dot(
        &mut self,
        source: usize,
        target: usize,
        chance_bp: u32,
        dot: SkillDot,
    ) -> bool {
        self.roll_skill_dot(source, target, chance_bp, dot, false)
    }

    fn roll_skill_dot(
        &mut self,
        source: usize,
        target: usize,
        chance_bp: u32,
        dot: SkillDot,
        separate: bool,
    ) -> bool {
        if self.hp(target).0 == 0
            || dot.turns == 0
            || dot.max_stacks == 0
            || self.target_dot_immune(target, dot.element)
            || !self.roll_effect(source, target, chance_bp)
        {
            return false;
        }
        self.inflict_skill_dot(source, target, dot, false, separate)
    }

    fn target_dot_immune(&self, target: usize, element: Element) -> bool {
        let super::Kit::OfficialEnemy(kit) = &self.plan.units[target].kit else {
            return false;
        };
        let key = match element {
            Element::Physical => "STAT_DOT_Bleed",
            Element::Fire => "STAT_DOT_Burn",
            Element::Thunder => "STAT_DOT_Electric",
            Element::Wind => "STAT_DOT_Poison",
            _ => return false,
        };
        kit.debuff_resist_for(key) >= 10_000
    }

    pub(super) fn inflict_skill_dot_with_vulnerability(
        &mut self,
        source: usize,
        target: usize,
        dot: SkillDot,
    ) -> bool {
        self.inflict_skill_dot(source, target, dot, true, false)
    }

    /// Applies a separate status that can coexist with this source's usual DoT.
    pub(super) fn apply_separate_skill_dot(
        &mut self,
        source: usize,
        target: usize,
        chance_bp: u32,
        dot: SkillDot,
    ) -> bool {
        self.roll_skill_dot(source, target, chance_bp, dot, true)
    }

    fn inflict_skill_dot(
        &mut self,
        source: usize,
        target: usize,
        dot: SkillDot,
        shares_vulnerability: bool,
        separate: bool,
    ) -> bool {
        if self.hp(target).0 == 0
            || dot.turns == 0
            || dot.max_stacks == 0
            || self.resists_debuff(target)
        {
            return false;
        }
        let existing = self.units[target].skill_dots.iter_mut().find(|current| {
            current.source == source
                && current.rule.element == dot.element
                && current.separate == separate
        });
        if let Some(current) = existing {
            current.stacks = current.stacks.saturating_add(1).min(dot.max_stacks);
            current.rule = dot;
            current.shares_vulnerability = shares_vulnerability;
        } else {
            self.units[target].skill_dots.push(ActiveSkillDot {
                source,
                rule: dot,
                stacks: 1,
                shares_vulnerability,
                separate,
            });
        }
        self.record_enemy_debuff(target);
        true
    }

    pub(super) fn has_owned_skill_dot(
        &self,
        source: usize,
        target: usize,
        element: Element,
        separate: bool,
    ) -> bool {
        self.units[target].skill_dots.iter().any(|dot| {
            dot.source == source && dot.rule.element == element && dot.separate == separate
        })
    }

    pub(super) fn finish_dot_batch(&mut self, timing: DotTiming) {
        if !std::mem::take(&mut self.pending_dot_batch) {
            return;
        }
        let previous_timing = std::mem::replace(&mut self.dot_timing, timing);
        self.resolving_dot_echoes = true;
        for owner in self.allies() {
            if let Some(rules) = self.mechanics(owner) {
                rules.on_dot_batch_finished(self, owner);
            }
        }
        self.resolving_dot_echoes = false;
        self.dot_timing = previous_timing;
    }

    pub(super) fn standalone_skill_dot_count(&self, target: usize) -> usize {
        let state = &self.units[target];
        state
            .skill_dots
            .iter()
            .filter(|dot| {
                !dot.shares_vulnerability
                    || !state.debuffs.iter().any(|debuff| {
                        debuff.kind == DebuffKind::Vulnerability && debuff.source == dot.source
                    })
            })
            .count()
    }

    pub(super) fn has_elemental_dot(&self, target: usize, element: Element) -> bool {
        (element == Element::Thunder && self.has_shock(target))
            || self.dot_stacks(target, element) > 0
            || self.units[target]
                .black_swan
                .arcana
                .is_some_and(|arcana| arcana.matches_element(self, target, element))
    }

    pub(super) fn dot_stacks(&self, target: usize, element: Element) -> u8 {
        let unit = &self.units[target];
        let skills = unit
            .skill_dots
            .iter()
            .filter(|dot| dot.rule.element == element)
            .map(|dot| dot.stacks);
        let breaks = unit
            .break_dots
            .iter()
            .filter(|dot| dot.element == element)
            .map(|_| {
                if element == Element::Wind && self.plan.units[target].kit.max_toughness() >= 120 {
                    3
                } else {
                    1
                }
            });
        let arcana = unit
            .black_swan
            .arcana
            .filter(|_| element == Element::Wind)
            .map_or(0, |arcana| u8::try_from(arcana.stacks).unwrap_or(u8::MAX));
        skills.chain(breaks).fold(arcana, u8::saturating_add)
    }

    pub(super) fn trigger_special_dots(&mut self, target: usize, trigger: SpecialDotTrigger) {
        for owner in self.allies() {
            if self.hp(target).0 == 0 {
                return;
            }
            if let Some(mechanics) = self.mechanics(owner) {
                mechanics.trigger_special_dot(self, owner, target, trigger);
            }
        }
    }

    pub(super) fn tick_skill_dots(&mut self, target: usize) {
        let active = self.units[target].skill_dots.clone();
        for dot in active {
            if self.units[target].hp == 0 {
                return;
            }
            self.periodic_hit(
                dot.source,
                target,
                PeriodicHit {
                    element: dot.rule.element,
                    damage: dot.rule.damage,
                    stacks: dot.stacks,
                    ratio_bp: 10_000,
                    origin: dot.rule.origin,
                },
            );
        }
        self.units[target].skill_dots.retain_mut(|dot| {
            dot.rule.turns = dot.rule.turns.saturating_sub(1);
            dot.rule.turns > 0
        });
    }

    pub(super) fn detonate_dots(
        &mut self,
        actor: usize,
        target: usize,
        element: Element,
        ratio_bp: u16,
        origin: Action,
    ) {
        self.detonate_matching_dots(actor, target, Some(element), ratio_bp, origin);
    }

    pub(super) fn detonate_all_dots(
        &mut self,
        actor: usize,
        target: usize,
        ratio_bp: u16,
        origin: Action,
    ) {
        self.detonate_matching_dots(actor, target, None, ratio_bp, origin);
    }

    fn detonate_matching_dots(
        &mut self,
        actor: usize,
        target: usize,
        element: Option<Element>,
        ratio_bp: u16,
        origin: Action,
    ) {
        if self.hp(actor).0 == 0 {
            return;
        }
        let had_special_dot = self.units[target].black_swan.arcana.is_some();
        let previous_timing = std::mem::replace(&mut self.dot_timing, DotTiming::Detonation);
        if element.is_none_or(|element| element == Element::Thunder) {
            self.detonate_shocks(target, ratio_bp, origin);
        }
        let active = self.units[target].skill_dots.clone();
        for dot in active
            .into_iter()
            .filter(|dot| element.is_none_or(|value| dot.rule.element == value))
        {
            self.periodic_hit(
                dot.source,
                target,
                PeriodicHit {
                    element: dot.rule.element,
                    damage: dot.rule.damage,
                    stacks: dot.stacks,
                    ratio_bp,
                    origin,
                },
            );
        }
        let breaks = self.units[target].break_dots.clone();
        for dot in breaks
            .into_iter()
            .filter(|dot| element.is_none_or(|value| dot.element == value))
        {
            self.break_dot_hit(dot.source, target, dot.element, origin, ratio_bp);
        }
        // New statuses can be applied by damage listeners. Only statuses present
        // at the start of the detonation belong to this batch.
        if had_special_dot {
            self.trigger_special_dots(
                target,
                SpecialDotTrigger::Detonation {
                    matching_element: element,
                    ratio_bp,
                    origin,
                },
            );
        }
        self.dot_timing = previous_timing;
    }
}
