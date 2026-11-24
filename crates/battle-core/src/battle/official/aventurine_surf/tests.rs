use crate::{
    Command, LogMode, PreparedBattle,
    battle::{Kit, official::yao_guang::tests::yao, support::OffenseContext},
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const ARGENTI: &str = "character.1302";
const AHA: &str = "character.1511";
const WAVES: &str = "trace.1513102";
const CRIT: &str = "trace.1513103";

fn surf(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(eidolon, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type.element = QUANTUM;
    actor.energy_capacity = 130;
    actor.stats.speed_micros = 140_000_000;
    actor.skill_parameters = [
        (BASIC.into(), vec![0.5]),
        (SKILL.into(), vec![1.2, 4.0, 4.0]),
        (ULTIMATE.into(), vec![2.4, 8.0, 6.0, 0.12, 4.0]),
        (TALENT.into(), vec![10.0, 0.2, 0.36, 30.0, 20.0, 1.0, 1.0]),
        (ELATION.into(), vec![0.3, 10.0, 0.09]),
        (ENHANCED_ELATION.into(), vec![0.3, 0.105, 10.0, 0.09]),
    ]
    .into();
    actor
}

fn argenti() -> OfficialCharacter {
    let mut actor = yao(0, &[]);
    actor.unit_id = ARGENTI.into();
    actor.form_id = ARGENTI.into();
    actor.combat_type.path = CombatPath::Erudition;
    actor.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    actor
}

fn aha(eidolon: u8) -> OfficialCharacter {
    let mut actor = yao(eidolon, &[]);
    actor.form_id = AHA.into();
    actor.unit_id = AHA.into();
    actor.combat_type.element = QUANTUM;
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

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
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

fn casts(battle: &Battle<'_>) -> Vec<(bool, u8, u32)> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::AventurineSurfElation {
                enhanced,
                heat,
                punchline,
            } => Some((enhanced, heat, punchline)),
            _ => None,
        })
        .collect()
}

fn instant(punchline: u32) -> AhaInstant {
    AhaInstant {
        punchline,
        skill_damage_factor_bp: 10_000,
    }
}

#[test]
fn own_skill_arms_fixed_cost_cast_then_only_aha_spends_heat_at_e0() {
    let prepared = plan(&[surf(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_heat(&mut battle, 0, 9);
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Skill, None);

    assert_eq!(casts(&battle), vec![(false, 0, 20)]);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 13);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert_eq!(battle.skill_points, 2);
    // Level 80, 500 defense and intact toughness give a combined 0.6 multiplier.
    let amounts: Vec<_> = battle
        .dummy_damage
        .iter()
        .take(10)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(
        amounts,
        [720, 720, 720, 904, 904, 904, 1_877, 1_877, 1_877, 563]
    );
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.actor == 0 && hit.action == Action::Skill && hit.kind == DamageKind::Elation
    }));
    battle.resolve_elation_skill(0, instant(12));
    assert_eq!(casts(&battle)[1], (false, 0, 12));
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 13);

    let before_hits = battle.dummy_damage.len();
    let punchline = battle.current_punchline();
    battle.resolve_natural_aha_turn();
    assert_eq!(casts(&battle)[2], (true, 13, punchline));
    assert_eq!(battle.dummy_damage.len() - before_hits, 3 + 10 + 13);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 0);
    assert_eq!(battle.units[0].aventurine_surf_state.queued_thresholds, 0);
    assert_eq!(battle.units[0].energy_milli, 110_000);
}

#[test]
fn solo_elation_attack_classifies_as_follow_up_and_advances_aha_speed() {
    let prepared = plan(&[surf(0, &[WAVES]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let initial = battle.aha_speed_micros().unwrap();
    for _ in 0..2 {
        battle.ordinary_actor = Some(1);
        act(&mut battle, ARGENTI, Action::Basic, Some("dpr:2"));
    }

    assert_eq!(battle.certified_banger(0), Some(4));
    assert_eq!(battle.current_punchline(), 5);
    assert_eq!(battle.aha_speed_micros(), Some(initial + 50_000_000));
    assert!(battle.is_follow_up_damage(0, Some(Action::ElationSkill), DamageKind::Elation));
    assert!(!battle.is_follow_up_damage(0, Some(Action::Skill), DamageKind::Elation));
    battle.resolve_natural_aha_turn();
    assert_eq!(battle.aha_speed_micros(), Some(initial));
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(FORM) && matches!(event.kind, EventKind::FollowUp)
    }));
    for _ in 0..2 {
        battle.units[0].normal_turns += 1;
        battle.finish_turn(0);
    }
    assert!(battle.certified_banger(0).is_some());
    battle.units[0].normal_turns += 1;
    battle.finish_turn(0);
    assert_eq!(battle.certified_banger(0), None);
}

