use super::*;
use crate::{
    CombatType, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    StatusKind,
    battle::{Kit, resolve::gauge_for, support::OffenseContext},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const MEI: &str = "ally.ruan-mei";
const GALLAGHER: &str = "ally.gallagher";
const ACHERON: &str = "ally.acheron";
const ENEMY: &str = "dpr:1";

const FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn ruan_mei(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: MEI.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: Element::Ice,
        },
        energy_capacity: 130,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![if eidolon >= 5 { 1.1 } else { 1.0 }]),
            (
                SKILL.into(),
                vec![if eidolon >= 5 { 0.352 } else { 0.32 }, 0.5, 3.0],
            ),
            (
                ULTIMATE.into(),
                if eidolon >= 3 {
                    vec![0.27, 2.0, 0.2, 0.1, 0.54]
                } else {
                    vec![0.25, 2.0, 0.2, 0.1, 0.5]
                },
            ),
            (
                TALENT.into(),
                if eidolon >= 3 {
                    vec![0.104, 1.32]
                } else {
                    vec![0.1, 1.2]
                },
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn gallagher(eidolon: u8) -> OfficialCharacter {
    let mut input = ruan_mei(eidolon, &[]);
    input.unit_id = GALLAGHER.into();
    input.form_id = "character.1301".into();
    input.energy_capacity = 110;
    input.combat_type = CombatType {
        path: CombatPath::Abundance,
        element: Element::Fire,
    };
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.130101".into(), vec![1.0]),
        ("skill.130102".into(), vec![1_600.0]),
        ("skill.130103".into(), vec![1.5, 2.0]),
        ("skill.130104".into(), vec![0.12, 640.0]),
        ("skill.130108".into(), vec![2.5, 0.15, 2.0]),
    ]
    .into();
    input
}

fn acheron() -> OfficialCharacter {
    let mut input = ruan_mei(0, &[]);
    input.unit_id = ACHERON.into();
    input.form_id = "character.1308".into();
    input.energy_capacity = 0;
    input.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Thunder,
    };
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.130801".into(), vec![1.0]),
        ("skill.130802".into(), vec![1.6, 0.6, 1.0]),
        (
            "skill.130803".into(),
            vec![0.24, 0.15, 1.2, 9.0, 0.6, 3.72, 3.0],
        ),
        ("skill.130804".into(), vec![9.0, 0.2]),
        ("skill.130814".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130815".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130816".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130817".into(), vec![1.2]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[120])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    battle
        .apply(&Command {
            decision_index: battle.snapshot().decision_index,
            actor_id: actor.into(),
            action,
            target_id: target.map(String::from),
        })
        .unwrap();
}

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.set_energy(0, 0, 130_000);
    act(battle, MEI, Action::Ultimate, None);
    assert_eq!(battle.ordinary_actor(), Some(MEI));
}

fn enemy_opportunity(battle: &mut Battle<'_>, target: usize) {
    battle.units[target].gauge_quanta = 0;
    battle.units[target].normal_turns += 1;
    battle.enemy_turn = Some(target);
    battle.resolve_enemy_turn(target);
    battle.enemy_turn = None;
}

