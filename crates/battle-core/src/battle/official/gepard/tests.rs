use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats, Outcome,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind, battle::Kit, calculate_dpr, fixture,
};

use super::{BASIC, COMMANDER, FIGHTING_SPIRIT, FORM, SKILL, TALENT, ULTIMATE};

fn gepard(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.gepard".into(),
        form_id: FORM.into(),
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 501,
            base_defense: 501,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.0, 0.65, 1.0, 0.3]),
            (ULTIMATE.into(), vec![0.3, 3.0, 150.0]),
            (TALENT.into(), vec![0.25]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn herta() -> OfficialCharacter {
    let mut ally = gepard(0);
    ally.unit_id = "ally.herta".into();
    ally.form_id = "character.1013".into();
    ally.stats.base_speed_micros = 80_000_000;
    ally.stats.speed_micros = 80_000_000;
    ally.energy_capacity = 110;
    ally.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    ally
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .expect("expected a legal action at this decision");
    battle.apply(&command).unwrap();
}

#[test]
fn basic_generates_a_point_and_fighting_spirit_adds_flat_attack() {
    for (trace, expected) in [(false, 300), (true, 352)] {
        let mut character = gepard(0);
        if trace {
            character.enabled_trace_ids.push(FIGHTING_SPIRIT.into());
        }
        let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Basic, Some("dpr:2"));
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.skill_points, 4);
        assert_eq!(snapshot.units[0].energy_milli, 70_000);
        assert_eq!(snapshot.units[2].toughness, 110);
        // ATK is 1000 or 1175 after adding floor(501×35%).
        assert_eq!(battle.dummy_damage[0].amount, expected);
    }
}

#[test]
fn skill_freeze_uses_its_base_chance_and_e2_slows_only_on_thaw() {
    let unupgraded = PreparedBattle::official_dpr(&[gepard(0)], 42, 3).unwrap();
    let mut battle = Battle::start(&unupgraded, LogMode::Detailed);
    // The first seeded draw is above 65%, and this fixture has zero crit rate.
    act(&mut battle, Action::Skill, Some("dpr:2"));
    assert!(!battle.is_frozen(2));
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.snapshot().units[0].energy_milli, 80_000);
    assert_eq!(battle.snapshot().units[2].toughness, 100);
    assert_eq!(battle.dummy_damage[0].amount, 600);

    let mut character = gepard(2);
    character.stats.crit_rate_bp = 10_000;
    let e2 = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
    let mut battle = Battle::start(&e2, LogMode::Detailed);
    act(&mut battle, Action::Skill, Some("dpr:2"));
    assert!(battle.is_frozen(2));
    assert_eq!(battle.snapshot().units[2].speed_micros, 100_000_000);
    assert_eq!(battle.dummy_damage[0].amount, 900);
    act(&mut battle, Action::Basic, Some("dpr:1"));
    assert!(!battle.is_frozen(2));
    assert_eq!(battle.snapshot().units[2].speed_micros, 80_000_000);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.target, 2);
    assert_eq!(hit.action, Action::Skill);
    assert_eq!(hit.kind, DamageKind::Additional);
    // 1000 ATK × 30% × 150% crit × 1000/(1000+500) DEF × 90% unbroken.
    assert_eq!(hit.amount, 270);
    assert!(hit.critical);
    assert_eq!(battle.snapshot().units[2].toughness, 100);

    while battle.snapshot().units[2].normal_turns < 2 {
        act(&mut battle, Action::Basic, Some("dpr:1"));
    }
    assert_eq!(battle.snapshot().units[2].speed_micros, 100_000_000);
}

