use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO,
    battle::{
        Kit,
        periodic::{DotScale, SkillDot},
        resolve::CounterAttack,
    },
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::{BASE, FORM, NOVAFLARE, OfficialCharacter, THUNDER};
use crate::fixture::Element;

const KAFKA: &str = "team.kafka";
const ARLAN: &str = "team.arlan";
const SERVAL: &str = "team.serval";

fn kafka(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let ids = if enhanced { NOVAFLARE } else { BASE };
    let mut skill = vec![1.6, 0.75, 0.6];
    let mut talent = vec![1.4, 1.0, 2.0];
    if enhanced {
        skill.push(0.5);
        talent.extend([1.0, 2.0]);
    }
    OfficialCharacter {
        unit_id: KAFKA.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Thunder,
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
            base_speed_micros: 100_000_000,
            speed_micros: 100_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (ids.basic.into(), vec![1.0]),
            (ids.skill.into(), skill),
            (
                ids.ultimate.into(),
                vec![0.8, 1.0, 2.0, 2.9, if enhanced { 1.2 } else { 1.0 }],
            ),
            (ids.talent.into(), talent),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|trace| (*trace).into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut input = kafka(false, 0, &[]);
    input.unit_id = ARLAN.into();
    input.form_id = "character.1008".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    input.energy_capacity = 110;
    input.stats.speed_micros = 200_000_000;
    input.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    input
}

fn serval() -> OfficialCharacter {
    let mut input = kafka(false, 0, &[]);
    input.unit_id = SERVAL.into();
    input.form_id = "character.1103".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Erudition,
        element: crate::Element::Thunder,
    };
    input.energy_capacity = 100;
    input.stats.speed_micros = 180_000_000;
    input.skill_parameters = [
        ("skill.110301".into(), vec![0.5]),
        ("skill.110302".into(), vec![0.7, 0.3, 0.8, 2.0, 0.4]),
        ("skill.110303".into(), vec![1.08, 2.0]),
        ("skill.110304".into(), vec![0.36]),
    ]
    .into();
    input
}

