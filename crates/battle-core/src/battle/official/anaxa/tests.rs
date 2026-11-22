use crate::{
    CombatType, Command, Event, EventKind, LogMode, OfficialStats, PreparedBattle, StatusKind,
    battle::{Kit, setup::UnitPlan},
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true, true, false, false, false, false, false],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};
const NO_WEAKNESS: EnemyKit = EnemyKit {
    weakness: [false; Element::COUNT],
    ..FINITE
};
const DISCLOSED: EnemyKit = EnemyKit {
    weakness: [true, true, true, true, true, false, false],
    ..FINITE
};

fn anaxa(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: WIND,
        },
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 97_000_000,
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
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![0.7, 4.0, 0.2]),
            (ULTIMATE.into(), vec![1.6]),
            (TALENT.into(), vec![0.3, 3.0, 5.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn argenti() -> OfficialCharacter {
    let mut character = anaxa(0, &[]);
    character.unit_id = "character.1302".into();
    character.form_id = character.unit_id.clone();
    character.combat_type.element = Element::Physical;
    character.energy_capacity = 180;
    character.stats.speed_micros = 180_000_000;
    character.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    character
}

fn finite_plan(team: &[OfficialCharacter], enemies: &[(&'static EnemyKit, u32)]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 3).unwrap();
    plan.units.truncate(plan.ally_count);
    for (position, &(kit, hp)) in enemies.iter().enumerate() {
        plan.units.push(UnitPlan {
            id: format!("enemy.{position}"),
            combat_type: None,
            has_control_resistance: false,
            kit: Kit::Enemy(kit),
            stats: crate::fixture::Stats {
                hp,
                base_hp: hp,
                base_speed_micros: 75_000_000,
                ..kit.stats
            },
            starting_energy_milli: 0,
        });
    }
    plan.wave_ends = vec![plan.units.len()];
    plan
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

fn owner_hits<'a>(battle: &'a Battle<'_>) -> impl Iterator<Item = (&'a str, u32)> {
    battle.events().iter().filter_map(|event| match event {
        Event {
            source_id: Some(source),
            target_id: Some(target),
            kind:
                EventKind::Damage {
                    kind: DamageKind::Direct,
                    amount,
                    ..
                },
            ..
        } if source == FORM => Some((target.as_str(), *amount)),
        _ => None,
    })
}

#[test]
fn registers_base_and_rejects_incompatible_frozen_parameters() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut character = anaxa(0, &[]);
    character.skill_parameters.get_mut(TALENT).unwrap()[2] = 4.0;
    assert!(matches!(
        build(&character),
        Err(BattleError::InvalidInput(_))
    ));
}

#[test]
fn skill_implants_each_hit_and_unlocks_an_extra_cast_on_the_next_command() {
    let plan = finite_plan(&[anaxa(0, &[])], &[(&FINITE, 10_000)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some("enemy.0"));
    assert_eq!(
        owner_hits(&battle)
            .map(|(_, damage)| damage)
            .collect::<Vec<_>>(),
        [504, 504, 504, 630, 630],
    );
    assert_eq!(battle.units[1].toughness, 90);
    assert_eq!(weakness_count(&battle, 1), 7);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::TriggeredSkill)
    );

    battle.set_energy(0, 0, 0);
    act(&mut battle, FORM, Action::Skill, Some("enemy.0"));
    assert_eq!(owner_hits(&battle).count(), 15);
    assert_eq!(battle.units[0].energy_milli, 60_000);
    assert_eq!(battle.skill_points, 1);
    let extras: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::TriggeredSkill)
        .collect();
    assert_eq!(extras.len(), 1);
    assert_eq!(extras[0].source_id.as_deref(), Some(FORM));
    assert_eq!(extras[0].target_id.as_deref(), Some("enemy.0"));
    assert_eq!(extras[0].av_micros, 100_000_000);
}

#[test]
fn bounces_prioritize_new_targets_and_preserve_the_cast_damage_bonus_on_defeat() {
    let plan = finite_plan(
        &[anaxa(0, &[])],
        &[
            (&NO_WEAKNESS, 30),
            (&NO_WEAKNESS, 10_000),
            (&DISCLOSED, 10_000),
        ],
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some("enemy.0"));
    let hits: Vec<_> = owner_hits(&battle).collect();
    assert_eq!(hits.len(), 5);
    assert_eq!(hits[0], ("enemy.0", 672));
    assert_eq!(
        hits[1..3]
            .iter()
            .map(|hit| hit.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    assert!(
        hits[1..]
            .iter()
            .filter(|hit| hit.0 == "enemy.1")
            .all(|hit| hit.1 >= 672)
    );
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::TriggeredSkill)
    );
}

#[test]
fn basic_kill_retargets_the_queued_skill_and_refunds_only_once() {
    let plan = finite_plan(
        &[anaxa(1, &[ROAMING_SIGNIFIER])],
        &[(&DISCLOSED, 30), (&FINITE, 10_000)],
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some("enemy.0"));
    assert_eq!(battle.skill_points, 5);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    let triggered = battle
        .events()
        .iter()
        .find(|event| event.kind == EventKind::TriggeredSkill)
        .unwrap();
    assert_eq!(triggered.target_id.as_deref(), Some("enemy.1"));
    assert_eq!(owner_hits(&battle).count(), 6);
    assert!(battle.units[2].debuffs.iter().any(|status| {
        status.source == 0 && status.kind == DebuffKind::DefenseDown && status.bp == 1_600
    }));
    act(&mut battle, FORM, Action::Skill, Some("enemy.1"));
    assert_eq!(battle.skill_points, 4);
}

#[test]
fn sublimation_imparts_all_weaknesses_before_damage_and_respects_control_resistance() {
    let mut plan = finite_plan(
        &[anaxa(0, &[QUALITATIVE_SHIFT]), argenti()],
        &[(&NO_WEAKNESS, 10_000); 2],
    );
    plan.units[0].starting_energy_milli = 140_000;
    plan.units[3].has_control_resistance = true;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(
        owner_hits(&battle).collect::<Vec<_>>(),
        [("enemy.0", 1_376), ("enemy.1", 1_376)]
    );
    assert_eq!(weakness_count(&battle, 2), 7);
    assert_eq!(weakness_count(&battle, 3), 7);
    assert!(battle.is_crowd_controlled(2));
    assert!(!battle.is_crowd_controlled(3));

    act(&mut battle, FORM, Action::Basic, Some("enemy.1"));
    act(
        &mut battle,
        "character.1302",
        Action::Basic,
        Some("enemy.0"),
    );
    assert!(battle.units[2].toughness <= 90);
    for unit in [2, 3] {
        battle.units[unit].normal_turns += 1;
        battle.resolve_enemy_turn(unit);
        assert!(weakness_count(&battle, unit) < 7);
    }
    let attacks: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::EnemyAttack)
        .map(|event| event.source_id.as_deref().unwrap())
        .collect();
    assert_eq!(attacks, ["enemy.1"]);
}

