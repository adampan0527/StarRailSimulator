mod analysis;
mod combat_status;
mod official;
mod resolve;
mod setup;
mod shock;

use std::ops::Range;

use crate::{
    Action, ActionOrderEntry, BattleError, Command, Event, EventKind, LogMode, Outcome, Snapshot,
    StatusKind, UnitSnapshot,
    fixture::{Ability, GAUGE_FULL, SKILL_POINT_CAP, TURN_LIMIT, Target},
    rng::Rng,
};
use analysis::DummyDamage;
pub use analysis::{
    DprCharacter, DprCredit, DprHit, DprResult, DprScenario, DprTarget, STANDARD_DPR_SCENARIO,
    VictorySearch, VictorySolution, calculate_dpr, search_fastest_victory,
};
use combat_status::{BreakDot, Debuff, DebuffKind, HealOverTime};
use official::{Mechanics, SkillPoints};
pub use official::{OfficialCharacter, OfficialStats, supports_official_dpr};
use setup::Kit;
pub use setup::PreparedBattle;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Status {
    source: usize,
    magnitude: u32,
    expires_after_turn: u32,
}

#[derive(Clone)]
struct UnitState {
    hp: u32,
    energy_milli: u32,
    toughness: u16,
    gauge_quanta: u64,
    normal_turns: u32,
    advanced_to_front: bool,
    statuses: [Option<Status>; 6],
    attack_bonus: Option<(u32, u32)>,
    crit_rate_bonus_bp: u32,
    frozen: Option<(usize, Action)>,
    shocks: Vec<shock::Shock>,
    debuffs: Vec<Debuff>,
    heals_over_time: Vec<HealOverTime>,
    break_dots: Vec<BreakDot>,
    survival_triggered: bool,
    finished_turns: u32,
    next_attack_bonus_bp: u32,
    active_attack_bonus_bp: u32,
    pending_penetration: Option<(u32, u32)>,
    active_penetration_bp: u32,
    penetration_ready_after_turn: u32,
}

impl UnitState {
    fn magnitude(&self, status: StatusKind) -> u32 {
        self.statuses[status.index()].map_or(0, |status| status.magnitude)
    }
}

/// A deterministic state machine advanced only by accepted commands and automatic actions.
#[derive(Clone)]
pub struct Battle<'a> {
    plan: &'a PreparedBattle,
    units: Vec<UnitState>,
    outcome: Outcome,
    av_micros: u64,
    wave_index: usize,
    ordinary_actor: Option<usize>,
    normal_turns: u32,
    decision_index: u32,
    skill_points: u8,
    enemy_hp_lost: u32,
    rng: Rng,
    log: Option<Vec<Event>>,
    dummy_damage: Vec<DummyDamage>,
    crossed_half: Vec<usize>,
    action: Option<Action>,
    pending_follow_ups: std::collections::VecDeque<resolve::FollowUp>,
}

impl<'a> Battle<'a> {
    /// Advance automatic play to the first player decision or a terminal outcome.
    pub fn start(plan: &'a PreparedBattle, mode: LogMode) -> Self {
        let mut battle = Self {
            plan,
            units: plan
                .units
                .iter()
                .map(|unit| UnitState {
                    hp: unit.stats.hp,
                    energy_milli: unit.starting_energy_milli,
                    toughness: unit.kit.max_toughness(),
                    gauge_quanta: GAUGE_FULL,
                    normal_turns: 0,
                    advanced_to_front: false,
                    statuses: [None; 6],
                    attack_bonus: None,
                    crit_rate_bonus_bp: 0,
                    frozen: None,
                    shocks: Vec::new(),
                    debuffs: Vec::new(),
                    heals_over_time: Vec::new(),
                    break_dots: Vec::new(),
                    survival_triggered: false,
                    finished_turns: 0,
                    next_attack_bonus_bp: 0,
                    active_attack_bonus_bp: 0,
                    pending_penetration: None,
                    active_penetration_bp: 0,
                    penetration_ready_after_turn: 0,
                })
                .collect(),
            outcome: Outcome::Ongoing,
            av_micros: 0,
            wave_index: 0,
            ordinary_actor: None,
            normal_turns: 0,
            decision_index: 0,
            skill_points: plan.starting_skill_points,
            enemy_hp_lost: 0,
            rng: Rng::new(plan.seed),
            log: match mode {
                LogMode::Detailed => Some(Vec::new()),
                LogMode::Off => None,
            },
            dummy_damage: Vec::new(),
            crossed_half: Vec::new(),
            action: None,
            pending_follow_ups: std::collections::VecDeque::new(),
        };
        battle.emit(EventKind::WaveStarted, None, None);
        for actor in battle.allies() {
            if let Some(mechanics) = battle.mechanics(actor) {
                mechanics.on_battle_start(&mut battle, actor);
            }
        }
        battle.advance_to_decision();
        battle
    }

