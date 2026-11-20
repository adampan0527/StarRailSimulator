use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, support::OffenseContext},
    calculate_dpr, fixture,
};

use super::{BASIC, FORM, NETHERWORLD, REANIMATED, SCRIVENER, SKILL, TALENT, ULTIMATE};

fn hanya_damage_bonus(battle: &Battle<'_>, actor: usize, element: fixture::Element) -> u32 {
    battle
        .offense_buffs_from(
            0,
            OffenseContext {
                actor,
                enemy: None,
                element,
                kind: DamageKind::Direct,
            },
        )
        .damage_bp
}

fn hanya(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.hanya".into(),
        form_id: FORM.into(),
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 2_000,
            base_attack: 900,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.2, 2.0]),
            (ULTIMATE.into(), vec![0.36, 2.0, 0.15]),
            (TALENT.into(), vec![0.15, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn herta() -> OfficialCharacter {
    let mut input = hanya(0);
    input.unit_id = "ally.herta".into();
    input.form_id = "character.1013".into();
    input.energy_capacity = 110;
    input.stats.attack = 1_200;
    input.stats.base_attack = 800;
    input.stats.base_speed_micros = 150_000_000;
    input.stats.speed_micros = 150_000_000;
    input.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    input
}

fn prepared(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
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
        .unwrap_or_else(|| panic!("expected {actor} {action:?} {target:?} to be legal"));
    battle.apply(&command).unwrap();
}

fn reach_turn(battle: &mut Battle<'_>, unit: &str) {
    for _ in 0..12 {
        let actor = battle.ordinary_actor().unwrap().to_owned();
        if actor == unit {
            return;
        }
        act(battle, &actor, Action::Basic, Some("dpr:1"));
    }
    panic!("{unit} did not get a turn");
}

#[test]
fn burden_counts_its_application_and_same_target_recasts_and_moves_to_a_new_target() {
    let plan = prepared(&[hanya(0), herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    assert_eq!(battle.units[3].toughness, 980);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert_eq!(
        battle.units[0]
            .hanya_state
            .burden
            .unwrap()
            .qualifying_actions,
        1
    );
    assert_eq!(battle.skill_points, 2);
    assert_eq!(
        hanya_damage_bonus(&battle, 0, fixture::Element::Physical),
        0
    );

    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:1"));
    assert_eq!(battle.skill_points, 3);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].hanya_state.burden.unwrap().refunds, 1);
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.skill_points, 4);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:3"));
    let mark = battle.units[0].hanya_state.burden.unwrap();
    assert_eq!(
        (mark.target, mark.qualifying_actions, mark.refunds),
        (4, 1, 0)
    );
    reach_turn(&mut battle, "ally.herta");
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.units[0].hanya_state.burden.unwrap().refunds, 0);
    reach_turn(&mut battle, "ally.herta");
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:3"));
    assert_eq!(battle.units[0].hanya_state.burden.unwrap().refunds, 1);
    reach_turn(&mut battle, "ally.herta");
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:3"));
    reach_turn(&mut battle, "ally.herta");
    let events_before_final_attack = battle.events().len();
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:3"));
    assert!(battle.units[0].hanya_state.burden.is_none());
    assert_eq!(battle.skill_points, 5);
    // The second refund still consumes the marker when there is no room to gain a point.
    assert!(
        !battle.events()[events_before_final_attack..]
            .iter()
            .any(|event| { matches!(event.kind, EventKind::SkillPoints { .. }) })
    );
}

