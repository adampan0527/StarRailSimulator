use super::{ARCHERION, BASIC, BOWMASTER, FORM, MAJESTAS, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind, calculate_dpr, fixture::GAUGE_FULL,
};

const YUKONG: &str = "ally.yukong";
const ARLAN: &str = "ally.arlan";
const MIDDLE: &str = "dpr:2";

fn yukong(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: YUKONG.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Harmony,
            element: crate::Element::Imaginary,
        },
        energy_capacity: 130,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![2.0, 0.8]),
            (ULTIMATE.into(), vec![3.8, 0.28, 0.65]),
            (TALENT.into(), vec![0.8, 1.0, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = yukong(0, &[]);
    ally.unit_id = ARLAN.into();
    ally.form_id = "character.1008".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    ally.energy_capacity = 110;
    ally.stats.base_speed_micros = 150_000_000;
    ally.stats.speed_micros = 150_000_000;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
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
        .unwrap();
    battle.apply(&command).unwrap();
}

#[test]
fn skill_preserves_both_stacks_and_majestas_pays_for_manual_actions_before_turn_expiry() {
    let plan = PreparedBattle::official_dpr(&[yukong(0, &[MAJESTAS]), arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    act(&mut battle, YUKONG, Action::Skill, None);
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.effective_attack(0), 1_800);
    assert_eq!(battle.effective_attack(1), 1_800);
    assert_eq!(battle.units[0].yukong_state.bowstrings, 2);
    assert_eq!(battle.units[0].energy_milli, 97_000);

    battle.fill_energy(1);
    act(&mut battle, ARLAN, Action::Ultimate, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 2);
    assert_eq!(battle.units[0].energy_milli, 99_000);

    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 1);
    assert_eq!(battle.units[0].energy_milli, 101_000);

    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 0);
    assert_eq!(battle.units[0].energy_milli, 123_000);
    assert_eq!(battle.effective_attack(1), 1_000);
}

#[test]
fn talent_doubles_toughness_and_adds_damage_with_a_one_turn_gap() {
    let plan = PreparedBattle::official_dpr(&[yukong(0, &[])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    for (remaining, total_hits) in [(100, 2), (90, 3), (70, 5)] {
        act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
        assert_eq!(battle.units[2].toughness, remaining);
        assert_eq!(battle.dummy_damage.len(), total_hits);
    }
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, 600),
            (DamageKind::Additional, 480),
            (DamageKind::Direct, 600),
            (DamageKind::Direct, 600),
            (DamageKind::Additional, 480),
        ]
    );
    assert_eq!(battle.units[0].energy_milli, 125_000);
    assert_eq!(battle.snapshot().random_draws, 0);
}

#[test]
fn critical_rate_attribution_uses_the_same_roll_and_expires_with_the_last_stack() {
    let mut damage_dealer = arlan();
    damage_dealer.stats.crit_rate_bp = 500;
    let plan = PreparedBattle::official_dpr(&[yukong(0, &[]), damage_dealer], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);

    act(&mut battle, YUKONG, Action::Skill, None);
    battle.fill_energy(0);
    act(&mut battle, YUKONG, Action::Ultimate, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 2);
    assert_eq!(battle.effective_crit_damage_bp(1), 11_500);
    assert_eq!(battle.dummy_damage[0].amount, 4_104);
    assert!(!battle.dummy_damage[0].critical); // Seed 42: 7415 against 2800.

    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    let hit = &battle.dummy_damage[1];
    assert!(hit.critical); // The second roll, 1599, is above base 500 and below buffed 3300.
    assert_eq!(hit.amount, 1_161);
    assert_eq!(hit.assists, [(0, 861)]); // 1161 - 300 (500 x 0.6 without Yukong).
    assert_eq!(battle.snapshot().random_draws, 2);

    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 0);
    assert_eq!(battle.effective_crit_damage_bp(1), 5_000);
}

#[test]
fn e6_grants_one_stack_before_ultimate_damage_and_combines_e4_and_bowmaster() {
    for (eidolon, stacks, critical, amount) in [(0, 0, false, 2_553), (6, 1, true, 12_529)] {
        let plan = PreparedBattle::official_dpr(&[yukong(eidolon, &[BOWMASTER])], 3, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Off);
        battle.fill_energy(0);
        act(&mut battle, YUKONG, Action::Ultimate, Some(MIDDLE));

        assert_eq!(battle.units[0].yukong_state.bowstrings, stacks);
        let hit = &battle.dummy_damage[0];
        // E0: 1000 x 3.8 x 1.12 x 0.6. E6: 1800 x 3.8 x 1.42 x 2.15 x 0.6.
        assert_eq!((hit.amount, hit.critical), (amount, critical));
        assert!(hit.assists.is_empty());
    }

    let plan = PreparedBattle::official_dpr(&[yukong(6, &[]), arlan()], 3, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, YUKONG, Action::Skill, None);
    battle.fill_energy(0);
    act(&mut battle, YUKONG, Action::Ultimate, Some(MIDDLE));
    assert_eq!(battle.units[0].yukong_state.bowstrings, 2);
}

