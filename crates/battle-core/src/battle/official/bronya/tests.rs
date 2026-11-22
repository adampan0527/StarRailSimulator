use super::{BASIC, BATTLEFIELD, COMMAND, MILITARY_MIGHT, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

const ACTOR: &str = "ally.bronya";
const COMPANION: &str = "ally.herta";
const MIDDLE: &str = "dpr:2";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: ACTOR.into(),
        form_id: "character.1101".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Harmony,
            element: crate::Element::Wind,
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
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.33, 0.0, 1.0, 1.0]),
            (ULTIMATE.into(), vec![0.33, 0.12, 0.12, 2.0]),
            (TALENT.into(), vec![0.15]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn companion(speed: u64) -> OfficialCharacter {
    let mut input = character(0, &[]);
    input.unit_id = COMPANION.into();
    input.form_id = "character.1013".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Erudition,
        element: crate::Element::Ice,
    };
    input.energy_capacity = 110;
    input.stats.base_speed_micros = speed * 1_000_000;
    input.stats.speed_micros = input.stats.base_speed_micros;
    input.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![1.0, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    input
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

#[test]
fn basic_lands_a_command_crit_and_its_talent_advances_the_next_turn() {
    let plan = PreparedBattle::official_dpr(&[character(0, &[COMMAND])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().av_micros, 50_000_000);
    act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
    let hit = &battle.dummy_damage[0];
    assert!(hit.critical);
    assert_eq!(hit.amount, 450);
    assert_eq!(battle.units[2].toughness, 110);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.skill_points, 4);
    // 50 AV for the opening turn plus 85% of another 50 AV.
    assert_eq!(battle.snapshot().av_micros, 92_500_000);
}

#[test]
fn skill_cleanses_then_advances_an_ally_and_credits_the_buffed_hit() {
    let plan = PreparedBattle::official_dpr(&[character(0, &[]), companion(125)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_speed_down(2, 1, 3_000, 2);
    assert!(battle.is_slowed(1));
    act(&mut battle, ACTOR, Action::Skill, Some(COMPANION));
    assert_eq!(battle.ordinary_actor(), Some(COMPANION));
    assert_eq!(battle.snapshot().av_micros, 50_000_000);
    assert!(!battle.is_slowed(1));
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(battle.skill_points, 2);
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(ACTOR)
            && event.target_id.as_deref() == Some(COMPANION)
            && matches!(
                event.kind,
                EventKind::GaugeShifted {
                    after_quanta: 0,
                    ..
                }
            )
    }));
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    let hit = &battle.dummy_damage[0];
    // 300 unbuffed damage multiplied by 1.33.
    assert_eq!(hit.amount, 399);
    assert_eq!(hit.assists, [(0, 99)]);
}

#[test]
fn casting_skill_on_bronya_keeps_the_regular_gauge() {
    let plan = PreparedBattle::official_dpr(&[character(0, &[])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, ACTOR, Action::Skill, Some(ACTOR));
    assert_eq!(battle.snapshot().av_micros, 100_000_000);
    act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.dummy_damage[0].amount, 399);
    assert!(battle.dummy_damage[0].assists.is_empty());
}

#[test]
fn ultimate_before_a_teammates_turn_increases_base_attack_and_critical_damage() {
    let mut bronya = character(0, &[]);
    bronya.stats.crit_damage_bp = 10_000;
    let mut herta = companion(250);
    herta.stats.attack = 1_300;
    herta.stats.crit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[bronya, herta], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    battle.units[0].energy_milli = 120_000;
    act(&mut battle, ACTOR, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.ordinary_actor(), Some(COMPANION));
    assert_eq!(battle.snapshot().av_micros, 40_000_000);
    assert_eq!(battle.effective_crit_damage_bp(0), 12_400);
    assert_eq!(battle.effective_crit_damage_bp(1), 7_400);
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    let hit = &battle.dummy_damage[0];
    // (1300 + 330 base-scaled ATK) × 50% × 1.74 CRIT × 2/3 DEF × 0.9 toughness.
    assert_eq!(hit.amount, 850);
    assert_eq!(hit.assists, [(0, 265)]);
}

#[test]
fn opening_defense_lasts_for_two_turns_of_each_ally() {
    let plan = PreparedBattle::official_dpr(&[character(0, &[BATTLEFIELD])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    assert_eq!(battle.defense(0), 600);
    act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.defense(0), 600);
    act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.defense(0), 500);
}

#[test]
fn military_might_buffs_the_allys_opening_hit_before_bronya_acts() {
    let plan =
        PreparedBattle::official_dpr(&[character(0, &[MILITARY_MIGHT]), companion(250)], 42, 3)
            .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.dummy_damage[0].amount, 330);
    assert_eq!(battle.dummy_damage[0].assists, [(0, 30)]);
}

