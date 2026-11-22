use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn argenti(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: Element::Physical,
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
            base_aggro: 75,
            base_speed_micros: 103_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 0,
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
            (SKILL.into(), vec![1.2]),
            (ULTIMATE.into(), vec![1.6, 90.0]),
            (ENHANCED_ULTIMATE.into(), vec![2.8, 6.0, 0.95, 180.0]),
            (TALENT.into(), vec![3.0, 0.025, 10.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn prepared(character: OfficialCharacter, enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(&[character], 42, 3)
        .unwrap()
        .with_dummies(&[400; 3][..enemies])
        .unwrap()
}

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) -> Command {
    let command = command(battle, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
    command
}

fn hits<'a>(battle: &'a Battle<'_>) -> impl Iterator<Item = &'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
}

#[test]
fn both_ultimate_thresholds_validate_before_changing_state() {
    let mut plan = prepared(argenti(0, &[]), 3);
    for (energy, expected) in [
        (89_999, vec![]),
        (90_000, vec![Action::Ultimate]),
        (179_999, vec![Action::Ultimate]),
        (180_000, vec![Action::EnhancedUltimate]),
    ] {
        plan.units[0].starting_energy_milli = energy;
        let battle = Battle::start(&plan, LogMode::Detailed);
        let ultimates: Vec<_> = battle
            .legal_actions()
            .into_iter()
            .filter(|command| command.action.is_ultimate())
            .collect();
        assert!(ultimates.iter().all(|command| command.target_id.is_none()));
        assert_eq!(
            ultimates
                .into_iter()
                .map(|command| command.action)
                .collect::<Vec<_>>(),
            expected
        );
    }

    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let before = battle.snapshot();
    let events = battle.events().len();
    assert_eq!(
        battle.apply(&command(&battle, Action::Ultimate, None)),
        Err(BattleError::IllegalTiming)
    );
    assert_eq!(
        battle.apply(&command(&battle, Action::EnhancedUltimate, Some(LEFT))),
        Err(BattleError::InvalidTarget)
    );
    assert_eq!(battle.snapshot(), before);
    assert_eq!(battle.events().len(), events);

    battle.set_energy(0, 0, 90_000);
    let before = battle.snapshot();
    assert_eq!(
        battle.apply(&command(&battle, Action::EnhancedUltimate, None)),
        Err(BattleError::InsufficientEnergy)
    );
    assert_eq!(battle.snapshot(), before);
}

#[test]
fn cheap_ultimate_spends_only_ninety_and_ordinary_turn_remains_pending() {
    let mut plan = prepared(argenti(0, &[]), 3);
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let gauge = battle.units[0].gauge_quanta;
    let at = battle.av_micros;
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 44_000); // 120 - 90 + 3*3 + 5
    assert_eq!(battle.units[0].argenti_apotheosis, 3);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].finished_turns, 0);
    assert_eq!(battle.units[0].gauge_quanta, gauge);
    assert_eq!(battle.av_micros, at);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(
        hits(&battle)
            .map(|hit| (hit.action, hit.amount))
            .collect::<Vec<_>>(),
        [(Action::Ultimate, 960); 3]
    );
    assert!(
        battle
            .enemies()
            .all(|enemy| battle.units[enemy].toughness == 380)
    );

    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli, 83_000); // 44 + 3*3 + 30
    assert_eq!(battle.units[0].argenti_apotheosis, 6);
    assert_eq!(battle.skill_points, 2);
    act(&mut battle, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[0].energy_milli, 106_000);
    assert_eq!(battle.units[0].argenti_apotheosis, 7);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn enhanced_ultimate_has_six_seeded_bounces_and_keeps_its_action_tag() {
    let mut plan = prepared(argenti(0, &[]), 3);
    plan.units[0].starting_energy_milli = 180_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let action = act(&mut battle, Action::EnhancedUltimate, None);
    assert_eq!(battle.units[0].energy_milli, 32_000); // 5 + 3*(3+6).
    assert_eq!(battle.units[0].argenti_apotheosis, 9);
    assert_eq!(battle.units[0].finished_turns, 0);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    let damage = hits(&battle).collect::<Vec<_>>();
    assert_eq!(damage.len(), 9);
    assert!(
        damage
            .iter()
            .all(|hit| hit.action == Action::EnhancedUltimate)
    );
    assert_eq!(
        damage[..3].iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [1_680; 3]
    );
    assert_eq!(
        damage[3..].iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [570; 6]
    );
    assert_eq!(
        battle
            .enemies()
            .map(|enemy| battle.units[enemy].toughness)
            .sum::<u16>(),
        1_110 // 1200 - 20*3 - 5*6.
    );
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"enhanced_ultimate\""));
    assert_eq!(
        serde_json::to_string(&Action::Ultimate).unwrap(),
        "\"ultimate\""
    );
    let command = serde_json::from_str(&json).unwrap();
    let replay = Battle::replay(&plan, &[command], LogMode::Detailed).unwrap();
    assert_eq!(battle.snapshot(), replay.snapshot());
    assert_eq!(
        serde_json::to_value(battle.events()).unwrap(),
        serde_json::to_value(replay.events()).unwrap()
    );
}

