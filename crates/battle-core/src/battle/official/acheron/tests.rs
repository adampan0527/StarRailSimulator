use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, resolve::FollowUpAttack},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const RIGHT: &str = "dpr:3";
const PELA: &str = "character.1106";
const JIAOQIU: &str = "character.1218";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 10,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn acheron(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: Element::Thunder,
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
            base_aggro: 100,
            base_speed_micros: 101_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 0,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.6, 0.6, 1.0]),
            (ULTIMATE.into(), vec![0.24, 0.15, 1.2, 9.0, 0.6, 3.72, 3.0]),
            (TALENT.into(), vec![9.0, 0.2]),
            (RAINBLADES[0].into(), vec![0.24, 0.15, 1.2]),
            (RAINBLADES[1].into(), vec![0.24, 0.15, 1.2]),
            (RAINBLADES[2].into(), vec![0.24, 0.15, 1.2]),
            (STYGIAN_RESURGE.into(), vec![1.2]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn pela() -> OfficialCharacter {
    let mut ally = acheron(0, &[]);
    ally.unit_id = PELA.into();
    ally.form_id = PELA.into();
    ally.energy_capacity = 110;
    ally.combat_type.element = Element::Ice;
    ally.skill_parameters = [
        ("skill.110601".into(), vec![0.5]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.3, 2.0, 1.0]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    ally
}

fn jiaoqiu() -> OfficialCharacter {
    let mut ally = acheron(0, &[]);
    ally.unit_id = JIAOQIU.into();
    ally.form_id = JIAOQIU.into();
    ally.energy_capacity = 100;
    ally.combat_type.element = Element::Fire;
    ally.stats.speed_micros = 250_000_000;
    ally.stats.effect_hit_rate_bp = 10_000;
    ally.skill_parameters = [
        ("skill.121801".into(), vec![0.5]),
        ("skill.121802".into(), vec![0.75, 0.45, 1.0]),
        ("skill.121803".into(), vec![0.6, 0.5, 0.09, 3.0, 6.0]),
        (
            "skill.121804".into(),
            vec![1.0, 0.075, 0.025, 5.0, 2.0, 0.9],
        ),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
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

fn dreams(battle: &Battle<'_>) -> (u8, u8) {
    let state = &battle.units[0].acheron_state;
    (state.dreams, state.overflow)
}

fn knots(battle: &Battle<'_>) -> Vec<u8> {
    battle
        .enemies()
        .map(|enemy| battle.units[enemy].crimson_knots)
        .collect()
}

#[test]
fn acheron_rejects_an_energy_pool_or_inconsistent_locked_parameters() {
    let mut input = acheron(0, &[]);
    input.energy_capacity = 9;
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_err());

    let mut input = acheron(0, &[]);
    input.skill_parameters.remove(RAINBLADES[2]);
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_err());

    let mut input = acheron(0, &[]);
    input.skill_parameters.insert(TALENT.into(), vec![8.0, 0.2]);
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_err());

    let mut input = acheron(6, &[]);
    input.skill_parameters.insert(
        ULTIMATE.into(),
        vec![0.2592, 0.162, 1.296, 9.0, 0.648, 4.0176, 3.24],
    );
    for id in RAINBLADES {
        input
            .skill_parameters
            .insert(id.into(), vec![0.2592, 0.162, 1.296]);
    }
    input
        .skill_parameters
        .insert(STYGIAN_RESURGE.into(), vec![1.296]);
    input
        .skill_parameters
        .insert(TALENT.into(), vec![9.0, 0.22]);
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_ok());
}

