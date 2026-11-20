use crate::{
    Action, Command, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Battle, Kit, Status},
    calculate_dpr, fixture,
};

use super::{BASIC, OfficialCharacter, SKILL, TALENT, ULTIMATE};

fn character(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.natasha".into(),
        form_id: "natasha.abundance".into(),
        energy_capacity: 90,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 125_000_000,
            speed_micros: 125_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.50]),
            (SKILL.into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
            (ULTIMATE.into(), vec![0.092, 92.0]),
            (TALENT.into(), vec![0.30, 0.25]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn companion(speed_micros: u64) -> OfficialCharacter {
    let mut arlan = character(0);
    arlan.unit_id = "ally.arlan".into();
    arlan.form_id = "character.1008".into();
    arlan.energy_capacity = 110;
    arlan.stats.base_speed_micros = speed_micros;
    arlan.stats.speed_micros = speed_micros;
    arlan.skill_parameters = [
        ("skill.100801".into(), vec![0.50]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    arlan
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .unwrap();
    battle.apply(&command).unwrap();
}

fn ally_heals(battle: &Battle<'_>, target: &str) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Heal { amount } if event.target_id.as_deref() == Some(target) => {
                Some(amount)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn skill_cleanses_and_heals_at_the_target_turn_start_with_duration_trace() {
    let mut natasha = character(0);
    natasha.stats.healing_bp = 2_000;
    natasha.enabled_trace_ids = ["trace.1105101", "trace.1105102", "trace.1105103"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let plan = PreparedBattle::official_dpr(&[natasha, companion(100_000_000)], 42, 1).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 600;
    battle.units[1].statuses[StatusKind::Vulnerability.index()] = Some(Status {
        source: 2,
        magnitude: 1_000,
        expires_after_turn: 10,
    });

    act(&mut battle, Action::Skill, Some("ally.arlan"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 0);
    assert_eq!(snapshot.units[0].energy_milli, 75_000);
    assert_eq!(snapshot.units[1].vulnerability_bp, 0);
    assert_eq!(snapshot.ordinary_actor_id.as_deref(), Some("ally.arlan"));
    // At 30% HP: (2000×7%+70)×(1+20%+10%+25%) = 325.5.
    // Next turn starts above 30% HP: (2000×4.8%+48)×(1+20%+10%) = 187.2.
    assert_eq!(ally_heals(&battle, "ally.arlan"), [325, 187]);
    while battle.snapshot().units[1].normal_turns < 4 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(ally_heals(&battle, "ally.arlan"), [325, 187, 187, 187]);
}

#[test]
fn ultimate_checks_e2_threshold_before_healing_and_its_hot_coexists_with_skill() {
    let mut natasha = character(2);
    natasha.stats.base_speed_micros = 200_000_000;
    natasha.stats.speed_micros = 200_000_000;
    let plan = PreparedBattle::official_dpr(&[natasha, companion(50_000_000)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 601;
    battle.units[1].hp = 600;
    act(&mut battle, Action::Skill, Some("ally.arlan"));
    battle.units[1].hp = 600;
    battle.units[0].energy_milli = 90_000;
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    // Immediate base 276. Only the ally at exactly 30% gets +25% and the E2 HoT.
    assert_eq!(ally_heals(&battle, "ally.natasha"), [276]);
    assert_eq!(ally_heals(&battle, "ally.arlan"), [262, 345]);
    while battle.snapshot().units[1].normal_turns < 3 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(ally_heals(&battle, "ally.natasha"), [276]);
    assert_eq!(ally_heals(&battle, "ally.arlan"), [262, 345, 144, 280, 144]);
}

#[test]
fn repeating_skill_refreshes_its_healing_instead_of_stacking() {
    let mut natasha = character(0);
    natasha.stats.base_speed_micros = 200_000_000;
    natasha.stats.speed_micros = 200_000_000;
    let plan = PreparedBattle::official_dpr(&[natasha, companion(50_000_000)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 900;
    act(&mut battle, Action::Skill, Some("ally.arlan"));
    act(&mut battle, Action::Skill, Some("ally.arlan"));
    while battle.snapshot().units[1].normal_turns < 3 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(ally_heals(&battle, "ally.arlan"), [210, 210, 144, 144]);
}

#[test]
fn first_low_hp_enemy_attack_triggers_e1_once_and_e4_per_hit() {
    let mut plan = PreparedBattle::official_dpr(&[character(4)], 42, 3).unwrap();
    let enemy = &mut plan.units[1];
    enemy.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
    enemy.stats.attack = 100;
    enemy.stats.hp = 10_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 640;

    act(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(ally_heals(&battle, "ally.natasha"), [875]);
    // 45 starting + 20 basic + 10 enemy hit + 5 E4.
    assert_eq!(battle.snapshot().units[0].energy_milli, 80_000);

    battle.units[0].hp = 640;
    battle.units[0].energy_milli = 0;
    act(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(ally_heals(&battle, "ally.natasha"), [875]);
    assert_eq!(battle.snapshot().units[0].energy_milli, 35_000);
}

#[test]
fn e6_basic_uses_max_hp_for_the_additional_critical_hit_without_extra_toughness() {
    let mut natasha = character(6);
    natasha.stats.crit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[natasha], 42, 0).unwrap();
    let script = [Command {
        decision_index: 0,
        actor_id: "ally.natasha".into(),
        target_id: Some("dpr:2".into()),
        action: Action::Basic,
    }];
    let battle = Battle::replay(&plan, &script, LogMode::Detailed).unwrap();
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[2].toughness, 110);
    assert_eq!(snapshot.skill_points, 1);
    assert_eq!(snapshot.units[0].energy_milli, 65_000);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.critical, hit.amount))
        .collect();
    // ATK×50% and MAXHP×40%, both ×1.5 critical ×1000/1500 defense ×0.9 toughness.
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, true, 450),
            (DamageKind::Additional, true, 720)
        ]
    );

    let scenario = STANDARD_DPR_SCENARIO;
    let first = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let repeated = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(first, repeated);
    assert!(
        first.targets[1]
            .hits
            .iter()
            .any(|hit| hit.kind == DamageKind::Additional)
    );
}
