use crate::{
    CombatType, Command, LogMode, OfficialStats, Outcome, PreparedBattle,
    battle::Kit,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;
use crate::battle::official::cerydra::tests as cerydra;

const MIDDLE: &str = "dpr:2";
const SPARKLE: &str = "character.1306";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: QUANTUM,
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
            base_aggro: 75,
            speed_micros: 150_000_000,
            base_speed_micros: 102_000_000,
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
            (SKILL.into(), vec![1.8]),
            (ULTIMATE.into(), vec![6.0, 2.0, 12.0, 1.0, 0.2, 3.0]),
            (TALENT.into(), vec![20.0, 2.0, 0.7, 7.0, 15.0, 999.0]),
            (JOINT.into(), vec![3.0, 4.0, 3.0, 3.0]),
            (
                ENHANCED_SKILL.into(),
                vec![0.9, 3.0, 0.9, 2.0, 2.0, 33.0, 0.0, 1.0],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn archer_character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut unit = character(eidolon, traces);
    unit.unit_id = archer::FORM.into();
    unit.form_id = archer::FORM.into();
    unit.energy_capacity = 220;
    unit.combat_type.path = CombatPath::Hunt;
    unit.stats.speed_micros = 200_000_000;
    unit.skill_parameters = [
        ("skill.101501".into(), vec![1.0]),
        ("skill.101502".into(), vec![3.6, 1.0, 2.0, 1.0, 5.0]),
        ("skill.101503".into(), vec![10.0, 2.0, 4.0]),
        ("skill.101504".into(), vec![2.0]),
    ]
    .into();
    unit
}

fn sparkle() -> OfficialCharacter {
    let mut unit = character(4, &[]);
    unit.unit_id = SPARKLE.into();
    unit.form_id = SPARKLE.into();
    unit.energy_capacity = 110;
    unit.combat_type.path = CombatPath::Harmony;
    unit.stats.speed_micros = 110_000_000;
    unit.skill_parameters = [
        ("skill.130601".into(), vec![1.0]),
        ("skill.130602".into(), vec![0.24, 0.45, 1.0, 0.5]),
        ("skill.130603".into(), vec![2.0, 4.0, 0.1, 2.0]),
        ("skill.130604".into(), vec![2.0, 0.06, 2.0, 3.0]),
    ]
    .into();
    unit
}

fn plan(team: &[OfficialCharacter], points: u8) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, points)
        .unwrap()
        .with_dummies(&[1_000, 1_000])
        .unwrap()
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

#[test]
fn talent_uses_the_points_actually_restored_and_tracks_crit_on_the_spending_unit() {
    let plan = plan(
        &[
            character(4, &[ELEGANT_CONDUCT, LADYLIKE_POISE]),
            archer_character(0, &[]),
        ],
        5,
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.skill_point_cap(), 7);
    assert_eq!(battle.effective_attack(0), 2_500);
    assert_eq!(battle.effective_attack(1), 2_500);
    assert_eq!(battle.speed_micros(0), 170_400_000);
    assert_eq!(battle.units[0].rin_state.gems, 20);

    assert_eq!(battle.restore_skill_points(1, 5), 2);
    assert_eq!(battle.restore_skill_points(1, 5), 0);
    battle.spend_skill_points(1, 2);
    assert_eq!(
        battle.ally_buff_value(0, 1, AllyBuffKind::CritDamage),
        7_000
    );
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage), 0);
    assert_eq!(battle.units[0].rin_state.gems, 24);

    battle.spend_skill_points(0, 4);
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage),
        7_000
    );
    battle.restore_skill_points(0, 1);
    battle.restore_skill_points(0, 1);
    assert_eq!(
        battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage),
        14_000
    );
    assert_eq!(battle.units[0].rin_state.gems, 30);
    battle.units[0].normal_turns += 2;
    battle.expire_ally_buffs(0);
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::CritDamage), 0);
    assert_eq!(battle.effective_attack(0), 2_500);
}

#[test]
fn seven_points_enable_the_low_level_enhanced_skill_and_fund_five_bounces() {
    let mut rin = character(0, &[ELEGANT_CONDUCT]);
    rin.skill_parameters.get_mut(ENHANCED_SKILL).unwrap()[0] = 0.45;
    rin.skill_parameters.get_mut(ENHANCED_SKILL).unwrap()[2] = 0.45;
    let plan = plan(&[rin], 7);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_gems(&mut battle, 0, 0);

    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|action| action.action == Action::Skill)
    );
    act(&mut battle, FORM, Action::EnhancedSkill, None);

    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].rin_state.gems, 0);
    assert_eq!(battle.units[0].energy_milli, 110_000);
    assert_eq!(battle.dummy_damage.len(), 7);
    // 1000 base ATK × 2.5 × 45% × 2/3 × 90% × 115%.
    assert!(battle.dummy_damage.iter().all(|hit| hit.amount == 776));
    assert_eq!(
        battle.units[1..3]
            .iter()
            .map(|enemy| enemy.toughness)
            .sum::<u16>(),
        1_950
    );
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|action| action.action == Action::EnhancedSkill)
    );
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.skill_points, 1);
    assert_eq!(battle.units[0].rin_state.gems, 1);
}

