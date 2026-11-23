use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const DAN_HENG: &str = "dan_heng.hunt";
const ORIGINAL_HIMEKO: &str = "character.1003";
const OFF_ELEMENT_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 10,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn character(
    form: &str,
    path: CombatPath,
    element: Element,
    energy_capacity: u32,
    skill_parameters: &[(&str, &[f64])],
) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: form.into(),
        form_id: form.into(),
        combat_type: CombatType { path, element },
        level: 80,
        energy_capacity,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
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
        skill_parameters: skill_parameters
            .iter()
            .map(|&(id, parameters)| (id.into(), parameters.into()))
            .collect(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

fn nova(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut kit = character(
        FORM,
        CombatPath::Erudition,
        FIRE,
        150,
        &[
            (BASIC, &[0.5]),
            (SKILL, &[0.1, 3.0]),
            (
                ULTIMATE,
                &[0.16, 1.0, 0.1, 1.0, 0.15, 3.0, 0.4, 3.0, 3.81, 1.26],
            ),
            (TALENT, &[0.4, 0.1, 4.0]),
            (BEAM, &[0.16, 1.0, 3.0]),
            (PULSE, &[0.1, 1.0, 0.15]),
            (FINISHER, &[3.0, 0.4]),
            (ASSIST, &[0.8, 3.0, 0.12, 1.0, 4.0, 0.18, 4.0, 2.0]),
            (
                VERDICT,
                &[0.8, 3.0, 0.12, 1.0, 4.0, 0.18, 4.0, 0.5, 2.0, 0.5],
            ),
            (
                DECIMATION,
                &[0.8, 3.0, 0.12, 1.0, 4.0, 0.18, 4.0, 0.5, 9.0, 0.5],
            ),
        ],
    );
    kit.eidolon = eidolon;
    kit.enabled_trace_ids = traces.iter().map(|id| (*id).into()).collect();
    kit
}

fn dan_heng() -> OfficialCharacter {
    character(
        DAN_HENG,
        CombatPath::Hunt,
        Element::Wind,
        100,
        &[
            ("skill.100201", &[0.5]),
            ("skill.100202", &[1.3, 0.12, 2.0, 0.5]),
            ("skill.100203", &[2.4, 0.72]),
            ("skill.100204", &[0.18, 2.0]),
        ],
    )
}

fn original_himeko() -> OfficialCharacter {
    character(
        ORIGINAL_HIMEKO,
        CombatPath::Erudition,
        FIRE,
        120,
        &[
            ("skill.100301", &[0.5]),
            ("skill.100302", &[1.0, 0.4]),
            ("skill.100303", &[1.38, 5.0]),
            ("skill.100304", &[0.7, 3.0]),
        ],
    )
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[2_000, 2_000, 2_000])
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
    battle.resolve_command(&command).unwrap();
}

fn direct_hits<'a>(battle: &'a Battle<'_>, actor: usize, action: Action) -> Vec<&'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == actor && hit.action == action && hit.kind == DamageKind::Direct)
        .collect()
}

#[test]
fn ally_assist_consumes_one_shared_use_and_gives_the_companion_back_a_turn() {
    let plan = plan(&[nova(0, &[FREE_ASSIST, EXTRA_TURN]), dan_heng()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.ordinary_actor = Some(1);
    let before = battle.units[1].energy_milli;
    let own_before = battle.units[0].energy_milli;
    let charges = battle.units[0].himeko_departure.assist_uses;
    let skill_points = battle.skill_points;
    assert_eq!(charges, 1);
    let invalid = Command {
        decision_index: battle.decision_index,
        actor_id: DAN_HENG.into(),
        action: Action::AssistSkill,
        target_id: Some("dpr:1".into()),
    };
    assert!(matches!(
        battle.resolve_command(&invalid),
        Err(BattleError::InvalidTarget)
    ));
    assert_eq!(battle.decision_index, invalid.decision_index);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 1);
    act(&mut battle, DAN_HENG, Action::AssistSkill, None);
    assert_eq!(battle.units[0].himeko_departure.protocol, Protocol::Verdict);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 0);
    assert_eq!(battle.units[1].energy_milli - before, 4_000);
    assert_eq!(battle.units[0].energy_milli - own_before, 18_000);
    assert_eq!(battle.skill_points, skill_points);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 6);
    assert!(direct_hits(&battle, 1, Action::Skill).is_empty());
    battle.advance_to_decision();
    assert!(battle.in_extra_turn(1));
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::AssistSkill)
    );
    act(&mut battle, DAN_HENG, Action::Basic, Some("dpr:2"));
    assert!(!battle.in_extra_turn(1));
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::AssistSkill, None);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 0);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 13);
}

#[test]
fn cresset_refills_to_two_uses_only_at_the_owners_ordinary_opening() {
    let plan = plan(&[nova(2, &[FREE_ASSIST, EXTRA_TURN]), dan_heng()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 2);
    let saved = battle.units[0].himeko_departure.cresset_expires;
    set_assist_uses(&mut battle, 0, 0);
    battle.notify_ally_turn_opening(0, true);
    battle.notify_ally_turn_opening(1, false);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 0);
    battle.units[0].normal_turns = saved - 1;
    battle.notify_ally_turn_opening(0, false);
    assert!(cresset_active(&battle, 0));
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 2);
    battle.units[0].normal_turns = saved;
    set_assist_uses(&mut battle, 0, 0);
    battle.notify_ally_turn_opening(0, false);
    assert!(!cresset_active(&battle, 0));
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 0);
}

