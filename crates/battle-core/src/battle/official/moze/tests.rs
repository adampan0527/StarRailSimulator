use crate::{
    Action, Battle, BattleError, Command, DamageKind, EventKind, LogMode, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage, resolve::CounterAttack, support::AllyBuffKind},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget, GAUGE_FULL},
};

use super::{
    BASIC, DAGGERHOLD, FORM, Moze, NIGHTFEATHER, OfficialCharacter, SKILL, TALENT, ULTIMATE,
    VENGEWISE,
};

const MOZE: &str = "ally.moze";
const NATASHA: &str = "ally.natasha";
const SERVAL: &str = "ally.serval";
const PREY: &str = "dpr:2";
const OTHER: &str = "dpr:1";
const FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 300,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn moze(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: MOZE.into(),
        form_id: FORM.into(),
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            base_aggro: 75,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 111_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: Some(0),
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.75, 9.0]),
            (ULTIMATE.into(), vec![1.62]),
            (TALENT.into(), vec![0.15, 3.0, 0.8]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn natasha() -> OfficialCharacter {
    let mut ally = moze(0, &[]);
    ally.unit_id = NATASHA.into();
    ally.form_id = "natasha.abundance".into();
    ally.energy_capacity = 90;
    ally.stats.base_speed_micros = 125_000_000;
    ally.stats.speed_micros = 125_000_000;
    ally.skill_parameters = [
        ("skill.110501".into(), vec![0.5]),
        ("skill.110502".into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
        ("skill.110503".into(), vec![0.092, 92.0]),
        ("skill.110504".into(), vec![0.30, 0.25]),
    ]
    .into();
    ally
}

fn serval() -> OfficialCharacter {
    let mut ally = natasha();
    ally.unit_id = SERVAL.into();
    ally.form_id = "character.1103".into();
    ally.energy_capacity = 100;
    ally.skill_parameters = [
        ("skill.110301".into(), vec![0.5]),
        ("skill.110302".into(), vec![0.7, 0.3, 0.8, 2.0, 0.4]),
        ("skill.110303".into(), vec![1.08, 2.0]),
        ("skill.110304".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter], seed: u32, sp: u8) -> PreparedBattle {
    PreparedBattle::official_dpr(team, seed, sp)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn finite(plan: &mut PreparedBattle, target: usize, hp: u32, attack: u32) {
    let unit = &mut plan.units[target];
    unit.kit = Kit::Enemy(&FINITE_ENEMY);
    unit.stats.hp = hp;
    unit.stats.attack = attack;
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
        .unwrap_or_else(|| panic!("expected {actor} {action:?} => {target:?}"));
    battle.apply(&command).unwrap();
    command
}

fn moze_hits<'b>(battle: &'b Battle<'_>) -> impl Iterator<Item = &'b DummyDamage> {
    battle.dummy_damage.iter().filter(|hit| hit.actor == 0)
}

fn charges(battle: &Battle<'_>) -> Option<u8> {
    Moze::prey(battle, 0).map(|prey| prey.charges)
}

#[test]
fn solo_moze_uses_basic_and_rejects_skill_without_mutating_the_decision() {
    let plan = prepared(&[moze(0, &[])], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let before = battle.snapshot();
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|action| action.action == Action::Skill)
    );
    let invalid = Command {
        decision_index: before.decision_index,
        actor_id: MOZE.into(),
        action: Action::Skill,
        target_id: Some(PREY.into()),
    };
    assert_eq!(battle.apply(&invalid), Err(BattleError::IllegalTiming));
    assert_eq!(battle.snapshot(), before);

    act(&mut battle, MOZE, Action::Basic, Some(PREY));
    let after = battle.snapshot();
    assert_eq!(after.skill_points, 4);
    assert_eq!(after.units[0].energy_milli, 80_000);
    assert_eq!(after.units[2].toughness, 390);
    assert_eq!(
        moze_hits(&battle).map(|hit| hit.amount).collect::<Vec<_>>(),
        [300]
    );
}

#[test]
fn skill_applies_nine_charges_and_single_target_actions_skip_departed_moze() {
    let plan = prepared(&[moze(1, &[DAGGERHOLD]), natasha()], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().av_micros, 35_000_000);
    assert_eq!(battle.snapshot().units[0].energy_milli, 80_000);

    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    assert_eq!(charges(&battle), Some(9));
    assert!(battle.units[0].departed);
    assert_eq!(battle.units[0].gauge_quanta, GAUGE_FULL);
    assert!(
        !battle
            .action_order()
            .iter()
            .any(|entry| entry.unit_id == MOZE)
    );
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|command| command.target_id.as_deref() != Some(MOZE))
    );
    let before = battle.snapshot();
    let invalid = Command {
        decision_index: before.decision_index,
        actor_id: NATASHA.into(),
        action: Action::Skill,
        target_id: Some(MOZE.into()),
    };
    assert_eq!(battle.apply(&invalid), Err(BattleError::InvalidTarget));
    assert_eq!(battle.snapshot(), before);
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.snapshot().units[0].energy_milli, 112_000);
    assert_eq!(battle.snapshot().units[3].toughness, 380);
    // Atk × skill ratio × 2/3 DEF × 0.9 intact toughness.
    assert_eq!(
        moze_hits(&battle)
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [(DamageKind::Direct, 450), (DamageKind::Additional, 90)]
    );

    act(&mut battle, NATASHA, Action::Skill, Some(NATASHA));
    act(&mut battle, NATASHA, Action::Basic, Some(OTHER));
    assert_eq!(charges(&battle), Some(9));
    assert_eq!(moze_hits(&battle).count(), 2);
}

