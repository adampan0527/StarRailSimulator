use crate::{
    Command, LogMode, PreparedBattle,
    battle::Kit,
    battle::official::yao_guang::tests::yao,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn wolf(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(eidolon, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type.element = IMAGINARY;
    actor.energy_capacity = RATING_THRESHOLD;
    actor.stats.base_speed_micros = 160_000_000;
    actor.stats.speed_micros = 160_000_000;
    actor.skill_parameters = [
        (BASIC.into(), vec![0.5]),
        (SKILL.into(), vec![0.8, 5.0]),
        (ULTIMATE.into(), vec![60.0, 0.0, 0.45, 0.2, 0.2, 3.0, 2.0]),
        (TALENT.into(), vec![60.0, 240.0, 0.2, 0.002, 3.0, 0.004]),
        (
            ENHANCED.into(),
            vec![1.2, 0.0, 100.0, 0.5, 3.0, 60.0, 2.0, 0.15],
        ),
        (ELATION.into(), vec![15.0]),
        (ENHANCED_ELATION.into(), vec![0.45, 6.0]),
    ]
    .into();
    actor
}

fn aha(eidolon: u8) -> OfficialCharacter {
    let mut actor = yao(eidolon, &[]);
    actor.form_id = "character.1511".into();
    actor.unit_id = actor.form_id.clone();
    actor.combat_type.element = Element::Quantum;
    actor.energy_capacity = 0;
    actor.skill_parameters = [
        ("skill.151101".into(), vec![0.25, 1.0]),
        ("skill.151102".into(), vec![0.1, 0.25, 0.0, 0.0, 1.0, 1.0]),
        ("skill.151103".into(), vec![8.0, 1.2, 5.0, 0.36, 0.2, 3.0]),
        (
            "skill.151104".into(),
            vec![1.0, 40.0, 1.0, 0.0, 20.0, 8.0, 1.0, 8.0, 0.2, 3.0, 10.0],
        ),
        ("skill.151120".into(), vec![0.3, 0.2]),
        (
            "skill.151129".into(),
            if eidolon >= 2 {
                vec![0.1, 0.6, 0.2]
            } else {
                vec![0.1, 0.3, 0.12]
            },
        ),
        ("skill.151131".into(), vec![0.25, 1.0, 0.1]),
    ]
    .into();
    actor
}

fn plan(team: &[OfficialCharacter], targets: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&vec![1_000; targets])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
        decision_index: battle.decision_index,
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

fn instant(punchline: u32) -> AhaInstant {
    AhaInstant {
        punchline,
        skill_damage_factor_bp: 10_000,
    }
}

#[test]
fn hidden_mmr_from_skill_and_elation_gates_the_free_ultimate_and_converts_crit() {
    let prepared = plan(
        &[wolf(
            0,
            &["trace.1506101", "trace.1506102", "trace.1506103"],
        )],
        2,
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(state(&battle, 0).rating, 1);
    assert!(matches!(
        battle.check_action(0, Action::Ultimate),
        Err(BattleError::InsufficientUltimateResource)
    ));
    assert_eq!(rules.personal_elation_bp(&battle, 0), 5_000);
    battle.ordinary_actor = Some(0);
    act(&mut battle, Action::Skill, None);
    assert_eq!(state(&battle, 0).rating, 6);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Elation)
            .count(),
        2
    );
    battle.resolve_elation_skill(0, instant(40));
    assert_eq!(state(&battle, 0).rating, 61);
    battle.ordinary_actor = Some(0);
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(state(&battle, 0).rating, 81);
    assert_eq!(battle.units[0].energy_milli, 0);
    assert!(rules.immune_to_enemy_control(&battle, 0));
    assert!(!rules.can_use_ultimate(&battle, 0));
    assert_eq!(
        rules.crit_rate_bonus_bp(&battle, 0, 1, Some(Action::Basic), DamageKind::Elation),
        1_620
    );
    assert_eq!(
        rules.crit_rate_overflow_damage_bp(&battle, 0, 11_619),
        3_200
    );
    assert_eq!(
        rules.crit_rate_overflow_damage_bp(&battle, 0, 15_000),
        3_240
    );
}

#[test]
fn enhanced_basic_uses_one_hundred_bounces_three_boxes_and_a_finisher_without_generating_sp() {
    let prepared = plan(&[wolf(0, &[])], 1);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_rating(&mut battle, 0, 60);
    battle.grant_certified_banger(0, 40);
    act(&mut battle, Action::Ultimate, None);

    for remaining in [2, 1, 0] {
        battle.ordinary_actor = Some(0);
        let before = battle.dummy_damage.len();
        let count_sp = |battle: &Battle<'_>| {
            battle
                .events()
                .iter()
                .filter(|event| {
                    event.kind
                        == EventKind::SkillPoints {
                            before: 3,
                            after: 4,
                        }
                })
                .count()
        };
        let generated = count_sp(&battle);
        act(&mut battle, Action::Basic, None);
        let hits = &battle.dummy_damage[before..];
        assert_eq!(
            hits.iter()
                .filter(|hit| hit.kind == DamageKind::Elation && hit.action == Action::Basic)
                .count(),
            104
        );
        assert_eq!(count_sp(&battle), generated);
        assert_eq!(
            state(&battle, 0)
                .godmode
                .as_ref()
                .map_or(0, |godmode| godmode.basics_left),
            remaining
        );
    }
    assert_eq!(state(&battle, 0).rating, 0);
}

