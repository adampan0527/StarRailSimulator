use std::num::NonZeroU32;

use star_rail_battle_core::{
    Action, Battle, Command, DamageKind, LogMode, Outcome, PreparedBattle, Session, calculate_dpr,
    search_fastest_victory,
};

const FIXTURE: &str = include_str!("../../../contracts/battle-session.v1.example.json");

fn session() -> Session {
    Session::from_json(FIXTURE).unwrap()
}

#[test]
fn dpr_targets_survive_repeated_breaks_and_grant_no_kill_energy() {
    let mut session = session();
    session.input.team.retain(|ally| ally.unit_id == "ally.3");
    // A kill energy bonus on the first hit would make the ultimate available at 160 AV.
    session.input.team[0].starting_energy_milli = 60_000;
    let plan = PreparedBattle::new(&session.input).unwrap();

    let first = calculate_dpr(&plan, &[20], NonZeroU32::MIN).unwrap();
    assert_eq!(first.total_damage, 525);
    assert_eq!(first.dpr, 525.0);

    let full = calculate_dpr(&plan, &[20], NonZeroU32::new(3).unwrap()).unwrap();
    let hits: Vec<_> = full.targets[0]
        .hits
        .iter()
        .map(|hit| (hit.av_micros, hit.action, hit.kind, hit.amount))
        .collect();
    use DamageKind::{Direct, WeaknessBreak};
    assert_eq!(
        hits,
        [
            (80_000_000, Action::Skill, Direct, 450),
            (80_000_000, Action::Skill, WeaknessBreak, 75),
            (160_000_000, Action::Skill, Direct, 450),
            (160_000_000, Action::Skill, WeaknessBreak, 75),
            (240_000_000, Action::Ultimate, Direct, 1_125),
            (240_000_000, Action::Skill, Direct, 625),
            (320_000_000, Action::Basic, Direct, 225),
        ]
    );
    assert_eq!(full.total_damage, 3_025);
    assert_eq!(full.dpr, 3_025.0 / 3.0);
    assert_eq!(full.characters[0].own_damage, 3_025);
    assert_eq!(full.characters[0].assist_damage, 0);
    assert_eq!(full.characters[0].dpr, full.dpr);
}

#[test]
fn dpr_credits_the_debuff_owner_for_direct_and_break_damage() {
    let mut session = session();
    session
        .input
        .team
        .retain(|ally| ["ally.2", "ally.3"].contains(&ally.unit_id.as_str()));
    let plan = PreparedBattle::new(&session.input).unwrap();
    let result = calculate_dpr(&plan, &[30; 3], NonZeroU32::MIN).unwrap();
    let [left, middle, right] = result.targets.as_slice() else {
        panic!("expected three dummies")
    };
    assert!(left.hits.is_empty() && right.hits.is_empty());
    // Dan Heng deals 450 and applies 25% vulnerability. March deals floor(90 * 1.25)
    // and breaks for floor(62.5 * 1.25). The unassisted integers are 90 and 62.
    assert_eq!(middle.damage, 640);
    for (hit, kind, damage, own, assist) in [
        (&middle.hits[1], DamageKind::Direct, 112, 90, 22),
        (&middle.hits[2], DamageKind::WeaknessBreak, 78, 62, 16),
    ] {
        assert_eq!(
            (hit.source_id.as_str(), hit.kind, hit.amount),
            ("ally.2", kind, damage)
        );
        assert_eq!(
            hit.credits
                .iter()
                .map(|credit| (credit.unit_id.as_str(), credit.amount))
                .collect::<Vec<_>>(),
            [("ally.2", own), ("ally.3", assist)]
        );
    }
    assert_eq!(result.characters[0].dpr, 152.0);
    assert_eq!(result.characters[1].dpr, 488.0);
    assert_eq!(result.dpr, 640.0);
}

#[test]
fn dpr_logs_area_damage_by_target_and_excludes_the_window_boundary() {
    let mut session = session();
    session.input.team.retain(|ally| ally.unit_id == "ally.2");
    session.input.team[0].starting_energy_milli = 120_000;
    let plan = PreparedBattle::new(&session.input).unwrap();
    let result = calculate_dpr(&plan, &[30, 120, 30], NonZeroU32::MIN).unwrap();
    assert_eq!(
        result
            .targets
            .iter()
            .map(|target| target.damage)
            .collect::<Vec<_>>(),
        [162, 252, 162]
    );
    assert!(
        result
            .targets
            .iter()
            .all(|target| target.hits[0].action == Action::Ultimate)
    );
    assert_eq!(result.total_damage, 576);
    assert_eq!(
        result,
        calculate_dpr(&plan, &[30, 120, 30], NonZeroU32::MIN).unwrap()
    );
    for invalid in [&[][..], &[0], &[30; 6]] {
        assert!(calculate_dpr(&plan, invalid, NonZeroU32::MIN).is_err());
    }

    let mut session = self::session();
    session.input.team.retain(|ally| ally.unit_id == "ally.4");
    let plan = PreparedBattle::new(&session.input).unwrap();
    let at_250 = calculate_dpr(&plan, &[120], NonZeroU32::new(2).unwrap()).unwrap();
    assert_eq!(at_250.targets[0].hits.len(), 1);
    assert_eq!(at_250.targets[0].hits[0].av_micros, 125_000_000);
    assert_eq!(at_250.total_damage, 135);
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
