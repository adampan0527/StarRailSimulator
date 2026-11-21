use super::{
    BASIC, CLAIRVOYANT_LOOM, FORM, INTREPID_ROLLERBEARINGS, PERSPICACIOUS_MAINFRAME, SKILL, TALENT,
    ULTIMATE,
};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats, Outcome,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

static NON_QUANTUM: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

const MIDDLE: Option<&str> = Some("dpr:2");

fn xueyi(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Destruction,
            element: crate::Element::Quantum,
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
            base_aggro: 125,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
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
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.7, 0.35]),
            (ULTIMATE.into(), vec![1.5, 0.09, 0.36]),
            (TALENT.into(), vec![8.0, 0.45, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn pela() -> OfficialCharacter {
    let mut input = xueyi(0, &[]);
    input.unit_id = "character.1106".into();
    input.form_id = input.unit_id.clone();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Nihility,
        element: crate::Element::Ice,
    };
    input.stats.speed_micros = 250_000_000;
    input.energy_capacity = 110;
    input.skill_parameters = [
        ("skill.110601".into(), vec![0.5]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.30, 2.0, 0.60]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    input
}

fn himeko() -> OfficialCharacter {
    let mut input = xueyi(0, &[]);
    input.unit_id = "character.1003".into();
    input.form_id = input.unit_id.clone();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Erudition,
        element: crate::Element::Fire,
    };
    input.skill_parameters = [
        ("skill.100301".into(), vec![0.5]),
        ("skill.100302".into(), vec![1.0, 0.4]),
        ("skill.100303".into(), vec![1.38, 5.0]),
        ("skill.100304".into(), vec![0.7, 3.0]),
    ]
    .into();
    input
}

fn apply(battle: &mut Battle<'_>, actor: &str, action: Action, target_id: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target_id
        })
        .expect("fixture action must be legal");
    battle.apply(&command).unwrap();
}

fn follow_up_damage(battle: &Battle<'_>) -> Vec<(usize, Action, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| (hit.target, hit.action, hit.amount))
        .collect()
}

