use crate::{Action, fixture::Element};

use super::{Battle, resolve::PeriodicHit};

#[derive(Clone, Copy)]
pub(super) struct DotEvent {
    pub source: usize,
    pub target: usize,
    pub element: Element,
    pub from_break: bool,
}

#[derive(Clone, Copy)]
pub(super) enum DotScale {
    Attack(u16),
    TargetHp { hp_bp: u16, attack_cap_bp: u16 },
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
        if self.hp(target).0 == 0
            || dot.turns == 0
            || dot.max_stacks == 0
            || !self.roll_effect(source, target, chance_bp)
        {
            return false;
        }
        self.inflict_skill_dot(source, target, dot)
    }

    pub(super) fn inflict_skill_dot(
        &mut self,
        source: usize,
        target: usize,
        dot: SkillDot,
    ) -> bool {
        if self.hp(target).0 == 0
            || dot.turns == 0
            || dot.max_stacks == 0
            || self.resists_debuff(target)
        {
            return false;
        }
        let existing = self.units[target]
            .skill_dots
            .iter_mut()
            .find(|current| current.source == source && current.rule.element == dot.element);
        if let Some(current) = existing {
            current.stacks = current.stacks.saturating_add(1).min(dot.max_stacks);
            current.rule = dot;
        } else {
            self.units[target].skill_dots.push(ActiveSkillDot {
                source,
                rule: dot,
                stacks: 1,
            });
        }
        true
    }

    pub(super) fn has_elemental_dot(&self, target: usize, element: Element) -> bool {
        (element == Element::Thunder && self.has_shock(target))
            || self.dot_stacks(target, element) > 0
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
        skills.chain(breaks).fold(0_u8, u8::saturating_add)
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
    }
}