#[test]
fn acheron_skill_break_grants_one_independent_stack_and_energy_cannot_fill_dreams() {
    let mut input = acheron(0, &[RED_ONI]);
    input.stats.energy_regeneration_bp = 25_000;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3)
        .unwrap()
        .with_dummies(&[10, 20, 10])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(dreams(&battle), (5, 0));
    assert_eq!(knots(&battle).into_iter().sum::<u8>(), 5);
    assert!(!(1..=3).any(|enemy| battle.is_debuffed(enemy)));

    battle.fill_energy(0);
    battle.gain_energy(0, 0, 99_000);
    battle.restore_energy_flat(0, 0, 50_000);
    let invalid = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action: Action::Ultimate,
        target_id: Some(CENTER.into()),
    };
    let before = battle.snapshot();
    assert!(battle.apply(&invalid).is_err());
    assert_eq!(battle.snapshot(), before);
    assert_eq!(dreams(&battle), (5, 0));

    act(&mut battle, FORM, Action::Skill, Some(CENTER));
    assert_eq!(dreams(&battle), (7, 0));
    assert_eq!(knots(&battle).into_iter().sum::<u8>(), 7);
    assert!((1..=3).all(|enemy| battle.has_shock(enemy)));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 0);
    assert_eq!(battle.snapshot().units[0].max_energy_milli, 0);
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::Energy { .. }))
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(2, 960), (1, 360), (3, 360)],
    );
}

#[test]
fn acheron_groups_multitarget_and_reapplied_debuffs_by_ability() {
    let plan = prepared(&[acheron(0, &[]), jiaoqiu()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Acheron::set_dreams(&mut battle, 0, 0, 3, 0);
    Acheron::set_knots(&mut battle, 0, 4, 3);
    act(&mut battle, JIAOQIU, Action::Skill, Some(CENTER));
    assert_eq!(
        battle
            .enemies()
            .map(|enemy| battle.debuff_count(enemy))
            .collect::<Vec<_>>(),
        [1, 1, 1]
    );
    assert_eq!(dreams(&battle), (4, 0));
    assert_eq!(knots(&battle), [0, 0, 4]);

    battle.fill_energy(1);
    act(&mut battle, JIAOQIU, Action::Ultimate, None);
    assert_eq!(dreams(&battle), (5, 0));
    assert_eq!(knots(&battle), [0, 0, 5]);
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(JIAOQIU)
            && event.kind
                == EventKind::SlashedDream {
                    before: 4,
                    after: 5,
                    overflow_before: 0,
                    overflow_after: 0,
                }
    }));
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(dreams(&battle), (5, 0));
    act(&mut battle, JIAOQIU, Action::Basic, Some(CENTER));
    assert_eq!(dreams(&battle), (6, 0));
    // At 100 AV the three targets start distinct actions. Each new Roast may
    // contribute once even though no enemy attacks in the dummy scenario.
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(JIAOQIU));
    assert_eq!(dreams(&battle), (9, 0));
}

#[test]
fn acheron_resolves_the_debuffs_from_a_queued_follow_up_in_an_independent_scope() {
    let plan = PreparedBattle::official_dpr(&[acheron(0, &[])], 42, 3)
        .unwrap()
        .with_dummies(&[10, 40, 40])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_defense_down(0, 3, 2_000, 1);
    assert_eq!(dreams(&battle), (0, 0));

    let parent = battle.begin_debuff_action();
    battle.apply_defense_down(0, 2, 2_000, 1);
    battle.follow_up_single(
        0,
        1,
        Action::Skill,
        FollowUpAttack {
            element: Element::Thunder,
            multiplier_bp: 1_000,
            toughness: 10,
            energy_milli: 0,
            first_hit: None,
        },
    );
    battle.resolve_follow_ups();
    assert!(battle.has_shock(1));
    assert_eq!(dreams(&battle), (1, 0));
    assert_eq!(knots(&battle), [1, 0, 0]);

    battle.finish_debuff_action(0, parent);
    assert_eq!(dreams(&battle), (2, 0));
    assert_eq!(knots(&battle), [1, 1, 0]);
}

