use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{analysis::DummyDamage, official::saber::tests::saber},
    calculate_dpr,
};

use super::*;

fn gilgamesh(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: THUNDER,
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
            base_aggro: 125,
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
            (SKILL.into(), vec![1.4, 0.7, 0.1, 10.0, 0.15, 3.0, 2.0]),
            (ULTIMATE.into(), vec![2.0, 0.5, 10.0]),
            (TALENT.into(), vec![3.0, 10.0, 0.2, 0.1]),
            (JOINT.into(), vec![4.4, 6.6, 3.0, 120.0, 8.0, 2.16]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 0)
        .unwrap()
        .with_dummies(&[400, 400, 400])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command));
    battle.resolve_command(&command).unwrap();
}

fn direct_hits<'a>(battle: &'a Battle<'_>, actor: usize, action: Action) -> Vec<&'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct && hit.actor == actor && hit.action == action)
        .collect()
}

#[test]
fn opening_basic_is_automatic_and_threshold_permanently_unlocks_the_free_skill() {
    let plan = plan(&[gilgamesh(0, &[HEROS_HAUTEUR]), saber(0, &[], true)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(saber::FORM));
    assert_eq!(direct_hits(&battle, 0, Action::Basic).len(), 1);
    assert_eq!(direct_hits(&battle, 0, Action::Basic)[0].amount, 300);
    assert_eq!(battle.units[0].energy_milli, 200_000);
    assert_eq!(battle.skill_points, 0);

    act(&mut battle, saber::FORM, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.units[0].gilgamesh_state.interest, 1);
    assert_eq!(battle.speed_micros(0), 210_000_000);
    gain_interest(&mut battle, 0, 9);
    assert_eq!(battle.speed_micros(0), 300_000_000);
    assert_eq!(battle.units[0].gilgamesh_state.lifetime_interest, 6);
    battle.ordinary_actor = Some(0);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|command| command.action != Action::Basic)
    );
    battle.skill_points = 0;
    act(&mut battle, FORM, Action::Skill, Some("dpr:2"));
    let state = &battle.units[0].gilgamesh_state;
    assert_eq!(state.interest, 0);
    assert_eq!(state.lifetime_interest, 6);
    assert!(state.piqued);
    assert_eq!(battle.skill_points, 0);
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 3);
}

#[test]
fn ally_ultimate_returns_thirty_percent_of_energy_paid_and_adds_three_interest() {
    let plan = plan(&[gilgamesh(0, &[EPICS_OPENING]), saber(0, &[], true)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(1, 1, 360_000);
    let before = battle.units[0].energy_milli;
    act(&mut battle, saber::FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli - before, 108_000);
    assert_eq!(battle.units[0].gilgamesh_state.interest, 3);
    assert!(battle.units[0].gilgamesh_state.burden_expires.is_some());
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(saber::FORM)
            && event.target_id.as_deref() == Some(FORM)
            && matches!(event.kind, EventKind::Energy { before_milli, after_milli } if after_milli - before_milli == 108_000)
    }));
}

#[test]
fn e1_e2_and_e4_use_fixed_refund_and_keep_the_shared_defense_ignore_timed() {
    let plan = plan(&[gilgamesh(4, &[HEGEMONS_STRIFE]), saber(0, &[], true)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].gilgamesh_state.interest, 5);
    gain_interest(&mut battle, 0, 5);
    battle.ordinary_actor = Some(0);
    battle.set_energy(0, 0, 0);
    act(&mut battle, FORM, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.units[0].energy_milli, 76_000);

    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.ally_attack_bonus_bp(&battle, 0, 0), 18_000);
    assert_eq!(mechanics.ally_crit_damage_bonus_bp(&battle, 0, 1), 12_000);
    assert_eq!(
        mechanics.damage_target_aura(&battle, 0, 1).defense_down_bp,
        1_500
    );
    let expires = battle.units[0]
        .gilgamesh_state
        .acknowledgement_expires
        .unwrap();
    battle.units[0].normal_turns = expires - 1;
    mechanics.after_ordinary_turn(&mut battle, 0, 0);
    assert_eq!(
        mechanics.damage_target_aura(&battle, 0, 1).defense_down_bp,
        1_500
    );
    battle.units[0].normal_turns = expires;
    mechanics.after_ordinary_turn(&mut battle, 0, 0);
    assert_eq!(
        mechanics.damage_target_aura(&battle, 0, 1).defense_down_bp,
        0
    );
}

