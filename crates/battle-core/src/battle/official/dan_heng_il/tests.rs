use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
    supports_official_dpr_variant,
};

use super::*;

const CENTER: &str = "dpr:2";
const SPARKLE: &str = "character.1306";

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: IMAGINARY,
        },
        energy_capacity: 140,
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
            (SKILL.into(), vec![0.12, 4.0]),
            (ULTIMATE.into(), vec![3.0, 1.4, 2.0, 3.0]),
            (TALENT.into(), vec![0.1, 6.0]),
            (TRANSCENDENCE.into(), vec![2.6]),
            (DIVINE_SPEAR.into(), vec![3.8, 0.6]),
            (FULGURANT_LEAP.into(), vec![5.0, 1.8]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn sparkle() -> OfficialCharacter {
    let mut input = character(0, &["trace.11306102"]);
    input.unit_id = SPARKLE.into();
    input.form_id = SPARKLE.into();
    input.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Quantum,
    };
    input.energy_capacity = 110;
    input.stats.base_speed_micros = 150_000_000;
    input.stats.speed_micros = 150_000_000;
    input.skill_parameters = [
        ("skill.1130601".into(), vec![1.0]),
        ("skill.1130602".into(), vec![0.24, 0.45, 2.0, 0.5]),
        ("skill.1130603".into(), vec![2.0, 6.0, 0.06, 3.0, 10.0]),
        ("skill.1130604".into(), vec![2.0, 0.04, 2.0, 3.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter], skill_points: u8) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, skill_points)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn enhance(battle: &mut Battle<'_>, tier: u8) {
    for _ in 0..tier {
        act(battle, FORM, Action::Skill, None);
    }
}

fn hits(battle: &Battle<'_>, target: usize, action: Action) -> Vec<u32> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| {
            hit.actor == 0
                && hit.target == target
                && hit.kind == DamageKind::Direct
                && hit.action == action
        })
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn dan_heng_il_each_tier_uses_its_own_hits_energy_and_toughness() {
    let plan = plan(&[character(0, &[])], 3);
    let normal_hits: &[u32] = &[180, 462];
    let no_adjacent_hits: &[u32] = &[];
    let expected = [
        (0, normal_hits, no_adjacent_hits, 90_000, 10, 0),
        (1, &[514, 566, 636], no_adjacent_hits, 100_000, 20, 0),
        (2, &[456, 501, 547, 592, 638], &[234, 252], 105_000, 30, 10),
        (
            3,
            &[426, 468, 511, 553, 596, 639, 710],
            &[351, 378, 405, 432],
            110_000,
            40,
            20,
        ),
    ];
    for (tier, main, adjacent, energy, main_toughness, adjacent_toughness) in expected {
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        enhance(&mut battle, tier);
        act(&mut battle, FORM, Action::Basic, Some(CENTER));
        assert_eq!(hits(&battle, 2, Action::Basic), main, "tier {tier}");
        assert_eq!(hits(&battle, 1, Action::Basic), adjacent, "tier {tier}");
        assert_eq!(hits(&battle, 3, Action::Basic), adjacent, "tier {tier}");
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(battle.units[2].toughness, 400 - main_toughness);
        assert_eq!(battle.units[1].toughness, 400 - adjacent_toughness);
        assert_eq!(battle.units[3].toughness, 400 - adjacent_toughness);
        assert_eq!(battle.skill_points, if tier == 0 { 4 } else { 3 - tier });
    }
}

#[test]
fn dan_heng_il_preparations_cancel_and_validate_without_spending_or_advancing() {
    let plan = plan(&[character(0, &[]), sparkle()], 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let snapshot = battle.snapshot();
    let event_start = battle.events().len();
    enhance(&mut battle, 3);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, snapshot.av_micros);
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 70_000);
    assert_eq!(
        battle.events()[event_start..]
            .iter()
            .map(|event| &event.kind)
            .collect::<Vec<_>>(),
        [
            &EventKind::BasicEnhancement {
                before: 0,
                after: 1
            },
            &EventKind::BasicEnhancement {
                before: 1,
                after: 2
            },
            &EventKind::BasicEnhancement {
                before: 2,
                after: 0
            },
        ]
    );

    enhance(&mut battle, 2);
    battle.set_skill_points(0, 1);
    let basic = command(&battle, FORM, Action::Basic, Some(CENTER));
    assert!(!battle.legal_actions().contains(&basic));
    let before_illegal = battle.snapshot();
    assert_eq!(
        battle.apply(&basic),
        Err(BattleError::InsufficientSkillPoints)
    );
    assert_eq!(battle.snapshot(), before_illegal);
    enhance(&mut battle, 1);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.skill_points, 2);
}

#[test]
fn dan_heng_il_squama_pays_first_and_notifies_sparkle_once_for_all_three_points() {
    let plan = plan(&[character(0, &[]), sparkle()], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.units[0].dan_heng_il_state.squama, 2);
    let sparkle_rules = battle.mechanics(1).unwrap();
    enhance(&mut battle, 3);
    assert_eq!(battle.skill_points, 1);
    assert!(
        sparkle_rules
            .ability_override(&battle, 1, Action::Skill)
            .is_none()
    );
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].dan_heng_il_state.squama, 0);
    assert_eq!(battle.skill_points, 0);
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    assert!(
        sparkle_rules
            .ability_override(&battle, 1, Action::Skill)
            .is_some()
    );
    act(&mut battle, SPARKLE, Action::Skill, Some(FORM));
    assert_eq!(battle.skill_points, 0);
}