    /// A valid prefix can stop at an ongoing decision. Only a completed run is terminal.
    pub fn replay(
        plan: &'a PreparedBattle,
        commands: &[Command],
        mode: LogMode,
    ) -> Result<Self, BattleError> {
        let mut battle = Self::start(plan, mode);
        for command in commands {
            battle.apply(command)?;
        }
        Ok(battle)
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn events(&self) -> &[Event] {
        self.log.as_deref().unwrap_or_default()
    }

    pub fn ordinary_actor(&self) -> Option<&str> {
        self.ordinary_actor
            .map(|unit| self.plan.units[unit].id.as_str())
    }

    /// Stable order: ordinary basic and skill, then ultimates in team order.
    pub fn legal_actions(&self) -> Vec<Command> {
        let mut actions = Vec::new();
        let Some(actor) = self.ordinary_actor else {
            return actions;
        };
        for action in [Action::Basic, Action::Skill] {
            self.list_actions(actor, action, &mut actions);
        }
        for actor in self.allies() {
            self.list_actions(actor, Action::Ultimate, &mut actions);
        }
        actions
    }

    /// Validate the whole command before consuming resources, rolling randomness or advancing time.
    pub fn apply(&mut self, command: &Command) -> Result<(), BattleError> {
        if self.outcome != Outcome::Ongoing {
            return Err(BattleError::BattleEnded);
        }
        if command.decision_index != self.decision_index {
            return Err(BattleError::StaleDecision);
        }
        let actor = self
            .allies()
            .find(|&unit| self.plan.units[unit].id == command.actor_id)
            .ok_or(BattleError::InvalidActor)?;
        let (ability, skill_points) = self.check_action(actor, command.action)?;
        let target = match (ability.target, command.target_id.as_deref()) {
            (Target::Ally, Some(id)) => self.find_target(self.allies(), id)?,
            (Target::Enemy, Some(id)) => self.find_target(self.enemies(), id)?,
            (Target::AllAllies | Target::AllEnemies, None) => None,
            _ => return Err(BattleError::InvalidTarget),
        };

        self.decision_index += 1;
        self.action = Some(command.action);
        self.emit(EventKind::Action(command.action), Some(actor), target);
        match skill_points {
            SkillPoints::Generate => {
                self.set_skill_points(actor, (self.skill_points + 1).min(SKILL_POINT_CAP))
            }
            SkillPoints::Spend => self.set_skill_points(actor, self.skill_points - 1),
            SkillPoints::Unchanged => {}
        }
        let ordinary = command.action != Action::Ultimate;
        if ordinary {
            self.reset_gauge(actor);
        } else {
            self.set_energy(actor, actor, 0);
        }
        self.notify_ally_targets(actor, target, ability.target, command.action);
        self.activate_attack_bonuses(actor, command.action);
        let mechanics = self.mechanics(actor);
        if let Some(mechanics) = &mechanics {
            mechanics.before_action(self, actor, command.action, target);
        }
        self.resolve_ability(actor, target, ability);
        self.gain_energy(actor, actor, ability.energy_milli);
        if let Some(mechanics) = &mechanics {
            mechanics.after_action(self, actor, command.action, target);
        }
        self.finish_attack(actor, command.action);
        self.units[actor].active_attack_bonus_bp = 0;
        self.units[actor].active_penetration_bp = 0;
        self.action = None;
        if ordinary {
            self.finish_turn(actor);
            self.ordinary_actor = None;
        }
        self.resolve_wave_or_end();
        if self.ordinary_actor.is_none() {
            self.advance_to_decision();
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            outcome: self.outcome,
            av_micros: self.av_micros,
            wave_index: self.wave_index,
            decision_index: self.decision_index,
            ordinary_actor_id: self.ordinary_actor().map(str::to_owned),
            skill_points: self.skill_points,
            random_draws: self.rng.draws,
            enemy_hp_lost: self.enemy_hp_lost,
            units: (0..self.enemies().end)
                .map(|unit| {
                    let state = &self.units[unit];
                    UnitSnapshot {
                        unit_id: self.plan.units[unit].id.clone(),
                        is_ally: unit < self.plan.ally_count,
                        hp: state.hp,
                        max_hp: self.plan.units[unit].stats.hp,
                        energy_milli: state.energy_milli,
                        max_energy_milli: self.plan.units[unit].kit.max_energy_milli(),
                        toughness: state.toughness,
                        max_toughness: self.plan.units[unit].kit.max_toughness(),
                        shield: state.magnitude(StatusKind::Shield),
                        vulnerability_bp: state.magnitude(StatusKind::Vulnerability),
                        speed_micros: self.speed_micros(unit),
                        gauge_quanta: state.gauge_quanta,
                        normal_turns: state.normal_turns,
                    }
                })
                .collect(),
        }
    }

    /// Shows the current actor, then pending turns ordered by the current action gauges.
    pub fn action_order(&self) -> Vec<ActionOrderEntry> {
        if self.outcome != Outcome::Ongoing {
            return Vec::new();
        }
        let mut pending: Vec<_> = self
            .turn_candidates()
            .filter(|&unit| Some(unit) != self.ordinary_actor)
            .map(|unit| self.turn_priority(unit))
            .collect();
        pending.sort_unstable();
        self.ordinary_actor
            .iter()
            .map(|&unit| (0, false, unit))
            .chain(pending)
            .map(|(remaining_av_micros, _, unit)| ActionOrderEntry {
                unit_id: self.plan.units[unit].id.clone(),
                remaining_av_micros,
            })
            .collect()
    }

    fn check_action(
        &self,
        actor: usize,
        action: Action,
    ) -> Result<(Ability<'a>, SkillPoints), BattleError> {
        let kit = &self.plan.units[actor].kit;
        let (ability, skill_points) = match kit {
            Kit::Ally(kit) => (
                kit.ability(action),
                match action {
                    Action::Basic => SkillPoints::Generate,
                    Action::Skill => SkillPoints::Spend,
                    Action::Ultimate => SkillPoints::Unchanged,
                },
            ),
            Kit::Official(kit) => kit.ability(action),
            Kit::Enemy(_) | Kit::Dummy { .. } => return Err(BattleError::InvalidActor),
        };
        if self.units[actor].hp == 0 {
            return Err(BattleError::InvalidActor);
        }
        if action == Action::Ultimate {
            if self.units[actor].energy_milli < kit.max_energy_milli() {
                return Err(BattleError::InsufficientEnergy);
            }
        } else {
            if self.ordinary_actor != Some(actor) {
                return Err(BattleError::IllegalTiming);
            }
            if skill_points == SkillPoints::Spend && self.skill_points == 0 {
                return Err(BattleError::InsufficientSkillPoints);
            }
        }
        Ok((ability, skill_points))
    }

    fn list_actions(&self, actor: usize, action: Action, actions: &mut Vec<Command>) {
        let Ok((ability, _)) = self.check_action(actor, action) else {
            return;
        };
        let mut add = |target: Option<usize>| {
            actions.push(Command {
                decision_index: self.decision_index,
                actor_id: self.plan.units[actor].id.clone(),
                action,
                target_id: target.map(|unit| self.plan.units[unit].id.clone()),
            })
        };
        match ability.target {
            Target::AllAllies | Target::AllEnemies => add(None),
            Target::Ally | Target::Enemy => {
                let range = match ability.target {
                    Target::Ally => self.allies(),
                    _ => self.enemies(),
                };
                for unit in range.filter(|&unit| self.units[unit].hp > 0) {
                    add(Some(unit));
                }
            }
        }
    }

    fn find_target(&self, mut units: Range<usize>, id: &str) -> Result<Option<usize>, BattleError> {
        units
            .find(|&unit| self.units[unit].hp > 0 && self.plan.units[unit].id == id)
            .map(Some)
            .ok_or(BattleError::InvalidTarget)
    }

    fn allies(&self) -> Range<usize> {
        0..self.plan.ally_count
    }

    fn mechanics(&self, unit: usize) -> Option<Arc<dyn Mechanics>> {
        match &self.plan.units[unit].kit {
            Kit::Official(kit) => Some(Arc::clone(&kit.mechanics)),
            _ => None,
        }
    }

    fn hp(&self, unit: usize) -> (u32, u32) {
        (self.units[unit].hp, self.plan.units[unit].stats.hp)
    }

    fn is_frozen(&self, unit: usize) -> bool {
        self.units[unit].frozen.is_some()
    }

    fn notify_ally_targets(
        &mut self,
        source: usize,
        target: Option<usize>,
        targeting: Target,
        action: Action,
    ) {
        let targets = match (targeting, target) {
            (Target::Ally, Some(unit)) => unit..unit + 1,
            (Target::AllAllies, None) => self.allies(),
            _ => return,
        };
        for owner in targets {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_ally_targeted(self, owner, source, action);
            }
        }
    }