fn pela() -> OfficialCharacter {
    let mut input = kafka(false, 0, &[]);
    input.unit_id = "team.pela".into();
    input.form_id = "character.1106".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Nihility,
        element: crate::Element::Ice,
    };
    input.energy_capacity = 110;
    input.skill_parameters = [
        ("skill.110601".into(), vec![0.5]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.3, 2.0, 0.6]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    input.enabled_trace_ids = vec!["trace.1106102".into()];
    input
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 0, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
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

fn seed_fire(battle: &mut Battle<'_>, source: usize, target: usize) {
    assert!(battle.apply_skill_dot(
        source,
        target,
        10_000,
        SkillDot {
            element: Element::Fire,
            damage: DotScale::Attack(10_000),
            origin: Action::Skill,
            turns: 2,
            max_stacks: 1,
        },
    ));
}

fn damage_at(battle: &Battle<'_>, kind: DamageKind, time: u64) -> Vec<(usize, usize, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == kind && hit.av_micros == time && !hit.critical)
        .map(|hit| (hit.actor, hit.target, hit.amount))
        .collect()
}

fn ally_counter(battle: &mut Battle<'_>, target: usize) {
    battle.counter_attack(
        1,
        target,
        CounterAttack {
            element: Element::Physical,
            multiplier_bp: 5_000,
            toughness: 0,
            defense_multiplier_bp: 0,
            splash: None,
            energy_milli: 0,
        },
    );
}

#[test]
fn mixed_or_incomplete_skill_variants_are_rejected() {
    let mut input = kafka(false, 0, &[]);
    input
        .skill_parameters
        .insert(NOVAFLARE.basic.into(), vec![1.0]);
    assert!(PreparedBattle::official_dpr(&[input], 0, 3).is_err());

    let mut input = kafka(true, 0, &[]);
    input.skill_parameters.remove(NOVAFLARE.talent);
    assert!(PreparedBattle::official_dpr(&[input], 0, 3).is_err());
}

#[test]
fn skill_detonates_all_elements_at_the_correct_blast_ratios() {
    for enhanced in [false, true] {
        let mut input = kafka(enhanced, 0, &[]);
        input.stats.speed_micros = 125_000_000;
        let plan = prepared(&[input, arlan()], &[100; 3]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, ARLAN, Action::Basic, Some("dpr:2"));
        for target in 2..=4 {
            seed_fire(&mut battle, 1, target);
        }

        act(&mut battle, KAFKA, Action::Skill, Some("dpr:2"));

        // 1000 ATK × DEF 2/3 × 0.9 for intact toughness.
        assert_eq!(
            damage_at(&battle, DamageKind::Direct, 80_000_000),
            [(0, 3, 960), (0, 2, 360), (0, 4, 360),]
        );
        let expected = if enhanced {
            vec![(0, 3, 1_305), (1, 3, 450), (1, 2, 300), (1, 4, 300)]
        } else {
            vec![(0, 3, 1_305), (1, 3, 450)]
        };
        assert_eq!(
            damage_at(&battle, DamageKind::DamageOverTime, 80_000_000),
            expected
        );
        assert_eq!(battle.units[0].energy_milli, 100_000);
        assert_eq!(battle.snapshot().skill_points, 3);
        assert_eq!(battle.units[2].toughness, 90);
        assert_eq!(battle.units[4].toughness, 90);
        assert!(battle.has_shock(3));
        assert!(battle.has_elemental_dot(3, Element::Fire));
    }
}

#[test]
fn ultimate_applies_shock_before_detonating_and_torture_selects_all_dots() {
    for (enhanced, traces, ratio, fire) in [
        (false, [].as_slice(), 1_740, 0),
        (false, [BASE.torture].as_slice(), 1_740, 600),
        (true, [].as_slice(), 2_088, 720),
    ] {
        let plan = prepared(&[kafka(enhanced, 0, traces)], &[100]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let time = battle.snapshot().av_micros;
        battle.units[0].energy_milli = 120_000;
        seed_fire(&mut battle, 0, 1);

        act(&mut battle, KAFKA, Action::Ultimate, None);

        assert_eq!(damage_at(&battle, DamageKind::Direct, time), [(0, 1, 480)]);
        let expected = if fire == 0 {
            vec![(0, 1, ratio)]
        } else {
            vec![(0, 1, fire), (0, 1, ratio)]
        };
        let mut actual = damage_at(&battle, DamageKind::DamageOverTime, time);
        actual.sort_unstable();
        assert_eq!(actual, expected);
        assert!(battle.has_shock(1));
        assert_eq!(battle.units[0].energy_milli, 5_000);
    }
}

#[test]
fn base_thorns_adds_thirty_points_to_the_shock_application_chance() {
    for traced in [false, true] {
        let traces = [BASE.thorns];
        let traces = if traced { &traces[..] } else { &[] };
        let mut plan = prepared(&[kafka(false, 0, traces)], &[100]);
        plan.units[1].stats.effect_resistance_bp = 3_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].energy_milli = 120_000;
        act(&mut battle, KAFKA, Action::Ultimate, None);
        // Seed 0's first roll is 88.33%. The final chances are 70% and 91%.
        assert_eq!(
            battle
                .events()
                .iter()
                .any(|event| matches!(event.kind, EventKind::ShockApplied { .. })),
            traced,
        );
    }
}

#[test]
fn base_talent_requires_an_actual_teammate_basic_attack() {
    let plan = prepared(&[kafka(false, 0, &[]), arlan(), serval()], &[100; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, ARLAN, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.units[0].kafka_follow_up_charges, 1);
    assert!(damage_at(&battle, DamageKind::FollowUp, 50_000_000).is_empty());
    ally_counter(&mut battle, 3);
    assert_eq!(battle.units[0].kafka_follow_up_charges, 1);

    act(&mut battle, SERVAL, Action::Basic, Some("dpr:3"));
    assert_eq!(battle.units[0].kafka_follow_up_charges, 0);
    assert_eq!(
        damage_at(&battle, DamageKind::FollowUp, 55_555_556),
        [(1, 3, 316), (0, 5, 840)]
    );
    assert!(battle.has_shock(5));
    assert_eq!(battle.units[0].energy_milli, 70_000);

    act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].kafka_follow_up_charges, 1);
    assert!(!battle.has_shock(3));
}

#[test]
fn novaflare_counts_ally_skills_and_counters_and_thorns_recharges_on_ultimate() {
    let plan = prepared(
        &[kafka(true, 1, &[NOVAFLARE.thorns]), arlan(), serval()],
        &[100; 3],
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    seed_fire(&mut battle, 1, 4);
    act(&mut battle, ARLAN, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.units[0].kafka_follow_up_charges, 0);
    assert_eq!(
        damage_at(&battle, DamageKind::FollowUp, 50_000_000),
        [(0, 4, 840)]
    );
    // E1 multiplies both DoTs. Arlan's 15% HP cost also raises his DoT by 5.4%.
    let mut actual = damage_at(&battle, DamageKind::DamageOverTime, 50_000_000);
    actual.sort_unstable();
    assert_eq!(actual, [(0, 4, 1_809), (1, 4, 657)]);

    battle.units[0].energy_milli = 120_000;
    act(&mut battle, KAFKA, Action::Ultimate, None);
    assert_eq!(battle.units[0].kafka_follow_up_charges, 1);
    let time = battle.snapshot().av_micros;
    ally_counter(&mut battle, 5);
    assert_eq!(
        damage_at(&battle, DamageKind::FollowUp, time),
        [(1, 5, 316), (0, 5, 840)]
    );
    assert_eq!(battle.units[0].kafka_follow_up_charges, 0);

    act(&mut battle, SERVAL, Action::Basic, Some("dpr:3"));
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].kafka_follow_up_charges, 1);
    battle.units[0].energy_milli = 120_000;
    act(&mut battle, KAFKA, Action::Ultimate, None);
    assert_eq!(battle.units[0].kafka_follow_up_charges, 2);
}

