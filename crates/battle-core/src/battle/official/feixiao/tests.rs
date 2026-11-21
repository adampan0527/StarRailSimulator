use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const CENTER: &str = "dpr:2";
const LEFT: &str = "dpr:1";
const TOPAZ: &str = "character.1112";
const BRONYA: &str = "character.1101";
const YUKONG: &str = "character.1207";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn feixiao(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: Element::Wind,
        },
        energy_capacity: 0,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 112_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![2.0]),
            (ULTIMATE.into(), vec![1.6, 1.0, 6.0, 7.0]),
            (BOLTSUNDER_BLITZ.into(), vec![0.6, 0.3]),
            (WARAXE_SKYWARD.into(), vec![0.6, 0.3]),
            (TALENT.into(), vec![1.1, 2.0, 6.0, 12.0, 0.6, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn topaz() -> OfficialCharacter {
    let mut input = feixiao(0, &[]);
    input.unit_id = TOPAZ.into();
    input.form_id = TOPAZ.into();
    input.energy_capacity = 130;
    input.combat_type.element = Element::Fire;
    input.stats.speed_micros = 240_000_000;
    input.skill_parameters = [
        ("skill.111201".into(), vec![0.5]),
        ("skill.111202".into(), vec![0.75, 0.25]),
        ("skill.111203".into(), vec![0.75, 0.125, 0.5, 2.0]),
        ("skill.111204".into(), vec![80.0, 0.75, 0.5]),
    ]
    .into();
    input
}

fn bronya() -> OfficialCharacter {
    let mut input = feixiao(0, &[]);
    input.unit_id = BRONYA.into();
    input.form_id = BRONYA.into();
    input.energy_capacity = 120;
    input.combat_type.path = CombatPath::Harmony;
    input.stats.speed_micros = 240_000_000;
    input.skill_parameters = [
        ("skill.110101".into(), vec![0.5]),
        ("skill.110102".into(), vec![0.33, 0.0, 1.0, 1.0]),
        ("skill.110103".into(), vec![0.33, 0.12, 0.12, 2.0]),
        ("skill.110104".into(), vec![0.15]),
    ]
    .into();
    input
}

fn yukong() -> OfficialCharacter {
    let mut input = feixiao(2, &[]);
    input.unit_id = YUKONG.into();
    input.form_id = YUKONG.into();
    input.energy_capacity = 130;
    input.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Imaginary,
    };
    input.skill_parameters = [
        ("skill.120701".into(), vec![1.0]),
        ("skill.120702".into(), vec![2.0, 0.8]),
        ("skill.120703".into(), vec![3.8, 0.28, 0.65]),
        ("skill.120704".into(), vec![0.8, 1.0, 1.0]),
    ]
    .into();
    input
}

fn prepare(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == Some(target)
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn aureus(battle: &Battle<'_>) -> u8 {
    battle.units[0].feixiao_state.aureus_halves
}

#[test]
fn feixiao_resource_is_independent_of_energy_and_spends_only_six_points() {
    let mut input = feixiao(0, &[HEAVENPATH]);
    input.stats.energy_regeneration_bp = 25_000;
    let mut plan = prepare(&[input]);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(aureus(&battle), 6);
    battle.gain_energy(0, 0, 30_000);
    battle.restore_energy_flat(0, 0, 50_000);
    battle.fill_energy(0);
    battle.resolve_enemy(2, &FINITE_ENEMY);
    assert_eq!(aureus(&battle), 6);
    assert_eq!(battle.snapshot().units[0].max_energy_milli, 0);
    assert_eq!(battle.snapshot().units[0].energy_milli, 0);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::EnemyAttack)
    );
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::Energy { .. }))
    );
    let command = Command {
        decision_index: battle.snapshot().decision_index,
        actor_id: FORM.into(),
        action: Action::Ultimate,
        target_id: Some(CENTER.into()),
    };
    let before = battle.snapshot();
    assert!(!battle.legal_actions().contains(&command));
    assert_eq!(
        battle.apply(&command),
        Err(BattleError::InsufficientUltimateResource)
    );
    assert_eq!(battle.snapshot(), before);

    Feixiao::change_aureus(&mut battle, 0, 0, 24);
    assert_eq!(aureus(&battle), 24);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    assert_eq!(aureus(&battle), 12);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    assert_eq!(aureus(&battle), 0);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert!(battle.events().iter().any(|event| {
        event.kind
            == EventKind::FlyingAureus {
                before_halves: 24,
                after_halves: 12,
            }
    }));
}

