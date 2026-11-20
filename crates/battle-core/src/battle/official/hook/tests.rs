use crate::{
    Action, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, calculate_dpr, fixture::GAUGE_FULL,
};

use super::super::super::{
    Battle,
    periodic::{DotScale, SkillDot},
};
use super::{
    BASIC, ENHANCED_SKILL, Element, FORM, INNOCENCE, NAIVETE, PLAYING_WITH_FIRE, SKILL, TALENT,
    ULTIMATE,
};

fn hook(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.hook".into(),
        form_id: FORM.into(),
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.2, 1.0, 2.0, 0.25]),
            (ULTIMATE.into(), vec![2.4]),
            (TALENT.into(), vec![0.5, 5.0]),
            (ENHANCED_SKILL.into(), vec![1.4, 1.0, 2.0, 0.25, 0.4]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = hook(0);
    ally.unit_id = "ally.arlan".into();
    ally.form_id = "character.1008".into();
    ally.energy_capacity = 110;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn act(battle: &mut Battle<'_>, action: Action, target: &str) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == "ally.hook"
                && command.action == action
                && command.target_id.as_deref() == Some(target)
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn set_burn(battle: &mut Battle<'_>, target: usize, turns: u32) {
    assert!(battle.apply_skill_dot(
        0,
        target,
        10_000,
        SkillDot {
            element: Element::Fire,
            damage: DotScale::Attack(2_500),
            origin: Action::Skill,
            turns,
            max_stacks: 1,
        }
    ));
}

#[test]
fn skill_burns_only_its_primary_target_and_does_not_retroactively_trigger_talent() {
    let mut character = hook(4);
    character.enabled_trace_ids.push(INNOCENCE.into());
    let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.spend_hp(0, 500, 1);
    act(&mut battle, Action::Skill, "dpr:2");
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.snapshot().units[0].energy_milli, 90_000);
    assert_eq!(battle.hp(0).0, 1_500);
    assert_eq!(battle.snapshot().units[2].toughness, 100);
    assert_eq!(battle.dummy_damage.len(), 1);
    assert_eq!(battle.dummy_damage[0].kind, DamageKind::Direct);
    // 1,000 ATK × 120% × 1000/1500 DEF × 90% unbroken.
    assert_eq!(battle.dummy_damage[0].amount, 720);
    assert!(battle.has_elemental_dot(2, Element::Fire));
    assert!(!battle.has_elemental_dot(1, Element::Fire));
    assert!(!battle.has_elemental_dot(3, Element::Fire));

    act(&mut battle, Action::Basic, "dpr:2");
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.snapshot().units[0].energy_milli, 115_000);
    assert_eq!(battle.hp(0).0, 1_600);
    let talent = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::Additional)
        .unwrap();
    assert_eq!((talent.target, talent.amount), (2, 300));
    assert!(battle.has_elemental_dot(1, Element::Fire));
    assert!(battle.has_elemental_dot(3, Element::Fire));
}

#[test]
fn ultimate_charges_one_skill_and_basic_attack_does_not_spend_the_charge() {
    for (eidolon, primary_damage, adjacent_damage) in [(0, 840, 240), (1, 1_008, 288)] {
        let mut plan = PreparedBattle::official_dpr(&[hook(eidolon)], 42, 3).unwrap();
        plan.units[0].starting_energy_milli = 120_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Ultimate, "dpr:2");
        assert!(battle.units[0].charged_skill);
        assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
        assert_eq!(battle.snapshot().units[2].toughness, 90);
        act(&mut battle, Action::Basic, "dpr:1");
        assert!(battle.units[0].charged_skill);
        battle.dummy_damage.clear();

        act(&mut battle, Action::Skill, "dpr:2");
        assert!(!battle.units[0].charged_skill);
        assert_eq!(battle.snapshot().skill_points, 3);
        let direct = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>();
        assert_eq!(
            direct,
            [
                (2, primary_damage),
                (1, adjacent_damage),
                (3, adjacent_damage)
            ]
        );
        assert_eq!(battle.snapshot().units[1].toughness, 10);
        assert_eq!(battle.snapshot().units[2].toughness, 70);
        assert_eq!(battle.snapshot().units[3].toughness, 20);
        assert!(!battle.has_elemental_dot(1, Element::Fire));
        assert!(!battle.has_elemental_dot(3, Element::Fire));
        battle.dummy_damage.clear();

        act(&mut battle, Action::Skill, "dpr:2");
        let direct = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>();
        assert_eq!(direct, [(2, 720)]);
        assert_eq!(battle.snapshot().skill_points, 2);
    }
}

