use crate::{Action, DamageKind, EventKind, fixture::Element};

use super::{Battle, periodic::DotScale, resolve::PeriodicHit};

#[derive(Clone, Copy)]
enum ShockDamage {
    Skill(u16),
    WeaknessBreak,
}

#[derive(Clone, Copy)]
pub(super) struct Shock {
    source: usize,
    origin: Action,
    damage: ShockDamage,
    turns: u32,
}

impl Shock {
    fn is_break(self) -> bool {
        matches!(self.damage, ShockDamage::WeaknessBreak)
    }

    fn same_slot(self, other: Self) -> bool {
        self.is_break() == other.is_break() && (self.is_break() || self.source == other.source)
    }
}

impl Battle<'_> {
    pub(super) fn has_shock(&self, target: usize) -> bool {
        !self.units[target].shocks.is_empty()
    }

    pub(super) fn clear_shock(&mut self, target: usize) -> bool {
        let shocks = &mut self.units[target].shocks;
        if shocks.is_empty() {
            return false;
        }
        let shock = shocks.remove(0);
        self.emit(
            EventKind::ShockExpired {
                origin: shock.origin,
                from_break: shock.is_break(),
            },
            Some(shock.source),
            Some(target),
        );
        true
    }

    pub(super) fn apply_skill_shock(
        &mut self,
        actor: usize,
        target: usize,
        origin: Action,
        chance_bp: u32,
        multiplier_bp: u16,
        turns: u32,
    ) {
        if self.hp(target).0 > 0 && self.roll_effect(actor, target, chance_bp) {
            self.apply_shock(
                target,
                Shock {
                    source: actor,
                    origin,
                    damage: ShockDamage::Skill(multiplier_bp),
                    turns,
                },
            );
        }
    }

    pub(super) fn apply_break_shock(&mut self, actor: usize, target: usize, origin: Action) {
        const BREAK_SHOCK_CHANCE_BP: u32 = 15_000;
        const BREAK_SHOCK_TURNS: u32 = 2;
        if self.hp(target).0 > 0 && self.roll_effect(actor, target, BREAK_SHOCK_CHANCE_BP) {
            self.apply_shock(
                target,
                Shock {
                    source: actor,
                    origin,
                    damage: ShockDamage::WeaknessBreak,
                    turns: BREAK_SHOCK_TURNS,
                },
            );
        }
    }

    fn apply_shock(&mut self, target: usize, shock: Shock) {
        if self.resists_debuff(target) {
            return;
        }
        let shocks = &mut self.units[target].shocks;
        // Reapplication refreshes the same skill or break status. Skill and break
        // Shock coexist, even when the same attacker applied both.
        if let Some(existing) = shocks.iter_mut().find(|existing| existing.same_slot(shock)) {
            *existing = shock;
        } else {
            shocks.push(shock);
        }
        self.record_enemy_debuff(target);
        self.emit(
            EventKind::ShockApplied {
                origin: shock.origin,
                from_break: shock.is_break(),
                turns: shock.turns,
            },
            Some(shock.source),
            Some(target),
        );
    }

    pub(super) fn extend_shocks(&mut self, actor: usize, target: usize, extra_turns: u32) {
        if !self.has_shock(target) {
            return;
        }
        for shock in &mut self.units[target].shocks {
            shock.turns = shock.turns.saturating_add(extra_turns);
        }
        self.emit(
            EventKind::ShockExtended { extra_turns },
            Some(actor),
            Some(target),
        );
    }

    pub(super) fn tick_shocks(&mut self, target: usize) {
        let mut index = 0;
        while let Some(shock) = self.units[target].shocks.get(index).copied() {
            match shock.damage {
                ShockDamage::Skill(multiplier_bp) => self.secondary_hit(
                    shock.source,
                    target,
                    Element::Thunder,
                    multiplier_bp,
                    DamageKind::DamageOverTime,
                    shock.origin,
                ),
                ShockDamage::WeaknessBreak => {
                    self.break_shock_hit(shock.source, target, shock.origin)
                }
            }
            if self.hp(target).0 == 0 {
                self.units[target].shocks.clear();
                return;
            }
            let shocks = &mut self.units[target].shocks;
            if !shocks
                .get(index)
                .is_some_and(|current| current.same_slot(shock))
            {
                // A reactive cleanse can remove this shock or shift it to another index.
                let Some(current_index) =
                    shocks.iter().position(|current| current.same_slot(shock))
                else {
                    continue;
                };
                index = current_index;
            }
            if shocks[index].turns == 1 {
                shocks.remove(index);
                self.emit(
                    EventKind::ShockExpired {
                        origin: shock.origin,
                        from_break: shock.is_break(),
                    },
                    Some(shock.source),
                    Some(target),
                );
            } else {
                shocks[index].turns -= 1;
                index += 1;
            }
        }
    }

    pub(super) fn detonate_shocks(&mut self, target: usize, ratio_bp: u16, origin: Action) {
        let shocks = self.units[target].shocks.clone();
        for shock in shocks {
            match shock.damage {
                ShockDamage::Skill(multiplier_bp) => self.periodic_hit(
                    shock.source,
                    target,
                    PeriodicHit {
                        element: Element::Thunder,
                        damage: DotScale::Attack(multiplier_bp),
                        stacks: 1,
                        ratio_bp,
                        origin,
                    },
                ),
                ShockDamage::WeaknessBreak => {
                    self.break_dot_hit(shock.source, target, Element::Thunder, origin, ratio_bp)
                }
            }
        }
    }
}
