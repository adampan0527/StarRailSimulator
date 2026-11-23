use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const CARRY: &str = "character.1106";
const FIRST: &str = "dpr:1";
const SECOND: &str = "dpr:2";

pub(in crate::battle::official) fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: WIND,
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
            base_speed_micros: 100_000_000,
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
            (SKILL.into(), vec![0.72, 1.0, 8.0, 6.0, 0.1]),
            (ULTIMATE.into(), vec![2.4, 2.0]),
            (TALENT.into(), vec![1.0, 0.24, 0.60, 20.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn carry() -> OfficialCharacter {
    let mut ally = character(0, &[]);
    ally.unit_id = CARRY.into();
    ally.form_id = CARRY.into();
    ally.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Ice,
    };
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 150_000_000;
    ally.stats.crit_rate_bp = 10_000;
    ally.skill_parameters = [
        ("skill.110601".into(), vec![0.50]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.30, 2.0, 0.60]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 2])
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

pub(in crate::battle::official) fn grant_merit(
    battle: &mut Battle<'_>,
    owner: usize,
    ally: usize,
    charge: u8,
) {
    set_holder(battle, owner, ally);
    set_charge(battle, owner, charge);
}

pub(in crate::battle::official) fn charge(battle: &Battle<'_>, owner: usize) -> u8 {
    battle.units[owner].cerydra_state.charge
}

#[test]
fn accepts_frozen_low_level_rows_and_rejects_an_incompatible_cap() {
    let mut input = character(0, &[]);
    input
        .skill_parameters
        .insert(SKILL.into(), vec![0.585, 1.0, 8.0, 6.0, 0.0925]);
    input
        .skill_parameters
        .insert(TALENT.into(), vec![1.0, 0.2175, 0.4875, 20.0]);
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(SKILL).unwrap()[2] = 7.0;
    assert!(build(&input).is_err());
}

#[test]
fn peerage_caps_charge_dispels_control_and_remarking_resets_it() {
    let prepared = plan(&[character(1, &[VICI]), carry()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let cura = battle.mechanics(0).unwrap();
    let before_energy = battle.units[1].energy_milli;
    act(&mut battle, FORM, Action::Skill, Some(CARRY));
    assert_eq!(charge(&battle, 0), 1);
    assert_eq!(battle.units[1].energy_milli, before_energy + 2_000);
    assert_eq!(battle.effective_attack(1), 1_240);
    assert_eq!(battle.speed_micros(1), 170_000_000);
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 100, Action::Skill));
    assert!(battle.is_frozen(1));
    gain_charge(&mut battle, 0, 10);
    assert_eq!(charge(&battle, 0), 8);
    assert!(!battle.is_frozen(1));
    cura.before_action(&mut battle, 0, Action::Skill, Some(0));
    assert_eq!(charge(&battle, 0), 1);
    assert_eq!(battle.effective_attack(1), 1_000);
    assert_eq!(battle.effective_attack(0), 1_240);
    assert_eq!(battle.speed_micros(0), 220_000_000);
}

#[test]
fn copy_and_original_share_peerage_and_use_one_command_and_skill_point() {
    let prepared = plan(&[character(0, &[]), carry()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(CARRY));
    set_charge(&mut battle, 0, 8);
    battle.set_energy(1, 1, 0);
    let av = battle.av_micros;
    let turns = battle.units[1].normal_turns;
    let decision = battle.decision_index;
    act(&mut battle, CARRY, Action::Skill, Some(FIRST));
    assert_eq!(battle.decision_index, decision + 1);
    assert_eq!(battle.skill_points, 1);
    assert_eq!(battle.units[1].normal_turns, turns);
    assert_eq!(battle.units[1].energy_milli, 60_000);
    assert_eq!(battle.units[2].toughness, 960);
    assert_eq!(charge(&battle, 0), 2);
    let hits = &battle.dummy_damage;
    assert_eq!(
        hits.iter()
            .map(|hit| (hit.actor, hit.kind, hit.amount, hit.av_micros))
            .collect::<Vec<_>>(),
        [
            (1, DamageKind::Direct, 1_907, av),
            (0, DamageKind::Additional, 360, av),
            (1, DamageKind::Direct, 1_907, av),
            (0, DamageKind::Additional, 360, av),
        ]
    );
    assert_eq!(hits[0].assists, [(0, 1_907)]);
    assert_eq!(hits[2].assists, [(0, 962)]);
}

#[test]
fn skill_that_fills_six_does_not_copy_until_the_next_cast() {
    let prepared = plan(&[character(0, &[VICI]), carry()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(CARRY));
    set_charge(&mut battle, 0, 5);
    let before = battle.units[0].energy_milli;
    act(&mut battle, CARRY, Action::Skill, Some(FIRST));
    assert_eq!(charge(&battle, 0), 6);
    assert_eq!(battle.units[0].energy_milli, before + 5_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 1)
            .count(),
        1
    );
}

#[test]
fn copy_defeating_a_finite_target_retargets_the_original_skill() {
    static ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 1_000,
        weakness: [true; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[character(0, &[]), carry()]);
    for enemy in 2..=3 {
        prepared.units[enemy].kit = Kit::Enemy(&ENEMY);
    }
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(CARRY));
    set_charge(&mut battle, 0, 6);
    battle.units[2].hp = 100;
    battle.units[3].hp = 10_000;
    act(&mut battle, CARRY, Action::Skill, Some(FIRST));
    assert_eq!(battle.units[2].hp, 0);
    assert!(battle.units[3].hp < 10_000);
    assert_eq!(charge(&battle, 0), 0);
    assert!(battle.log.iter().flatten().any(|event| {
        event.kind == EventKind::TriggeredSkill && event.target_id.as_deref() == Some(FIRST)
    }));
}

#[test]
fn anaxa_copy_keeps_two_extra_skills_and_one_paid_skill_refund() {
    let anaxa = super::super::anaxa::tests::anaxa(1, &[]);
    let anaxa_id = anaxa.unit_id.clone();
    let prepared = plan(&[character(0, &[]), anaxa]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(&anaxa_id));
    set_charge(&mut battle, 0, 6);
    battle.set_energy(1, 1, 0);
    act(&mut battle, &anaxa_id, Action::Skill, Some(SECOND));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[1].energy_milli, 120_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 1)
            .count(),
        20
    );
    assert_eq!(
        battle
            .log
            .iter()
            .flatten()
            .filter(|event| event.kind == EventKind::TriggeredSkill)
            .count(),
        3
    );
    assert_eq!(charge(&battle, 0), 0);
}

