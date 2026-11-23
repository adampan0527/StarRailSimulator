use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, combat_status::DebuffKind, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const TARGET: &str = "dpr:2";
const SPRITE: &str = "character.1409:little-ica";
const CASTORICE: &str = "character.1407";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

fn hyacine(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: Element::Wind,
        },
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 4_000,
            base_hp: 4_000,
            attack: 500,
            base_attack: 500,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            elation_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.08, 160.0, 0.1, 200.0]),
            (
                ULTIMATE.into(),
                vec![0.1, 200.0, 0.3, 600.0, 3.0, 0.12, 240.0],
            ),
            (TALENT.into(), vec![0.5, 1.0, 0.8, 2.0, 3.0]),
            (MEM_SKILL.into(), vec![0.2, 0.5]),
            (MEM_TALENT.into(), vec![0.04, 0.02, 20.0, 0.02, 20.0]),
            (MEM_ARRIVAL.into(), vec![15.0, 30.0]),
            (MEM_DEPARTURE.into(), vec![0.3]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn castorice() -> OfficialCharacter {
    let mut input = hyacine(0, &[]);
    input.form_id = CASTORICE.into();
    input.unit_id = CASTORICE.into();
    input.combat_type.element = Element::Quantum;
    input.energy_capacity = 0;
    input.stats.hp = 10_000;
    input.stats.base_hp = 10_000;
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.140701".into(), vec![0.5]),
        ("skill.140702".into(), vec![0.3, 0.5, 0.3]),
        ("skill.140703".into(), vec![165.0, 3.0, 1.0, 0.2, 0.0]),
        ("skill.140704".into(), vec![0.0, 0.2, 3.0, 3.0]),
        ("skill.140709".into(), vec![0.4, 0.3, 0.5]),
        ("skill.1140701".into(), vec![0.4]),
        ("skill.1140702".into(), vec![0.25, 0.24, 0.28, 0.34, 0.25]),
        ("skill.1140703".into(), vec![5.0]),
        ("skill.1140705".into(), vec![0.1, 3.0]),
        ("skill.1140706".into(), vec![0.4, 6.0, 0.06, 800.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
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
    battle.resolve_command(&command).unwrap();
}

fn ultimate(battle: &mut Battle<'_>) {
    if battle.ordinary_actor.is_none() {
        battle.advance_to_decision();
    }
    battle.fill_energy(0);
    act(battle, Action::Ultimate, None);
}

#[test]
fn basic_uses_the_saved_max_hp_and_preserves_the_toughness_scale() {
    let prepared = plan(&[hyacine(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Basic, Some(TARGET));
    let hit = &battle.dummy_damage[0];
    assert_eq!(hit.kind, DamageKind::Direct);
    assert_eq!(hit.amount, 1_200);
    assert_eq!(battle.units[hit.target].toughness, 990);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 90_000);
}

#[test]
fn accepts_fractional_frozen_heal_parameters_and_rejects_missing_sprite_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    let mut input = hyacine(0, &[]);
    input
        .skill_parameters
        .insert(SKILL.into(), vec![0.05, 82.0, 0.0625, 102.5]);
    input.skill_parameters.insert(
        ULTIMATE.into(),
        vec![0.0625, 102.5, 0.18, 307.5, 3.0, 0.075, 123.0],
    );
    let prepared = plan(std::slice::from_ref(&input));
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].hyacine_state.healing_tally, 634);
    input.skill_parameters.remove(MEM_ARRIVAL);
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_err());
}

#[test]
fn sprite_has_a_real_hp_pool_and_no_ordinary_action_or_debuffs() {
    let mut prepared = plan(&[hyacine(0, &[])]);
    let enemy = 1;
    prepared.units[enemy].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.hp(sprite), (2_000, 2_000));
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert!(battle.units[0].summon_turn.is_none());
    assert!(
        battle
            .action_order()
            .iter()
            .all(|entry| entry.unit_id != SPRITE)
    );
    assert_eq!(battle.speed_micros(sprite), 0);
    battle.advance_ally(0, sprite, 10_000);
    assert_eq!(battle.gauge_quanta(sprite), gauge_for(10_000));
    assert!(!battle.apply_debuff(enemy, sprite, DebuffKind::AttackDown, 1_000, 2));
    assert!(!battle.apply_skill_freeze(enemy, sprite, 10_000, 1, 100, Action::Skill));
    battle.true_damage(enemy, sprite, Action::Skill, 500, Vec::new());
    assert_eq!(battle.hp(sprite).0, 1_500);
    assert_eq!(battle.hp(0).0, 4_000);
}

#[test]
fn ultimate_uses_full_overheal_tally_for_each_seeded_target_and_only_one_extra_turn() {
    let prepared = plan(&[hyacine(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].hyacine_state.healing_tally, 1_080);
    ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.hp(0), (5_800, 5_800));
    assert_eq!(battle.hp(sprite), (3_200, 3_200));
    assert_eq!(battle.units[0].hyacine_state.healing_tally, 1_200);
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert_eq!(battle.units[0].energy_milli, 10_000);
    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == sprite)
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 3);
    assert!(
        hits.iter()
            .all(|hit| hit.kind == DamageKind::Direct && hit.amount == 979)
    );
    assert!(
        battle
            .enemies()
            .all(|enemy| battle.units[enemy].toughness == 990)
    );
}