#[test]
fn three_attacks_per_follow_up_and_ninth_charge_returns_moze_with_a4_advance() {
    let plan = prepared(
        &[moze(0, &[NIGHTFEATHER, DAGGERHOLD, VENGEWISE]), natasha()],
        42,
        1,
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mut commands = vec![act(&mut battle, MOZE, Action::Skill, Some(PREY))];
    for spent in 1..=9 {
        commands.push(act(&mut battle, NATASHA, Action::Basic, Some(PREY)));
        assert_eq!(charges(&battle), (spent < 9).then_some(9 - spent));
        assert_eq!(
            moze_hits(&battle)
                .filter(|hit| hit.kind == DamageKind::FollowUp)
                .count(),
            usize::from(spent / 3)
        );
    }
    assert!(!battle.units[0].departed);
    assert_eq!(battle.snapshot().units[0].normal_turns, 2);
    // Natasha's ninth turn starts at 720 AV. Moze returns with 40 AV (80% at speed 200).
    assert_eq!(battle.snapshot().av_micros, 760_000_000);
    assert_eq!(battle.snapshot().units[0].energy_milli, 120_000);
    let hits = moze_hits(&battle).collect::<Vec<_>>();
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::Additional && hit.amount == 90)
            .count(),
        13
    );
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp && hit.amount == 600)
            .count(),
        3
    );
    let refunds = battle.events().iter().filter(|event| {
        event.source_id.as_deref() == Some(MOZE)
            && matches!(event.kind, EventKind::SkillPoints { before, after } if after > before)
    });
    assert_eq!(refunds.count(), 1);
    assert_eq!(battle.snapshot().units[3].toughness, 260);
    let replay = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
    assert_eq!(replay.snapshot(), battle.snapshot());
    assert_eq!(replay.events(), battle.events());
    let quiet = Battle::replay(&plan, &commands, LogMode::Off).unwrap();
    assert_eq!(quiet.snapshot(), battle.snapshot());
}

#[test]
fn ultimate_uses_a6_and_e4_then_queues_a_distinct_e6_follow_up() {
    let mut caster = moze(6, &[VENGEWISE]);
    caster
        .skill_parameters
        .insert(TALENT.into(), vec![0.33, 3.0, 1.76]);
    let plan = prepared(&[caster, natasha()], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    battle.fill_energy(0);
    battle.dummy_damage.clear();

    act(&mut battle, MOZE, Action::Ultimate, Some(PREY));
    assert_eq!(charges(&battle), Some(8));
    assert_eq!(battle.snapshot().units[0].energy_milli, 19_000);
    assert!(battle.units[0].departed);
    let hits = moze_hits(&battle)
        .map(|hit| (hit.kind, hit.amount, hit.action))
        .collect::<Vec<_>>();
    // A6 ×1.25 applies to the active ultimate and queued strike; E6 adds 25 pp only to the latter.
    // E4 ×1.30 applies throughout. E1 recovers 2 energy on each 0.33× ATK additional hit.
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, 1579, Action::Ultimate),
            (DamageKind::Additional, 257, Action::Ultimate),
            (DamageKind::FollowUp, 1959, Action::Ultimate),
            (DamageKind::Additional, 257, Action::Ultimate),
        ]
    );
    assert_eq!(battle.snapshot().units[3].toughness, 340);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 3_000);

    for _ in 0..8 {
        act(&mut battle, NATASHA, Action::Basic, Some(PREY));
    }
    assert_eq!(battle.snapshot().units[0].normal_turns, 2);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 3_000);
    act(&mut battle, MOZE, Action::Basic, Some(PREY));
    act(&mut battle, NATASHA, Action::Basic, Some(OTHER));
    act(&mut battle, MOZE, Action::Basic, Some(PREY));
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 0);
}

