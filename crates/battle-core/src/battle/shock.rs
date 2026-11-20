use crate::{Action, DamageKind, EventKind, fixture::Element};

use super::Battle;

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
}

impl Battle<'_> {
    pub(super) fn has_shock(&self, target: usize) -> bool {
        !self.units[target].shocks.is_empty()
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
        let shocks = &mut self.units[target].shocks;
        // Reapplication refreshes the same skill or break status. Skill and break
        // Shock coexist, even when the same attacker applied both.
        if let Some(existing) = shocks.iter_mut().find(|existing| {
            existing.is_break() == shock.is_break()
                && (shock.is_break() || existing.source == shock.source)
        }) {
            *existing = shock;
        } else {
            shocks.push(shock);
        }
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
            if shock.turns == 1 {
                self.units[target].shocks.remove(index);
                self.emit(
                    EventKind::ShockExpired {
                        origin: shock.origin,
                        from_break: shock.is_break(),
                    },
                    Some(shock.source),
                    Some(target),
                );
            } else {
                self.units[target].shocks[index].turns -= 1;
                index += 1;
            }
        }
    }
}
