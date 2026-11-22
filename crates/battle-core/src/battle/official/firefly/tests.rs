use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, combat_status::DebuffKind, support::AllyBuffKind},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget, GAUGE_FULL},
};

const LEFT: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";

fn firefly(novaflare: bool, eidolon: u8, traces: [bool; 3]) -> OfficialCharacter {
    let ids = if novaflare { NOVAFLARE } else { BASE };
    let skill = if novaflare {
        vec![2.0, 1.0, 0.0, 0.0, 0.2, 0.1, 3.6, 0.0]
    } else {
        vec![2.0, 1.0, 0.25, 2.0, 0.2, 0.1, 3.6]
    };
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: FIRE,
        },
        energy_capacity: 240,
        level: 80,
        stats: OfficialStats {
            hp: 10_000,
            base_hp: 10_000,
            attack: 2_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 104_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 22_000,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 15_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (ids.basic.into(), vec![1.0]),
            (ids.skill.into(), vec![2.0, 0.4, 0.6, 0.25]),
            (ids.ultimate.into(), vec![0.2, 0.5, 60.0, 70.0]),
            (ids.talent.into(), vec![0.4, 0.5, 0.2, 0.3]),
            (ids.enhanced_basic.into(), vec![2.0, 0.2]),
            (ids.enhanced_skill.into(), skill),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: ids
            .traces
            .into_iter()
            .zip(traces)
            .filter_map(|(id, enabled)| enabled.then_some(id.into()))
            .collect(),
    }
}

fn tingyun() -> OfficialCharacter {
    let mut ally = firefly(false, 0, [false; 3]);
    ally.unit_id = "character.1202".into();
    ally.form_id = ally.unit_id.clone();
    ally.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Thunder,
    };
    ally.energy_capacity = 130;
    ally.stats.speed_micros = 110_000_000;
    ally.skill_parameters = [
        ("skill.120201".into(), vec![1.0]),
        ("skill.120202".into(), vec![0.4, 0.5, 3.0, 0.25]),
        ("skill.120203".into(), vec![50.0, 2.0, 0.5]),
        ("skill.120204".into(), vec![0.6]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn transform(battle: &mut Battle<'_>) {
    battle.set_energy(0, 0, ENERGY_CAP_MILLI);
    act(battle, Action::Ultimate, None);
}

#[test]
fn firefly_both_versions_require_the_correct_locked_rows() {
    for novaflare in [false, true] {
        let mut input = firefly(novaflare, 6, [true; 3]);
        assert!(build(&input).is_ok());
        let other = if novaflare {
            BASE.basic
        } else {
            NOVAFLARE.basic
        };
        input.skill_parameters.insert(other.into(), vec![1.0]);
        assert!(build(&input).is_err());
    }
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(crate::supports_official_dpr_variant(FORM, "novaflare"));
}

#[test]
fn firefly_skill_spends_hp_restores_fixed_energy_and_purges_at_cap() {
    let prepared = plan(&[firefly(false, 0, [true; 3])], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    battle.units[0].hp = 3_000;
    battle.set_energy(0, 0, 30_000);
    battle.apply_defense_down(1, 0, 2_000, 3);
    battle.apply_debuff(1, 0, DebuffKind::AttackDown, 3_000, 3);
    act(&mut battle, Action::Skill, Some(LEFT));
    assert_eq!(battle.units[0].hp, 1);
    assert_eq!(battle.units[0].energy_milli, 174_000);
    assert_eq!(battle.units[1].toughness, 100);
    assert_eq!(battle.av_micros, 87_500_000);
    assert_eq!(battle.units[0].debuffs.len(), 2);
    battle.set_energy(0, 0, ENERGY_CAP_MILLI);
    assert!(battle.units[0].debuffs.is_empty());
}

#[test]
fn firefly_countdown_has_fixed_speed_and_blocks_a_second_transformation() {
    let prepared = plan(&[firefly(false, 0, [false; 3])], &[400]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle);
    let start = battle.av_micros;
    assert_eq!(battle.units[0].energy_milli, 7_500);
    assert_eq!(battle.speed_micros(0), 260_000_000);
    let mut countdown = battle.units[0].summon_turn.unwrap();
    assert!(!countdown.can_advance);
    countdown.advance(10_000);
    assert_eq!(countdown.gauge_quanta, GAUGE_FULL);
    battle.set_energy(0, 0, ENERGY_CAP_MILLI);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::Ultimate)
    );
    while combustion(&battle, 0) {
        act(&mut battle, Action::Basic, Some(LEFT));
    }
    let exit = battle.events().iter().find_map(|event| {
        (event.kind == EventKind::CompleteCombustion { active: false }).then_some(event.av_micros)
    });
    assert_eq!(
        exit,
        Some(start + GAUGE_FULL.div_ceil(COUNTDOWN_SPEED_MICROS))
    );
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert!(
        battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::Ultimate)
    );
}

#[test]
fn firefly_variants_differ_in_implant_range_and_off_element_toughness() {
    static ENEMY: EnemyKit = EnemyKit {
        stats: crate::fixture::Stats {
            level: 80,
            hp: 100_000,
            base_hp: 100_000,
            ..DUMMY_STATS
        },
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    for novaflare in [false, true] {
        let mut prepared = plan(&[firefly(novaflare, 0, [true; 3])], &[120; 3]);
        for enemy in &mut prepared.units[1..] {
            enemy.kit = Kit::Enemy(&ENEMY);
            enemy.stats = ENEMY.stats;
        }
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.units[0].hp = 4_000;
        transform(&mut battle);
        act(&mut battle, Action::Skill, Some(MIDDLE));
        assert_eq!(battle.units[0].hp, 6_500);
        assert_eq!(battle.units[0].energy_milli, 7_500);
        assert_eq!(battle.skill_points, 2);
        assert!(battle.has_weakness(2, FIRE));
        assert_eq!(battle.units[2].toughness, 75);
        for neighbor in [1, 3] {
            assert_eq!(battle.has_weakness(neighbor, FIRE), novaflare);
            assert_eq!(
                battle.units[neighbor].toughness,
                if novaflare { 98 } else { 108 }
            );
        }
        let implants = battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::StatusApplied(StatusKind::WeaknessImplanted));
        assert_eq!(implants.count(), if novaflare { 3 } else { 1 });
    }
}