#[test]
fn ultimate_killing_prey_retargets_once_with_seeded_replay() {
    let mut plan = prepared(&[moze(0, &[DAGGERHOLD]), natasha()], 42, 3);
    finite(&mut plan, 3, 1_000, 1_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let skill = act(&mut battle, MOZE, Action::Skill, Some(PREY));
    assert_eq!(battle.snapshot().units[3].hp, 460);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.actor_id == MOZE)
    );
    // Give Moze sufficient initial energy to choose the ultimate after marking.
    plan.units[0].starting_energy_milli = 90_000;
    let mut battle =
        Battle::replay(&plan, std::slice::from_ref(&skill), LogMode::Detailed).unwrap();
    let ultimate = act(&mut battle, MOZE, Action::Ultimate, Some(PREY));
    assert_eq!(charges(&battle), None);
    assert_eq!(battle.snapshot().units[3].hp, 0);
    assert_eq!(battle.snapshot().random_draws, 1);
    let retarget = moze_hits(&battle).collect::<Vec<_>>();
    assert_eq!(retarget.len(), 1);
    assert!(matches!(retarget[0].target, 2 | 4));
    assert_eq!(retarget[0].kind, DamageKind::FollowUp);
    assert_eq!(
        Battle::replay(&plan, &[skill, ultimate], LogMode::Detailed)
            .unwrap()
            .events(),
        battle.events()
    );
}

#[test]
fn enemy_targets_present_ally_and_moze_returns_when_last_ally_is_defeated() {
    let mut plan = prepared(&[moze(0, &[DAGGERHOLD]), natasha()], 42, 3);
    finite(&mut plan, 3, 100_000, 3_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    act(&mut battle, NATASHA, Action::Basic, Some(OTHER));

    let enemy_attack = battle
        .events()
        .iter()
        .find(|event| event.kind == EventKind::EnemyAttack)
        .unwrap();
    assert_eq!(enemy_attack.target_id.as_deref(), Some(NATASHA));
    assert_eq!(battle.snapshot().units[0].hp, 1_000);
    assert_eq!(battle.snapshot().units[1].hp, 0);
    assert_eq!(charges(&battle), None);
    assert_eq!(battle.ordinary_actor(), Some(MOZE));
    assert_eq!(battle.snapshot().av_micros, 140_000_000);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|command| command.action != Action::Skill)
    );
}

#[test]
fn defeating_the_last_prey_combines_return_and_new_wave_advances() {
    let mut plan = prepared(&[moze(0, &[DAGGERHOLD]), natasha()], 42, 3);
    finite(&mut plan, 2, 700, 1_000);
    plan.wave_ends = vec![3, 5];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(OTHER));
    act(&mut battle, NATASHA, Action::Basic, Some(OTHER));

    let snapshot = battle.snapshot();
    assert_eq!(snapshot.wave_index, 1);
    assert_eq!(snapshot.units[2].hp, 0);
    assert_eq!(charges(&battle), None);
    assert_eq!(battle.ordinary_actor(), Some(MOZE));
    // Natasha kills at 80 AV. The 20% return plus the 30% wave advance leave 25 AV.
    assert_eq!(snapshot.av_micros, 105_000_000);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|command| command.target_id.as_deref() != Some(OTHER))
    );
}

