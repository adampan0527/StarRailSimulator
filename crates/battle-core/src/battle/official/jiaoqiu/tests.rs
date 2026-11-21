use std::{collections::BTreeMap, num::NonZeroU32};

use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, StatusKind, calculate_dpr,
};

use super::{
    BASIC, FORM, HEARTH_KINDLE, PYRE_CLEANSE, SEARED_SCENT, SKILL, TALENT, ULTIMATE, field_active,
};
use crate::battle::Kit;
use crate::fixture::{self, Element};

fn jiaoqiu(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "team.jiaoqiu".into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Fire,
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
            (SKILL.into(), vec![0.75, 0.45, 1.0]),
            (ULTIMATE.into(), vec![0.6, 0.5, 0.09, 3.0, 6.0]),
            (TALENT.into(), vec![1.0, 0.075, 0.025, 5.0, 2.0, 0.9]),
        ]),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = jiaoqiu(0, &[]);
    ally.unit_id = "team.arlan".into();
    ally.form_id = "character.1008".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    ally.energy_capacity = 110;
    ally.skill_parameters = BTreeMap::from([
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]);
    ally
}

fn prepared(team: &[OfficialCharacter], enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 0, 3)
        .unwrap()
        .with_dummies(&vec![1_000; enemies])
        .unwrap()
}

fn apply(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn jiaoqiu_action(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    apply(battle, "team.jiaoqiu", action, target);
}

#[test]
fn skill_hits_the_blast_slots_and_sets_a_single_burn_with_separate_roast_stacks() {
    let plan = prepared(&[jiaoqiu(0, &[PYRE_CLEANSE])], 4);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].energy_milli, 65_000);
    jiaoqiu_action(&mut battle, Action::Skill, Some("dpr:2"));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[0].energy_milli, 95_000);
    assert_eq!(snapshot.skill_points, 2);
    assert_eq!(
        snapshot.units[1..]
            .iter()
            .map(|unit| (unit.toughness, unit.vulnerability_bp))
            .collect::<Vec<_>>(),
        [(990, 750), (980, 1_000), (990, 750), (1_000, 0)]
    );
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.kind, hit.amount, hit.critical))
        .collect();
    // 1,000 ATK × [0.75, 0.45] × 0.6. DoT is 900 × 0.6 × (1 + vulnerability).
    assert_eq!(
        hits,
        [
            (2, DamageKind::Direct, 450, false),
            (1, DamageKind::Direct, 270, false),
            (3, DamageKind::Direct, 270, false),
            (1, DamageKind::DamageOverTime, 580, false),
            (2, DamageKind::DamageOverTime, 594, false),
            (3, DamageKind::DamageOverTime, 580, false),
        ]
    );
    assert!((1..=3).all(|enemy| battle.dot_stacks(enemy, Element::Fire) == 1));
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:4"));
    assert_eq!(battle.dot_stacks(2, Element::Fire), 0);
    assert_eq!(battle.vulnerability_from(0, 2), 0);
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.snapshot().units[4].toughness, 990);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 2 && hit.kind == DamageKind::DamageOverTime)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [594, 594]
    );
}

#[test]
fn e1_only_doubles_talent_stacks_and_e2_adds_300_points_to_the_burn() {
    for (eidolon, amount) in [(1, 850), (2, 3_685)] {
        let plan = prepared(&[jiaoqiu(eidolon, &[])], 1);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        jiaoqiu_action(&mut battle, Action::Skill, Some("dpr:1"));
        assert_eq!(battle.vulnerability_from(0, 1), 1_250);
        // Skill stack + two Talent stacks. The 40% damage bonus applies after the hit.
        // A single 900/3,900 ATK-based burn = multiplier × 0.6 × 1.125 × 1.4.
        assert_eq!(battle.dummy_damage[0].amount, 450);
        assert_eq!(battle.dummy_damage[1].amount, amount);
        assert_eq!(battle.dummy_damage[1].kind, DamageKind::DamageOverTime);
    }
}

