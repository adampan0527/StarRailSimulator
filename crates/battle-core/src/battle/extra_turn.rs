use std::collections::VecDeque;

use crate::EventKind;

use super::Battle;

#[derive(Clone, Default)]
pub(super) struct ExtraTurns {
    pending: VecDeque<usize>,
    active: bool,
    interrupted: Option<usize>,
}

impl Battle<'_> {
    pub(super) fn queue_extra_turn(&mut self, actor: usize) {
        self.extra_turns.pending.push_back(actor);
    }

    pub(super) fn cancel_queued_extra_turn(&mut self, actor: usize) {
        self.extra_turns.pending.retain(|&pending| pending != actor);
    }

    pub(super) fn in_extra_turn(&self, actor: usize) -> bool {
        self.extra_turns.active && self.ordinary_actor == Some(actor)
    }

    pub(super) fn start_extra_turn(&mut self) -> bool {
        if self.extra_turns.active {
            return true;
        }
        loop {
            let next = if let Some(owner) = self.territory_owner {
                self.extra_turns
                    .pending
                    .iter()
                    .position(|&actor| actor == owner)
                    .and_then(|index| self.extra_turns.pending.remove(index))
            } else {
                self.extra_turns.pending.pop_front()
            };
            let Some(actor) = next else { return false };
            if !self.available_for_ordinary_turn(actor) {
                continue;
            }
            self.extra_turns.interrupted = self.ordinary_actor.replace(actor);
            self.extra_turns.active = true;
            self.emit(EventKind::ExtraTurnStarted, Some(actor), None);
            self.notify_ally_turn_opening(actor, true);
            if let Some(rules) = self.mechanics(actor) {
                rules.on_extra_turn_start(self, actor);
            }
            return true;
        }
    }

    pub(super) fn finish_extra_turn(&mut self) {
        let actor = self.ordinary_actor;
        self.extra_turns.active = false;
        self.ordinary_actor = self
            .extra_turns
            .interrupted
            .take()
            .filter(|&actor| self.available_for_ordinary_turn(actor));
        if let Some(actor) = actor {
            self.notify_battle_turn_finished(actor, true);
        }
    }

    pub(super) fn close_turn_before_suspension(&mut self, actor: usize) {
        if self.in_extra_turn(actor) {
            self.finish_extra_turn();
        } else if self.ordinary_actor == Some(actor) {
            self.ordinary_actor = None;
            self.finish_turn(actor);
        } else if self.extra_turns.interrupted == Some(actor) {
            self.extra_turns.interrupted = None;
            self.finish_turn(actor);
        }
    }

    pub(super) fn interrupted_turn_actor(&self) -> Option<usize> {
        self.extra_turns.interrupted
    }

    pub(super) fn extra_turn_active(&self) -> bool {
        self.extra_turns.active
    }
}
