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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ElationSkillSource {
    AhaInstant,
    Directed,
    Triggered,
}

#[derive(Clone, Copy)]
pub(super) struct ElationAscension {
    pub innate_level: u8,
    pub reclaimed_bp: u32,
}

#[derive(Clone, Copy)]
pub(super) struct ElationDebt {
    pub ratio_bp: u32,
    pub cap_bp: u32,
    pub minimum_bp: u32,
}

#[derive(Clone, Copy)]
struct ElationDamageTotal {
    actor: Option<usize>,
    amount: u64,
}

#[derive(Clone)]
struct CertifiedBanger {
    actor: usize,
    punchline: u32,
    last_turn: u32,
}

#[derive(Clone, Copy)]
struct ExtraTurnElation {
    actor: usize,
    banger: u32,
    punchline: u32,
}

#[derive(Clone)]
pub(super) struct AhaState {
    pub gauge_quanta: u64,
    punchline: u32,
    extra_turn: Option<ExtraTurnElation>,
    pending_extra: Option<AhaTurn>,
    bangers: Vec<CertifiedBanger>,
    instant_damage: Option<ElationDamageTotal>,
    resolve_advanced_turn: bool,
}

impl Battle<'_> {
    pub(super) fn elation_team_size(&self) -> usize {
        self.allies()
            .filter(|&actor| {
                self.plan.units[actor]
                    .combat_type
                    .is_some_and(|kind| kind.path == CombatPath::Elation)
            })
            .count()
    }

    pub(super) fn current_punchline(&self) -> u32 {
        self.aha.as_ref().map_or(0, |aha| {
            aha.punchline
                .saturating_add(aha.extra_turn.map_or(0, |extra| extra.punchline))
        })
    }

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
            extra_turn: None,
            pending_extra: None,
            bangers: Vec::new(),
            instant_damage: None,
            resolve_advanced_turn: false,
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
        let base = self
            .elation_allies()
            .find_map(|actor| self.mechanics(actor)?.aha_base_speed_micros(self, actor));
        let flat = self
            .elation_allies()
            .filter_map(|actor| Some(self.mechanics(actor)?.aha_flat_speed_micros(self, actor)))
            .sum::<u64>();
        Some(
            base.unwrap_or(AHA_BASE_SPEED_MICROS)
                + flat
                + speeds
                    .into_iter()
                    .zip(AHA_SPEED_WEIGHTS)
                    .map(|(speed, denominator)| speed / denominator)
                    .sum::<u64>(),
        )
    }

    pub(super) fn elation_ascension(&self) -> Option<ElationAscension> {
        self.aha.as_ref()?;
        self.elation_allies()
            .find_map(|actor| self.mechanics(actor)?.elation_ascension(self, actor))
    }

    pub(super) fn advance_aha(&mut self, bp: u16) {
        if let Some(aha) = &mut self.aha {
            aha.gauge_quanta = aha
                .gauge_quanta
                .saturating_sub(super::resolve::gauge_for(bp));
            aha.resolve_advanced_turn = aha.gauge_quanta == 0;
        }
    }

    pub(super) fn accrue_elation_debt(&mut self, source: usize, target: usize, hp_loss: u32) {
        if self.aha.is_none()
            || hp_loss == 0
            || !self.is_party_unit(source)
            || !self.enemies().contains(&target)
        {
            return;
        }
        let Some((provider, rule)) = self
            .elation_allies()
            .find_map(|actor| Some((actor, self.mechanics(actor)?.elation_debt()?)))
        else {
            return;
        };
        self.apply_elation_debt(provider, target, rule, hp_loss);
    }

    pub(super) fn apply_elation_debt(
        &mut self,
        provider: usize,
        target: usize,
        rule: ElationDebt,
        hp_loss: u32,
    ) {
        use super::support::percentage_of;

        let max_hp = self.plan.units[target].stats.hp;
        let before = self.units[target].elation_debt;
        let minimum = percentage_of(max_hp, rule.minimum_bp);
        let cap = minimum.max(percentage_of(max_hp, rule.cap_bp));
        let after = before
            .saturating_add(percentage_of(hp_loss, rule.ratio_bp))
            .clamp(minimum, cap);
        let overflow = self.units[target]
            .hp
            .saturating_sub(max_hp.saturating_sub(after));
        if before != after {
            self.units[target].elation_debt = after;
            self.emit(
                EventKind::ElationDebt { before, after },
                Some(provider),
                Some(target),
            );
        }
        self.units[target].hp -= overflow;
        self.enemy_hp_lost += overflow;
    }

    pub(super) fn record_aha_damage(
        &mut self,
        source: usize,
        target: usize,
        kind: crate::DamageKind,
        amount: u32,
    ) {
        if kind != crate::DamageKind::TrueDamage
            && self.is_party_unit(source)
            && self.enemies().contains(&target)
            && let Some(total) = self
                .aha
                .as_mut()
                .and_then(|aha| aha.instant_damage.as_mut())
            && total
                .actor
                .is_none_or(|actor| actor == source && kind == crate::DamageKind::Elation)
        {
            total.amount = total.amount.saturating_add(u64::from(amount));
        }
    }

    pub(super) fn aha_instant_damage(&self) -> u64 {
        self.aha
            .as_ref()
            .and_then(|aha| aha.instant_damage)
            .map_or(0, |total| total.amount)
    }

    fn banger_change(&self, before: u32, after: u32) -> EventKind {
        if self.elation_ascension().is_some() {
            EventKind::PartyTrick { before, after }
        } else {
            EventKind::CertifiedBanger { before, after }
        }
    }

    pub(super) fn certified_banger(&self, actor: usize) -> Option<u32> {
        let aha = self.aha.as_ref()?;
        aha.bangers
            .iter()
            .filter(|banger| banger.actor == actor)
            .map(|banger| banger.punchline)
            .chain(
                aha.extra_turn
                    .filter(|extra| extra.actor == actor)
                    .map(|extra| extra.banger),
            )
            .reduce(u32::saturating_add)
    }

    pub(super) fn grant_certified_banger(&mut self, actor: usize, punchline: u32) {
        self.add_certified_banger(actor, punchline, true);
    }

    fn add_certified_banger(&mut self, actor: usize, punchline: u32, new_gain: bool) {
        let rules = self.mechanics(actor);
        let cap = rules
            .as_ref()
            .and_then(|rules| rules.permanent_banger_cap(self));
        let last_turn = if cap.is_some() {
            u32::MAX
        } else {
            let duration = rules.map_or(2, |rules| rules.certified_banger_duration());
            self.units[actor].normal_turns.saturating_add(duration)
        };
        let existing = self.certified_banger(actor);
        let before = existing.unwrap_or(0);
        let punchline = cap.map_or(punchline, |limit| {
            punchline.min(limit.saturating_sub(before))
        });
        if punchline == 0 && existing.is_some() && cap.is_some() {
            return;
        }
        let Some(aha) = &mut self.aha else { return };
        aha.bangers.push(CertifiedBanger {
            actor,
            punchline,
            last_turn,
        });
        self.emit(
            self.banger_change(before, before.saturating_add(punchline)),
            Some(actor),
            None,
        );
        if new_gain {
            self.notify_certified_banger_gained(actor, punchline);
        }
    }

    fn notify_certified_banger_gained(&mut self, actor: usize, points: u32) {
        if points > 0
            && let Some(rules) = self.mechanics(actor)
        {
            rules.on_certified_banger_gained(self, actor, points);
        }
    }

    pub(super) fn extend_certified_banger_by_one_turn(&mut self, actor: usize) {
        if let Some(aha) = &mut self.aha {
            for banger in aha
                .bangers
                .iter_mut()
                .filter(|banger| banger.actor == actor)
            {
                banger.last_turn = banger.last_turn.saturating_add(1);
            }
        }
    }

    pub(super) fn spend_certified_banger(&mut self, actor: usize, mut points: u32) {
        let before = self.certified_banger(actor).unwrap_or(0);
        if points == 0 || before == 0 {
            return;
        }
        let Some(aha) = &mut self.aha else { return };
        if let Some(extra) = &mut aha.extra_turn
            && extra.actor == actor
        {
            let spent = points.min(extra.banger);
            extra.banger -= spent;
            points -= spent;
        }
        for banger in &mut aha.bangers {
            if banger.actor == actor {
                let spent = points.min(banger.punchline);
                banger.punchline -= spent;
                points -= spent;
            }
        }
        aha.bangers
            .retain(|banger| banger.actor != actor || banger.punchline > 0);
        let after = self.certified_banger(actor).unwrap_or(0);
        self.emit(self.banger_change(before, after), Some(actor), None);
    }

    pub(super) fn enforce_permanent_banger_caps(&mut self) {
        if self.aha.is_none() {
            return;
        }
        for actor in self.allies() {
            if let Some(limit) = self
                .mechanics(actor)
                .and_then(|rules| rules.permanent_banger_cap(self))
            {
                let current = self.certified_banger(actor).unwrap_or(0);
                self.spend_certified_banger(actor, current.saturating_sub(limit));
            }
        }
    }

    pub(super) fn start_extra_turn_elation(&mut self, actor: usize, banger: u32, punchline: u32) {
        let before_banger = self.certified_banger(actor).unwrap_or(0);
        let before = self.current_punchline();
        let Some(aha) = &mut self.aha else { return };
        aha.extra_turn = Some(ExtraTurnElation {
            actor,
            banger,
            punchline,
        });
        self.emit(
            self.banger_change(before_banger, before_banger.saturating_add(banger)),
            Some(actor),
            None,
        );
        self.notify_certified_banger_gained(actor, banger);
        self.notify_punchline_changed(Some(actor), before);
    }

    pub(super) fn finish_extra_turn_elation(&mut self, banger_bp: u32, punchline_bp: u32) {
        use super::support::percentage_of;

        let before = self.current_punchline();
        let Some(extra) = self.aha.as_ref().and_then(|aha| aha.extra_turn) else {
            return;
        };
        let before_banger = self.certified_banger(extra.actor).unwrap_or(0);
        let Some(aha) = &mut self.aha else { return };
        aha.extra_turn = None;
        aha.punchline = aha
            .punchline
            .saturating_add(percentage_of(extra.punchline, punchline_bp));
        let after_banger = before_banger.saturating_sub(extra.banger);
        self.emit(
            self.banger_change(before_banger, after_banger),
            Some(extra.actor),
            None,
        );
        let retained_banger = percentage_of(extra.banger, banger_bp);
        if retained_banger != 0 {
            self.add_certified_banger(extra.actor, retained_banger, false);
        }
        self.notify_punchline_changed(Some(extra.actor), before);
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
            self.banger_change(before, self.certified_banger(actor).unwrap_or(0)),
            Some(actor),
            None,
        );
    }

    pub(super) fn add_punchline(&mut self, actor: usize, points: u32) {
        self.set_punchline(Some(actor), self.current_punchline().saturating_add(points));
    }

    fn set_punchline(&mut self, actor: Option<usize>, after: u32) {
        let before = self.current_punchline();
        if let Some(aha) = &mut self.aha {
            if let Some(extra) = &mut aha.extra_turn {
                // Pool consumption spends the transient share first.
                extra.punchline = extra.punchline.saturating_sub(before.saturating_sub(after));
            }
            aha.punchline = after.saturating_sub(aha.extra_turn.map_or(0, |extra| extra.punchline));
            self.notify_punchline_changed(actor, before);
        }
    }

    fn notify_punchline_changed(&mut self, actor: Option<usize>, before: u32) {
        let after = self.current_punchline();
        if before == after {
            return;
        }
        let kind = if self.elation_ascension().is_some() {
            EventKind::Bliss { before, after }
        } else {
            EventKind::Punchline { before, after }
        };
        self.emit(kind, actor, None);
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_punchline_changed(self, owner);
                if after > before {
                    rules.on_punchline_gained(self, owner, after - before);
                }
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
        if self
            .aha
            .as_ref()
            .is_some_and(|aha| aha.resolve_advanced_turn)
            && self.aha_speed_micros().is_some()
        {
            self.resolve_natural_aha_turn();
        }
    }

    pub(super) fn resolve_natural_aha_turn(&mut self) {
        if let Some(aha) = &mut self.aha {
            aha.gauge_quanta = GAUGE_FULL;
            aha.resolve_advanced_turn = false;
            self.resolve_aha_instant(AhaTurn::Natural);
        }
    }

    pub(super) fn resolve_elation_skill(&mut self, actor: usize, instant: AhaInstant) {
        self.resolve_elation_skill_from(actor, instant, ElationSkillSource::Directed);
    }

    pub(super) fn resolve_elation_skill_from(
        &mut self,
        actor: usize,
        instant: AhaInstant,
        source: ElationSkillSource,
    ) {
        let Some(rules) = self
            .mechanics(actor)
            .filter(|rules| rules.elation_participant().is_some())
        else {
            return;
        };
        let previous_action = self.action.replace(Action::ElationSkill);
        let previous_follow_up = self.resolving_follow_up;
        if rules.primary_attack_counts_as_follow_up(self, actor, Action::ElationSkill) {
            self.resolving_follow_up = true;
            self.emit(EventKind::FollowUp, Some(actor), None);
        }
        let parent_debuff_targets = self.begin_debuff_action();
        self.begin_attack();
        self.emit(EventKind::Action(Action::ElationSkill), Some(actor), None);
        rules.on_elation_skill_from(self, actor, instant, source);
        self.notify_attack(actor, Action::ElationSkill, None);
        self.finish_debuff_action(actor, parent_debuff_targets);
        self.resolving_follow_up = previous_follow_up;
        if source != ElationSkillSource::Triggered {
            self.resolve_follow_ups();
        }
        self.notify_ally_action_finished(actor, Action::ElationSkill);
        self.notify_unit_action_finished(actor);
        self.action = previous_action;
    }

    pub(super) fn resolve_elation_skill_tracking_damage(
        &mut self,
        actor: usize,
        instant: AhaInstant,
    ) -> u64 {
        let Some(aha) = &mut self.aha else {
            return 0;
        };
        let previous = aha.instant_damage.replace(ElationDamageTotal {
            actor: Some(actor),
            amount: 0,
        });
        self.resolve_elation_skill(actor, instant);
        let amount = self.aha_instant_damage();
        if let Some(aha) = &mut self.aha {
            aha.instant_damage = previous;
        }
        amount
    }

    fn resolve_aha_instant(&mut self, turn: AhaTurn) {
        let (actor, punchline, skill_damage_factor_bp) = match turn {
            AhaTurn::Natural => (None, self.current_punchline(), 10_000),
            AhaTurn::Extra { owner, punchline } => (
                Some(owner),
                punchline,
                self.mechanics(owner)
                    .map_or(10_000, |rules| rules.aha_extra_skill_damage_factor_bp()),
            ),
        };
        let mut actors: Vec<_> = self.elation_allies().collect();
        let event = if self.elation_ascension().is_some() {
            EventKind::AscendedAhaInstant {
                bliss: punchline,
                extra: actor.is_some(),
            }
        } else {
            EventKind::AhaInstant {
                punchline,
                extra: actor.is_some(),
            }
        };
        self.emit(event, actor, None);
        if let Some(aha) = &mut self.aha {
            aha.instant_damage = Some(ElationDamageTotal {
                actor: None,
                amount: 0,
            });
        }
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
            let punchline = actor.map_or_else(|| self.current_punchline(), |_| punchline);
            self.resolve_elation_skill_from(
                owner,
                AhaInstant {
                    punchline,
                    skill_damage_factor_bp,
                },
                ElationSkillSource::AhaInstant,
            );
            participants.push(owner);
        }
        let natural = matches!(turn, AhaTurn::Natural);
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_aha_instant_ending(self, owner, natural, skill_damage_factor_bp);
            }
        }
        let retained = if natural {
            let consumed = self.current_punchline();
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
        let current = self.current_punchline();
        let reclaimed = if natural {
            self.elation_ascension().map_or(0, |ascension| {
                super::support::percentage_of(retained, ascension.reclaimed_bp)
            })
        } else {
            0
        };
        self.set_punchline(
            actor,
            current
                .saturating_add(replenished)
                .saturating_add(reclaimed),
        );
        if let Some(aha) = &mut self.aha {
            aha.instant_damage = None;
        }
        for owner in self.allies() {
            if self.is_targetable(owner)
                && let Some(rules) = self.mechanics(owner)
            {
                rules.on_aha_instant_finished(self, owner);
            }
        }
    }

    pub(super) fn intrinsic_elation_bp(&self, actor: usize) -> u32 {
        self.plan.units[actor].elation_bp.saturating_add(
            self.mechanics(actor)
                .map_or(0, |rules| rules.personal_elation_bp(self, actor)),
        )
    }
}
