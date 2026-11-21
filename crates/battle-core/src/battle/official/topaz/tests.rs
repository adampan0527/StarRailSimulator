use std::num::NonZeroU32;

use crate::{
    Action, BattleError, Command, DamageKind, DprScenario, EventKind, LogMode, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Battle, Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

use super::{
    BASIC, FINANCIAL_TURMOIL, FORM, OVERDRAFT, OfficialCharacter, SKILL, STONKS_MARKET, TALENT,
    ULTIMATE,
};

const TOPAZ: &str = "ally.topaz";
const SERVAL: &str = "ally.serval";
const NUMBY: &str = "summon:ally.topaz:numby";
const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const RIGHT: &str = "dpr:3";
const FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn topaz(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: TOPAZ.into(),
        form_id: FORM.into(),
        energy_capacity: 130,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 110_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.75, 0.25]),
            (ULTIMATE.into(), vec![0.75, 0.125, 0.5, 2.0]),
            (TALENT.into(), vec![80.0, 0.75, 0.5]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn serval() -> OfficialCharacter {
    let mut ally = topaz(0, &[]);
    ally.unit_id = SERVAL.into();
    ally.form_id = "character.1103".into();
    ally.energy_capacity = 100;
    ally.stats.base_aggro = 100;
    ally.skill_parameters = [
        ("skill.110301".into(), vec![0.5]),
        ("skill.110302".into(), vec![0.7, 0.3, 0.8, 2.0, 0.4]),
        ("skill.110303".into(), vec![1.08, 2.0]),
        ("skill.110304".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap_or_else(|| panic!("expected {actor} {action:?} targeting {target:?}"));
    battle.apply(&command).unwrap();
    command
}

fn numby_remaining(battle: &Battle<'_>) -> Option<u64> {
    battle
        .action_order()
        .iter()
        .find(|entry| entry.unit_id == NUMBY)
        .map(|entry| entry.remaining_av_micros)
}

fn follow_up_hits<'b>(battle: &'b Battle<'_>, actor: usize) -> Vec<&'b DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == actor && hit.kind == DamageKind::FollowUp)
        .collect()
}

#[test]
fn numby_acts_at_125_av_independently_and_does_not_advance_itself() {
    let plan = prepared(&[topaz(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().av_micros, 50_000_000);
    assert_eq!(numby_remaining(&battle), Some(75_000_000));
    let first = act(&mut battle, TOPAZ, Action::Basic, Some(CENTER));
    assert_eq!(numby_remaining(&battle), Some(25_000_000));
    let second = act(&mut battle, TOPAZ, Action::Basic, Some(CENTER));

    let hits = follow_up_hits(&battle, 0);
    assert_eq!(hits.len(), 1);
    // 1000 ATK × 0.75 × (1000/1500) × 0.9 × 1.25 = 562.5.
    assert_eq!((hits[0].av_micros, hits[0].amount), (125_000_000, 562));
    assert_eq!(numby_remaining(&battle), Some(100_000_000));
    assert_eq!(battle.snapshot().units[0].normal_turns, 3);
    assert_eq!(
        (battle.skill_points, battle.units[0].energy_milli),
        (5, 105_000)
    );
    assert_eq!(battle.ordinary_actor(), Some(TOPAZ));
    let replay = Battle::replay(&plan, &[first, second], LogMode::Detailed).unwrap();
    assert_eq!(replay.snapshot(), battle.snapshot());
    assert_eq!(replay.events(), battle.events());
}

#[test]
fn skill_marks_before_damage_advances_numby_and_preserves_its_own_energy() {
    let plan = prepared(&[topaz(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Skill, Some(CENTER));

    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 2);
    assert_eq!(
        (hits[0].kind, hits[0].target, hits[0].amount),
        (DamageKind::Direct, 2, 562)
    );
    assert_eq!(
        (hits[1].kind, hits[1].target, hits[1].amount),
        (DamageKind::FollowUp, 2, 562)
    );
    assert_eq!(hits[1].av_micros, 62_500_000);
    assert_eq!(battle.units[2].toughness, 360);
    assert_eq!(
        (battle.skill_points, battle.units[0].energy_milli),
        (2, 95_000)
    );
    assert_eq!(numby_remaining(&battle), Some(87_500_000));
}

#[test]
fn enhanced_skill_consumes_a_charge_and_both_advances_apply_before_refund() {
    let mut plan = prepared(&[topaz(0, &[STONKS_MARKET])]);
    plan.units[0].starting_energy_milli = 130_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[0].topaz_state.enhanced_attacks, 2);
    act(&mut battle, TOPAZ, Action::Skill, Some(CENTER));

    assert_eq!(battle.units[0].topaz_state.enhanced_attacks, 0);
    assert_eq!(battle.units[0].energy_milli, 55_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.av_micros, hit.amount))
            .collect::<Vec<_>>(),
        [(50_000_000, 1125), (50_000_000, 1125)]
    );
    assert_eq!(numby_remaining(&battle), Some(75_000_000));
}

#[test]
fn eidolon_six_adds_charge_and_penetration_and_only_autonomous_attacks_trigger_e2_and_e4() {
    let mut caster = topaz(6, &[OVERDRAFT, STONKS_MARKET]);
    caster.stats.crit_rate_bp = 10_000;
    let mut plan = prepared(&[caster]);
    plan.units[0].starting_energy_milli = 130_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Ultimate, None);
    assert_eq!(battle.units[0].topaz_state.enhanced_attacks, 3);
    act(&mut battle, TOPAZ, Action::Skill, Some(CENTER));

    assert_eq!(battle.units[0].topaz_state.enhanced_attacks, 1);
    assert_eq!(battle.units[0].energy_milli, 60_000);
    assert_eq!(battle.av_micros, 90_000_000);
    // 1500 × 0.6 × 1.25 × 1.10 × (1 + .5 + .125 + E1 stacks × .25).
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [2320, 2629]
    );
    act(&mut battle, TOPAZ, Action::Basic, Some(CENTER));

    assert_eq!(battle.units[0].topaz_state.enhanced_attacks, 0);
    assert_eq!(battle.units[0].energy_milli, 95_000);
    assert_eq!(battle.av_micros, 130_000_000);
    assert_eq!(battle.dummy_damage[2].amount, 750);
    assert_eq!(
        (
            battle.dummy_damage[3].amount,
            battle.dummy_damage[3].av_micros
        ),
        (2629, 90_000_000)
    );
}

