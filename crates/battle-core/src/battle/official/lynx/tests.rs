use crate::{
    Action, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle, StatusKind,
    battle::{Battle, Kit},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget, GAUGE_FULL},
};

use super::{
    ADVANCE_SURVEYING, BASIC, EXPLORATION_TECHNIQUES, OfficialCharacter, SKILL,
    SURVIVAL_IN_THE_EXTREME, TALENT, ULTIMATE,
};

fn lynx(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.lynx".into(),
        form_id: "character.1110".into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Abundance,
            element: crate::Element::Quantum,
        },
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 125_000_000,
            speed_micros: 125_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.25]),
            (SKILL.into(), vec![0.05, 50.0, 2.0, 0.08, 80.0, 5.0]),
            (ULTIMATE.into(), vec![1.0, 0.09, 90.0]),
            (TALENT.into(), vec![2.0, 0.024, 24.0, 0.03, 30.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = lynx(0, &[]);
    ally.unit_id = "ally.arlan".into();
    ally.form_id = "character.1008".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 100_000_000;
    ally.stats.base_speed_micros = 100_000_000;
    ally.stats.base_aggro = 125;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.50]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
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

fn arlan_heals(battle: &Battle<'_>) -> Vec<u32> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Heal { amount } if event.target_id.as_deref() == Some("ally.arlan") => {
                Some(amount)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn skill_and_talent_apply_e1_and_e6_then_expire_on_the_recipient_turn() {
    let mut healer = lynx(6, &[SURVIVAL_IN_THE_EXTREME]);
    healer.stats.healing_bp = 1_000;
    let plan = PreparedBattle::official_dpr(&[healer, arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 500;

    act(&mut battle, "ally.lynx", Action::Skill, Some("ally.arlan"));
    assert_eq!(battle.ordinary_actor(), Some("ally.arlan"));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    // 270 additional Max HP, then 240 * 1.3 instant and (72 + 90) * 1.3 at target turn start.
    assert_eq!(battle.hp(1), (1_292, 2_270));
    assert_eq!(arlan_heals(&battle), [312, 210]);
    assert_eq!(battle.effective_attack(1), 1_060);
    assert_eq!(battle.support_effect_resistance(1), 3_000);
    act(&mut battle, "ally.arlan", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.effective_attack(1), 1_000);

    act(&mut battle, "ally.lynx", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.ordinary_actor(), Some("ally.arlan"));
    assert_eq!(arlan_heals(&battle), [312, 210, 178]);
    battle.units[1].hp = 2_200;
    act(&mut battle, "ally.arlan", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.hp(1), (2_000, 2_000));
    assert_eq!(battle.support_effect_resistance(1), 0);
    battle.units[1].hp = 1_000;
    act(&mut battle, "ally.lynx", Action::Basic, Some("dpr:2"));
    assert_eq!(arlan_heals(&battle), [312, 210, 178, 93]);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusExpired(StatusKind::SurvivalResponse))
    );
}

#[test]
fn ultimate_cleanses_once_per_living_ally_and_refreshes_a_single_talent_heal() {
    let mut plan = PreparedBattle::official_dpr(&[lynx(0, &[]), arlan()], 42, 3).unwrap();
    plan.units[0].starting_energy_milli = 100_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 500;
    battle.units[1].hp = 500;
    act(&mut battle, "ally.lynx", Action::Skill, Some("ally.arlan"));
    for target in [0, 1] {
        battle.apply_vulnerability(2, target, 1_000, 4);
        battle.apply_defense_down(2, target, 1_000, 4);
    }

    act(&mut battle, "ally.lynx", Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.hp(0).0, 770);
    assert_eq!(arlan_heals(&battle), [240, 162, 270]);
    for target in [0, 1] {
        assert_eq!(battle.vulnerability_from(2, target), 0);
        assert_eq!(battle.units[target].debuffs.len(), 1);
        assert_eq!(battle.units[target].heals_over_time.len(), 1);
    }
    let cleared = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::StatusCleared)
        .count();
    assert_eq!(cleared, 2);
    act(&mut battle, "ally.arlan", Action::Basic, Some("dpr:2"));
    act(&mut battle, "ally.lynx", Action::Basic, Some("dpr:2"));
    assert_eq!(arlan_heals(&battle), [240, 162, 270, 162]);
}

#[test]
fn survival_guard_blocks_one_debuff_and_refreshes_when_reapplied() {
    let mut healer = lynx(2, &[EXPLORATION_TECHNIQUES]);
    healer.stats.speed_micros = 300_000_000;
    let plan = PreparedBattle::official_dpr(&[healer, arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.mechanics(0).unwrap().control_resistance_bp(), 3_500);
    for _ in 0..2 {
        act(&mut battle, "ally.lynx", Action::Skill, Some("ally.arlan"));
        assert_eq!(battle.hp(1).1, 2_150);
        battle.apply_vulnerability(2, 1, 1_000, 3);
        assert_eq!(battle.vulnerability_from(2, 1), 0);
        battle.apply_speed_down(2, 1, 1_000, 3);
        assert!(battle.is_slowed(1));
    }
}

static ENEMY: EnemyKit = EnemyKit {
    stats: crate::fixture::Stats {
        attack: 200,
        base_attack: 200,
        base_speed_micros: 110_000_000,
        ..DUMMY_STATS
    },
    max_toughness: 50,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

#[test]
fn finite_enemy_uses_seeded_aggro_and_attack_triggers_a2_energy() {
    for marked in [false, true] {
        let mut healer = lynx(0, &[ADVANCE_SURVEYING]);
        healer.stats.energy_regeneration_bp = 12_500;
        let mut plan = PreparedBattle::official_dpr(&[arlan(), healer], 42, 3).unwrap();
        plan.units.truncate(3);
        plan.wave_ends[0] = 3;
        plan.units[2].stats = ENEMY.stats;
        plan.units[2].kit = Kit::Enemy(&ENEMY);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let recipient = if marked { "ally.arlan" } else { "ally.lynx" };
        act(&mut battle, "ally.lynx", Action::Skill, Some(recipient));
        assert_eq!(battle.ordinary_actor(), Some("ally.arlan"));
        assert_eq!(battle.lynx_aggro_bonus(0), if marked { 625 } else { 0 });
        assert_eq!(battle.lynx_aggro_bonus(1), 0);
        assert_eq!(battle.hp(if marked { 0 } else { 1 }).1, 2_150);
        let enemy_attack = battle
            .events()
            .iter()
            .find(|event| event.kind == EventKind::EnemyAttack)
            .unwrap();
        assert_eq!(enemy_attack.target_id.as_deref(), Some(recipient));
        assert_eq!(
            battle.units[1].energy_milli,
            if marked { 90_000 } else { 100_000 }
        );
    }
}

#[test]
fn quantum_break_delays_and_entanglement_caps_before_one_delayed_noncritical_hit() {
    let mut healer = lynx(0, &[]);
    healer.stats.speed_micros = 400_000_000;
    healer.stats.break_effect_bp = 5_000;
    let plan = PreparedBattle::official_dpr(&[healer], 42, 3)
        .unwrap()
        .with_dummies(&[10])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..6 {
        act(&mut battle, "ally.lynx", Action::Basic, Some("dpr:1"));
    }
    assert_eq!(battle.skill_points, 5);
    let hits = &battle.dummy_damage;
    assert_eq!((hits[0].kind, hits[0].amount), (DamageKind::Direct, 300));
    assert_eq!(
        (hits[1].kind, hits[1].amount),
        (DamageKind::WeaknessBreak, 1_412)
    );
    let delayed: Vec<_> = hits
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .collect();
    assert_eq!(delayed.len(), 1);
    assert_eq!(
        (delayed[0].actor, delayed[0].target, delayed[0].action),
        (0, 1, Action::Basic)
    );
    assert_eq!(
        (delayed[0].av_micros, delayed[0].amount, delayed[0].critical),
        (155_000_000, 8_476, false)
    );
    assert_eq!(battle.units[1].toughness, 10);
    assert!(battle.units[1].entanglement.is_none());
    let shifts = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::GaugeShifted {
                before_quanta,
                after_quanta,
            } => Some(after_quanta - before_quanta),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(shifts, [GAUGE_FULL / 4, GAUGE_FULL * 3 / 10]);
    let first = calculate_dpr(&plan, &[10, 20, 10], crate::STANDARD_DPR_SCENARIO.cycles).unwrap();
    let replay = calculate_dpr(&plan, &[10, 20, 10], crate::STANDARD_DPR_SCENARIO.cycles).unwrap();
    assert_eq!(first, replay);
    assert!(
        first.targets[1]
            .hits
            .iter()
            .any(|hit| hit.kind == DamageKind::Additional)
    );
    assert_eq!(
        first.total_damage,
        first
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>()
    );
}

#[test]
fn self_response_uses_live_max_hp_in_direct_damage_and_hp_credit_is_conserved() {
    let plan = PreparedBattle::official_dpr(&[lynx(6, &[]), arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    battle.apply_max_hp_increase(1, 0, 500, 5);
    act(&mut battle, "ally.lynx", Action::Skill, Some("ally.lynx"));
    assert_eq!(battle.snapshot().units[0].max_hp, 2_825);
    act(&mut battle, "ally.arlan", Action::Basic, Some("dpr:1"));
    act(&mut battle, "ally.lynx", Action::Basic, Some("dpr:1"));
    let hit = battle.dummy_damage.last().unwrap();
    // 2825 * .25 * .9 * 2/3 = 423.75. HP attributable to Arlan adds 75.
    assert_eq!(hit.amount, 423);
    assert_eq!(hit.assists, [(1, 75)]);
}

#[test]
fn an_entangled_target_counts_one_stack_per_attack_and_can_be_cleansed() {
    let plan = PreparedBattle::official_dpr(&[lynx(0, &[])], 42, 3)
        .unwrap()
        .with_dummies(&[10])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..2 {
        battle.action = Some(Action::Basic);
        for _ in 0..2 {
            battle.attack_hit(0, 1, Element::Quantum, 2_500, 10);
        }
        battle.attack_targets.clear();
    }
    let mut uncleansed = battle.clone();
    uncleansed.tick_entanglement(1);
    let hit = uncleansed.dummy_damage.last().unwrap();
    // Two attacks total, the first applies one stack and the next adds one stack.
    assert_eq!((hit.kind, hit.amount), (DamageKind::Additional, 2_260));

    battle.cleanse_one(0, 1);
    assert!(!battle.is_debuffed(1));
    battle.tick_entanglement(1);
    assert!(
        battle
            .dummy_damage
            .iter()
            .all(|hit| hit.kind != DamageKind::Additional)
    );
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusCleared)
    );
}