#[test]
fn ruan_mei_skill_adds_efficiency_before_rounding_and_a6_uses_live_break_effect() {
    let mut mei = ruan_mei(0, &[A2, A4, A6]);
    mei.stats.break_effect_bp = 13_500;
    mei.stats.energy_regeneration_bp = 12_000;
    let plan = plan(&[mei, gallagher(6)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(MEI));
    assert_eq!(battle.units[0].energy_milli, 71_000);
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert_eq!(battle.speed_micros(1), 110_000_000);

    act(&mut battle, MEI, Action::Skill, None);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 107_000);
    let offense = OffenseContext {
        actor: 1,
        max_hp_source: 1,
        enemy: Some(2),
        element: Element::Fire,
        kind: DamageKind::Direct,
        break_damage: false,
        follow_up: false,
        summon: false,
    };
    let buffs = battle.offense_buffs_from(0, offense);
    assert_eq!(buffs.break_effect_bp, 2_000);
    assert_eq!(buffs.damage_bp, 5_000); // 32% + 3 whole 6% steps beyond 120%.
    battle.apply_ally_buff(1, 0, AllyBuffKind::BreakEffect, 4_000, 3);
    assert_eq!(battle.offense_buffs_from(0, offense).damage_bp, 6_800);

    act(&mut battle, GALLAGHER, Action::Basic, Some(ENEMY));
    assert_eq!(battle.units[2].toughness, 103); // 10 × (1 + 20% + 50%).
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.ordinary_actor(), Some(MEI));
    assert_eq!(battle.units[0].energy_milli, 113_000);
    for _ in 0..8 {
        if battle.units[0].normal_turns == 4 {
            break;
        }
        let current = battle.ordinary_actor().unwrap().to_owned();
        act(&mut battle, &current, Action::Basic, Some(ENEMY));
    }
    assert_eq!(battle.units[0].normal_turns, 4);
    assert_eq!(battle.offense_buffs_from(0, offense).damage_bp, 0);
}

#[test]
fn ruan_mei_field_and_e2_damage_differences_credit_the_same_provider() {
    let prepare = plan(&[ruan_mei(2, &[]), gallagher(0)]);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].energy_milli, 5_000);

    battle.attack_hit(1, 2, Element::Fire, 10_000, 0);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.amount, 803); // 1000 × (1000 / 1400) × 1.25 × 0.9.
    assert_eq!(hit.assists, [(0, 203)]);

    battle.set_toughness(1, 2, 0);
    battle.attack_hit(1, 2, Element::Fire, 10_000, 0);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.amount, 1_250); // 1400 × (1000 / 1400) × 1.25.
    assert_eq!(hit.assists, [(0, 584)]);

    battle.units[0].normal_turns = 3;
    battle.attack_hit(1, 2, Element::Fire, 10_000, 0);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.amount, 933); // Only the conditional 40% ATK remains.
    assert_eq!(hit.assists, [(0, 267)]);
}

#[test]
fn ruan_mei_talent_scales_initial_ice_break_and_only_creates_one_actual_break() {
    let prepare = plan(&[ruan_mei(0, &[A2]), acheron()]);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    battle.set_toughness(0, 2, 1);
    act(&mut battle, MEI, Action::Skill, None);
    act(&mut battle, ACHERON, Action::Basic, Some(ENEMY));

    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::WeaknessBreak)
        .collect();
    assert_eq!(hits.len(), 2);
    // Base 3767.5533 × 3.5 for 120 max toughness × 2/3 target defense.
    assert_eq!((hits[0].actor, hits[0].amount), (1, 10_549));
    assert_eq!(hits[0].assists, [(0, 1_759)]);
    assert_eq!((hits[1].actor, hits[1].amount), (0, 12_658));
    assert!(hits[1].assists.is_empty());
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::WeaknessBroken)
            .count(),
        1,
    );
    assert!(!battle.is_frozen(2));
}

#[test]
fn ruan_mei_e4_refreshes_before_talent_and_e6_does_not_scale_rebloom() {
    let prepare = plan(&[ruan_mei(6, &[A2]), acheron()]);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].ruan_mei_state.field_until_turn, 4);
    battle.set_toughness(0, 2, 1);
    act(&mut battle, MEI, Action::Skill, None);
    act(&mut battle, ACHERON, Action::Basic, Some(ENEMY));

    assert_eq!(battle.effective_break_effect(0), 12_000);
    let talent = battle.dummy_damage.last().unwrap();
    assert_eq!(talent.actor, 0);
    // 3767.5533 × 3.5 × (1000/1400) × 1.27 × 2.2 × 3.32.
    assert_eq!(talent.amount, 87_370);
    enemy_opportunity(&mut battle, 2);
    let rebloom = battle.dummy_damage.last().unwrap();
    assert_eq!(rebloom.action, Action::Ultimate);
    // The same formula uses 54%, without E6's 200% talent increase.
    assert_eq!(rebloom.amount, 14_210);
    assert_eq!(battle.units[2].gauge_quanta, gauge_for(3_400));
}

