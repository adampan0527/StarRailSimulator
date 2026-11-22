use super::{
    BASIC, CRUSH_FIGHTING_WILL, CYCLE_BRAKING, ENHANCED_BASIC, FORM, KINETIC_OVERLOAD, SKILL,
    TALENT, ULTIMATE,
};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{
        periodic::{DotScale, SkillDot},
        setup::Kit,
    },
    calculate_dpr,
    fixture::{self, DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

fn luka(eidolon: u8, idle_speed: u64, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Physical,
        },
        level: 80,
        energy_capacity: 130,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 600,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: idle_speed,
            speed_micros: idle_speed,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.6, 1.0, 0.24, 1.3, 3.0]),
            (ULTIMATE.into(), vec![1.98, 1.0, 0.12, 3.0, 2.0]),
            (TALENT.into(), vec![1.0, 0.68]),
            (ENHANCED_BASIC.into(), vec![0.1, 0.4]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn prepared(eidolon: u8, traces: &[&str]) -> PreparedBattle {
    PreparedBattle::official_dpr(&[luka(eidolon, 200_000_000, traces)], 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn apply(battle: &mut Battle<'_>, action: Action) {
    apply_to(battle, action, "dpr:2");
}

fn apply_to(battle: &mut Battle<'_>, action: Action, target: &str) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == Some(target))
        .expect("Luka can select the requested target");
    battle.apply(&command).unwrap();
}

#[test]
fn ordinary_basic_grants_fighting_will_and_enhanced_basic_spends_two_without_refund() {
    let plan = prepared(0, &[CYCLE_BRAKING]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].luka_state.fighting_will, 1);
    assert_eq!(battle.snapshot().units[0].energy_milli, 65_000);

    apply(&mut battle, Action::Basic);
    assert_eq!(battle.units[0].luka_state.fighting_will, 2);
    assert_eq!(battle.snapshot().units[0].energy_milli, 88_000);
    assert_eq!(battle.snapshot().skill_points, 4);
    assert_eq!(battle.snapshot().units[2].toughness, 990);

    apply(&mut battle, Action::Basic);
    assert_eq!(battle.units[0].luka_state.fighting_will, 0);
    assert_eq!(battle.snapshot().units[0].energy_milli, 108_000);
    assert_eq!(battle.snapshot().skill_points, 5);
    assert_eq!(battle.snapshot().units[2].toughness, 970);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [300, 60, 60, 60, 240]
    );
}

#[test]
fn combat_skill_bleed_scales_with_target_hp_and_refreshes_without_stacking() {
    for (target_hp, expected) in [(1_000, 144), (100_000, 780)] {
        let mut plan = prepared(0, &[]);
        plan.units[2].stats.hp = target_hp;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply(&mut battle, Action::Skill);
        assert!(battle.has_elemental_dot(2, Element::Physical));
        assert_eq!(battle.snapshot().skill_points, 2);

        apply(&mut battle, Action::Skill);
        assert_eq!(battle.snapshot().units[2].normal_turns, 1);
        let ticks = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::DamageOverTime)
            .map(|hit| (hit.target, hit.amount, hit.action, hit.critical))
            .collect::<Vec<_>>();
        // Unbroken target's damage factor is 0.9 × 1000 / (1000 + 500).
        // With 1000 ATK the cap is 1300 base damage.
        assert_eq!(ticks, [(2, expected, Action::Skill, false)]);
    }
}

#[test]
fn e2_awards_extra_fighting_will_only_against_physical_weakness() {
    static NO_PHYSICAL_WEAKNESS: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 1_000,
        weakness: [false; DUMMY_STATS.resist_bp.len()],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    for (weak, expected_will, expected_energy) in [(true, 3, 101_000), (false, 2, 98_000)] {
        let mut plan = prepared(2, &[CYCLE_BRAKING]);
        if !weak {
            plan.units[2].kit = Kit::Enemy(&NO_PHYSICAL_WEAKNESS);
        }
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply(&mut battle, Action::Skill);
        assert_eq!(battle.units[0].luka_state.fighting_will, expected_will);
        assert_eq!(battle.snapshot().units[0].energy_milli, expected_energy);
    }
}

