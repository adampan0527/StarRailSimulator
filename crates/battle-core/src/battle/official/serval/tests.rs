use std::{collections::BTreeMap, num::NonZeroU32};

use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, calculate_dpr,
};

use super::{BASIC, SKILL, TALENT, ULTIMATE};

fn serval(eidolon: u8, enabled_traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "team.serval".into(),
        form_id: "character.1103".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Erudition,
            element: crate::Element::Thunder,
        },
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 125_000_000,
            speed_micros: 125_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: BTreeMap::from([
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.7, 0.3, 0.8, 2.0, 0.4]),
            (ULTIMATE.into(), vec![1.08, 2.0]),
            (TALENT.into(), vec![0.36]),
        ]),
        eidolon,
        enabled_trace_ids: enabled_traces.iter().map(|trace| (*trace).into()).collect(),
    }
}

fn prepared(character: OfficialCharacter, toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(&[character], 0, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn apply(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .unwrap();
    battle.apply(&command).unwrap();
}

#[test]
fn skill_uses_blast_targets_and_resolves_skill_shock_for_two_enemy_turns() {
    let plan = prepared(serval(0, &["trace.1103101"]), &[100; 5]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    apply(&mut battle, Action::Skill, Some("dpr:3"));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 2);
    assert_eq!(snapshot.units[0].energy_milli, 80_000);
    assert_eq!(
        snapshot.units[1..]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [100, 90, 80, 90, 100],
    );
    // The defense multiplier is 2/3 and intact toughness contributes 0.9.
    // Primary/adjacent skill ratios are 0.7/0.3, and Shock is 0.4.
    for (target, expected) in [(2, 180), (3, 420), (4, 180)] {
        assert!(battle.dummy_damage.iter().any(|hit| {
            hit.target == target && hit.kind == DamageKind::Direct && hit.amount == expected
        }));
    }
    let dots = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::DamageOverTime)
            .map(|hit| {
                (
                    hit.target,
                    hit.av_micros,
                    hit.amount,
                    hit.critical,
                    hit.action,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        dots(&battle),
        [2, 3, 4].map(|target| (target, 100_000_000, 240, false, Action::Skill)),
    );

    apply(&mut battle, Action::Basic, Some("dpr:1"));
    apply(&mut battle, Action::Basic, Some("dpr:1"));

    let expected_dots: Vec<_> = [100_000_000, 200_000_000]
        .into_iter()
        .flat_map(|time| [2, 3, 4].map(|target| (target, time, 240, false, Action::Skill)))
        .collect();
    assert_eq!(dots(&battle), expected_dots);
    let additional_targets: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional && hit.action == Action::Basic)
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(additional_targets, [(2, 216), (3, 216), (4, 216)]);
    assert!((1..=5).all(|target| !battle.has_shock(target)));
}

#[test]
fn skill_rolls_for_each_enemy_and_its_trace_changes_the_probability() {
    // With no crit rolls, seed 0 yields 88.33% for the first of the three
    // targets and under 80% for the other two.
    let plan = prepared(serval(0, &[]), &[100; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill, Some("dpr:2"));
    let shock_count = |battle: &Battle<'_>| {
        battle
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::ShockApplied { .. }))
            .count()
    };
    assert_eq!(shock_count(&battle), 2);

    let plan = prepared(serval(0, &["trace.1103101"]), &[100; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill, Some("dpr:2"));
    assert_eq!(shock_count(&battle), 3);
}

#[test]
fn basic_eidolon_hits_one_adjacent_enemy_and_talent_refunds_per_shocked_enemy() {
    let plan = prepared(serval(2, &["trace.1103102"]), &[100; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].energy_milli, 65_000);
    for enemy in 1..=3 {
        battle.apply_skill_shock(0, enemy, Action::Skill, 10_000, 4_000, 2);
    }

    apply(&mut battle, Action::Basic, Some("dpr:2"));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[0].energy_milli, 97_000);
    assert_eq!(snapshot.units[2].toughness, 90);
    assert_eq!(
        snapshot.units[1].toughness + snapshot.units[3].toughness,
        190
    );
    let additional: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .collect();
    assert_eq!(additional.iter().filter(|hit| hit.amount == 180).count(), 1);
    assert_eq!(additional.iter().filter(|hit| hit.amount == 216).count(), 3);
}

#[test]
fn ultimate_extends_break_and_existing_skill_shocks_and_applies_fourth_eidolon() {
    let mut plan = prepared(serval(4, &[]), &[10, 100, 100]);
    plan.units[0].starting_energy_milli = 100_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_skill_shock(0, 1, Action::Skill, 10_000, 4_000, 2);

    apply(&mut battle, Action::Ultimate, None);

    assert_eq!(battle.snapshot().units[0].energy_milli, 17_000);
    assert_eq!(battle.units[1].shocks.len(), 2);
    assert_eq!(battle.units[2].shocks.len(), 1);
    assert_eq!(battle.units[3].shocks.len(), 1);
    let extensions: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::ShockExtended { extra_turns: 2 } => event.target_id.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(extensions, ["dpr:1", "dpr:2", "dpr:3"]);

    apply(&mut battle, Action::Basic, Some("dpr:2"));
    let dots: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::DamageOverTime)
        .map(|hit| (hit.target, hit.action, hit.amount))
        .collect();
    assert_eq!(
        dots,
        [
            (2, Action::Ultimate, 240),
            (3, Action::Ultimate, 240),
            (1, Action::Skill, 266),
            (1, Action::Ultimate, 5_023),
        ],
    );
    assert!((1..=3).all(|target| battle.has_shock(target)));
}

#[test]
fn fixed_window_dpr_repeats_and_keeps_the_shock_source_action() {
    let plan = prepared(serval(6, &["trace.1103101"]), &[30, 120, 30]);
    let simulate = || calculate_dpr(&plan, &[30, 120, 30], NonZeroU32::new(1).unwrap()).unwrap();
    let result = simulate();
    assert_eq!(result, simulate());
    assert_eq!(result.characters[0].own_damage, result.total_damage);
    let hits = &result.targets[1].hits;
    // 0.4 Shock × 1.3 E6 damage multiplier × 2/3 defense × 0.9 toughness.
    assert!(hits.iter().any(|hit| {
        hit.kind == DamageKind::DamageOverTime
            && hit.action == Action::Skill
            && hit.av_micros == 100_000_000
            && hit.source_id == "team.serval"
            && hit.amount == 312
    }));
}
