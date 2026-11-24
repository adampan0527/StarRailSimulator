use crate::{
    Command, EventKind, LogMode, PreparedBattle,
    battle::{combat_status::DebuffKind, official::yao_guang::tests::yao, support::AllyBuffKind},
};

use super::*;

const WATCH: &str = "trace.1505101";
const WEIGH: &str = "trace.1505102";
const BLOOM: &str = "trace.1505103";
const AHA: &str = "character.1511";
const HUOHUO: &str = "character.1217";

fn evanescia(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(eidolon, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type.element = PHYSICAL;
    actor.energy_capacity = ENERGY_CAP;
    actor.stats.elation_bp = 4_000;
    actor.stats.crit_rate_bp = 10_000;
    actor.skill_parameters = [
        (BASIC.into(), vec![0.5]),
        (SKILL.into(), vec![1.0, 1.5, 0.75, 10.0]),
        (ULTIMATE.into(), vec![0.8, 5.0, 0.72, 88.0]),
        (
            TALENT.into(),
            vec![0.5, 0.125, 240.0, 10.0, 0.2, 0.12, 0.08, 0.14],
        ),
        (ELATION.into(), vec![5.0, 0.55]),
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

fn huohuo() -> OfficialCharacter {
    let mut actor = yao(0, &[]);
    actor.form_id = HUOHUO.into();
    actor.unit_id = HUOHUO.into();
    actor.combat_type.path = CombatPath::Abundance;
    actor.combat_type.element = Element::Wind;
    actor.energy_capacity = 140;
    actor.skill_parameters = [
        ("skill.121701".into(), vec![0.25]),
        ("skill.121702".into(), vec![0.14, 140.0, 0.112, 112.0, 1.0]),
        ("skill.121703".into(), vec![0.15, 0.24, 2.0]),
        (
            "skill.121704".into(),
            vec![2.0, 1.0, 0.03, 0.0, 30.0, 0.5, 6.0],
        ),
    ]
    .into();
    actor
}

fn plan(team: &[OfficialCharacter], dummies: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(dummies)
        .unwrap()
}

fn cast(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<String>) {
    let command = Command {
        actor_id: actor.into(),
        action,
        target_id: target,
        decision_index: battle.decision_index,
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

#[test]
fn energy_and_banger_convert_once_and_fox_uses_a_separate_counter() {
    let mut prepared = plan(&[evanescia(1, &[WEIGH])], &[1_000; 3]);
    prepared.units[0].starting_energy_milli = 0;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.restore_energy_flat(0, 0, 240_000);
    assert_eq!(battle.certified_banger(0), Some(100));
    assert_eq!(battle.units[0].energy_milli, 240_000);
    assert_eq!(battle.units[0].evanescia_state.pending_fox_attacks, 1);

    let before = battle.events().len();
    battle.resolve_follow_ups();
    let events = &battle.events()[before..];
    let fox = events
        .iter()
        .position(|event| matches!(event.kind, EventKind::FollowUp))
        .unwrap();
    let elation = events
        .iter()
        .position(|event| matches!(event.kind, EventKind::Action(Action::ElationSkill)))
        .unwrap();
    assert!(fox < elation);
    assert_eq!(battle.units[0].energy_milli, 270_000);
    assert_eq!(battle.certified_banger(0), Some(130));
    assert_eq!(battle.units[0].evanescia_state.fox_energy_milli, 30_000);
    assert_eq!(battle.units[0].evanescia_state.pending_fox_attacks, 0);
    for target in battle.enemies() {
        assert!(battle.units[target].debuffs.iter().any(|debuff| {
            debuff.kind == DebuffKind::Vulnerability && debuff.source == 0 && debuff.bp == 1_200
        }));
    }
}

#[test]
fn a_support_ultimate_resolves_fox_in_the_same_command() {
    let mut prepared = plan(&[evanescia(0, &[]), huohuo()], &[1_000]);
    prepared.units[0].starting_energy_milli = 0;
    prepared.units[1].starting_energy_milli = 140_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.restore_energy_flat(0, 0, 230_000);
    let start = battle.events().len();
    cast(&mut battle, HUOHUO, Action::Ultimate, None);
    assert!(battle.events()[start..].iter().any(|event| {
        event.kind == EventKind::FollowUp && event.source_id.as_deref() == Some(FORM)
    }));
    assert_eq!(battle.units[0].energy_milli, 312_000);
    assert_eq!(battle.units[0].evanescia_state.pending_fox_attacks, 0);
}

#[test]
fn teammate_banger_gain_and_expiration_obey_own_trace_ratios() {
    let mut prepared = plan(&[evanescia(2, &[WATCH, BLOOM]), yao(0, &[])], &[1_000]);
    prepared.units[0].starting_energy_milli = 0;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.grant_certified_banger(1, 40);
    assert_eq!(battle.certified_banger(0), Some(30));
    assert_eq!(battle.units[0].energy_milli, 30_000);
    battle.spend_certified_banger(1, 10);
    assert_eq!(battle.certified_banger(0), Some(30));
    battle.units[1].normal_turns = 2;
    battle.expire_certified_banger(1);
    assert_eq!(battle.certified_banger(0), Some(60));
    assert_eq!(battle.units[0].energy_milli, 60_000);
    assert_eq!(battle.certified_banger(1).unwrap_or(0), 0);
}

#[test]
fn skill_pays_one_point_and_adds_talent_hits_to_the_blast() {
    let prepared = plan(&[evanescia(0, &[])], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.grant_certified_banger(0, 10);
    let target = battle.plan.units[2].id.clone();
    battle.ordinary_actor = Some(0);
    let before = battle.current_punchline();
    cast(&mut battle, FORM, Action::Skill, Some(target));

    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.current_punchline(), before + 10);
    for kind in [DamageKind::Direct, DamageKind::Elation] {
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .filter(|hit| hit.action == Action::Skill && hit.kind == kind)
                .count(),
            3,
        );
    }
    assert_eq!(battle.units[0].energy_milli, 280_000);
    assert_eq!(battle.certified_banger(0), Some(40));
}

#[test]
fn ultimate_pays_240_and_uses_the_target_count_and_banger_floor() {
    for (count, direct, energy) in [(1, 10, 15_000), (2, 9, 15_000), (3, 9, 15_000)] {
        let prepared = plan(&[evanescia(0, &[WATCH])], &vec![1_000; count]);
        let run = |banger| {
            let mut battle = Battle::start(&prepared, LogMode::Detailed);
            battle.grant_certified_banger(0, banger);
            // Preserve the seeded energy and pre-cast Banger independently.
            battle.set_energy(0, 0, 250_000);
            cast(&mut battle, FORM, Action::Ultimate, None);
            assert_eq!(battle.units[0].energy_milli, energy);
            let hits = || {
                battle
                    .dummy_damage
                    .iter()
                    .filter(|hit| hit.action == Action::Ultimate)
            };
            assert_eq!(
                hits().filter(|hit| hit.kind == DamageKind::Direct).count(),
                direct
            );
            assert_eq!(
                hits().filter(|hit| hit.kind == DamageKind::Elation).count(),
                direct
            );
            hits()
                .filter(|hit| hit.kind == DamageKind::Elation)
                .map(|hit| hit.amount)
                .collect::<Vec<_>>()
        };
        assert_eq!(run(10), run(100));
    }
}

#[test]
fn ascension_overflow_refund_skips_banger_and_arms_only_one_ultimate() {
    for (eidolon, overflow, boost_factor) in [(0, 60_000, 17_500), (2, 240_000, 25_000)] {
        let prepared = plan(&[evanescia(0, &[]), aha(eidolon)], &[1_000; 3]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let aha_rules = battle.mechanics(1).unwrap();
        aha_rules.activate_special_ultimate(&mut battle, 1);
        cast(&mut battle, AHA, Action::Ultimate, None);
        assert!(battle.elation_ascension().is_some());
        battle.units[0].energy_milli = 480_000;
        battle.restore_energy_flat(0, 0, 300_000);
        assert_eq!(
            battle.units[0].evanescia_state.overflow_energy_milli,
            overflow
        );
        battle.grant_certified_banger(0, 960);
        let mechanics = battle.mechanics(0).unwrap();
        mechanics.before_action(&mut battle, 0, Action::Ultimate, None);
        assert_eq!(
            battle.units[0].evanescia_state.active_ultimate_factor_bp,
            boost_factor
        );
        assert!(!battle.units[0].evanescia_state.next_ultimate_amplified);
        assert_eq!(
            mechanics.damage_multiplier_bp(
                &battle,
                0,
                2,
                Some(Action::Ultimate),
                DamageKind::Elation,
                1_000
            ),
            boost_factor / 10
        );
        battle.units[0].energy_milli = 0;
        let banger = battle.certified_banger(0);
        mechanics.after_action(&mut battle, 0, Action::Ultimate, None);
        assert_eq!(battle.units[0].energy_milli, overflow);
        assert_eq!(battle.certified_banger(0), banger);
        assert_eq!(battle.units[0].evanescia_state.overflow_energy_milli, 0);
        assert_eq!(battle.units[0].evanescia_state.active_ultimate_factor_bp, 0);
    }
}

#[test]
fn crit_damage_converts_and_preserves_the_buff_provider_credit() {
    let prepared = plan(&[evanescia(0, &[]), yao(0, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.intrinsic_elation_bp(0), 5_000);
    battle.apply_ally_buff(1, 0, AllyBuffKind::CritDamage, 5_000, 2);
    assert_eq!(battle.intrinsic_elation_bp(0), 6_000);
    let target = battle.enemies().start;
    Evanescia::elation_hit(&mut battle, 0, target, Action::Skill, 5_000, 100);
    let hit = battle.dummy_damage.last().unwrap();
    assert!(
        hit.assists
            .iter()
            .any(|&(actor, amount)| actor == 1 && amount > 0)
    );
    assert!(hit.assists.iter().map(|&(_, amount)| amount).sum::<u32>() < hit.amount);
}

#[test]
fn sixth_eidolon_extends_banger_and_refunds_first_and_fifth_ultimate() {
    let prepared = plan(&[evanescia(6, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.certified_banger_duration(), 3);
    battle.grant_certified_banger(0, 250);
    assert_eq!(rules.ally_merrymake_bp(&battle, 0, 0), 1_900);
    assert_eq!(rules.defense_ignore_bp(&battle, 0, 1), 1_500);
    assert_eq!(rules.resistance_penetration_bp(&battle, 0, PHYSICAL), 2_000);
    for expected in [120_000, 0, 0, 0, 120_000] {
        battle.units[0].energy_milli = 0;
        rules.after_action(&mut battle, 0, Action::Ultimate, None);
        assert_eq!(battle.units[0].energy_milli, expected);
    }
}