#[test]
fn ultimate_assigns_first_slot_and_resets_twenty_additional_attacks() {
    let prepared = plan(&[carry(), character(0, &[VIDI])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.fill_energy(1);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(holder(&battle, 1), Some(0));
    assert_eq!(charge(&battle, 1), 2);
    let rules = battle.mechanics(1).unwrap();
    rules.on_ally_action_finished(&mut battle, 1, 0, Action::Ultimate);
    rules.on_ally_action_finished(&mut battle, 1, 0, Action::Ultimate);
    assert_eq!(charge(&battle, 1), 3);
    for _ in 0..21 {
        rules.on_ally_attack(&mut battle, 1, 0, Action::Basic, Some(2));
    }
    assert_eq!(battle.units[1].cerydra_state.additional_used, 20);
    battle.fill_energy(1);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[1].cerydra_state.additional_used, 0);
}

#[test]
fn eidolons_apply_distinct_skill_bonuses_and_raise_ultimate_and_additional_multipliers() {
    let mut leader = character(6, &[VENI, VIDI]);
    leader.stats.attack = 2_500;
    let prepared = plan(&[leader, carry()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    grant_merit(&mut battle, 0, 1, 6);
    let rules = battle.mechanics(0).unwrap();
    let aura = rules.damage_target_aura(&battle, 0, 1);
    assert_eq!(aura.resistance_down_bp, 3_000);
    assert_eq!(aura.defense_down_bp, 1_600);
    assert_eq!(rules.ally_skill_defense_ignore_bp(&battle, 0, 1), 2_000);
    assert_eq!(rules.ally_skill_crit_damage_bp(&battle, 0, 1), 7_200);
    assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 0, WIND), 16_000);
    assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 1, WIND), 4_000);
    assert_eq!(
        rules.crit_damage_bonus_bp(&battle, 0, 2, Some(Action::Ultimate), DamageKind::Direct),
        9_000
    );
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, None);
    rules.on_ally_attack(&mut battle, 0, 1, Action::Basic, Some(2));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.kind, hit.amount, hit.critical))
            .collect::<Vec<_>>(),
        [
            (DamageKind::Direct, 53_913, true),
            (DamageKind::Direct, 53_913, true),
            (DamageKind::Additional, 40_435, true),
        ]
    );
}

#[test]
fn replay_conserves_copy_provenance_and_damage_attribution() {
    let prepared = plan(&[character(6, &[VENI, VIDI, VICI]), carry()]);
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.kind == DamageKind::Additional));
    assert!(hits().any(|hit| {
        hit.source_id == CARRY
            && hit.action == Action::Skill
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == FORM && credit.amount == hit.amount)
    }));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
