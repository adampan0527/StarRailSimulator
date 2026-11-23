use crate::{
    CombatType, Command, LogMode, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{
        Kit,
        official::{sparxie::tests::character as sparxie, yao_guang::tests::yao},
    },
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const YAO: &str = "character.1502";
const SPARXIE: &str = "character.1501";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn aha(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let mut actor = yao(0, traces);
    actor.form_id = FORM.into();
    actor.unit_id = FORM.into();
    actor.combat_type = CombatType {
        path: CombatPath::Elation,
        element: QUANTUM,
    };
    actor.energy_capacity = 0;
    actor.eidolon = eidolon;
    actor.stats.base_speed_micros = 94_000_000;
    actor.stats.speed_micros = 150_000_000;
    let faces = if eidolon >= 2 {
        vec![0.1, 0.6, 0.2]
    } else {
        vec![0.1, 0.3, 0.12]
    };
    actor.skill_parameters = [
        (BASIC.into(), vec![0.25, 1.0]),
        (SKILL.into(), vec![0.1, 0.25, 0.0, 0.0, 1.0, 1.0]),
        (ULTIMATE.into(), vec![8.0, 1.2, 5.0, 0.36, 0.2, 3.0]),
        (
            TALENT.into(),
            vec![1.0, 40.0, 1.0, 0.0, 20.0, 8.0, 1.0, 8.0, 0.2, 3.0, 10.0],
        ),
        (ELATION_SKILL.into(), vec![0.30, 0.20]),
        (FACES.into(), faces),
        (DEBT.into(), vec![0.25, 1.0, 0.1]),
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
fn wishpower_uses_own_cap_overflow_and_sustain_rules() {
    let prepared = plan(&[aha(1, &[PRANK, CARNIVAL, BOX]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.snapshot().units[0].max_energy_milli, 0);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 5);
    assert_eq!(battle.aha_speed_micros(), Some(149_000_000));
    assert_eq!(battle.av_micros, 46_666_667);
    assert_eq!(battle.intrinsic_elation_bp(0), 15_000);
    assert!(
        battle
            .apply(&command(&battle, FORM, Action::Ultimate, None))
            .is_err()
    );
    battle.gain_energy(1, 0, 60_000);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 5);
    battle.heal_unit(0, 0, 100);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 5);
    battle.heal_unit(1, 0, 100);
    battle.heal_unit(1, 0, 100);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 6);

    act(&mut battle, FORM, Action::Skill, Some(YAO));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].aeon_aha_state.emanator, Some(1));
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 8);
    assert_eq!(
        battle.mechanics(0).unwrap().ally_elation_bp(&battle, 0, 1),
        1_000
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0 && hit.action == Action::Skill && hit.assists.is_empty())
            .count(),
        6
    );
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|row| row.kind == EventKind::FollowUp)
            .count(),
        1
    );
    grant_wishpower(&mut battle, 0, 10);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 10);
    let gauge = battle.aha.as_ref().unwrap().gauge_quanta;
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 2);
    assert_eq!(
        battle.aha.as_ref().unwrap().gauge_quanta,
        gauge - crate::battle::resolve::gauge_for(2_000)
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 0
                && hit.action == Action::Ultimate
                && hit.assists.is_empty())
            .count(),
        8
    );
    battle.heal_unit(1, 0, 100);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 3);
}

