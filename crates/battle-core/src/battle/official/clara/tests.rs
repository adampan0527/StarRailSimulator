use super::{BASIC, KINSHIP, REVENGE, SKILL, TALENT, ULTIMATE, UNDER_PROTECTION};
use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: {
        let mut weaknesses = [false; Element::COUNT];
        weaknesses[Element::Physical.index()] = true;
        weaknesses
    },
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn clara(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.clara".into(),
        form_id: "character.1107".into(),
        level: 80,
        energy_capacity: 110,
        stats: OfficialStats {
            hp: 10_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 520,
            base_speed_micros: 90_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![1.2, 1.2]),
            (ULTIMATE.into(), vec![5.0, 1.6, 2.0, 0.25, 2.0]),
            (TALENT.into(), vec![1.0, 1.6, 0.1]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = clara(0, &[]);
    ally.unit_id = "ally.arlan".into();
    ally.form_id = "character.1008".into();
    ally.stats.base_speed_micros = 80_000_000;
    ally.stats.speed_micros = 80_000_000;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn herta() -> OfficialCharacter {
    let mut ally = clara(0, &[]);
    ally.unit_id = "ally.herta".into();
    ally.form_id = "character.1013".into();
    ally.stats.base_speed_micros = 80_000_000;
    ally.stats.speed_micros = 80_000_000;
    ally.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    ally
}

fn finite_plan(team: &[OfficialCharacter]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 5).unwrap();
    for unit in &mut plan.units[plan.ally_count..] {
        unit.kit = Kit::Enemy(&ENEMY);
        unit.stats.hp = 20_000;
        unit.stats.attack = 1_000;
        // Tests trigger a specific enemy attack through the core resolver.
        // Keeping other automatic turns remote makes the assertions independent.
        unit.stats.base_speed_micros = 10_000_000;
    }
    plan
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|entry| {
            entry.actor_id == "ally.clara"
                && entry.action == action
                && entry.target_id.as_deref() == target
        })
        .expect("expected a legal Clara action");
    battle.apply(&command).unwrap();
}

