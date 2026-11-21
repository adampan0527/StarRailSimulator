use crate::{
    Action, BattleError, Command, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Battle, Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LIGHTNING_LORD: &str = "summon:character.1204:lightning-lord";
const LEFT: Option<&str> = Some("dpr:1");
const FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn character(eidolon: u8, traces: &[&str], speed: u64) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Erudition,
            element: Element::Thunder,
        },
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
            base_speed_micros: speed * 1_000_000,
            speed_micros: speed * 1_000_000,
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
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.5, 2.0]),
            (ULTIMATE.into(), vec![1.2, 3.0]),
            (TALENT.into(), vec![60.0, 0.33, 10.0, 3.0, 0.25, 10.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn prepared(caster: OfficialCharacter, seed: u32, enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(&[caster], seed, 3)
        .unwrap()
        .with_dummies(&[400; 3][..enemies])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == FORM
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
    command
}

fn lightning_lord_remaining(battle: &Battle<'_>) -> Option<u64> {
    battle
        .action_order()
        .iter()
        .find(|entry| entry.unit_id == LIGHTNING_LORD)
        .map(|entry| entry.remaining_av_micros)
}

fn follow_up_hits<'b>(battle: &'b Battle<'_>) -> impl Iterator<Item = &'b DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
}

#[test]
fn skill_and_ultimate_change_summon_speed_without_resetting_distance() {
    let mut plan = prepared(character(0, &[], 100), 42, 3);
    plan.units[0].starting_energy_milli = 130_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(lightning_lord_remaining(&battle), Some(66_666_667));

    let ultimate = act(&mut battle, Action::Ultimate, None);
    assert_eq!(lightning_lord_remaining(&battle), Some(44_444_445));
    assert_eq!(
        battle.units[0].summon_turn.unwrap().speed_micros,
        90_000_000
    );
    let skill = act(&mut battle, Action::Skill, None);

    // At 100 AV the summon has traveled 60% of its gauge. Inserting Ultimate and
    // Skill adds five hits and lets it cover the remaining distance at 110 SPD.
    assert_eq!(
        follow_up_hits(&battle)
            .filter(|hit| hit.amount == 198)
            .count(),
        8
    );
    assert!(follow_up_hits(&battle).all(|hit| hit.av_micros == 136_363_637));
    assert_eq!(
        battle.dummy_damage[..6]
            .iter()
            .map(|hit| (hit.target, hit.action, hit.amount))
            .collect::<Vec<_>>(),
        [
            (1, Action::Ultimate, 720),
            (2, Action::Ultimate, 720),
            (3, Action::Ultimate, 720),
            (1, Action::Skill, 300),
            (2, Action::Skill, 300),
            (3, Action::Skill, 300),
        ]
    );
    assert_eq!(
        battle.units[1..]
            .iter()
            .map(|unit| u32::from(unit.toughness))
            .sum::<u32>(),
        3 * (400 - 20 - 10) - 8 * 5
    );
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 3);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().speed_micros,
        60_000_000
    );
    assert_eq!(battle.units[0].normal_turns, 2);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 35_000);

    let replay = Battle::replay(&plan, &[ultimate, skill], LogMode::Detailed).unwrap();
    assert_eq!(replay.events(), battle.events());
    assert_eq!(replay.snapshot(), battle.snapshot());
}

#[test]
fn hit_cap_does_not_force_an_immediate_summon_turn() {
    let mut plan = prepared(character(0, &[], 300), 42, 1);
    plan.units[0].starting_energy_milli = 130_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    act(&mut battle, Action::Ultimate, None);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 10);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().speed_micros,
        130_000_000
    );
    assert!(follow_up_hits(&battle).next().is_none());
    assert!(lightning_lord_remaining(&battle).unwrap() > 0);

    act(&mut battle, Action::Skill, None);
    assert_eq!(follow_up_hits(&battle).count(), 10);
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 3);
}

#[test]
fn ordinary_turn_precedes_summon_with_equal_av() {
    let plan = prepared(character(0, &[], 100), 42, 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].summon_turn.as_mut().unwrap().gauge_quanta = 6_000_000_000_000_000;
    act(&mut battle, Action::Basic, LEFT);

    assert_eq!(battle.snapshot().av_micros, 200_000_000);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(lightning_lord_remaining(&battle), Some(0));
    assert!(follow_up_hits(&battle).next().is_none());

    act(&mut battle, Action::Skill, None);
    assert_eq!(
        follow_up_hits(&battle)
            .filter(|hit| hit.av_micros == 200_000_000)
            .count(),
        5
    );
}