#[test]
fn xueyi_basic_and_blast_track_resources_and_toughness() {
    let plan = PreparedBattle::official_dpr(&[xueyi(0, &[])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    apply(&mut battle, FORM, Action::Basic, MIDDLE);
    assert_eq!(battle.snapshot().skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.units[0].xueyi_state.karma, 1);
    assert_eq!(battle.units[2].toughness, 110);
    assert_eq!(battle.dummy_damage[0].amount, 300);

    apply(&mut battle, FORM, Action::Skill, MIDDLE);
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 110_000);
    assert_eq!(battle.units[0].xueyi_state.karma, 5);
    assert_eq!(battle.units[2].toughness, 90);
    assert_eq!(battle.units[1].toughness, 20);
    assert_eq!(battle.units[3].toughness, 20);
    let hits: Vec<_> = battle.dummy_damage[1..]
        .iter()
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(hits, [(2, 420), (1, 210), (3, 210)]);
}

#[test]
fn xueyi_karma_uses_actual_reduction_and_one_stack_per_ally_attack() {
    let support = pela();
    let plan = PreparedBattle::official_dpr(&[xueyi(0, &[]), support], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(1);
    apply(&mut battle, "character.1106", Action::Ultimate, None);
    assert_eq!(battle.units[0].xueyi_state.karma, 1, "three targets");

    apply(&mut battle, "character.1106", Action::Basic, MIDDLE);
    assert_eq!(battle.units[0].xueyi_state.karma, 2, "two hits");
    battle.units[3].toughness = 15;
    battle.units[2].toughness = 0;
    battle.units[4].toughness = 0;
    apply(&mut battle, FORM, Action::Skill, MIDDLE);
    assert_eq!(battle.units[3].toughness, 0);
    assert_eq!(
        battle.units[0].xueyi_state.karma, 3,
        "floor of 15 divided by 10"
    );
    apply(&mut battle, "character.1106", Action::Skill, MIDDLE);
    assert_eq!(battle.units[0].xueyi_state.karma, 3, "no actual reduction");
}

#[test]
fn xueyi_ally_attack_can_trigger_follow_up_once_with_original_action() {
    let plan = PreparedBattle::official_dpr(&[xueyi(0, &[]), pela()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(1);
    battle.units[0].xueyi_state.karma = 7;
    apply(&mut battle, "character.1106", Action::Ultimate, None);
    let follow_up = follow_up_damage(&battle);
    assert_eq!(follow_up.len(), 3);
    assert_eq!(
        follow_up,
        [
            (4, Action::Ultimate, 300),
            (2, Action::Ultimate, 300),
            (2, Action::Ultimate, 300)
        ]
    );
    assert_eq!(battle.units[0].xueyi_state.karma, 0);
}

#[test]
fn xueyi_and_himeko_follow_ups_keep_team_order_and_track_ally_toughness() {
    let plan = PreparedBattle::official_dpr(&[himeko(), xueyi(0, &[]), pela()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].xueyi_state.karma = 7;
    battle.units[3].toughness = 20;
    battle.units[5].toughness = 20;
    battle.fill_energy(2);
    apply(&mut battle, "character.1106", Action::Ultimate, None);
    let follow_ups: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::FollowUp)
        .filter_map(|event| event.source_id.as_deref())
        .collect();
    assert_eq!(follow_ups, ["character.1003", FORM]);
    assert_eq!(battle.units[1].xueyi_state.karma, 1);
}

#[test]
fn xueyi_ultimate_uses_capped_toughness_bonus_and_half_toughness_trace() {
    let xueyi = xueyi(0, &[INTREPID_ROLLERBEARINGS]);
    let plan = PreparedBattle::official_dpr(&[xueyi], 42, 3).unwrap();
    for (remaining, expected_damage, expected_karma) in [
        (0, 1_000, 0),
        (15, 1_021, 1),
        (30, 1_143, 3),
        (40, 1_224, 4),
        (59, 1_224, 4),
        (60, 1_314, 4),
        (120, 1_314, 4),
    ] {
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[2].toughness = remaining;
        battle.fill_energy(0);
        apply(&mut battle, FORM, Action::Ultimate, MIDDLE);
        assert_eq!(
            battle.dummy_damage[0].amount, expected_damage,
            "{remaining}"
        );
        assert_eq!(battle.units[2].toughness, remaining.saturating_sub(40));
        assert_eq!(battle.units[0].xueyi_state.karma, expected_karma);
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.snapshot().skill_points, 3);
    }
}

#[test]
fn xueyi_ultimate_breaks_non_quantum_enemy_but_respects_damage_resistance() {
    let mut plan = PreparedBattle::official_dpr(&[xueyi(0, &[])], 42, 3).unwrap();
    plan.units[2].kit = Kit::Enemy(&NON_QUANTUM);
    plan.units[2].stats.resist_bp[Element::Quantum.index()] = 2_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[2].toughness = 30;
    apply(&mut battle, FORM, Action::Basic, MIDDLE);
    assert_eq!(battle.units[2].toughness, 30);
    assert_eq!(battle.units[0].xueyi_state.karma, 0);
    battle.fill_energy(0);
    apply(&mut battle, FORM, Action::Ultimate, MIDDLE);
    assert_eq!(battle.units[2].toughness, 0);
    let effects: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.target_id.as_deref() == MIDDLE)
        .filter_map(|event| match event.kind {
            EventKind::Damage { kind, amount, .. } => Some((kind, amount)),
            _ => None,
        })
        .collect();
    assert_eq!(
        effects,
        [
            (DamageKind::Direct, 240),
            (DamageKind::Direct, 914),
            (DamageKind::WeaknessBreak, 3516)
        ],
    );
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusApplied(StatusKind::Entangled)
            && event.source_id.as_deref() == Some(FORM)
            && event.target_id.as_deref() == MIDDLE
    }));
    assert_eq!(battle.units[0].xueyi_state.karma, 3);
}

