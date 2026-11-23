use crate::{
    CombatType, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};
const PHYSICALLY_WEAK: EnemyKit = EnemyKit {
    weakness: {
        let mut weakness = [false; Element::COUNT];
        weakness[PHYSICAL.index()] = true;
        weakness
    },
    ..FINITE
};

fn trailblazer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: PHYSICAL,
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
            base_aggro: 125,
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
            (SKILL.into(), vec![1.25]),
            (TALENT.into(), vec![0.2, 2.0]),
            (SINGLE_ULTIMATE.into(), vec![4.5]),
            (BLAST_ULTIMATE.into(), vec![2.7, 1.62]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn plan(character: OfficialCharacter, toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(&[character], 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn finite(plan: &mut PreparedBattle, slot: usize, hp: u32, physical: bool) {
    let target = &mut plan.units[slot];
    target.stats.hp = hp;
    target.stats.base_hp = hp;
    target.kit = Kit::Enemy(if physical { &PHYSICALLY_WEAK } else { &FINITE });
}

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: &str) {
    let command = command(battle, action, Some(target));
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn direct_hits(battle: &Battle<'_>) -> Vec<(usize, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| (hit.target, hit.amount))
        .collect()
}

#[test]
fn both_ultimates_share_one_energy_cost_and_require_a_target() {
    let mut prepared = plan(trailblazer(0, &[]), &[120, 120, 120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    for action in [Action::Ultimate, Action::EnhancedUltimate] {
        let before = battle.snapshot();
        let invalid = command(&battle, action, Some(CENTER));
        assert_eq!(battle.apply(&invalid), Err(BattleError::InsufficientEnergy));
        assert_eq!(battle.snapshot(), before);
    }

    prepared.units[0].starting_energy_milli = 120_000;
    for (action, hits, toughness) in [
        (Action::Ultimate, vec![(2, 2_700)], [120, 90, 120]),
        (
            Action::EnhancedUltimate,
            vec![(2, 1_620), (1, 972), (3, 972)],
            [100, 100, 100],
        ),
    ] {
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let before = battle.snapshot();
        let invalid = command(&battle, action, None);
        assert_eq!(battle.apply(&invalid), Err(BattleError::InvalidTarget));
        assert_eq!(battle.snapshot(), before);
        act(&mut battle, action, CENTER);
        assert_eq!(direct_hits(&battle), hits);
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.skill_points, before.skill_points);
        assert_eq!(battle.ordinary_actor, Some(0));
        assert_eq!(
            battle
                .enemies()
                .map(|enemy| battle.units[enemy].toughness)
                .collect::<Vec<_>>(),
            toughness
        );
        assert!(
            !battle
                .legal_actions()
                .iter()
                .any(|cmd| cmd.action.is_ultimate())
        );
    }
}

#[test]
fn battle_start_and_fighting_will_use_only_the_frozen_trace_conditions() {
    let prepared = plan(
        trailblazer(0, &[READY_FOR_BATTLE, FIGHTING_WILL]),
        &[120, 120, 120],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 75_000);
    act(&mut battle, Action::Basic, CENTER);
    assert_eq!(battle.units[0].energy_milli, 95_000);
    assert_eq!(battle.skill_points, 4);
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(
        direct_hits(&battle),
        [(2, 600), (2, 937), (1, 750), (3, 750)]
    );
    act(&mut battle, Action::EnhancedUltimate, LEFT);
    assert_eq!(direct_hits(&battle)[4..], [(1, 2_025), (2, 972)]);
}

#[test]
fn own_breaks_raise_base_attack_and_defense_and_cap_at_two_stacks() {
    for (traces, expected_defense) in [(&[][..], 500), (&[PERSEVERANCE][..], 600)] {
        let prepared = plan(trailblazer(0, traces), &[10, 20, 10]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        act(&mut battle, Action::Skill, CENTER);
        assert_eq!(direct_hits(&battle), [(2, 750), (1, 900), (3, 1_050)]);
        assert_eq!(battle.effective_attack(0), 1_400);
        assert_eq!(battle.effective_defense(0), expected_defense);
        assert_eq!(
            battle.units[0].destruction_trailblazer_state.talent_stacks,
            2
        );
        assert_eq!(
            battle
                .events()
                .iter()
                .filter(|event| event.kind == EventKind::WeaknessBroken)
                .count(),
            3
        );
    }
}

#[test]
fn eidolon_one_refunds_each_ultimate_only_once_and_e6_triggers_on_defeat() {
    for (eidolon, energy, stacks) in [(0, 25_000, 0), (1, 35_000, 0), (6, 35_000, 2)] {
        let mut prepared = plan(trailblazer(eidolon, &[]), &[120, 120, 120]);
        prepared.units[0].starting_energy_milli = 120_000;
        finite(&mut prepared, 1, 100, true);
        finite(&mut prepared, 2, 100, true);
        finite(&mut prepared, 3, 100_000, true);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        act(&mut battle, Action::EnhancedUltimate, CENTER);
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(
            battle.units[0].destruction_trailblazer_state.talent_stacks,
            stacks
        );
        assert!(
            !battle.units[0]
                .destruction_trailblazer_state
                .ultimate_kill_energy_available
        );
        assert_eq!(
            battle
                .events()
                .iter()
                .filter(|event| event.kind == EventKind::UnitDefeated)
                .count(),
            2
        );
    }
}

#[test]
fn eidolon_two_heals_once_if_any_landed_target_has_physical_weakness() {
    for (some_weakness, hp) in [(false, 700), (true, 750)] {
        let mut prepared = plan(trailblazer(2, &[]), &[120, 120, 120]);
        finite(&mut prepared, 1, 100, some_weakness);
        finite(&mut prepared, 2, 100_000, false);
        finite(&mut prepared, 3, 100_000, some_weakness);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.units[0].hp = 700;
        act(&mut battle, Action::Skill, CENTER);
        assert_eq!(battle.units[0].hp, hp);
        assert_eq!(battle.units[1].hp, 0);
    }
}

#[test]
fn eidolon_four_applies_crit_rate_only_to_hits_on_already_broken_targets() {
    let prepared = plan(trailblazer(4, &[]), &[120, 20, 120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 2, Some(Action::Basic), DamageKind::Direct),
        0
    );
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 2, Some(Action::Basic), DamageKind::Direct),
        2_500
    );
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 1, Some(Action::Skill), DamageKind::Direct),
        0
    );
    assert_eq!(
        mechanics.crit_rate_bonus_bp(
            &battle,
            0,
            2,
            Some(Action::Skill),
            DamageKind::WeaknessBreak
        ),
        0
    );
}

#[test]
fn fixed_dpr_selects_the_matching_ultimate_area_and_replays_damage_credits() {
    let prepared = plan(trailblazer(0, &[READY_FOR_BATTLE]), &[120]);
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    for (toughness, ultimate) in [
        (&[120][..], Action::Ultimate),
        (&[120, 120, 120][..], Action::EnhancedUltimate),
    ] {
        let run = || calculate_dpr(&prepared, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap();
        let result = run();
        assert_eq!(result, run());
        assert!(result.targets.iter().all(|target| {
            target.hits.iter().any(|hit| hit.action == ultimate)
                && target.hits.iter().all(|hit| {
                    hit.source_id == FORM
                        && (!hit.action.is_ultimate() || hit.action == ultimate)
                        && hit.credits.len() == 1
                        && hit.credits[0].unit_id == FORM
                        && hit.credits[0].amount == hit.amount
                })
        }));
        assert_eq!(result.total_damage, result.characters[0].own_damage);
    }
}