#[test]
fn e1_speed_expires_by_recipient_and_e2_refunds_each_energy_cap_once_per_ultimate() {
    let mut owner = yukong(2, &[]);
    owner.stats.energy_regeneration_bp = 12_500;
    let plan = PreparedBattle::official_dpr(&[owner, arlan()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.speed_micros(0), 220_000_000);
    assert_eq!(battle.speed_micros(1), 165_000_000);

    battle.fill_energy(1);
    assert_eq!(battle.units[0].energy_milli, 71_250);
    battle.set_energy(1, 1, 100_000);
    battle.fill_energy(1);
    assert_eq!(battle.units[0].energy_milli, 71_250);

    battle.fill_energy(0);
    act(&mut battle, YUKONG, Action::Ultimate, Some(MIDDLE));
    assert_eq!(battle.units[0].energy_milli, 6_250);
    battle.set_energy(1, 1, 100_000);
    battle.fill_energy(1);
    assert_eq!(battle.units[0].energy_milli, 12_500);

    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert_eq!(battle.speed_micros(1), 165_000_000);
    act(&mut battle, ARLAN, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.speed_micros(1), 150_000_000);
}

#[test]
fn archerion_covers_non_control_and_freeze_after_two_completed_turns() {
    let plan = PreparedBattle::official_dpr(&[yukong(0, &[ARCHERION])], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(!battle.apply_skill_freeze(2, 0, 0, 1, 2_500, Action::Skill));

    battle.apply_speed_down(2, 0, 1_000, 1);
    assert!(!battle.is_slowed(0));
    battle.apply_speed_down(2, 0, 1_000, 1);
    assert!(battle.is_slowed(0));
    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));

    battle.apply_speed_down(2, 0, 1_000, 1);
    assert!(battle.is_slowed(0));
    act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));

    assert!(!battle.apply_skill_freeze(2, 0, 10_000, 1, 2_500, Action::Skill));
    assert!(!battle.is_frozen(0));
    assert!(battle.apply_skill_freeze(2, 0, 10_000, 1, 2_500, Action::Skill));
    assert!(battle.is_frozen(0));
}

#[test]
fn imaginary_break_delays_and_slows_without_followup_damage() {
    let mut owner = yukong(0, &[BOWMASTER]);
    owner.stats.break_effect_bp = 5_000;
    let plan = PreparedBattle::official_dpr(&[owner], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().av_micros, 50_000_000);
    battle.fill_energy(0);
    act(&mut battle, YUKONG, Action::Ultimate, Some("dpr:1"));

    assert_eq!(battle.units[1].toughness, 0);
    assert_eq!(battle.units[1].gauge_quanta, GAUGE_FULL * 12 / 10);
    assert_eq!(battle.speed_micros(1), 90_000_000);
    assert_eq!(battle.dummy_damage[1].kind, DamageKind::WeaknessBreak);
    assert_eq!(battle.dummy_damage[1].amount, 2_354);

    for _ in 0..3 {
        act(&mut battle, YUKONG, Action::Basic, Some(MIDDLE));
    }
    let next = battle.events().iter().find(|event| {
        event.source_id.as_deref() == Some("dpr:1") && event.kind == EventKind::TurnStarted
    });
    assert_eq!(next.unwrap().av_micros, 183_333_334);
    assert_eq!(battle.units[1].toughness, 30);
    assert_eq!(battle.speed_micros(1), 100_000_000);
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusExpired(StatusKind::Slow)
            && event.target_id.as_deref() == Some("dpr:1")
    }));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == 1)
            .count(),
        2
    );
}

#[test]
fn fixed_dpr_replays_bowstring_assists_and_conserves_every_hits_damage() {
    let plan =
        PreparedBattle::official_dpr(&[yukong(6, &[BOWMASTER, MAJESTAS]), arlan()], 42, 3).unwrap();
    let simulate = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = simulate();
    assert_eq!(result, simulate());
    assert!(result.characters[0].assist_damage > 0);
    assert!(result.targets[1].hits.iter().any(|hit| {
        hit.source_id == YUKONG && hit.action == Action::Ultimate && hit.kind == DamageKind::Direct
    }));
    for hit in result.targets.iter().flat_map(|target| &target.hits) {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
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
