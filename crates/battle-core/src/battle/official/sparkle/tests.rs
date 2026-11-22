use super::{BASE, FORM, KitIds, NOVAFLARE};
use crate::{
    Action, Battle, CombatPath, CombatType, Element, EventKind, LogMode, OfficialCharacter,
    OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, calculate_dpr,
    supports_official_dpr_variant,
};

const SPARKLE: &str = "ally.sparkle";
const HERTA: &str = "ally.herta";
const MIDDLE: &str = "dpr:2";

fn sparkle(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let (ids, skill_turns, restored, cipher, cipher_turns, talent) = if enhanced {
        (NOVAFLARE, 2.0, 6.0, 0.06, 3.0, 0.04)
    } else {
        (BASE, 1.0, 4.0, 0.10, 2.0, 0.06)
    };
    let KitIds {
        basic,
        skill,
        ultimate,
        talent: talent_id,
        ..
    } = ids;
    let mut ultimate_parameters = vec![2.0, restored, cipher, cipher_turns];
    if enhanced {
        ultimate_parameters.push(10.0);
    }
    OfficialCharacter {
        unit_id: SPARKLE.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: Element::Quantum,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 250_000_000,
            speed_micros: 250_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 10_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (basic.into(), vec![1.0]),
            (skill.into(), vec![0.24, 0.45, skill_turns, 0.5]),
            (ultimate.into(), ultimate_parameters),
            (talent_id.into(), vec![2.0, talent, 2.0, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn herta(speed: u64) -> OfficialCharacter {
    let mut input = sparkle(false, 0, &[]);
    input.unit_id = HERTA.into();
    input.form_id = "character.1013".into();
    input.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    input.stats.base_speed_micros = speed * 1_000_000;
    input.stats.speed_micros = input.stats.base_speed_micros;
    input.stats.crit_rate_bp = 10_000;
    input.stats.crit_damage_bp = 5_000;
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

fn use_ultimate(battle: &mut Battle<'_>) {
    battle.units[0].energy_milli = 110_000;
    act(battle, SPARKLE, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
}

#[test]
fn sparkle_half_advance_and_critical_damage_are_quantitative_in_both_modes() {
    for enhanced in [false, true] {
        let plan =
            PreparedBattle::official_dpr(&[sparkle(enhanced, 0, &[]), herta(100)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.snapshot().av_micros, 40_000_000);
        act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
        assert_eq!(battle.snapshot().av_micros, 50_000_000);
        assert_eq!(battle.ordinary_actor(), Some(HERTA));
        assert_eq!(battle.effective_crit_damage_bp(1), 11_900);
        assert!(battle.events().iter().any(|event| {
            event.source_id.as_deref() == Some(SPARKLE)
                && event.target_id.as_deref() == Some(HERTA)
                && matches!(event.kind, EventKind::GaugeShifted { .. })
        }));
        act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
        // 300 non-crit base before Talent. 2.19 critical multiplier plus 6% damage or 4% vulnerability.
        let (damage, assist) = if enhanced { (683, 233) } else { (696, 246) };
        assert_eq!(battle.dummy_damage[0].amount, damage);
        assert_eq!(battle.dummy_damage[0].assists, [(0, assist)]);
        assert_eq!(
            battle.effective_crit_damage_bp(1),
            if enhanced { 11_900 } else { 5_000 }
        );
    }
}

#[test]
fn sparkle_original_a4_lasts_between_turns_and_self_cast_keeps_the_gauge() {
    let plan = PreparedBattle::official_dpr(
        &[sparkle(false, 0, &[BASE.artificial_flower]), herta(100)],
        42,
        3,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, SPARKLE, Action::Skill, Some(SPARKLE));
    assert_eq!(battle.snapshot().av_micros, 80_000_000);
    assert_eq!(battle.effective_crit_damage_bp(0), 16_900);
    act(&mut battle, SPARKLE, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    assert_eq!(battle.effective_crit_damage_bp(0), 16_900);
    act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.snapshot().av_micros, 120_000_000);
    assert_eq!(battle.effective_crit_damage_bp(0), 10_000);
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::GaugeShifted { .. }))
    );
}

#[test]
fn sparkle_ultimate_overflow_is_bounded_and_only_releases_on_ally_turn_end() {
    for enhanced in [false, true] {
        let plan =
            PreparedBattle::official_dpr(&[sparkle(enhanced, 4, &[]), herta(100)], 42, 8).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.snapshot().skill_point_cap, 8);
        use_ultimate(&mut battle);
        use_ultimate(&mut battle);
        assert_eq!(battle.skill_points, 8);
        assert_eq!(
            battle.units[0].sparkle_state.overflow,
            if enhanced { 10 } else { 0 }
        );
        act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
        assert_eq!(battle.skill_points, if enhanced { 8 } else { 7 });
        assert_eq!(
            battle.units[0].sparkle_state.overflow,
            if enhanced { 9 } else { 0 }
        );
        assert_eq!(
            battle
                .events()
                .iter()
                .filter_map(|event| match event.kind {
                    EventKind::SkillPoints { before, after } => Some((before, after)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            if enhanced {
                vec![(8, 7), (7, 8)]
            } else {
                vec![(8, 7)]
            }
        );

        // The teammate’s basic attack uses the same team cap without drawing from the bank.
        act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
        assert_eq!(battle.skill_points, 8);
        assert_eq!(
            battle.units[0].sparkle_state.overflow,
            if enhanced { 9 } else { 0 }
        );
    }
    assert!(PreparedBattle::official_dpr(&[sparkle(true, 0, &[])], 42, 8).is_err());
    assert!(PreparedBattle::official_dpr(&[herta(100)], 42, 6).is_err());
}

#[test]
fn sparkle_enhanced_a2_and_a4_are_driven_by_actual_spends() {
    let plan = PreparedBattle::official_dpr(
        &[
            sparkle(true, 0, &[NOVAFLARE.almanac, NOVAFLARE.artificial_flower]),
            herta(100),
        ],
        42,
        4,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.skill_points, 3);

    // Model a teammate paying two points before the normal one-point Skill on the same turn.
    battle.spend_skill_points(1, 2);
    assert!(!battle.units[0].sparkle_state.free_skill);
    act(&mut battle, HERTA, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli, 87_000);
    assert_eq!(battle.skill_points, 0);
    assert!(battle.units[0].sparkle_state.free_skill);
    act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
    assert_eq!(battle.skill_points, 0);
    assert!(!battle.units[0].sparkle_state.free_skill);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::Skill)
    );

    act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
    battle.units[0].energy_milli = 0;
    act(&mut battle, SPARKLE, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 30_000);
    assert_eq!(battle.skill_points, 2);
}

#[test]
fn sparkle_e6_spreads_dreamdiver_on_ultimate_and_keeps_one_original_buff() {
    let plan = PreparedBattle::official_dpr(&[sparkle(false, 6, &[]), herta(100)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
    assert_eq!(battle.effective_crit_damage_bp(0), 10_000);
    assert_eq!(battle.effective_crit_damage_bp(1), 14_900);
    use_ultimate(&mut battle);
    assert_eq!(battle.skill_points, 7);
    assert_eq!(battle.effective_crit_damage_bp(0), 19_900);
    assert_eq!(battle.effective_crit_damage_bp(1), 14_900);
    assert_eq!(battle.effective_attack(1), 1_400);
    assert_eq!(battle.units[0].sparkle_state.allies[1].cipher_expires, 4);
}

#[test]
fn sparkle_enhanced_a6_e1_and_e2_are_attributed_through_distinct_multipliers() {
    let plan = PreparedBattle::official_dpr(
        &[sparkle(true, 2, &[NOVAFLARE.nocturne]), herta(100)],
        42,
        3,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    assert_eq!(battle.speed_micros(0), 287_500_000);
    assert_eq!(battle.effective_attack(1), 1_450);
    use_ultimate(&mut battle);
    assert_eq!(battle.effective_attack(1), 1_850);
    act(&mut battle, SPARKLE, Action::Skill, Some(HERTA));
    act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
    // 1850 ATK × 50% × 2.19 CRIT × 1000/(1000+450) DEF × 1.10 RES × 1.10 vulnerability × 0.9.
    assert_eq!(battle.dummy_damage[0].amount, 1_521);
    assert_eq!(battle.dummy_damage[0].assists, [(0, 1_071)]);
}

#[test]
fn sparkle_fixed_dpr_runs_both_variants_and_conserves_attribution() {
    for enhanced in [false, true] {
        assert!(supports_official_dpr_variant(
            FORM,
            if enhanced { "novaflare" } else { "base" }
        ));
        let traces = if enhanced { NOVAFLARE } else { BASE };
        let mut actor = sparkle(
            enhanced,
            0,
            &[traces.almanac, traces.artificial_flower, traces.nocturne],
        );
        actor.stats.base_speed_micros = 134_000_000;
        actor.stats.speed_micros = 134_000_000;
        let plan = PreparedBattle::official_dpr(&[actor, herta(100)], 42, 3).unwrap();
        let result = calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap();
        assert_eq!(
            result,
            calculate_dpr(
                &plan,
                &STANDARD_DPR_SCENARIO.toughness,
                STANDARD_DPR_SCENARIO.cycles
            )
            .unwrap()
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
        let (damage, assists) = if enhanced {
            (64_976, 36_996)
        } else {
            (42_241, 17_291)
        };
        assert_eq!(
            (result.total_damage, result.characters[0].assist_damage),
            (damage, assists)
        );
    }
}
