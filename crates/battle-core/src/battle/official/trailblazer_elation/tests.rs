use crate::{Command, EventKind, LogMode, PreparedBattle, battle::official::yao_guang::tests::yao};

use super::*;
use crate::battle::combat_status::DebuffKind;
use crate::battle::support::OffenseContext;

const AHA: &str = "character.1511";
const YAO: &str = "character.1502";

fn trailblazer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(eidolon, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type.element = THUNDER;
    actor.energy_capacity = 160;
    actor.stats.attack = 1_600;
    actor.stats.energy_regeneration_bp = 12_000;
    actor.skill_parameters = [
        (BASIC.into(), vec![0.5]),
        (SKILL.into(), vec![0.3, 20.0]),
        (ULTIMATE.into(), vec![0.3, 3.0, 0.5, 10.0, 20.0, 5.0]),
        (TALENT.into(), vec![10.0, 3.0, 0.15]),
        (ELATION_SKILL.into(), vec![8.0, 0.1, 0.3]),
    ]
    .into();
    actor
}

fn aha(eidolon: u8) -> OfficialCharacter {
    let mut actor = yao(0, &[]);
    actor.form_id = AHA.into();
    actor.unit_id = AHA.into();
    actor.combat_type.element = Element::Quantum;
    actor.energy_capacity = 0;
    actor.eidolon = eidolon;
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

fn arlan() -> OfficialCharacter {
    let mut actor = trailblazer(0, &[]);
    actor.form_id = "character.1008".into();
    actor.unit_id = actor.form_id.clone();
    actor.combat_type.path = CombatPath::Destruction;
    actor.energy_capacity = 110;
    actor.stats.base_speed_micros = 90_000_000;
    actor.stats.speed_micros = 90_000_000;
    actor.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    actor
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn buff_context(actor: usize, target: usize) -> OffenseContext {
    OffenseContext {
        actor,
        max_hp_source: actor,
        enemy: Some(target),
        element: Element::Thunder,
        kind: DamageKind::Elation,
        break_damage: false,
        follow_up: false,
        summon: false,
        direct_skill: false,
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
        decision_index: battle.decision_index,
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

#[test]
fn skill_uses_highest_banger_and_keeps_fixed_energy_separate() {
    let prepared = plan(&[trailblazer(1, &[CLOUD_NINE, SIC_EM]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor, Some(0));
    battle.grant_certified_banger(1, 60);
    battle.units[0].elation_trailblazer_state.next_skill_bonus = true;
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli, 126_000);
    assert_eq!(battle.current_punchline(), 5);
    assert_eq!(battle.certified_banger(0), Some(22));
    assert_eq!(battle.units[0].elation_trailblazer_state.e1_stacks, 1);
    assert!(!battle.units[0].elation_trailblazer_state.next_skill_bonus);
    assert_eq!(
        battle.mechanics(0).unwrap().elation_from_attack_bp(1_600),
        3_000
    );
    assert_eq!(
        battle.mechanics(0).unwrap().elation_from_attack_bp(9_000),
        6_000
    );
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.action == Action::Skill && hit.assists.is_empty())
        .collect();
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .count(),
        3
    );
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::Elation)
            .count(),
        3
    );
    let talent_damage = |battle: &Battle<'_>| -> u32 {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| {
                hit.actor == 0
                    && hit.action == Action::Skill
                    && hit.kind == DamageKind::Elation
                    && hit.assists.is_empty()
            })
            .map(|hit| hit.amount)
            .sum()
    };
    let mut no_other_banger = Battle::start(&prepared, LogMode::Detailed);
    no_other_banger.units[0]
        .elation_trailblazer_state
        .next_skill_bonus = true;
    act(&mut no_other_banger, FORM, Action::Skill, None);
    assert!(talent_damage(&battle) > talent_damage(&no_other_banger));
}

