use crate::{
    CombatType, Command, DamageKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    calculate_dpr,
};

use super::*;

const ARGENTI: &str = "character.1302";
const MIDDLE: &str = "dpr:2";

pub(in crate::battle::official) fn yao(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Elation,
            element: PHYSICAL,
        },
        energy_capacity: 180,
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
            elation_bp: 0,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.9, 0.3]),
            (SKILL.into(), vec![3.0, 0.2, 3.0]),
            (ULTIMATE.into(), vec![5.0, 0.2, 3.0, 20.0, 1.0]),
            (TALENT.into(), vec![0.2]),
            (ELATION_SKILL.into(), vec![1.0, 1.0, 0.16, 3.0, 5.0, 0.2]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn argenti(elation_bp: u32) -> OfficialCharacter {
    let mut actor = yao(0, &[]);
    actor.unit_id = ARGENTI.into();
    actor.form_id = ARGENTI.into();
    actor.energy_capacity = 180;
    actor.combat_type.path = CombatPath::Erudition;
    actor.stats.speed_micros = 190_000_000;
    actor.stats.elation_bp = elation_bp;
    actor.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    actor
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        actor_id: actor.into(),
        action,
        decision_index: battle.decision_index,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, FORM, Action::Ultimate, None);
}

fn aha_instants(battle: &Battle<'_>) -> Vec<(u64, u32, bool)> {
    battle
        .events()
        .iter()
        .filter_map(|event| {
            let EventKind::AhaInstant { punchline, extra } = event.kind else {
                return None;
            };
            Some((event.av_micros, punchline, extra))
        })
        .collect()
}

fn shared_punchline(battle: &Battle<'_>) -> u32 {
    battle
        .events()
        .iter()
        .rev()
        .find_map(|event| {
            let EventKind::Punchline { after, .. } = event.kind else {
                return None;
            };
            Some(after)
        })
        .unwrap_or_default()
}

#[test]
fn recognizes_base_and_accepts_low_frozen_skill_levels() {
    let mut input = yao(0, &[]);
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    input
        .skill_parameters
        .get_mut(BASIC)
        .unwrap()
        .copy_from_slice(&[0.45, 0.15]);
    input
        .skill_parameters
        .get_mut(ELATION_SKILL)
        .unwrap()
        .copy_from_slice(&[1.0, 0.5, 0.16, 3.0, 5.0, 0.1]);
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_ok());
    let mut input = yao(0, &[]);
    input.skill_parameters.get_mut(ELATION_SKILL).unwrap()[4] = 1.0;
    assert!(PreparedBattle::official_dpr(&[input], 42, 3).is_err());
}

#[test]
fn natural_aha_consumes_the_shared_pool_and_grants_separate_banger_duration() {
    let prepared = plan(&[yao(0, &[FELICITY_ENSEMBLE, POISED_AND_SATED])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.av_micros, 50_000_000);
    assert_eq!(battle.aha_speed_micros(), Some(120_000_000));
    assert_eq!(battle.certified_banger(0), Some(0));
    assert_eq!(shared_punchline(&battle), 1);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(aha_instants(&battle), [(83_333_334, 4, false)]);
    assert_eq!(shared_punchline(&battle), 1);
    assert_eq!(battle.certified_banger(0), Some(4));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 125_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .take(3)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [5_674; 3]
    );
    let total_toughness: u16 = battle
        .enemies()
        .map(|enemy| battle.units[enemy].toughness)
        .sum();
    assert_eq!(total_toughness, 3_000 - 65);
    assert!(battle.enemies().all(|enemy| {
        battle.units[enemy]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::WoesWhisper && debuff.bp == 1_600)
    }));

    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(
        aha_instants(&battle),
        [(83_333_334, 4, false), (166_666_668, 7, false)]
    );
    assert_eq!(battle.certified_banger(0), Some(11));
}

#[test]
fn ultimate_extra_keeps_the_normal_gauge_and_payment_and_uses_fixed_punchline() {
    let prepared = plan(&[yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let gauge = battle.aha.as_ref().unwrap().gauge_quanta;
    cast_ultimate(&mut battle);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, 50_000_000);
    assert_eq!(battle.aha.as_ref().unwrap().gauge_quanta, gauge);
    assert_eq!(aha_instants(&battle), [(50_000_000, 20, true)]);
    assert_eq!(shared_punchline(&battle), 7);
    assert_eq!(battle.certified_banger(0), Some(20));
    assert_eq!(battle.units[0].energy_milli, 10_000);
    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 9);
    assert!(hits.iter().all(|hit| hit.kind == DamageKind::Elation
        && hit.actor == 0
        && hit.action == Action::ElationSkill));
    assert_eq!(
        hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [
            8_713, 8_713, 8_713, 1_742, 1_742, 1_742, 1_742, 1_742, 1_258
        ]
    );
    assert!(hits[3..8].iter().all(|hit| hit.target == hits[3].target));

    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(
        aha_instants(&battle),
        [(50_000_000, 20, true), (83_333_334, 7, false)]
    );
    assert_eq!(battle.certified_banger(0), Some(27));
}