#[test]
fn defeated_targets_are_removed_before_enhanced_bounces() {
    let mut plan = prepared(argenti(0, &[]), 3);
    plan.units[0].starting_energy_milli = 180_000;
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&FINITE);
    }
    plan.units[1].stats.hp = 100;
    plan.units[3].stats.hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::EnhancedUltimate, None);
    let targets = battle
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::Direct,
                    ..
                }
            )
        })
        .map(|event| event.target_id.as_deref().unwrap());
    assert_eq!(
        targets.collect::<Vec<_>>(),
        [
            "dpr:1", "dpr:2", "dpr:3", "dpr:2", "dpr:2", "dpr:2", "dpr:2", "dpr:2", "dpr:2"
        ]
    );
    assert_eq!(battle.units[0].energy_milli, 52_000); // 32 + two kills.
}

#[test]
fn piety_and_e4_set_initial_stacks_and_e1_uses_the_twelve_stack_cap() {
    let mut plan = prepared(argenti(4, &[PIETY]), 3);
    plan.units[0].starting_energy_milli = 180_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].argenti_apotheosis, 3); // 2 at battle start, 1 at turn start.
    act(&mut battle, Action::EnhancedUltimate, None);
    assert_eq!(battle.units[0].argenti_apotheosis, 12);
    act(&mut battle, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[0].argenti_apotheosis, 12);
    assert_eq!(battle.units[0].energy_milli, 55_000);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(
        mechanics.crit_rate_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
        3_000
    );
    assert_eq!(
        mechanics.crit_damage_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
        4_800
    );
}

#[test]
fn generosity_grants_energy_per_enemy_on_each_wave_and_receives_normal_hit_energy() {
    let mut character = argenti(0, &[GENEROSITY]);
    character.stats.base_speed_micros = 90_000_000;
    character.stats.speed_micros = 90_000_000;
    let mut plan = prepared(character, 3);
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&FINITE);
    }
    plan.units[1].stats.hp = 100;
    plan.wave_ends = vec![2, 4];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 102_000); // 90 + 2 for first entrant + 10 on hit.
    act(&mut battle, Action::Basic, Some(LEFT));
    assert_eq!(battle.wave_index, 1);
    assert_eq!(battle.units[0].energy_milli, 159_000); // +23, +10 kill, +4 entrants, two hits.
    let gains: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Energy {
                before_milli,
                after_milli,
            } if event.target_id.as_deref() == Some(FORM) => {
                Some((event.wave_index, after_milli - before_milli))
            }
            _ => None,
        })
        .collect();
    assert!(gains.contains(&(0, 2_000)));
    assert!(gains.contains(&(1, 4_000)));
}