#[test]
fn e1_refund_cooldown_only_starts_on_success_and_skips_the_following_turn() {
    for (seed, expected) in [(7, [(3, 1), (2, 1), (2, 2)]), (0, [(2, 1), (2, 2), (1, 2)])] {
        let plan = PreparedBattle::official_dpr(&[character(1, &[])], seed, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Off);
        for (points, draws) in expected {
            act(&mut battle, ACTOR, Action::Skill, Some(ACTOR));
            assert_eq!((battle.skill_points, battle.rng.draws), (points, draws));
        }
    }
}

#[test]
fn e2_speed_activates_after_the_redeployed_allys_action() {
    for action in [Action::Basic, Action::Ultimate] {
        let plan =
            PreparedBattle::official_dpr(&[character(2, &[]), companion(125)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Off);
        battle.units[1].energy_milli = 110_000;
        act(&mut battle, ACTOR, Action::Skill, Some(COMPANION));
        assert_eq!(battle.speed_micros(1), 125_000_000);
        act(
            &mut battle,
            COMPANION,
            action,
            (action == Action::Basic).then_some(MIDDLE),
        );
        assert_eq!(battle.speed_micros(1), 162_500_000);
        if action == Action::Ultimate {
            act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
        }
        act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
        act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
        assert_eq!(battle.speed_micros(1), 125_000_000);
    }
}

#[test]
fn e4_follows_an_allys_wind_weak_basic_once_before_each_bronya_turn() {
    let mut bronya = character(4, &[COMMAND]);
    bronya.stats.base_speed_micros = 100_000_000;
    bronya.stats.speed_micros = 100_000_000;
    let mut plan = PreparedBattle::official_dpr(&[bronya, companion(400)], 42, 3).unwrap();
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.base_speed_micros = 1_000_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, COMPANION, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].energy_milli, 60_000);
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 65_000);
    assert_eq!(battle.units[3].toughness, 100);
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 65_000);
    act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
    act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
    let follow_ups: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| (hit.actor, hit.target, hit.amount, hit.critical))
        .collect();
    assert_eq!(follow_ups, [(0, 3, 240, false), (0, 3, 240, false)]);
    assert_eq!(battle.units[0].energy_milli, 90_000);
}

#[test]
fn e6_skill_damage_bonus_covers_two_ally_turns() {
    let last_ally_hit = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .rev()
            .find(|hit| hit.actor == 1 && hit.kind == DamageKind::Direct)
            .unwrap()
            .amount
    };
    for (eidolon, expected) in [(0, 300), (6, 399)] {
        let plan = PreparedBattle::official_dpr(&[character(eidolon, &[]), companion(125)], 42, 3)
            .unwrap();
        let mut battle = Battle::start(&plan, LogMode::Off);
        act(&mut battle, ACTOR, Action::Skill, Some(COMPANION));
        act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
        assert_eq!(last_ally_hit(&battle), 399);
        act(&mut battle, ACTOR, Action::Basic, Some(MIDDLE));
        act(&mut battle, COMPANION, Action::Basic, Some(MIDDLE));
        assert_eq!(last_ally_hit(&battle), expected);
    }
}

#[test]
fn dpr_prioritizes_support_actions_and_conserves_credited_damage() {
    let plan =
        PreparedBattle::official_dpr(&[character(0, &[MILITARY_MIGHT]), companion(125)], 42, 3)
            .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    // The first unassisted Herta turn would be at 80 AV. Bronya advances it to 50 AV.
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == COMPANION
                    && hit.av_micros == 50_000_000
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == ACTOR && credit.amount > 0)
            })
    );
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