#[test]
fn ruan_mei_rebloom_stays_after_field_expiry_and_delays_only_one_recovery() {
    let prepare = plan(&[ruan_mei(0, &[A2])]);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    cast_ultimate(&mut battle);
    act(&mut battle, MEI, Action::Basic, Some(ENEMY));
    assert!(has_rebloom(&battle, 0, 1));
    battle.set_toughness(0, 1, 0);

    battle.units[0].normal_turns = 3;
    let before = battle.dummy_damage.len();
    enemy_opportunity(&mut battle, 1);
    assert_eq!(battle.units[1].toughness, 0);
    assert_eq!(battle.units[1].gauge_quanta, gauge_for(1_400));
    assert!(!has_rebloom(&battle, 0, 1));
    assert_eq!(battle.dummy_damage.len(), before + 1);
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 5_274);

    battle.set_energy(0, 0, 130_000);
    act(&mut battle, MEI, Action::Ultimate, None);
    act(&mut battle, MEI, Action::Basic, Some(ENEMY));
    assert!(!has_rebloom(&battle, 0, 1));
    enemy_opportunity(&mut battle, 1);
    assert_eq!(battle.units[1].toughness, 120);
    act(&mut battle, MEI, Action::Basic, Some(ENEMY));
    assert!(has_rebloom(&battle, 0, 1));
}

#[test]
fn ruan_mei_rebloom_applies_once_per_target_and_only_after_ice_thaws() {
    let prepare = plan(&[ruan_mei(0, &[]), acheron()]);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    cast_ultimate(&mut battle);
    battle.set_toughness(0, 2, 1);
    act(&mut battle, MEI, Action::Basic, Some(ENEMY));
    let gains = |battle: &Battle<'_>| {
        battle.events().iter().filter(|event| {
            matches!(event.kind, EventKind::SlashedDream { after, before, .. } if after > before)
        }).count()
    };
    assert_eq!(gains(&battle), 1);
    assert!(has_rebloom(&battle, 0, 2));
    assert!(battle.is_frozen(2));
    act(&mut battle, ACHERON, Action::Basic, Some(ENEMY));
    assert_eq!(gains(&battle), 1);

    enemy_opportunity(&mut battle, 2);
    assert!(!battle.is_frozen(2));
    assert!(has_rebloom(&battle, 0, 2));
    assert_eq!(battle.units[2].gauge_quanta, gauge_for(5_000));
    enemy_opportunity(&mut battle, 2);
    assert!(!has_rebloom(&battle, 0, 2));
    assert_eq!(battle.units[2].toughness, 0);
}

#[test]
fn ruan_mei_rebloom_prevents_a_lethal_enemys_attack_and_finishes_once() {
    let mut prepare = plan(&[ruan_mei(0, &[])]);
    prepare.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&prepare, LogMode::Detailed);
    cast_ultimate(&mut battle);
    act(&mut battle, MEI, Action::Basic, Some(ENEMY));
    battle.units[1].hp = 500;
    battle.set_toughness(0, 1, 0);

    enemy_opportunity(&mut battle, 1);
    assert_eq!(battle.units[1].hp, 0);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::UnitDefeated)
    );
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::EnemyAttack)
    );
    assert!(
        battle.events().iter().any(|event| {
            event.kind == EventKind::StatusExpired(StatusKind::ThanatoplumRebloom)
        })
    );
}

#[test]
fn ruan_mei_fixed_dpr_replays_with_break_damage_and_conserved_assists() {
    let prepare = plan(&[ruan_mei(6, &[A2, A4, A6]), gallagher(6), acheron()]);
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepare, &scenario.toughness, scenario.cycles).unwrap();
    let first = run();
    assert_eq!(first, run());
    let hits = || first.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == MEI && hit.kind == DamageKind::WeaknessBreak));
    assert!(hits().any(|hit| hit.source_id != MEI && hit.credits.iter().any(|c| c.unit_id == MEI)));
    assert_eq!(
        first.total_damage,
        first
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
