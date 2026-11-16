use star_rail_battle_core::{
    Action, Battle, BattleError, Command, DamageKind, EventKind, LogMode, Outcome, PreparedBattle,
    Session, StatusKind,
};

const SHARED: &str = include_str!("../../../contracts/battle-session.v1.example.json");
const COMMANDS: &str = include_str!("../fixtures/full-replay.commands.json");

fn shared() -> Session {
    Session::from_json(SHARED).unwrap()
}

fn command(index: u32, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

#[test]
fn accepts_the_shared_v1_prefix_and_freezes_equipment_speed() {
    let session = shared();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let battle = Battle::replay(&setup, &session.commands, LogMode::Detailed).unwrap();
    assert_eq!(battle.ordinary_actor(), Some("ally.1"));
    let state = battle.snapshot();
    // ceil(10000 / (90 + 2.3) × 1e6)
    assert_eq!(state.av_micros, 108_342_362);
    assert_eq!(state.skill_points, 3);
    assert_eq!(state.units[2].energy_milli, 80_000);
    assert_eq!(state.units[2].hp, 960);
    assert_eq!(state.units[2].shield, 0);
}

#[test]
fn full_fixture_matches_independent_turn_order_and_damage() {
    let session = shared();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let commands = serde_json::from_str::<Vec<Command>>(COMMANDS).unwrap();
    assert_eq!(&commands[..2], &session.commands);
    let battle = Battle::replay(&setup, &commands, LogMode::Detailed).unwrap();
    let state = battle.snapshot();
    assert_eq!(state.outcome, Outcome::Victory);
    assert_eq!(state.av_micros, 216_684_724);
    assert_eq!(state.decision_index, 8);
    assert_eq!(state.skill_points, 4);
    assert_eq!(state.enemy_hp_lost, 1_950);
    assert_eq!(state.units[2].energy_milli, 15_000);
    let turns: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::TurnStarted)
        .map(|event| (event.av_micros, event.source_id.as_deref().unwrap()))
        .collect();
    assert_eq!(
        turns,
        [
            (80_000_000, "ally.3"),
            (100_000_000, "ally.2"),
            (100_000_000, "enemy.1.1"),
            (100_000_000, "enemy.1.2"),
            (108_342_362, "ally.1"),
            (125_000_000, "ally.4"),
            (160_000_000, "ally.3"),
            (200_000_000, "ally.2"),
            (216_684_724, "ally.1"),
        ]
    );
    let hits: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind,
                amount,
                hp_loss,
                absorbed,
                ..
            } => Some((
                event.target_id.as_deref().unwrap(),
                kind,
                amount,
                hp_loss,
                absorbed,
            )),
            _ => None,
        })
        .collect();
    use DamageKind::{Direct, WeaknessBreak};
    assert_eq!(
        hits,
        [
            ("enemy.1.1", Direct, 225, 225, 0),
            ("ally.1", Direct, 100, 100, 0),
            ("ally.3", Direct, 100, 40, 60),
            ("enemy.1.2", Direct, 247, 247, 0),
            ("enemy.1.2", WeaknessBreak, 75, 75, 0),
            ("enemy.1.2", Direct, 150, 150, 0),
            ("enemy.1.1", Direct, 450, 450, 0),
            ("enemy.1.1", WeaknessBreak, 75, 75, 0),
            ("enemy.1.1", Direct, 1_125, 250, 0),
            ("enemy.1.2", Direct, 80, 80, 0),
            ("enemy.1.2", Direct, 660, 398, 0),
        ]
    );
    let breaks: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::WeaknessBroken)
        .map(|event| (event.av_micros, event.target_id.as_deref().unwrap()))
        .collect();
    assert_eq!(
        breaks,
        [(108_342_362, "enemy.1.2"), (160_000_000, "enemy.1.1")]
    );
    assert!(
        battle
            .events()
            .iter()
            .any(|e| e.kind == EventKind::StatusApplied(StatusKind::Vulnerability))
    );
}