#[test]
fn simultaneous_hp_losses_are_coalesced_and_heal_after_the_unit_action() {
    let prepared = plan(&[hyacine(2, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.spend_ally_hp(0, 0, 500, 1);
    battle.spend_ally_hp(0, 0, 500, 1);
    assert_eq!(battle.hp(0).0, 3_000);
    assert_eq!(battle.hp(sprite).0, 2_000);
    assert_eq!(battle.speed_micros(0), 230_000_000);
    battle.notify_unit_action_finished(1);
    assert_eq!(battle.hp(0).0, 3_100);
    assert_eq!(battle.hp(sprite).0, 1_920);
    assert_eq!(battle.units[0].hyacine_state.healing_tally, 1_180);
    battle.notify_ally_turn_opening(0, false);
    assert_eq!(battle.hp(sprite).0, 1_920);
}

#[test]
fn after_rain_expiration_is_bound_to_hyacines_ordinary_turn_start() {
    let prepared = plan(&[hyacine(1, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.hp(0), (7_800, 7_800));
    assert_eq!(battle.hp(sprite), (4_200, 4_200));
    let expiry = battle.units[0].hyacine_state.after_rain_until.unwrap();
    battle.units[0].normal_turns = expiry - 1;
    battle.notify_ally_turn_opening(0, true);
    assert!(rain_active(&battle, 0));
    battle.units[0].normal_turns = expiry;
    battle.notify_ally_turn_opening(0, false);
    assert!(!rain_active(&battle, 0));
    assert_eq!(battle.hp(0), (4_000, 4_000));
    assert_eq!(battle.hp(sprite), (2_000, 2_000));
}

#[test]
fn exhausted_sprite_departs_once_and_only_the_first_summon_grants_extra_energy() {
    let prepared = plan(&[hyacine(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.units[sprite].hp = 40;
    battle.spend_ally_hp(0, 0, 500, 1);
    battle.notify_unit_action_finished(1);
    assert_eq!(battle.hp(0).0, 3_600);
    assert!(battle.active_memosprite(0).is_none());
    assert_eq!(battle.gauge_quanta(0), gauge_for(7_000));
    battle.ordinary_actor = Some(0);
    battle.set_energy(0, 0, 0);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.active_memosprite(0), Some(sprite));
    assert_eq!(battle.units[sprite].memosprite_generation, 2);
    assert_eq!(battle.units[0].energy_milli, 45_000);
}

#[test]
fn traces_supply_low_hp_healing_cleanse_and_speed_based_crit_damage() {
    let mut input = hyacine(4, &[GLOOMY_GRIN, STORMY_CARESS, TEMPESTUOUS_HALT]);
    input.stats.speed_micros = 210_000_000;
    let prepared = plan(&[input]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.hp(0), (4_800, 4_800));
    battle.units[0].hp = 2_000;
    assert!(battle.apply_debuff(1, 0, DebuffKind::AttackDown, 1_000, 2));
    act(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.units[0].hp, 2_734);
    assert_eq!(battle.hp(sprite), (2_400, 2_400));
    assert!(battle.units[0].debuffs.is_empty());
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.effect_resistance_bonus_bp(&battle, 0), 5_000);
    assert_eq!(
        mechanics.ally_crit_damage_bonus_bp(&battle, 0, sprite),
        2_000
    );
    ultimate(&mut battle);
    assert!(battle.dummy_damage.iter().all(|hit| hit.critical));
}

#[test]
fn e6_keeps_88_percent_and_applies_team_penetration_only_while_ica_is_present() {
    let prepared = plan(&[hyacine(6, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 0).resistance_down_bp,
        2_000
    );
    assert_eq!(
        rules
            .damage_target_aura(&battle, 0, sprite)
            .resistance_down_bp,
        2_000
    );
    // Initial ultimate healing is 1320, then E6 keeps 1162 and Ica's E1 attack adds 624.
    assert_eq!(battle.units[0].hyacine_state.healing_tally, 1_786);
    battle.remove_memosprite_of(0);
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 0).resistance_down_bp,
        0
    );
}

#[test]
fn fixed_dpr_credits_little_ica_and_team_hp_scaling_without_losing_damage() {
    let prepared = plan(&[hyacine(1, &[GLOOMY_GRIN]), castorice()]);
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(
        hits().any(|hit| hit.source_id == SPRITE
            && hit.credits.iter().any(|credit| credit.unit_id == FORM))
    );
    assert!(hits().any(|hit| {
        hit.source_id == CASTORICE && hit.credits.iter().any(|credit| credit.unit_id == FORM)
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

#[test]
fn ica_heals_before_castorice_resets_each_targets_overflow_cap() {
    let mut caster = castorice();
    caster.enabled_trace_ids.push("trace.1407101".into());
    let prepared = plan(&[caster, hyacine(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(1).unwrap();
    battle.heal_unit(1, 0, 4_080);
    battle.spend_ally_hp(1, 0, 500, 1);
    battle.notify_unit_action_finished(2);
    assert_eq!(battle.hp(0).0, 9_600);
    assert_eq!(battle.hp(sprite).0, 1_920);
    let last_newbud = |battle: &Battle<'_>| {
        battle
            .log
            .iter()
            .flatten()
            .rev()
            .find_map(|event| match event.kind {
                EventKind::Newbud { after, .. } => Some(after),
                _ => None,
            })
    };
    // 10200 initial, 1560 skill including overheal, 4080 capped direct healing, 580 HP loss.
    assert_eq!(last_newbud(&battle), Some(16_420));
    battle.heal_unit(1, 0, 100);
    assert_eq!(last_newbud(&battle), Some(16_520));
}