#[test]
fn target_ultimate_grants_stacked_banger_and_one_elation_skill_without_a_clock_turn() {
    let prepared = plan(&[trailblazer(6, &[SCREW_IT, SIC_EM]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(0, 0, 160_000);
    battle.units[0].elation_trailblazer_state.e1_stacks = 3;
    let gauge = battle.aha.as_ref().unwrap().gauge_quanta;
    let at = battle.events().len();

    act(&mut battle, FORM, Action::Ultimate, Some(YAO));

    assert_eq!(battle.certified_banger(1), Some(16));
    assert_eq!(battle.units[0].elation_trailblazer_state.e1_stacks, 0);
    assert!(battle.units[0].elation_trailblazer_state.next_skill_bonus);
    assert_eq!(battle.skill_points, 4);
    let support = battle.offense_buffs_from(0, buff_context(1, 2));
    assert_eq!(support.elation_bp, 1_200);
    assert_eq!(support.crit_damage_bp, 3_000);
    assert_eq!(battle.aha.as_ref().unwrap().gauge_quanta, gauge);
    let events = &battle.events()[at..];
    assert!(!events.iter().any(|event| matches!(
        event.kind,
        EventKind::AhaInstant { .. } | EventKind::AscendedAhaInstant { .. }
    )));
    let elation_actors: Vec<_> = events
        .iter()
        .filter(|event| event.kind == EventKind::Action(Action::ElationSkill))
        .map(|event| event.source_id.as_deref().unwrap())
        .collect();
    assert_eq!(elation_actors, vec![YAO]);
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.actor == 1 && hit.kind == DamageKind::Elation)
    );
}

#[test]
fn ultimate_cleanses_and_advances_a_non_elation_ally_without_triggering_elation() {
    let prepared = plan(&[trailblazer(2, &[]), arlan()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(0, 0, 160_000);
    let before = battle.units[1].gauge_quanta;
    assert!(battle.apply_debuff(2, 1, DebuffKind::SkillImprisonment, 1_000, 1));
    assert!(battle.is_crowd_controlled(1));

    act(&mut battle, FORM, Action::Ultimate, Some("character.1008"));

    assert!(!battle.is_crowd_controlled(1));
    assert_eq!(battle.certified_banger(1), None);
    assert_eq!(
        battle.units[1].gauge_quanta,
        before.saturating_sub(crate::battle::resolve::gauge_for(5_000))
    );
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::Action(Action::ElationSkill))
    );
}

#[test]
fn directed_elation_waits_for_a_new_wave_at_the_same_action_value() {
    let mut prepared = plan(&[trailblazer(0, &[]), yao(0, &[])]);
    prepared.wave_ends = vec![3, 5];
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[2].hp = 0;
    battle.set_energy(0, 0, 160_000);
    let before = battle.av_micros;
    let cursor = battle.events().len();

    act(&mut battle, FORM, Action::Ultimate, Some(YAO));

    assert_eq!(battle.wave_index, 1);
    let events = &battle.events()[cursor..];
    let wave = events
        .iter()
        .position(|event| event.kind == EventKind::WaveStarted)
        .unwrap();
    let elation = events
        .iter()
        .position(|event| event.kind == EventKind::Action(Action::ElationSkill))
        .unwrap();
    assert!(wave < elation);
    assert_eq!(events[elation].av_micros, before);
    assert!(battle.dummy_damage.iter().all(|hit| hit.target >= 3));
}

#[test]
fn elation_attack_has_eight_bounces_and_toughness_only_in_the_finisher() {
    let prepared = plan(&[trailblazer(6, &[SIC_EM])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.resolve_elation_skill(
        0,
        AhaInstant {
            punchline: 20,
            skill_damage_factor_bp: 10_000,
        },
    );
    assert_eq!(battle.dummy_damage.len(), 11);
    assert!(
        battle
            .dummy_damage
            .iter()
            .all(|hit| hit.kind == DamageKind::Elation)
    );
    assert_eq!(battle.units[0].energy_milli, 96_000);
    assert_eq!(battle.current_punchline(), 4);
    for enemy in battle.enemies() {
        assert_eq!(battle.units[enemy].toughness, 980);
        assert_eq!(battle.vulnerability_from(0, enemy), 1_000);
    }
}

#[test]
fn active_ascension_strikes_with_trailblazer_before_aha_and_replaces_the_truth_basis() {
    for eidolon in [1, 2, 4] {
        let prepared = plan(&[trailblazer(0, &[]), aha(eidolon)]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        for _ in 0..3 {
            battle.add_punchline(0, 40);
            battle.resolve_natural_aha_turn();
        }
        act(&mut battle, AHA, Action::Ultimate, None);
        assert_eq!(
            battle.elation_ascension().unwrap().innate_level,
            if eidolon >= 2 { 2 } else { 1 }
        );
        battle.set_energy(0, 0, 160_000);
        let start = battle.dummy_damage.len();
        let at = battle.events().len();

        act(&mut battle, FORM, Action::Ultimate, Some(AHA));

        let all_crit_bp = if eidolon >= 2 { 6_000 } else { 5_000 };
        for actor in [0, 1] {
            assert_eq!(
                battle
                    .offense_buffs_from(0, buff_context(actor, 2))
                    .crit_damage_bp,
                all_crit_bp
            );
        }

        let events = &battle.events()[at..];
        let order: Vec<_> = events
            .iter()
            .filter(|event| event.kind == EventKind::Action(Action::ElationSkill))
            .map(|event| event.source_id.as_deref().unwrap())
            .collect();
        assert_eq!(order, vec![FORM, AHA]);
        let hits = &battle.dummy_damage[start..];
        let boosted: u64 = hits
            .iter()
            .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::Elation)
            .map(|hit| u64::from(hit.amount))
            .sum();
        let truth: Vec<_> = hits
            .iter()
            .filter(|hit| hit.actor == 1 && hit.kind == DamageKind::TrueDamage)
            .map(|hit| u64::from(hit.amount))
            .collect();
        assert!(boosted > 0);
        assert_eq!(truth.len(), 3);
        assert_eq!(truth.iter().sum::<u64>(), boosted / 5);
        assert!(truth.iter().max().unwrap() - truth.iter().min().unwrap() <= 1);
    }
}