#[test]
fn ultimate_equalizes_before_damage_and_only_ultimate_hits_gain_zone_vulnerability() {
    let mut character = jiaoqiu(1, &[]);
    character.stats.effect_hit_rate_bp = 10_000;
    let plan = prepared(&[character, arlan()], 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    jiaoqiu_action(&mut battle, Action::Skill, Some("dpr:1"));
    battle.set_energy(0, 0, 100_000);
    jiaoqiu_action(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    assert_eq!(battle.vulnerability_from(0, 2), 1_750);
    let snapshot = battle.snapshot();
    assert_eq!(
        (snapshot.units[2].toughness, snapshot.units[3].toughness),
        (960, 970)
    );
    // Equalized to three stacks (12.5%) before both 60%-ATK hits. The field
    // adds 9% vulnerability, and E1 adds 40% DMG to a target with Roast.
    assert_eq!(
        battle.dummy_damage[2..]
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [612, 612]
    );

    battle.set_energy(1, 1, 110_000);
    apply(&mut battle, "team.arlan", Action::Ultimate, Some("dpr:1"));
    let ally_ultimate = &battle.dummy_damage[4];
    assert_eq!(
        (ally_ultimate.amount, ally_ultimate.assists.as_slice()),
        (2_040, &[(0, 888)][..])
    );
    apply(&mut battle, "team.arlan", Action::Basic, Some("dpr:1"));
    let ally_basic = &battle.dummy_damage[6];
    assert_eq!(
        (ally_basic.amount, ally_basic.assists.as_slice()),
        (493, &[(0, 193)][..])
    );
    // 900 × 0.6 × 1.175 × 1.4 = 888.3. An Ultimate origin on the DoT
    // does not add the 9% Ultimate vulnerability.
    assert_eq!(
        battle.dummy_damage[7..]
            .iter()
            .map(|hit| (hit.action, hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [(Action::Ultimate, DamageKind::DamageOverTime, 888); 2]
    );
}

#[test]
fn zone_rolls_after_periodic_damage_and_non_acting_enemies_do_not_use_the_budget() {
    let mut character = jiaoqiu(0, &[]);
    character.stats.effect_hit_rate_bp = 10_000;
    let plan = prepared(&[character], 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 100_000);
    jiaoqiu_action(&mut battle, Action::Ultimate, None);
    assert!(battle.apply_skill_freeze(0, 1, 10_000, 2, 1_000, Action::Skill));
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:2"));

    assert_eq!(battle.units[0].jiaoqiu_field.triggers, 1);
    assert_eq!(battle.vulnerability_from(0, 1), 750);
    assert_eq!(battle.vulnerability_from(0, 2), 1_250);
    // Target 2 ticks with two stacks before the enemy action adds its third.
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::DamageOverTime)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(1, 580), (2, 594)]
    );
}

#[test]
fn zone_caps_at_six_successes_and_recasting_restarts_the_budget() {
    let mut character = jiaoqiu(0, &[]);
    character.stats.effect_hit_rate_bp = 10_000;
    let mut plan = prepared(&[character], 5);
    plan.units[1].stats.effect_resistance_bp = 10_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 100_000);
    jiaoqiu_action(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.vulnerability_from(0, 1), 0);
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].jiaoqiu_field.triggers, 4);
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].jiaoqiu_field.triggers, 6);
    assert_eq!(
        (1..=5)
            .map(|target| battle.vulnerability_from(0, target))
            .collect::<Vec<_>>(),
        [0, 1_250, 1_250, 1_000, 1_000]
    );
    battle.set_energy(0, 0, 100_000);
    jiaoqiu_action(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.units[0].jiaoqiu_field.triggers, 0);
    // Equalization is a direct setting and succeeds on the fully resistant target.
    assert_eq!(battle.vulnerability_from(0, 1), 1_250);
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].jiaoqiu_field.triggers, 4);
}

#[test]
fn e4_reduces_base_attack_until_the_third_owner_turn_or_defeat() {
    let mut plan = prepared(&[jiaoqiu(4, &[])], 1);
    plan.units[1].stats.base_attack = 800;
    plan.units[1].stats.attack = 1_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.effective_attack(1), 1_000);
    battle.set_energy(0, 0, 100_000);
    jiaoqiu_action(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.effective_attack(1), 880);
    let mut defeated = battle.clone();
    defeated.units[0].hp = 0;
    assert!(!field_active(&defeated, 0));
    assert_eq!(defeated.effective_attack(1), 1_000);
    for _ in 0..2 {
        jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
        assert!(field_active(&battle, 0));
        assert_eq!(battle.effective_attack(1), 880);
    }
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
    assert!(!field_active(&battle, 0));
    assert_eq!(battle.effective_attack(1), 1_000);
}

