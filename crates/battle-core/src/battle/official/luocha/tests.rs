use super::{
    BASIC, CLEANSING_REVIVAL, FORM, SANCTIFIED, SKILL, TALENT, THROUGH_THE_VALLEY, ULTIMATE,
};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, Status},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

const LUOCHA: &str = "ally.luocha";
const ARLAN: &str = "ally.arlan";
const MIDDLE: &str = "dpr:2";

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn luocha(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: LUOCHA.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Abundance,
            element: crate::Element::Imaginary,
        },
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            base_aggro: 100,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.4, 200.0, 0.5, 2.0]),
            (ULTIMATE.into(), vec![1.2, 1.0]),
            (TALENT.into(), vec![2.0, 0.12, 2.0, 60.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = luocha(0, &[]);
    ally.unit_id = ARLAN.into();
    ally.form_id = "character.1008".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 160_000_000;
    ally.stats.base_speed_micros = 160_000_000;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn finite_plan(team: &[OfficialCharacter]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[120; 3])
        .unwrap();
    for enemy in &mut plan.units[plan.ally_count..] {
        enemy.kit = Kit::Enemy(&ENEMY);
        enemy.stats.hp = 25_000;
        enemy.stats.level = 80;
        enemy.stats.attack = 750;
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

fn heal_amounts(battle: &Battle<'_>, target: &str) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|entry| match entry.kind {
            EventKind::Heal { amount }
                if entry.source_id.as_deref() == Some(LUOCHA)
                    && entry.target_id.as_deref() == Some(target) =>
            {
                Some(amount)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn skill_cleanses_one_debuff_and_ultimate_dispels_one_buff_per_living_enemy() {
    let mut owner = luocha(0, &[CLEANSING_REVIVAL]);
    owner.stats.healing_bp = 1_000;
    let plan = finite_plan(&[owner, arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 1_100;
    battle.apply_skill_shock(2, 1, Action::Skill, 10_000, 5_000, 3);
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 2, 5_000, Action::Skill));

    act(&mut battle, LUOCHA, Action::Skill, Some(ARLAN));
    assert_eq!(heal_amounts(&battle, ARLAN), [660]);
    assert!(!battle.is_frozen(1));
    assert!(battle.has_shock(1));
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.units[0].luocha_state.abyss_flowers, 1);

    for target in battle.enemies() {
        battle.apply_speed_up(target, target, 1_000, 5);
    }
    battle.units[2].statuses[StatusKind::Shield.index()] = Some(Status {
        source: 2,
        magnitude: 10_000,
        expires_after_turn: 5,
    });
    battle.fill_energy(0);
    act(&mut battle, LUOCHA, Action::Ultimate, None);

    for target in battle.enemies() {
        // 1000 ATK × 120% × (1000 / 1500) × 90% toughness.
        assert_eq!(battle.hp(target).0, 24_280);
        assert_eq!(battle.units[target].toughness, 100);
        assert_eq!(battle.shield_amount(target), 0);
        assert_eq!(
            battle.units[target].magnitude(StatusKind::SpeedUp),
            if target == 2 { 1_000 } else { 0 }
        );
    }
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.snapshot().skill_points, 2);
    assert!(super::field_active(&battle, 0));
    assert_eq!(heal_amounts(&battle, ARLAN), [660]);
}

#[test]
fn automatic_skill_has_no_point_cost_and_waits_for_two_owner_turns() {
    let plan = finite_plan(&[luocha(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 1_500;
    // A hit deals 750 × (1000 / 1500) = 500 damage, reaching exactly half HP.
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.hp(0).0, 1_600);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.snapshot().skill_points, 3);

    battle.resolve_enemy(1, &ENEMY);
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.hp(0).0, 600);
    assert_eq!(heal_amounts(&battle, LUOCHA), [600]);
    for _ in 0..2 {
        act(&mut battle, LUOCHA, Action::Basic, Some(MIDDLE));
        assert_eq!(heal_amounts(&battle, LUOCHA), [600]);
    }
    act(&mut battle, LUOCHA, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].finished_turns, 3);
    assert_eq!(heal_amounts(&battle, LUOCHA), [600, 600]);
    assert!(super::field_active(&battle, 0));

    let mut lethal = Battle::start(&plan, LogMode::Detailed);
    lethal.units[0].hp = 400;
    lethal.resolve_enemy(1, &ENEMY);
    assert_eq!(lethal.hp(0).0, 0);
    assert!(heal_amounts(&lethal, LUOCHA).is_empty());
}

#[test]
fn automatic_skill_reacts_to_hp_spending_and_is_blocked_while_controlled() {
    let mut owner = luocha(0, &[THROUGH_THE_VALLEY]);
    owner.stats.base_speed_micros = 150_000_000;
    owner.stats.speed_micros = 150_000_000;
    let plan = finite_plan(&[owner, arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 1_200;

    // With seed 42, the first roll is 7415 against Luocha's 30% effective chance.
    assert!(!battle.apply_skill_freeze(2, 0, 10_000, 1, 5_000, Action::Skill));
    assert!(battle.roll_effect(2, 0, 10_000));
    act(&mut battle, ARLAN, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.hp(1).0, 1_500);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.snapshot().skill_points, 3);
    assert_eq!(heal_amounts(&battle, ARLAN), [600]);

    let mut frozen = Battle::start(&plan, LogMode::Detailed);
    frozen.units[1].hp = 1_200;
    assert!(frozen.apply_skill_freeze(2, 0, 100_000, 2, 5_000, Action::Skill));
    act(&mut frozen, ARLAN, Action::Skill, Some(MIDDLE));
    assert_eq!(frozen.hp(1).0, 900);
    assert!(heal_amounts(&frozen, ARLAN).is_empty());
}

#[test]
fn auto_cleansing_during_shock_damage_keeps_other_shock_timers_consistent() {
    let plan = finite_plan(&[luocha(0, &[CLEANSING_REVIVAL])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 1_400;
    battle.apply_skill_shock(1, 0, Action::Skill, 10_000, 5_000, 3);
    battle.apply_skill_shock(2, 0, Action::Skill, 10_000, 5_000, 1);

    battle.tick_shocks(0);

    // Two 250 damage ticks, then the second tick invokes an automatic heal for 600.
    assert_eq!(battle.hp(0).0, 1_500);
    assert_eq!(heal_amounts(&battle, LUOCHA), [600]);
    assert!(!battle.has_shock(0));
    let cleared = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::StatusCleared)
        .count();
    assert_eq!(cleared, 1);
}

#[test]
fn e2_uses_target_hp_before_healing_and_shield_expires_after_two_turns() {
    let mut owner = luocha(2, &[]);
    owner.stats.healing_bp = 1_000;
    let plan = finite_plan(&[owner]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 999;

    act(&mut battle, LUOCHA, Action::Skill, Some(LUOCHA));
    assert_eq!(heal_amounts(&battle, LUOCHA), [840]);
    assert_eq!(battle.shield_amount(0), 0);

    let mut exact_half = Battle::start(&plan, LogMode::Detailed);
    exact_half.units[0].hp = 1_500;
    exact_half.resolve_enemy(1, &ENEMY);
    assert_eq!(heal_amounts(&exact_half, LUOCHA), [660]);
    assert_eq!(exact_half.shield_amount(0), 420);
    for _ in 0..2 {
        act(&mut exact_half, LUOCHA, Action::Basic, Some(MIDDLE));
        assert_eq!(exact_half.shield_amount(0), 420);
    }
    act(&mut exact_half, LUOCHA, Action::Basic, Some(MIDDLE));
    assert_eq!(exact_half.shield_amount(0), 0);
}

#[test]
fn field_heals_once_per_attack_and_expires_after_two_owner_turns() {
    let plan = finite_plan(&[luocha(1, &[SANCTIFIED]), arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, LUOCHA, Action::Skill, Some(ARLAN));
    battle.fill_energy(0);
    act(&mut battle, LUOCHA, Action::Ultimate, None);
    assert!(super::field_active(&battle, 0));
    assert_eq!(battle.effective_attack(1), 1_200);

    battle.units[0].hp = 1_300;
    battle.units[1].hp = 1_300;
    battle.fill_energy(1);
    act(&mut battle, ARLAN, Action::Ultimate, Some(MIDDLE));
    // E1 boosts Luocha's ATK to 1200. Arlan hits three enemies in this action.
    assert_eq!(heal_amounts(&battle, ARLAN), [0, 204]);
    assert_eq!(heal_amounts(&battle, LUOCHA), [177]);
    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    act(&mut battle, LUOCHA, Action::Skill, Some(LUOCHA));
    assert_eq!(battle.units[0].luocha_state.abyss_flowers, 0);
    assert!(super::field_active(&battle, 0));

    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    act(&mut battle, LUOCHA, Action::Basic, Some(MIDDLE));
    assert!(!super::field_active(&battle, 0));
    assert_eq!(battle.effective_attack(1), 1_000);
    let healed = heal_amounts(&battle, ARLAN);
    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    assert_eq!(heal_amounts(&battle, ARLAN), healed);
}

#[test]
fn e4_reduces_enemy_damage_only_with_a_living_field_owner() {
    for (eidolon, expected_damage) in [(3, 500), (4, 440)] {
        let plan = finite_plan(&[luocha(eidolon, &[]), arlan()]);
        let mut battle = Battle::start(&plan, LogMode::Off);
        act(&mut battle, LUOCHA, Action::Skill, Some(ARLAN));
        battle.fill_energy(0);
        act(&mut battle, LUOCHA, Action::Ultimate, None);
        battle.resolve_enemy(2, &ENEMY);
        assert_eq!(battle.hp(0).0, 2_000 - expected_damage);
        battle.units[0].hp = 0;
        battle.units[1].shields.clear();
        battle.resolve_enemy(2, &ENEMY);
        assert_eq!(battle.hp(1).0, 1_500);
    }
}

#[test]
fn e6_reduces_all_resistance_and_credits_teammate_damage_until_enemy_expiry() {
    let plan = PreparedBattle::official_dpr(&[luocha(6, &[]), arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    act(&mut battle, LUOCHA, Action::Ultimate, None);

    assert_eq!(battle.dummy_damage.len(), 3);
    assert!(battle.dummy_damage.iter().all(|hit| hit.amount == 864));
    let statuses = battle
        .events()
        .iter()
        .filter(|entry| entry.kind == EventKind::StatusApplied(StatusKind::ResistanceDown))
        .count();
    assert_eq!(statuses, 3);
    assert_eq!(battle.snapshot().random_draws, 0);

    act(&mut battle, LUOCHA, Action::Basic, Some(MIDDLE));
    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    let thunder = battle.dummy_damage.last().unwrap();
    assert_eq!(thunder.amount, 360);
    assert_eq!(thunder.assists, [(0, 60)]);

    // Both 100-SPD enemy turns have concluded when reaching 200+ AV.
    while battle.snapshot().av_micros < 210_000_000 {
        let actor = battle.ordinary_actor().unwrap().to_owned();
        act(&mut battle, &actor, Action::Basic, Some(MIDDLE));
    }
    let actor = battle.ordinary_actor().unwrap().to_owned();
    act(&mut battle, &actor, Action::Basic, Some(MIDDLE));
    let unbuffed = battle.dummy_damage.last().unwrap();
    assert_eq!(unbuffed.amount, 300);
    assert!(unbuffed.assists.is_empty());
}

#[test]
fn fixed_dpr_uses_offensive_actions_and_conserves_all_damage_attribution() {
    let plan = PreparedBattle::official_dpr(&[luocha(6, &[]), arlan()], 42, 3).unwrap();
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
    assert_eq!(result.cycles, 3);
    assert!(result.characters[0].own_damage > 0);
    assert!(result.characters[0].assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|unit| &unit.hits)
            .any(|hit| { hit.source_id == LUOCHA && hit.kind == DamageKind::WeaknessBreak })
    );
    for hit in result.targets.iter().flat_map(|unit| &unit.hits) {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
        if hit.source_id == LUOCHA {
            assert!(matches!(hit.action, Action::Basic | Action::Ultimate));
        }
    }
}