#[test]
fn replay_and_silent_mode_agree_and_seed_controls_enemy_target() {
    let mut session = shared();
    let commands = serde_json::from_str::<Vec<Command>>(COMMANDS).unwrap();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let first = Battle::replay(&setup, &commands, LogMode::Detailed).unwrap();
    let repeat = Battle::replay(&setup, &commands, LogMode::Detailed).unwrap();
    let silent = Battle::replay(&setup, &commands, LogMode::Off).unwrap();
    assert_eq!(first.events(), repeat.events());
    assert_eq!(first.snapshot(), repeat.snapshot());
    assert_eq!(first.snapshot(), silent.snapshot());
    assert_eq!(first.snapshot().random_draws, 1);
    assert!(silent.events().is_empty());

    session.input.seed = 0; // First SplitMix64 output selects fourth of four allies.
    let alternate_setup = PreparedBattle::new(&session.input).unwrap();
    let alternate = Battle::replay(&alternate_setup, &session.commands, LogMode::Detailed).unwrap();
    assert_eq!(alternate.snapshot().units[3].hp, 900);
    assert_eq!(alternate.snapshot().units[2].shield, 60);
}

#[test]
fn rejected_commands_do_not_advance_or_consume_resources() {
    let mut session = shared();
    session.input.starting_skill_points = 0;
    let setup = PreparedBattle::new(&session.input).unwrap();
    let mut battle = Battle::start(&setup, LogMode::Detailed);
    let before = battle.snapshot();
    let events = battle.events().to_vec();
    let legal = battle.legal_actions();
    for (invalid, expected) in [
        (
            command(1, "ally.3", Action::Basic, Some("enemy.1.1")),
            BattleError::StaleDecision,
        ),
        (
            command(0, "ally.2", Action::Basic, Some("enemy.1.1")),
            BattleError::IllegalTiming,
        ),
        (
            command(0, "ally.3", Action::Skill, Some("enemy.1.1")),
            BattleError::InsufficientSkillPoints,
        ),
        (
            command(0, "ally.3", Action::Ultimate, Some("enemy.1.1")),
            BattleError::InsufficientEnergy,
        ),
        (
            command(0, "ally.3", Action::Basic, Some("ally.2")),
            BattleError::InvalidTarget,
        ),
    ] {
        assert_eq!(battle.apply(&invalid), Err(expected));
        assert_eq!(battle.snapshot(), before);
        assert_eq!(battle.events(), events);
        assert_eq!(battle.legal_actions(), legal);
    }
    let valid = command(0, "ally.3", Action::Basic, Some("enemy.1.1"));
    assert!(legal.contains(&valid));
    battle.apply(&valid).unwrap();
    let reference = Battle::replay(&setup, &[valid], LogMode::Detailed).unwrap();
    assert_eq!(battle.events(), reference.events());
    assert_eq!(battle.snapshot(), reference.snapshot());
}

#[test]
fn advance_changes_gauge_without_waiting_a_percentage_of_remaining_av() {
    let mut session = shared();
    session.input.team[1].character_form_id = "fixture.support".into();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let mut battle = Battle::start(&setup, LogMode::Detailed);
    battle.apply(&session.commands[0]).unwrap();
    // Support acts at 100 AV. Natasha's remaining gauge is 2000, so a 2500 advance brings her to the front.
    battle
        .apply(&command(1, "ally.2", Action::Skill, Some("ally.4")))
        .unwrap();
    assert_eq!(battle.ordinary_actor(), Some("ally.4"));
    assert_eq!(battle.snapshot().av_micros, 100_000_000);
}

#[test]
fn invalid_versions_and_data_fail_before_battle() {
    let mut session = shared();
    session.input.rng_version = "fixture.rng.future".into();
    assert!(matches!(
        PreparedBattle::new(&session.input),
        Err(BattleError::UnsupportedVersion {
            field: "rng_version",
            ..
        })
    ));
    let mut session = shared();
    session.input.waves[0][1].unit_id = session.input.waves[0][0].unit_id.clone();
    assert!(matches!(
        PreparedBattle::new(&session.input),
        Err(BattleError::InvalidInput(_))
    ));
    let mut session = shared();
    session.input.team[0].character_form_id = "fixture.unknown".into();
    assert!(matches!(
        PreparedBattle::new(&session.input),
        Err(BattleError::UnsupportedData(_))
    ));
    assert!(
        Session::from_json(&SHARED.replace("\"schema_version\": 1", "\"schema_version\": 2"))
            .is_err()
    );
    assert!(
        Session::from_json(&SHARED.replace(",\n      \"target_id\": \"enemy.1.1\"", "")).is_err()
    );
}

