use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const MIDDLE: &str = "dpr:2";
const LEFT: &str = "dpr:1";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

pub(in crate::battle::official) fn saber(
    eidolon: u8,
    traces: &[&str],
    low_level: bool,
) -> OfficialCharacter {
    let [
        basic,
        skill,
        neighbor,
        resonance,
        ultimate,
        bounce,
        talent,
        enhanced,
        single,
    ] = if low_level {
        [0.5, 0.825, 0.4125, 0.077, 1.54, 0.605, 0.33, 0.75, 1.1]
    } else {
        [1.0, 1.5, 0.75, 0.14, 2.8, 1.1, 0.6, 1.5, 2.2]
    };
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: WIND,
        },
        energy_capacity: 360,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 101_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 0,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![basic]),
            (SKILL.into(), vec![skill, neighbor, 3.0, resonance]),
            (ULTIMATE.into(), vec![ultimate, bounce, 10.0]),
            (ENHANCED_BASIC.into(), vec![enhanced, 2.0, enhanced, single]),
            (
                TALENT.into(),
                vec![3.0, 25.0, talent, 2.0, 8.0, 0.1, 99.0, 1.0],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn prepared(character: OfficialCharacter, enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(&[character], 42, 3)
        .unwrap()
        .with_dummies(&[400; 3][..enemies])
        .unwrap()
}

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) -> Command {
    let command = command(battle, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
    command
}

fn direct_hits<'a>(
    battle: &'a Battle<'_>,
    action: Action,
) -> impl Iterator<Item = &'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(move |hit| hit.kind == DamageKind::Direct && hit.action == action)
}

#[test]
fn skill_uses_own_regeneration_to_test_threshold_but_pays_resonance_flat() {
    let mut input = saber(0, &[], true);
    input.stats.energy_regeneration_bp = 15_000;
    let mut plan = prepared(input, 3);
    for (energy, paid, expected) in [
        (306_999, false, [495, 247, 247]),
        (307_000, true, [541, 293, 293]),
    ] {
        plan.units[0].starting_energy_milli = energy;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Skill, Some(MIDDLE));
        assert_eq!(
            direct_hits(&battle, Action::Skill)
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            battle.units[0].energy_milli,
            if paid { 360_000 } else { 351_999 }
        );
        assert_eq!(
            battle.units[0].saber_state.resonance,
            if paid { 0 } else { 4 }
        );
        assert_eq!(battle.skill_points, 2);
        assert_eq!(battle.units[1].toughness, 390);
        assert_eq!(battle.units[2].toughness, 380);
        assert_eq!(battle.units[3].toughness, 390);
        assert_eq!(
            battle
                .legal_actions()
                .iter()
                .any(|entry| entry.action == Action::Ultimate),
            paid
        );
    }
}

#[test]
fn ultimate_records_ten_seeded_bounces_and_the_enhanced_basic_replaces_skill() {
    let mut plan = prepared(saber(0, &[], false), 3);
    plan.units[0].starting_energy_milli = 360_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let commands = [act(&mut battle, Action::Ultimate, None), {
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.units[0].saber_state.resonance, 4);
        let hits: Vec<_> = direct_hits(&battle, Action::Ultimate)
            .map(|hit| hit.amount)
            .collect();
        assert_eq!(&hits[..3], &[2_688; 3]);
        assert_eq!(&hits[3..], &[1_056; 10]);
        assert_eq!(
            battle
                .enemies()
                .map(|enemy| battle.units[enemy].toughness)
                .sum::<u16>(),
            1_060
        );
        assert_eq!(
            battle.legal_actions(),
            [command(&battle, Action::Basic, None)]
        );
        act(&mut battle, Action::Basic, None)
    }];
    assert_eq!(battle.units[0].energy_milli, 35_000);
    assert_eq!(battle.units[0].saber_state.resonance, 6);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(direct_hits(&battle, Action::Basic).count(), 3);
    assert!(
        battle
            .legal_actions()
            .contains(&command(&battle, Action::Skill, Some(LEFT)))
    );
    let replay = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
    assert_eq!(battle.snapshot(), replay.snapshot());
    assert_eq!(battle.events(), replay.events());
    assert!(battle.events().iter().any(|event| matches!(
        event.kind,
        EventKind::SaberCoreResonance {
            before: 4,
            after: 6
        }
    )));
}

