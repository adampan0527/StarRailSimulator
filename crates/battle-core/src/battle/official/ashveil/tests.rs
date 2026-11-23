use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle,
    battle::{
        Kit,
        official::{cerydra::tests as cerydra, saber::tests::saber},
    },
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn ashveil(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: Element::Thunder,
        },
        energy_capacity: ENERGY_CAP,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 100_000_000,
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
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.0, 1.0, 0.5, 0.2, 1.0]),
            (ULTIMATE.into(), vec![2.0, 3.0, 4.0, 1.0]),
            (TALENT.into(), vec![2.0, 3.0, 1.0, 1.0, 2.0, 12.0, 8.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: &str) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: Some(target.into()),
    };
    assert!(battle.legal_actions().contains(&command));
    battle.resolve_command(&command).unwrap();
}

fn allied_attack(battle: &mut Battle<'_>, target: usize, hits: usize) {
    battle.action = Some(Action::Basic);
    battle.begin_attack();
    for _ in 0..hits {
        battle.attack_hit(1, target, Element::Wind, 1_000, 0);
    }
    battle.notify_attack(1, Action::Basic, Some(target));
    battle.resolve_follow_ups();
    battle.action = None;
}

#[test]
fn skill_only_refunds_if_the_bait_was_already_marked() {
    let plan = plan(&[ashveil(0, &[DAMNATION_TRAIL])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(bait(&battle, 0), Some(1));
    assert_eq!(battle.units[0].ashveil_state.charges, 2);

    for expected_points in [2, 2] {
        battle.ordinary_actor = Some(0);
        act(&mut battle, Action::Skill, "dpr:2");
        assert_eq!(battle.skill_points, expected_points);
        assert_eq!(bait(&battle, 0), Some(2));
    }
    let amounts: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(amounts, [642, 642, 321]);
    assert_eq!(battle.units[2].toughness, 360);
    assert_eq!(battle.units[0].ashveil_state.gluttony, 2);
}

#[test]
fn copied_skill_preserves_the_bait_damage_and_refunds_only_the_paid_cast() {
    let mut support = cerydra::character(0, &[]);
    support.stats.speed_micros = 110_000_000;
    let plan = plan(&[ashveil(0, &[DAMNATION_TRAIL]), support]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    cerydra::grant_merit(&mut battle, 1, 0, 6);

    act(&mut battle, Action::Skill, "dpr:1");

    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct && hit.actor == 0)
        .collect();
    assert_eq!(hits.len(), 4);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].ashveil_state.gluttony, 1);
}

#[test]
fn marked_allied_attack_restores_fixed_energy_once_without_requiring_charge() {
    let mut character = ashveil(0, &[]);
    character.stats.energy_regeneration_bp = 15_000;
    let plan = plan(&[character, saber(0, &[], false)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let target = bait(&battle, 0).unwrap();
    battle.set_energy(0, 0, 0);

    allied_attack(&mut battle, target + 1, 3);
    assert_eq!(battle.units[0].energy_milli, 0);
    for (energy, charges, stacks) in [(15_500, 1, 2), (31_000, 0, 4), (39_000, 0, 4)] {
        allied_attack(&mut battle, target, 3);
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(battle.units[0].ashveil_state.charges, charges);
        assert_eq!(battle.units[0].ashveil_state.gluttony, stacks);
    }
    let follow_ups: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::FollowUp)
        .collect();
    assert_eq!(follow_ups.len(), 2);
    assert!(follow_ups.iter().all(|hit| hit.target == target));
    assert_eq!(battle.units[target].toughness, 390);
}

#[test]
fn enhanced_follow_up_stays_in_one_attack_and_retargets_after_defeat() {
    let mut plan = plan(&[ashveil(0, &[DAMNATION_TRAIL, PHANTOM_LIMB])]);
    for (enemy, hp) in plan.units[1..].iter_mut().zip([1_800, 7_000, 12_000]) {
        enemy.stats.hp = hp;
        enemy.stats.base_hp = hp;
        enemy.kit = Kit::Enemy(&FINITE);
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_gluttony(&mut battle, 0, 10);
    battle.set_energy(0, 0, 150_000);
    act(&mut battle, Action::Ultimate, "dpr:1");
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(bait(&battle, 0), Some(2));
    assert_eq!(battle.units[0].ashveil_state.charges, 3);
    assert_eq!(battle.units[0].ashveil_state.gluttony, 0);
    let follow_ups: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::FollowUp)
        .collect();
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(follow_ups[0].target_id.as_deref(), Some("dpr:1"));
    let hits: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::FollowUp,
                    ..
                }
            )
        })
        .map(|event| event.target_id.as_deref().unwrap())
        .collect();
    assert_eq!(hits, ["dpr:1", "dpr:2", "dpr:2", "dpr:2"]);
}

#[test]
fn eidolon_two_refunds_after_the_last_hit_and_eidolon_six_caps_lifetime_gains() {
    let plan = plan(&[ashveil(6, &[PHANTOM_LIMB])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 150_000);
    battle.units[0].ashveil_state.lifetime_gluttony = 28;
    set_gluttony(&mut battle, 0, 18);

    act(&mut battle, Action::Ultimate, "dpr:2");

    let state = &battle.units[0].ashveil_state;
    assert_eq!(state.gluttony, 7);
    assert_eq!(state.lifetime_gluttony, 30);
    assert_eq!(state.charges, 3);
    assert_eq!(battle.units[0].energy_milli, 10_000);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(hits.len(), 5);
    assert!(hits.windows(2).all(|pair| pair[1] < pair[0]));
    assert_eq!(battle.units[2].toughness, 345);
    assert_eq!(battle.effective_attack(0), 1_400);
}

#[test]
fn team_auras_are_attributed_per_hit_and_follow_up_crit_is_separate() {
    let mut partner = saber(0, &[], false);
    partner.stats.crit_rate_bp = 10_000;
    partner.stats.crit_damage_bp = 0;
    let plan = plan(&[ashveil(6, &[ALPHA_WOLF]), partner]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let target = 3;
    let rules = battle.mechanics(0).unwrap();

    assert_eq!(rules.target_defense_down_bp(&battle, 0, target), 2_000);
    assert_eq!(rules.all_resistance_down_bp(&battle, 0, target), 2_000);
    assert_eq!(rules.target_vulnerability_bp(&battle, 0, target), 2_400);
    battle.units[target].hp /= 2;
    assert_eq!(rules.target_vulnerability_bp(&battle, 0, target), 3_600);
    assert_eq!(rules.ally_crit_damage_bonus_bp(&battle, 0, 1), 4_000);
    assert_eq!(
        rules.follow_up_crit_damage_bonus_bp(&battle, 0, target),
        8_000
    );

    allied_attack(&mut battle, target, 1);
    let hit = battle.dummy_damage.last().unwrap();
    assert!(hit.critical);
    assert!(
        hit.assists
            .iter()
            .any(|&(owner, amount)| owner == 0 && amount > 0)
    );
}