#[test]
fn ultimate_applies_vulnerability_before_damage_and_capped_will_cannot_grant_energy() {
    let plan = prepared(4, &[CYCLE_BRAKING]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill);
    apply(&mut battle, Action::Skill);
    let state = &battle.units[0].luka_state;
    assert_eq!((state.fighting_will, state.attack_stacks), (4, 3));
    assert_eq!(battle.snapshot().units[0].energy_milli, 130_000);
    battle.dummy_damage.clear();

    apply(&mut battle, Action::Ultimate);
    let state = &battle.units[0].luka_state;
    assert_eq!((state.fighting_will, state.attack_stacks), (4, 3));
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    assert_eq!(battle.snapshot().units[2].vulnerability_bp, 1_200);
    // 1090 ATK includes 3 × 5% of 600 base ATK. E1 and the ultimate
    // vulnerability both apply before the 198% direct hit.
    assert_eq!(battle.dummy_damage[0].amount, 1_667);
}

#[test]
fn extra_jabs_roll_once_per_original_jab_and_e6_detonates_after_the_uppercut() {
    let plan = prepared(6, &[CRUSH_FIGHTING_WILL]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].luka_state.fighting_will = 2;
    assert!(battle.apply_skill_dot(
        0,
        2,
        10_000,
        SkillDot {
            element: Element::Physical,
            damage: DotScale::TargetHp {
                hp_bp: 2_400,
                attack_cap_bp: 13_000
            },
            origin: Action::Skill,
            turns: 3,
            max_stacks: 1,
        },
    ));
    apply(&mut battle, Action::Basic);

    // Seed 42 yields one failed and two successful judgements. E1 is
    // already active because the target was bleeding at action start.
    assert_eq!(battle.snapshot().random_draws, 3);
    assert_eq!(battle.snapshot().units[2].toughness, 980);
    let hits = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.amount, hit.action))
        .collect::<Vec<_>>();
    let mut expected = vec![(DamageKind::Direct, 69, Action::Basic); 5];
    expected.push((DamageKind::Direct, 276, Action::Basic));
    expected.push((DamageKind::DamageOverTime, 609, Action::Basic));
    expected.extend([(DamageKind::DamageOverTime, 71, Action::Basic); 5]);
    assert_eq!(hits, expected);
    assert!(battle.has_elemental_dot(2, Element::Physical));
}

#[test]
fn enhanced_basic_detonates_both_skill_and_its_new_break_bleed() {
    let plan = PreparedBattle::official_dpr(&[luka(0, 200_000_000, &[])], 42, 3)
        .unwrap()
        .with_dummies(&[30; 3])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill);
    let enhanced_at = battle.snapshot().av_micros;
    apply(&mut battle, Action::Basic);

    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.action == Action::Basic && hit.av_micros == enhanced_at)
        .map(|hit| (hit.kind, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 7);
    assert_eq!(hits[4].0, DamageKind::WeaknessBreak);
    assert_eq!(hits[5].0, DamageKind::DamageOverTime);
    assert_eq!(hits[6].0, DamageKind::DamageOverTime);
    assert!(hits[5].1 > 0 && hits[6].1 > 0 && hits[5].1 != hits[6].1);
    assert!(battle.has_elemental_dot(2, Element::Physical));
}

#[test]
fn dispel_trace_removes_an_enemy_buff_before_skill_damage() {
    let mut plan = prepared(0, &[KINETIC_OVERLOAD]);
    plan.units[2].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_speed_up(2, 2, 2_000, 3);
    apply(&mut battle, Action::Skill);
    let removed = battle
        .events()
        .iter()
        .position(|event| event.kind == EventKind::StatusExpired(StatusKind::SpeedUp));
    let damaged = battle.events().iter().position(|event| {
        matches!(
            event.kind,
            EventKind::Damage {
                kind: DamageKind::Direct,
                amount: 360,
                ..
            }
        )
    });
    assert!(
        removed
            .zip(damaged)
            .is_some_and(|(removed, damaged)| removed < damaged)
    );
    assert_eq!(battle.snapshot().units[2].speed_micros, 100_000_000);
}

#[test]
fn e1_bonus_persists_for_two_following_luka_turns_and_expires() {
    let plan = prepared(1, &[]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, Action::Skill);
    apply(&mut battle, Action::Basic);

    battle.dummy_damage.clear();
    for _ in 0..3 {
        apply_to(&mut battle, Action::Basic, "dpr:1");
    }
    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.target == 1 && hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect::<Vec<_>>();
    assert_eq!(hits, [345, 345, 60, 60, 60, 240]);
}

#[test]
fn fixed_dpr_prioritizes_active_enhanced_basic_and_is_reproducible() {
    let plan = PreparedBattle::official_dpr(&[luka(0, 103_000_000, &[])], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let first = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let second = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(first, second);
    assert!(
        first.targets[1]
            .hits
            .iter()
            .any(|hit| { hit.kind == DamageKind::DamageOverTime && hit.action == Action::Basic })
    );
}