#[test]
fn enhanced_basic_extra_hits_depend_on_target_count_when_cast() {
    for (enemies, expected) in [
        (1, vec![1_440, 2_112]),
        (2, vec![1_440; 4]),
        (3, vec![1_440; 3]),
    ] {
        let mut plan = prepared(saber(0, &[], false), enemies);
        plan.units[0].starting_energy_milli = 360_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Ultimate, None);
        let before = battle
            .enemies()
            .map(|enemy| battle.units[enemy].toughness)
            .sum::<u16>();
        act(&mut battle, Action::Basic, None);
        assert_eq!(
            direct_hits(&battle, Action::Basic)
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected
        );
        let after = battle
            .enemies()
            .map(|enemy| battle.units[enemy].toughness)
            .sum::<u16>();
        assert_eq!(before - after, 20 * enemies as u16);
    }
}

#[test]
fn finite_targets_removed_by_the_initial_hit_are_skipped_by_random_bounces() {
    let mut plan = prepared(saber(0, &[], false), 3);
    plan.units[0].starting_energy_milli = 360_000;
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&FINITE);
    }
    plan.units[1].stats.hp = 100;
    plan.units[2].stats.hp = 50_000;
    plan.units[3].stats.hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, None);
    let targets: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::Direct,
                    ..
                }
            )
        })
        .map(|event| event.target_id.as_deref().unwrap())
        .collect();
    assert_eq!(&targets[..3], &[LEFT, MIDDLE, "dpr:3"]);
    assert_eq!(&targets[3..], &[MIDDLE; 10]);
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(battle.units[3].hp, 0);
}

#[test]
fn allied_ultimate_consumes_mana_burst_to_restore_points_and_advance_once() {
    let mut ally = saber(0, &[], false);
    ally.unit_id = "support".into();
    ally.form_id = "character.1302".into();
    ally.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Physical,
    };
    ally.energy_capacity = 180;
    ally.stats.speed_micros = 300_000_000;
    ally.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    let input = saber(0, &[KNIGHT_OF_THE_DRAGON], false);
    let mut plan = PreparedBattle::official_dpr(&[input, ally], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 298_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some("support"));
    let av = battle.av_micros;
    battle
        .apply(&Command {
            decision_index: battle.decision_index,
            actor_id: "support".into(),
            action: Action::Ultimate,
            target_id: None,
        })
        .unwrap();
    assert_eq!(battle.units[0].saber_state.resonance, 4);
    assert!(!battle.units[0].saber_state.mana_burst);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 298_000);
    battle
        .apply(&Command {
            decision_index: battle.decision_index,
            actor_id: "support".into(),
            action: Action::Basic,
            target_id: Some(MIDDLE.into()),
        })
        .unwrap();
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, av);
    act(&mut battle, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 360_000);
    assert_eq!(battle.units[0].saber_state.resonance, 0);
    assert_eq!(battle.skill_points, 4);
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.units[0].saber_state.resonance, 3);
    assert!(!battle.units[0].saber_state.mana_burst);
    if battle.ordinary_actor() == Some("support") {
        battle
            .apply(&Command {
                decision_index: battle.decision_index,
                actor_id: "support".into(),
                action: Action::Basic,
                target_id: Some(MIDDLE.into()),
            })
            .unwrap();
    }
    act(&mut battle, Action::Basic, None);
    assert!(battle.units[0].saber_state.mana_burst);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| {
                event.source_id.as_deref() == Some(FORM)
                    && matches!(
                        event.kind,
                        EventKind::GaugeShifted {
                            after_quanta: 0,
                            ..
                        }
                    )
            })
            .count(),
        1
    );
}