#[test]
fn ascension_adds_solo_effects_only_for_the_aha_pair() {
    for eidolon in [0, 2] {
        let prepared = plan(&[surf(0, &[WAVES]), aha(eidolon), argenti()]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let rules = battle.mechanics(0).unwrap();
        assert_eq!(rules.ally_elation_bp(&battle, 0, 0), 10_000);
        assert_eq!(rules.ally_elation_bp(&battle, 0, 2), 2_000);
        assert!(!battle.is_follow_up_damage(0, Some(Action::ElationSkill), DamageKind::Elation));
        battle
            .mechanics(1)
            .unwrap()
            .activate_special_ultimate(&mut battle, 1);
        act(&mut battle, AHA, Action::Ultimate, None);

        let level = if eidolon >= 2 { 2 } else { 1 };
        assert_eq!(
            rules.ally_speed_bonus_bp(&battle, 0, 2),
            500 + u32::from(level) * 1_000
        );
        assert!(battle.is_follow_up_damage(0, Some(Action::ElationSkill), DamageKind::Elation));
        let banger = battle.certified_banger(0).unwrap();
        let punchline = battle.current_punchline();
        let heat = battle.units[0].aventurine_surf_state.heat;
        battle.ordinary_actor = Some(2);
        act(&mut battle, ARGENTI, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.certified_banger(0), Some(banger + 2));
        assert_eq!(battle.current_punchline(), punchline + 2 + u32::from(level));
        assert_eq!(battle.units[0].aventurine_surf_state.heat, heat + 1 + level);
    }
}

#[test]
fn e6_upgrades_after_two_casts_but_keeps_heat_outside_aha() {
    let prepared = plan(&[surf(6, &[WAVES]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.resolve_elation_skill(0, instant(12));
    battle.resolve_elation_skill(0, instant(12));
    assert_eq!(casts(&battle), vec![(false, 0, 12); 2]);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 8);
    battle.units[0].aventurine_surf_state.queued_thresholds = 2;
    set_heat(&mut battle, 0, 29);

    let before_hits = battle.dummy_damage.len();
    battle.ordinary_actor = Some(1);
    act(&mut battle, ARGENTI, Action::Basic, Some("dpr:2"));
    assert_eq!(casts(&battle)[2], (true, 0, 20));
    assert_eq!(battle.dummy_damage.len() - before_hits, 1 + 3 + 10 + 30);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 34);
    let punchline = battle.current_punchline();
    battle.resolve_natural_aha_turn();
    assert_eq!(casts(&battle)[3], (true, 34, punchline));
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 4);
}

#[test]
fn e2_raises_the_cap_and_queues_each_of_five_thresholds_once() {
    let prepared = plan(&[surf(2, &[]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    for _ in 0..60 {
        rules.on_ally_attack(&mut battle, 0, 1, Action::Basic, Some(2));
        battle.resolve_follow_ups();
    }
    assert_eq!(casts(&battle), vec![(false, 0, 20); 5]);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 50);
    let before_hits = battle.dummy_damage.len();
    let punchline = battle.current_punchline();
    battle.resolve_natural_aha_turn();
    assert_eq!(casts(&battle)[5], (true, 50, punchline));
    assert_eq!(battle.dummy_damage.len() - before_hits, 3 + 10 + 50);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 4);
}

#[test]
fn speed_crit_and_defense_traces_use_the_correct_sources_and_limits() {
    let prepared = plan(&[surf(4, &["trace.1513101", CRIT]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.personal_elation_bp(&battle, 0), 3_000);
    for _ in 0..7 {
        rules.on_ally_attack(&mut battle, 0, 1, Action::Basic, Some(2));
    }
    battle.resolve_follow_ups();
    assert_eq!(battle.units[0].aventurine_surf_state.trace_activations, 6);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 27);
    let context = OffenseContext {
        actor: 1,
        max_hp_source: 1,
        enemy: Some(2),
        element: Element::Physical,
        kind: DamageKind::Direct,
        break_damage: false,
        follow_up: false,
        summon: false,
        direct_skill: false,
    };
    assert_eq!(battle.offense_buffs_from(0, context).crit_damage_bp, 4_800);
    assert_eq!(battle.offense_buffs_from(0, context).defense_ignore_bp, 0);
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].aventurine_surf_state.trace_activations, 0);
    assert_eq!(
        battle.offense_buffs_from(0, context).defense_ignore_bp,
        1_800
    );
    battle.fill_energy(0);
    battle.ordinary_actor = Some(1);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.speed_micros(0), 152_000_000);
    assert_eq!(rules.personal_elation_bp(&battle, 0), 4_200);
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 47);
}

#[test]
fn lethal_attack_defers_a_new_threshold_to_the_next_wave() {
    const FINITE: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 1_000,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[surf(0, &[])]);
    prepared.wave_ends = vec![2, 4];
    prepared.units[1].stats.hp = 1;
    prepared.units[1].kit = Kit::Enemy(&FINITE);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_heat(&mut battle, 0, 9);
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Skill, None);

    assert_eq!(battle.wave_index, 1);
    assert_eq!(casts(&battle), vec![(false, 0, 20)]);
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.action == Action::ElationSkill)
    );
    assert_eq!(battle.units[0].aventurine_surf_state.heat, 13);
}
