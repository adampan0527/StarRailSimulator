mod analysis;
mod combat_status;
mod extra_turn;
mod official;
mod periodic;
mod protection;
mod resolve;
mod setup;
mod shock;
mod support;

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
use combat_status::{BreakDot, Debuff, DebuffKind, Entanglement, HealOverTime};
use extra_turn::ExtraTurns;
use official::{
    AcheronState, AventurineState, BailuState, BlackSwanTarget, DrRatioState, FeixiaoState,
    FuXuanState, FugueState, GallagherState, HanyaState, HuohuoState, Invigoration, JingYuanState,
    LuochaState, LynxResponse, MarchHuntState, Mechanics, MishaState, MozeState, SeeleState,
    SkillPoints, SushangState, TopazState, WeltState, XueyiState, YanqingState, YukongState,
};
pub use official::{
    CombatPath, CombatType, OfficialCharacter, OfficialStats, supports_official_dpr,
    supports_official_dpr_variant,
};
use periodic::{ActiveSkillDot, DotTiming, LukaState, SpecialDotTrigger};
use protection::{CounterState, Freeze, Shield};
use setup::Kit;
pub use setup::PreparedBattle;
use std::sync::Arc;
use support::{AllyBuff, AstaState, BronyaState, TingyunState};

#[derive(Clone, Copy)]
struct Status {
    source: usize,
    magnitude: u32,
    expires_after_turn: u32,
}

#[derive(Clone, Copy)]
struct SummonTurn {
    name: &'static str,
    speed_micros: u64,
    gauge_quanta: u64,
    advanced_to_front: bool,
    can_advance: bool,
}

impl SummonTurn {
    fn new(name: &'static str, speed_micros: u64) -> Self {
        Self::with_action_advance(name, speed_micros, true)
    }

    fn with_action_advance(name: &'static str, speed_micros: u64, can_advance: bool) -> Self {
        Self {
            name,
            speed_micros,
            gauge_quanta: GAUGE_FULL,
            advanced_to_front: false,
            can_advance,
        }
    }

    fn advance(&mut self, bp: u16) {
        if self.can_advance {
            self.gauge_quanta = self.gauge_quanta.saturating_sub(resolve::gauge_for(bp));
            self.advanced_to_front = self.gauge_quanta == 0;
        }
    }
}

// Equal AV and advance priority preserve unit order, then summon owner order.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum TurnSlot {
    Unit(usize),
    Summon(usize),
}

#[derive(Clone, Copy)]
struct ExtraToughness {
    source: usize,
    remaining: u16,
    maximum: u16,
}