#[test]
fn courage_threshold_uses_current_hp_and_e2_scales_both_ultimates_at_three_targets() {
    let plan = prepared(argenti(0, &[COURAGE]), 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 50_000;
    battle.units[2].hp = 50_001;
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(
        hits(&battle).map(|hit| hit.amount).collect::<Vec<_>>(),
        [1_104, 960, 960]
    );

    for (energy, action, neutral, buffed) in [
        (90_000, Action::Ultimate, 960, 1_344),
        (180_000, Action::EnhancedUltimate, 1_680, 2_352),
    ] {
        for (enemy_count, expected) in [(2, neutral), (3, buffed)] {
            let mut plan = prepared(argenti(2, &[]), enemy_count);
            plan.units[0].starting_energy_milli = energy;
            let mut battle = Battle::start(&plan, LogMode::Detailed);
            act(&mut battle, action, None);
            assert_eq!(hits(&battle).next().unwrap().amount, expected);
            act(&mut battle, Action::Basic, Some(LEFT));
            assert_eq!(
                battle.has_ally_buff(0, 0, AllyBuffKind::AttackPercent, E2),
                enemy_count == 3
            );
            act(&mut battle, Action::Basic, Some(LEFT));
            assert!(!battle.has_ally_buff(0, 0, AllyBuffKind::AttackPercent, E2));
        }
    }
}

#[test]
fn e6_ignores_defense_only_for_both_ultimate_damage_types() {
    for (energy, ultimate, baseline, ignored) in [
        (90_000, Action::Ultimate, 1_036, 1_152),
        (180_000, Action::EnhancedUltimate, 1_814, 2_016),
    ] {
        for (eidolon, damage) in [(5, baseline), (6, ignored)] {
            let mut input = argenti(eidolon, &[]);
            input.stats.crit_rate_bp = 10_000;
            let mut plan = prepared(input, 1);
            plan.units[0].starting_energy_milli = energy;
            let mut battle = Battle::start(&plan, LogMode::Detailed);
            act(&mut battle, Action::Basic, Some(LEFT));
            assert_eq!(hits(&battle).next().unwrap().amount, 648);
            // Reset just the isolated fixture's pre-cast state for the Ultimate comparison.
            battle.set_energy(0, 0, energy);
            battle.units[0].argenti_apotheosis = 2;
            act(&mut battle, ultimate, None);
            assert_eq!(hits(&battle).nth(1).unwrap().amount, damage);
        }
    }
}

#[test]
fn tingyuns_e1_recognizes_either_ultimate_as_a_teammate_ultimate() {
    let mut tingyun = argenti(1, &[]);
    tingyun.unit_id = "character.1202".into();
    tingyun.form_id = tingyun.unit_id.clone();
    tingyun.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Thunder,
    };
    tingyun.energy_capacity = 130;
    tingyun.stats.speed_micros = 220_000_000;
    tingyun.skill_parameters = [
        ("skill.120201".into(), vec![0.5]),
        ("skill.120202".into(), vec![0.2, 0.25, 3.0, 0.15]),
        ("skill.120203".into(), vec![50.0, 2.0, 0.2]),
        ("skill.120204".into(), vec![0.3]),
    ]
    .into();
    let mut plan = PreparedBattle::official_dpr(&[tingyun, argenti(0, &[])], 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap();
    for (energy, action) in [
        (90_000, Action::Ultimate),
        (180_000, Action::EnhancedUltimate),
    ] {
        plan.units[1].starting_energy_milli = energy;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let benediction = battle
            .legal_actions()
            .into_iter()
            .find(|command| {
                command.action == Action::Skill && command.target_id.as_deref() == Some(FORM)
            })
            .unwrap();
        battle.apply(&benediction).unwrap();
        assert_eq!(battle.speed_micros(1), 200_000_000);
        act(&mut battle, action, None);
        assert_eq!(battle.speed_micros(1), 220_600_000);
    }
}

#[test]
fn fixed_dpr_saves_for_the_enhanced_ultimate_and_replays_damage_attribution() {
    let plan = prepared(argenti(0, &[PIETY, GENEROSITY, COURAGE]), 3);
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    let entries = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(entries().all(|hit| hit.action != Action::Ultimate));
    assert!(entries().any(|hit| hit.action == Action::EnhancedUltimate));
    assert_eq!(
        result.total_damage,
        entries().map(|hit| u64::from(hit.amount)).sum::<u64>()
    );
    assert_eq!(result.total_damage, result.characters[0].own_damage);
    assert_eq!(result, run());
}
