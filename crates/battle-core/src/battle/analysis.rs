use std::{cmp::Reverse, collections::BinaryHeap, num::NonZeroU32};

use serde::Serialize;

use super::{Battle, Kit, PreparedBattle};
use crate::{
    Action, BattleError, Command, DamageKind, LogMode, Outcome,
    fixture::{Effect, EnemyTarget},
};

const AV_MICROS: u64 = 1_000_000;

#[derive(Serialize)]
pub struct DprScenario {
    pub cycles: NonZeroU32,
    pub toughness: [u16; 3],
}

pub const STANDARD_DPR_SCENARIO: DprScenario = DprScenario {
    cycles: NonZeroU32::new(3).unwrap(),
    toughness: [30, 120, 30],
};

#[derive(Debug, PartialEq, Serialize)]
pub struct DprResult {
    pub cycles: u32,
    pub total_damage: u64,
    pub dpr: f64,
    pub characters: Vec<DprCharacter>,
    pub targets: Vec<DprTarget>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DprCharacter {
    pub unit_id: String,
    pub own_damage: u64,
    pub assist_damage: u64,
    pub dpr: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DprTarget {
    pub max_toughness: u16,
    pub damage: u64,
    pub hits: Vec<DprHit>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DprHit {
    pub av_micros: u64,
    pub source_id: String,
    pub action: Action,
    pub kind: DamageKind,
    pub critical: bool,
    pub amount: u32,
    pub credits: Vec<DprCredit>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DprCredit {
    pub unit_id: String,
    pub amount: u32,
}

#[derive(Clone)]
pub(super) struct DummyDamage {
    pub av_micros: u64,
    pub actor: usize,
    pub target: usize,
    pub kind: DamageKind,
    pub critical: bool,
    pub amount: u32,
    pub assists: Vec<(usize, u32)>,
    pub action: Action,
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

/// Evaluates the frozen team against one to five indestructible dummies.
/// Uses the fixed offensive policy, with single-target attacks aimed at the middle dummy.
/// The window is half-open, with 150 AV for the first cycle and 100 AV thereafter.
pub fn calculate_dpr(
    plan: &PreparedBattle,
    toughness: &[u16],
    cycles: NonZeroU32,
) -> Result<DprResult, BattleError> {
    let plan = plan.with_dummies(toughness)?;
    let end = (150 + 100 * (u64::from(cycles.get()) - 1)) * AV_MICROS;
    let mut battle = Battle::start(&plan, LogMode::Off);
    let mut characters: Vec<_> = plan.units[..plan.ally_count]
        .iter()
        .map(|unit| DprCharacter {
            unit_id: unit.id.clone(),
            own_damage: 0,
            assist_damage: 0,
            dpr: 0.0,
        })
        .collect();
    let mut targets: Vec<_> = toughness
        .iter()
        .map(|&max_toughness| DprTarget {
            max_toughness,
            damage: 0,
            hits: Vec::new(),
        })
        .collect();
    let middle = &plan.units[plan.ally_count + (targets.len() - 1) / 2].id;
    while battle.outcome == Outcome::Ongoing && battle.av_micros < end {
        let actions = battle.legal_actions();
        let command = actions
            .iter()
            .min_by_key(|command| {
                (
                    offensive_priority(&battle, command),
                    command.target_id.as_ref() != Some(middle),
                )
            })
            .ok_or(BattleError::InvalidActor)?;
        battle.apply(command)?;
        for hit in battle
            .dummy_damage
            .drain(..)
            .filter(|hit| hit.av_micros < end)
        {
            let own_damage =
                hit.amount - hit.assists.iter().map(|&(_, amount)| amount).sum::<u32>();
            let actor = &mut characters[hit.actor];
            actor.own_damage += u64::from(own_damage);
            let mut credits = vec![DprCredit {
                unit_id: actor.unit_id.clone(),
                amount: own_damage,
            }];
            for (owner, amount) in hit.assists {
                characters[owner].assist_damage += u64::from(amount);
                credits.push(DprCredit {
                    unit_id: characters[owner].unit_id.clone(),
                    amount,
                });
            }
            let target = &mut targets[hit.target - plan.ally_count];
            target.damage += u64::from(hit.amount);
            target.hits.push(DprHit {
                av_micros: hit.av_micros,
                source_id: plan.units[hit.actor].id.clone(),
                action: hit.action,
                kind: hit.kind,
                critical: hit.critical,
                amount: hit.amount,
                credits,
            });
        }
    }
    let rounds = f64::from(cycles.get());
    for character in &mut characters {
        character.dpr = (character.own_damage + character.assist_damage) as f64 / rounds;
    }
    let total_damage: u64 = targets.iter().map(|target| target.damage).sum();
    Ok(DprResult {
        cycles: cycles.get(),
        total_damage,
        dpr: total_damage as f64 / rounds,
        characters,
        targets,
    })
}

fn offensive_priority(battle: &Battle<'_>, command: &Command) -> u8 {
    let damaging = battle
        .allies()
        .find(|&unit| battle.plan.units[unit].id == command.actor_id)
        .and_then(|unit| battle.check_action(unit, command.action).ok())
        .is_some_and(|(ability, _)| {
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
            && match &unit.kit {
                Kit::Ally(_) => true,
                Kit::Enemy(kit) => {
                    matches!(kit.target, EnemyTarget::FirstLiving) || plan.ally_count == 1
                }
                Kit::Dummy { .. } | Kit::Official(_) => false,
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