#[test]
fn novaflare_a2_uses_the_inclusive_effect_hit_rate_threshold_and_affects_self() {
    let mut input = kafka(true, 0, &[NOVAFLARE.torture]);
    input.stats.effect_hit_rate_bp = 6_500;
    input.stats.attack = 1_200;
    let mut arlan = arlan();
    arlan.stats.effect_hit_rate_bp = 6_499;
    let mut serval = serval();
    serval.stats.effect_hit_rate_bp = 6_500;
    let plan = prepared(&[input, arlan, serval, pela()], &[100]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.effective_attack(0), 2_200);
    assert_eq!(battle.effective_attack(1), 1_000);
    assert_eq!(battle.effective_attack(2), 2_000);
    battle.units[3].hp = 0;
    assert_eq!(battle.effective_attack(0), 1_200);
    assert_eq!(battle.effective_attack(2), 1_000);
}

#[test]
fn e2_aura_amplifies_allied_skill_dots_while_kafka_is_alive() {
    for (enhanced, expected) in [(false, 750), (true, 798)] {
        let plan = prepared(&[kafka(enhanced, 2, &[]), arlan()], &[100]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let time = battle.snapshot().av_micros;
        seed_fire(&mut battle, 1, 2);
        battle.tick_skill_dots(2);
        battle.units[0].hp = 0;
        battle.tick_skill_dots(2);
        assert_eq!(
            damage_at(&battle, DamageKind::DamageOverTime, time),
            [(1, 2, expected), (1, 2, 600)],
        );
        assert_eq!(battle.dummy_damage[0].assists, [(0, expected - 600)]);
        assert!(battle.dummy_damage[1].assists.is_empty());
    }
}

#[test]
fn e4_refunds_only_own_skill_shock_and_e6_extends_and_adds_multiplier_points() {
    let mut input = kafka(false, 6, &[]);
    input.stats.speed_micros = 125_000_000;
    input.stats.effect_hit_rate_bp = 10_000;
    let plan = prepared(&[input, serval()], &[100]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, SERVAL, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].energy_milli, 70_000);
    battle.apply_skill_shock(1, 2, Action::Skill, 10_000, 5_000, 2);
    battle.apply_break_shock(0, 2, Action::Skill);

    // E1 landed on the follow-up and E2 is active.
    // 1000 × (2.90 + 1.56) × 0.6 × 1.25 × 1.30 = 4348.5.
    let time = battle.snapshot().av_micros;
    battle.tick_shocks(2);
    assert_eq!(battle.units[0].energy_milli, 72_000);
    assert!(damage_at(&battle, DamageKind::DamageOverTime, time).contains(&(0, 2, 4_348)));
    battle.detonate_dots(0, 2, THUNDER, 5_000, Action::Skill);
    assert_eq!(battle.units[0].energy_milli, 74_000);
    battle.tick_shocks(2);
    assert_eq!(battle.units[0].energy_milli, 76_000);
    assert!(battle.has_shock(2));
    battle.tick_shocks(2);
    assert_eq!(battle.units[0].energy_milli, 78_000);
    assert!(!battle.has_shock(2));
}

#[test]
fn plunder_restores_energy_when_a_teammate_defeats_a_shocked_enemy() {
    static FINITE_ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 100,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    for shocked in [false, true] {
        let mut plan = prepared(&[kafka(false, 0, &[BASE.plunder]), arlan()], &[100; 2]);
        plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
        plan.units[2].stats.hp = 1;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if shocked {
            battle.apply_skill_shock(1, 2, Action::Skill, 10_000, 5_000, 2);
        }
        act(&mut battle, ARLAN, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.hp(2).0, 0);
        assert_eq!(
            battle.units[0].energy_milli,
            if shocked { 65_000 } else { 60_000 }
        );
        assert_eq!(battle.units[0].kafka_follow_up_charges, 1);
    }
}

#[test]
fn fixed_dpr_attributes_allied_detonations_without_transferring_damage_ownership() {
    let toughness = &STANDARD_DPR_SCENARIO.toughness;
    let mut serval = serval();
    serval.stats.effect_hit_rate_bp = 2_500;
    let plan = prepared(&[kafka(true, 0, &[NOVAFLARE.thorns]), serval], toughness);
    let result = calculate_dpr(&plan, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap()
    );
    let mut first_detonation = result.targets[1]
        .hits
        .iter()
        .filter(|hit| hit.av_micros == 55_555_556 && hit.kind == DamageKind::DamageOverTime)
        .map(|hit| (hit.source_id.as_str(), hit.amount))
        .collect::<Vec<_>>();
    first_detonation.sort_unstable();
    assert_eq!(first_detonation, [(KAFKA, 1_392), (SERVAL, 192)]);
    assert!(result.targets[1].hits.iter().any(|hit| {
        hit.source_id == SERVAL
            && hit.action == Action::Ultimate
            && hit.kind == DamageKind::DamageOverTime
    }));
    assert!(
        result.targets[1]
            .hits
            .iter()
            .any(|hit| { hit.source_id == KAFKA && hit.kind == DamageKind::FollowUp })
    );
    assert_eq!(
        result.total_damage,
        result
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>()
    );
}