#[test]
fn bounce_chooses_new_primary_targets_and_uses_neighbor_multipliers() {
    for eidolon in [0, 1] {
        let plan = prepared(character(eidolon, &[], 100), 1, 3);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Basic, LEFT);
        let splash = if eidolon == 0 { 49 } else { 99 };

        // The seeded first three primaries are center, right, right. Each
        // instance independently applies defense and the 0.9 toughness factor.
        assert_eq!(
            follow_up_hits(&battle)
                .map(|hit| (hit.target, hit.amount, hit.av_micros))
                .collect::<Vec<_>>(),
            [2, 1, 3, 3, 2, 3, 2]
                .into_iter()
                .zip([198, splash, splash, 198, splash, 198, splash])
                .map(|(target, damage)| (target, damage, 166_666_667))
                .collect::<Vec<_>>()
        );
        assert_eq!(battle.rng.draws, 3);
        assert_eq!(
            battle.units[1..]
                .iter()
                .map(|unit| unit.toughness)
                .collect::<Vec<_>>(),
            [390, 395, 390]
        );
        assert_eq!(
            battle
                .events()
                .iter()
                .filter(|event| event.kind == EventKind::FollowUp)
                .count(),
            1
        );
    }
}

#[test]
fn bounce_reselects_after_a_primary_target_is_defeated() {
    let mut plan = prepared(character(0, &[], 100), 1, 3);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Basic, LEFT);

    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(
        follow_up_hits(&battle)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(1, 49), (3, 49), (3, 198), (3, 198)]
    );
}

#[test]
fn frozen_owner_blocks_summon_and_keeps_its_hits_until_the_next_slot() {
    let plan = prepared(character(0, &[], 100), 42, 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(1, 0, 100_000, 1, 0, Action::Basic));
    act(&mut battle, Action::Skill, None);

    assert!(!battle.is_frozen(0));
    assert_eq!(battle.snapshot().av_micros, 250_000_000);
    assert!(follow_up_hits(&battle).next().is_none());
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 5);
    assert_eq!(lightning_lord_remaining(&battle), Some(25_000_000));

    act(&mut battle, Action::Basic, LEFT);
    assert_eq!(follow_up_hits(&battle).count(), 5);
    assert!(follow_up_hits(&battle).all(|hit| hit.av_micros == 275_000_000));
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 3);
}

#[test]
fn imprisonment_and_entanglement_also_block_summon() {
    let plan = prepared(character(0, &[], 100), 42, 1);
    for kind in [DebuffKind::SkillImprisonment, DebuffKind::BreakImprisonment] {
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert!(battle.apply_debuff(1, 0, kind, 1_000, 2));
        act(&mut battle, Action::Skill, None);
        assert!(follow_up_hits(&battle).next().is_none());
        assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 5);
    }

    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_entanglement(1, 0, Action::Basic, 0);
    act(&mut battle, Action::Skill, None);
    assert!(follow_up_hits(&battle).next().is_none());
    assert_eq!(battle.units[0].jing_yuan_state.lightning_lord_hits, 5);

    let mut slowed = Battle::start(&plan, LogMode::Detailed);
    slowed.apply_speed_down(1, 0, 1_000, 2);
    act(&mut slowed, Action::Skill, None);
    assert_eq!(follow_up_hits(&slowed).count(), 5);
}

#[test]
fn summon_is_removed_when_jing_yuan_is_defeated() {
    let mut plan = prepared(character(0, &[], 100), 42, 2);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.attack = 100_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].hp, 0);
    assert!(battle.units[0].summon_turn.is_none());
    assert!(follow_up_hits(&battle).next().is_none());
}

#[test]
fn savant_providence_and_battalia_crush_apply_only_at_their_thresholds() {
    for (action, count, amount) in [(Action::Skill, 5, 297), (Action::Ultimate, 6, 346)] {
        let mut caster = character(0, &[BATTALIA_CRUSH, SAVANT_PROVIDENCE], 100);
        caster.stats.crit_rate_bp = 10_000;
        let mut plan = prepared(caster, 42, 1);
        if action == Action::Ultimate {
            plan.units[0].starting_energy_milli = 115_000;
        }
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(
            battle.units[0].energy_milli,
            if action == Action::Skill {
                80_000
            } else {
                130_000
            }
        );
        act(&mut battle, action, None);
        if action == Action::Ultimate {
            act(&mut battle, Action::Basic, LEFT);
        }
        assert_eq!(follow_up_hits(&battle).count(), count);
        assert!(follow_up_hits(&battle).all(|hit| hit.critical && hit.amount == amount));
    }
}