#[test]
fn feixiao_does_not_count_as_full_energy_for_yukong_e2() {
    let plan = prepare(&[feixiao(0, &[HEAVENPATH]), yukong()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[1].energy_milli, 65_000);
    Feixiao::change_aureus(&mut battle, 0, 0, 24);
    assert_eq!(battle.units[1].energy_milli, 65_000);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    assert_eq!(battle.units[1].energy_milli, 65_000);
}

#[test]
fn feixiao_skill_and_its_follow_up_are_two_attacks_with_buffs_in_hit_order() {
    let plan = prepare(&[feixiao(0, &[BOLTING_GRASP, HEAVENPATH])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, CENTER);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[2].toughness, 375);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    // 1480 ATK x 0.6 x 2.0 = 1776; 1480 x 0.6 x 1.1 x 1.6 = 1562.88.
    assert_eq!(
        hits,
        [(DamageKind::Direct, 1776), (DamageKind::FollowUp, 1562)]
    );
    // Two real attacks, then Heavenpath's half-point at the start of the next turn.
    assert_eq!(aureus(&battle), 9);
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::AttackPercent),
        4_800
    );
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 6_000);
    for _ in 0..3 {
        act(&mut battle, FORM, Action::Basic, CENTER);
    }
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::AttackPercent), 0);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 0);
}

#[test]
fn feixiao_ally_attack_triggers_once_each_turn_and_has_a_living_target_fallback() {
    let mut plan = prepare(&[feixiao(0, &[HEAVENPATH]), topaz()]);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Basic, LEFT);
    assert_eq!(aureus(&battle), 8);
    assert_eq!(battle.units[2].hp, 0);
    let targets: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| {
            event.kind == EventKind::FollowUp && event.source_id.as_deref() == Some(FORM)
        })
        .map(|event| event.target_id.as_deref().unwrap())
        .collect();
    assert_eq!(targets.len(), 1);
    assert_ne!(targets[0], LEFT);
    // The first ally attack was before Feixiao's first turn. That turn resets the trigger.
    act(&mut battle, FORM, Action::Basic, CENTER);
    act(&mut battle, TOPAZ, Action::Skill, CENTER);
    // The Skill and Feixiao's response advance Numby, who adds another half-point.
    assert_eq!(aureus(&battle), 12);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| { hit.actor == 0 && hit.kind == DamageKind::FollowUp })
            .count(),
        2
    );
}

#[test]
fn feixiao_non_attacking_ally_skill_does_not_charge_or_trigger_a_follow_up() {
    let plan = prepare(&[feixiao(0, &[]), bronya()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, BRONYA, Action::Skill, FORM);
    assert_eq!(aureus(&battle), 0);
    assert!(battle.dummy_damage.is_empty());
    act(&mut battle, FORM, Action::Basic, CENTER);
    assert_eq!(aureus(&battle), 1);
}

#[test]
fn feixiao_ultimate_reads_toughness_before_each_hit_and_ignores_wind_weakness() {
    let mut input = feixiao(0, &[]);
    // Unequal coefficients isolate the choice. Both alternatives have equal coefficients in 4.7.0.
    input.skill_parameters.get_mut(BOLTSUNDER_BLITZ).unwrap()[1] = 0.4;
    let mut plan = prepare(&[input]);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[2].toughness = 25;
    Feixiao::change_aureus(&mut battle, 0, 0, 12);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    let toughness: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Toughness { before, after } => Some((before, after)),
            _ => None,
        })
        .collect();
    assert_eq!(toughness, [(25, 15), (15, 5), (5, 0)]);
    let damage: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::Direct,
                amount,
                ..
            } => Some(amount),
            _ => None,
        })
        .collect();
    // The breaking hit still reads 5 toughness. The next three hits use Boltsunder's 100%.
    assert_eq!(damage, [540, 540, 540, 666, 666, 666, 1066]);
    assert_eq!(aureus(&battle), 0);
}