#[test]
fn charged_group_ultimate_preserves_pending_turn_and_requires_null_target() {
    let mut session = shared();
    session.input.team[1].starting_energy_milli = 120_000;
    session.input.starting_skill_points = 5;
    let setup = PreparedBattle::new(&session.input).unwrap();
    let mut battle = Battle::start(&setup, LogMode::Detailed);
    let before = battle.snapshot();
    let bad = command(0, "ally.2", Action::Ultimate, Some("enemy.1.1"));
    assert_eq!(battle.apply(&bad), Err(BattleError::InvalidTarget));
    let ultimate = command(0, "ally.2", Action::Ultimate, None);
    assert!(battle.legal_actions().contains(&ultimate));
    battle.apply(&ultimate).unwrap();
    let after = battle.snapshot();
    assert_eq!(after.ordinary_actor_id, before.ordinary_actor_id);
    assert_eq!(after.av_micros, before.av_micros);
    assert_eq!(after.units[1].normal_turns, 0);
    assert_eq!(after.units[1].gauge_quanta, before.units[1].gauge_quanta);
    assert_eq!(after.units[1].energy_milli, 5_000);
    assert_eq!(after.units[4].toughness, 20); // Ice damage does not match this target's weakness.
    assert_eq!(after.units[4].hp, 871); // floor(400 × 0.9 × 0.5 × 0.8 × 0.9)
    assert_eq!(after.units[5].hp, 821);
    battle
        .apply(&command(1, "ally.3", Action::Basic, Some("enemy.1.1")))
        .unwrap();
    assert_eq!(battle.snapshot().skill_points, 5);
}

#[test]
fn a_second_wave_does_not_restore_resources_or_allow_old_targets() {
    let mut session = shared();
    let mut next_enemy = session.input.waves[0][0].clone();
    next_enemy.unit_id = "enemy.2.1".into();
    session.input.waves.push(vec![next_enemy]);
    let commands = serde_json::from_str::<Vec<Command>>(COMMANDS).unwrap();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let mut battle = Battle::replay(&setup, &commands, LogMode::Detailed).unwrap();
    assert_eq!(battle.outcome(), Outcome::Ongoing);
    assert_eq!(battle.ordinary_actor(), Some("ally.3"));
    let before = battle.snapshot();
    assert_eq!(before.wave_index, 1);
    assert_eq!(before.av_micros, 240_000_000);
    assert_eq!(before.skill_points, 4);
    assert_eq!(before.units[6].hp, 1_000);
    assert_eq!(before.units[0].hp, 900);
    let dead_target = command(8, "ally.3", Action::Basic, Some("enemy.1.1"));
    assert_eq!(battle.apply(&dead_target), Err(BattleError::InvalidTarget));
    assert_eq!(battle.snapshot(), before);
}

#[test]
fn vulnerability_expires_after_target_turn_and_toughness_recovers() {
    let session = shared();
    let commands = serde_json::from_str::<Vec<Command>>(COMMANDS).unwrap();
    let setup = PreparedBattle::new(&session.input).unwrap();
    let mut battle = Battle::replay(&setup, &commands[..5], LogMode::Detailed).unwrap();
    // Skip the ultimate by using the ordinary action. Both enemies are still alive.
    battle
        .apply(&command(5, "ally.2", Action::Basic, Some("enemy.1.2")))
        .unwrap();
    battle
        .apply(&command(6, "ally.1", Action::Basic, Some("enemy.1.2")))
        .unwrap();
    let recovery = battle
        .events()
        .iter()
        .find(|e| {
            e.target_id.as_deref() == Some("enemy.1.1")
                && e.kind
                    == EventKind::Toughness {
                        before: 0,
                        after: 20,
                    }
        })
        .unwrap();
    assert_eq!(recovery.av_micros, 225_000_000);
    assert_eq!(battle.snapshot().units[4].vulnerability_bp, 0);
    assert!(battle.events().iter().any(|e| {
        e.kind == EventKind::StatusExpired(StatusKind::Vulnerability)
            && e.target_id.as_deref() == Some("enemy.1.1")
            && e.av_micros == 225_000_000
    }));
}
