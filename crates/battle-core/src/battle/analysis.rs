use std::{cmp::Reverse, collections::BinaryHeap, num::NonZeroU32};

use serde::Serialize;

use super::{Battle, Kit, PreparedBattle};
use crate::{
    Action, BattleError, Command, EventKind, LogMode, Outcome,
    fixture::{Effect, EnemyTarget},
};

const AV_MICROS: u64 = 1_000_000;

#[derive(Serialize)]
pub struct DprResult {
    pub cycles: u32,
    pub enemy_hp_lost: u32,
    pub dpr: f64,
}

#[derive(Serialize)]
pub struct VictorySolution {
    pub av_micros: u64,
    pub commands: Vec<Command>,
}

#[derive(Serialize)]
pub struct VictorySearch {
    pub exact: bool,
    pub evaluated: u32,
    pub budget: u32,
    pub victory: Option<VictorySolution>,
}

struct SearchNode<'a> {
    battle: Option<Battle<'a>>,
    step: Option<(usize, Command)>,
}

/// Runs a fixed offensive policy (damaging ultimate, damaging skill, basic; first target).
/// The window starts at 0 AV and excludes its end, with 150 AV for the first cycle
/// and 100 AV for each subsequent cycle. Only actual enemy HP removed counts.
pub fn calculate_dpr(plan: &PreparedBattle, cycles: NonZeroU32) -> Result<DprResult, BattleError> {
    let end = (150 + 100 * (u64::from(cycles.get()) - 1)) * AV_MICROS;
    let mut battle = Battle::start(plan, LogMode::Detailed);
    while battle.outcome == Outcome::Ongoing && battle.av_micros < end {
        let actions = battle.legal_actions();
        let command = actions
            .iter()
            .min_by_key(|command| offensive_priority(&battle, command))
            .ok_or(BattleError::InvalidActor)?;
        battle.apply(command)?;
    }
    let enemy_hp_lost = battle
        .events()
        .iter()
        .take_while(|event| event.av_micros < end)
        .filter_map(|event| match event.kind {
            EventKind::Damage { hp_loss, .. }
                if plan.units[plan.ally_count..]
                    .iter()
                    .any(|unit| event.target_id.as_ref() == Some(&unit.id)) =>
            {
                Some(hp_loss)
            }
            _ => None,
        })
        .sum();
    Ok(DprResult {
        cycles: cycles.get(),
        enemy_hp_lost,
        dpr: f64::from(enemy_hp_lost) / f64::from(cycles.get()),
    })
}

fn offensive_priority(battle: &Battle<'_>, command: &Command) -> u8 {
    let damaging = battle
        .allies()
        .find(|&unit| battle.plan.units[unit].id == command.actor_id)
        .and_then(|unit| battle.check_action(unit, command.action).ok())
        .is_some_and(|ability| {
            ability
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Hit { .. }))
        });
    match (command.action, damaging) {
        (Action::Ultimate, true) => 0,
        (Action::Skill, true) => 1,
        (Action::Basic, _) => 2,
        _ => 3,
    }
}

/// Whether all reachable decisions can be evaluated without predicting a random outcome.
fn supports_fastest_victory(plan: &PreparedBattle) -> bool {
    plan.units.iter().all(|unit| {
        unit.stats.crit_rate_bp == 0
            && match unit.kit {
                Kit::Ally(_) => true,
                Kit::Enemy(kit) => {
                    matches!(kit.target, EnemyTarget::FirstLiving) || plan.ally_count == 1
                }
            }
    })
}

/// Searches legal commands by elapsed AV, a lower bound for every descendant.
/// The budget counts applied commands. If it runs out, callers must label any victory
/// as the best found within the budget, and an empty result as inconclusive.
pub fn search_fastest_victory(
    plan: &PreparedBattle,
    budget: NonZeroU32,
) -> Result<VictorySearch, BattleError> {
    if !supports_fastest_victory(plan) {
        return Err(BattleError::UnsupportedData(
            "randomized victory search".into(),
        ));
    }
    let battle = Battle::start(plan, LogMode::Off);
    let mut pending = BinaryHeap::from([Reverse((battle.av_micros, 0))]);
    let mut nodes = vec![SearchNode {
        battle: Some(battle),
        step: None,
    }];
    let mut result = VictorySearch {
        exact: true,
        evaluated: 0,
        budget: budget.get(),
        victory: None,
    };

    'search: while let Some(Reverse((av, index))) = pending.pop() {
        if result
            .victory
            .as_ref()
            .is_some_and(|best| av >= best.av_micros)
        {
            break;
        }
        let Some(battle) = nodes[index].battle.take() else {
            continue;
        };
        for command in battle.legal_actions() {
            if result.evaluated == result.budget {
                result.exact = false;
                break 'search;
            }
            result.evaluated += 1;
            let mut next = battle.clone();
            next.apply(&command)?;
            if result
                .victory
                .as_ref()
                .is_some_and(|best| next.av_micros >= best.av_micros)
            {
                continue;
            }
            match next.outcome {
                Outcome::Ongoing => {
                    pending.push(Reverse((next.av_micros, nodes.len())));
                    nodes.push(SearchNode {
                        battle: Some(next),
                        step: Some((index, command)),
                    });
                }
                Outcome::Victory => {
                    let mut commands = vec![command];
                    let mut ancestor = index;
                    while let Some((parent, command)) = &nodes[ancestor].step {
                        commands.push(command.clone());
                        ancestor = *parent;
                    }
                    commands.reverse();
                    result.victory = Some(VictorySolution {
                        av_micros: next.av_micros,
                        commands,
                    });
                }
                Outcome::Defeat | Outcome::TurnLimit => {}
            }
            if result
                .victory
                .as_ref()
                .is_some_and(|best| best.av_micros == av)
            {
                // The popped node is the earliest remaining state, including its unexpanded actions.
                break 'search;
            }
        }
    }
    Ok(result)
}
