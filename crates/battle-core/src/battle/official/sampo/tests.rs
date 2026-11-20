use std::{collections::BTreeMap, num::NonZeroU32};

use crate::{
    Action, Battle, DamageKind, LogMode, OfficialCharacter, OfficialStats, PreparedBattle,
    battle::Kit, calculate_dpr, fixture,
};

use super::super::super::periodic::{DotScale, SkillDot};
use super::{BASIC, DEFENSIVE_POSITION, SKILL, SPICE_UP, TALENT, TRAP, ULTIMATE};
use crate::fixture::Element;

fn sampo(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "team.sampo".into(),
        form_id: "character.1108".into(),
        energy_capacity: 120,
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
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 6_000,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: BTreeMap::from([
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![4.0, 0.28]),
            (ULTIMATE.into(), vec![0.96, 0.2, 2.0, 1.0]),
            (TALENT.into(), vec![0.65, 0.2, 3.0, 5.0]),
        ]),
        eidolon,
        enabled_trace_ids: traces.iter().map(|trace| (*trace).into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut character = sampo(0, &[]);
    character.unit_id = "team.arlan".into();
    character.form_id = "character.1008".into();
    character.energy_capacity = 110;
    character.stats.base_speed_micros = 110_000_000;
    character.stats.speed_micros = 110_000_000;
    character.skill_parameters = BTreeMap::from([
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]);
    character
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 0, 3)
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

fn apply_wind_shear(battle: &mut Battle<'_>, source: usize, target: usize, turns: u32) {
    assert!(battle.apply_skill_dot(
        source,
        target,
        10_000,
        SkillDot {
            element: Element::Wind,
            damage: DotScale::Attack(2_000),
            origin: Action::Skill,
            turns,
            max_stacks: 5,
        },
    ));
}

#[test]
fn basic_hits_three_times_and_each_hit_adds_a_stack() {
    let plan = prepared(&[sampo(0, &[])], &[1_000]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Basic, Some("dpr:1"));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[0].energy_milli, 80_000);
    assert_eq!(snapshot.units[1].toughness, 990);
    assert_eq!(battle.dot_stacks(1, Element::Wind), 3);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    // 1000 ATK × 0.5 × [0.3, 0.3, 0.4] × (2/3) × 0.9.
    // The next target turn ticks three wind shear stacks of 20% ATK.
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, 90),
            (DamageKind::Direct, 90),
            (DamageKind::Direct, 120),
            (DamageKind::DamageOverTime, 360),
        ]
    );
}

#[test]
fn skill_targets_first_hit_and_e1_adds_one_bounce_with_its_own_energy_and_toughness() {
    for (eidolon, count, energy, reduction) in [(0, 5, 90_000, 30), (1, 6, 96_000, 35)] {
        let plan = prepared(&[sampo(eidolon, &[])], &[1_000; 3]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply(&mut battle, Action::Skill, Some("dpr:2"));
        let direct: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .collect();
        assert_eq!(direct.len(), count);
        assert_eq!(direct[0].target, 2);
        assert!(direct.iter().any(|hit| hit.target != 2));
        assert!(direct.iter().all(|hit| hit.amount == 168));
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.units[0].energy_milli, energy);
        assert_eq!(snapshot.skill_points, 2);
        assert_eq!(
            snapshot.units[1..]
                .iter()
                .map(|unit| 1_000 - unit.toughness)
                .sum::<u16>(),
            reduction
        );
    }
}

#[test]
fn e4_detonates_full_wind_shear_per_skill_hit_and_e6_adds_fifteen_multiplier_points() {
    for (eidolon, detonation, normal_tick) in [(4, 48, 600), (6, 84, 1_050)] {
        let plan = prepared(&[sampo(eidolon, &[])], &[1_000]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply(&mut battle, Action::Skill, Some("dpr:1"));

        // Successful first through fifth talent rolls fill the stacks. The
        // sixth hit (E1) detonates without consuming stacks or a turn.
        assert_eq!(battle.dot_stacks(1, Element::Wind), 5);
        let dots: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::DamageOverTime)
            .map(|hit| (hit.av_micros, hit.amount, hit.critical))
            .collect();
        assert_eq!(
            dots,
            [
                (80_000_000, detonation, false),
                (100_000_000, normal_tick, false)
            ]
        );
    }
}