#[test]
fn proof_of_debt_credits_only_follow_up_and_stacks_debtor_once_for_multiple_hits() {
    let mut teammate = serval();
    teammate.stats.crit_rate_bp = 10_000;
    let plan = prepared(&[topaz(1, &[]), teammate]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Skill, Some(CENTER));
    assert_eq!(battle.units[0].topaz_state.proof.unwrap().debtor_stacks, 1);
    assert_eq!(battle.debuff_count(3), 2);
    let attack = super::FollowUpAttack {
        element: Element::Physical,
        multiplier_bp: 10_000,
        toughness: 0,
        energy_milli: 0,
        first_hit: Some((5_000, 0)),
    };
    battle.follow_up_single(1, 3, Action::Basic, attack);
    battle.resolve_follow_ups();

    let hits = follow_up_hits(&battle, 1);
    assert_eq!(hits.len(), 2);
    for hit in hits {
        // 500 × 0.6 × 2 × 1.25 = 750, without either effect 500 × 0.6 × 1.5 = 450.
        assert_eq!((hit.amount, hit.assists.as_slice()), (750, &[(0, 300)][..]));
    }
    assert_eq!(battle.units[0].topaz_state.proof.unwrap().debtor_stacks, 2);
    assert_eq!(numby_remaining(&battle), Some(0));

    act(&mut battle, SERVAL, Action::Basic, Some(CENTER));
    let own_basic = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 1 && hit.kind == DamageKind::Direct)
        .unwrap();
    assert_eq!(
        (own_basic.amount, own_basic.assists.as_slice()),
        (450, &[][..])
    );
    act(&mut battle, TOPAZ, Action::Skill, Some(RIGHT));
    let proof = battle.units[0].topaz_state.proof.unwrap();
    assert_eq!((proof.target, proof.debtor_stacks), (4, 1));
    assert_eq!(battle.debuff_count(3), 0);
    assert_eq!(battle.debuff_count(4), 2);
}

