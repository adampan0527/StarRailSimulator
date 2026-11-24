use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle,
    battle::Kit,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: FIRE,
        },
        energy_capacity: ENERGY_CAP,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 10_000,
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
            (BASIC.into(), vec![0.25]),
            (ENHANCED_BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.36, 4.0, 0.12, 0.1]),
            (
                ULTIMATE.into(),
                vec![0.2, 0.2, 0.3, 0.3, 70.0, 0.5, 0.2, 2.0],
            ),
            (TALENT.into(), vec![9.0, 15.0]),
            (FOLLOW_UP.into(), vec![0.1]),
            (ENHANCED_ULTIMATE.into(), vec![2.1]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn pela() -> OfficialCharacter {
    let mut ally = character(0, &[]);
    ally.unit_id = "character.1106".into();
    ally.form_id = ally.unit_id.clone();
    ally.combat_type.element = Element::Ice;
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 120_000_000;
    ally.skill_parameters = [
        ("skill.110601".into(), vec![0.5]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.3, 2.0, 0.6]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: usize, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: battle.plan.units[actor].id.clone(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

fn open_zone(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, 0, Action::Ultimate, None);
}

fn primary_attack(battle: &mut Battle<'_>, actor: usize, target: usize) {
    battle.action = Some(Action::Basic);
    battle.begin_attack();
    for _ in 0..2 {
        battle.attack_hit(actor, target, FIRE, 1_000, 0);
    }
    battle.notify_attack(actor, Action::Basic, Some(target));
    battle.resolve_follow_ups();
    battle.action = None;
}

#[test]
fn fury_runs_its_own_clock_and_unlocks_the_free_skill_until_expiry() {
    let plan = plan(&[character(0, &[OVERFLOW_TRACE])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert!(battle.check_action(0, Action::Skill).is_err());
    act(&mut battle, 0, Action::Basic, Some("dpr:1"));
    let basic = battle.dummy_damage[0].amount;
    assert_eq!(battle.debuff_count(1), 1);
    assert!(battle.mechanics(0).unwrap().taunts_enemy(&battle, 0, 1));

    battle.advance_to_decision();
    open_zone(&mut battle);
    let clock = battle.units[0].countdown.unwrap();
    assert_eq!(clock.speed_micros, 70_000_000);
    assert_eq!(clock.gauge_quanta, crate::battle::GAUGE_FULL);
    assert!(!clock.can_advance);
    assert_eq!(battle.units[0].hp, 4_000);
    for target in battle.enemies() {
        assert_eq!(
            battle.units[target]
                .debuffs
                .iter()
                .filter(|effect| matches!(effect.kind, DebuffKind::BalefireBind { .. }))
                .count(),
            1
        );
    }

    battle.ordinary_actor = Some(0);
    battle.skill_points = 0;
    let before = battle.dummy_damage.len();
    act(&mut battle, 0, Action::Skill, None);
    assert_eq!(battle.units[0].hp, 3_500);
    assert_eq!(battle.skill_points, 0);
    assert_eq!(battle.units[0].energy_milli, 35_000);
    assert_eq!(battle.dummy_damage.len() - before, 7);

    battle.ordinary_actor = Some(0);
    act(&mut battle, 0, Action::Basic, Some("dpr:1"));
    assert!(battle.dummy_damage.last().unwrap().amount > basic * 2);
    assert_eq!(battle.skill_points, 1);
    battle.units[0].hp = 1;
    battle.ordinary_actor = Some(0);
    assert!(battle.check_action(0, Action::Skill).is_err());
    battle.ordinary_actor = None;
    battle.units[0].countdown.as_mut().unwrap().gauge_quanta = 0;
    battle.advance_to_decision();
    assert!(!zone_active(&battle, 0));
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert!(battle.events().iter().any(|event| matches!(
        event.kind,
        EventKind::StatusExpired(StatusKind::InfiniteFury)
    )));
}

#[test]
fn charge_counts_attacks_not_hits_and_extra_skill_pays_its_own_cost() {
    for (eidolon, required) in [(0, 9), (2, 7)] {
        let plan = plan(&[character(eidolon, &[]), pela()]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        open_zone(&mut battle);
        let before = battle.units[0].countdown.unwrap().gauge_quanta;
        for count in 1..required {
            primary_attack(&mut battle, 1, 2);
            assert_eq!(battle.units[0].blade_recast_state.charge, count);
        }
        primary_attack(&mut battle, 1, 2);
        let follow_up: Vec<_> = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .collect();
        assert_eq!(follow_up.len(), 7);
        assert!(
            follow_up
                .iter()
                .all(|hit| hit.action == Action::Skill && hit.actor == 0)
        );
        assert_eq!(battle.units[0].blade_recast_state.charge, 1);
        assert_eq!(battle.units[0].hp, 3_500);
        assert_eq!(battle.units[0].energy_milli, 50_000);
        assert_eq!(
            battle.units[0].countdown.unwrap().gauge_quanta,
            before + if eidolon >= 1 { gauge_for(1_500) } else { 0 }
        );
    }
}

#[test]
fn a_full_charge_waits_for_healing_when_hp_is_one() {
    let plan = plan(&[character(0, &[]), pela()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    open_zone(&mut battle);
    battle.units[0].hp = 1;
    set_charge(&mut battle, 0, 8);
    primary_attack(&mut battle, 1, 2);
    assert_eq!(battle.units[0].blade_recast_state.charge, 9);
    assert!(!battle.has_pending_special_follow_up(0));
    battle.heal_unit(1, 0, 1_000);
    battle.notify_unit_action_finished(1);
    assert_eq!(battle.units[0].hp, 501);
    assert_eq!(battle.units[0].blade_recast_state.charge, 1);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .count(),
        7
    );
}

#[test]
fn lethal_enemy_attack_ends_fury_but_damage_reduction_and_healing_apply_first() {
    let mut plan = plan(&[character(0, &[OVERFLOW_TRACE, DEFENSIVE_TRACE])]);
    static ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 400,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    plan.units[1].kit = Kit::Enemy(&ENEMY);
    plan.units[1].stats.hp = 100_000;
    plan.units[1].stats.attack = 100_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    open_zone(&mut battle);
    assert_eq!(battle.incoming_healing_amount(0, 100), 150);
    assert_eq!(battle.enemy_targeting_weight(0), 11 * 100 * 10_000);
    assert_eq!(
        battle.mechanics(0).unwrap().incoming_damage_reduction_bp(
            &battle,
            0,
            1,
            DamageKind::Direct
        ),
        5_000
    );
    battle.apply_official_shield(0, 0, 100_000, 1, None);
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.units[0].hp, 4_000);
    assert_eq!(battle.units[0].blade_recast_state.charge, 1);
    battle.deplete_shields(0, 100_000);
    battle.resolve_enemy(1, &ENEMY);
    assert!(!zone_active(&battle, 0));
    assert_eq!(battle.units[0].hp, 2_500);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.incoming_healing_amount(0, 100), 100);
}

#[test]
fn full_energy_cleanses_all_debuffs_and_ultimate_returns_saved_overflow() {
    let plan = plan(&[character(0, &[OVERFLOW_TRACE])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_debuff(1, 0, DebuffKind::DefenseDown, 1_000, 2);
    battle.apply_debuff(1, 0, DebuffKind::Vulnerability, 2_000, 2);
    assert_eq!(battle.debuff_count(0), 2);
    battle.restore_energy_flat(0, 0, 150_000);
    assert_eq!(battle.debuff_count(0), 0);
    assert_eq!(battle.units[0].blade_recast_state.overflow_milli, 80_000);
    act(&mut battle, 0, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.units[0].blade_recast_state.overflow_milli, 0);
}

#[test]
fn another_nihility_activates_team_ultimate_damage_and_e2_converts_its_class() {
    let mut base = Vec::new();
    for eidolon in [1, 2, 4] {
        let plan = plan(&[character(eidolon, &[TEAM_TRACE]), pela()]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(
            battle.is_follow_up_damage(1, Some(Action::Ultimate), DamageKind::Direct),
            eidolon >= 2
        );
        assert!(!battle.is_follow_up_damage(1, Some(Action::Skill), DamageKind::Direct));
        open_zone(&mut battle);
        battle.fill_energy(1);
        act(&mut battle, 1, Action::Ultimate, None);
        let hit = battle
            .dummy_damage
            .iter()
            .find(|hit| hit.actor == 1)
            .unwrap();
        assert!(
            hit.assists
                .iter()
                .any(|&(owner, amount)| owner == 0 && amount > 0)
        );
        base.push(hit.amount);
    }
    assert!((base[1] * 3).abs_diff(base[0] * 4) <= 4);
    assert!((base[2] * 6).abs_diff(base[1] * 7) <= 7);
}

#[test]
fn e6_awards_one_hp_charge_per_turn_and_scales_the_damage_ultimate() {
    let mut damage = Vec::new();
    for eidolon in [5, 6] {
        let plan = plan(&[character(eidolon, &[])]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        open_zone(&mut battle);
        battle.spend_hp(0, 25, 1);
        battle.spend_hp(0, 25, 1);
        assert_eq!(
            battle.units[0].blade_recast_state.charge,
            u8::from(eidolon == 6)
        );
        battle.turn_sequence += 1;
        battle.spend_hp(0, 25, 1);
        assert_eq!(
            battle.units[0].blade_recast_state.charge,
            2 * u8::from(eidolon == 6)
        );
        let clock = battle.units[0].countdown.unwrap().gauge_quanta;
        battle.fill_energy(0);
        act(&mut battle, 0, Action::Ultimate, None);
        assert_eq!(battle.units[0].countdown.unwrap().gauge_quanta, clock);
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.dummy_damage.len(), 3);
        damage.push(battle.dummy_damage[0].amount);
    }
    assert!(damage[1].abs_diff(damage[0] * 3 / 2) <= 1);
}
