use std::sync::Arc;

use super::{BASIC, FASTER_THAN_LIGHT, HIGH_GALE, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{self, Effect, Target},
};

fn character(eidolon: u8, crit_rate_bp: u32) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.dan_heng".into(),
        form_id: "dan_heng.hunt".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Hunt,
            element: crate::Element::Wind,
        },
        level: 80,
        energy_capacity: 100,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 600,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            healing_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![1.3, 0.12, 2.0, 1.0]),
            (ULTIMATE.into(), vec![2.4, 0.72]),
            (TALENT.into(), vec![0.18, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
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
        .expect("expected a legal action for the specified actor and target")
}

fn use_action(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    battle
        .apply(&command(battle, actor, action, target))
        .unwrap();
}

#[test]
fn skill_has_four_independent_wind_hits_and_only_critical_hits_slow() {
    for (crit_rate_bp, expected, expected_speed) in [
        (0, [234, 117, 117, 312], 100_000_000),
        (10_000, [351, 175, 175, 468], 88_000_000),
    ] {
        let plan = PreparedBattle::official_dpr(&[character(0, crit_rate_bp)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        use_action(&mut battle, "ally.dan_heng", Action::Skill, "dpr:2");
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected,
        );
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.skill_points, 2);
        assert_eq!(snapshot.units[0].energy_milli, 80_000);
        assert_eq!(snapshot.units[2].toughness, 100);
        assert_eq!(snapshot.units[2].speed_micros, expected_speed);
        assert_eq!(snapshot.units[1].speed_micros, 100_000_000);
    }
}

#[test]
fn eidolon_one_adds_crit_rate_and_eidolon_six_increases_slow() {
    let mut checks = Vec::new();
    for eidolon in [0, 1, 6] {
        // Seed 42 starts with a 0.7415 roll, between the base 70% and E1's 82%.
        let plan = PreparedBattle::official_dpr(&[character(eidolon, 7_000)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        use_action(&mut battle, "ally.dan_heng", Action::Skill, "dpr:2");
        checks.push((
            battle.dummy_damage[0].critical,
            battle.snapshot().units[2].speed_micros,
        ));
    }
    assert_eq!(
        checks,
        [(false, 88_000_000), (true, 88_000_000), (true, 80_000_000)],
    );
}

#[test]
fn slow_expires_after_the_targets_second_turn() {
    let plan = PreparedBattle::official_dpr(&[character(0, 10_000)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    use_action(&mut battle, "ally.dan_heng", Action::Skill, "dpr:2");

    // Dan Heng acts at 50, 100, 150, 200, and 250 AV. The middle dummy takes
    // its slowed turns just after 106 and 220 AV.
    for (turns, speed) in [(1, 88_000_000), (1, 88_000_000), (2, 100_000_000)] {
        use_action(&mut battle, "ally.dan_heng", Action::Basic, "dpr:1");
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.units[2].normal_turns, turns);
        assert_eq!(snapshot.units[2].speed_micros, speed);
    }
}

#[test]
fn slowed_ultimate_changes_the_skill_multiplier_and_high_gale_changes_basic_damage() {
    let mut dan_heng = character(0, 10_000);
    dan_heng.enabled_trace_ids.push(HIGH_GALE.into());
    let plan = PreparedBattle::official_dpr(&[dan_heng], 42, 3).unwrap();
    let opening = [
        Command {
            decision_index: 0,
            actor_id: "ally.dan_heng".into(),
            action: Action::Skill,
            target_id: Some("dpr:2".into()),
        },
        Command {
            decision_index: 1,
            actor_id: "ally.dan_heng".into(),
            action: Action::Basic,
            target_id: Some("dpr:2".into()),
        },
    ];
    let slowed = Battle::replay(&plan, &opening, LogMode::Detailed).unwrap();
    assert_eq!(slowed.snapshot().units[0].energy_milli, 100_000);
    // 1000 ATK x 45/55% x 1.40 DMG x 1.5 Crit x 2/3 DEF x 0.90 toughness.
    assert_eq!(slowed.dummy_damage[4].amount, 567);
    assert_eq!(slowed.dummy_damage[5].amount, 693);

    for (target, expected_damage) in [("dpr:2", 2808), ("dpr:1", 2160)] {
        let mut branch = slowed.clone();
        let ultimate = command(&branch, "ally.dan_heng", Action::Ultimate, target);
        branch.apply(&ultimate).unwrap();
        // 1000 ATK x (2.40 + 0.72 only while slowed) x 1.5 x 2/3 x 0.90.
        assert_eq!(branch.dummy_damage[6].amount, expected_damage);
        assert_eq!(branch.dummy_damage[6].kind, DamageKind::Direct);
        assert_eq!(branch.snapshot().units[0].energy_milli, 5_000);
        let replay = Battle::replay(
            &plan,
            &[opening[0].clone(), opening[1].clone(), ultimate],
            LogMode::Detailed,
        )
        .unwrap();
        assert_eq!(replay.snapshot(), branch.snapshot());
        assert_eq!(replay.events(), branch.events());
    }
}

#[test]
fn speed_trace_consumes_one_fixed_roll_per_attack() {
    for seed in [1, 3] {
        let mut dan_heng = character(0, 0);
        dan_heng.enabled_trace_ids.push(FASTER_THAN_LIGHT.into());
        let plan = PreparedBattle::official_dpr(&[dan_heng], seed, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        use_action(&mut battle, "ally.dan_heng", Action::Basic, "dpr:2");
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.random_draws, 1);
        // Initial rolls are 0.5665 and 0.1134. A 20% speed buff from 200 is 240.
        assert_eq!(
            snapshot.units[0].speed_micros,
            if seed == 3 { 240_000_000 } else { 200_000_000 },
        );
    }
}

fn support_plan(eidolon: u8) -> PreparedBattle {
    let support = OfficialCharacter {
        unit_id: "ally.support".into(),
        form_id: "character.1013".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Erudition,
            element: crate::Element::Ice,
        },
        energy_capacity: 110,
        stats: OfficialStats {
            speed_micros: 250_000_000,
            ..character(0, 0).stats
        },
        skill_parameters: [
            ("skill.101301".into(), vec![0.5]),
            ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
            ("skill.101303".into(), vec![1.2]),
            ("skill.101304".into(), vec![0.5, 0.25]),
        ]
        .into(),
        ..character(0, 0)
    };
    // The first six rolls with seed 2 are above E1's 12% critical rate.
    let mut plan = PreparedBattle::official_dpr(&[character(eidolon, 0), support], 2, 5).unwrap();
    let Kit::Official(kit) = &mut plan.units[1].kit else {
        panic!("expected a support kit")
    };
    let kit = Arc::get_mut(kit).expect("the plan owns its kit");
    kit.skill.target = Target::Ally;
    kit.skill.effects = vec![Effect::Heal(1)];
    plan
}

#[test]
fn ally_targeting_applies_talent_to_all_hits_then_starts_the_eidolon_cooldown() {
    for (eidolon, expected_damage) in [
        (0, [318, 389, 270, 330, 270, 330]),
        (2, [318, 389, 270, 330, 318, 389]),
    ] {
        let plan = support_plan(eidolon);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        for _ in 0..3 {
            use_action(&mut battle, "ally.support", Action::Skill, "ally.dan_heng");
            use_action(&mut battle, "ally.dan_heng", Action::Basic, "dpr:2");
        }
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected_damage,
        );
    }
}

#[test]
fn eidolon_four_advances_after_ultimate_kill_on_another_ordinary_turn() {
    for eidolon in [3, 4] {
        let mut plan = support_plan(eidolon);
        let target = &mut plan.units[2];
        target.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        target.stats.hp = 1_000;
        target.stats.base_speed_micros = 1_000_000;
        plan.units[0].starting_energy_milli = 100_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.ordinary_actor(), Some("ally.support"));
        use_action(&mut battle, "ally.dan_heng", Action::Ultimate, "dpr:1");
        assert!(
            battle
                .events()
                .iter()
                .any(|event| event.kind == EventKind::UnitDefeated)
        );
        assert_eq!(battle.ordinary_actor(), Some("ally.support"));
        let av = battle.snapshot().av_micros;
        use_action(&mut battle, "ally.support", Action::Basic, "dpr:2");
        assert_eq!(battle.ordinary_actor(), Some("ally.dan_heng"));
        assert_eq!(
            battle.snapshot().av_micros,
            if eidolon == 4 { av } else { 50_000_000 }
        );
    }
}

#[test]
fn official_wind_kit_can_run_repeatable_fixed_dpr() {
    let plan = PreparedBattle::official_dpr(&[character(0, 7_000)], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(run(), result);
    assert!(result.targets[0].hits.is_empty());
    assert!(result.targets[2].hits.is_empty());
    assert!(
        result.targets[1]
            .hits
            .iter()
            .any(|hit| hit.action == Action::Ultimate)
    );
}