#[test]
fn war_marshal_starts_after_skill_and_expires_after_two_subsequent_turns() {
    let mut caster = character(0, &[WAR_MARSHAL], 200);
    caster.stats.crit_rate_bp = 9_000;
    let plan = prepared(caster, 52, 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    for _ in 0..3 {
        act(&mut battle, Action::Basic, LEFT);
    }

    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| hit.critical)
            .collect::<Vec<_>>(),
        [false, true, true, false]
    );
    assert_eq!(follow_up_hits(&battle).count(), 5);
    assert!(follow_up_hits(&battle).all(|hit| hit.critical));
}

#[test]
fn eidolon_two_only_buffs_the_next_two_ordinary_turns_of_direct_damage() {
    let mut plan = prepared(character(2, &[], 200), 42, 1);
    plan.units[0].starting_energy_milli = 130_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, None);
    for _ in 0..5 {
        act(&mut battle, Action::Basic, LEFT);
    }

    assert_eq!(follow_up_hits(&battle).count(), 9);
    assert!(follow_up_hits(&battle).all(|hit| hit.amount == 198));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct && hit.action == Action::Basic)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [300, 300, 360, 360, 300]
    );
}

#[test]
fn eidolon_six_applies_after_each_primary_hit_caps_and_expires_at_attack_end() {
    let plan = prepared(character(6, &[], 100), 42, 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);

    assert_eq!(
        follow_up_hits(&battle)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [198, 221, 245, 269, 269]
    );
    assert_eq!(battle.units[0].energy_milli, 105_000);
    assert_eq!(battle.debuff_count(1), 0);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::StatusApplied(StatusKind::Vulnerability))
            .count(),
        3
    );
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::StatusExpired(StatusKind::Vulnerability))
            .count(),
        1
    );
}

#[test]
fn eidolon_four_regenerates_energy_once_per_bounce_with_neighbors() {
    let plan = prepared(character(4, &[], 100), 1, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Basic, LEFT);
    assert_eq!(follow_up_hits(&battle).count(), 7);
    assert_eq!(battle.units[0].energy_milli, 91_000);
}

#[test]
fn eidolon_six_stacks_separately_for_primaries_and_keeps_other_sources() {
    let plan = prepared(character(6, &[], 100), 1, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_debuff(1, 3, DebuffKind::Vulnerability, 1_000, 4));
    act(&mut battle, Action::Basic, LEFT);

    // Center, right, right are the primary targets. The first splash on the
    // right only uses its 10% existing vulnerability. Center has 12% for
    // subsequent splashes, and the last right primary uses both sources.
    assert_eq!(
        follow_up_hits(&battle)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [
            (2, 198),
            (1, 99),
            (3, 108),
            (3, 217),
            (2, 110),
            (3, 241),
            (2, 110)
        ]
    );
    assert_eq!(battle.debuff_count(2), 0);
    assert_eq!(battle.debuff_count(3), 1);
    assert_eq!(battle.snapshot().units[3].vulnerability_bp, 1_000);
}

#[test]
fn fixed_dpr_replays_and_credits_all_lightning_lord_damage_to_jing_yuan() {
    let plan = prepared(
        character(6, &[BATTALIA_CRUSH, SAVANT_PROVIDENCE, WAR_MARSHAL], 100),
        42,
        3,
    );
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.kind == DamageKind::FollowUp
                    && hit.action == Action::Skill
                    && hit.source_id == FORM
            })
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
}

#[test]
fn rejects_invalid_talent_and_action_hit_counts_before_battle() {
    for (id, parameter, invalid) in [(TALENT, 0, 0.0), (SKILL, 1, 2.5), (ULTIMATE, 1, 12.0)] {
        let mut caster = character(0, &[], 100);
        caster.skill_parameters.get_mut(id).unwrap()[parameter] = invalid;
        assert!(matches!(
            PreparedBattle::official_dpr(&[caster], 42, 3),
            Err(BattleError::InvalidInput(_))
        ));
    }
}