#[test]
fn firefly_novaflare_breaks_delay_the_countdown_three_times_and_grant_one_extra_turn() {
    let prepared = plan(&[firefly(true, 2, [true; 3])], &[10; 4]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle);
    act(&mut battle, Action::Skill, Some(MIDDLE));
    let state = &battle.units[0].firefly_state;
    assert_eq!(battle.av_micros, 50_000_000);
    assert_eq!(state.countdown_delays, 3);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        GAUGE_FULL + 3 * gauge_for(1_000)
    );
    assert!(battle.in_extra_turn(0));
    assert_eq!(battle.skill_points, 3);
    act(&mut battle, Action::Skill, Some("dpr:4"));
    assert_eq!(battle.units[0].firefly_state.countdown_delays, 3);
    assert!(!battle.in_extra_turn(0));
}

#[test]
fn firefly_e2_has_one_ordinary_turn_more_cooldown_in_the_base_variant() {
    for novaflare in [false, true] {
        let prepared = plan(&[firefly(novaflare, 2, [false; 3])], &[10; 3]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        transform(&mut battle);
        act(&mut battle, Action::Basic, Some(LEFT));
        assert!(battle.in_extra_turn(0));
        act(&mut battle, Action::Basic, Some(MIDDLE));
        assert!(!battle.in_extra_turn(0));
        act(&mut battle, Action::Basic, Some("dpr:3"));
        assert_eq!(battle.in_extra_turn(0), novaflare);
    }
}

#[test]
fn firefly_a6_attack_conversion_and_a4_threshold_are_credited_to_the_buffer() {
    let mut fly = firefly(true, 1, [true; 3]);
    fly.stats.break_effect_bp = 10_800;
    let prepared = plan(&[fly, tingyun()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle);
    assert_eq!(battle.effective_break_effect(0), 14_900);
    battle.apply_ally_buff(1, 0, AllyBuffKind::AttackFlat, 300, 3);
    assert_eq!(battle.effective_break_effect(0), 17_300);
    battle.set_toughness(0, 2, 0);
    act(&mut battle, Action::Skill, Some(LEFT));
    let hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::SuperBreak)
        .unwrap();
    // 45 toughness, 173% Break Effect, 20% vulnerability and 15% DEF ignore.
    assert_eq!(hit.amount, 38_976);
    assert_eq!(hit.assists, [(1, hit.amount)]);
}

#[test]
fn firefly_caps_the_direct_attack_scaling_and_uses_the_higher_super_break_tier() {
    for novaflare in [false, true] {
        let mut fly = firefly(novaflare, 0, [false, true, false]);
        fly.stats.break_effect_bp = 40_000;
        let prepared = plan(&[fly], &[120]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        transform(&mut battle);
        battle.set_toughness(0, 1, 0);
        act(&mut battle, Action::Skill, Some(LEFT));
        let hits: Vec<_> = battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect();
        // Direct: 2000 × (2 + 0.2 × 3.6) × 2/3. Super break uses the full 400%.
        assert_eq!(
            hits,
            [
                (DamageKind::Direct, 3_626),
                (
                    DamageKind::SuperBreak,
                    if novaflare { 101_723 } else { 33_907 }
                ),
            ]
        );
    }
}

#[test]
fn firefly_defensive_talent_and_e6_use_the_matching_state_and_element() {
    let prepared = plan(&[firefly(true, 6, [true; 3])], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    battle.units[0].hp = 6_000;
    assert_eq!(
        rules.incoming_damage_reduction_bp(&battle, 0, 1, DamageKind::Direct),
        2_000
    );
    assert_eq!(rules.effect_resistance_bonus_bp(&battle, 0), 0);
    transform(&mut battle);
    assert_eq!(
        rules.incoming_damage_reduction_bp(&battle, 0, 1, DamageKind::Direct),
        4_000
    );
    assert_eq!(rules.effect_resistance_bonus_bp(&battle, 0), 8_000);
    assert_eq!(rules.resistance_penetration_bp(&battle, 0, FIRE), 2_000);
    assert_eq!(rules.resistance_penetration_bp(&battle, 0, Element::Ice), 0);
    act(&mut battle, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[0].hp, 8_000);
    assert_eq!(battle.units[1].toughness, 90);
}

#[test]
fn firefly_fixed_dpr_replays_separate_break_and_super_break_for_each_variant() {
    for novaflare in [false, true] {
        let prepared = plan(&[firefly(novaflare, 6, [true; 3])], &[30, 120, 30]);
        let run =
            || calculate_dpr(&prepared, &[30, 120, 30], STANDARD_DPR_SCENARIO.cycles).unwrap();
        let result = run();
        assert_eq!(result, run());
        let hits: Vec<_> = result.targets.iter().flat_map(|t| &t.hits).collect();
        for kind in [DamageKind::WeaknessBreak, DamageKind::SuperBreak] {
            assert!(hits.iter().any(|hit| hit.kind == kind && !hit.critical));
        }
        assert_eq!(result.characters[0].own_damage, result.total_damage);
    }
}