#[test]
fn lake_stores_capped_definite_overflow_but_direct_fill_only_adds_missing_energy() {
    let mut input = saber(0, &[BLESSING_OF_THE_LAKE], false);
    input.stats.energy_regeneration_bp = 15_000;
    let plan = prepared(input, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 216_000);
    battle.fill_energy(0);
    assert_eq!(battle.units[0].energy_milli, 360_000);
    assert_eq!(battle.units[0].saber_state.stored_energy_milli, 0);
    battle.gain_energy(0, 0, 30_000);
    assert_eq!(battle.units[0].saber_state.stored_energy_milli, 45_000);
    battle.restore_energy_flat(0, 0, 100_000);
    assert_eq!(battle.units[0].saber_state.stored_energy_milli, 120_000);
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.units[0].saber_state.stored_energy_milli, 0);
    assert_eq!(battle.units[0].energy_milli, 127_500);
}

#[test]
fn eidolons_and_crown_of_the_star_keep_cumulative_bonuses_after_payment() {
    let mut input = saber(4, &[CROWN_OF_THE_STAR], false);
    input.stats.crit_rate_bp = 10_000;
    let mut plan = prepared(input, 3);
    plan.units[0].starting_energy_milli = 360_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.resistance_penetration_bp(&battle, 0, WIND), 800);
    assert_eq!(
        rules.resistance_penetration_bp(&battle, 0, Element::Physical),
        0
    );
    add_resonance(&mut battle, 0, 100);
    assert_eq!(battle.units[0].saber_state.resonance, 99);
    assert_eq!(rules.defense_ignore_bp(&battle, 0, 1), 1_500);
    assert_eq!(
        rules.crit_damage_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
        3_200
    );
    act(&mut battle, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.units[0].saber_state.resonance, 1);
    assert_eq!(rules.defense_ignore_bp(&battle, 0, 1), 1_500);
    assert!(direct_hits(&battle, Action::Skill).all(|hit| hit.amount > 20_000));
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage),
        5_000
    );
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 6_000);
    assert_eq!(
        rules.damage_bonus_bp(&battle, 0, 1, Some(Action::Ultimate), DamageKind::Direct),
        6_000
    );
    assert_eq!(rules.resistance_penetration_bp(&battle, 0, WIND), 1_200);
    act(&mut battle, Action::Basic, None);
    assert_eq!(battle.units[0].saber_state.resonance, 7);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage), 0);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 6_000);
    act(&mut battle, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 0);
}

#[test]
fn six_eidolons_extend_overflow_and_refund_every_third_cast_after_the_first() {
    let plan = prepared(saber(6, &[BLESSING_OF_THE_LAKE], false), 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .ultimate_resistance_penetration_bp(),
        2_000
    );
    battle.fill_energy(0);
    battle.restore_energy_flat(0, 0, 240_000);
    assert_eq!(battle.units[0].saber_state.stored_energy_milli, 200_000);
    for (current, energy, overflow) in [
        (1, 360_000, 145_000),
        (2, 150_000, 0),
        (3, 5_000, 0),
        (4, 305_000, 0),
    ] {
        battle.fill_energy(0);
        act(&mut battle, Action::Ultimate, None);
        assert_eq!(battle.units[0].energy_milli, energy, "cast {current}");
        assert_eq!(
            battle.units[0].saber_state.stored_energy_milli, overflow,
            "cast {current}"
        );
    }
    assert_eq!(battle.units[0].saber_state.wind_penetration_stacks, 3);
}

#[test]
fn fixed_dpr_uses_the_same_kit_and_attributes_damage_to_saber() {
    let input = saber(
        0,
        &[
            KNIGHT_OF_THE_DRAGON,
            BLESSING_OF_THE_LAKE,
            CROWN_OF_THE_STAR,
        ],
        false,
    );
    let plan = prepared(input, 3);
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.action == Action::Skill));
    assert!(hits().any(|hit| hit.action == Action::Ultimate));
    assert!(hits().any(|hit| hit.action == Action::Basic));
    assert!(hits().all(|hit| hit.source_id == FORM));
    assert_eq!(result.total_damage, result.characters[0].own_damage);
    assert_eq!(result, run());
}