#[test]
fn each_certified_banger_lease_expires_at_its_own_ordinary_turn() {
    let prepared = plan(&[yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    assert_eq!(battle.certified_banger(0), Some(20));
    battle.units[0].normal_turns = 2;
    battle.expire_certified_banger(0);
    assert_eq!(battle.certified_banger(0), Some(20));
    battle.units[0].normal_turns = 3;
    battle.expire_certified_banger(0);
    assert_eq!(battle.certified_banger(0), None);
}

#[test]
fn skill_zone_and_trace_use_live_speed_without_feedback() {
    let prepared = plan(&[yao(2, &[AMAZE_IN_GRACE]), argenti(0)]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(battle.intrinsic_elation_bp(0), 11_000);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.speed_micros(0), 212_000_000);
    assert_eq!(battle.speed_micros(1), 202_000_000);
    assert_eq!(battle.aha_speed_micros(), Some(122_400_000));
    assert_eq!(rules.ally_elation_bp(&battle, 0, 1), 4_040);
    battle.units[0].normal_turns = 4;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(battle.aha_speed_micros(), Some(120_000_000));
    assert_eq!(rules.ally_elation_bp(&battle, 0, 1), 0);
}

#[test]
fn great_boon_records_two_distinct_hits_for_one_paid_attack_without_recursion() {
    let mut input = yao(0, &[]);
    input.stats.elation_bp = 6_000;
    let prepared = plan(&[input, argenti(1_000)]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    battle.dummy_damage.clear();
    act(&mut battle, ARGENTI, Action::Skill, None);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Elation && hit.actor == 1)
        .collect();
    assert_eq!(hits.len(), 2);
    assert!(
        hits.iter()
            .all(|hit| hit.actor == 1 && hit.assists == [(0, hit.amount)])
    );
    assert_eq!(hits[0].amount, 1_555);
    assert_eq!(hits[1].amount, 1_555);
}

#[test]
fn elation_applies_eidolon_multipliers_without_ordinary_attack_or_damage_bonus() {
    let mut input = yao(6, &[]);
    input.stats.attack = 99_999;
    input.stats.element_damage_bp[PHYSICAL.index()] = 90_000;
    let prepared = plan(&[input]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    assert_eq!(aha_instants(&battle), [(50_000_000, 40, true)]);
    assert_eq!(battle.dummy_damage[0].amount, 43_346);
    assert_eq!(battle.dummy_damage[3].amount, 8_669);
    assert_eq!(battle.certified_banger(0), Some(40));
}

#[test]
fn aha_cleanses_control_before_the_automatic_elation_skill() {
    let prepared = plan(&[yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(1, 0, 10_000, 1, 100, Action::Skill));
    assert!(battle.is_frozen(0));
    battle.queue_aha_extra_turn(0, 20);
    battle.resolve_aha_extra_turn();
    assert!(!battle.is_frozen(0));
    let events = battle.events();
    let removed = events
        .iter()
        .position(|event| matches!(event.kind, EventKind::StatusCleared))
        .unwrap();
    let skill = events
        .iter()
        .position(|event| matches!(event.kind, EventKind::Action(Action::ElationSkill)))
        .unwrap();
    assert!(removed < skill);
}

#[test]
fn no_elation_character_adds_no_timeline_slot_or_random_draw() {
    let plan_a = plan(&[argenti(0)]);
    let plan_b = plan(&[argenti(9_999)]);
    let mut baseline = Battle::start(&plan_a, LogMode::Detailed);
    let mut with_stat = Battle::start(&plan_b, LogMode::Detailed);
    for battle in [&mut baseline, &mut with_stat] {
        assert!(battle.aha.is_none());
        assert!(
            battle
                .action_order()
                .iter()
                .all(|entry| entry.unit_id != "summon:aha")
        );
        act(battle, ARGENTI, Action::Basic, Some(MIDDLE));
        act(battle, ARGENTI, Action::Skill, None);
    }
    assert_eq!(baseline.events(), with_stat.events());
    assert_eq!(
        baseline.snapshot().random_draws,
        with_stat.snapshot().random_draws
    );
}

#[test]
fn fixed_dpr_replays_and_credits_yao_guang_for_teammates_elation_hits() {
    let prepared = PreparedBattle::official_dpr(
        &[
            yao(6, &[AMAZE_IN_GRACE, POISED_AND_SATED, FELICITY_ENSEMBLE]),
            argenti(0),
        ],
        42,
        3,
    )
    .unwrap();
    let run = || {
        calculate_dpr(
            &prepared,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(result.characters[0].assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| target.hits.iter())
            .any(|hit| hit.source_id == ARGENTI
                && hit.kind == DamageKind::Elation
                && hit.credits.iter().any(|credit| credit.unit_id == FORM))
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
