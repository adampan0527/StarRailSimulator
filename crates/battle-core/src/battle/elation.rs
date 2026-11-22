use crate::{Action, EventKind, fixture::GAUGE_FULL};

use super::{Battle, CombatPath};

const AHA_BASE_SPEED_MICROS: u64 = 80_000_000;
const AHA_SPEED_WEIGHTS: [u64; 4] = [5, 10, 20, 40];
pub(super) const AHA_ACTION_ORDER_ID: &str = "summon:aha";

#[derive(Clone, Copy)]
enum AhaTurn {
    Natural,
    Extra { owner: usize, punchline: u32 },
}

#[derive(Clone, Copy)]
pub(super) struct AhaInstant {
    pub punchline: u32,
    pub skill_damage_factor_bp: u16,
}

#[derive(Clone)]
struct CertifiedBanger {
    actor: usize,
    punchline: u32,
    last_turn: u32,
}

#[derive(Clone)]
pub(super) struct AhaState {
    pub gauge_quanta: u64,
    punchline: u32,
    pending_extra: Option<AhaTurn>,
    bangers: Vec<CertifiedBanger>,
}

impl Battle<'_> {
    fn elation_allies(&self) -> impl Iterator<Item = usize> + '_ {
        self.allies().filter(|&actor| {
            self.plan.units[actor]
                .combat_type
                .is_some_and(|kind| kind.path == CombatPath::Elation)
                && self.is_targetable(actor)
        })
    }

    pub(super) fn setup_aha(&mut self) {
        let actors: Vec<_> = self.elation_allies().collect();
        if actors.is_empty() {
            return;
        }
        self.aha = Some(AhaState {
            gauge_quanta: GAUGE_FULL,
            punchline: 0,
            pending_extra: None,
            bangers: Vec::new(),
        });
        self.add_punchline(actors[0], actors.len() as u32);
        for actor in actors {
            self.grant_certified_banger(actor, 0);
        }
    }

    pub(super) fn aha_speed_micros(&self) -> Option<u64> {
        self.aha.as_ref()?;
        let mut speeds = [0; 4];
        let mut actors = self.elation_allies().peekable();
        actors.peek()?;
        for (slot, actor) in speeds.iter_mut().zip(actors) {
            *slot = self.speed_micros(actor);
        }
        speeds.sort_unstable_by(|left, right| right.cmp(left));
        Some(
            AHA_BASE_SPEED_MICROS
                + speeds
                    .into_iter()
                    .zip(AHA_SPEED_WEIGHTS)
                    .map(|(speed, denominator)| speed / denominator)
                    .sum::<u64>(),
        )
    }

    pub(super) fn certified_banger(&self, actor: usize) -> Option<u32> {
        self.aha
            .as_ref()?
            .bangers
            .iter()
            .filter(|banger| banger.actor == actor)
            .map(|banger| banger.punchline)
            .reduce(u32::saturating_add)
    }

    fn grant_certified_banger(&mut self, actor: usize, punchline: u32) {
        let duration = self
            .mechanics(actor)
            .map_or(2, |rules| rules.certified_banger_duration());
        let before = self.certified_banger(actor).unwrap_or(0);
        let Some(aha) = &mut self.aha else { return };
        aha.bangers.push(CertifiedBanger {
            actor,
            punchline,
            last_turn: self.units[actor].normal_turns + duration,
        });
        self.emit(
            EventKind::CertifiedBanger {
                before,
                after: before.saturating_add(punchline),
            },
            Some(actor),
            None,
        );
    }

    pub(super) fn expire_certified_banger(&mut self, actor: usize) {
        let Some(before) = self.certified_banger(actor) else {
            return;
        };
        let turn = self.units[actor].normal_turns;
        if let Some(aha) = &mut self.aha {
            let count = aha.bangers.len();
            aha.bangers
                .retain(|banger| banger.actor != actor || banger.last_turn > turn);
            if aha.bangers.len() == count {
                return;
            }
        }
        self.emit(
            EventKind::CertifiedBanger {
                before,
                after: self.certified_banger(actor).unwrap_or(0),
            },
            Some(actor),
            None,
        );
    }

    pub(super) fn add_punchline(&mut self, actor: usize, points: u32) {
        if let Some(aha) = &self.aha {
            self.set_punchline(Some(actor), aha.punchline.saturating_add(points));
        }
    }

    fn set_punchline(&mut self, actor: Option<usize>, after: u32) {
        if let Some(aha) = &mut self.aha {
            let before = std::mem::replace(&mut aha.punchline, after);
            if before != after {
                self.emit(EventKind::Punchline { before, after }, actor, None);
            }
        }
    }

    pub(super) fn queue_aha_extra_turn(&mut self, owner: usize, punchline: u32) {
        if let Some(aha) = &mut self.aha {
            aha.pending_extra = Some(AhaTurn::Extra { owner, punchline });
        }
    }

    pub(super) fn resolve_aha_extra_turn(&mut self) {
        if let Some(turn) = self.aha.as_mut().and_then(|aha| aha.pending_extra.take()) {
            self.resolve_aha_instant(turn);
        }
    }

    pub(super) fn resolve_natural_aha_turn(&mut self) {
        if let Some(aha) = &mut self.aha {
            aha.gauge_quanta = GAUGE_FULL;
            self.resolve_aha_instant(AhaTurn::Natural);
        }
    }

    fn resolve_aha_instant(&mut self, turn: AhaTurn) {
        let (actor, punchline, skill_damage_factor_bp) = match turn {
            AhaTurn::Natural => (
                None,
                self.aha.as_ref().map_or(0, |aha| aha.punchline),
                10_000,
            ),
            AhaTurn::Extra { owner, punchline } => (
                Some(owner),
                punchline,
                self.mechanics(owner)
                    .map_or(10_000, |rules| rules.aha_extra_skill_damage_factor_bp()),
            ),
        };
        let mut actors: Vec<_> = self.elation_allies().collect();
        self.emit(
            EventKind::AhaInstant {
                punchline,
                extra: actor.is_some(),
            },
            actor,
            None,
        );
        for &owner in &actors {
            self.cleanse_crowd_control(owner, owner);
        }
        actors.sort_by_key(|&unit| {
            self.mechanics(unit)
                .and_then(|rules| rules.elation_participant())
                .unwrap_or(u16::MAX)
        });
        let mut participants = Vec::new();
        for owner in actors {
            if !self.enemies().any(|target| self.is_targetable(target)) {
                break;
            }
            let Some(rules) = self.mechanics(owner) else {
                continue;
            };
            if rules.elation_participant().is_none() {
                continue;
            }
            let punchline = actor.map_or_else(
                || self.aha.as_ref().map_or(0, |aha| aha.punchline),
                |_| punchline,
            );
            let previous_action = self.action.replace(Action::ElationSkill);
            let parent_debuff_targets = self.begin_debuff_action();
            self.begin_attack();
            self.emit(EventKind::Action(Action::ElationSkill), Some(owner), None);
            rules.on_elation_skill(
                self,
                owner,
                AhaInstant {
                    punchline,
                    skill_damage_factor_bp,
                },
            );
            self.notify_attack(owner, Action::ElationSkill, None);
            self.finish_debuff_action(owner, parent_debuff_targets);
            self.resolve_follow_ups();
            self.notify_ally_action_finished(owner, Action::ElationSkill);
            self.notify_unit_action_finished(owner);
            self.action = previous_action;
            participants.push(owner);
        }
        let retained = if matches!(turn, AhaTurn::Natural) {
            let consumed = self.aha.as_ref().map_or(0, |aha| aha.punchline);
            self.set_punchline(None, 0);
            consumed
        } else {
            punchline
        };
        for owner in participants {
            if self.is_targetable(owner) {
                self.grant_certified_banger(owner, retained);
            }
        }
        let replenished = self.elation_allies().count() as u32;
        let current = self.aha.as_ref().map_or(0, |aha| aha.punchline);
        self.set_punchline(actor, current.saturating_add(replenished));
    }

    pub(super) fn intrinsic_elation_bp(&self, actor: usize) -> u32 {
        self.plan.units[actor].elation_bp.saturating_add(
            self.mechanics(actor)
                .map_or(0, |rules| rules.personal_elation_bp(self, actor)),
        )
    }
}