#[test]
fn e2_adds_a_target_turn_and_e6_boosts_the_noncritical_burn_tick() {
    for (eidolon, expected_ticks, expected_tick_amount) in [(0, 2, 150), (6, 3, 180)] {
        let mut character = hook(eidolon);
        character.stats.crit_rate_bp = 10_000;
        let plan = PreparedBattle::official_dpr(&[character], 42, 3)
            .unwrap()
            .with_dummies(&[120, 120, 120])
            .unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Skill, "dpr:2");
        while battle.snapshot().units[2].normal_turns < 4 {
            // Its high toughness lets us step turns without spreading Fire Break Burn.
            act(&mut battle, Action::Basic, "dpr:1");
        }
        let ticks = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 2 && hit.kind == DamageKind::DamageOverTime)
            .collect::<Vec<_>>();
        assert_eq!(ticks.len(), expected_ticks);
        assert!(ticks.iter().all(|hit| {
            hit.amount == expected_tick_amount && !hit.critical && hit.action == Action::Skill
        }));
        assert!(!battle.has_elemental_dot(2, Element::Fire));
    }
}

#[test]
fn enhanced_skill_uses_preexisting_burn_per_target_without_e4_chain_reactions() {
    let plan = PreparedBattle::official_dpr(&[hook(6)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].charged_skill = true;
    set_burn(&mut battle, 1, 3);
    set_burn(&mut battle, 2, 3);
    act(&mut battle, Action::Skill, "dpr:2");
    let additional = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .map(|hit| (hit.target, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(additional, [(2, 360), (1, 360)]);
    assert_eq!(battle.snapshot().units[0].energy_milli, 100_000);
    assert!(battle.has_elemental_dot(3, Element::Fire));
    assert_eq!(battle.dummy_damage[0].amount, 1_176);
    assert_eq!(battle.dummy_damage[1].amount, 336);
    assert_eq!(battle.dummy_damage[2].amount, 288);
}

#[test]
fn fire_break_is_burn_for_later_attacks_without_an_extra_hit_on_the_break_itself() {
    let mut plan = PreparedBattle::official_dpr(&[hook(0)], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, "dpr:1");
    assert!(battle.has_elemental_dot(1, Element::Fire));
    assert_eq!(battle.snapshot().units[1].toughness, 0);
    assert!(
        !battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::Additional)
    );
    act(&mut battle, Action::Basic, "dpr:1");
    assert_eq!(battle.snapshot().units[0].energy_milli, 30_000);
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.target == 1 && hit.kind == DamageKind::Additional)
    );
}

#[test]
fn playing_with_fire_advances_by_a_fifth_of_the_full_gauge_and_grants_extra_energy() {
    let mut character = hook(0);
    character.stats.base_speed_micros = 100_000_000;
    character.stats.speed_micros = 100_000_000;
    character.enabled_trace_ids = vec![PLAYING_WITH_FIRE.into(), NAIVETE.into()];
    let mut plan = PreparedBattle::official_dpr(&[character, arlan()], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor, Some(1));
    assert_eq!(battle.units[0].gauge_quanta, GAUGE_FULL / 2);
    assert_eq!(battle.mechanics(0).unwrap().control_resistance_bp(), 3_500);
    act(&mut battle, Action::Ultimate, "dpr:2");
    assert_eq!(battle.ordinary_actor, Some(1));
    assert_eq!(battle.units[0].gauge_quanta, GAUGE_FULL * 3 / 10);
    assert_eq!(battle.snapshot().units[0].energy_milli, 10_000);
    assert!(battle.events().iter().any(|event| matches!(
        event.kind,
        EventKind::GaugeShifted { .. }
    ) && event.source_id.as_deref()
        == Some("ally.hook")));
}

#[test]
fn fixed_dpr_repeats_and_attributes_skill_burns_to_hook() {
    let plan = PreparedBattle::official_dpr(&[hook(6)], 42, 3).unwrap();
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let first = run();
    assert_eq!(first, run());
    assert!(
        first
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == "ally.hook"
                    && hit.kind == DamageKind::DamageOverTime
                    && hit.action == Action::Skill
                    && !hit.critical
            })
    );
}