#[test]
fn acheron_rainblades_consume_knots_before_resurge_and_keep_excess_separately() {
    let plan = prepared(&[acheron(0, &[RED_ONI, THUNDER_CORE])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Acheron::set_dreams(&mut battle, 0, 0, 9, 0);
    Acheron::set_knots(&mut battle, 0, 1, 3);
    Acheron::set_knots(&mut battle, 0, 2, 0);
    Acheron::set_knots(&mut battle, 0, 3, 6);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(
        mechanics.dpr_priority(&battle, 0, Action::Ultimate, Some(3)),
        Some(0)
    );
    assert_eq!(
        mechanics.dpr_priority(&battle, 0, Action::Ultimate, Some(2)),
        Some(1)
    );

    // Zero-damage, non-breaking applications model extra stacks banked before release.
    for _ in 0..4 {
        let parent = battle.begin_debuff_action();
        battle.apply_defense_down(0, 2, 0, 1);
        battle.finish_debuff_action(0, parent);
    }
    assert_eq!(dreams(&battle), (9, 3));
    assert_eq!(knots(&battle), [3, 0, 6]);

    act(&mut battle, FORM, Action::Ultimate, Some(RIGHT));
    assert_eq!(dreams(&battle), (3, 0));
    assert_eq!(knots(&battle).into_iter().sum::<u8>(), 3);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].acheron_state.thunder_core_stacks, 3);
    assert!(!battle.units[0].acheron_state.ultimate_active);

    // 1000 ATK × 2/3 DEF × .9 toughness × 1.2 RES, then apply Thunder Core.
    // A rainblade's own hit precedes the new 30% stack and each Knot AoE follows it.
    let hits = battle
        .dummy_damage
        .iter()
        .map(|hit| {
            assert_eq!(
                (hit.action, hit.kind),
                (Action::Ultimate, DamageKind::Direct)
            );
            (hit.target, hit.amount)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        &hits[..15],
        [
            (3, 172),
            (1, 561),
            (2, 561),
            (3, 561),
            (3, 224),
            (1, 691),
            (2, 691),
            (3, 691),
            (1, 276),
            (1, 820),
            (2, 820),
            (3, 820),
            (1, 1641),
            (2, 1641),
            (3, 1641),
        ],
    );
    assert_eq!(hits.len(), 21);
    assert!(hits[15..].iter().all(|&(_, amount)| amount == 342));
    assert_eq!(
        [
            battle.units[1].toughness,
            battle.units[2].toughness,
            battle.units[3].toughness
        ],
        [390, 395, 385]
    );

    battle.dummy_damage.clear();
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.dummy_damage[0].amount, 570);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [570, 570, 570, 570, 300]
    );
}

#[test]
fn acheron_rainblade_transfers_only_unused_knots_on_lethal_hits() {
    let mut plan = prepared(&[acheron(0, &[])]);
    plan.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[1].stats.hp = 100;
    plan.units[1].stats.base_hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Acheron::set_dreams(&mut battle, 0, 0, 9, 0);
    Acheron::set_knots(&mut battle, 0, 1, 6);
    Acheron::set_knots(&mut battle, 0, 3, 3);

    act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
    // Left target consumes 3 before falling, and the remaining 3 transfer to
    // the right target. Both remaining cuts can consume three stacks each.
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(knots(&battle), [0, 0, 0]);
    assert_eq!(dreams(&battle), (0, 0));
    let changes = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::CrimsonKnot { before, after }
                if event.target_id.as_deref() == Some(RIGHT) =>
            {
                Some((before, after))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(changes, [(0, 3), (3, 6), (6, 3), (3, 0)]);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 2)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [432, 432, 432, 864],
    );
}