#[test]
fn assist_breaks_off_element_toughness_without_enabling_it_for_basic_attacks() {
    let mut plan = plan(&[nova(0, &[])]);
    plan.units[1].kit = Kit::Enemy(&OFF_ELEMENT_ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[1].toughness, 10);
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::AssistSkill, None);
    assert_eq!(battle.units[1].toughness, 0);
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(FORM)
            && event.target_id.as_deref() == Some("dpr:1")
            && matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::WeaknessBreak,
                    ..
                }
            )
    }));
}

#[test]
fn verdict_counts_other_ultimates_limits_automatic_uses_and_refreshes_on_ultimate() {
    let plan = plan(&[nova(0, &[]), dan_heng()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.ordinary_actor = Some(1);
    act(&mut battle, DAN_HENG, Action::AssistSkill, None);
    let send_ultimate = |battle: &mut Battle<'_>| {
        battle.ordinary_actor = Some(1);
        battle.set_energy(1, 1, 100_000);
        act(battle, DAN_HENG, Action::Ultimate, Some("dpr:2"));
    };
    send_ultimate(&mut battle);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 6);
    send_ultimate(&mut battle);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 13);
    for _ in 0..4 {
        send_ultimate(&mut battle);
    }
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 20);
    assert_eq!(battle.units[0].himeko_departure.protocol_uses, 2);
    assert_eq!(battle.units[0].himeko_departure.assist_uses, 0);
    battle.set_energy(0, 0, ENERGY_CAP);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].himeko_departure.protocol_uses, 0);
    send_ultimate(&mut battle);
    send_ultimate(&mut battle);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 27);
}

#[test]
fn decimation_counts_unique_attacked_targets_and_cannot_chain_its_own_assist() {
    let plan = plan(&[nova(0, &[]), original_himeko()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.ordinary_actor = Some(1);
    act(&mut battle, ORIGINAL_HIMEKO, Action::AssistSkill, None);
    assert_eq!(
        battle.units[0].himeko_departure.protocol,
        Protocol::Decimation
    );
    assert_eq!(battle.units[0].himeko_departure.protocol_progress, 3);
    let send_ultimate = |battle: &mut Battle<'_>| {
        battle.ordinary_actor = Some(1);
        battle.set_energy(1, 1, 120_000);
        act(battle, ORIGINAL_HIMEKO, Action::Ultimate, None);
    };
    send_ultimate(&mut battle);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 6);
    send_ultimate(&mut battle);
    assert_eq!(direct_hits(&battle, 0, Action::Skill).len(), 13);
    assert_eq!(battle.units[0].himeko_departure.protocol_progress, 0);
    assert_eq!(battle.units[0].himeko_departure.protocol_uses, 1);
}

#[test]
fn ultimate_pays_150_and_resolves_six_beams_two_pulses_and_three_final_hits() {
    let plan = plan(&[nova(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, ENERGY_CAP);
    act(&mut battle, FORM, Action::Ultimate, None);
    let hits = direct_hits(&battle, 0, Action::Ultimate);
    assert_eq!(hits.len(), 33);
    assert_eq!(hits.iter().filter(|hit| hit.amount == 96).count(), 18);
    assert_eq!(hits.iter().filter(|hit| hit.amount == 60).count(), 6);
    assert_eq!(hits.iter().filter(|hit| hit.amount == 90).count(), 6);
    assert_eq!(hits.iter().filter(|hit| hit.amount == 240).count(), 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[0].himeko_departure.source_energy, 0);
}

#[test]
fn e6_trace_pulse_spends_six_and_adds_three_area_bursts() {
    let plan = plan(&[nova(6, &[INITIAL_SOURCE])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_source(&mut battle, 0, 3);
    battle.set_energy(0, 0, ENERGY_CAP);
    act(&mut battle, FORM, Action::Ultimate, None);
    let hits = direct_hits(&battle, 0, Action::Ultimate);
    assert_eq!(hits.len(), 57);
    assert_eq!(battle.units[0].himeko_departure.source_energy, 0);
    assert_eq!(battle.units[0].himeko_departure.protocol_uses, 0);
}

#[test]
fn fixed_dpr_replays_assists_ultimate_and_team_damage_credit() {
    let plan = PreparedBattle::official_dpr(
        &[
            nova(2, &[FREE_ASSIST, EXTRA_TURN, INITIAL_SOURCE]),
            dan_heng(),
        ],
        42,
        3,
    )
    .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.action == Action::Skill));
    assert!(hits().any(|hit| hit.source_id == FORM && hit.action == Action::Ultimate));
    assert!(hits().any(|hit| {
        hit.source_id == DAN_HENG
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == FORM && credit.amount > 0)
    }));
    assert!(
        hits().all(|hit| hit.amount == hit.credits.iter().map(|credit| credit.amount).sum::<u32>())
    );
}
