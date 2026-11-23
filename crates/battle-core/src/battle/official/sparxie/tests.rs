use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    calculate_dpr,
};

use super::*;

const YAO: &str = "character.1502";
const MIDDLE: &str = "dpr:2";

pub(in crate::battle::official) fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Elation,
            element: FIRE,
        },
        energy_capacity: ENERGY,
        level: 80,
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
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            elation_bp: 0,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![20.0, 28.0, 3.0, 7.0, 4.0]),
            (ULTIMATE.into(), vec![2.0, 0.5, 0.6]),
            (TALENT.into(), vec![0.20, 0.48, 0.40, 0.20]),
            (ENHANCED_BASIC.into(), vec![1.0, 0.5]),
            (ENGAGEMENT_FARMING.into(), vec![2.0, 1.0, 2.0, 0.2, 0.1]),
            (ELATION_SKILL.into(), vec![0.25, 0.5, 20.0, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn yao(speed_micros: u64) -> OfficialCharacter {
    let mut unit = character(0, &[]);
    unit.unit_id = YAO.into();
    unit.form_id = YAO.into();
    unit.combat_type.element = Element::Physical;
    unit.energy_capacity = 180;
    unit.stats.speed_micros = speed_micros;
    unit.stats.elation_bp = 10_000;
    unit.skill_parameters = [
        ("skill.150201".into(), vec![0.9, 0.3]),
        ("skill.150202".into(), vec![3.0, 0.2, 3.0]),
        ("skill.150203".into(), vec![5.0, 0.2, 3.0, 20.0, 1.0]),
        ("skill.150204".into(), vec![0.2]),
        ("skill.150220".into(), vec![1.0, 1.0, 0.16, 3.0, 5.0, 0.2]),
    ]
    .into();
    unit
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        actor_id: actor.into(),
        action,
        decision_index: battle.decision_index,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

#[test]
fn registers_low_level_values_and_rejects_invalid_frozen_caps() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut low = character(0, &[]);
    for (skill, row) in [
        (BASIC, vec![0.5]),
        (ULTIMATE, vec![2.0, 0.3, 0.6]),
        (TALENT, vec![0.1, 0.24, 0.2, 0.1]),
        (ENHANCED_BASIC, vec![0.5, 0.25]),
        (ENGAGEMENT_FARMING, vec![2.0, 1.0, 2.0, 0.1, 0.05]),
        (ELATION_SKILL, vec![0.125, 0.25, 20.0, 2.0]),
    ] {
        low.skill_parameters.insert(skill.into(), row);
    }
    assert!(PreparedBattle::official_dpr(std::slice::from_ref(&low), 42, 3).is_ok());
    low.skill_parameters.get_mut(SKILL).unwrap()[0] = 21.0;
    assert!(PreparedBattle::official_dpr(&[low], 42, 3).is_err());
}

#[test]
fn twenty_interactions_pay_thrill_first_without_a_turn_or_skill_event() {
    let prepared = plan(&[character(2, &[])], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_skill_points(0, 0);
    let unavailable = command(&battle, FORM, Action::Skill, None);
    assert!(matches!(
        battle.apply(&unavailable),
        Err(BattleError::InsufficientSkillPoints)
    ));
    set_thrill(&mut battle, 0, 21);
    let first_av = battle.av_micros;
    for _ in 0..20 {
        act(&mut battle, FORM, Action::Skill, None);
    }
    assert_eq!(battle.av_micros, first_av);
    assert_eq!(battle.units[0].sparxie_state.live_interactions, [20, 0]);
    assert_eq!(battle.units[0].sparxie_state.thrill, 1);
    assert_eq!(battle.units[0].sparxie_state.thrill_crit_expires, [3; 4]);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::Action(Action::Skill)))
    );
    assert_eq!(battle.units[0].sparxie_state.live_straight_fire, [3, 0]);
    assert_eq!(battle.current_punchline(), 24);
    let capped = command(&battle, FORM, Action::Skill, None);
    assert!(matches!(
        battle.apply(&capped),
        Err(BattleError::IllegalTiming)
    ));
    assert_eq!(battle.units[0].sparxie_state.thrill, 1);
}

