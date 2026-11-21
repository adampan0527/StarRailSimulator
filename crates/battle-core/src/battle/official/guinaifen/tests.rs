use std::num::NonZeroU32;

use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, StatusKind, calculate_dpr, rng::Rng,
};

use super::{
    BASIC, BLADED_HOOP, DotScale, Element, FORM, HIGH_POLES, SKILL, SkillDot, TALENT, ULTIMATE,
    WALKING_ON_KNIVES,
};

fn guinaifen(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.guinaifen".into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Fire,
        },
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
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 2_500,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.6, 0.2, 1.0, 0.83904, 2.0]),
            (ULTIMATE.into(), vec![0.72, 0.72]),
            (TALENT.into(), vec![1.0, 0.0, 0.0, 0.04, 3.0, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|trace| (*trace).into()).collect(),
    }
}

fn hook() -> OfficialCharacter {
    let mut ally = guinaifen(0, &[]);
    ally.unit_id = "ally.hook".into();
    ally.form_id = "character.1109".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Fire,
    };
    ally.skill_parameters = [
        ("skill.110901".into(), vec![0.5]),
        ("skill.110902".into(), vec![1.2, 1.0, 2.0, 0.25]),
        ("skill.110903".into(), vec![2.4]),
        ("skill.110904".into(), vec![0.5, 5.0]),
        ("skill.110909".into(), vec![1.4, 1.0, 2.0, 0.25, 0.4]),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 0, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn cast(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == "ally.guinaifen"
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn put_dot(battle: &mut Battle<'_>, source: usize, target: usize, element: Element) {
    assert!(battle.apply_skill_dot(
        source,
        target,
        10_000,
        SkillDot {
            element,
            damage: DotScale::Attack(5_000),
            origin: Action::Skill,
            turns: 2,
            max_stacks: 1,
        },
    ));
}

#[test]
fn basic_generates_skill_points_and_high_poles_adds_the_skill_burn() {
    for (traces, burns) in [(&[][..], 0), (&[HIGH_POLES][..], 1)] {
        let plan = prepared(&[guinaifen(0, traces)], &[1_000]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        cast(&mut battle, Action::Basic, Some("dpr:1"));

        let snapshot = battle.snapshot();
        assert_eq!(snapshot.skill_points, 4);
        assert_eq!(snapshot.units[0].energy_milli, 80_000);
        assert_eq!(snapshot.units[1].toughness, 990);
        assert_eq!(battle.dummy_damage[0].amount, 300);
        assert_eq!(battle.dot_stacks(1, Element::Fire), burns);
        assert_eq!(battle.vulnerability_from(0, 1), u32::from(burns) * 400);
    }
}

#[test]
fn bladed_hoop_advances_only_the_first_turn() {
    for (traces, first_av) in [(&[][..], 80_000_000), (&[BLADED_HOOP][..], 60_000_000)] {
        let plan = prepared(&[guinaifen(0, traces)], &[1_000]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.snapshot().av_micros, first_av);
        cast(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.snapshot().av_micros, first_av + 80_000_000);
    }
}

#[test]
fn skill_applies_burn_only_to_the_main_and_adjacent_targets() {
    let plan = prepared(&[guinaifen(0, &[])], &[1_000; 5]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    cast(&mut battle, Action::Skill, Some("dpr:3"));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 2);
    assert_eq!(snapshot.units[0].energy_milli, 90_000);
    let toughness: Vec<_> = snapshot.units[1..]
        .iter()
        .map(|unit| unit.toughness)
        .collect();
    assert_eq!(toughness, [1_000, 990, 980, 990, 1_000]);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(hits, [(3, 360), (2, 120), (4, 120)]);
    assert_eq!(
        (1..=5)
            .map(|unit| battle.dot_stacks(unit, Element::Fire))
            .collect::<Vec<_>>(),
        [0, 1, 1, 1, 0]
    );
}

#[test]
fn first_eidolon_reduces_main_target_effect_resistance_for_two_turns() {
    let mut guinaifen = guinaifen(1, &[]);
    guinaifen.stats.effect_hit_rate_bp = 0;
    let mut plan = prepared(&[guinaifen], &[1_000; 3]);
    for unit in &mut plan.units[1..] {
        unit.stats.effect_resistance_bp = 1_500;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.rng = Rng::new(3);
    cast(&mut battle, Action::Skill, Some("dpr:2"));

    let affected: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::StatusApplied(StatusKind::EffectResistanceDown))
        .filter_map(|event| event.target_id.as_deref())
        .collect();
    assert_eq!(affected, ["dpr:2"]);
    // Seed 0 starts with a roll between the unaided 85% and debuffed 95% chances.
    for (unit, expected) in [(1, false), (2, true), (3, false)] {
        battle.rng = Rng::new(0);
        assert_eq!(battle.roll_effect(0, unit, 10_000), expected);
    }
    assert_eq!(battle.snapshot().units[2].normal_turns, 1);
    battle.units[2].normal_turns += 1;
    battle.expire_debuffs(2);
    battle.rng = Rng::new(0);
    assert!(!battle.roll_effect(0, 2, 10_000));
}

#[test]
fn second_eidolon_increases_the_refreshed_burn_and_walking_on_knives_bonus_is_conditional() {
    let plan = prepared(&[guinaifen(2, &[HIGH_POLES, WALKING_ON_KNIVES])], &[1_000]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    cast(&mut battle, Action::Skill, Some("dpr:1"));
    cast(&mut battle, Action::Basic, Some("dpr:1"));

    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, 360),
            (DamageKind::DamageOverTime, 604),
            (DamageKind::Direct, 374),
            (DamageKind::DamageOverTime, 927),
        ]
    );
    assert_eq!(battle.dot_stacks(1, Element::Fire), 1);
    battle.tick_skill_dots(1);
    assert_eq!(battle.dot_stacks(1, Element::Fire), 0);
}

#[test]
fn ultimate_triggers_each_burn_after_direct_damage_and_preserves_its_owner() {
    let plan = prepared(&[guinaifen(6, &[]), hook()], &[1_000; 2]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    put_dot(&mut battle, 0, 2, Element::Fire);
    put_dot(&mut battle, 1, 2, Element::Fire);
    put_dot(&mut battle, 1, 2, Element::Thunder);
    battle.set_energy(0, 0, 120_000);
    cast(&mut battle, Action::Ultimate, None);

    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.actor, hit.target, hit.action, hit.kind, hit.amount))
        .collect();
    assert_eq!(
        hits,
        [
            (0, 2, Action::Ultimate, DamageKind::Direct, 432),
            (0, 3, Action::Ultimate, DamageKind::Direct, 432),
            (0, 2, Action::Ultimate, DamageKind::DamageOverTime, 216),
            (1, 2, Action::Ultimate, DamageKind::DamageOverTime, 224),
        ]
    );
    assert_eq!(battle.dummy_damage[3].assists, [(0, 8)]);
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 3);
    assert_eq!(snapshot.units[0].energy_milli, 7_000);
    assert_eq!(snapshot.units[2].toughness, 980);
    assert_eq!(snapshot.units[3].toughness, 980);
    assert_eq!(battle.vulnerability_from(0, 2), 800);
    assert_eq!(battle.vulnerability_from(0, 3), 0);

    for energy in [9_000, 11_000] {
        let before = battle.dummy_damage.len();
        battle.tick_skill_dots(2);
        assert_eq!(battle.dummy_damage.len(), before + 3);
        assert_eq!(battle.snapshot().units[0].energy_milli, energy);
        assert_eq!(battle.vulnerability_from(0, 2), 1_600);
    }
    assert_eq!(battle.dot_stacks(2, Element::Fire), 0);
    assert_eq!(battle.dot_stacks(2, Element::Thunder), 0);
}