#[test]
fn natural_aha_rearms_threshold_and_reclaims_consumed_bliss() {
    let prepared = plan(&[aha(2, &[BOX]), yao(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.add_punchline(1, 44);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 6);
    assert_eq!(
        battle.ally_buff_value(0, 1, AllyBuffKind::CritDamage),
        2_000
    );
    battle.add_punchline(1, 1);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 6);
    grant_wishpower(&mut battle, 0, 2);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.elation_ascension().unwrap().innate_level, 2);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .crit_rate_bonus_bp(&battle, 0, 2, None, DamageKind::Elation),
        2_000
    );
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .defense_ignore_bp(&battle, 0, 2),
        6_000
    );

    battle.queue_aha_extra_turn(1, 20);
    battle.resolve_aha_extra_turn();
    assert_eq!(battle.current_punchline(), 49);
    assert_eq!(battle.certified_banger(0), Some(20));
    assert!(battle.units[0].aeon_aha_state.threshold_triggered);
    battle.resolve_natural_aha_turn();
    assert_eq!(battle.current_punchline(), 6);
    assert_eq!(battle.certified_banger(0), Some(69));
    assert!(!battle.units[0].aeon_aha_state.threshold_triggered);
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::AscendedAhaInstant {
            bliss: 49,
            extra: false
        }));
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::PartyTrick {
            before: 20,
            after: 69
        }));
    battle.add_punchline(1, 34);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 3);
    let mechanics = battle.mechanics(0).unwrap();
    for _ in 0..3 {
        mechanics.on_ally_turn_opening(&mut battle, 0, 0, false);
    }
    assert!(battle.elation_ascension().is_none());
    battle.add_punchline(1, 1);
    assert_eq!(
        battle.events().last().unwrap().kind,
        EventKind::Punchline {
            before: 40,
            after: 41
        }
    );
}

#[test]
fn last_participant_splits_non_true_damage_evenly_and_e4_scales_tail() {
    for (eidolon, ratio) in [(0, 20_u64), (4, 25)] {
        let prepared = plan(&[aha(eidolon, &[BOX]), yao(0, &[]), sparxie(0, &[])]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.queue_aha_extra_turn(1, 40);
        battle.resolve_aha_extra_turn();
        let order: Vec<_> = battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::Action(Action::ElationSkill))
            .filter_map(|event| event.source_id.as_deref())
            .collect();
        assert_eq!(order, [YAO, SPARXIE, FORM]);
        let boundary = battle
            .dummy_damage
            .partition_point(|hit| hit.kind != DamageKind::TrueDamage);
        let (damage, truth) = battle.dummy_damage.split_at(boundary);
        let recorded = damage.iter().map(|hit| u64::from(hit.amount)).sum::<u64>();
        assert!(recorded > 0);
        assert_eq!(truth.len(), 3);
        assert!(truth.iter().all(|hit| hit.actor == 0
            && hit.action == Action::ElationSkill
            && hit.kind == DamageKind::TrueDamage));
        assert_eq!(
            truth.iter().map(|hit| u64::from(hit.amount)).sum::<u64>(),
            recorded * ratio / 100
        );
        assert!(truth[0].amount - truth[2].amount <= 1);
        assert_eq!(battle.aha_instant_damage(), 0);
        assert_eq!(
            battle
                .mechanics(0)
                .unwrap()
                .elation_punchline_bonus(&battle, 0),
            if eidolon >= 4 { 80 } else { 0 }
        );
    }
}

#[test]
fn innate_traces_require_active_faces_and_use_aeons_eidolon() {
    for (eidolon, expected_yield, defense_ignore) in [(0, 12_000, 1_500), (2, 30_000, 2_000)] {
        let mut fighter = sparxie(0, &[]);
        fighter.stats.elation_bp = 3_000;
        let prepared = plan(&[aha(eidolon, &[BOX]), yao(0, &[]), fighter]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let sparxie_rules = battle.mechanics(2).unwrap();
        assert_eq!(sparxie_rules.ally_defense_ignore_bp(&battle, 2), 0);
        grant_wishpower(&mut battle, 0, 8);
        act(&mut battle, FORM, Action::Ultimate, None);
        assert_eq!(
            sparxie_rules.ally_defense_ignore_bp(&battle, 2),
            defense_ignore
        );
        let shared = battle.current_punchline();
        battle.fill_energy(1);
        act(&mut battle, YAO, Action::Ultimate, None);
        assert_eq!(battle.units[1].energy_milli, expected_yield + 10_000);
        assert_eq!(battle.current_punchline(), shared + 5 + 10 + 3);
        assert!(battle.dummy_damage.iter().any(|hit| {
            hit.assists
                .iter()
                .any(|&(unit, amount)| unit == 2 && amount > 0)
        }));
    }
}

#[test]
fn elation_debt_caps_finite_health_but_never_creates_dummy_damage() {
    for (eidolon, debt, hp) in [(0, 100, 500), (6, 200, 300)] {
        let mut prepared = plan(&[aha(eidolon, &[])]);
        prepared.units[1].kit = Kit::Enemy(&FINITE);
        prepared.units[1].stats.hp = 1_000;
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.true_damage(0, 1, Action::ElationSkill, 500, Vec::new());
        assert_eq!(battle.units[1].elation_debt, debt);
        assert_eq!(battle.units[1].hp, hp);
        assert_eq!(battle.effective_max_hp(1), 1_000 - debt);
        assert_eq!(battle.units[2].elation_debt, 0);
        assert_eq!(battle.enemy_hp_lost, 1_000 - hp);
        assert!(battle.dummy_damage.is_empty());
    }
}

#[test]
fn e6_first_ultimate_advances_natural_aha_without_spending_another_turn() {
    let prepared = plan(&[aha(6, &[BOX])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let at = battle.av_micros;
    grant_wishpower(&mut battle, 0, 3);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.av_micros, at);
    assert_eq!(battle.current_punchline(), 11);
    assert_eq!(battle.units[0].aeon_aha_state.wishpower, 2);
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::AscendedAhaInstant {
            bliss: 101,
            extra: false
        }));
    grant_wishpower(&mut battle, 0, 6);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.current_punchline(), 11);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::AscendedAhaInstant { .. }))
            .count(),
        1
    );
}