#[test]
fn straight_fire_guarantee_starts_at_four_and_survives_livestream_settlement() {
    let prepared = plan(&[character(0, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    set_thrill(&mut battle, 0, 3);
    for _ in 0..3 {
        act(&mut battle, FORM, Action::Skill, None);
    }
    assert!(battle.units[0].sparxie_state.live_straight_fire[0] > 0);
    battle.units[0]
        .sparxie_state
        .interactions_since_straight_fire = Some(6);
    let settle = command(&battle, FORM, Action::Basic, Some("dpr:1"));
    battle.resolve_command(&settle).unwrap();
    assert_eq!(battle.units[0].sparxie_state.live_straight_fire, [0, 0]);
    assert_eq!(
        battle.units[0]
            .sparxie_state
            .interactions_since_straight_fire,
        Some(6)
    );
    battle.advance_to_decision();
    let before = battle.current_punchline();
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.current_punchline(), before + 2);
    assert_eq!(
        battle.units[0]
            .sparxie_state
            .interactions_since_straight_fire,
        Some(0)
    );
}

#[test]
fn enhanced_basic_adds_each_livestream_bonus_and_ends_one_turn() {
    let prepared = plan(&[character(0, &[])], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    // Advance only the command on purpose so automatic Aha actions are excluded.
    set_thrill(&mut battle, 0, 3);
    for _ in 0..3 {
        act(&mut battle, FORM, Action::Skill, None);
    }
    let av = battle.av_micros;
    let points = battle.skill_points;
    let finish = command(&battle, FORM, Action::Basic, Some(MIDDLE));
    battle.resolve_command(&finish).unwrap();
    assert_eq!(battle.units[0].sparxie_state.live_interactions, [0, 0]);
    assert_eq!(battle.skill_points, (points + 1).min(5));
    assert_eq!(battle.units[0].energy_milli, 120_000);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.av_micros == av)
        .collect();
    // Level 80 ATK 1000 times 160%/80%, defense 2/3, unbroken factor 0.9.
    assert_eq!(
        hits.iter()
            .take(3)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [960, 480, 480]
    );
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::Elation)
            .count(),
        6
    );
    assert_eq!(
        battle
            .enemies()
            .map(|enemy| battle.units[enemy].toughness)
            .sum::<u16>(),
        2_965
    );
}

#[test]
fn encore_respects_e6_cap_and_carries_toughness_thirds() {
    let prepared = plan(&[character(6, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.add_punchline(0, 60);
    battle.resolve_natural_aha_turn();
    assert_eq!(battle.dummy_damage.len(), 41);
    assert!(
        battle
            .dummy_damage
            .iter()
            .all(|hit| hit.kind == DamageKind::Elation)
    );
    assert_eq!(battle.units[1].toughness, 927);
    assert_eq!(battle.units[0].sparxie_state.thrill, 4);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.certified_banger(0), Some(61));
    assert_eq!(battle.current_punchline(), 6);
}

#[test]
fn yao_extra_turn_plays_both_skills_and_preserves_interrupted_livestream() {
    let prepared = plan(&[character(2, &[]), yao(180_000_000)], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    let natural_gauge = battle.aha.as_ref().unwrap().gauge_quanta;
    let punchline = battle.current_punchline();
    battle.fill_energy(1);
    act(&mut battle, YAO, Action::Ultimate, None);
    let participants: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| matches!(event.kind, EventKind::Action(Action::ElationSkill)))
        .map(|event| event.source_id.as_deref().unwrap())
        .collect();
    assert_eq!(participants, [YAO, FORM]);
    assert_eq!(battle.aha.as_ref().unwrap().gauge_quanta, natural_gauge);
    assert_eq!(battle.current_punchline(), punchline + 12);
    assert_eq!(battle.certified_banger(0), Some(20));
    assert_eq!(battle.certified_banger(1), Some(20));
    assert!(battle.in_extra_turn(0));
    assert_eq!(battle.units[0].sparxie_state.live_interactions, [1, 0]);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].sparxie_state.live_interactions, [1, 1]);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert!(!battle.in_extra_turn(0));
    assert_eq!(battle.units[0].sparxie_state.live_interactions, [1, 0]);
    let elation_hits = battle.dummy_damage.iter().filter(|hit| {
        hit.actor == 0 && hit.action == Action::Basic && hit.kind == DamageKind::Elation
    });
    // Three talent targets, one talent bounce, and two Yao Boons from the paid attack.
    assert_eq!(elation_hits.count(), 6);
}

#[test]
fn ultimate_uses_live_elation_and_attributes_yaos_zone() {
    let mut fire = character(4, &[SWEET, DAZZLING]);
    fire.stats.attack = 2_199;
    let prepared = plan(&[fire, yao(220_000_000)], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, YAO, Action::Skill, None);
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, None);
    let hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::Direct && hit.actor == 0)
        .unwrap();
    // 50% + 60% x (36% E4 + 5% ATK trace + 20% Yao) = 86.6%, with E1's 15% RES PEN.
    assert_eq!(hit.amount, 1_313);
    assert_eq!(hit.assists, [(1, 182)]);
    assert_eq!(battle.units[0].sparxie_state.thrill, 1);
    assert_eq!(battle.current_punchline(), 16);
}

#[test]
fn teammate_attack_buff_can_cross_attack_to_elation_threshold() {
    use crate::battle::support::AllyBuffKind;

    let mut fire = character(0, &[SWEET]);
    fire.stats.attack = 2_099;
    let prepared = plan(&[fire, yao(190_000_000)], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.apply_ally_buff(1, 0, AllyBuffKind::AttackFlat, 100, 2);
    talent_hit(&mut battle, 0, 2, 0, Action::Basic, 4_000, 0);
    let hit = &battle.dummy_damage[0];
    assert_eq!(hit.amount, 1_898);
    assert_eq!(hit.assists, [(1, 90)]);
}

#[test]
fn dual_elation_dpr_replays_and_conserves_damage_credit() {
    let roster = [character(2, &[SWEET, DAZZLING, SIZZLING]), yao(180_000_000)];
    let prepared = PreparedBattle::official_dpr(&roster, 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    for id in [FORM, YAO] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| hit.source_id == id && hit.kind == DamageKind::Elation)
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
