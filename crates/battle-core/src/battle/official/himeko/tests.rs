use super::{BASIC, BENCHMARK, FORM, MAGMA, SKILL, STARFIRE, TALENT, ULTIMATE};
use crate::{
    Action, Battle, Command, DamageKind, LogMode, OfficialCharacter, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO,
    battle::setup::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

const ACTOR: &str = "ally.himeko";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn character(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: ACTOR.into(),
        form_id: FORM.into(),
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
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
            (SKILL.into(), vec![1.0, 0.4]),
            (ULTIMATE.into(), vec![1.38, 5.0]),
            (TALENT.into(), vec![0.7, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut input = character(0);
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

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
    command
}

fn set_finite_enemy(plan: &mut PreparedBattle, target: usize, hp: u32) {
    let unit = &mut plan.units[target];
    unit.kit = Kit::Enemy(&FINITE_ENEMY);
    unit.stats.hp = hp;
}

fn follow_up_damage(battle: &Battle<'_>) -> Vec<(usize, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| (hit.target, hit.amount))
        .collect()
}

#[test]
fn basic_and_blast_use_pinned_damage_and_resource_rules() {
    let plan = PreparedBattle::official_dpr(&[character(0)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].himeko_charge, 1);
    act(&mut battle, ACTOR, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.snapshot().skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.dummy_damage[0].amount, 300);
    assert_eq!(battle.units[1].toughness, 20);

    act(&mut battle, ACTOR, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 110_000);
    let damage: Vec<_> = battle.dummy_damage[1..]
        .iter()
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(damage, [(2, 600), (1, 240), (3, 240)]);
    assert_eq!(battle.units[2].toughness, 100);
    assert_eq!(battle.units[1].toughness, 10);
    assert_eq!(battle.units[3].toughness, 20);
}

#[test]
fn an_allied_elite_break_queues_the_follow_up_and_e1_speed_buff() {
    let plan = PreparedBattle::official_dpr(&[arlan(), character(1)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[3].toughness = 10;

    act(&mut battle, "ally.arlan", Action::Basic, Some("dpr:2"));

    assert_eq!(follow_up_damage(&battle), [(2, 420), (3, 466), (4, 420)]);
    assert_eq!(battle.units[1].himeko_charge, 0);
    assert_eq!(battle.units[1].energy_milli, 70_000);
    assert_eq!(battle.snapshot().units[1].speed_micros, 240_000_000);
}

#[test]
fn e4_only_grants_extra_charge_on_own_skill_breaks() {
    for (rank, action, charge, hits) in [
        (0, Action::Skill, 2, 0),
        (4, Action::Basic, 2, 0),
        (4, Action::Skill, 0, 3),
    ] {
        let plan = PreparedBattle::official_dpr(&[character(rank)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[1].toughness = 10;
        act(&mut battle, ACTOR, action, Some("dpr:1"));
        assert_eq!(battle.units[0].himeko_charge, charge);
        assert_eq!(follow_up_damage(&battle).len(), hits);
    }
}

#[test]
fn an_own_follow_up_break_charges_the_next_attack_without_reentering_itself() {
    let plan = PreparedBattle::official_dpr(&[character(0)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].himeko_charge = 2;
    battle.units[1].toughness = 10;
    battle.units[2].toughness = 10;
    act(&mut battle, ACTOR, Action::Basic, Some("dpr:1"));
    assert_eq!(follow_up_damage(&battle).len(), 3);
    assert_eq!(battle.units[0].himeko_charge, 3);

    act(&mut battle, ACTOR, Action::Basic, Some("dpr:1"));
    assert_eq!(follow_up_damage(&battle).len(), 6);
    assert_eq!(battle.units[0].himeko_charge, 0);
}

#[test]
fn starfire_burns_the_selected_and_adjacent_slots_before_later_magma_damage() {
    let mut input = character(0);
    input.stats.effect_hit_rate_bp = 10_000;
    input.enabled_trace_ids = vec![STARFIRE.into(), MAGMA.into()];
    let plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    act(&mut battle, ACTOR, Action::Skill, Some("dpr:1"));

    assert_eq!(battle.dummy_damage[0].amount, 600);
    assert!(battle.has_elemental_dot(1, Element::Fire));
    assert!(battle.has_elemental_dot(2, Element::Fire));
    assert!(!battle.has_elemental_dot(3, Element::Fire));

    act(&mut battle, ACTOR, Action::Skill, Some("dpr:2"));

    let skill_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(skill_hits, [600, 240, 720, 288, 240]);
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.kind == DamageKind::DamageOverTime && hit.target == 2 && hit.amount == 180
    }));
}

#[test]
fn e2_and_benchmark_include_the_exact_health_thresholds() {
    let mut input = character(2);
    input.enabled_trace_ids = vec![BENCHMARK.into()];
    let mut plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    set_finite_enemy(&mut plan, 1, 2_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();

    for (actor_hp, target_hp, crit, damage) in [(800, 1_000, 1_500, 1_500), (799, 1_001, 0, 0)] {
        battle.units[0].hp = actor_hp;
        battle.units[1].hp = target_hp;
        assert_eq!(
            mechanics.crit_rate_bonus_bp(&battle, 0, 1, Some(Action::Basic), DamageKind::Direct),
            crit
        );
        assert_eq!(
            mechanics.damage_bonus_bp(&battle, 0, 1, Some(Action::Basic), DamageKind::Direct),
            damage
        );
    }
}

#[test]
fn only_the_ultimate_that_defeats_an_enemy_earns_its_extra_energy() {
    for (hp, trigger_follow_up, expected_energy) in [(100, false, 20_000), (1_000, true, 25_000)] {
        let mut plan = PreparedBattle::official_dpr(&[character(0)], 42, 3).unwrap();
        plan.units[0].starting_energy_milli = 120_000;
        set_finite_enemy(&mut plan, 1, hp);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if trigger_follow_up {
            battle.units[2].toughness = 20;
        }

        act(&mut battle, ACTOR, Action::Ultimate, None);

        assert_eq!(battle.units[1].hp, 0);
        assert_eq!(battle.units[0].energy_milli, expected_energy);
    }
}

#[test]
fn e6_appends_two_random_hits_total_and_replays_the_same_targets() {
    let mut plan = PreparedBattle::official_dpr(&[character(6)], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 120_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let command = act(&mut battle, ACTOR, Action::Ultimate, None);

    let direct = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>()
    };
    let hits = direct(&battle);
    assert_eq!(&hits[..3], [(1, 828), (2, 828), (3, 828)]);
    assert_eq!(hits.len(), 5);
    assert_eq!([hits[3].1, hits[4].1], [331, 331]);

    let toughness: u16 = battle.units[1..].iter().map(|unit| unit.toughness).sum();
    assert_eq!(toughness, 110);
    let replay = Battle::replay(&plan, &[command], LogMode::Detailed).unwrap();
    assert_eq!(direct(&replay), hits);
    assert_eq!(replay.events(), battle.events());
}

#[test]
fn fixed_dpr_keeps_burn_and_follow_up_attribution() {
    let mut input = character(4);
    input.enabled_trace_ids.push(STARFIRE.into());
    input.stats.effect_hit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    for kind in [DamageKind::FollowUp, DamageKind::DamageOverTime] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.kind == kind && hit.source_id == ACTOR && hit.credits[0].unit_id == ACTOR
                })
        );
    }
}