#[test]
fn e6_caps_golden_rule_consumes_it_once_and_applies_only_ultimate_crit() {
    let mut gilgamesh = gilgamesh(6, &[]);
    gilgamesh.stats.crit_rate_bp = 10_000;
    let plan = plan(&[gilgamesh, saber(0, &[], true)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    for _ in 0..4 {
        rules.on_ally_ultimate_started(&mut battle, 0, 1, 0);
    }
    assert_eq!(battle.units[0].gilgamesh_state.golden_rule, 3);
    assert_eq!(
        rules.crit_damage_bonus_bp(&battle, 0, 2, Some(Action::Skill), DamageKind::Direct),
        0
    );
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 1).resistance_down_bp,
        2_000
    );
    let mut no_golden_rule = battle.clone();
    no_golden_rule.units[0].gilgamesh_state.golden_rule = 0;

    for state in [&mut battle, &mut no_golden_rule] {
        state.set_energy(0, 0, 360_000);
        act(state, FORM, Action::Ultimate, None);
    }
    let boosted = direct_hits(&battle, 0, Action::Ultimate);
    let baseline = direct_hits(&no_golden_rule, 0, Action::Ultimate);
    assert_eq!(boosted.len(), 13);
    assert_eq!(baseline.len(), 13);
    for (boosted, baseline) in boosted.iter().zip(baseline) {
        assert_eq!(boosted.target, baseline.target);
        assert!(boosted.amount.abs_diff(baseline.amount * 4) <= 3);
    }
    assert!(boosted[3].amount * 20 >= boosted[0].amount * 13 - 20);
    assert!(boosted[3].amount * 20 <= boosted[0].amount * 13 + 20);
    assert_eq!(battle.units[0].gilgamesh_state.golden_rule, 0);
    assert_eq!(battle.units[0].gilgamesh_state.spent_golden_rule, 0);
}

#[test]
fn eighth_attack_resolves_two_joint_sources_and_arms_one_saber_ultimate() {
    let mut input = gilgamesh(0, &[]);
    input.stats.speed_micros = 100_000_000;
    let plan = plan(&[input, saber(0, &[], true)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(1, 1, 240_000);
    battle.action = Some(Action::Basic);
    for count in 1..=8 {
        let actor = (count % 2) as usize;
        battle.begin_attack();
        battle.attack_hit(actor, 3, THUNDER, 100, 0);
        battle.notify_attack(actor, Action::Basic, Some(3));
        battle.resolve_follow_ups();
        assert_eq!(
            battle.units[0].gilgamesh_state.joint_count,
            if count < 8 { count } else { 0 }
        );
    }
    battle.action = None;
    assert_eq!(battle.units[0].gilgamesh_state.interest, 3);
    assert_eq!(battle.units[1].energy_milli, 360_000);
    let joint_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .collect();
    assert_eq!(joint_hits.len(), 6);
    assert_eq!(joint_hits[0].actor, 0);
    assert_eq!(joint_hits[0].amount, 2_640);
    assert_eq!(joint_hits[3].actor, 1);
    assert_eq!(joint_hits[3].amount, 3_960);

    let mut baseline = battle.clone();
    baseline.units[1].saber_state.arm_gilgamesh_ultimate(10_000);
    act(&mut battle, saber::FORM, Action::Ultimate, None);
    act(&mut baseline, saber::FORM, Action::Ultimate, None);
    let buffed = direct_hits(&battle, 1, Action::Ultimate);
    let base = direct_hits(&baseline, 1, Action::Ultimate);
    for (buffed, base) in buffed.iter().zip(base) {
        assert!(buffed.amount > 2 * base.amount);
        assert_eq!(
            buffed.assists.as_slice(),
            &[(0, buffed.amount - base.amount)]
        );
    }
    battle.set_energy(1, 1, 360_000);
    act(&mut battle, saber::FORM, Action::Ultimate, None);
    assert!(
        direct_hits(&battle, 1, Action::Ultimate)
            .iter()
            .skip(13)
            .all(|hit| hit.assists.is_empty())
    );
}

#[test]
fn three_cycle_dpr_replays_automatic_basic_and_credits_saber_support() {
    let mut input = gilgamesh(2, &[EPICS_OPENING, HEGEMONS_STRIFE]);
    input.stats.speed_micros = 200_000_000;
    let plan = plan(&[input, saber(0, &[], false)]);
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.action == Action::Basic));
    assert!(hits().any(|hit| hit.source_id == FORM && hit.action == Action::Skill));
    assert!(hits().any(|hit| {
        hit.source_id == saber::FORM
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == FORM && credit.amount > 0)
    }));
    assert!(
        hits().all(|hit| hit.credits.iter().map(|credit| credit.amount).sum::<u32>() == hit.amount)
    );
}