#[test]
fn trace_energy_and_e2_apply_at_their_distinct_turn_and_wave_boundaries() {
    let mut plan = finite_plan(
        &[anaxa(2, &[ROAMING_SIGNIFIER])],
        &[(&FINITE, 30), (&FINITE, 10_000)],
    );
    plan.wave_ends = vec![2, 3];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert_eq!(weakness_count(&battle, 1), 3);
    assert!(battle.units[2].debuffs.is_empty());
    battle.set_energy(0, 0, 0);
    act(&mut battle, FORM, Action::Basic, Some("enemy.0"));
    assert_eq!(battle.units[0].energy_milli, 70_000);
    assert_eq!(weakness_count(&battle, 2), 3);
    assert!(battle.units[2].debuffs.iter().any(|status| {
        status.source == 0
            && status.kind == DebuffKind::AllResistanceDown
            && status.bp == 2_000
            && status.remaining_turns(0).is_none()
    }));
    for turn in 1..=IMPLANT_TURNS {
        battle.units[2].normal_turns = turn;
        battle.finish_turn(2);
        assert_eq!(
            weakness_count(&battle, 2),
            if turn < IMPLANT_TURNS { 3 } else { 2 }
        );
    }
    assert!(battle.events().iter().any(|event| {
        event.target_id.as_deref() == Some("enemy.1")
            && event.kind == EventKind::StatusExpired(StatusKind::WeaknessImplanted)
    }));
}

#[test]
fn fourth_eidolon_stacks_on_the_command_and_extra_skill_then_expires() {
    let plan = finite_plan(&[anaxa(4, &[])], &[(&DISCLOSED, 50_000)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some("enemy.0"));
    let hits: Vec<_> = owner_hits(&battle).map(|(_, damage)| damage).collect();
    assert_eq!(hits.len(), 10);
    assert_eq!(hits[..5], [1_038; 5]);
    assert_eq!(hits[5..], [1_277; 5]);
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::AttackPercent),
        6_000
    );
    battle.units[0].normal_turns = 3;
    battle.finish_turn(0);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::AttackPercent), 0);
}

#[test]
fn erudition_trace_switches_modes_and_e6_uses_both_with_independent_damage() {
    for (eidolon, paired, crit_bonus, team_bonus) in [
        (0, false, 14_000, 0),
        (0, true, 0, 5_000),
        (6, false, 14_000, 5_000),
        (6, true, 14_000, 5_000),
    ] {
        let mut team = vec![anaxa(eidolon, &[IMPERATIVE_HIATUS])];
        if paired {
            team.push(argenti());
        }
        let plan = finite_plan(&team, &[(&DISCLOSED, 10_000)]);
        let battle = Battle::start(&plan, LogMode::Detailed);
        let rules = battle.mechanics(0).unwrap();
        assert_eq!(
            rules.crit_damage_bonus_bp(
                &battle,
                0,
                team.len(),
                Some(Action::Skill),
                DamageKind::Direct
            ),
            crit_bonus
        );
        assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 0, WIND), team_bonus);
    }

    let basic_damage = |eidolon| {
        let plan = finite_plan(&[anaxa(eidolon, &[])], &[(&DISCLOSED, 50_000)]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, FORM, Action::Basic, Some("enemy.0"));
        owner_hits(&battle).next().unwrap().1
    };
    assert_eq!(basic_damage(5), 936);
    assert_eq!(basic_damage(6), 1_216);
}
