use std::num::NonZeroU32;

use star_rail_battle_core::{
    Battle, Command, LogMode, Outcome, PreparedBattle, Session, calculate_dpr,
    search_fastest_victory,
};

const FIXTURE: &str = include_str!("../../../contracts/battle-session.v1.example.json");

fn session() -> Session {
    Session::from_json(FIXTURE).unwrap()
}

#[test]
fn dpr_counts_effective_damage_and_uses_the_full_cycle_window() {
    let mut session = session();
    session.input.team.retain(|ally| ally.unit_id == "ally.3");
    session.input.waves[0].truncate(1);
    let plan = PreparedBattle::new(&session.input).unwrap();

    // At 80 AV, Dan Heng's skill deals 450 direct and 75 break damage.
    let first = calculate_dpr(&plan, NonZeroU32::MIN).unwrap();
    assert_eq!(first.enemy_hp_lost, 525);
    assert_eq!(first.dpr, 525.0);

    // The second skill kills the 1,000 HP target. Overkill and the empty
    // portion of the three-cycle window do not inflate the reported DPR.
    let full = calculate_dpr(&plan, NonZeroU32::new(3).unwrap()).unwrap();
    assert_eq!(full.enemy_hp_lost, 1_000);
    assert_eq!(full.dpr, 1_000.0 / 3.0);
}

fn exhaustive_minimum(plan: &PreparedBattle, prefix: &mut Vec<Command>) -> Option<u64> {
    let battle = Battle::replay(plan, prefix, LogMode::Off).unwrap();
    let snapshot = battle.snapshot();
    if snapshot.av_micros > 125_000_000 {
        return None;
    }
    match snapshot.outcome {
        Outcome::Victory => Some(snapshot.av_micros),
        Outcome::Defeat | Outcome::TurnLimit => None,
        Outcome::Ongoing => battle
            .legal_actions()
            .into_iter()
            .filter_map(|action| {
                prefix.push(action);
                let result = exhaustive_minimum(plan, prefix);
                prefix.pop();
                result
            })
            .min(),
    }
}

#[test]
fn fastest_victory_matches_independent_enumeration_and_replays() {
    let mut session = session();
    session.input.waves[0].truncate(1);
    let plan = PreparedBattle::new(&session.input).unwrap();

    let budget = NonZeroU32::new(20_000).unwrap();
    let search = search_fastest_victory(&plan, budget).unwrap();
    let solution = search.victory.unwrap();
    assert!(search.exact);
    assert_eq!(
        Some(solution.av_micros),
        exhaustive_minimum(&plan, &mut vec![])
    );
    assert_eq!(solution.av_micros, 108_342_362);
    let replay = Battle::replay(&plan, &solution.commands, LogMode::Detailed).unwrap();
    assert_eq!(replay.outcome(), Outcome::Victory);
    assert_eq!(replay.snapshot().av_micros, solution.av_micros);
    assert_eq!(replay.snapshot().enemy_hp_lost, 1_000);
    let repeated = search_fastest_victory(&plan, budget).unwrap();
    assert_eq!(repeated.evaluated, search.evaluated);
    assert_eq!(repeated.victory.unwrap().commands, solution.commands);

    let limited = search_fastest_victory(&plan, NonZeroU32::MIN).unwrap();
    assert_eq!(limited.evaluated, 1);
    assert!(!limited.exact);
    assert!(limited.victory.is_none());
}

#[test]
fn fastest_victory_requires_every_wave_and_rejects_random_encounters() {
    let mut session = session();
    let plan = PreparedBattle::new(&session.input).unwrap();
    assert!(search_fastest_victory(&plan, NonZeroU32::MIN).is_err());

    session.input.team.retain(|ally| ally.unit_id == "ally.3");
    session.input.waves[0].truncate(1);
    let mut next_enemy = session.input.waves[0][0].clone();
    next_enemy.unit_id = "enemy.2.1".into();
    session.input.waves.push(vec![next_enemy]);
    let plan = PreparedBattle::new(&session.input).unwrap();
    let search = search_fastest_victory(&plan, NonZeroU32::new(20_000).unwrap()).unwrap();
    assert!(search.exact);
    let solution = search.victory.unwrap();
    let replay = Battle::replay(&plan, &solution.commands, LogMode::Off).unwrap();
    assert_eq!(replay.outcome(), Outcome::Victory);
    assert_eq!(replay.snapshot().enemy_hp_lost, 2_000);
}
