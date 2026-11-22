use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const LEFT: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";
const ALL_TRACES: &[&str] = &[A2, A4, A6];

static ENEMY: EnemyKit = EnemyKit {
    stats: crate::fixture::Stats {
        level: 80,
        attack: 1_000,
        ..DUMMY_STATS
    },
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn yunli(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: Element::Physical,
        },
        energy_capacity: ENERGY_CAP,
        level: 80,
        stats: OfficialStats {
            hp: 10_000,
            base_hp: 10_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 94_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 1_000,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![1.2, 0.6, 0.3, 200.0]),
            (
                ULTIMATE.into(),
                vec![2.2, 1.0, 0.0, 6.0, 0.0, 1.1, 0.72, 120.0],
            ),
            (TALENT.into(), vec![1.2, 0.6, 15.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn boothill() -> OfficialCharacter {
    let mut ally = yunli(0, &[]);
    ally.unit_id = "character.1315".into();
    ally.form_id.clone_from(&ally.unit_id);
    ally.combat_type.path = CombatPath::Hunt;
    ally.energy_capacity = 115;
    ally.stats.speed_micros = 250_000_000;
    ally.skill_parameters = [
        ("skill.131501".into(), vec![1.0]),
        ("skill.131502".into(), vec![0.3, 0.15, 2.0]),
        ("skill.131503".into(), vec![4.0, 0.4, 2.0]),
        ("skill.131504".into(), vec![0.7, 1.2, 1.7, 0.5, 3.0, 16.0]),
        ("skill.131508".into(), vec![2.2]),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter], count: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&vec![120; count])
        .unwrap()
}

fn finite(plan: &mut PreparedBattle, target: usize, hp: u32) {
    plan.units[target].kit = Kit::Enemy(&ENEMY);
    plan.units[target].stats = crate::fixture::Stats {
        hp,
        base_hp: hp,
        ..ENEMY.stats
    };
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

fn follow_up_hits<'a>(battle: &'a Battle<'_>) -> Vec<&'a crate::Event> {
    battle
        .events()
        .iter()
        .filter(|event| {
            event.source_id.as_deref() == Some(FORM)
                && matches!(
                    event.kind,
                    EventKind::Damage {
                        kind: DamageKind::FollowUp,
                        ..
                    }
                )
        })
        .collect()
}

#[test]
fn yunli_basic_and_skill_apply_resources_and_heal_before_blast() {
    let plan = prepared(&[yunli(0, &[])], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 8_000;
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.units[0].hp, 8_550);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 150_000);
    assert_eq!(
        battle.units[1..]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [110, 100, 110]
    );
    let first_effect = battle.events().iter().find_map(|event| match event.kind {
        EventKind::Heal { .. } | EventKind::Damage { .. } => Some(event.kind),
        _ => None,
    });
    assert_eq!(first_effect, Some(EventKind::Heal { amount: 550 }));
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[1].toughness, 100);
    assert_eq!(battle.units[0].energy_milli, 170_000);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn yunli_uses_half_energy_and_expires_after_an_extra_turn() {
    let plan = prepared(&[yunli(0, &[A2])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 119_999);
    let rejected = command(&battle, FORM, Action::Ultimate, None);
    assert_eq!(
        battle.apply(&rejected),
        Err(BattleError::InsufficientEnergy)
    );
    battle.set_energy(0, 0, 240_000);
    let av = battle.av_micros;
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 125_000);
    assert_eq!(battle.av_micros, av);
    assert!(follow_up_hits(&battle).is_empty());
    let repeat = command(&battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.apply(&repeat), Err(BattleError::IllegalTiming));

    battle.queue_extra_turn(0);
    assert!(battle.start_extra_turn());
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.av_micros, av);
    assert!(!parrying(&battle, 0));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .count(),
        1
    );
    assert_eq!(battle.units[0].energy_milli, 155_000);
    assert!(battle.units[0].yunli_state.next_slash_is_cull);

    act(&mut battle, FORM, Action::Ultimate, None);
    battle.queue_extra_turn(0);
    assert!(battle.start_extra_turn());
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .count(),
        8
    );
    assert!(!battle.units[0].yunli_state.next_slash_is_cull);
}

#[test]
fn yunli_parry_survives_the_turn_in_which_ultimate_was_inserted() {
    let plan = prepared(&[boothill(), yunli(0, &[])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    const BOOTHILL: &str = "character.1315";
    assert_eq!(battle.ordinary_actor(), Some(BOOTHILL));
    act(&mut battle, FORM, Action::Ultimate, None);
    act(&mut battle, BOOTHILL, Action::Basic, Some(LEFT));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert!(parrying(&battle, 1));
    assert!(
        battle
            .dummy_damage
            .iter()
            .all(|hit| hit.kind != DamageKind::FollowUp)
    );
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert!(!parrying(&battle, 1));
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::FollowUp)
    );
}

