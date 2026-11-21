use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit, calculate_dpr, fixture,
};

const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const ALLY: &str = "character.1008";

fn jingliu(novaflare: bool, eidolon: u8, traces: bool) -> OfficialCharacter {
    let (ids, multipliers) = if novaflare {
        (
            NOVAFLARE,
            [
                vec![0.5],
                vec![1.5, 1.0],
                vec![1.8, 1.0, 0.9, 1.0],
                vec![0.3, 0.05, 0.44, 5.0, 2.0, 1.0, 0.5, 20.0],
                vec![1.5, 1.0, 0.75],
            ],
        )
    } else {
        (
            BASE,
            [
                vec![1.0],
                vec![2.0, 1.0],
                vec![3.0, 1.0, 1.5, 1.0],
                vec![0.3, 0.04, 5.4, 1.8, 2.0, 1.0, 0.5],
                vec![2.5, 1.0, 1.25],
            ],
        )
    };
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: ICE,
        },
        energy_capacity: ENERGY_CAPACITY,
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
            crit_rate_bp: 10_000,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            ids.basic,
            ids.skill,
            ids.ultimate,
            ids.talent,
            ids.enhanced_skill,
        ]
        .into_iter()
        .zip(multipliers)
        .map(|(id, values)| (id.to_owned(), values))
        .collect(),
        eidolon,
        enabled_trace_ids: if traces {
            [ids.deathrealm, ids.sword_champion, ids.frost_wraith]
                .map(str::to_owned)
                .to_vec()
        } else {
            Vec::new()
        },
    }
}

fn ally() -> OfficialCharacter {
    let mut input = jingliu(false, 0, false);
    input.unit_id = ALLY.into();
    input.form_id = ALLY.into();
    input.combat_type.element = Element::Thunder;
    input.energy_capacity = 110;
    input.stats.base_speed_micros = 80_000_000;
    input.stats.speed_micros = 80_000_000;
    input.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400, 400, 400])
        .unwrap()
}

fn command(battle: &Battle<'_>, action: Action, target: &str) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: Some(target.into()),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: &str) {
    battle.apply(&command(battle, action, target)).unwrap();
}

fn enter(battle: &mut Battle<'_>) {
    act(battle, Action::Skill, CENTER);
    act(battle, Action::Skill, CENTER);
}

fn state<'a>(battle: &'a Battle<'_>) -> &'a JingliuState {
    &battle.units[0].jingliu_state
}

#[test]
fn jingliu_rejects_mixed_or_incomplete_rows_and_registers_both_variants() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(crate::supports_official_dpr_variant(FORM, "novaflare"));
    assert!(!crate::supports_official_dpr_variant(FORM, "enhanced"));
    let mut input = jingliu(false, 0, false);
    input
        .skill_parameters
        .insert(NOVAFLARE.basic.into(), vec![0.5]);
    assert!(build(&input).is_err());
    let mut input = jingliu(true, 0, false);
    input.skill_parameters.remove(NOVAFLARE.enhanced_skill);
    assert!(build(&input).is_err());
    let mut input = jingliu(true, 0, false);
    input.skill_parameters.get_mut(NOVAFLARE.talent).unwrap()[7] = 19.0;
    assert!(build(&input).is_err());
}

#[test]
fn jingliu_base_enters_on_two_stacks_and_gets_the_next_turn_at_the_same_av() {
    let prep = plan(&[jingliu(false, 0, true), ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.snapshot().av_micros, 95_000_000);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(state(&battle).syzygy, 1);
    let av = battle.av_micros;
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(state(&battle).syzygy, 2);
    assert_eq!(battle.skill_points, 1);
    let illegal = command(&battle, Action::Basic, CENTER);
    assert!(!battle.legal_actions().contains(&illegal));
    let snapshot = battle.snapshot();
    assert_eq!(battle.apply(&illegal), Err(BattleError::IllegalTiming));
    assert_eq!(battle.snapshot(), snapshot);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .effect_resistance_bonus_bp(&battle, 0),
        3_500
    );
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.skill_points, 1);
    assert_eq!(state(&battle).syzygy, 1);
    assert_eq!(battle.hp(1).0, 960);
    assert_eq!(battle.dummy_damage[2].amount, 2_736);
    assert_eq!(
        battle.events().iter().find_map(|event| match event.kind {
            EventKind::HpSpent { amount } => Some((
                event.source_id.as_deref(),
                event.target_id.as_deref(),
                amount
            )),
            _ => None,
        }),
        Some((Some(FORM), Some(ALLY), 40)),
    );
}