#[derive(Clone)]
struct UnitState {
    hp: u32,
    energy_milli: u32,
    toughness: u16,
    extra_toughness: Option<ExtraToughness>,
    gauge_quanta: u64,
    summon_turn: Option<SummonTurn>,
    normal_turns: u32,
    departed: bool,
    advanced_to_front: bool,
    statuses: [Option<Status>; StatusKind::COUNT],
    attack_bonus: Option<(u32, u32)>,
    crit_rate_bonus_bp: u32,
    frozen: Option<Freeze>,
    shocks: Vec<shock::Shock>,
    shields: Vec<Shield>,
    counter_state: CounterState,
    debuffs: Vec<Debuff>,
    heals_over_time: Vec<HealOverTime>,
    invigoration: Option<Invigoration>,
    bailu_state: BailuState,
    break_dots: Vec<BreakDot>,
    entanglement: Option<Entanglement>,
    lynx_response: Option<LynxResponse>,
    skill_dots: Vec<ActiveSkillDot>,
    charged_skill: bool,
    burning_targets_at_action_start: Vec<usize>,
    kafka_follow_up_charges: u8,
    black_swan: BlackSwanTarget,
    luka_state: LukaState,
    himeko_charge: u8,
    march_hunt_state: MarchHuntState,
    moze_state: MozeState,
    topaz_state: TopazState,
    jing_yuan_state: JingYuanState,
    jingliu_state: official::JingliuState,
    xueyi_state: XueyiState,
    sushang_state: SushangState,
    yanqing_state: YanqingState,
    ally_buffs: Vec<AllyBuff>,
    asta_state: AstaState,
    bronya_state: BronyaState,
    tingyun_state: TingyunState,
    sparkle_state: official::SparkleState,
    sunday_state: official::SundayState,
    qingque_state: official::QingqueState,
    dan_heng_il_state: official::DanHengIlState,
    ruan_mei_state: official::RuanMeiState,
    fugue_state: FugueState,
    harmony_trailblazer_state: official::HarmonyTrailblazerState,
    robin_state: official::RobinState,
    boothill_state: official::BoothillState,
    blade_state: official::BladeState,
    gallagher_state: GallagherState,
    hanya_state: HanyaState,
    misha_state: MishaState,
    welt_state: WeltState,
    seele_state: SeeleState,
    yukong_state: YukongState,
    luocha_state: LuochaState,
    jiaoqiu_field: official::JiaoqiuField,
    dr_ratio_state: DrRatioState,
    feixiao_state: FeixiaoState,
    fu_xuan_state: FuXuanState,
    acheron_state: AcheronState,
    huohuo_state: HuohuoState,
    argenti_apotheosis: u8,
    crimson_knots: u8,
    rebloom_spent: bool,
    aventurine_state: AventurineState,
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

struct CheckedAction<'a> {
    ability: Ability<'a>,
    skill_points: SkillPoints,
    ultimate_cost_milli: Option<u32>,
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
    extra_turns: ExtraTurns,
    normal_turns: u32,
    decision_index: u32,
    skill_points: u8,
    enemy_hp_lost: u32,
    rng: Rng,
    log: Option<Vec<Event>>,
    dummy_damage: Vec<DummyDamage>,
    crossed_half: Vec<usize>,
    action: Option<Action>,
    action_target: Option<usize>,
    attack_targets: Vec<usize>,
    super_break_targets: Vec<resolve::SuperBreakTarget>,
    tracks_super_break: bool,
    attack_index: u32,
    enemy_turn: Option<usize>,
    enemy_action: Option<usize>,
    dot_timing: DotTiming,
    pending_follow_ups: std::collections::VecDeque<resolve::PendingAttack>,
    resolving_follow_up: bool,
    observes_enemy_debuffs: bool,
    enemy_debuff_targets: Option<Vec<usize>>,
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
                    extra_toughness: None,
                    gauge_quanta: GAUGE_FULL,
                    summon_turn: None,
                    normal_turns: 0,
                    departed: false,
                    advanced_to_front: false,
                    statuses: [None; StatusKind::COUNT],
                    attack_bonus: None,
                    crit_rate_bonus_bp: 0,
                    frozen: None,
                    shocks: Vec::new(),
                    shields: Vec::new(),
                    counter_state: CounterState::default(),
                    debuffs: Vec::new(),
                    heals_over_time: Vec::new(),
                    invigoration: None,
                    bailu_state: BailuState::default(),
                    break_dots: Vec::new(),
                    entanglement: None,
                    lynx_response: None,
                    skill_dots: Vec::new(),
                    charged_skill: false,
                    burning_targets_at_action_start: Vec::new(),
                    kafka_follow_up_charges: 0,
                    black_swan: BlackSwanTarget::default(),
                    luka_state: LukaState::default(),
                    himeko_charge: 0,
                    march_hunt_state: MarchHuntState::default(),
                    moze_state: MozeState::default(),
                    topaz_state: TopazState::default(),
                    jing_yuan_state: JingYuanState::default(),
                    jingliu_state: official::JingliuState::default(),
                    xueyi_state: XueyiState::default(),
                    sushang_state: SushangState::default(),
                    yanqing_state: YanqingState::default(),
                    ally_buffs: Vec::new(),
                    asta_state: AstaState::default(),
                    bronya_state: BronyaState::default(),
                    tingyun_state: TingyunState::default(),
                    sparkle_state: official::SparkleState::default(),
                    sunday_state: official::SundayState::default(),
                    qingque_state: official::QingqueState::default(),
                    dan_heng_il_state: official::DanHengIlState::default(),
                    ruan_mei_state: official::RuanMeiState::default(),
                    fugue_state: FugueState::default(),
                    harmony_trailblazer_state: official::HarmonyTrailblazerState::default(),
                    robin_state: official::RobinState::default(),
                    boothill_state: official::BoothillState::default(),
                    blade_state: official::BladeState::default(),
                    gallagher_state: GallagherState::default(),
                    hanya_state: HanyaState::default(),
                    misha_state: MishaState::default(),
                    welt_state: WeltState::default(),
                    seele_state: SeeleState::default(),
                    yukong_state: YukongState::default(),
                    luocha_state: LuochaState::default(),
                    jiaoqiu_field: official::JiaoqiuField::default(),
                    dr_ratio_state: DrRatioState::default(),
                    feixiao_state: FeixiaoState::default(),
                    fu_xuan_state: FuXuanState::default(),
                    acheron_state: AcheronState::default(),
                    huohuo_state: HuohuoState::default(),
                    argenti_apotheosis: 0,
                    crimson_knots: 0,
                    rebloom_spent: false,
                    aventurine_state: AventurineState::default(),
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
            extra_turns: ExtraTurns::default(),
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
            action_target: None,
            attack_targets: Vec::new(),
            super_break_targets: Vec::new(),
            tracks_super_break: plan.units[..plan.ally_count]
                .iter()
                .any(|unit| matches!(&unit.kit, Kit::Official(kit) if kit.mechanics.provides_super_break())),
            attack_index: 0,
            enemy_turn: None,
            enemy_action: None,
            dot_timing: DotTiming::Other,
            pending_follow_ups: std::collections::VecDeque::new(),
            resolving_follow_up: false,
            observes_enemy_debuffs: plan.units.iter().any(|unit| {
                matches!(&unit.kit, Kit::Official(kit) if kit.mechanics.observes_enemy_debuffs())
            }),
            enemy_debuff_targets: None,
        };
        battle.emit(EventKind::WaveStarted, None, None);
        for actor in battle.allies() {
            if let Some(mechanics) = battle.mechanics(actor) {
                mechanics.on_battle_start(&mut battle, actor);
            }
        }
        battle.notify_wave_start();
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
            self.list_actions(actor, Action::EnhancedUltimate, &mut actions);
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
        let CheckedAction {
            ability,
            skill_points,
            ultimate_cost_milli,
        } = self.check_action(actor, command.action)?;
        let target = match (ability.target, command.target_id.as_deref()) {
            (Target::Ally | Target::OtherAlly, Some(id)) => {
                let target = self.find_target(self.allies(), id)?;
                if matches!(ability.target, Target::OtherAlly) && target == Some(actor) {
                    return Err(BattleError::InvalidTarget);
                }
                target
            }
            (Target::Enemy, Some(id)) => self.find_target(self.enemies(), id)?,
            (Target::SelfOnly | Target::AllAllies | Target::AllEnemies, None) => None,
            _ => return Err(BattleError::InvalidTarget),
        };

        let mechanics = self.mechanics(actor);
        if let Some(unit) = mechanics
            .as_ref()
            .and_then(|rules| rules.forced_action_target(self, actor, command.action))
            && target != Some(unit)
        {
            return Err(BattleError::InvalidTarget);
        }
        let attacking = ability.effects.iter().any(|effect| effect.deals_damage());
        let keeps_turn = mechanics
            .as_ref()
            .is_some_and(|rules| rules.keeps_turn(command.action));
        let ends_turn = ultimate_cost_milli.is_none() && !keeps_turn;
        self.decision_index += 1;
        if keeps_turn
            && mechanics
                .as_ref()
                .is_some_and(|rules| rules.prepare_action(self, actor, command.action))
        {
            return Ok(());
        }
        self.action = Some(command.action);
        self.action_target = target;
        let parent_debuff_targets = self.begin_debuff_action();
        self.begin_attack();
        self.emit(EventKind::Action(command.action), Some(actor), target);
        match skill_points {
            SkillPoints::Generate => {
                self.restore_skill_points(actor, 1);
            }
            SkillPoints::Spend => self.spend_skill_points(actor, 1),
            SkillPoints::Unchanged => {}
        }
        let extra_turn = ends_turn && self.in_extra_turn(actor);
        if let Some(cost) = ultimate_cost_milli {
            self.set_energy(actor, actor, self.units[actor].energy_milli - cost);
        } else if ends_turn && !extra_turn {
            self.reset_gauge(actor);
        }
        self.notify_ally_targets(actor, target, ability.target, command.action);
        if !keeps_turn || attacking {
            self.activate_attack_bonuses(actor, command.action);
        }
        if let Some(mechanics) = &mechanics {
            mechanics.before_action(self, actor, command.action, target);
        }
        self.notify_ally_action_started(actor, target, ability);
        self.resolve_ability(actor, target, ability);
        self.gain_energy(actor, actor, ability.energy_milli);
        if let Some(mechanics) = &mechanics {
            mechanics.after_action(self, actor, command.action, target);
        }
        if attacking {
            self.notify_attack(actor, command.action, target);
        }
        self.finish_debuff_action(actor, parent_debuff_targets);
        if attacking {
            self.resolve_follow_ups();
        }
        self.notify_ally_action_finished(actor, command.action);
        self.units[actor].active_attack_bonus_bp = 0;
        self.units[actor].active_penetration_bp = 0;
        self.action = None;
        self.action_target = None;
        if extra_turn {
            self.finish_extra_turn();
        } else if ends_turn {
            self.finish_turn(actor);
            self.ordinary_actor = None;
        }
        self.resolve_wave_or_end();
        if self.outcome == Outcome::Ongoing
            && !self.start_extra_turn()
            && self.ordinary_actor.is_none()
        {
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
            skill_point_cap: self.skill_point_cap(),
            random_draws: self.rng.draws,
            enemy_hp_lost: self.enemy_hp_lost,
            units: (0..self.enemies().end)
                .map(|unit| {
                    let state = &self.units[unit];
                    UnitSnapshot {
                        unit_id: self.plan.units[unit].id.clone(),
                        is_ally: unit < self.plan.ally_count,
                        hp: state.hp,
                        max_hp: self.effective_max_hp(unit),
                        energy_milli: state.energy_milli,
                        max_energy_milli: self.plan.units[unit].kit.max_energy_milli(),
                        toughness: state.toughness,
                        max_toughness: self.plan.units[unit].kit.max_toughness(),
                        shield: self.shield_amount(unit),
                        vulnerability_bp: state
                            .debuffs
                            .iter()
                            .filter(|status| status.kind == DebuffKind::Vulnerability)
                            .map(|status| status.bp)
                            .fold(
                                state.magnitude(StatusKind::Vulnerability),
                                u32::saturating_add,
                            ),
                        speed_micros: self.speed_micros(unit),
                        gauge_quanta: state.gauge_quanta,
                        normal_turns: state.normal_turns,
                    }
                })
                .collect(),
        }
    }

    /// Shows the acting unit, an interrupted ordinary turn, then action-gauge order.
    pub fn action_order(&self) -> Vec<ActionOrderEntry> {
        if self.outcome != Outcome::Ongoing {
            return Vec::new();
        }
        let interrupted = self.interrupted_turn_actor();
        let current_scheduled = if self.extra_turn_active() {
            interrupted
        } else {
            self.ordinary_actor
        };
        let mut pending: Vec<_> = self
            .scheduled_turns()
            .filter(|(_, _, slot)| current_scheduled.map(TurnSlot::Unit).as_ref() != Some(slot))
            .collect();
        pending.sort_unstable();
        self.ordinary_actor
            .into_iter()
            .chain(interrupted)
            .map(|unit| (0, false, TurnSlot::Unit(unit)))
            .chain(pending)
            .filter_map(|(remaining_av_micros, _, slot)| {
                let unit_id = match slot {
                    TurnSlot::Unit(unit) => self.plan.units[unit].id.clone(),
                    TurnSlot::Summon(owner) => {
                        let summon = self.units[owner].summon_turn?;
                        format!("summon:{}:{}", self.plan.units[owner].id, summon.name)
                    }
                };
                Some(ActionOrderEntry {
                    unit_id,
                    remaining_av_micros,
                })
            })
            .collect()
    }

    fn check_action(&self, actor: usize, action: Action) -> Result<CheckedAction<'a>, BattleError> {
        if self.units[actor].hp == 0 {
            return Err(BattleError::InvalidActor);
        }
        if self.turn_is_suspended(actor) {
            return Err(BattleError::IllegalTiming);
        }
        let kit = &self.plan.units[actor].kit;
        let (ability, skill_points) = match kit {
            Kit::Ally(kit) => (
                kit.ability(action).ok_or(BattleError::IllegalTiming)?,
                match action {
                    Action::Basic => SkillPoints::Generate,
                    Action::Skill => SkillPoints::Spend,
                    Action::Ultimate | Action::EnhancedUltimate => SkillPoints::Unchanged,
                },
            ),
            Kit::Official(kit) => kit
                .mechanics
                .ability_override(self, actor, action)
                .map(official::OfficialAbility::resolve)
                .or_else(|| kit.ability(action))
                .ok_or(BattleError::IllegalTiming)?,
            Kit::Enemy(_) | Kit::Dummy { .. } => return Err(BattleError::InvalidActor),
        };
        let ultimate_cost_milli = if action.is_ultimate() {
            match self
                .mechanics(actor)
                .and_then(|mechanics| mechanics.alternate_ultimate_resource_ready(self, actor))
            {
                Some(false) => return Err(BattleError::InsufficientUltimateResource),
                Some(true) => Some(0),
                None => {
                    let energy_range = self
                        .mechanics(actor)
                        .and_then(|mechanics| mechanics.ultimate_energy_range(action))
                        .unwrap_or_else(|| kit.max_energy_milli()..=kit.max_energy_milli());
                    let current_energy = self.units[actor].energy_milli;
                    let cost = *energy_range.start();
                    if current_energy < cost {
                        return Err(BattleError::InsufficientEnergy);
                    }
                    if current_energy > *energy_range.end() {
                        return Err(BattleError::IllegalTiming);
                    }
                    Some(cost)
                }
            }
        } else {
            if self.ordinary_actor != Some(actor)
                || self.mechanics(actor).is_some_and(|rules| match action {
                    Action::Basic => !rules.can_use_basic(self, actor),
                    Action::Skill => !rules.can_use_skill(self, actor),
                    Action::Ultimate | Action::EnhancedUltimate => false,
                })
            {
                return Err(BattleError::IllegalTiming);
            }
            let shared_cost = self.mechanics(actor).map_or(0, |rules| {
                rules.shared_skill_point_cost(self, actor, action)
            });
            let cost = shared_cost.saturating_add(u8::from(skill_points == SkillPoints::Spend));
            if self.skill_points < cost {
                return Err(BattleError::InsufficientSkillPoints);
            }
            None
        };
        Ok(CheckedAction {
            ability,
            skill_points,
            ultimate_cost_milli,
        })
    }

    fn list_actions(&self, actor: usize, action: Action, actions: &mut Vec<Command>) {
        let Ok(CheckedAction { ability, .. }) = self.check_action(actor, action) else {
            return;
        };
        let forced_target = self
            .mechanics(actor)
            .and_then(|rules| rules.forced_action_target(self, actor, action));
        let mut add = |target: Option<usize>| {
            actions.push(Command {
                decision_index: self.decision_index,
                actor_id: self.plan.units[actor].id.clone(),
                action,
                target_id: target.map(|unit| self.plan.units[unit].id.clone()),
            })
        };
        match ability.target {
            Target::SelfOnly | Target::AllAllies | Target::AllEnemies => add(None),
            Target::Ally | Target::OtherAlly | Target::Enemy => {
                let range = match ability.target {
                    Target::Ally | Target::OtherAlly => self.allies(),
                    _ => self.enemies(),
                };
                for unit in range.filter(|&unit| {
                    self.is_targetable(unit)
                        && forced_target.is_none_or(|forced| forced == unit)
                        && (!matches!(ability.target, Target::OtherAlly) || unit != actor)
                }) {
                    add(Some(unit));
                }
            }
        }
    }

    fn find_target(&self, mut units: Range<usize>, id: &str) -> Result<Option<usize>, BattleError> {
        units
            .find(|&unit| self.is_targetable(unit) && self.plan.units[unit].id == id)
            .map(Some)
            .ok_or(BattleError::InvalidTarget)
    }

    fn allies(&self) -> Range<usize> {
        0..self.plan.ally_count
    }

    fn is_targetable(&self, unit: usize) -> bool {
        let state = &self.units[unit];
        state.hp > 0 && !state.departed
    }

    fn available_for_ordinary_turn(&self, unit: usize) -> bool {
        self.is_targetable(unit) && !self.turn_is_suspended(unit)
    }

    fn turn_is_suspended(&self, unit: usize) -> bool {
        matches!(&self.plan.units[unit].kit, Kit::Official(kit)
            if kit.mechanics.suspends_turn(self, unit))
    }

    fn mechanics(&self, unit: usize) -> Option<Arc<dyn Mechanics>> {
        match &self.plan.units[unit].kit {
            Kit::Official(kit) => Some(Arc::clone(&kit.mechanics)),
            _ => None,
        }
    }

    fn hp(&self, unit: usize) -> (u32, u32) {
        (self.units[unit].hp, self.effective_max_hp(unit))
    }

    fn defense(&self, unit: usize) -> u32 {
        self.effective_defense(unit)
    }

    fn has_weakness(&self, unit: usize, element: crate::fixture::Element) -> bool {
        self.plan.units[unit].kit.matches_weakness(element)
            || self.units[unit]
                .debuffs
                .iter()
                .any(|status| status.kind == combat_status::DebuffKind::ImplantedWeakness(element))
    }

    fn is_elite(&self, unit: usize) -> bool {
        self.plan.units[unit].kit.max_toughness() >= 120
    }

    fn in_follow_up(&self) -> bool {
        self.resolving_follow_up
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
            (Target::SelfOnly, None) => source..source + 1,
            (Target::Ally | Target::OtherAlly, Some(unit)) => unit..unit + 1,
            (Target::AllAllies, None) => self.allies(),
            _ => return,
        };
        for owner in targets {
            if self.is_targetable(owner)
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
            let after_this_turn = u32::from(!action.is_ultimate());
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

    fn current_target(&self) -> Option<usize> {
        self.action_target
    }

    fn attack_targets(&self) -> &[usize] {
        &self.attack_targets
    }

    fn begin_attack(&mut self) {
        self.attack_targets.clear();
        self.super_break_targets.clear();
        self.attack_index += 1;
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
        let bonus_bp = bonus_bp
            + u64::from(
                self.mechanics(unit)
                    .map_or(0, |mechanics| mechanics.speed_bonus_bp(self, unit)),
            );
        let slow_bp: u64 = self.units[unit]
            .debuffs
            .iter()
            .filter(|status| status.kind.is_speed_down())
            .map(|status| u64::from(status.bp))
            .sum();
        let (flat_buff, ratio_buff) = self.support_speed(unit);
        let speed = stats.base_speed_micros
            + stats.flat_speed_micros
            + stats.base_speed_micros * (bonus_bp + ratio_buff) / 10_000
            + flat_buff;
        speed
            .saturating_sub(stats.base_speed_micros * slow_bp / 10_000)
            .max(1_000_000)
    }

    fn turn_candidates(&self) -> impl Iterator<Item = usize> {
        self.allies()
            .chain(self.enemies())
            .filter(|&unit| self.available_for_ordinary_turn(unit))
    }

    fn scheduled_turns(&self) -> impl Iterator<Item = (u64, bool, TurnSlot)> {
        let units = self.turn_candidates().map(|unit| {
            let state = &self.units[unit];
            (
                state.gauge_quanta.div_ceil(self.speed_micros(unit)),
                !state.advanced_to_front,
                TurnSlot::Unit(unit),
            )
        });
        let summons = self.allies().filter_map(|owner| {
            let state = &self.units[owner];
            let summon = state.summon_turn.filter(|_| state.hp > 0)?;
            Some((
                summon.gauge_quanta.div_ceil(summon.speed_micros),
                !summon.advanced_to_front,
                TurnSlot::Summon(owner),
            ))
        });
        units.chain(summons)
    }

    fn advance_to_decision(&mut self) {
        while self.outcome == Outcome::Ongoing {
            if self.start_extra_turn() {
                return;
            }
            if self.normal_turns == TURN_LIMIT {
                self.end(Outcome::TurnLimit);
                return;
            }
            let Some((delta, _, slot)) = self.scheduled_turns().min() else {
                self.end(Outcome::Defeat);
                return;
            };

            self.av_micros += delta;
            for unit in self.allies().chain(self.enemies()) {
                if self.available_for_ordinary_turn(unit) {
                    let consumed = delta * self.speed_micros(unit);
                    self.units[unit].gauge_quanta =
                        self.units[unit].gauge_quanta.saturating_sub(consumed);
                }
            }
            self.normal_turns += 1;
            for owner in self.allies() {
                let state = &mut self.units[owner];
                if state.hp > 0
                    && let Some(summon) = &mut state.summon_turn
                {
                    summon.gauge_quanta = summon
                        .gauge_quanta
                        .saturating_sub(delta * summon.speed_micros);
                }
            }
            let actor = match slot {
                TurnSlot::Unit(actor) => actor,
                TurnSlot::Summon(owner) => {
                    if let Some(summon) = &mut self.units[owner].summon_turn {
                        summon.gauge_quanta = GAUGE_FULL;
                        summon.advanced_to_front = false;
                    }
                    let parent_debuff_targets = self.begin_debuff_action();
                    if let Some(mechanics) = self.mechanics(owner) {
                        mechanics.on_summon_turn(self, owner);
                    }
                    self.finish_debuff_action(owner, parent_debuff_targets);
                    self.resolve_wave_or_end();
                    continue;
                }
            };
            self.units[actor].normal_turns += 1;
            self.units[actor].advanced_to_front = false;
            self.emit(EventKind::TurnStarted, Some(actor), None);

            let kit = &self.plan.units[actor].kit;
            match kit {
                Kit::Ally(_) | Kit::Official(_) => {
                    self.tick_heals_over_time(actor);
                    self.notify_ally_turn_opening(actor, false);
                    if self.thaw(actor) {
                        self.finish_turn(actor);
                        self.resolve_wave_or_end();
                        continue;
                    }
                    for owner in self.allies() {
                        if let Some(mechanics) = self.mechanics(owner) {
                            mechanics.on_turn_start(self, owner, actor);
                        }
                    }
                    self.ordinary_actor = Some(actor);
                    return;
                }
                Kit::Enemy(_) | Kit::Dummy { .. } => {
                    self.enemy_turn = Some(actor);
                    self.resolve_enemy_turn(actor);
                    self.enemy_turn = None;
                    self.resolve_wave_or_end();
                }
            }
        }
    }

    fn resolve_enemy_turn(&mut self, actor: usize) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_enemy_turn_opening(self, owner, actor);
            }
        }
        self.dot_timing = DotTiming::TurnStart(actor);
        self.tick_shocks(actor);
        self.tick_skill_dots(actor);
        self.tick_break_dots(actor);
        self.trigger_special_dots(actor, SpecialDotTrigger::TurnStart);
        self.dot_timing = DotTiming::Other;
        self.tick_entanglement(actor);
        if self.units[actor].hp == 0 {
            return;
        }
        if !self.thaw(actor) {
            let kit = &self.plan.units[actor].kit;
            if self.units[actor].toughness == 0 {
                let extends_break = self.allies().any(|owner| {
                    self.mechanics(owner)
                        .is_some_and(|rules| rules.prevent_toughness_recovery(self, owner, actor))
                });
                if extends_break {
                    if self.units[actor].hp > 0 {
                        self.finish_turn(actor);
                    }
                    return;
                }
                self.set_toughness(actor, actor, kit.max_toughness());
            }
            self.reset_gauge(actor);
            let parent_debuff_targets = self.begin_debuff_action();
            for owner in self.allies() {
                if self.units[owner].hp > 0
                    && let Some(mechanics) = self.mechanics(owner)
                {
                    mechanics.on_enemy_turn_start(self, owner, actor);
                }
            }
            if let Kit::Enemy(enemy) = kit {
                self.resolve_enemy(actor, enemy);
            }
            self.finish_debuff_action(actor, parent_debuff_targets);
        }
        self.finish_turn(actor);
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
                self.notify_wave_start();
            }
        }
    }

    fn notify_wave_start(&mut self) {
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_wave_start(self, owner);
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

    fn skill_point_cap(&self) -> u8 {
        SKILL_POINT_CAP
            + self
                .allies()
                .filter(|&actor| self.is_targetable(actor))
                .map(|actor| self.plan.units[actor].kit.skill_point_cap_bonus())
                .sum::<u8>()
    }

    // Only the caller decides whether overflow is banked or discarded.
    fn restore_skill_points(&mut self, actor: usize, points: u8) -> u8 {
        let restored = points.min(self.skill_point_cap().saturating_sub(self.skill_points));
        self.set_skill_points(actor, self.skill_points + restored);
        restored
    }

    fn set_skill_points(&mut self, actor: usize, after: u8) {
        let before = std::mem::replace(&mut self.skill_points, after);
        if before != after {
            self.emit(EventKind::SkillPoints { before, after }, Some(actor), None);
        }
    }

    fn spend_skill_points(&mut self, actor: usize, points: u8) {
        self.spend_skill_points_with_substitute(actor, points, 0);
    }

    fn spend_skill_points_with_substitute(&mut self, actor: usize, shared: u8, substitute: u8) {
        self.set_skill_points(actor, self.skill_points - shared);
        let points = shared + substitute;
        for owner in self.allies() {
            if self.units[owner].hp > 0
                && let Some(mechanics) = self.mechanics(owner)
            {
                mechanics.on_ally_skill_points_spent(self, owner, actor, points);
            }
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
