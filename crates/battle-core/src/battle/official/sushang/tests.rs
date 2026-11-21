use super::{BASIC, RIPOSTE, SKILL, TALENT, ULTIMATE, VANQUISHER};
use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, calculate_dpr, fixture::GAUGE_FULL,
};

const SUSHANG: &str = "ally.sushang";

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: SUSHANG.into(),
        form_id: "character.1206".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Hunt,
            element: crate::Element::Physical,
        },
        level: 80,
        energy_capacity: 120,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 600,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            healing_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.05, 0.5, 0.33]),
            (ULTIMATE.into(), vec![1.92, 2.0, 0.5, 0.18]),
            (TALENT.into(), vec![0.15, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|trace| (*trace).into()).collect(),
    }
}

fn dan_heng() -> OfficialCharacter {
    let mut character = character(0, &[]);
    character.unit_id = "ally.dan_heng".into();
    character.form_id = "dan_heng.hunt".into();
    character.combat_type = crate::CombatType {
        path: crate::CombatPath::Hunt,
        element: crate::Element::Wind,
    };
    character.energy_capacity = 100;
    character.stats.base_speed_micros = 190_000_000;
    character.stats.speed_micros = 190_000_000;
    character.skill_parameters = [
        ("skill.100201".into(), vec![1.0]),
        ("skill.100202".into(), vec![1.3, 0.12, 2.0, 1.0]),
        ("skill.100203".into(), vec![2.4, 0.72]),
        ("skill.100204".into(), vec![0.18, 2.0]),
    ]
    .into();
    character
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: &str) -> Command {
    battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == Some(target)
        })
        .expect("expected legal action")
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    battle
        .apply(&command(battle, actor, action, target))
        .unwrap();
}

#[test]
fn basic_and_skill_spend_and_refill_resources_with_two_direct_skill_hits() {
    let plan = PreparedBattle::official_dpr(&[character(0, &[])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[0].energy_milli, 80_000);
    assert_eq!(snapshot.units[2].toughness, 110);
    assert_eq!(battle.dummy_damage[0].amount, 300);

    act(&mut battle, SUSHANG, Action::Skill, "dpr:2");
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 3);
    assert_eq!(snapshot.units[0].energy_milli, 110_000);
    assert_eq!(snapshot.units[2].toughness, 90);
    assert_eq!(snapshot.random_draws, 1);
    // 1000 ATK x 0.315/0.735 x 0.9 x 2/3 against an unbroken target.
    assert_eq!(
        battle.dummy_damage[1..]
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [(DamageKind::Direct, 189), (DamageKind::Direct, 441)]
    );
}

#[test]
fn skill_on_unbroken_target_uses_one_fixed_chance_for_stance() {
    for (seed, expected) in [(3, Some(300)), (42, None)] {
        let plan = PreparedBattle::official_dpr(&[character(0, &[])], seed, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, SUSHANG, Action::Skill, "dpr:2");
        assert_eq!(battle.snapshot().random_draws, 1);
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .find_map(|hit| { (hit.kind == DamageKind::Additional).then_some(hit.amount) }),
            expected,
        );
    }
}

#[test]
fn breaking_with_the_skill_guarantees_stance_and_e1_refunds_on_already_broken_targets() {
    let mut plan = PreparedBattle::official_dpr(&[character(1, &[])], 42, 3).unwrap();
    plan.units[1].stats.base_speed_micros = 1_000_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, SUSHANG, Action::Basic, "dpr:1");

    act(&mut battle, SUSHANG, Action::Skill, "dpr:1");
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.snapshot().units[1].toughness, 0);
    assert_eq!(battle.snapshot().random_draws, 0);
    assert_eq!(battle.dummy_damage[3].kind, DamageKind::WeaknessBreak);
    assert_eq!(battle.dummy_damage[4].kind, DamageKind::Additional);
    assert_eq!(battle.dummy_damage[4].amount, 333);
    assert_eq!(battle.snapshot().units[0].speed_micros, 230_000_000);

    act(&mut battle, SUSHANG, Action::Skill, "dpr:1");
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.snapshot().random_draws, 0);
    assert_eq!(battle.snapshot().units[0].speed_micros, 230_000_000);

    battle.set_skill_points(0, 0);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::Skill)
    );
}

#[test]
fn ultimate_buffs_the_next_two_ordinary_turns_and_extra_stances_use_half_scaling() {
    let mut plan =
        PreparedBattle::official_dpr(&[character(2, &[RIPOSTE]), dan_heng()], 42, 3).unwrap();
    plan.units[3].stats.base_speed_micros = 1_000_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_toughness(0, 3, 0);
    act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
    battle.fill_energy(0);

    act(&mut battle, SUSHANG, Action::Ultimate, "dpr:2");
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    assert_eq!(battle.dummy_damage[1].amount, 1280);

    let skill = |battle: &mut Battle<'_>| {
        if battle.ordinary_actor() == Some("ally.dan_heng") {
            act(battle, "ally.dan_heng", Action::Basic, "dpr:2");
        }
        act(battle, SUSHANG, Action::Skill, "dpr:2");
    };
    let own_hits = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0)
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>()
    };

    skill(&mut battle);
    // (1000 + 600 x 0.18) ATK. Riposte first applies its new 2.5% stack.
    assert_eq!(
        own_hits(&battle)[2..],
        [
            (DamageKind::Direct, 232),
            (DamageKind::Direct, 542),
            (DamageKind::Additional, 378),
            (DamageKind::Additional, 193),
            (DamageKind::Additional, 198),
        ]
    );
    assert_eq!(battle.snapshot().random_draws, 0);

    skill(&mut battle);
    assert_eq!(own_hits(&battle).len(), 12);
    assert_eq!(battle.units[0].sushang_state.sword_stance_stacks, 6);
    assert_eq!(own_hits(&battle)[7].1, 232);

    skill(&mut battle);
    assert_eq!(own_hits(&battle).len(), 15);
    assert_eq!(own_hits(&battle)[12].1, 210);
    assert_eq!(own_hits(&battle)[14].1, 391);
    assert_eq!(battle.units[0].sushang_state.sword_stance_stacks, 7);

    for _ in 0..4 {
        skill(&mut battle);
    }
    assert_eq!(battle.units[0].sushang_state.sword_stance_stacks, 10);
    assert_eq!(own_hits(&battle).last().unwrap().1, 416);
}

