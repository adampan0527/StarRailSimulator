use crate::{
    CombatType, Command, LogMode, OfficialStats, Outcome, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;
use crate::battle::official::cerydra::tests as cerydra;

const SPARKLE: &str = "character.1306";
const LEFT: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn archer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: QUANTUM,
        },
        energy_capacity: 220,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            speed_micros: 200_000_000,
            base_speed_micros: 105_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![3.6, 1.0, 2.0, 1.0, 5.0]),
            (ULTIMATE.into(), vec![10.0, 2.0, 4.0]),
            (TALENT.into(), vec![2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn sparkle(speed: u64) -> OfficialCharacter {
    let mut input = archer(0, &[]);
    input.unit_id = SPARKLE.into();
    input.form_id = SPARKLE.into();
    input.combat_type.path = CombatPath::Harmony;
    input.energy_capacity = 110;
    input.stats.speed_micros = speed * 1_000_000;
    input.stats.base_speed_micros = input.stats.speed_micros;
    input.skill_parameters = [
        ("skill.130601".into(), vec![1.0]),
        ("skill.130602".into(), vec![0.24, 0.45, 1.0, 0.5]),
        ("skill.130603".into(), vec![2.0, 4.0, 0.1, 2.0]),
        ("skill.130604".into(), vec![2.0, 0.06, 2.0, 3.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter], points: u8, enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, points)
        .unwrap()
        .with_dummies(&[1_000; 2][..enemies])
        .unwrap()
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

#[test]
fn builds_frozen_low_levels_with_a_wide_ultimate_and_validates_circuit_constants() {
    let mut input = archer(0, &[]);
    input.skill_parameters.get_mut(BASIC).unwrap()[0] = 0.5;
    input.skill_parameters.get_mut(SKILL).unwrap()[..2].copy_from_slice(&[1.8, 0.6]);
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[0] = 6.0;
    input.skill_parameters.get_mut(TALENT).unwrap()[0] = 1.0;
    let kit = build(&input).unwrap();
    assert_eq!(kit.max_energy_milli, 220_000);
    assert_eq!(kit.ultimate.energy_milli, 5_000);
    assert!(matches!(
        kit.ultimate.effects.as_slice(),
        [Effect::WideHit {
            multiplier_bp: 60_000,
            toughness: 30,
            ..
        }]
    ));
    input.energy_capacity = 219;
    assert!(build(&input).is_err());
    input.energy_capacity = 220;
    input.skill_parameters.get_mut(SKILL).unwrap()[4] = 6.0;
    assert!(build(&input).is_err());
}

#[test]
fn pays_two_points_and_explicitly_ends_the_same_turn_without_attacking() {
    let prepared = plan(&[archer(0, &[]), sparkle(110)], 5, 2);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let opening_av = battle.av_micros;
    let previous_command = command(&battle, FORM, Action::Skill, Some(MIDDLE));
    battle.apply(&previous_command).unwrap();
    assert_eq!(battle.ordinary_actor, Some(0));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(battle.units[0].archer_state.circuit_skills, 1);
    assert_eq!(battle.av_micros, opening_av);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|action| action.actor_id != FORM || action.action != Action::Basic)
    );
    let snapshot = battle.snapshot();
    let event_count = battle.events().len();
    assert!(battle.apply(&previous_command).is_err());
    let wrong_target = command(&battle, FORM, Action::EnhancedSkill, Some(MIDDLE));
    assert!(battle.apply(&wrong_target).is_err());
    assert_eq!(battle.snapshot(), snapshot);
    assert_eq!(battle.events().len(), event_count);

    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.ordinary_actor, Some(1));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(battle.units[0].archer_state.circuit_skills, 0);
    assert_eq!(battle.dummy_damage.len(), 1);
}

#[test]
fn closes_the_circuit_when_the_next_skill_is_unaffordable() {
    let prepared = plan(&[archer(0, &[]), sparkle(110)], 2, 2);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));

    assert_eq!(battle.skill_points, 0);
    assert_eq!(battle.ordinary_actor, Some(1));
    assert_eq!(battle.units[0].archer_state.circuit_skills, 0);
    assert_eq!(battle.units[0].energy_milli, 140_000);
}

