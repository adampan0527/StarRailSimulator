use std::num::NonZeroU32;

use super::{BASIC, CONSTELLATION, FORM, IGNITE, SKILL, SPARKS, TALENT, ULTIMATE};
use crate::{
    Action, Battle, Command, DamageKind, LogMode, OfficialCharacter, OfficialStats, PreparedBattle,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

const ASTA: &str = "ally.asta";
const HIMEKO: &str = "ally.himeko";
const ARLAN: &str = "ally.arlan";

static NON_FIRE_WEAK: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn asta(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: ASTA.into(),
        form_id: FORM.into(),
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 400,
            base_speed_micros: 200_000_000,
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
            (SKILL.into(), vec![0.25]),
            (ULTIMATE.into(), vec![36.0, 2.0]),
            (TALENT.into(), vec![0.07, 5.0, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn himeko() -> OfficialCharacter {
    let mut input = asta(0, &[]);
    input.unit_id = HIMEKO.into();
    input.form_id = "character.1003".into();
    input.skill_parameters = [
        ("skill.100301".into(), vec![0.5]),
        ("skill.100302".into(), vec![1.0, 0.4]),
        ("skill.100303".into(), vec![1.38, 5.0]),
        ("skill.100304".into(), vec![0.7, 3.0]),
    ]
    .into();
    input
}

fn arlan() -> OfficialCharacter {
    let mut input = asta(0, &[]);
    input.unit_id = ARLAN.into();
    input.form_id = "character.1008".into();
    input.energy_capacity = 110;
    input.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    input
}

fn act(battle: &mut Battle<'_>, id: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == id
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
    command
}

#[test]
fn repeated_bounces_count_one_target_and_e1_adds_hit_energy_and_toughness_damage() {
    for (rank, hits, energy, toughness) in
        [(0, 5, 90_000, 90), (1, 6, 96_000, 85), (4, 6, 101_400, 85)]
    {
        let mut plan = PreparedBattle::official_dpr(&[asta(rank, &[]), himeko()], 42, 3).unwrap();
        plan.units[2].stats.hp = 0;
        plan.units[4].stats.hp = 0;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, ASTA, Action::Skill, Some("dpr:2"));

        assert_eq!(battle.units[0].asta_state.charge_stacks, 2);
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(battle.units[3].toughness, toughness);
        assert_eq!(battle.snapshot().skill_points, 2);
        assert_eq!(battle.dummy_damage.len(), hits);
        assert_eq!(battle.dummy_damage[0].amount, 150);
        assert!(battle.dummy_damage[1..].iter().all(|hit| hit.amount == 171));
    }
}

#[test]
fn non_fire_weak_target_grants_one_charge_and_cannot_enable_e4_by_itself() {
    let mut plan = PreparedBattle::official_dpr(&[asta(4, &[]), himeko()], 42, 3).unwrap();
    plan.units[2].stats.hp = 0;
    plan.units[4].stats.hp = 0;
    plan.units[3].kit = Kit::Enemy(&NON_FIRE_WEAK);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, ASTA, Action::Skill, Some("dpr:2"));

    assert_eq!(battle.units[0].asta_state.charge_stacks, 1);
    assert_eq!(battle.units[0].energy_milli, 96_000);
    assert_eq!(battle.units[3].toughness, 120);
}

#[test]
fn hits_on_three_fire_weak_targets_cap_charging_at_five_and_raise_defense_from_base() {
    let plan = PreparedBattle::official_dpr(&[asta(1, &[CONSTELLATION]), himeko()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.defense(0), 500);
    act(&mut battle, ASTA, Action::Skill, Some("dpr:2"));

    assert_eq!(battle.units[0].asta_state.hit_targets.len(), 3);
    assert_eq!(battle.units[0].asta_state.charge_stacks, 5);
    assert_eq!(battle.defense(0), 620);
}

#[test]
fn ultimate_accelerates_each_ally_and_e2_preserves_only_the_next_asta_turn() {
    for (rank, first_remaining, second_remaining) in [(0, 2, 1), (2, 5, 2), (6, 5, 3)] {
        let mut plan = PreparedBattle::official_dpr(&[asta(rank, &[]), himeko()], 42, 3).unwrap();
        plan.units[0].starting_energy_milli = 120_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].asta_state.charge_stacks = 5;
        act(&mut battle, ASTA, Action::Ultimate, None);
        assert_eq!(battle.snapshot().units[0].speed_micros, 236_000_000);
        assert_eq!(battle.snapshot().units[1].speed_micros, 236_000_000);
        assert_eq!(
            battle.units[0].energy_milli,
            if rank >= 4 { 5_750 } else { 5_000 }
        );

        act(&mut battle, ASTA, Action::Basic, Some("dpr:2"));
        act(&mut battle, HIMEKO, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.units[0].normal_turns, 2);
        assert_eq!(battle.units[0].asta_state.charge_stacks, first_remaining);
        assert!(!battle.units[0].asta_state.preserve_charge_on_next_turn);

        act(&mut battle, ASTA, Action::Basic, Some("dpr:2"));
        act(&mut battle, HIMEKO, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.units[0].normal_turns, 3);
        assert_eq!(battle.units[0].asta_state.charge_stacks, second_remaining);
        assert_eq!(battle.snapshot().units[1].speed_micros, 200_000_000);
    }
}

#[test]
fn sparks_ticks_three_times_using_half_the_basic_multiplier_and_current_attack() {
    let mut input = asta(0, &[SPARKS]);
    input.stats.effect_hit_rate_bp = 10_000;
    input.skill_parameters.insert(BASIC.into(), vec![0.7]);
    let plan = PreparedBattle::official_dpr(&[input, himeko()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, ASTA, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.snapshot().skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.dummy_damage[0].amount, 420);

    battle.tick_skill_dots(3);
    battle.units[0].asta_state.charge_stacks = 0;
    for _ in 0..3 {
        battle.tick_skill_dots(3);
    }
    let damage: Vec<_> = battle.dummy_damage[1..]
        .iter()
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    assert_eq!(
        damage,
        [
            (DamageKind::DamageOverTime, 239),
            (DamageKind::DamageOverTime, 210),
            (DamageKind::DamageOverTime, 210),
        ]
    );
}

#[test]
fn team_fire_and_non_fire_damage_credits_both_auras_to_the_living_asta() {
    for alive in [true, false] {
        let mut fire_ally = himeko();
        fire_ally.stats.base_attack = 800;
        let mut thunder_ally = arlan();
        thunder_ally.stats.base_attack = 900;
        let plan =
            PreparedBattle::official_dpr(&[asta(0, &[IGNITE]), fire_ally, thunder_ally], 42, 3)
                .unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, ASTA, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.dummy_damage[0].amount, 354);
        assert!(battle.dummy_damage[0].assists.is_empty());
        if !alive {
            battle.units[0].hp = 0;
        }
        act(&mut battle, HIMEKO, Action::Basic, Some("dpr:2"));
        act(&mut battle, ARLAN, Action::Basic, Some("dpr:2"));

        let (fire, thunder) = (&battle.dummy_damage[1], &battle.dummy_damage[2]);
        if alive {
            assert_eq!(fire.amount, 393);
            assert_eq!(fire.assists, [(0, 93)]);
            assert_eq!(thunder.amount, 337);
            assert_eq!(thunder.assists, [(0, 37)]);
        } else {
            assert_eq!(fire.amount, 300);
            assert!(fire.assists.is_empty());
            assert_eq!(thunder.amount, 300);
            assert!(thunder.assists.is_empty());
        }
    }
}

#[test]
fn automatic_dpr_uses_the_available_support_ultimate_and_replays() {
    let mut plan = PreparedBattle::official_dpr(&[asta(0, &[])], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let calculate = || calculate_dpr(&plan, &[1_000], NonZeroU32::new(1).unwrap()).unwrap();
    let first = calculate();
    let mut attack_times: Vec<_> = first.targets[0]
        .hits
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| hit.av_micros)
        .collect();
    attack_times.dedup();
    assert_eq!(attack_times[0], 50_000_000);
    assert!(attack_times[1] < 100_000_000);
    assert_eq!(first, calculate());
}
