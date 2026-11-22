use super::*;

use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget, GAUGE_FULL},
};

const SPRITE: &str = "character.1402:garmentmaker";
const MIDDLE: &str = "dpr:2";
const FIRST: &str = "dpr:1";
const HERTA: &str = "character.1013";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: LIGHTNING,
        },
        energy_capacity: 350,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
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
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0, 1.0]),
            (SKILL.into(), vec![0.5, 0.0, 5.0]),
            (ULTIMATE.into(), vec![0.15, 0.0, 0.0, 100.0, 0.0, 0.0]),
            (TALENT.into(), vec![0.3, 0.0, 0.0, 0.35, 0.66, 720.0]),
            (JOINT_BASIC.into(), vec![2.0, 0.9, 2.0, 0.9]),
            (GARMENTMAKER_SKILL.into(), vec![1.1, 0.66, 1.0]),
            (GARMENTMAKER_TALENT.into(), vec![55.0, 3.0, 6.0]),
            (GARMENTMAKER_ARRIVAL.into(), vec![1.0]),
            (GARMENTMAKER_DEPARTURE.into(), vec![20.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn herta() -> OfficialCharacter {
    let mut herta = character(0, &[]);
    herta.unit_id = HERTA.into();
    herta.form_id = HERTA.into();
    herta.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    herta.energy_capacity = 110;
    herta.stats.speed_micros = 110_000_000;
    herta.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    herta
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, FORM, Action::Ultimate, None);
}

#[test]
fn accepts_frozen_kit_and_rejects_incomplete_or_invalid_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut missing = character(0, &[]);
    missing.skill_parameters.remove(GARMENTMAKER_TALENT);
    assert!(PreparedBattle::official_dpr(&[missing], 42, 3).is_err());
    let mut invalid_countdown = character(0, &[]);
    invalid_countdown
        .skill_parameters
        .get_mut(ULTIMATE)
        .unwrap()[3] = 90.0;
    assert!(PreparedBattle::official_dpr(&[invalid_countdown], 42, 3).is_err());
}

#[test]
fn first_skill_summons_and_advances_both_and_repeated_skill_only_heals() {
    let prepared = plan(&[character(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.av_micros, 50_000_000);
    act(&mut battle, FORM, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.plan.units[sprite].id, SPRITE);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, 50_000_000);
    assert_eq!(battle.hp(sprite), (1_380, 1_380));
    assert_eq!(battle.speed_micros(sprite), 70_000_000);
    assert_eq!(battle.enemy_targeting_weight(sprite), 1_250_000);
    assert_eq!(battle.gauge_quanta(sprite), 0);
    assert_eq!(battle.units[0].energy_milli, 195_000);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[sprite].memosprite_generation, 1);

    battle.units[sprite].hp = 200;
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.hp(sprite), (890, 1_380));
    assert_eq!(battle.hp(0), (1_000, 1_000));
    assert_eq!(battle.units[sprite].memosprite_generation, 1);
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert_eq!(battle.units[0].energy_milli, 225_000);
    assert_eq!(battle.av_micros, 100_000_000);
}

#[test]
fn seam_moves_between_targets_and_sprite_targets_mark_and_gains_speed() {
    let prepared = plan(&[character(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 225_000);
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 1);
    assert_eq!(battle.speed_micros(sprite), 125_000_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.actor, hit.target, hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [
            (0, 2, DamageKind::Direct, 600),
            (0, 2, DamageKind::Additional, 180),
            (sprite, 2, DamageKind::Direct, 660),
            (sprite, 1, DamageKind::Direct, 396),
            (sprite, 3, DamageKind::Direct, 396),
        ]
    );
    assert_eq!(
        [
            battle.units[1].toughness,
            battle.units[2].toughness,
            battle.units[3].toughness
        ],
        [995, 980, 995]
    );
    act(&mut battle, FORM, Action::Basic, Some(FIRST));
    assert!(
        battle.units[1]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::SeamStitch)
    );
    assert!(
        !battle.units[2]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::SeamStitch)
    );
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 2);
}