    fn activate_attack_bonuses(&mut self, actor: usize, action: Action) {
        let state = &mut self.units[actor];
        state.active_attack_bonus_bp = std::mem::take(&mut state.next_attack_bonus_bp);
        if let Some((bp, cooldown)) = state.pending_penetration.take() {
            state.active_penetration_bp = bp;
            let after_this_turn = u32::from(action != Action::Ultimate);
            state.penetration_ready_after_turn = state.finished_turns + after_this_turn + cooldown;
        }
    }

    fn arm_next_attack_bonus(&mut self, actor: usize, bp: u32) {
        self.units[actor].next_attack_bonus_bp = bp;
    }

    fn arm_resistance_penetration(&mut self, actor: usize, bp: u32, cooldown_turns: u32) {
        let state = &mut self.units[actor];
        if state.finished_turns >= state.penetration_ready_after_turn {
            state.pending_penetration = Some((bp, cooldown_turns));
        }
    }

    fn current_action(&self) -> Option<Action> {
        self.action
    }

    fn take_once_per_battle(&mut self, actor: usize) -> bool {
        !std::mem::replace(&mut self.units[actor].survival_triggered, true)
    }

    fn enemies(&self) -> Range<usize> {
        let start = if self.wave_index == 0 {
            self.plan.ally_count
        } else {
            self.plan.wave_ends[self.wave_index - 1]
        };
        start..self.plan.wave_ends[self.wave_index]
    }

