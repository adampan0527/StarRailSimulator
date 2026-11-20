use super::{BASIC, ICE_SPELL, PURIFY, REINFORCE, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

fn march(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.march".into(),
        form_id: "march_7th.preservation".into(),
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
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
            (SKILL.into(), vec![0.38, 3.0, 0.3, 190.0, 5.0]),
            (ULTIMATE.into(), vec![0.9, 0.5, 1.0, 0.3]),
            (TALENT.into(), vec![0.5, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut input = march(0);
    input.unit_id = "ally.arlan".into();
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

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .expect("expected a legal March 7th action")
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = command(battle, action, target);
    battle.apply(&command).unwrap();
}

#[test]
fn basic_builds_energy_skill_points_and_deals_ice_toughness_damage() {
    let plan = PreparedBattle::official_dpr(&[march(0)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Basic, Some("dpr:2"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[0].energy_milli, 80_000);
    assert_eq!(snapshot.units[2].toughness, 110);
    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 1);
    // 1000 ATK × 50% × 2/3 DEF × 90% toughness.
    assert_eq!(hits[0].amount, 300);
}

#[test]
fn skill_cleanses_and_the_trace_extends_the_defense_scaled_shield() {
    let mut input = march(0);
    input.enabled_trace_ids = [PURIFY.into(), REINFORCE.into()].into();
    // Preserve the fractional ratio from the real second-level row.
    input.skill_parameters.get_mut(SKILL).unwrap()[0] = 0.40375;
    input.skill_parameters.get_mut(SKILL).unwrap()[3] = 304.0;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_speed_down(1, 0, 2_000, 5);
    assert!(battle.is_slowed(0));
    act(&mut battle, Action::Skill, Some("ally.march"));
    assert!(!battle.is_slowed(0));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 2);
    assert_eq!(snapshot.units[0].energy_milli, 90_000);
    // floor(500 DEF × 0.40375 + 304).
    assert_eq!(snapshot.units[0].shield, 505);

    for _ in 0..3 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.snapshot().units[0].shield, 505);
    }
    act(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.snapshot().units[0].shield, 0);
}

#[test]
fn opening_shield_uses_party_order_on_equal_hp_ratios_and_only_skill_shield_heals() {
    let plan = PreparedBattle::official_dpr(&[arlan(), march(2)], 42, 3).unwrap();
    let battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].shield, 440);
    assert_eq!(battle.snapshot().units[1].shield, 0);

    let plan = PreparedBattle::official_dpr(&[march(6)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 1_000;
    act(&mut battle, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.hp(0).0, 1_000);
    act(&mut battle, Action::Skill, Some("ally.march"));
    assert_eq!(battle.hp(0).0, 1_186);
}

#[test]
fn ultimate_rolls_freeze_per_target_then_regenerates_energy_for_each_success() {
    let run = |has_trace| {
        let mut input = march(1);
        if has_trace {
            input.enabled_trace_ids.push(ICE_SPELL.into());
        }
        let mut plan = PreparedBattle::official_dpr(&[input], 2, 3).unwrap();
        plan.units[0].starting_energy_milli = 120_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let ultimate = command(&battle, Action::Ultimate, None);
        battle.apply(&ultimate).unwrap();
        assert_eq!(battle.snapshot().skill_points, 3);
        assert_eq!(
            battle.snapshot().units[0].energy_milli,
            if has_trace { 17_000 } else { 5_000 }
        );
        // Seed 2 draws 5911, 7491, and 5956, so only the 65% trace hits two.
        assert_eq!(
            [
                battle.is_frozen(1),
                battle.is_frozen(2),
                battle.is_frozen(3)
            ],
            [has_trace, false, has_trace]
        );
        let mut commands = vec![ultimate];
        for _ in 0..2 {
            let basic = command(&battle, Action::Basic, Some("dpr:2"));
            battle.apply(&basic).unwrap();
            commands.push(basic);
        }
        let frozen_damage: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional && hit.action == Action::Ultimate)
            .map(|hit| (hit.target, hit.amount, hit.critical))
            .collect();
        if has_trace {
            // 1000 ATK × 30% × 2/3 DEF × 90% toughness.
            assert_eq!(frozen_damage, [(1, 180, false), (3, 180, false)]);
        } else {
            assert!(frozen_damage.is_empty());
        }
        let replay = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
        assert_eq!(replay.snapshot(), battle.snapshot());
        assert_eq!(replay.events(), battle.events());
    };
    run(false);
    run(true);
}

#[test]
fn weakness_break_freeze_alone_does_not_refund_first_eidolon_energy() {
    let mut plan = PreparedBattle::official_dpr(&[march(1)], 2, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].toughness = 20;
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    assert!(battle.is_frozen(1));
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.target == 1 && hit.kind == DamageKind::WeaknessBreak)
    );
}

static COUNTER_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn counter_plan(eidolon: u8) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(&[march(eidolon)], 42, 3).unwrap();
    plan.units.truncate(2);
    plan.wave_ends[0] = 2;
    plan.units[1].kit = Kit::Enemy(&COUNTER_ENEMY);
    plan
}

fn counter_damage(battle: &Battle<'_>) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::FollowUp,
                amount,
                ..
            } if event.source_id.as_deref() == Some("ally.march") => Some(amount),
            _ => None,
        })
        .collect()
}

#[test]
fn enemy_hit_reactions_use_pre_hit_shields_and_refresh_charges_each_march_turn() {
    let plan = counter_plan(2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..3 {
        battle.resolve_enemy(1, &COUNTER_ENEMY);
    }
    assert_eq!(counter_damage(&battle), [300; 2]);
    assert_eq!(battle.snapshot().units[0].energy_milli, 80_000);

    let plan = counter_plan(4);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..4 {
        battle.resolve_enemy(1, &COUNTER_ENEMY);
    }
    // (1000 ATK × 50% + 500 DEF × 30%) × 2/3 DEF × 90% toughness.
    assert_eq!(counter_damage(&battle), [390; 3]);
    assert_eq!(battle.snapshot().units[0].energy_milli, 90_000);
    assert_eq!(battle.snapshot().units[0].shield, 36);
    act(&mut battle, Action::Basic, Some("dpr:1"));
    battle.resolve_enemy(1, &COUNTER_ENEMY);
    assert_eq!(battle.snapshot().units[0].shield, 0);
    assert_eq!(counter_damage(&battle), [390; 4]);
    battle.resolve_enemy(1, &COUNTER_ENEMY);
    assert_eq!(counter_damage(&battle), [390; 4]);
}

#[test]
fn fixed_dpr_is_repeatable_and_dummies_never_trigger_counters() {
    let mut input = march(6);
    input.stats.effect_hit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert_eq!(result.characters[0].own_damage, result.total_damage);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .all(|hit| hit.kind != DamageKind::FollowUp && hit.action != Action::Skill)
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.kind == DamageKind::Additional && hit.action == Action::Ultimate)
    );
}