#[test]
fn joint_attack_uses_two_blasts_one_action_and_neither_gains_nor_spends_sp() {
    let prepared = plan(&[character(1, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    cast_ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.units[0].summon_turn.as_mut().unwrap().gauge_quanta = GAUGE_FULL;
    let sp_before = battle.skill_points;
    let attacks_before = battle.attack_index;
    let events_before = battle.events().len();
    battle.dummy_damage.clear();
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.attack_index, attacks_before + 2);
    assert_eq!(battle.skill_points, sp_before);
    assert_eq!(battle.units[0].energy_milli, 65_000);
    assert_eq!(
        battle.events()[events_before..]
            .iter()
            .filter(|event| event.kind == EventKind::Action(Action::Basic))
            .count(),
        1
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.actor, hit.target, hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [
            (0, 2, DamageKind::Direct, 1_380),
            (0, 1, DamageKind::Direct, 540),
            (0, 3, DamageKind::Direct, 540),
            (0, 2, DamageKind::Additional, 207),
            (sprite, 2, DamageKind::Direct, 1_380),
            (sprite, 1, DamageKind::Direct, 540),
            (sprite, 3, DamageKind::Direct, 540),
        ]
    );
    assert_eq!(
        [
            battle.units[1].toughness,
            battle.units[2].toughness,
            battle.units[3].toughness
        ],
        [985, 960, 985]
    );
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 2);
    assert_eq!(battle.speed_micros(0), 230_000_000);
}

#[test]
fn stance_refreshes_its_own_countdown_and_expiry_removes_only_its_sprite() {
    let prepared = plan(&[character(0, &[MYOPICS_DOOM, LAST_THREAD])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|cmd| cmd.action != Action::Skill)
    );
    let order = battle.action_order();
    assert_eq!(order[0].unit_id, FORM);
    assert_eq!(order[1].unit_id, SPRITE);
    let timer = order
        .iter()
        .find(|entry| entry.unit_id == "summon:character.1402:supreme-stance")
        .unwrap();
    assert_eq!(timer.remaining_av_micros, 100_000_000);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert!(battle.units[0].aglaea_state.speed_stacks > 0);
    let remaining = battle.units[0].countdown.unwrap().gauge_quanta;
    assert!(remaining < GAUGE_FULL);
    let sprite_gauge = battle.gauge_quanta(sprite);
    battle.units[sprite].hp = 13;
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].countdown.unwrap().gauge_quanta, GAUGE_FULL);
    assert_eq!(battle.gauge_quanta(sprite), sprite_gauge);
    assert_eq!(battle.hp(sprite), (1_380, 1_380));
    assert_eq!(battle.units[sprite].memosprite_generation, 1);

    let expiry = battle.av_micros + 100_000_000;
    for _ in 0..8 {
        if !in_stance(&battle, 0) {
            break;
        }
        act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    }
    assert!(!in_stance(&battle, 0));
    assert!(battle.active_memosprite(0).is_none());
    assert!(battle.is_targetable(0));
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 1);
    assert!(battle.events().iter().any(|event| event.av_micros == expiry
        && event.target_id.as_deref() == Some(SPRITE)
        && event.kind == EventKind::UnitDefeated));
    assert!(
        battle
            .legal_actions()
            .iter()
            .any(|cmd| cmd.action == Action::Skill)
    );
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[sprite].memosprite_generation, 2);
    assert_eq!(battle.speed_micros(sprite), 125_000_000);
}

#[test]
fn finite_enemy_can_control_unprotected_sprite_but_lethal_hit_cancels_stance() {
    let mut prepared = plan(&[character(0, &[])]);
    prepared.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[1].stats.hp = 10_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert!(battle.apply_skill_freeze(1, sprite, 10_000, 1, 100, Action::Skill));
    assert!(battle.is_frozen(sprite));
    cast_ultimate(&mut battle);
    assert!(!battle.is_frozen(sprite));
    assert!(!battle.apply_skill_freeze(1, sprite, 10_000, 1, 100, Action::Skill));
    assert!(battle.apply_skill_freeze(1, 0, 10_000, 1, 100, Action::Skill));
    let energy = battle.units[0].energy_milli;
    battle.true_damage(1, sprite, Action::Basic, 9_999, Vec::new());
    assert_eq!(battle.units[0].energy_milli, energy + 20_000);
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 0);
    assert!(battle.active_memosprite(0).is_none());
    assert!(!in_stance(&battle, 0));
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::StatusExpired(StatusKind::SupremeStance))
            .count(),
        1
    );
}