#[test]
fn spending_a_point_rolls_a_guaranteed_box_and_the_enhanced_elation_skill_resets_its_odds() {
    let prepared = plan(&[wolf(4, &[])], 3);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_rating(&mut battle, 0, 60);
    act(&mut battle, Action::Ultimate, None);
    let rules = battle.mechanics(0).unwrap();
    battle.spend_skill_points_with_substitute(0, 0, 1);
    assert!(state(&battle, 0).pending_boxes.is_empty());
    battle.spend_skill_points(0, 1);
    let godmode = state(&battle, 0).godmode.as_ref().unwrap();
    assert_eq!(godmode.box_chance_bp, 2_000);
    assert_eq!(state(&battle, 0).pending_boxes.len(), 1);
    rules.on_ally_action_finished(&mut battle, 0, 0, Action::Skill);
    assert!(state(&battle, 0).pending_boxes.is_empty());
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::Elation)
            .count(),
        3
    );
    let before = battle.dummy_damage.len();
    battle.resolve_elation_skill(0, instant(20));
    assert_eq!(
        state(&battle, 0).godmode.as_ref().unwrap().box_chance_bp,
        FULL_CHANCE_BP
    );
    assert_eq!(
        battle.dummy_damage[before..]
            .iter()
            .filter(|hit| hit.actor == 0
                && hit.kind == DamageKind::Elation
                && hit.action == Action::ElationSkill)
            .count(),
        6
    );
}

#[test]
fn lethal_bounce_resumes_on_the_next_wave_without_consuming_an_enhanced_basic_early() {
    let mut prepared = plan(&[wolf(0, &[])], 2);
    prepared.wave_ends = vec![2, 3];
    prepared.units[1].kit = Kit::Enemy(&FINITE);
    prepared.units[1].stats.hp = 1;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_rating(&mut battle, 0, 60);
    battle.grant_certified_banger(0, 40);
    act(&mut battle, Action::Ultimate, None);
    battle.ordinary_actor = Some(0);
    let av = battle.av_micros;
    act(&mut battle, Action::Basic, None);

    assert_eq!(battle.wave_index, 1);
    let godmode = state(&battle, 0).godmode.as_ref().unwrap();
    assert_eq!(godmode.basics_left, 3);
    assert_eq!(godmode.continuation.unwrap().shots, 1);
    assert!(battle.start_extra_turn());
    assert_eq!(battle.av_micros, av);
    act(&mut battle, Action::Basic, None);
    let godmode = state(&battle, 0).godmode.as_ref().unwrap();
    assert_eq!(godmode.basics_left, 2);
    assert!(godmode.continuation.is_none());
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 2
                && hit.kind == DamageKind::Elation
                && hit.action == Action::Basic)
            .count(),
        103
    );
}