    fn speed_micros(&self, unit: usize) -> u64 {
        let stats = self.plan.units[unit].stats;
        let bonus_bp = u64::from(self.units[unit].magnitude(StatusKind::SpeedUp));
        let slow_bp: u64 = self.units[unit]
            .debuffs
            .iter()
            .filter(|status| status.kind == DebuffKind::Slow)
            .map(|status| u64::from(status.bp))
            .sum();
        let speed = stats.base_speed_micros
            + stats.flat_speed_micros
            + stats.base_speed_micros * bonus_bp / 10_000;
        speed
            .saturating_sub(stats.base_speed_micros * slow_bp / 10_000)
            .max(1_000_000)
    }

    fn turn_candidates(&self) -> impl Iterator<Item = usize> {
        self.allies()
            .chain(self.enemies())
            .filter(|&unit| self.units[unit].hp > 0)
    }

    fn turn_priority(&self, unit: usize) -> (u64, bool, usize) {
        let state = &self.units[unit];
        (
            state.gauge_quanta.div_ceil(self.speed_micros(unit)),
            !state.advanced_to_front,
            unit,
        )
    }

    fn advance_to_decision(&mut self) {
        while self.outcome == Outcome::Ongoing {
            if self.normal_turns == TURN_LIMIT {
                self.end(Outcome::TurnLimit);
                return;
            }
            let Some((delta, _, actor)) = self
                .turn_candidates()
                .map(|unit| self.turn_priority(unit))
                .min()
            else {
                self.end(Outcome::Defeat);
                return;
            };

            self.av_micros += delta;
            for unit in self.allies().chain(self.enemies()) {
                if self.units[unit].hp > 0 {
                    let consumed = delta * self.speed_micros(unit);
                    self.units[unit].gauge_quanta =
                        self.units[unit].gauge_quanta.saturating_sub(consumed);
                }
            }
            self.normal_turns += 1;
            self.units[actor].normal_turns += 1;
            self.units[actor].advanced_to_front = false;
            self.emit(EventKind::TurnStarted, Some(actor), None);

            let kit = &self.plan.units[actor].kit;
            match kit {
                Kit::Ally(_) | Kit::Official(_) => {
                    self.tick_heals_over_time(actor);
                    self.ordinary_actor = Some(actor);
                    return;
                }
                Kit::Enemy(_) | Kit::Dummy { .. } => {
                    self.tick_shocks(actor);
                    self.tick_break_dots(actor);
                    if self.units[actor].hp == 0 {
                        self.resolve_wave_or_end();
                        continue;
                    }
                    if self.thaw(actor) {
                        self.finish_turn(actor);
                        self.resolve_wave_or_end();
                        continue;
                    }
                    if self.units[actor].toughness == 0 {
                        self.set_toughness(actor, actor, kit.max_toughness());
                    }
                    self.reset_gauge(actor);
                    if let Kit::Enemy(enemy) = kit {
                        self.resolve_enemy(actor, enemy);
                    }
                    self.finish_turn(actor);
                    self.resolve_wave_or_end();
                }
            }
        }
    }

