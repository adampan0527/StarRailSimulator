use super::{
    BASIC, BOTTOMS_UP, ENHANCED_BASIC, NOVEL_CONCOCTION, ORGANIC_YEAST, SKILL, TALENT, ULTIMATE,
};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn gallagher(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.gallagher".into(),
        form_id: "character.1301".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Abundance,
            element: crate::Element::Fire,
        },
        level: 80,
        energy_capacity: 110,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 520,
            base_defense: 520,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![200.0]),
            (ULTIMATE.into(), vec![0.75, 2.0]),
            (TALENT.into(), vec![0.06, 80.0]),
            (ENHANCED_BASIC.into(), vec![1.25, 0.1, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn clara() -> OfficialCharacter {
    let mut input = gallagher(0, &[]);
    input.unit_id = "ally.clara".into();
    input.form_id = "character.1107".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Physical,
    };
    input.stats.base_speed_micros = 160_000_000;
    input.stats.speed_micros = 160_000_000;
    input.skill_parameters = [
        ("skill.110701".into(), vec![0.5]),
        ("skill.110702".into(), vec![0.6, 0.6]),
        ("skill.110703".into(), vec![5.0, 0.96, 2.0, 0.15, 2.0]),
        ("skill.110704".into(), vec![1.0, 0.8, 0.1]),
    ]
    .into();
    input
}

fn finite_plan(team: &[OfficialCharacter]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 3).unwrap();
    for enemy in &mut plan.units[plan.ally_count..] {
        enemy.kit = Kit::Enemy(&ENEMY);
        enemy.stats.hp = 25_000;
        enemy.stats.attack = 1_200;
        enemy.stats.base_attack = 1_000;
        enemy.stats.base_speed_micros = 10_000_000;
    }
    plan
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
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
}

fn heal_amounts(battle: &Battle<'_>, recipient: &str) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|entry| match entry.kind {
            EventKind::Heal { amount }
                if amount > 0
                    && entry.source_id.as_deref() == Some("ally.gallagher")
                    && entry.target_id.as_deref() == Some(recipient) =>
            {
                Some(amount)
            }
            _ => None,
        })
        .collect()
}

fn finish_test_turn(battle: &mut Battle<'_>, unit: usize) {
    battle.units[unit].normal_turns += 1;
    battle.finish_turn(unit);
}

#[test]
fn ultimate_advances_and_arms_one_enhanced_basic_with_e6_toughness() {
    let plan =
        PreparedBattle::official_dpr(&[gallagher(6, &[ORGANIC_YEAST]), clara()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].energy_milli, 75_000);
    act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
    let av_before = battle.snapshot().av_micros;
    battle.fill_energy(0);

    act(&mut battle, "ally.gallagher", Action::Ultimate, None);
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    assert_eq!(battle.snapshot().units[2].toughness, 6);
    assert_eq!(battle.snapshot().units[3].toughness, 84);
    // The interrupt queues Gallagher without replacing Clara's already active turn.
    assert_eq!(battle.ordinary_actor(), Some("ally.clara"));
    act(&mut battle, "ally.clara", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.ordinary_actor(), Some("ally.gallagher"));
    assert_eq!(battle.snapshot().av_micros, av_before);
    act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.units[3].toughness, 38);
    assert_eq!(snapshot.units[0].energy_milli, 25_000);
    assert_eq!(snapshot.skill_points, 5);
    assert!(!battle.units[0].gallagher_state.enhanced_basic);
    let direct: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.target == 3 && hit.kind == DamageKind::Direct)
        .map(|hit| (hit.action, hit.amount))
        .collect();
    // 1000 ATK × [0.5, 0.75, 1.25] × 1000/1500 DEF × 0.9 toughness.
    assert_eq!(
        direct,
        [
            (Action::Basic, 300),
            (Action::Ultimate, 450),
            (Action::Basic, 750)
        ]
    );
}

#[test]
fn e2_skill_cleanses_one_status_and_refreshes_target_resistance() {
    let mut character = gallagher(2, &[NOVEL_CONCOCTION]);
    character.stats.break_effect_bp = 20_000;
    character.stats.healing_bp = 1_000;
    character.stats.effect_resistance_bp = 5_000;
    let mut ally = clara();
    ally.stats.effect_resistance_bp = 7_000;
    let plan = finite_plan(&[character, ally]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(!battle.roll_effect(2, 0, 100_000));
    assert!(battle.roll_effect(2, 1, 100_000));
    battle.resolve_enemy(2, &ENEMY);
    battle.resolve_enemy(2, &ENEMY);
    battle.units[1].hp = battle.hp(0).0;
    battle.apply_speed_down(2, 1, 1_000, 5);
    battle.apply_vulnerability(2, 1, 1_000, 5);

    act(
        &mut battle,
        "ally.gallagher",
        Action::Skill,
        Some("ally.clara"),
    );
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.snapshot().units[0].energy_milli, 110_000);
    assert_eq!(heal_amounts(&battle, "ally.clara"), [370]);
    assert!(!battle.is_slowed(1));
    assert_eq!(battle.snapshot().units[1].vulnerability_bp, 1_000);
    assert!(!battle.roll_effect(2, 1, 100_000));
    act(&mut battle, "ally.clara", Action::Basic, Some("dpr:2"));
    assert!(!battle.roll_effect(2, 1, 100_000));
    act(
        &mut battle,
        "ally.gallagher",
        Action::Skill,
        Some("ally.clara"),
    );
    assert_eq!(battle.snapshot().units[1].vulnerability_bp, 0);
    act(&mut battle, "ally.clara", Action::Basic, Some("dpr:2"));
    assert!(!battle.roll_effect(2, 1, 100_000));
    act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
    act(&mut battle, "ally.clara", Action::Basic, Some("dpr:2"));
    assert!(battle.roll_effect(2, 1, 100_000));
}