#[test]
fn shadow_gems_replay_thirty_three_bounces_without_spending_points_or_normal_gems() {
    let plan = plan(&[character(1, &[ELEGANT_CONDUCT])], 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_gems(&mut battle, 0, 118);

    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.dummy_damage.len(), 35);
    assert_eq!(battle.units[0].rin_state.gems, 19);
    assert_eq!(battle.units[0].rin_state.shadow_gems, 99);

    set_gems(&mut battle, 0, 0);
    battle.set_skill_points(0, 7);
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.dummy_damage.len(), 70);
    assert_eq!(battle.units[0].rin_state.gems, 0);
    assert_eq!(battle.units[0].rin_state.shadow_gems, 0);
    assert_eq!(battle.skill_points, 7);
}

#[test]
fn finite_defeat_stops_bouncing_and_preserves_the_remaining_gems() {
    let mut plan = PreparedBattle::official_dpr(&[character(1, &[])], 42, 2)
        .unwrap()
        .with_dummies(&[1_000])
        .unwrap();
    plan.units[1].kit = Kit::Enemy(&FINITE);
    plan.units[1].stats.hp = 700;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_gems(&mut battle, 0, 30);

    act(&mut battle, FORM, Action::EnhancedSkill, None);

    assert_eq!(battle.outcome, Outcome::Victory);
    assert_eq!(battle.units[0].rin_state.gems, 27);
    assert_eq!(battle.units[0].rin_state.shadow_gems, 0);
}

#[test]
fn ultimate_spreads_vulnerability_before_damage_and_e6_gets_an_extra_turn() {
    let mut plan = plan(&[character(6, &[INBOUND_PROSPERITY])], 0);
    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, Some(MIDDLE));

    assert_eq!(battle.units[0].rin_state.gems, 57);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.skill_points, 1);
    assert_eq!(battle.vulnerability_from(0, 1), 2_000);
    assert_eq!(battle.vulnerability_from(0, 2), 2_000);
    assert_eq!(battle.units[1].toughness, 980);
    assert_eq!(battle.units[2].toughness, 970);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [5_184, 1_728]
    );
    assert!(battle.in_extra_turn(0));
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::ExtraTurnStarted)
    );
}

#[test]
fn archer_joint_restores_points_before_retained_turn_checks_and_waits_for_rins_turn_end() {
    let plan = plan(
        &[archer_character(0, &["trace.1015102"]), character(0, &[])],
        2,
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let opening_av = battle.av_micros;
    act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.ordinary_actor, Some(0));
    assert_eq!(battle.av_micros, opening_av);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(battle.units[1].energy_milli, 90_000);
    assert_eq!(battle.units[1].rin_state.gems, 26);
    assert_eq!(
        battle.units[2..]
            .iter()
            .map(|enemy| enemy.toughness)
            .sum::<u16>(),
        1_940
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .map(|hit| (hit.actor, hit.amount))
            .collect::<Vec<_>>(),
        [(1, 1_800), (1, 1_800), (0, 1_800), (0, 1_800)]
    );
    assert!(!battle.events().iter().any(|event| {
        matches!(
            event.kind,
            EventKind::ArcherCharge {
                before: 1,
                after: 0
            }
        )
    }));

    act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
    act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.skill_points, 0);
    assert!(battle.units[1].rin_state.joint_used);
    assert_eq!(battle.ordinary_actor, Some(1));
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert!(!battle.units[1].rin_state.joint_used);
}

#[test]
fn archer_fifth_skill_triggers_the_joint_even_above_three_skill_points() {
    let plan = plan(
        &[
            archer_character(1, &["trace.1015101"]),
            character(0, &[ELEGANT_CONDUCT]),
            sparkle(),
        ],
        12,
    );
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for remaining in [10, 8, 8, 6] {
        act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
        assert_eq!(battle.skill_points, remaining);
        assert!(!battle.units[1].rin_state.joint_used);
    }
    act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(battle.skill_points, 8);
    assert!(battle.units[1].rin_state.joint_used);
    assert_ne!(battle.ordinary_actor, Some(0));
}

#[test]
fn e2_applies_a_final_skill_factor_and_assigns_its_extra_damage_to_rin() {
    for (eidolon, expected_damage) in [(0, 2_160), (2, 2_808)] {
        let plan = plan(
            &[
                archer_character(0, &["trace.1015101"]),
                character(eidolon, &[]),
            ],
            7,
        );
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, archer::FORM, Action::Skill, Some(MIDDLE));
        let hit = &battle.dummy_damage[0];
        assert_eq!(hit.amount, expected_damage);
        assert_eq!(
            hit.assists
                .iter()
                .filter(|&&(actor, _)| actor == 1)
                .map(|&(_, amount)| amount)
                .sum::<u32>(),
            expected_damage - 2_160
        );
    }
    let plan = plan(&[character(2, &[])], 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert!(battle.dummy_damage.iter().all(|hit| hit.amount == 912));
}

#[test]
fn cerydras_copy_uses_the_same_gem_budget_and_the_original_pays_once() {
    let mut support = cerydra::character(0, &[]);
    support.stats.speed_micros = 110_000_000;
    let plan = plan(&[character(0, &[]), support], 5);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    cerydra::grant_merit(&mut battle, 1, 0, 6);
    act(&mut battle, FORM, Action::EnhancedSkill, None);

    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].rin_state.gems, 2);
    // Each cast deals two area hits plus nine bounces from the same 29-gem budget.
    let rin_hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::Direct)
        .count();
    assert_eq!(rin_hits, 22);
}