    fn resolve_wave_or_end(&mut self) {
        if !self.allies().any(|unit| self.units[unit].hp > 0) {
            self.end(Outcome::Defeat);
        } else if !self.enemies().any(|unit| self.units[unit].hp > 0) {
            if self.wave_index + 1 == self.plan.wave_ends.len() {
                self.end(Outcome::Victory);
            } else {
                self.wave_index += 1;
                self.emit(EventKind::WaveStarted, None, None);
            }
        }
    }

    fn end(&mut self, outcome: Outcome) {
        self.outcome = outcome;
        self.ordinary_actor = None;
        self.emit(EventKind::BattleEnded(outcome), None, None);
    }

    fn reset_gauge(&mut self, actor: usize) {
        self.units[actor].gauge_quanta = GAUGE_FULL;
        self.units[actor].advanced_to_front = false;
    }

    fn set_skill_points(&mut self, actor: usize, after: u8) {
        let before = std::mem::replace(&mut self.skill_points, after);
        if before != after {
            self.emit(EventKind::SkillPoints { before, after }, Some(actor), None);
        }
    }

    fn emit(&mut self, kind: EventKind, source: Option<usize>, target: Option<usize>) {
        if let Some(events) = &mut self.log {
            events.push(Event {
                av_micros: self.av_micros,
                wave_index: self.wave_index,
                source_id: source.map(|unit| self.plan.units[unit].id.clone()),
                target_id: target.map(|unit| self.plan.units[unit].id.clone()),
                kind,
            });
        }
    }
}