#[test]
fn xueyi_e6_queues_three_seeded_hits_and_mainframe_preserves_overflow() {
    for (traces, expected_karma) in [(&[PERSPICACIOUS_MAINFRAME][..], 3), (&[][..], 0)] {
        let plan = PreparedBattle::official_dpr(&[xueyi(6, traces)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].xueyi_state.karma = 5;
        apply(&mut battle, FORM, Action::Skill, MIDDLE);
        assert_eq!(battle.snapshot().random_draws, 3);
        assert_eq!(battle.units[0].energy_milli, 96_000);
        assert_eq!(battle.units[0].xueyi_state.karma, expected_karma);
        assert_eq!(
            follow_up_damage(&battle),
            [
                (3, Action::Skill, 378),
                (1, Action::Skill, 378),
                (1, Action::Skill, 378)
            ]
        );
        assert_eq!(
            [battle.units[1].toughness, battle.units[3].toughness],
            [10, 15]
        );
    }
}

#[test]
fn xueyi_e2_follow_up_ignores_weakness_and_heals_once() {
    for eidolon in [1, 2] {
        let mut plan = PreparedBattle::official_dpr(&[xueyi(eidolon, &[])], 42, 3).unwrap();
        plan.units[2].kit = Kit::Enemy(&NON_QUANTUM);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].hp = 500;
        battle.units[0].xueyi_state.karma = 5;
        battle.units[1].hp = 0;
        battle.units[2].toughness = 35;
        battle.units[3].hp = 0;
        battle.fill_energy(0);
        apply(&mut battle, FORM, Action::Ultimate, MIDDLE);
        assert_eq!(battle.units[2].toughness, 0);
        assert_eq!(battle.units[0].xueyi_state.karma, 0);
        assert_eq!(battle.units[0].energy_milli, 11_000);
        assert_eq!(battle.units[0].hp, if eidolon == 2 { 550 } else { 500 });

        // The follow-up itself must cross a non-Quantum break for E2.
        battle.units[2].toughness = 10;
        battle.units[2].entanglement = None;
        battle.units[0].xueyi_state.karma = 8;
        apply(&mut battle, FORM, Action::Basic, MIDDLE);
        assert_eq!(battle.units[2].toughness, if eidolon == 2 { 0 } else { 10 });
        assert_eq!(battle.units[0].hp, if eidolon == 2 { 600 } else { 500 });
        assert_eq!(battle.units[0].xueyi_state.karma, 0);
        let breaks = battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::WeaknessBroken)
            .count();
        assert_eq!(breaks, usize::from(eidolon));
    }
}

#[test]
fn xueyi_follow_up_reselects_living_enemies_and_awards_energy_per_landed_hit() {
    let mut plan = PreparedBattle::official_dpr(&[xueyi(2, &[])], 42, 3).unwrap();
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&NON_QUANTUM);
        enemy.stats.hp = 100;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].xueyi_state.karma = 8;
    apply(&mut battle, FORM, Action::Basic, Some("dpr:3"));
    assert_eq!(battle.snapshot().outcome, Outcome::Victory);
    assert_eq!(battle.snapshot().random_draws, 1);
    assert_eq!(battle.units[0].energy_milli, 114_000);
    let targets: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::FollowUp,
                ..
            } => event.target_id.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(targets, ["dpr:2", "dpr:1"]);
}

#[test]
fn xueyi_e4_break_effect_applies_before_ultimate_and_expires_after_two_turns() {
    let mut input = xueyi(4, &[CLAIRVOYANT_LOOM, INTREPID_ROLLERBEARINGS]);
    input.stats.break_effect_bp = 23_000;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    apply(&mut battle, FORM, Action::Ultimate, MIDDLE);
    for _ in 0..3 {
        apply(&mut battle, FORM, Action::Basic, MIDDLE);
    }
    let direct: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(direct, [3_474, 1_020, 1_020, 990]);

    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    apply(&mut battle, FORM, Action::Ultimate, Some("dpr:1"));
    assert_eq!(battle.dummy_damage[1].kind, DamageKind::WeaknessBreak);
    assert_eq!(battle.dummy_damage[1].amount, 5_808);
}

#[test]
fn xueyi_fixed_dpr_replay_conserves_follow_up_and_break_assists() {
    let mut input = xueyi(
        6,
        &[
            CLAIRVOYANT_LOOM,
            INTREPID_ROLLERBEARINGS,
            PERSPICACIOUS_MAINFRAME,
        ],
    );
    input.stats.crit_rate_bp = 6_000;
    input.stats.break_effect_bp = 8_000;
    let plan = PreparedBattle::official_dpr(&[input, pela()], 42, 3).unwrap();
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(result.characters[1].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|character| character.own_damage + character.assist_damage)
            .sum::<u64>()
    );
    let hits: Vec<_> = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .filter(|hit| hit.source_id == FORM)
        .collect();
    for kind in [
        DamageKind::FollowUp,
        DamageKind::WeaknessBreak,
        DamageKind::Additional,
    ] {
        assert!(hits.iter().any(|hit| hit.kind == kind), "{kind:?}");
    }
    assert!(
        hits.iter()
            .any(|hit| hit.kind == DamageKind::FollowUp && hit.credits.len() > 1)
    );
    for hit in hits {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
}