#[test]
fn ascended_banger_grants_rating_and_e2_counts_initial_and_new_rating_thresholds() {
    for eidolon in [0, 2] {
        let prepared = plan(&[wolf(2, &["trace.1506103"]), aha(eidolon)], 1);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        set_rating(&mut battle, 0, 118);
        battle
            .mechanics(1)
            .unwrap()
            .activate_special_ultimate(&mut battle, 1);
        let command = Command {
            actor_id: "character.1511".into(),
            action: Action::Ultimate,
            target_id: None,
            decision_index: battle.decision_index,
        };
        battle.resolve_command(&command).unwrap();
        assert!(battle.elation_ascension().is_some());
        battle.grant_certified_banger(0, 10);
        let expected = if eidolon == 0 { 120 } else { 122 };
        assert_eq!(state(&battle, 0).rating, expected);
        act(&mut battle, Action::Ultimate, None);
        assert_eq!(state(&battle, 0).godmode.as_ref().unwrap().basics_left, 4);
        assert!(battle.start_extra_turn());
        battle.add_punchline(1, 1_000);
        let cap = if eidolon == 0 { 360 } else { 999 };
        assert_eq!(state(&battle, 0).rating, cap);
        assert_eq!(
            state(&battle, 0).godmode.as_ref().unwrap().basics_left,
            3 + (cap / 120) as u8
        );
    }
}

#[test]
fn eidolon_six_implants_all_weaknesses_on_each_wave_and_reduces_native_resistance() {
    let mut prepared = plan(&[wolf(6, &[]), yao(0, &[])], 2);
    prepared.wave_ends = vec![3, 4];
    for unit in &mut prepared.units[2..] {
        unit.kit = Kit::Enemy(&FINITE);
        unit.stats.hp = 10_000;
        unit.stats.resist_bp[Element::Physical.index()] = 4_000;
    }
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert!(
        Element::ALL
            .into_iter()
            .all(|element| battle.has_weakness(2, element))
    );
    assert!(
        battle.units[2]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::RestrictedWeakness
                && status.bp == 2_000
                && status.source == 0)
    );
    assert!(!battle.has_weakness(3, IMAGINARY));
    battle.units[2].hp = 0;
    battle.resolve_wave_or_end();
    assert!(
        Element::ALL
            .into_iter()
            .all(|element| battle.has_weakness(3, element))
    );
    assert_eq!(battle.plan.units[3].id, "dpr:2");
    assert_eq!(battle.plan.units[2].id, LEFT);
}

#[test]
fn restricted_weakness_alone_credits_the_native_resistance_change_to_silver_wolf() {
    let simulate = |eidolon| {
        let mut ally = yao(0, &[]);
        ally.stats.crit_rate_bp = 0;
        let mut prepared = plan(&[wolf(eidolon, &[]), ally], 2);
        prepared.units[2].stats.resist_bp[Element::Physical.index()] = 4_000;
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        assert!(state(&battle, 0).godmode.is_none());
        battle.action = Some(Action::Basic);
        for target in [2, 3] {
            battle.attack_hit(1, target, Element::Physical, 10_000, 0);
        }
        battle
            .dummy_damage
            .iter()
            .map(|hit| {
                let credit = hit.assists.iter().find(|&&(owner, _)| owner == 0);
                (hit.amount, credit.map_or(0, |(_, amount)| *amount))
            })
            .collect::<Vec<_>>()
    };
    let baseline = simulate(0);
    let modified = simulate(6);
    assert_eq!(baseline.len(), 2);
    assert_eq!(modified.len(), 2);
    for ((base, none), (actual, credit)) in baseline.into_iter().zip(modified) {
        assert_eq!(none, 0);
        assert!(credit > 0);
        assert_eq!(actual, base + credit);
    }
}
