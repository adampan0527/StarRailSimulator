use crate::{
    Command, LogMode, PreparedBattle,
    battle::{
        combat_status::DebuffKind,
        official::{sparxie::tests::character as sparxie, yao_guang::tests::yao},
    },
};

use super::*;

const YAO: &str = "character.1502";
const AHA: &str = "character.1511";

fn pearl(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(eidolon, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type.element = ICE;
    actor.stats.defense = 3_000;
    actor.stats.base_defense = 3_000;
    actor.stats.speed_micros = 210_000_000;
    actor.stats.elation_bp = 10_000;
    actor.skill_parameters = [
        (BASIC.into(), vec![0.45]),
        (SKILL.into(), vec![15.0, 0.06, 120.0]),
        (
            ULTIMATE.into(),
            vec![3.0, 0.3, 20.0, 0.1, 0.15, 0.3, 60.0, 30.0],
        ),
        (TALENT.into(), vec![0.5, 0.6, 50.0, 0.15, 200.0]),
        (MOON_BASIC.into(), vec![0.5, 0.04, 1.0, 80.0, 0.1]),
        (WAVE_BASIC.into(), vec![0.5, 0.04, 1.0, 80.0]),
        (ELATION.into(), vec![0.05, 0.075, 0.1, 0.2]),
    ]
    .into();
    actor
}

fn aha(eidolon: u8) -> OfficialCharacter {
    let mut actor = yao(eidolon, &[]);
    actor.form_id = AHA.into();
    actor.unit_id = AHA.into();
    actor.combat_type.element = Element::Quantum;
    actor.energy_capacity = 0;
    let faces = if eidolon >= 2 {
        vec![0.1, 0.6, 0.2]
    } else {
        vec![0.1, 0.3, 0.12]
    };
    actor.skill_parameters = [
        ("skill.151101".into(), vec![0.25, 1.0]),
        ("skill.151102".into(), vec![0.1, 0.25, 0.0, 0.0, 1.0, 1.0]),
        ("skill.151103".into(), vec![8.0, 1.2, 5.0, 0.36, 0.2, 3.0]),
        (
            "skill.151104".into(),
            vec![1.0, 40.0, 1.0, 0.0, 20.0, 8.0, 1.0, 8.0, 0.2, 3.0, 10.0],
        ),
        ("skill.151120".into(), vec![0.3, 0.2]),
        ("skill.151129".into(), faces),
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

#[test]
fn skill_heals_the_lowest_ally_twice_and_cleanses_without_reviving() {
    let prepared = plan(&[
        pearl(0, &["trace.1503101", "trace.1503102"]),
        yao(0, &[]),
        sparxie(0, &[]),
    ]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.ordinary_actor = Some(0);
    battle.units[0].hp = 600;
    battle.units[1].hp = 100;
    battle.units[2].hp = 0;
    battle.apply_debuff(3, 0, DebuffKind::Slow, 2_000, 3);
    battle.apply_debuff(3, 1, DebuffKind::Slow, 2_000, 3);
    let before = battle.certified_banger(0).unwrap_or(0);
    act(&mut battle, FORM, Action::Skill, None);
    // 3000 DEF × 6% + 120, with 0.2 × (100% sheet + 50% vision) outgoing healing.
    assert_eq!(battle.units[0].hp, 990);
    assert_eq!(battle.units[1].hp, 880);
    assert_eq!(battle.units[2].hp, 0);
    assert!(battle.units[0].debuffs.is_empty());
    assert!(battle.units[1].debuffs.is_empty());
    assert_eq!(battle.certified_banger(0), Some(before + 15));
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.skill_points, 2);
}

#[test]
fn talent_spends_remainder_before_exhausting_a_point_and_stacks_with_ordinary_shields() {
    let prepared = plan(&[pearl(0, &["trace.1503102"]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.spend_certified_banger(0, 50);
    battle.grant_certified_banger(0, 1);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.effect_resistance_aura_bp(&battle, 0, 1), 5_000);
    for expected_hp in [960, 920, 880, 800] {
        battle.true_damage(2, 1, Action::Basic, 100, Vec::new());
        assert_eq!(battle.units[1].hp, expected_hp);
    }
    assert_eq!(battle.certified_banger(0).unwrap_or(0), 0);
    assert_eq!(mechanics.effect_resistance_aura_bp(&battle, 0, 1), 0);
    battle.units[1].hp = 500;
    assert_eq!(
        mechanics.ally_damage_reduction_bp_for_target(&battle, 0, 1),
        1_500
    );
    battle.units[1].hp = 501;
    assert_eq!(
        mechanics.ally_damage_reduction_bp_for_target(&battle, 0, 1),
        0
    );
    battle.grant_certified_banger(0, 1);
    battle.units[1].hp = 1_000;
    battle.apply_official_shield(1, 1, 100, 3, None);
    battle.true_damage(2, 1, Action::Basic, 200, Vec::new());
    assert_eq!(battle.units[1].hp, 960);
    assert_eq!(battle.shield_amount(1), 0);
    assert_eq!(battle.certified_banger(0), Some(1));
    assert_eq!(battle.units[0].pearl_state.repellency_spent, 60);

    battle.grant_certified_banger(0, 90);
    battle.units[0].normal_turns += 5;
    battle.expire_certified_banger(0);
    assert_eq!(battle.certified_banger(0), Some(50));
}

#[test]
fn learning_uses_three_defense_basics_then_expires_and_refunds_fixed_energy_once() {
    let mut target = yao(0, &[]);
    target.stats.elation_bp = 5_000;
    let prepared = plan(&[pearl(6, &["trace.1503103"]), target]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[0].energy_milli = 180_000;
    act(&mut battle, FORM, Action::Ultimate, Some(YAO));
    assert_eq!(battle.certified_banger(0), Some(20));
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.ally_resistance_penetration_bp(&battle, 0), 2_000);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    for charges in [2, 1, 0] {
        battle.ordinary_actor = Some(0);
        let hits = battle.dummy_damage.len();
        act(&mut battle, FORM, Action::Basic, None);
        let dealt = &battle.dummy_damage[hits..];
        assert_eq!(
            dealt
                .iter()
                .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::Direct)
                .count(),
            3
        );
        assert_eq!(
            dealt
                .iter()
                .filter(|hit| hit.actor == 1
                    && hit.kind == DamageKind::Elation
                    && hit.assists.iter().map(|entry| entry.1).sum::<u32>() == hit.amount)
                .count(),
            3
        );
        assert_eq!(
            battle.units[0]
                .pearl_state
                .learning
                .map_or(0, |learning| learning.charges),
            charges
        );
    }
    assert_eq!(mechanics.ally_resistance_penetration_bp(&battle, 0), 0);
    assert_eq!(battle.units[0].energy_milli, 95_000);
    battle.units[0].energy_milli = 0;
    mechanics.on_ally_action_finished(&mut battle, 0, 1, Action::Ultimate);
    mechanics.on_ally_action_finished(&mut battle, 0, 1, Action::Ultimate);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::StatusExpired(StatusKind::DeepLearning)
        && event.target_id.as_deref() == Some(YAO)));
}

#[test]
fn next_attack_addition_refreshes_once_per_unit_and_credits_pearl() {
    let mut target = yao(0, &[]);
    target.stats.elation_bp = 10_000;
    let prepared = plan(&[pearl(4, &[]), target]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let instant = AhaInstant {
        punchline: 40,
        skill_damage_factor_bp: 10_000,
    };
    battle.resolve_elation_skill(0, instant);
    battle.resolve_elation_skill(0, instant);
    assert_eq!(battle.units[0].pearl_state.next_attacks.len(), 2);
    assert_eq!(
        battle.units[0].pearl_state.next_attacks[0].1.multiplier_bp,
        1_500
    );
    assert_eq!(battle.units[0].energy_milli, 100_000);
    for expected in [3, 0] {
        battle.ordinary_actor = Some(1);
        let hits = battle.dummy_damage.len();
        act(&mut battle, YAO, Action::Basic, Some("dpr:2"));
        assert_eq!(
            battle.dummy_damage[hits..]
                .iter()
                .filter(|hit| {
                    hit.actor == 1
                        && hit.kind == DamageKind::Elation
                        && hit.assists == [(0, hit.amount)]
                })
                .count(),
            expected
        );
    }
    assert_eq!(battle.units[0].pearl_state.next_attacks.len(), 1);
}

#[test]
fn four_elation_advances_and_scopes_temporary_resources_to_the_granted_turn() {
    for (aha_eidolon, retain_banger, retain_punchline) in [(0, 6, 12), (2, 12, 24)] {
        let prepared = plan(&[
            pearl(2, &[]),
            yao(0, &[]),
            sparxie(0, &[]),
            aha(aha_eidolon),
        ]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle
            .mechanics(3)
            .unwrap()
            .activate_special_ultimate(&mut battle, 3);
        act(&mut battle, AHA, Action::Ultimate, None);
        battle.units[0].energy_milli = 180_000;
        battle.grant_certified_banger(0, 90);
        assert_eq!(battle.certified_banger(0), Some(75));

        let punchline = battle.current_punchline();
        let banger = battle.certified_banger(1).unwrap_or(0);
        let ordinary = battle.ordinary_actor;
        battle.queue_extra_turn(1);
        act(&mut battle, FORM, Action::Ultimate, Some(YAO));
        assert_eq!(battle.current_punchline(), punchline);
        assert!(battle.events().iter().any(|event| {
            event.source_id.as_deref() == Some(FORM)
                && event.target_id.as_deref() == Some(YAO)
                && matches!(event.kind, EventKind::GaugeShifted { .. })
        }));

        assert!(battle.start_extra_turn());
        assert_eq!(battle.current_punchline(), punchline);
        battle.finish_extra_turn();
        assert!(battle.start_extra_turn());
        assert_eq!(battle.current_punchline(), punchline + 120);
        assert_eq!(battle.certified_banger(1), Some(banger + 60));
        battle.add_punchline(1, 7);
        battle.finish_extra_turn();
        assert_eq!(battle.current_punchline(), punchline + 7 + retain_punchline);
        assert_eq!(battle.certified_banger(1), Some(banger + retain_banger));
        assert_eq!(battle.ordinary_actor, ordinary);

        let punchline = battle.current_punchline();
        battle.queue_granted_extra_turn(0, 1);
        assert!(battle.start_extra_turn());
        battle.end(crate::Outcome::Victory);
        assert_eq!(battle.current_punchline(), punchline + retain_punchline);
    }
}

#[test]
fn eidolon_one_prevents_two_fatal_hits_even_if_pearl_is_the_target() {
    let prepared = plan(&[pearl(1, &[]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.true_damage(2, 0, Action::Basic, 20_000, Vec::new());
    assert_eq!(battle.units[0].hp, 500);
    battle.true_damage(2, 1, Action::Basic, 20_000, Vec::new());
    assert_eq!(battle.units[1].hp, 500);
    battle.true_damage(2, 1, Action::Basic, 20_000, Vec::new());
    assert_eq!(battle.units[1].hp, 0);
}