#[test]
fn fixed_dpr_replays_e0_and_e6_and_conserves_true_damage_credit() {
    for eidolon in [0, 6] {
        let prepared = plan(&[
            aha(eidolon, &[PRANK, CARNIVAL, BOX]),
            yao(0, &[]),
            sparxie(2, &[]),
        ]);
        let scenario = STANDARD_DPR_SCENARIO;
        let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
        let result = run();
        assert_eq!(result, run());
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|unit| unit.own_damage + unit.assist_damage)
                .sum::<u64>()
        );
        assert!(
            result
                .targets
                .iter()
                .flat_map(|row| &row.hits)
                .any(|hit| hit.source_id == FORM && hit.kind == DamageKind::TrueDamage)
        );
        assert!(
            result
                .targets
                .iter()
                .flat_map(|row| &row.hits)
                .any(|hit| hit.source_id == FORM && hit.action == Action::Ultimate)
        );
    }
}

#[test]
fn sparxie_straight_fire_uses_only_her_party_trick_and_caps_the_extra_bliss() {
    for (eidolon, expected) in [(0, 10), (2, 14)] {
        let prepared = plan(&[aha(eidolon, &[]), sparxie(0, &[])]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.add_punchline(1, 150);
        battle.resolve_natural_aha_turn();
        assert_eq!(battle.certified_banger(1), Some(152));

        set_wishpower(&mut battle, 0, 8);
        act(&mut battle, FORM, Action::Ultimate, None);
        let before = battle.events().len();
        for _ in 0..3 {
            act(&mut battle, SPARXIE, Action::Skill, None);
        }
        let gifts: Vec<_> = battle.events()[before..]
            .iter()
            .filter_map(|event| match event.kind {
                EventKind::Bliss { before, after } => Some(after - before),
                _ => None,
            })
            .collect();
        assert!(gifts.contains(&expected));
        assert!(gifts.iter().all(|gain| *gain == 1 || *gain == expected));
    }
}

#[test]
fn yao_boon_changes_only_while_faces_of_elation_is_active() {
    for (eidolon, ratio) in [(0, 120_u32), (2, 140)] {
        let prepared = plan(&[aha(eidolon, &[]), yao(0, &[])]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let rules = battle.mechanics(1).unwrap();
        let boon = |battle: &mut Battle<'_>| {
            battle.attack_targets = vec![2];
            rules.on_ally_attack(battle, 1, 1, Action::Basic, Some(2));
            battle.dummy_damage.last().unwrap().amount
        };
        let base = boon(&mut battle);
        set_wishpower(&mut battle, 0, 8);
        act(&mut battle, FORM, Action::Ultimate, None);
        let ascended = boon(&mut battle);
        assert!(ascended.abs_diff(base * ratio / 100) <= 1);
        let faces = battle.mechanics(0).unwrap();
        for _ in 0..3 {
            faces.on_ally_turn_opening(&mut battle, 0, 0, false);
        }
        assert_eq!(boon(&mut battle), base);
    }
}