#[test]
fn ice_break_freeze_does_not_grant_eidolon_two_slow() {
    let plan = PreparedBattle::official_dpr(&[gepard(2)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].toughness = 10;
    act(&mut battle, Action::Basic, Some("dpr:1"));
    assert!(battle.is_frozen(1));
    while battle.is_frozen(1) {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(battle.snapshot().units[1].speed_micros, 100_000_000);
    assert!(!battle.events().iter().any(|event| {
        event.target_id.as_deref() == Some("dpr:1")
            && event.kind == EventKind::StatusApplied(StatusKind::Slow)
    }));
}

#[test]
fn skill_freeze_skips_an_attacking_enemys_turn() {
    for (eidolon, attacks) in [(0, true), (1, false)] {
        let mut plan = PreparedBattle::official_dpr(&[gepard(eidolon)], 42, 3).unwrap();
        plan.units[2].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Skill, Some("dpr:2"));
        act(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(
            battle.events().iter().any(|event| {
                event.source_id.as_deref() == Some("dpr:2") && event.kind == EventKind::EnemyAttack
            }),
            attacks,
        );
    }
}

#[test]
fn ultimate_applies_defense_scaled_shield_to_allies_for_their_own_turns() {
    let mut character = gepard(0);
    character
        .skill_parameters
        .insert(ULTIMATE.into(), vec![0.45, 3.0, 600.0]);
    let plan = PreparedBattle::official_dpr(&[character, herta()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[0].energy_milli, 5_000);
    assert_eq!(snapshot.skill_points, 3);
    assert_eq!(snapshot.units[0].shield, 825);
    assert_eq!(snapshot.units[1].shield, 825);
    assert_eq!(snapshot.units[2].shield, 0);
    assert!(battle.dummy_damage.is_empty());

    while battle.snapshot().units[0].shield > 0 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(battle.snapshot().units[0].normal_turns, 5);
    assert_eq!(battle.snapshot().units[1].shield, 825);
    while battle.snapshot().units[1].shield > 0 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert!(battle.events().iter().any(|event| {
        event.target_id.as_deref() == Some("ally.herta")
            && event.kind == EventKind::StatusExpired(StatusKind::Shield)
    }));
}

#[test]
fn defense_scaled_shield_absorbs_up_to_its_strength() {
    let mut plan = PreparedBattle::official_dpr(&[gepard(0)], 42, 3).unwrap();
    let enemy = &mut plan.units[1];
    enemy.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
    enemy.stats.attack = 1_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    act(&mut battle, Action::Basic, Some("dpr:2"));
    act(&mut battle, Action::Basic, Some("dpr:2"));

    // Defense 501, enemy level 32, and a 300 shield yield 509 total and 209 HP damage.
    assert_eq!(battle.snapshot().units[0].hp, 1_791);
    assert_eq!(battle.snapshot().units[0].shield, 0);
    assert!(battle.events().iter().any(|event| {
        event.target_id.as_deref() == Some("ally.gepard")
            && event.kind
                == EventKind::Damage {
                    kind: DamageKind::Direct,
                    amount: 509,
                    hp_loss: 209,
                    absorbed: 300,
                    critical: false,
                }
    }));
}

#[test]
fn talent_survives_one_lethal_enemy_attack_and_e6_advances_before_another_turn() {
    let enemy_kit = fixture::enemy("fixture.target_left", 32).unwrap();
    for (eidolon, trace, restored, av, energy) in [
        (0, false, 500, 150_000_000, 50_000),
        (6, true, 1_500, 100_000_000, 100_000),
    ] {
        let mut character = gepard(eidolon);
        character.stats.energy_regeneration_bp = 0;
        if trace {
            character.enabled_trace_ids.push(COMMANDER.into());
        }
        let mut plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
        let enemy = &mut plan.units[1];
        enemy.kit = Kit::Enemy(enemy_kit);
        enemy.stats.attack = 5_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Basic, Some("dpr:2"));
        act(&mut battle, Action::Basic, Some("dpr:2"));
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.av_micros, av);
        assert_eq!(snapshot.ordinary_actor_id.as_deref(), Some("ally.gepard"));
        assert_eq!(snapshot.units[0].hp, restored);
        assert_eq!(snapshot.units[0].energy_milli, energy);
        assert!(
            !battle
                .events()
                .iter()
                .any(|event| event.kind == EventKind::UnitDefeated)
        );
        assert!(battle.events().iter().any(|event| {
            event.target_id.as_deref() == Some("ally.gepard")
                && event.kind == EventKind::Heal { amount: restored }
        }));

        battle.resolve_enemy(1, enemy_kit);
        assert_eq!(battle.snapshot().units[0].hp, 0);
        assert!(
            battle
                .events()
                .iter()
                .any(|event| event.kind == EventKind::UnitDefeated)
        );
    }
}

#[test]
fn living_e4_gepard_increases_other_allies_effect_resistance() {
    for (eidolon, alive, expected) in [(3, true, true), (4, true, false), (4, false, true)] {
        let mut companion = herta();
        companion.stats.effect_resistance_bp = 500;
        let plan = PreparedBattle::official_dpr(&[gepard(eidolon), companion], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if !alive {
            battle.units[0].hp = 0;
        }
        // Base chance with the target's 5% RES is 85.5%, vs 67.5% with E4.
        assert_eq!(battle.roll_effect(2, 1, 9_000), expected);
    }
}

#[test]
fn fixed_dpr_repeats_and_attributes_freeze_damage_to_the_skill_source() {
    let plan = PreparedBattle::official_dpr(&[gepard(2)], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let repeated = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(result, repeated);
    assert!(result.total_damage > 0);
    assert_eq!(result.characters[0].own_damage, result.total_damage);
    assert!(result.targets[1].hits.iter().any(|hit| {
        hit.source_id == "ally.gepard"
            && hit.action == Action::Skill
            && hit.kind == DamageKind::Additional
            && !hit.critical
    }));
    assert_eq!(
        Battle::start(&plan, LogMode::Off).outcome(),
        Outcome::Ongoing
    );
}