#[test]
fn fire_break_feeds_firekiss_and_refreshes_the_capped_duration() {
    let plan = prepared(&[guinaifen(0, &[])], &[10]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    cast(&mut battle, Action::Basic, Some("dpr:1"));
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::WeaknessBreak)
    );
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::DamageOverTime)
    );
    assert_eq!(battle.vulnerability_from(0, 1), 400);

    put_dot(&mut battle, 0, 1, Element::Fire);
    battle.tick_skill_dots(1);
    battle.tick_skill_dots(1);
    assert_eq!(battle.vulnerability_from(0, 1), 1_200);

    battle.units[1].normal_turns += 2;
    put_dot(&mut battle, 0, 1, Element::Fire);
    battle.tick_skill_dots(1);
    assert_eq!(battle.vulnerability_from(0, 1), 1_200);
    battle.units[1].normal_turns += 2;
    battle.expire_debuffs(1);
    assert_eq!(battle.vulnerability_from(0, 1), 1_200);
    battle.units[1].normal_turns += 1;
    battle.expire_debuffs(1);
    assert_eq!(battle.vulnerability_from(0, 1), 0);
}

#[test]
fn fourth_eidolon_counts_the_owners_break_burn_at_the_energy_regeneration_rate() {
    for (eidolon, energy) in [(0, 85_000), (4, 87_500)] {
        let mut guinaifen = guinaifen(eidolon, &[]);
        guinaifen.stats.energy_regeneration_bp = 12_500;
        let plan = prepared(&[guinaifen], &[10]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        cast(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.snapshot().units[0].energy_milli, energy);
        assert_eq!(battle.vulnerability_from(0, 1), 400);
    }
}

#[test]
fn fixed_dpr_replays_team_burns_and_conserves_assist_credit() {
    let toughness = [30, 120, 30];
    let plan = prepared(
        &[guinaifen(6, &[HIGH_POLES, BLADED_HOOP]), hook()],
        &toughness,
    );
    let run = || calculate_dpr(&plan, &toughness, NonZeroU32::new(3).unwrap()).unwrap();
    let first = run();
    assert_eq!(first, run());
    assert!(first.characters[0].assist_damage > 0);
    for hit in first.targets.iter().flat_map(|target| &target.hits) {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
}