#[test]
fn jingliu_base_conversion_uses_actual_loss_and_respects_the_e4_cap() {
    let prep = plan(&[jingliu(false, 4, false), ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    battle.units[1].hp = 13;
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.hp(1).0, 1);
    // 12 actual HP × 6.3 yields 75 whole ATK. E1 raises the critical multiplier to 1.74.
    assert_eq!(battle.dummy_damage[2].amount, 2_805);
    assert_eq!(battle.dummy_damage[3].amount, 1_402);

    let mut tank = ally();
    tank.stats.hp = 20_000;
    let prep = plan(&[jingliu(false, 4, false), tank]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.hp(1).0, 19_200);
    // 2,100 cap from 210% of 1,000 base ATK, below the conversion from 800 HP.
    assert_eq!(battle.dummy_damage[2].amount, 8_091);
}

#[test]
fn jingliu_novaflare_scales_main_and_adjacent_hits_with_attributable_max_hp() {
    let mut input = jingliu(true, 0, true);
    input.stats.attack = 20_000;
    let prep = plan(&[input, ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(state(&battle).syzygy, 3);
    battle.units[0].energy_milli = 0;
    battle.apply_max_hp_increase(1, 0, 500, 2);
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.units[0].energy_milli, 38_000);
    assert_eq!(battle.hp(1).0, 950);
    assert_eq!(state(&battle).moonlight, 1);
    let hits = &battle.dummy_damage[2..];
    assert_eq!(
        hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [2_619, 1_309, 1_309]
    );
    assert_eq!(hits[0].assists, [(1, 873)]);
    assert_eq!(hits[1].assists, [(1, 436)]);
}

#[test]
fn jingliu_novaflare_counts_fully_absorbed_damage_and_deduplicates_an_attack_per_target() {
    let mut prep = plan(&[jingliu(true, 0, false), ally()]);
    let kit = fixture::enemy("fixture.target_left", 32).unwrap();
    prep.units[2].kit = Kit::Enemy(kit);
    prep.units[2].stats.hp = 10_000;
    prep.units[2].stats.attack = 1_000;
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    battle.apply_official_shield(0, 0, 20_000, 20, None);

    for expected in 1..=18 {
        battle.resolve_enemy(2, kit);
        assert_eq!(state(&battle).health_events, expected);
    }
    assert_eq!(battle.hp(0).0, 1_000);
    let mechanics = battle.mechanics(0).unwrap();
    mechanics.on_ally_health_event(&mut battle, 0, 0, Some(90_000));
    mechanics.on_ally_health_event(&mut battle, 0, 0, Some(90_000));
    assert_eq!(state(&battle).health_events, 19);
    assert_eq!(state(&battle).syzygy, 0);
    mechanics.on_ally_health_event(&mut battle, 0, 1, Some(90_000));
    assert_eq!(state(&battle).health_events, 0);
    assert_eq!(state(&battle).syzygy, 1);
    assert_eq!(battle.spend_ally_hp(0, 1, 1_000, 1), 999);
    assert_eq!(battle.spend_ally_hp(0, 1, 1_000, 1), 0);
    assert_eq!(state(&battle).health_events, 1);
}

#[test]
fn jingliu_novaflare_overflow_grants_defense_ignore_to_exactly_the_next_attack() {
    let prep = plan(&[jingliu(true, 0, true), ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    act(&mut battle, Action::Ultimate, CENTER);
    assert_eq!(state(&battle).syzygy, 4);
    assert!(!state(&battle).defense_ignore_ready);
    battle.fill_energy(0);
    act(&mut battle, Action::Ultimate, CENTER);
    assert!(state(&battle).defense_ignore_ready);

    let attack_start = battle.dummy_damage.len();
    act(&mut battle, Action::Skill, CENTER);
    assert!(!state(&battle).defense_ignore_ready);
    // Third Moonlight stack raises the total critical multiplier to 2.82.
    // Ignoring 25% DEF gives 1500 × 2.82 × (1000 / 1375) × 0.9.
    assert_eq!(battle.dummy_damage[attack_start].amount, 2_768);
    assert_eq!(battle.dummy_damage[attack_start + 1].amount, 1_384);
}

#[test]
fn jingliu_e1_extra_hit_uses_the_appropriate_target_and_scaling() {
    for novaflare in [false, true] {
        let prep = plan(&[jingliu(novaflare, 1, false), ally()]);
        let mut battle = Battle::start(&prep, LogMode::Detailed);
        enter(&mut battle);
        battle.fill_energy(0);
        act(&mut battle, Action::Ultimate, CENTER);
        let extras: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .collect();
        if novaflare {
            assert_eq!(extras.len(), 1);
            assert_eq!(extras[0].target, 3);
            assert_eq!(extras[0].amount, 1_104);
        } else {
            assert!(extras.is_empty());
        }
    }

    let prep = plan(&[jingliu(false, 1, false), ally()])
        .with_dummies(&[400])
        .unwrap();
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    act(&mut battle, Action::Skill, LEFT);
    act(&mut battle, Action::Skill, LEFT);
    act(&mut battle, Action::Skill, LEFT);
    let extra = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::Additional)
        .unwrap();
    assert_eq!(extra.amount, 1_269);
}

#[test]
fn jingliu_e2_only_boosts_one_enhanced_skill_and_e6_increases_entry_stacks() {
    let prep = plan(&[jingliu(false, 2, false), ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    battle.fill_energy(0);
    act(&mut battle, Action::Ultimate, CENTER);
    assert_eq!(state(&battle).syzygy, 3);
    assert!(state(&battle).amplified_skill_ready);
    let start = battle.dummy_damage.len();
    act(&mut battle, Action::Skill, CENTER);
    assert!(!state(&battle).amplified_skill_ready);
    // 1216 ATK × 2.5 × 1.8 E2 DMG × 1.74 CRIT × 0.6 target multipliers.
    assert_eq!(battle.dummy_damage[start].amount, 5_712);

    for (novaflare, entry, crit_damage, penetration) in
        [(false, 3, 5_000, 0), (true, 5, 6_400, 3_000)]
    {
        let prep = plan(&[jingliu(novaflare, 6, false), ally()]);
        let mut battle = Battle::start(&prep, LogMode::Detailed);
        enter(&mut battle);
        assert_eq!(state(&battle).syzygy, entry);
        let rules = battle.mechanics(0).unwrap();
        rules.on_ally_health_event(&mut battle, 0, 1, None);
        assert_eq!(
            rules.crit_damage_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
            crit_damage
        );
        assert_eq!(
            rules.resistance_penetration_bp(&battle, 0, ICE),
            penetration
        );
    }
}

#[test]
fn jingliu_novaflare_exits_at_zero_and_clears_moonlight() {
    let prep = plan(&[jingliu(true, 0, false), ally()]);
    let mut battle = Battle::start(&prep, LogMode::Detailed);
    enter(&mut battle);
    battle.units[0].jingliu_state.syzygy = 1;
    battle.units[0].jingliu_state.moonlight = 5;
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(state(&battle).syzygy, 0);
    assert_eq!(state(&battle).moonlight, 0);
    assert!(!state(&battle).transformed);
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::Moonlight {
            before: 5,
            after: 0
        }));
}

#[test]
fn jingliu_fixed_dpr_replays_both_versions_with_blast_and_ultimate_damage() {
    let scenario = STANDARD_DPR_SCENARIO;
    for novaflare in [false, true] {
        let prep = plan(&[jingliu(novaflare, 6, true), ally()]);
        let run = || calculate_dpr(&prep, &scenario.toughness, scenario.cycles).unwrap();
        let result = run();
        assert_eq!(run(), result);
        assert!(result.targets.iter().any(|target| {
            target
                .hits
                .iter()
                .any(|hit| hit.source_id == FORM && hit.action == Action::Ultimate)
        }));
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|character| character.own_damage + character.assist_damage)
                .sum::<u64>(),
        );
    }
}