fn amounts(battle: &Battle<'_>, source: &str, target: &str, expected_kind: DamageKind) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|entry| match entry.kind {
            EventKind::Damage { kind, amount, .. }
                if kind == expected_kind
                    && entry.source_id.as_deref() == Some(source)
                    && entry.target_id.as_deref() == Some(target) =>
            {
                Some(amount)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn normal_counter_marks_one_attacker_and_e1_preserves_its_skill_bonus() {
    for (eidolon, extra_damage) in [(0, vec![720]), (1, vec![720, 720])] {
        let plan = finite_plan(&[clara(eidolon, &[REVENGE])]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.resolve_enemy(1, &ENEMY);
        // 1600 × 1.30 × 2/3 × 90%. A normal counter hits no neighbors.
        assert_eq!(
            amounts(&battle, "ally.clara", "dpr:1", DamageKind::FollowUp),
            [1_248]
        );
        assert!(amounts(&battle, "ally.clara", "dpr:2", DamageKind::FollowUp).is_empty());
        assert_eq!(battle.snapshot().units[0].energy_milli, 70_000);
        assert_eq!(battle.snapshot().units[1].toughness, 110);

        act(&mut battle, Action::Skill, None);
        act(&mut battle, Action::Skill, None);
        assert_eq!(
            amounts(&battle, "ally.clara", "dpr:1", DamageKind::Additional),
            extra_damage
        );
        assert!(amounts(&battle, "ally.clara", "dpr:2", DamageKind::Additional).is_empty());
        assert_eq!(battle.snapshot().units[1].toughness, 90);
        assert_eq!(battle.snapshot().skill_points, 3);
    }
}

#[test]
fn enhanced_counter_hits_adjacent_slots_once_per_charge_and_e6_adds_one_use() {
    for (eidolon, charges, primary, adjacent) in [(0, 2, 1_920, 960), (6, 3, 2_496, 1_248)] {
        let mut plan = finite_plan(&[arlan(), clara(eidolon, &[])]);
        plan.units[1].starting_energy_milli = 110_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Ultimate, None);
        assert_eq!(battle.snapshot().units[1].energy_milli, 5_000);
        for _ in 0..charges {
            battle.resolve_enemy(3, &ENEMY);
        }
        assert_eq!(battle.units[1].counter_state.enhanced_charges, 0);
        // Seed 42 starts above 50%, so E6 won't counter this first unenhanced allied hit.
        battle.resolve_enemy(3, &ENEMY);
        for (target, amount) in [("dpr:1", adjacent), ("dpr:2", primary), ("dpr:3", adjacent)] {
            assert_eq!(
                amounts(&battle, "ally.clara", target, DamageKind::FollowUp),
                vec![amount; charges]
            );
        }
        assert_eq!(battle.units[1].counter_state.marked, [3]);
        assert_eq!(
            battle.snapshot().units[1].energy_milli,
            5_000 * (1 + charges as u32)
        );
        for enemy in &battle.snapshot().units[2..] {
            assert_eq!(enemy.toughness, 120 - 10 * charges as u16);
        }
    }
}

#[test]
fn e6_unenhanced_counter_has_an_rng_trigger_and_marks_the_attacker() {
    let plan = finite_plan(&[arlan(), clara(6, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.resolve_enemy(3, &ENEMY);
    assert!(amounts(&battle, "ally.clara", "dpr:2", DamageKind::FollowUp).is_empty());
    battle.resolve_enemy(3, &ENEMY);
    assert_eq!(
        amounts(&battle, "ally.clara", "dpr:2", DamageKind::FollowUp),
        [960]
    );
    assert_eq!(battle.units[1].counter_state.marked, [3]);
}

#[test]
fn e4_damage_reduction_begins_after_the_hit_and_resets_at_the_next_own_turn() {
    let mut plan = finite_plan(&[clara(4, &[])]);
    plan.units[0].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, None);
    battle.resolve_enemy(1, &ENEMY);
    battle.resolve_enemy(1, &ENEMY);
    act(&mut battle, Action::Basic, Some("dpr:2"));
    battle.resolve_enemy(1, &ENEMY);

    // 500 incoming before modifiers. 10% Talent and 25% Ultimate apply throughout,
    // with an extra 30% multiplicative reduction only on the second attack.
    assert_eq!(
        amounts(&battle, "dpr:1", "ally.clara", DamageKind::Direct),
        [337, 236, 337]
    );
}

#[test]
fn ultimate_reduction_expires_by_turn_but_unused_counter_charges_remain() {
    let mut plan = finite_plan(&[clara(0, &[])]);
    plan.units[0].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, None);
    act(&mut battle, Action::Basic, Some("dpr:2"));
    act(&mut battle, Action::Basic, Some("dpr:2"));
    act(&mut battle, Action::Basic, Some("dpr:2"));
    battle.resolve_enemy(1, &ENEMY);

    // Clara's 10% passive is active, her 25% ultimate reduction has expired,
    // and the first enhanced counter still resolves at its enhanced multiplier.
    assert_eq!(
        amounts(&battle, "dpr:1", "ally.clara", DamageKind::Direct),
        [450]
    );
    assert_eq!(
        amounts(&battle, "ally.clara", "dpr:1", DamageKind::FollowUp),
        [1_920]
    );
    assert_eq!(battle.units[0].counter_state.enhanced_charges, 1);
}

#[test]
fn a_living_clara_can_counter_an_enemy_that_defeats_an_ally() {
    let mut plan = finite_plan(&[arlan(), clara(0, &[])]);
    plan.units[0].stats.hp = 1;
    plan.units[1].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Ultimate, None);
    battle.resolve_enemy(3, &ENEMY);

    assert_eq!(battle.hp(0).0, 0);
    assert_eq!(
        amounts(&battle, "ally.clara", "dpr:2", DamageKind::FollowUp),
        [1_920]
    );
    assert_eq!(battle.units[1].counter_state.enhanced_charges, 1);
}

#[test]
fn counter_during_an_enemy_turn_can_trigger_hertas_half_hp_follow_up() {
    let mut plan = finite_plan(&[clara(0, &[]), herta()]);
    plan.units[3].stats.hp = 2_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[3].hp = 1_500;
    battle.resolve_enemy(3, &ENEMY);

    let follow_ups: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::FollowUp)
        .map(|event| event.source_id.as_deref().unwrap())
        .collect();
    assert_eq!(follow_ups, ["ally.clara", "ally.herta"]);
    assert_eq!(battle.hp(3).0, 390);
}

#[test]
fn kinship_uses_fixed_chance_and_control_resistance_requires_its_own_trace() {
    let plan = finite_plan(&[clara(0, &[KINSHIP, UNDER_PROTECTION])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.mechanics(0).unwrap().control_resistance_bp(), 3_500);
    battle.apply_speed_down(1, 0, 2_000, 2);
    battle.resolve_enemy(1, &ENEMY);
    assert!(battle.is_slowed(0));
    battle.resolve_enemy(1, &ENEMY);
    assert!(!battle.is_slowed(0));
}

#[test]
fn fixed_dpr_never_invents_enemy_attacks_or_counter_marks() {
    let plan = PreparedBattle::official_dpr(&[clara(6, &[REVENGE])], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert_eq!(result.targets.len(), 3);
    for target in &result.targets {
        assert!(target.hits.iter().any(|hit| hit.action == Action::Skill));
        assert!(
            target
                .hits
                .iter()
                .all(|hit| !matches!(hit.kind, DamageKind::Additional | DamageKind::FollowUp))
        );
    }
}

#[test]
fn replay_keeps_the_state_after_a_scheduled_enemy_counter_and_skill() {
    let mut plan = finite_plan(&[clara(0, &[])]);
    plan.units[1].stats.base_speed_micros = 150_000_000;
    let script = [
        Command {
            decision_index: 0,
            actor_id: "ally.clara".into(),
            action: Action::Basic,
            target_id: Some("dpr:2".into()),
        },
        Command {
            decision_index: 1,
            actor_id: "ally.clara".into(),
            action: Action::Skill,
            target_id: None,
        },
    ];
    let detailed = Battle::replay(&plan, &script, LogMode::Detailed).unwrap();
    let silent = Battle::replay(&plan, &script, LogMode::Off).unwrap();
    assert_eq!(detailed.snapshot(), silent.snapshot());
    assert_eq!(
        amounts(&detailed, "ally.clara", "dpr:1", DamageKind::Additional),
        [720]
    );
    assert!(!amounts(&detailed, "ally.clara", "dpr:1", DamageKind::FollowUp).is_empty());
    assert!(silent.events().is_empty());
}