#[test]
fn besotted_heals_per_attacked_target_in_aoe_counters_and_finishing_blows() {
    let mut plan = finite_plan(&[clara(), gallagher(0, &[BOTTOMS_UP])]);
    plan.units[1].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 3_000;
    battle.units[1].hp = 3_000;

    act(&mut battle, "ally.gallagher", Action::Ultimate, None);
    assert_eq!(heal_amounts(&battle, "ally.gallagher"), [80; 3]);
    battle.resolve_enemy(3, &ENEMY);
    assert_eq!(heal_amounts(&battle, "ally.clara"), [80]);
    assert!(battle.events().iter().any(|entry| {
        entry.kind == EventKind::FollowUp && entry.source_id.as_deref() == Some("ally.clara")
    }));

    battle.units[3].hp = 1;
    act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.hp(3).0, 0);
    assert_eq!(heal_amounts(&battle, "ally.clara"), [80; 2]);
    assert_eq!(heal_amounts(&battle, "ally.gallagher"), [80; 4]);
    act(&mut battle, "ally.clara", Action::Skill, None);
    assert_eq!(heal_amounts(&battle, "ally.clara"), [80; 4]);
}

#[test]
fn besotted_duration_and_attack_down_tick_on_the_enemy_turn() {
    for (eidolon, turns) in [(0, 2), (4, 3)] {
        let mut plan = finite_plan(&[gallagher(eidolon, &[])]);
        plan.units[0].starting_energy_milli = 110_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, "ally.gallagher", Action::Ultimate, None);
        act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
        let mut hits = Vec::new();
        for turn in 0..=turns {
            let hp_before = battle.hp(0).0;
            battle.resolve_enemy(2, &ENEMY);
            hits.push(hp_before - battle.hp(0).0);
            assert_eq!(battle.has_besotted_from(2, 0), turn < turns);
            finish_test_turn(&mut battle, 2);
        }
        // DEF 520 = level-32 attacker scale. Reduction applies to 1000 base ATK.
        assert_eq!(&hits[..2], [550, 550]);
        assert!(hits[2..].iter().all(|amount| *amount == 600));
        assert!(battle.events().iter().any(|entry| {
            entry.kind == EventKind::StatusExpired(StatusKind::AttackDown)
                && entry.source_id.as_deref() == Some("ally.gallagher")
                && entry.target_id.as_deref() == Some("dpr:2")
        }));
    }
}

#[test]
fn besotted_credits_gallagher_for_ally_break_damage_only() {
    let mut plan = PreparedBattle::official_dpr(&[gallagher(0, &[]), clara()], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.gallagher", Action::Ultimate, None);
    act(&mut battle, "ally.gallagher", Action::Basic, Some("dpr:2"));
    act(&mut battle, "ally.clara", Action::Basic, Some("dpr:1"));
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1)
        .collect();
    assert_eq!(hits.len(), 2);
    assert_eq!((hits[0].kind, hits[0].amount), (DamageKind::Direct, 300));
    assert!(hits[0].assists.is_empty());
    // 3767.5533 × 2 × (20+30)/40 × 1000/1500 = 6279.2555, then +6%.
    assert_eq!(
        (hits[1].kind, hits[1].amount),
        (DamageKind::WeaknessBreak, 6656)
    );
    assert_eq!(hits[1].assists, [(0, 377)]);
    battle.tick_break_dots(2);
    let bleed = battle.dummy_damage.last().unwrap();
    assert_eq!(bleed.kind, DamageKind::DamageOverTime);
    assert!(bleed.assists.is_empty());
}

#[test]
fn e6_break_effect_is_used_for_healing_and_ultimate_break_and_dpr_replays() {
    let input = gallagher(6, &[NOVEL_CONCOCTION, ORGANIC_YEAST]);
    let mut plan = PreparedBattle::official_dpr(&[input, clara()], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 3_000;
    battle.units[2].toughness = 24;
    act(&mut battle, "ally.gallagher", Action::Ultimate, None);
    let hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::WeaknessBreak)
        .unwrap();
    // Self-applied mark and E6: floor(6279.2555 × 1.06 × 1.2).
    assert_eq!(hit.amount, 7_987);
    assert!(hit.assists.is_empty());
    assert_eq!(heal_amounts(&battle, "ally.gallagher"), [88; 3]);

    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    let member_damage: u64 = result
        .characters
        .iter()
        .map(|member| member.own_damage + member.assist_damage)
        .sum();
    assert_eq!(member_damage, result.total_damage);
}