#[test]
fn departed_moze_skips_new_targeted_heals_and_shields_but_retains_existing_buffs() {
    let mut healer = natasha();
    healer.eidolon = 2;
    let plan = prepared(&[moze(0, &[]), healer], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 250;
    battle.units[1].hp = 250;
    battle.apply_ally_buff(1, 0, AllyBuffKind::Damage, 1_000, 2);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));

    battle.fill_energy(1);
    act(&mut battle, NATASHA, Action::Ultimate, None);
    assert_eq!(battle.units[0].hp, 250);
    // Natasha restores (1,000 × 9.2% + 92) × 1.25 below the talent threshold.
    assert_eq!(battle.units[1].hp, 480);
    assert!(battle.units[0].heals_over_time.is_empty());
    assert_eq!(battle.units[1].heals_over_time.len(), 1);
    battle.apply_official_shield(1, 0, 1_000, 3, Some(Action::Ultimate));
    assert_eq!(battle.shield_amount(0), 0);
    battle.apply_ally_buff(1, 0, AllyBuffKind::Damage, 3_000, 2);
    assert_eq!(battle.ally_buff_value(1, 0, AllyBuffKind::Damage), 1_000);
}

#[test]
fn a6_adds_vulnerability_credit_and_a_teammate_counter_consumes_charge() {
    let plan = prepared(&[moze(0, &[VENGEWISE]), natasha()], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    for target in [2, 3] {
        battle.counter_attack(
            1,
            target,
            CounterAttack {
                element: Element::Physical,
                multiplier_bp: 10_000,
                defense_multiplier_bp: 0,
                toughness: 0,
                splash: None,
                energy_milli: 0,
            },
        );
    }
    assert_eq!(charges(&battle), Some(8));
    let counters = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1)
        .collect::<Vec<_>>();
    assert_eq!(
        (counters[0].amount, counters[0].assists.as_slice()),
        (600, [].as_slice())
    );
    assert_eq!(
        (counters[1].amount, counters[1].assists.as_slice()),
        (750, [(0, 150)].as_slice())
    );
}

#[test]
fn ultimate_spending_the_last_charge_queues_the_marked_hit_before_the_free_follow_up() {
    let mut plan = prepared(&[moze(0, &[VENGEWISE]), natasha()], 42, 3);
    plan.units[0].starting_energy_milli = 70_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    for _ in 0..8 {
        act(&mut battle, NATASHA, Action::Basic, Some(PREY));
    }
    assert_eq!(charges(&battle), Some(1));
    battle.dummy_damage.clear();

    act(&mut battle, MOZE, Action::Ultimate, Some(PREY));
    assert_eq!(charges(&battle), None);
    assert!(!battle.units[0].departed);
    assert_eq!(
        moze_hits(&battle)
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [
            (DamageKind::Direct, 1215),
            (DamageKind::Additional, 90),
            (DamageKind::FollowUp, 600),
            (DamageKind::Additional, 90),
            (DamageKind::FollowUp, 480),
        ]
    );
}

#[test]
fn e2_credits_only_critical_hits_on_prey_and_preserves_fixed_dpr_total() {
    let mut ally = serval();
    ally.stats.crit_rate_bp = 10_000;
    let plan = prepared(&[moze(2, &[VENGEWISE]), ally], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MOZE, Action::Skill, Some(PREY));
    act(&mut battle, SERVAL, Action::Skill, Some(OTHER));
    assert_eq!(charges(&battle), Some(8));
    let direct = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1 && hit.kind == DamageKind::Direct)
        .collect::<Vec<_>>();
    // Serval's blast can hit Prey on an adjacent slot. (0.3 × 1000 × 0.6 × 1.9) − 270 = 72.
    assert_eq!(direct.len(), 2);
    assert_eq!(
        (
            direct[0].target,
            direct[0].amount,
            direct[0].assists.as_slice()
        ),
        (2, 630, [].as_slice())
    );
    assert_eq!(
        (
            direct[1].target,
            direct[1].amount,
            direct[1].assists.as_slice()
        ),
        (3, 342, [(0, 72)].as_slice())
    );

    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert!(result.characters[0].assist_damage > 0);
    let hits = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .collect::<Vec<_>>();
    assert!(
        hits.iter()
            .any(|hit| hit.source_id == MOZE && hit.kind == DamageKind::FollowUp)
    );
    assert!(
        hits.iter()
            .any(|hit| hit.source_id == MOZE && hit.kind == DamageKind::Additional)
    );
    assert!(
        hits.iter()
            .all(|hit| hit.credits.iter().map(|credit| credit.amount).sum::<u32>() == hit.amount)
    );
    assert_eq!(
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
}