#[test]
fn acheron_abyss_uses_frozen_nihility_count_and_e2_adds_a_turn_start_knot() {
    for (eidolon, others, expected) in [
        (0, 0, 300),
        (0, 1, 345),
        (0, 2, 480),
        (2, 0, 300),
        (2, 1, 480),
    ] {
        let mut team = vec![acheron(eidolon, &[ABYSS])];
        if others > 0 {
            team.push(pela());
        }
        if others > 1 {
            let mut fox = jiaoqiu();
            fox.stats.speed_micros = 150_000_000;
            team.push(fox);
        }
        let plan = prepared(&team);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let started = u8::from(eidolon >= 2);
        assert_eq!(dreams(&battle), (started, 0));
        assert_eq!(knots(&battle), [started, 0, 0]);
        act(&mut battle, FORM, Action::Basic, Some(CENTER));
        assert_eq!(battle.dummy_damage[0].amount, expected);
    }
}

#[test]
fn acheron_e4_labels_vulnerability_and_e6_uses_lightning_against_other_weaknesses() {
    let mut plan = prepared(&[acheron(6, &[]), pela()]);
    let target = 3;
    plan.units[target].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[target].stats.hp = 20_000;
    plan.units[target].stats.base_hp = 20_000;
    plan.units[target].stats.resist_bp[Element::Thunder.index()] = 4_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.debuff_count(target), 1);
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusApplied(StatusKind::UltimateVulnerability)
            && event.target_id.as_deref() == Some(CENTER)
    }));

    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[target].toughness, 0);
    assert!(battle.has_shock(target));
    assert_eq!(dreams(&battle), (2, 0));
    // E6 Basic benefits from 20% penetration and E4's 8% ultimate vulnerability.
    // Only a real Ultimate activates the Talent's separate 20% RES reduction.
    let direct = battle
        .events()
        .iter()
        .find(|event| {
            event.target_id.as_deref() == Some(CENTER)
                && matches!(
                    event.kind,
                    EventKind::Damage {
                        kind: DamageKind::Direct,
                        ..
                    }
                )
        })
        .unwrap();
    assert!(matches!(direct.kind, EventKind::Damage { amount: 259, .. }));

    battle.fill_energy(1);
    act(&mut battle, PELA, Action::Ultimate, None);
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.actor == 1
            && hit.action == Action::Ultimate
            && hit
                .assists
                .iter()
                .any(|&(owner, amount)| owner == 0 && amount > 0)
    }));
}

#[test]
fn acheron_e1_uses_actual_debuffs_and_ultimate_break_does_not_refill_dreams() {
    let mut input = acheron(1, &[]);
    input.stats.crit_damage_bp = 5_000;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3)
        .unwrap()
        .with_dummies(&[5, 5, 5])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();
    Acheron::set_dreams(&mut battle, 0, 0, 9, 0);
    Acheron::set_knots(&mut battle, 0, 2, 9);
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 2, Some(Action::Ultimate), DamageKind::Direct),
        0
    );
    battle.apply_defense_down(0, 2, 3_000, 1);
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 2, Some(Action::Ultimate), DamageKind::Direct),
        1_800
    );
    act(&mut battle, FORM, Action::Ultimate, Some(CENTER));
    assert_eq!(dreams(&battle), (0, 0));
    assert!((1..=3).all(|enemy| battle.has_shock(enemy)));
    assert_eq!(knots(&battle), [0, 0, 0]);
}

#[test]
fn acheron_fixed_dpr_replays_action_order_and_attributes_nearby_hits() {
    let plan = PreparedBattle::official_dpr(
        &[
            acheron(2, &[RED_ONI, ABYSS, THUNDER_CORE]),
            jiaoqiu(),
            pela(),
        ],
        42,
        3,
    )
    .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let first = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let second = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(first.total_damage, second.total_damage);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    for target in &first.targets {
        assert!(target.hits.iter().any(|hit| hit.source_id == FORM
            && hit.action == Action::Ultimate
            && hit.kind == DamageKind::Direct));
        for hit in &target.hits {
            assert_eq!(
                u64::from(hit.amount),
                hit.credits
                    .iter()
                    .map(|credit| u64::from(credit.amount))
                    .sum::<u64>()
            );
        }
    }
}
