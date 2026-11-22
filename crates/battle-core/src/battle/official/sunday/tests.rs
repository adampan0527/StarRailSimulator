use crate::{
    CombatType, Command, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO,
    battle::{Kit, SummonTurn},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const JING_YUAN: &str = "character.1204";
const LIGHTNING_LORD: &str = "summon:character.1204:lightning-lord";
const HERTA: &str = "character.1013";
const TOPAZ: &str = "character.1112";
const FEIXIAO: &str = "character.1220";
const LEFT: &str = "dpr:1";

fn sunday(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: Element::Imaginary,
        },
        energy_capacity: ENERGY_CAP,
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
            crit_damage_bp: 20_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![1.0, 0.30, 2.0, 0.50]),
            (ULTIMATE.into(), vec![0.20, 0.30, 3.0, 0.12]),
            (TALENT.into(), vec![0.20, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn ally(form: &str, path: CombatPath, element: Element, cap: u32) -> OfficialCharacter {
    let mut input = sunday(0, &[]);
    input.unit_id = form.into();
    input.form_id = form.into();
    input.combat_type = CombatType { path, element };
    input.energy_capacity = cap;
    input.stats.base_speed_micros = 100_000_000;
    input.stats.speed_micros = 100_000_000;
    input.stats.crit_damage_bp = 0;
    input
}

fn jing_yuan() -> OfficialCharacter {
    let mut input = ally(JING_YUAN, CombatPath::Erudition, Element::Thunder, 130);
    input.skill_parameters = [
        ("skill.120401".into(), vec![1.0]),
        ("skill.120402".into(), vec![0.50, 2.0]),
        ("skill.120403".into(), vec![1.20, 3.0]),
        (
            "skill.120404".into(),
            vec![60.0, 0.33, 10.0, 3.0, 0.25, 10.0],
        ),
    ]
    .into();
    input
}

fn herta() -> OfficialCharacter {
    let mut input = ally(HERTA, CombatPath::Erudition, Element::Ice, 110);
    input.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    input
}

fn feixiao() -> OfficialCharacter {
    let mut input = ally(FEIXIAO, CombatPath::Hunt, Element::Wind, 0);
    input.skill_parameters = [
        ("skill.122001".into(), vec![0.5]),
        ("skill.122002".into(), vec![2.0]),
        ("skill.122003".into(), vec![1.6, 1.0, 6.0, 7.0]),
        ("skill.122008".into(), vec![0.6, 0.3]),
        ("skill.122009".into(), vec![0.6, 0.3]),
        ("skill.122004".into(), vec![1.1, 2.0, 6.0, 12.0, 0.6, 2.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: Some(target.into()),
    };
    battle.apply(&command).unwrap();
}

fn force_ultimate(battle: &mut Battle<'_>, target: &str) {
    battle.fill_energy(0);
    act(battle, FORM, Action::Ultimate, target);
}

#[test]
fn sunday_validates_locked_skill_rows_and_supports_only_the_base_kit() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = sunday(0, &[]);
    input.skill_parameters.get_mut(SKILL).unwrap()[0] = 0.5;
    assert!(build(&input).is_err());
    input = sunday(0, &[]);
    input.skill_parameters.get_mut(TALENT).unwrap()[0] = f64::NAN;
    assert!(build(&input).is_err());
}

#[test]
fn sunday_basic_deals_imaginary_damage_and_supplies_energy_and_skill_points() {
    let plan = plan(&[sunday(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, LEFT);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[1].toughness, 390);
    assert_eq!(battle.dummy_damage[0].amount, 600);
    assert_eq!(battle.dummy_damage[0].kind, DamageKind::Direct);
}

#[test]
fn sunday_advances_jing_yuan_before_lightning_lord_and_distinguishes_e1_defense_ignore() {
    let plan = plan(&[sunday(1, &[]), jing_yuan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let av = battle.av_micros;
    assert_eq!(av, 50_000_000);
    act(&mut battle, FORM, Action::Skill, JING_YUAN);
    assert_eq!(battle.ordinary_actor(), Some(JING_YUAN));
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(
        battle
            .action_order()
            .iter()
            .take(2)
            .map(|entry| { (entry.unit_id.as_str(), entry.remaining_av_micros) })
            .collect::<Vec<_>>(),
        [(JING_YUAN, 0), (LIGHTNING_LORD, 0)]
    );
    act(&mut battle, JING_YUAN, Action::Basic, LEFT);
    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 4);
    // 1000 × 1.8 × 0.9 / (1 + 0.5 × 0.84) for Jing Yuan's basic hit.
    assert_eq!((hits[0].kind, hits[0].amount), (DamageKind::Direct, 1_140));
    assert_eq!(hits[0].assists, [(0, 540)]);
    // Each Lightning-Lord hit is 330 × 1.8 × 0.9 / (1 + 0.5 × 0.6).
    for hit in &hits[1..] {
        assert_eq!(
            (hit.kind, hit.amount, hit.av_micros),
            (DamageKind::FollowUp, 411, av)
        );
        assert_eq!(hit.assists, [(0, 213)]);
    }
}

#[test]
fn sunday_does_not_advance_harmony_or_treat_an_independent_clock_as_a_summon() {
    let mut support = herta();
    support.combat_type.path = CombatPath::Harmony;
    let plan = plan(&[sunday(0, &[]), support, jing_yuan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].summon_turn = Some(SummonTurn::with_action_advance("clock", 90_000_000, false));
    let clock_gauge = battle.units[1].summon_turn.unwrap().gauge_quanta;
    act(&mut battle, FORM, Action::Skill, HERTA);
    assert_eq!(battle.av_micros, 100_000_000);
    assert_eq!(
        battle.units[1].summon_turn.unwrap().gauge_quanta,
        clock_gauge - 50_000_000 * 90_000_000
    );
    assert!(battle.events().iter().all(|event| {
        !matches!(event.kind, EventKind::GaugeShifted { .. })
            || event.target_id.as_deref() != Some(HERTA)
    }));
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::Damage), 3_000);

    let plan = PreparedBattle::official_dpr(&[sunday(0, &[]), herta()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].summon_turn = Some(SummonTurn::with_action_advance("clock", 90_000_000, false));
    let clock_gauge = battle.units[1].summon_turn.unwrap().gauge_quanta;
    act(&mut battle, FORM, Action::Skill, HERTA);
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    assert_eq!(
        battle.units[1].summon_turn.unwrap().gauge_quanta,
        clock_gauge
    );
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::Damage), 3_000);
}

#[test]
fn sunday_a6_cleanses_before_the_advanced_turn() {
    static ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 400,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut plan = plan(&[sunday(0, &[HAVEN_IN_PALM]), herta()]);
    plan.units[2].kit = Kit::Enemy(&ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 1_000, Action::Basic));
    act(&mut battle, FORM, Action::Skill, HERTA);
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    assert_eq!(battle.av_micros, 50_000_000);
    let events = battle.events();
    let cleared = events.iter().position(|event| {
        event.source_id.as_deref() == Some(FORM)
            && event.target_id.as_deref() == Some(HERTA)
            && matches!(event.kind, EventKind::StatusCleared)
    });
    let moved = events.iter().position(|event| {
        event.source_id.as_deref() == Some(FORM)
            && event.target_id.as_deref() == Some(HERTA)
            && matches!(event.kind, EventKind::GaugeShifted { .. })
    });
    assert!(matches!((cleared, moved), (Some(cleared), Some(moved)) if cleared < moved));
}

#[test]
fn sunday_ultimate_only_buffs_the_latest_target_and_applies_flat_energy_with_a2() {
    let mut carry = herta();
    carry.stats.energy_regeneration_bp = 15_000;
    let plan = plan(&[sunday(2, &[REST_DAYS_LONGING]), carry, feixiao()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    force_ultimate(&mut battle, HERTA);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[1].energy_milli, 95_000);
    assert_eq!(battle.effective_crit_damage_bp(1), 7_200);
    assert_eq!(battle.skill_points, 5);
    act(&mut battle, FORM, Action::Skill, HERTA);
    assert_eq!(battle.skill_points, 5);
    let action_points: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::SkillPoints { before, after } => Some((before, after)),
            _ => None,
        })
        .collect();
    assert_eq!(action_points, [(3, 5), (5, 4), (4, 5)]);

    let non_energy_start = battle.events().len();
    force_ultimate(&mut battle, FEIXIAO);
    assert_eq!(battle.units[2].energy_milli, 0);
    assert!(!battle.events()[non_energy_start..].iter().any(|event| {
        event.target_id.as_deref() == Some(FEIXIAO)
            && matches!(event.kind, EventKind::Energy { .. })
    }));
    assert_eq!(battle.effective_crit_damage_bp(1), 0);
    assert_eq!(battle.effective_crit_damage_bp(2), 7_200);
    assert_eq!(battle.skill_points, 5);

    let plan = PreparedBattle::official_dpr(&[sunday(0, &[]), herta()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    force_ultimate(&mut battle, HERTA);
    assert_eq!(battle.units[1].energy_milli, 77_000);
}

#[test]
fn sunday_beatified_counts_own_ordinary_turn_openings_and_clears_on_defeat() {
    let mut caster = sunday(4, &[EXALTED_SWEEP]);
    caster.stats.energy_regeneration_bp = 12_000;
    let plan = plan(&[caster, herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 104_600); // 65 + 25 × 1.2 + 8 × 1.2.
    force_ultimate(&mut battle, HERTA);
    let mechanics = battle.mechanics(0).unwrap();
    mechanics.on_ally_turn_opening(&mut battle, 0, 1, false);
    mechanics.on_ally_turn_opening(&mut battle, 0, 0, true);
    assert_eq!(Sunday::beatified(&battle, 0, 1).unwrap().turns, 3);
    for remaining in [2, 1] {
        mechanics.on_ally_turn_opening(&mut battle, 0, 0, false);
        assert_eq!(Sunday::beatified(&battle, 0, 1).unwrap().turns, remaining);
    }
    mechanics.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(battle.effective_crit_damage_bp(1), 0);
    force_ultimate(&mut battle, HERTA);
    mechanics.on_defeated(&mut battle, 0);
    assert!(Sunday::beatified(&battle, 0, 1).is_none());
}

#[test]
fn sunday_e6_converts_only_overflow_and_credits_the_same_critical_hit() {
    let mut carry = herta();
    carry.stats.crit_rate_bp = 8_500;
    let plan = plan(&[sunday(6, &[]), carry]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, HERTA);
    act(&mut battle, HERTA, Action::Basic, LEFT);
    let hit = &battle.dummy_damage[0];
    // 105% CR contributes 10% CD. Include +30% DMG, 16% DEF ignore, and intact toughness.
    assert!(hit.critical);
    assert_eq!(hit.amount, 906);
    assert_eq!(hit.assists, [(0, 306)]);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritRate), 2_000);
    force_ultimate(&mut battle, HERTA);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritRate), 4_000);
    act(&mut battle, FORM, Action::Skill, HERTA);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritRate), 6_000);
    force_ultimate(&mut battle, HERTA);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritRate), 6_000);
    for _ in 0..4 {
        battle.units[1].normal_turns += 1;
        battle.finish_turn(1);
    }
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritRate), 0);
    assert_eq!(
        battle.ally_buff_value(0, 1, AllyBuffKind::CritOverflowRatio),
        0
    );
}