#[test]
fn ultimate_inserted_during_a_teammates_turn_advances_sushang_at_the_same_av() {
    let mut plan = PreparedBattle::official_dpr(&[character(0, &[]), dan_heng()], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let first = command(&battle, SUSHANG, Action::Basic, "dpr:2");
    battle.apply(&first).unwrap();
    assert_eq!(battle.ordinary_actor(), Some("ally.dan_heng"));

    let av = battle.snapshot().av_micros;
    let ultimate = command(&battle, SUSHANG, Action::Ultimate, "dpr:2");
    battle.apply(&ultimate).unwrap();
    assert_eq!(battle.ordinary_actor(), Some("ally.dan_heng"));
    assert_eq!(battle.snapshot().units[0].gauge_quanta, 0);
    let pending = command(&battle, "ally.dan_heng", Action::Basic, "dpr:1");
    battle.apply(&pending).unwrap();
    assert_eq!(battle.ordinary_actor(), Some(SUSHANG));
    assert_eq!(battle.snapshot().av_micros, av);

    let replay = Battle::replay(&plan, &[first, ultimate, pending], LogMode::Detailed).unwrap();
    assert_eq!(replay.snapshot(), battle.snapshot());
    assert_eq!(replay.events(), battle.events());
}

#[test]
fn vanquisher_checks_all_enemies_and_e2_mitigation_expires_after_next_turn() {
    let mut plan = PreparedBattle::official_dpr(&[character(2, &[VANQUISHER])], 42, 3).unwrap();
    plan.units[1].stats.base_speed_micros = 1_000_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_toughness(0, 1, 0);

    // The broken left target grants advance after a Basic that hits the middle target.
    act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
    assert!(battle.events().iter().any(|event| {
        matches!(
            event.kind,
            EventKind::GaugeShifted { before_quanta, after_quanta }
            if before_quanta == GAUGE_FULL && after_quanta == GAUGE_FULL * 85 / 100
        )
    }));
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(
        mechanics.incoming_damage_reduction_bp(&battle, 0, 1, DamageKind::Direct),
        0
    );
    act(&mut battle, SUSHANG, Action::Skill, "dpr:1");
    assert_eq!(
        mechanics.incoming_damage_reduction_bp(&battle, 0, 1, DamageKind::Direct),
        2_000
    );
    act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
    assert_eq!(
        mechanics.incoming_damage_reduction_bp(&battle, 0, 1, DamageKind::Direct),
        0
    );
}

#[test]
fn eidolon_four_changes_break_and_later_bleed_without_changing_direct_damage() {
    for (eidolon, expected_damage) in [(3, 6279), (4, 8790)] {
        let plan = PreparedBattle::official_dpr(&[character(eidolon, &[])], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.set_toughness(0, 1, 20);
        act(&mut battle, SUSHANG, Action::Skill, "dpr:1");
        act(&mut battle, SUSHANG, Action::Basic, "dpr:2");

        // At AV 125 the left dummy bleeds before its toughness can recover.
        assert_eq!(battle.dummy_damage[0].amount, 189);
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .filter(|hit| hit.target == 1)
                .filter(|hit| matches!(
                    hit.kind,
                    DamageKind::WeaknessBreak | DamageKind::DamageOverTime
                ))
                .map(|hit| (hit.kind, hit.amount))
                .collect::<Vec<_>>(),
            [
                (DamageKind::WeaknessBreak, expected_damage),
                (DamageKind::DamageOverTime, expected_damage)
            ],
        );
    }
}

#[test]
fn eidolon_six_starts_with_a_stack_and_teammate_break_adds_an_independent_stack() {
    let plan = PreparedBattle::official_dpr(&[character(6, &[]), dan_heng()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].speed_micros, 230_000_000);

    act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
    battle.set_toughness(1, 2, 10);
    act(&mut battle, "ally.dan_heng", Action::Basic, "dpr:1");
    assert_eq!(battle.snapshot().units[0].speed_micros, 260_000_000);

    for speed in [230_000_000, 200_000_000] {
        if battle.ordinary_actor() == Some("ally.dan_heng") {
            act(&mut battle, "ally.dan_heng", Action::Basic, "dpr:2");
        }
        act(&mut battle, SUSHANG, Action::Basic, "dpr:2");
        assert_eq!(battle.snapshot().units[0].speed_micros, speed);
    }
}

#[test]
fn fixed_dpr_produces_repeatable_damage_and_preserves_source_credit() {
    let plan = PreparedBattle::official_dpr(&[character(6, &[RIPOSTE, VANQUISHER])], 3, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let first = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let second = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(first, second);
    assert!(first.targets[1].hits.iter().any(|hit| {
        hit.source_id == SUSHANG
            && hit.kind == DamageKind::Additional
            && hit.action == Action::Skill
    }));
    assert_eq!(first.total_damage, first.characters[0].own_damage);
}