#[test]
fn proof_reselects_a_living_enemy_when_the_marked_enemy_is_defeated() {
    let mut plan = prepared(&[topaz(1, &[])]);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let first = act(&mut battle, TOPAZ, Action::Skill, Some(CENTER));

    assert_eq!(battle.units[2].hp, 0);
    let hit = follow_up_hits(&battle, 0)[0];
    assert!(matches!(hit.target, 1 | 3));
    assert_eq!(hit.av_micros, 62_500_000);
    assert_eq!(
        battle.units[0].topaz_state.proof.unwrap().target,
        hit.target
    );
    let replay = Battle::replay(&plan, &[first], LogMode::Detailed).unwrap();
    assert_eq!(battle.events(), replay.events());
}

#[test]
fn numby_disappears_when_topaz_is_defeated() {
    let mut plan = prepared(&[topaz(0, &[]), serval()]);
    plan.units[3].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[3].stats.attack = 100_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..2 {
        act(&mut battle, TOPAZ, Action::Basic, Some(LEFT));
        act(&mut battle, SERVAL, Action::Basic, Some(LEFT));
    }
    assert_eq!(battle.units[0].hp, 0);
    assert!(battle.units[0].summon_turn.is_none());
    assert_eq!(numby_remaining(&battle), None);
    assert!(follow_up_hits(&battle, 0).is_empty());
}

#[test]
fn financial_turmoil_increases_damage_only_against_fire_weakness() {
    let mut plan = prepared(&[topaz(0, &[FINANCIAL_TURMOIL])]);
    plan.units[3].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, TOPAZ, Action::Basic, Some(CENTER));
    act(&mut battle, TOPAZ, Action::Basic, Some(RIGHT));
    // 500 × 0.6 = 300, and a 15% bonus against the Fire-weak center dummy makes it 345.
    let amount = |id| {
        battle.events().iter().find_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::Direct,
                amount,
                ..
            } if event.target_id.as_deref() == Some(id) => Some(amount),
            _ => None,
        })
    };
    assert_eq!(battle.dummy_damage[0].amount, 345);
    assert_eq!(amount(RIGHT), Some(300));
}

#[test]
fn dpr_records_summon_damage_before_the_first_player_decision() {
    let mut caster = topaz(0, &[]);
    caster.stats.base_speed_micros = 50_000_000;
    caster.stats.speed_micros = 50_000_000;
    let plan = prepared(&[caster]);
    let scenario = DprScenario {
        cycles: NonZeroU32::new(1).unwrap(),
        toughness: [400; 3],
    };
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let hits: Vec<_> = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .collect();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        (
            hits[0].av_micros,
            hits[0].kind,
            hits[0].source_id.as_str(),
            result.dpr
        ),
        (125_000_000, DamageKind::FollowUp, TOPAZ, 562.0)
    );
}

#[test]
fn fixed_dpr_replays_deterministically_and_accounts_for_every_hit() {
    let plan = prepared(&[
        topaz(0, &[OVERDRAFT, FINANCIAL_TURMOIL, STONKS_MARKET]),
        serval(),
    ]);
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    let replay = run();
    assert_eq!(result, replay);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.source_id == TOPAZ && hit.kind == DamageKind::FollowUp)
    );
    let credited: u64 = result
        .characters
        .iter()
        .map(|entry| entry.own_damage + entry.assist_damage)
        .sum();
    assert_eq!(credited, result.total_damage);
    assert_eq!(result.cycles, 3);
}

#[test]
fn invalid_scheduler_parameters_fail_before_battle_starts() {
    let mut caster = topaz(0, &[]);
    caster.skill_parameters.get_mut(TALENT).unwrap()[0] = 0.0;
    assert!(matches!(
        PreparedBattle::official_dpr(&[caster], 42, 3),
        Err(BattleError::InvalidInput(_))
    ));
}