#[test]
fn limits_e1_and_e6_to_five_skills_with_one_refund_and_capped_damage_stacks() {
    for (eidolon, points, expected) in [
        (1, 9, [2_419, 4_708, 6_868, 6_868, 6_868]),
        (6, 8, [2_592, 5_045, 7_359, 9_673, 9_673]),
    ] {
        let prepared = plan(
            &[archer(eidolon, &[SKILL_POINT_CAP_TRACE]), sparkle(110)],
            points,
            2,
        );
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        assert_eq!(battle.skill_point_cap(), 9);
        assert_eq!(battle.skill_points, 9);
        let opening_av = battle.av_micros;
        for remaining in [7, 5, 5, 3, 1] {
            assert_eq!(battle.av_micros, opening_av);
            act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
            assert_eq!(battle.skill_points, remaining);
        }
        assert_eq!(battle.ordinary_actor, Some(1));
        assert_eq!(battle.units[0].archer_state.circuit_skills, 0);
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn fifth_cast_ends_the_turn_even_with_points_for_a_sixth() {
    let mut support = sparkle(110);
    support.eidolon = 4;
    let prepared = plan(&[archer(1, &[SKILL_POINT_CAP_TRACE]), support], 10, 2);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.skill_point_cap(), 10);
    for _ in 0..5 {
        act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    }
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.ordinary_actor, Some(1));
    assert_eq!(battle.units[0].archer_state.circuit_skills, 0);
}

#[test]
fn cerydra_copy_does_not_pay_again_or_advance_the_circuit_counter() {
    let mut support = cerydra::character(0, &[]);
    support.stats.speed_micros = 110_000_000;
    let prepared = plan(&[archer(1, &[SKILL_POINT_CAP_TRACE]), support], 7, 2);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cerydra::grant_merit(&mut battle, 1, 0, 6);
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::Direct)
            .count(),
        2
    );
    assert_eq!(battle.units[0].archer_state.circuit_skills, 1);
    assert_eq!(battle.skill_points, 5);

    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.units[0].archer_state.circuit_skills, 2);
    assert_eq!(battle.skill_points, 3);
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.units[0].archer_state.circuit_skills, 3);
    assert_eq!(battle.skill_points, 3);

    let merit = cerydra::charge(&battle, 1);
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(cerydra::charge(&battle, 1), merit);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn applies_the_crit_damage_trace_on_real_shared_point_gains() {
    let prepared = plan(&[archer(0, &[CRIT_DAMAGE_TRACE]), sparkle(110)], 2, 2);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.restore_skill_points(1, 1), 1);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage), 0);
    assert_eq!(battle.restore_skill_points(1, 1), 1);
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage),
        12_000
    );
    assert_eq!(battle.restore_skill_points(1, 10), 3);
    let event_count = battle.events().len();
    assert_eq!(battle.restore_skill_points(1, 1), 0);
    assert_eq!(battle.events().len(), event_count);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage),
        12_000
    );
    act(&mut battle, SPARKLE, Action::Basic, Some(MIDDLE));
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage), 0);
}

#[test]
fn follow_up_retargets_a_finite_kill_and_ignores_non_attacking_actions() {
    let mut prepared = plan(&[archer(0, &[INITIAL_CHARGE_TRACE]), sparkle(250)], 3, 2);
    prepared.units[2].kit = Kit::Enemy(&FINITE);
    prepared.units[2].stats.hp = 1;
    prepared.units[0].starting_energy_milli = 220_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);

    assert_eq!(battle.units[0].archer_state.charge, 1);
    act(&mut battle, FORM, Action::Ultimate, Some(MIDDLE));
    assert_eq!(battle.units[0].archer_state.charge, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    act(&mut battle, SPARKLE, Action::Basic, Some(LEFT));

    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(battle.units[0].archer_state.charge, 2);
    assert_eq!(battle.units[0].energy_milli, 10_000);
    assert_eq!(battle.skill_points, 5);
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| { hit.actor == 0 && hit.target == 3 && hit.kind == DamageKind::FollowUp })
    );
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    act(&mut battle, SPARKLE, Action::Skill, Some(FORM));
    assert_eq!(battle.units[0].archer_state.charge, 2);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::FollowUp))
            .count(),
        1
    );
}