#[test]
fn multihit_aoe_gets_talent_before_first_hit_and_triggers_only_one_refund() {
    let mut support = hanya(6);
    support.enabled_trace_ids = [SCRIVENER.into(), REANIMATED.into()].into();
    support.stats.energy_regeneration_bp = 15_000;
    let plan = prepared(&[support, herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    act(&mut battle, "ally.herta", Action::Skill, None);

    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1 && hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect();
    // Herta's 30/70 hits use 1200 ATK, 20% personal + 25% Sanction, and 0.6 mitigation.
    assert_eq!(hits, [156, 156, 156, 365, 365, 365]);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 118_000);
    let mark = battle.units[0].hanya_state.burden.unwrap();
    assert_eq!((mark.qualifying_actions, mark.refunds), (0, 1));
    assert_eq!(battle.effective_attack(1), 1_280);

    reach_turn(&mut battle, "ally.herta");
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:1"));
    assert_eq!(battle.effective_attack(1), 1_200);
    assert_eq!(hanya_damage_bonus(&battle, 1, fixture::Element::Ice), 2_500);
    reach_turn(&mut battle, "ally.herta");
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:1"));
    assert_eq!(hanya_damage_bonus(&battle, 1, fixture::Element::Ice), 0);
}

#[test]
fn follow_ups_and_ally_targeted_actions_do_not_consume_burden() {
    let mut plan = prepared(&[hanya(0), herta()]);
    plan.units[0].starting_energy_milli = 140_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    act(
        &mut battle,
        "ally.hanya",
        Action::Ultimate,
        Some("ally.herta"),
    );
    battle.follow_up(1, fixture::Element::Ice, 1_000, 0, Action::Basic, 0);
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:1"));
    assert!(
        battle
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::FollowUp))
    );
    assert_eq!(
        battle.units[0]
            .hanya_state
            .burden
            .unwrap()
            .qualifying_actions,
        1
    );
    assert_eq!(battle.skill_points, 3);
    assert_eq!(hanya_damage_bonus(&battle, 1, fixture::Element::Ice), 0);
}

#[test]
fn ultimate_snapshots_fractional_speed_and_scrivener_stacks_with_its_attack_buff() {
    let mut support = hanya(4);
    support.enabled_trace_ids = vec![SCRIVENER.into()];
    support.skill_parameters.get_mut(ULTIMATE).unwrap()[2] = 0.18125;
    let mut plan = prepared(&[support, herta()]);
    plan.units[0].starting_energy_milli = 140_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    assert_eq!(battle.ordinary_actor(), Some("ally.herta"));
    assert_eq!(battle.speed_micros(0), 240_000_000);
    act(
        &mut battle,
        "ally.hanya",
        Action::Ultimate,
        Some("ally.herta"),
    );
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.speed_micros(1), 193_500_000);
    assert_eq!(battle.effective_attack(1), 1_488);
    act(&mut battle, "ally.herta", Action::Basic, Some("dpr:2"));
    assert_eq!(battle.effective_attack(1), 1_568);

    for turn in 2..=4 {
        reach_turn(&mut battle, "ally.herta");
        assert_eq!(battle.units[1].normal_turns, turn);
        assert_eq!(battle.speed_micros(1), 193_500_000);
        act(&mut battle, "ally.herta", Action::Basic, Some("dpr:1"));
        if turn == 2 {
            assert_eq!(battle.effective_attack(1), 1_488);
        }
    }
    assert_eq!(battle.speed_micros(1), 150_000_000);
    assert_eq!(battle.effective_attack(1), 1_200);
}

#[test]
fn netherworld_refunds_on_defeat_and_e1_advances_hanya_once_for_multiple_kills() {
    let mut support = hanya(1);
    support.enabled_trace_ids = [NETHERWORLD.into(), REANIMATED.into()].into();
    let mut plan = prepared(&[support, herta()]);
    plan.units[0].starting_energy_milli = 140_000;
    plan.units[1].starting_energy_milli = 110_000;
    static FINITE_TARGET: fixture::EnemyKit = fixture::EnemyKit {
        stats: fixture::DUMMY_STATS,
        max_toughness: 100,
        weakness: [true; fixture::Element::COUNT],
        target: fixture::EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    for (slot, hp) in [(2, 1), (3, 1_900)] {
        plan.units[slot].kit = Kit::Enemy(&FINITE_TARGET);
        plan.units[slot].stats.hp = hp;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, "ally.hanya", Action::Skill, Some("dpr:2"));
    assert!(battle.hp(3).0 > 0);
    act(
        &mut battle,
        "ally.hanya",
        Action::Ultimate,
        Some("ally.herta"),
    );
    let before = battle.units[0].gauge_quanta;
    act(&mut battle, "ally.herta", Action::Ultimate, None);
    assert_eq!(battle.hp(2).0, 0);
    assert_eq!(battle.hp(3).0, 0);
    assert_eq!(
        battle.units[0].gauge_quanta,
        before - fixture::GAUGE_FULL / 100 * 15
    );
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert!(battle.units[0].hanya_state.burden.is_none());
}

#[test]
fn fixed_dpr_credits_hanya_and_repeats_exactly() {
    let team = [hanya(6), herta()];
    let plan = PreparedBattle::official_dpr(&team, 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert!(result.characters[0].own_damage > 0);
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(
        result,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
}