#[test]
fn second_eidolon_counts_abilities_and_closes_on_other_units_active_action() {
    let prepared = plan(&[character(2, &[]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Off);
    let rules = battle.mechanics(0).unwrap();
    act(&mut battle, FORM, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(
        rules.damage_target_aura(&battle, 0, sprite).defense_down_bp,
        1_400
    );
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    // The dummy takes no active action and the sprite gets its own stack at the same AV.
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 0).defense_down_bp,
        4_200
    );
    assert_eq!(
        rules.damage_target_aura(&battle, 0, sprite).defense_down_bp,
        4_200
    );
    assert_eq!(rules.damage_target_aura(&battle, 0, 1).defense_down_bp, 0);
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    act(&mut battle, HERTA, Action::Basic, Some(MIDDLE));
    assert_eq!(
        rules.damage_target_aura(&battle, 0, sprite).defense_down_bp,
        0
    );
}

#[test]
fn speed_conversion_and_fourth_and_sixth_eidolon_use_live_individual_speed() {
    let prepared = plan(&[character(6, &[MYOPICS_DOOM, SPEEDING_SOL])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    battle.set_energy(0, 0, 100_000);
    rules.on_battle_start(&mut battle, 0);
    assert_eq!(battle.units[0].energy_milli, 175_000);
    cast_ultimate(&mut battle);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(
        rules.resistance_penetration_bp(&battle, 0, LIGHTNING),
        2_000
    );
    assert_eq!(
        rules.memosprite_resistance_penetration_bp(&battle, 0, LIGHTNING),
        2_000
    );
    assert_eq!(
        rules.memosprite_resistance_penetration_bp(&battle, 0, Element::Ice),
        0
    );
    assert_eq!(battle.effective_attack(0), 2_692);
    assert_eq!(battle.effective_attack(sprite), 2_692);
    assert_eq!(
        rules.damage_bonus_bp(&battle, 0, 2, Some(Action::Basic), DamageKind::Direct),
        1_000
    );
    assert_eq!(
        rules.memosprite_damage_bonus_bp(&battle, sprite, Some(Action::Basic), DamageKind::Direct),
        0
    );
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 3);
    assert_eq!(battle.speed_micros(0), 245_000_000);
    assert_eq!(battle.speed_micros(sprite), 235_000_000);
    assert_eq!(battle.effective_attack(sprite), 3_610);
    assert_eq!(
        rules.damage_bonus_bp(&battle, 0, 2, Some(Action::Basic), DamageKind::Direct),
        3_000
    );
    assert_eq!(
        rules.memosprite_damage_bonus_bp(&battle, sprite, Some(Action::Basic), DamageKind::Direct),
        1_000
    );
    assert_eq!(
        rules.memosprite_damage_bonus_bp(&battle, sprite, Some(Action::Skill), DamageKind::Direct),
        0
    );
    battle.units[0].aglaea_state.speed_stacks = 6;
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].aglaea_state.speed_stacks, 7);
    assert_eq!(
        rules.memosprite_damage_bonus_bp(&battle, sprite, Some(Action::Basic), DamageKind::Direct),
        6_000
    );
}

#[test]
fn fixed_dpr_replays_sprite_damage_and_credits_owner() {
    let prepared = PreparedBattle::official_dpr(
        &[
            character(6, &[MYOPICS_DOOM, LAST_THREAD, SPEEDING_SOL]),
            herta(),
        ],
        42,
        3,
    )
    .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let first = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let replay = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.targets.len(), 3);
    assert_eq!(first.characters.len(), 2);
    for hit in first
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .filter(|hit| hit.source_id == SPRITE)
    {
        assert_eq!(hit.credits.len(), 1);
        assert_eq!(hit.credits[0].unit_id, FORM);
        assert_eq!(hit.credits[0].amount, hit.amount);
    }
    assert!(
        first
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.source_id == SPRITE && hit.action == Action::Basic)
    );
    assert!(
        first
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.source_id == SPRITE && hit.action == Action::Skill)
    );
    assert_eq!(
        first.total_damage,
        first
            .characters
            .iter()
            .map(|character| character.own_damage + character.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(
        first.total_damage,
        first
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>()
    );
}