#[test]
fn accepted_talent_stack_refreshes_the_shared_duration_and_trap_extends_it() {
    for (traces, expected_ticks) in [(&[][..], 3), (&[TRAP][..], 4)] {
        let plan = prepared(&[sampo(0, traces)], &[1_000, 1_000]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        for _ in 0..5 {
            apply_wind_shear(&mut battle, 0, 1, 1);
        }
        apply(&mut battle, Action::Basic, Some("dpr:1"));
        for _ in 0..5 {
            apply(&mut battle, Action::Basic, Some("dpr:2"));
        }
        let target_ticks: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 1 && hit.kind == DamageKind::DamageOverTime)
            .collect();
        assert_eq!(target_ticks.len(), expected_ticks);
        assert!(target_ticks.iter().all(|hit| hit.amount == 600));
        assert_eq!(battle.dot_stacks(1, Element::Wind), 0);
    }
}

#[test]
fn ultimate_applies_dot_only_vulnerability_and_credits_sampo_for_ally_damage() {
    let plan = prepared(&[sampo(0, &[DEFENSIVE_POSITION]), arlan()], &[1_000]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 120_000);
    battle.apply_skill_shock(1, 2, Action::Skill, 10_000, 4_000, 4);
    apply(&mut battle, Action::Ultimate, None);

    assert_eq!(battle.snapshot().units[0].energy_milli, 15_000);
    assert_eq!(battle.snapshot().units[2].toughness, 980);
    assert_eq!(battle.dot_stacks(2, Element::Wind), 4);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [144; 4]
    );
    for _ in 0..8 {
        apply(&mut battle, Action::Basic, Some("dpr:1"));
    }
    let arlan_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1)
        .collect();
    let dot_hits: Vec<_> = arlan_hits
        .iter()
        .filter(|hit| hit.kind == DamageKind::DamageOverTime)
        .map(|hit| (hit.amount, hit.assists.as_slice()))
        .collect();
    assert_eq!(
        &dot_hits[..3],
        &[(288, &[(0, 48)][..]), (288, &[(0, 48)][..]), (240, &[][..])]
    );
    assert!(
        arlan_hits
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .all(|hit| hit.assists.is_empty())
    );
}

#[test]
fn e2_spreads_wind_shear_only_when_a_defeated_target_was_afflicted() {
    for (eidolon, afflicted, expected) in [(0, true, 0), (2, false, 0), (2, true, 1)] {
        let mut plan = prepared(&[sampo(eidolon, &[])], &[1_000; 3]);
        plan.units[1].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        plan.units[1].stats.hp = 1;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if afflicted {
            apply_wind_shear(&mut battle, 0, 1, 3);
        }
        apply(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.hp(1).0, 0);
        assert_eq!(battle.dot_stacks(2, Element::Wind), expected);
        assert_eq!(battle.dot_stacks(3, Element::Wind), expected);
    }
}

#[test]
fn e2_spreads_when_the_defeating_hit_is_the_affliction_itself() {
    let mut plan = prepared(&[sampo(2, &[])], &[1_000; 3]);
    let enemy = fixture::enemy("fixture.target_left", 32).unwrap();
    plan.units[1].kit = Kit::Enemy(enemy);
    plan.units[1].stats = enemy.stats;
    plan.units[1].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply_wind_shear(&mut battle, 0, 1, 3);
    apply(&mut battle, Action::Basic, Some("dpr:2"));

    assert_eq!(battle.hp(1).0, 0);
    assert_eq!(battle.dot_stacks(2, Element::Wind), 4);
    assert_eq!(battle.dot_stacks(3, Element::Wind), 1);
}

#[test]
fn spice_up_mitigates_actual_enemy_damage_only_from_an_afflicted_attacker() {
    for (trace, affected, expected_hp) in
        [(false, true, 899), (true, false, 899), (true, true, 914)]
    {
        let traces = if trace { &[SPICE_UP][..] } else { &[] };
        let mut plan = prepared(&[sampo(0, traces)], &[1_000; 2]);
        let enemy = fixture::enemy("fixture.target_left", 32).unwrap();
        plan.units[1].kit = Kit::Enemy(enemy);
        plan.units[1].stats = enemy.stats;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if affected {
            apply_wind_shear(&mut battle, 0, 1, 3);
        }
        apply(&mut battle, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.hp(0).0, expected_hp);
    }
}

#[test]
fn fixed_dpr_replays_bounces_and_dot_attribution_from_the_same_seed() {
    let plan = prepared(&[sampo(6, &[TRAP, DEFENSIVE_POSITION])], &[30, 120, 30]);
    let run = || calculate_dpr(&plan, &[30, 120, 30], NonZeroU32::new(3).unwrap()).unwrap();
    let first = run();
    assert_eq!(first, run());
    assert!(first.total_damage > 0);
    for hit in first.targets.iter().flat_map(|target| &target.hits) {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
    assert!(
        first
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| { hit.kind == DamageKind::DamageOverTime && hit.source_id == "team.sampo" })
    );
}