#[test]
fn a4_turns_full_effect_hit_rate_steps_into_capped_base_attack_bonus() {
    for (effect_hit_rate, expected) in [(9_499, 300), (9_500, 408), (15_500, 732)] {
        let mut character = jiaoqiu(0, &[HEARTH_KINDLE]);
        character.stats.base_attack = 600;
        character.stats.effect_hit_rate_bp = effect_hit_rate;
        let plan = prepared(&[character], 1);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.dummy_damage[0].amount, expected);
    }
}

#[test]
fn a6_inherits_the_zones_record_even_after_the_last_holder_dies() {
    for (traces, prior_vulnerability, expected_vulnerability) in [
        (&[SEARED_SCENT][..], 1_750, 1_750),
        (&[SEARED_SCENT][..], 0, 750),
        (&[][..], 1_750, 0),
    ] {
        let mut plan = prepared(&[jiaoqiu(0, traces)], 3);
        plan.wave_ends = vec![2, 4];
        plan.units[1].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        plan.units[1].stats.hp = 300;
        plan.units[2].stats.effect_resistance_bp = 10_000;
        plan.units[3].stats.effect_resistance_bp = 10_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if prior_vulnerability > 0 {
            battle.apply_vulnerability(0, 1, prior_vulnerability, 2);
        }
        battle.set_energy(0, 0, 100_000);
        jiaoqiu_action(&mut battle, Action::Ultimate, None);
        assert_eq!(battle.wave_index, 1);
        assert_eq!(battle.units[0].jiaoqiu_field.triggers, 0);
        assert!(
            (2..=3).all(|target| battle.vulnerability_from(0, target) == expected_vulnerability)
        );
        assert!((2..=3).all(|target| {
            battle.has_elemental_dot(target, Element::Fire) == (expected_vulnerability > 0)
        }));
        let wave_events: Vec<_> = battle
            .events()
            .iter()
            .filter_map(|event| match event.kind {
                EventKind::WaveStarted => Some(true),
                EventKind::StatusApplied(StatusKind::Vulnerability)
                    if event.target_id.as_deref() == Some("dpr:2") =>
                {
                    Some(false)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            wave_events,
            if expected_vulnerability > 0 {
                vec![true, true, false]
            } else {
                vec![true, true]
            }
        );
    }
}

#[test]
fn e6_transfers_to_the_first_lowest_stack_enemy_and_only_applies_one_burn() {
    let mut plan = prepared(&[jiaoqiu(6, &[])], 3);
    plan.units[1].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
    plan.units[1].stats.hp = 1;
    plan.units[2].stats.effect_resistance_bp = 10_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_vulnerability(0, 1, 2_500, 2);
    battle.apply_vulnerability(0, 2, 750, 2);
    battle.apply_vulnerability(0, 3, 750, 2);
    jiaoqiu_action(&mut battle, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.hp(1).0, 0);
    assert_eq!(battle.vulnerability_from(0, 2), 2_750);
    assert_eq!(battle.vulnerability_from(0, 3), 750);
    assert_eq!(battle.dot_stacks(2, Element::Fire), 1);
    // 3,900 × 0.6 × 1.4 × 1.275 vulnerability × 1.27 resistance.
    assert_eq!(
        (battle.dummy_damage[0].target, battle.dummy_damage[0].amount),
        (2, 5_304)
    );
}

#[test]
fn fixed_seed_replays_the_full_dpr_with_conserved_ally_damage_credit() {
    let plan = prepared(&[jiaoqiu(6, &[PYRE_CLEANSE, HEARTH_KINDLE]), arlan()], 3);
    let cycles = NonZeroU32::new(3).unwrap();
    let run = || calculate_dpr(&plan, &[30, 120, 30], cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert!(result.characters[0].assist_damage > 0);
    assert!(result.characters[0].own_damage > 0);
    for hit in result.targets.iter().flat_map(|target| &target.hits) {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
}
