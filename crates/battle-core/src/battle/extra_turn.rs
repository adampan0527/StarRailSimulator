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

    pub(super) fn in_extra_turn(&self, actor: usize) -> bool {
        self.extra_turns.active && self.ordinary_actor == Some(actor)
    }

    pub(super) fn start_extra_turn(&mut self) -> bool {
        if self.extra_turns.active {
            return true;
        }
        while let Some(actor) = self.extra_turns.pending.pop_front() {
            if !self.is_targetable(actor) {
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
        false
    }

    pub(super) fn finish_extra_turn(&mut self) {
        self.extra_turns.active = false;
        self.ordinary_actor = self
            .extra_turns
            .interrupted
            .take()
            .filter(|&actor| self.is_targetable(actor));
    }

    pub(super) fn interrupted_turn_actor(&self) -> Option<usize> {
        self.extra_turns.interrupted
    }

    pub(super) fn extra_turn_active(&self) -> bool {
        self.extra_turns.active
    }
}