#[test]
fn sunday_recognizes_topaz_skill_as_summon_damage_but_still_classifies_her_basic_separately() {
    let mut topaz = ally(TOPAZ, CombatPath::Hunt, Element::Fire, 130);
    topaz.skill_parameters = [
        ("skill.111201".into(), vec![0.50]),
        ("skill.111202".into(), vec![0.75, 0.25]),
        ("skill.111203".into(), vec![0.75, 0.125, 0.50, 2.0]),
        ("skill.111204".into(), vec![80.0, 0.75, 0.50]),
    ]
    .into();
    let plan = plan(&[sunday(1, &[]), topaz]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, TOPAZ);
    let rules = battle.mechanics(1).unwrap();
    assert!(rules.damage_comes_from_summon(&battle, 1, Some(Action::Skill), DamageKind::Direct));
    assert!(rules.damage_comes_from_summon(&battle, 1, Some(Action::Skill), DamageKind::FollowUp));
    assert!(!rules.damage_comes_from_summon(&battle, 1, Some(Action::Basic), DamageKind::Direct));
}

#[test]
fn sunday_fixed_dpr_replays_and_credits_all_summon_damage() {
    let mut caster = sunday(0, &[EXALTED_SWEEP, REST_DAYS_LONGING, HAVEN_IN_PALM]);
    caster.stats.base_speed_micros = 99_000_000;
    caster.stats.speed_micros = 99_000_000;
    let plan = PreparedBattle::official_dpr(&[jing_yuan(), caster], 42, 3).unwrap();
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
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == JING_YUAN
                    && hit.kind == DamageKind::FollowUp
                    && hit
                        .credits
                        .iter()
                        .any(|item| item.unit_id == FORM && item.amount > 0)
            })
    );
}