#[test]
fn feixiao_e1_ultimate_stacks_after_each_strike_and_resets_on_next_cast() {
    let plan = prepare(&[feixiao(1, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Feixiao::change_aureus(&mut battle, 0, 0, 24);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    let hits: Vec<_> = battle.dummy_damage.iter().map(|hit| hit.amount).collect();
    assert_eq!(hits, [540, 594, 648, 702, 756, 810, 1440].repeat(2));
    assert_eq!(battle.units[2].toughness, 280);
}

#[test]
fn feixiao_e2_replaces_half_points_for_six_follow_ups_and_resets_per_turn() {
    let plan = prepare(&[feixiao(2, &[]), topaz()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..7 {
        // Use the shared follow-up queue to generate seven distinct attacks before any new turn.
        battle.follow_up_single(
            1,
            3,
            Action::Skill,
            FollowUpAttack {
                element: Element::Fire,
                multiplier_bp: 1_000,
                toughness: 0,
                energy_milli: 0,
                first_hit: None,
            },
        );
    }
    battle.resolve_follow_ups();
    assert_eq!(battle.units[0].feixiao_state.eidolon_follow_ups, 6);
    // Eight attacks including Feixiao's one response. Six x 1 and two x 0.5.
    assert_eq!(aureus(&battle), 14);
    act(&mut battle, TOPAZ, Action::Basic, CENTER);
    assert_eq!(battle.units[0].feixiao_state.eidolon_follow_ups, 0);
    act(&mut battle, FORM, Action::Skill, CENTER);
    assert_eq!(battle.units[0].feixiao_state.eidolon_follow_ups, 1);
    assert_eq!(aureus(&battle), 18);
}

#[test]
fn feixiao_counts_many_targets_in_one_follow_up_as_one_attack() {
    let plan = prepare(&[feixiao(2, &[]), topaz()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.follow_up(1, Element::Fire, 1_000, 0, Action::Skill, 0);
    battle.resolve_follow_ups();
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 1)
            .count(),
        3
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0)
            .count(),
        1
    );
    assert_eq!(aureus(&battle), 4);
    assert_eq!(battle.units[0].feixiao_state.eidolon_follow_ups, 2);
}

#[test]
fn feixiao_e4_e6_follow_up_uses_extra_toughness_speed_ultimate_penetration_and_crit() {
    let mut input = feixiao(6, &[FORMSHIFT, BOLTING_GRASP]);
    input.stats.crit_rate_bp = 10_000;
    let plan = prepare(&[input]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, CENTER);
    assert_eq!(battle.units[2].toughness, 370);
    assert_eq!(battle.snapshot().units[0].speed_micros, 208_960_000);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusApplied(StatusKind::SpeedUp))
    );
    // Skill: 1480 x 2.0 x 0.6 x 1.5. Follow-up: 1480 x 2.5 x 0.6 x 1.6 x 1.2 x 1.86.
    assert_eq!(battle.dummy_damage[0].amount, 2664);
    assert_eq!(battle.dummy_damage[1].amount, 7928);
    Feixiao::change_aureus(&mut battle, 0, 0, 12);
    act(&mut battle, FORM, Action::Ultimate, CENTER);
    // The first ultimate strike also receives the 20% penetration and the A4 critical bonus.
    assert_eq!(battle.dummy_damage[2].amount, 2854);
    assert_eq!(aureus(&battle), 3);
}

#[test]
fn feixiao_dpr_uses_independent_gauge_with_numby_and_keeps_hit_attribution() {
    let plan =
        PreparedBattle::official_dpr(&[feixiao(2, &[HEAVENPATH, FORMSHIFT]), topaz()], 42, 3)
            .unwrap();
    let scenario = &STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    let hits: Vec<_> = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .collect();
    let ultimates = hits
        .iter()
        .filter(|hit| {
            hit.source_id == FORM
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::Direct
        })
        .count();
    assert!(ultimates >= 7);
    assert_eq!(ultimates % 7, 0);
    assert!(
        hits.iter()
            .any(|hit| { hit.source_id == FORM && hit.kind == DamageKind::FollowUp })
    );
    assert!(
        hits.iter()
            .any(|hit| hit.source_id == TOPAZ && hit.kind == DamageKind::FollowUp)
    );
    assert!(hits.iter().any(|hit| {
        hit.source_id == FORM
            && hit.action == Action::Ultimate
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == TOPAZ && credit.amount > 0)
    }));
    for hit in hits {
        assert_eq!(
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>(),
            hit.amount
        );
    }
    assert_eq!(result, run());
}
