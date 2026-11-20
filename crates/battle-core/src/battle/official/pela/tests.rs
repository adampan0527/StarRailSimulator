use std::{collections::BTreeMap, num::NonZeroU32};

use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, calculate_dpr,
};

use super::{A2, A4, A6, BASIC, SKILL, TALENT, ULTIMATE};

fn character(
    id: &str,
    energy_capacity: u32,
    eidolon: u8,
    traces: &[&str],
    parameters: &[(&str, &[f64])],
) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: id.into(),
        form_id: id.into(),
        energy_capacity,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
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
        skill_parameters: parameters
            .iter()
            .map(|(id, values)| ((*id).into(), values.to_vec()))
            .collect::<BTreeMap<_, _>>(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn pela(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    character(
        "character.1106",
        110,
        eidolon,
        traces,
        &[
            (BASIC, &[0.50]),
            (SKILL, &[1.05, 1.0]),
            (ULTIMATE, &[1.0, 0.30, 2.0, 0.60]),
            (TALENT, &[5.0]),
        ],
    )
}

fn arlan() -> OfficialCharacter {
    character(
        "character.1008",
        110,
        0,
        &[],
        &[
            ("skill.100801", &[0.5]),
            ("skill.100802", &[0.15, 1.2]),
            ("skill.100803", &[1.92, 0.96]),
            ("skill.100804", &[0.36]),
        ],
    )
}

fn prepared(team: &[OfficialCharacter], seed: u32) -> PreparedBattle {
    PreparedBattle::official_dpr(team, seed, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
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
fn basic_and_skill_use_their_resources_and_require_a_debuff_for_talent() {
    let plan = prepared(&[pela(0, &[A2])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[0].energy_milli, 75_000);
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[2].toughness, 990);

    apply(&mut battle, Action::Skill, Some("dpr:2"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[0].energy_milli, 105_000);
    assert_eq!(snapshot.skill_points, 3);
    assert_eq!(snapshot.units[2].toughness, 970);
    assert!(!battle.is_debuffed(2));
    // No debuff means no A2 bonus. DEF multiplier 2/3 × unbroken 0.9.
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [150, 150, 630],
    );
}

#[test]
fn ultimate_shreds_each_targets_defense_before_damage_and_recharges_talent_once() {
    let plan = prepared(&[pela(6, &[A2])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 110_000);
    apply(&mut battle, Action::Ultimate, None);
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[0].energy_milli, 10_000);
    assert_eq!(snapshot.skill_points, 3);
    assert!((1..=3).all(|unit| snapshot.units[unit].toughness == 980 && battle.is_debuffed(unit)));
    // DEF = 500 × .70 = 350. A2 adds 20% against each now-debuffed target.
    // 600/400 × .90 × 1000/1350 × 1.20 = 480/320.
    for target in 1..=3 {
        let hits = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == target)
            .map(|hit| (hit.kind, hit.amount, hit.action))
            .collect::<Vec<_>>();
        assert_eq!(
            hits,
            [
                (DamageKind::Direct, 480, Action::Ultimate),
                (DamageKind::Additional, 320, Action::Ultimate),
            ]
        );
    }
    let energy_steps = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Energy { after_milli, .. } => Some(after_milli),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(energy_steps, [110_000, 0, 5_000, 10_000]);
}

#[test]
fn talent_checks_hit_targets_and_triggers_once_after_both_basic_hits() {
    let plan = prepared(&[pela(0, &[A2])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_defense_down(0, 2, 3_000, 2);
    apply(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.snapshot().units[0].energy_milli, 75_000);
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.snapshot().units[0].energy_milli, 100_000);
    // Each basic hit uses 25% ATK and sees both DEF down and the A2 bonus.
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [150, 150, 200, 200],
    );
}

#[test]
fn exposed_expires_after_two_target_turns_and_does_not_leave_stale_talent_triggers() {
    let plan = prepared(&[pela(0, &[])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 110_000);
    apply(&mut battle, Action::Ultimate, None);
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert!(battle.is_debuffed(2));
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert!(!battle.is_debuffed(2));

    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 150);
}

#[test]
fn skill_e4_applies_ice_resistance_down_and_e6_only_hits_the_attacked_target() {
    let plan = prepared(&[pela(6, &[A2])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.snapshot().units[0].energy_milli, 90_000);
    // 1050/400 × 1000/1500 × .90 × 1.12 (E4) × 1.20 (A2).
    let hits = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.kind, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        [
            (2, DamageKind::Direct, 846),
            (2, DamageKind::Additional, 322),
        ]
    );
    assert_eq!(battle.snapshot().units[2].toughness, 980);
    assert!(battle.is_debuffed(2));
    apply(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.snapshot().units[0].energy_milli, 110_000);
    assert_eq!(battle.dummy_damage.len(), 4);
    let last_hit = battle.dummy_damage.last().unwrap();
    assert_eq!(
        (last_hit.target, last_hit.kind, last_hit.amount),
        (1, DamageKind::Direct, 150)
    );
}

#[test]
fn secret_strategy_increases_the_other_ally_effect_hit_rate_only_while_alive() {
    for (enabled, alive, expected) in [
        (false, true, false),
        (true, true, true),
        (true, false, false),
    ] {
        let traces = if enabled { &[A4][..] } else { &[] };
        let plan = prepared(&[pela(0, traces), arlan()], 44);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if !alive {
            battle.units[0].hp = 0;
        }
        // Seed 44 first draws 9815 / 10000 when the chance requires a roll.
        assert_eq!(battle.roll_effect(1, 2, 9_500), expected);
    }
}

#[test]
fn dispelling_an_enemy_buff_raises_speed_and_only_boosts_the_next_attack() {
    let plan = prepared(&[pela(2, &[A6])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_speed_up(2, 2, 2_000, 2);
    apply(&mut battle, Action::Skill, Some("dpr:2"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[2].speed_micros, 100_000_000);
    assert_eq!(snapshot.units[0].speed_micros, 220_000_000);
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 630);

    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 180);
    apply(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 150);
    assert_eq!(battle.snapshot().units[0].speed_micros, 200_000_000);
}

#[test]
fn eidolon_one_gains_energy_when_another_ally_defeats_a_target() {
    use crate::{battle::Kit, fixture};

    for (eidolon, expected_energy) in [(0, 75_000), (1, 80_000)] {
        let mut plan = prepared(&[pela(eidolon, &[]), arlan()], 42);
        let target = &mut plan.units[2];
        target.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        target.stats.hp = 1;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply(&mut battle, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.ordinary_actor(), Some("character.1008"));
        apply(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.snapshot().units[0].energy_milli, expected_energy);
        assert_eq!(battle.snapshot().units[2].hp, 0);
    }
}

#[test]
fn team_dpr_attributes_defense_down_to_pela_and_repeats_for_the_same_seed() {
    let plan = prepared(&[pela(0, &[A2]), arlan()], 42);
    let calculate = || calculate_dpr(&plan, &[1_000; 3], NonZeroU32::new(3).unwrap()).unwrap();
    let first = calculate();
    assert_eq!(first, calculate());
    assert!(first.characters[0].assist_damage > 0);
    for hit in first.targets.iter().flat_map(|target| &target.hits) {
        let attributed = hit.credits.iter().map(|credit| credit.amount).sum::<u32>();
        assert_eq!(hit.amount, attributed);
    }
    assert!(
        first
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == "character.1008"
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == "character.1106" && credit.amount > 0)
            })
    );
}

#[test]
fn replay_keeps_the_debuff_damage_and_energy_sequence_with_or_without_logging() {
    let plan = prepared(&[pela(6, &[A2])], 42);
    let commands: Vec<_> = [
        (Action::Skill, Some("dpr:2")),
        (Action::Skill, Some("dpr:3")),
        (Action::Ultimate, None),
        (Action::Basic, Some("dpr:1")),
    ]
    .into_iter()
    .enumerate()
    .map(|(decision_index, (action, target))| Command {
        decision_index: decision_index as u32,
        actor_id: "character.1106".into(),
        action,
        target_id: target.map(str::to_owned),
    })
    .collect();
    let replay = |mode| Battle::replay(&plan, &commands, mode).unwrap();
    let detailed = replay(LogMode::Detailed);
    assert_eq!(detailed.events(), replay(LogMode::Detailed).events());
    assert_eq!(detailed.snapshot(), replay(LogMode::Off).snapshot());
    assert_eq!(detailed.snapshot().units[0].energy_milli, 35_000);
    assert_eq!(detailed.snapshot().skill_points, 2);
}