#[test]
fn yunli_parry_taunts_all_and_converts_shielded_enemy_hit_to_one_grouped_cull() {
    let mut plan = prepared(&[boothill(), yunli(1, ALL_TRACES)], 3);
    for unit in 2..5 {
        finite(&mut plan, unit, 100_000);
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(1, 1, 120_000);
    act(&mut battle, FORM, Action::Ultimate, None);
    let rules = battle.mechanics(1).unwrap();
    assert!(rules.immune_to_enemy_control(&battle, 1));
    assert_eq!(
        rules.incoming_damage_reduction_bp(&battle, 1, 3, DamageKind::Direct),
        2_000
    );
    for unit in 2..5 {
        assert!(rules.taunts_enemy(&battle, 1, unit));
    }
    battle.apply_official_shield(1, 1, 1_000, 2, Some(Action::Skill));
    battle.resolve_enemy(3, &ENEMY);
    assert_eq!(battle.units[0].hp, 10_000);
    assert_eq!(battle.units[1].hp, 10_000);
    assert_eq!(battle.units[1].energy_milli, 40_000);
    assert_eq!(battle.effective_attack(1), 1_300);
    assert!(!rules.immune_to_enemy_control(&battle, 1));
    let hits = follow_up_hits(&battle);
    assert_eq!(hits.len(), 12);
    assert_eq!(hits[0].target_id.as_deref(), Some(MIDDLE));
    assert_eq!(hits[1].target_id.as_deref(), Some(LEFT));
    assert_eq!(hits[2].target_id.as_deref(), Some("dpr:3"));
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::FollowUp)
            .count(),
        1
    );
    battle.finish_turn(3);
    assert_eq!(follow_up_hits(&battle).len(), 12);
    for unit in 2..5 {
        assert!(!rules.taunts_enemy(&battle, 1, unit));
    }
}

#[test]
fn yunli_unblocked_counter_uses_talent_and_only_hits_adjacent_targets() {
    let mut plan = prepared(&[yunli(0, &[])], 3);
    for unit in 1..4 {
        finite(&mut plan, unit, 100_000);
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.units[0].energy_milli, 155_000);
    assert_eq!(follow_up_hits(&battle).len(), 2);
    assert_eq!(battle.units[1].toughness, 110);
    assert_eq!(battle.units[2].toughness, 110);
    assert_eq!(battle.units[3].toughness, 120);
    let rules = battle.mechanics(0).unwrap();
    assert!(!rules.follow_up_counts_as_ultimate(Some(Action::Basic)));
    assert!(rules.follow_up_counts_as_ultimate(Some(Action::Ultimate)));
}

#[test]
fn yunli_e6_counters_a_competing_taunted_attack_without_receiving_talent_energy() {
    let mut plan = prepared(&[boothill(), yunli(6, &[])], 1);
    finite(&mut plan, 2, 100_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "character.1315", Action::Skill, Some(LEFT));
    act(&mut battle, FORM, Action::Ultimate, None);
    battle.resolve_enemy(2, &ENEMY);
    let attack = battle
        .events()
        .iter()
        .find(|event| event.kind == EventKind::EnemyAttack)
        .unwrap();
    assert_eq!(attack.target_id.as_deref(), Some("character.1315"));
    assert_eq!(battle.units[1].energy_milli, 15_000);
    assert_eq!(follow_up_hits(&battle).len(), 10);
    assert_eq!(battle.support_effect_resistance(1), 5_000);
    battle.finish_turn(1);
    battle.units[1].normal_turns += 1;
    battle.finish_turn(1);
    assert_eq!(battle.support_effect_resistance(1), 0);
}

#[test]
fn yunli_cull_only_bounces_among_survivors_and_carries_half_toughness() {
    let mut plan = prepared(&[yunli(0, &[])], 2);
    finite(&mut plan, 1, 500);
    finite(&mut plan, 2, 100_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.units[1].hp, 0);
    let hits = follow_up_hits(&battle);
    assert_eq!(hits.len(), 8);
    assert_eq!(hits[0].target_id.as_deref(), Some(LEFT));
    assert!(
        hits[1..]
            .iter()
            .all(|hit| hit.target_id.as_deref() == Some(MIDDLE))
    );
    assert_eq!(battle.units[2].toughness, 95);
}

#[test]
fn yunli_e2_applies_to_all_counters_and_ultimate_vulnerability_only_buffs_the_enhanced_hits() {
    let counter = |eidolon, ultimate, vulnerability_bp| {
        let mut plan = prepared(&[yunli(eidolon, &[])], 1);
        finite(&mut plan, 1, 100_000);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if ultimate {
            act(&mut battle, FORM, Action::Ultimate, None);
        }
        battle.apply_permanent_debuff(0, 1, DebuffKind::UltimateVulnerability, vulnerability_bp);
        battle.resolve_enemy(1, &ENEMY);
        follow_up_hits(&battle)
            .into_iter()
            .map(|hit| match hit.kind {
                EventKind::Damage { amount, .. } => amount,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    assert!(counter(2, false, 0)[0] > counter(1, false, 0)[0]);
    assert_eq!(counter(2, false, 5_000), counter(2, false, 0));
    for (with_vulnerability, base) in counter(2, true, 5_000).into_iter().zip(counter(2, true, 0)) {
        assert!(with_vulnerability > base);
    }
    for (with_e2, base) in counter(2, true, 0).into_iter().zip(counter(1, true, 0)) {
        assert!(with_e2 > base);
    }
}

#[test]
fn yunli_dpr_replays_parry_expiry_and_conserves_damage_credits() {
    let plan = prepared(&[yunli(6, ALL_TRACES)], 3);
    let scenario = STANDARD_DPR_SCENARIO;
    let report = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        report,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    let hits = || report.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.kind == DamageKind::FollowUp));
    for hit in hits() {
        assert_eq!(
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>(),
            hit.amount
        );
    }
}

#[test]
fn yunli_rejects_incorrect_energy_cost_from_locked_parameters() {
    let mut input = yunli(0, &[]);
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[7] = 240.0;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
}