#[test]
fn dan_heng_il_ultimate_adds_three_strikes_and_e2_advances_then_replaces_all_shared_points() {
    let plan = plan(&[character(2, &[]), sparkle()], 0);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    let av = battle.av_micros;
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some(CENTER));
    assert_eq!(hits(&battle, 3, Action::Ultimate), [540, 648, 1008]);
    assert_eq!(hits(&battle, 2, Action::Ultimate), [252, 302, 470]);
    assert_eq!(battle.units[0].dan_heng_il_state.squama, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[0].gauge_quanta, 0);
    act(&mut battle, SPARKLE, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, av);
    let after_support = battle.skill_points;
    enhance(&mut battle, 3);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].dan_heng_il_state.squama, 0);
    assert_eq!(battle.skill_points, after_support);
}

#[test]
fn dan_heng_il_e1_advances_righteous_heart_twice_and_e4_carries_outroar_for_one_turn() {
    let mut input = character(4, &[]);
    input.stats.crit_rate_bp = 10_000;
    let plan = plan(&[input], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    enhance(&mut battle, 3);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    let leap_hits = hits(&battle, 2, Action::Basic);
    assert_eq!(&leap_hits[..3], [639, 766, 894]);
    // 74% × 200% damage factor × 198% crit factor × 0.6 DEF/toughness.
    assert_eq!(leap_hits[6], 1_758);
    assert_eq!(battle.units[0].dan_heng_il_state.righteous_heart, 0);
    assert_eq!(battle.units[0].dan_heng_il_state.outroar, 4);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(hits(&battle, 2, Action::Basic)[7], 356);
    assert_eq!(battle.units[0].dan_heng_il_state.outroar, 0);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(hits(&battle, 2, Action::Basic)[9], 270);
}

#[test]
fn dan_heng_il_e6_caps_allied_ultimate_stacks_and_consumes_them_on_leap_only() {
    let plan = plan(&[character(6, &[]), sparkle()], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..4 {
        battle.fill_energy(1);
        act(&mut battle, SPARKLE, Action::Ultimate, None);
    }
    assert_eq!(battle.units[0].dan_heng_il_state.e6_stacks, 3);
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.units[0].dan_heng_il_state.e6_stacks, 3);
    enhance(&mut battle, 3);
    let mut baseline = battle.clone();
    baseline.units[0].dan_heng_il_state.e6_stacks = 0;
    act(&mut baseline, FORM, Action::Basic, Some(CENTER));
    act(&mut battle, FORM, Action::Basic, Some(CENTER));

    let baseline = hits(&baseline, 3, Action::Basic);
    let boosted = hits(&battle, 3, Action::Basic);
    assert_eq!(boosted.len(), 7);
    assert!((boosted[0] * 5).abs_diff(baseline[0] * 8) <= 8);
    assert_eq!(battle.units[0].dan_heng_il_state.e6_stacks, 0);
    assert_eq!(battle.units[0].dan_heng_il_state.active_penetration_bp, 0);
}

#[test]
fn dan_heng_il_major_traces_grant_starting_energy_control_resist_and_weakness_crit() {
    let mut input = character(0, &[STAR_VEIL, AQUA_REIGN, JOLT_ANEW]);
    input.stats.crit_rate_bp = 10_000;
    let plan = plan(&[input], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.mechanics(0).unwrap().control_resistance_bp(), 3_500);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(hits(&battle, 2, Action::Basic), [313, 803]);
}

#[test]
fn dan_heng_il_finite_blast_keeps_its_original_neighbors_after_the_main_target_dies() {
    let mut plan = plan(&[character(0, &[])], 3);
    const FINITE: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 400,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    for enemy in 1..=3 {
        plan.units[enemy].kit = Kit::Enemy(&FINITE);
    }
    plan.units[2].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    enhance(&mut battle, 3);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    let damage_to = |id: &str| {
        battle
            .events()
            .iter()
            .filter(|event| {
                event.source_id.as_deref() == Some(FORM)
                    && event.target_id.as_deref() == Some(id)
                    && matches!(
                        &event.kind,
                        EventKind::Damage {
                            kind: DamageKind::Direct,
                            ..
                        }
                    )
            })
            .count()
    };
    assert_eq!(damage_to(CENTER), 1);
    assert_eq!(damage_to("dpr:1"), 4);
    assert_eq!(damage_to("dpr:3"), 4);
}

#[test]
fn dan_heng_il_fixed_dpr_replays_with_limited_resources_and_source_attribution() {
    let plan = PreparedBattle::official_dpr(
        &[character(6, &[STAR_VEIL, AQUA_REIGN, JOLT_ANEW]), sparkle()],
        42,
        3,
    )
    .unwrap();
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
    assert!(result.characters[0].own_damage > 0);
    assert!(result.characters[1].assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == FORM
                    && hit.action == Action::Ultimate
                    && hit.kind == DamageKind::Direct
            })
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|role| role.own_damage + role.assist_damage)
            .sum::<u64>()
    );
}

#[test]
fn dan_heng_il_validates_all_attack_rows_and_only_enables_the_base_kit() {
    let mut input = character(0, &[]);
    assert!(supports_official_dpr_variant(FORM, "base"));
    assert!(!supports_official_dpr_variant(FORM, "novaflare"));
    // The aggregate can exceed u16 while individual strikes still fit.
    input.skill_parameters.get_mut(FULGURANT_LEAP).unwrap()[0] = 7.0;
    assert!(build(&input).is_ok());
    for skill in [
        BASIC,
        SKILL,
        ULTIMATE,
        TALENT,
        TRANSCENDENCE,
        DIVINE_SPEAR,
        FULGURANT_LEAP,
    ] {
        let mut invalid = character(0, &[]);
        invalid.skill_parameters.remove(skill);
        assert!(matches!(build(&invalid), Err(BattleError::InvalidInput(_))));
    }
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[3] = 4.0;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
}