#[test]
fn does_not_spend_charge_when_an_earlier_follow_up_finishes_the_wave() {
    let mut prepared = plan(&[archer(0, &[INITIAL_CHARGE_TRACE]), sparkle(110)], 3, 1);
    prepared.units[2].kit = Kit::Enemy(&FINITE);
    prepared.units[2].stats.hp = 1;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();

    battle.follow_up_single_or_random(
        1,
        Some(2),
        Action::Basic,
        FollowUpAttack {
            element: QUANTUM,
            multiplier_bp: 10_000,
            toughness: 10,
            energy_milli: 0,
            first_hit: None,
        },
    );
    mechanics.on_ally_attack(&mut battle, 0, 1, Action::Basic, Some(2));
    battle.resolve_follow_ups();

    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(battle.units[0].archer_state.charge, 1);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn an_ultimate_killing_a_wave_closes_the_open_circuit_without_leaking_stacks() {
    let mut prepared = plan(&[archer(0, &[SKILL_POINT_CAP_TRACE])], 7, 2);
    prepared.wave_ends = vec![2, 3];
    prepared.units[1].kit = Kit::Enemy(&FINITE);
    prepared.units[1].stats.hp = 3_000;
    prepared.units[2].kit = Kit::Enemy(&FINITE);
    prepared.units[2].stats.hp = 20_000;
    prepared.units[0].starting_energy_milli = 220_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    assert_eq!(battle.wave_index, 0);
    assert_eq!(battle.units[0].archer_state.circuit_skills, 1);
    let before = battle.av_micros;

    act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
    assert_eq!(battle.outcome, Outcome::Ongoing);
    assert_eq!(battle.wave_index, 1);
    assert!(battle.av_micros > before);
    assert_eq!(battle.units[0].archer_state.circuit_skills, 0);
    assert_eq!(battle.units[0].archer_state.charge, 2);
    assert!(
        battle
            .legal_actions()
            .iter()
            .any(|action| action.action == Action::Basic)
    );
}

#[test]
fn e2_implants_before_damage_e4_increases_ultimate_damage_and_charge_caps_at_four() {
    let mut amounts = Vec::new();
    for eidolon in [0, 2, 4] {
        let mut prepared = plan(&[archer(eidolon, &[INITIAL_CHARGE_TRACE])], 5, 1);
        prepared.units[1].kit = Kit::Enemy(&FINITE);
        prepared.units[1].stats.hp = 100_000;
        prepared.units[0].starting_energy_milli = 220_000;
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
        assert_eq!(battle.units[0].archer_state.charge, 3);
        assert_eq!(battle.has_weakness(1, QUANTUM), eidolon >= 2);
        assert_eq!(
            battle.units[1].toughness,
            if eidolon >= 2 { 970 } else { 1_000 }
        );
        amounts.push(100_000 - battle.units[1].hp);
        battle.units[0].energy_milli = 220_000;
        act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
        assert_eq!(battle.units[0].archer_state.charge, 4);
    }
    assert!(amounts[1] > amounts[0]);
    assert!(amounts[2] > amounts[1] * 2);
}

#[test]
fn fixed_dpr_is_deterministic_and_credits_archer_follow_up_damage() {
    let prepared = plan(
        &[
            archer(
                6,
                &[
                    SKILL_POINT_CAP_TRACE,
                    INITIAL_CHARGE_TRACE,
                    CRIT_DAMAGE_TRACE,
                ],
            ),
            sparkle(160),
        ],
        3,
        2,
    );
    let run = || {
        calculate_dpr(
            &prepared,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
    for kind in [DamageKind::Direct, DamageKind::FollowUp] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| { hit.source_id == FORM && hit.kind == kind })
        );
    }
}
