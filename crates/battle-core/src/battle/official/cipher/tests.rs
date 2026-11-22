use super::*;

use crate::{
    CombatType, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, setup::UnitPlan},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const TRIBBIE: &str = "character.1403";
const ARGENTI: &str = "character.1302";
const TRAILBLAZER: &str = "trailblazer.remembrance";

static FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn cipher(eidolon: u8, speed: u64, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: QUANTUM,
        },
        energy_capacity: 130,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 106_000_000,
            speed_micros: speed,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![if eidolon >= 3 { 1.1 } else { 1.0 }]),
            (
                SKILL.into(),
                vec![
                    if eidolon >= 5 { 2.2 } else { 2.0 },
                    if eidolon >= 5 { 1.1 } else { 1.0 },
                    0.1,
                    2.0,
                    0.3,
                    1.2,
                ],
            ),
            (
                ULTIMATE.into(),
                if eidolon >= 3 {
                    vec![1.32, 0.25, 0.75, 0.44]
                } else {
                    vec![1.2, 0.25, 0.75, 0.4]
                },
            ),
            (
                TALENT.into(),
                vec![if eidolon >= 5 { 1.65 } else { 1.5 }, 0.12, 1.0, 5.0],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn teammate(id: &str, path: CombatPath, element: Element, capacity: u32) -> OfficialCharacter {
    let mut input = cipher(0, 106_000_000, &[]);
    input.unit_id = id.into();
    input.form_id = id.into();
    input.combat_type = CombatType { path, element };
    input.energy_capacity = capacity;
    input
}

fn argenti() -> OfficialCharacter {
    let mut input = teammate(ARGENTI, CombatPath::Erudition, Element::Physical, 180);
    input.stats.speed_micros = 250_000_000;
    input.stats.attack = 1_300;
    input.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    input
}

fn tribbie() -> OfficialCharacter {
    let mut input = teammate(TRIBBIE, CombatPath::Harmony, QUANTUM, 120);
    input.eidolon = 1;
    input.skill_parameters = [
        ("skill.140301".into(), vec![0.3, 0.15]),
        ("skill.140302".into(), vec![0.24, 3.0]),
        ("skill.140303".into(), vec![0.3, 0.3, 0.12, 2.0]),
        ("skill.140304".into(), vec![0.18]),
    ]
    .into();
    input
}

fn trailblazer() -> OfficialCharacter {
    let mut input = teammate(TRAILBLAZER, CombatPath::Remembrance, Element::Ice, 160);
    input.skill_parameters = [
        ("skill.800701".into(), vec![1.0]),
        ("skill.800702".into(), vec![0.6, 0.1]),
        ("skill.800703".into(), vec![2.4, 0.4]),
        ("skill.800704".into(), vec![130.0, 0.8, 10.0, 640.0]),
        ("skill.800708".into(), vec![1.2, 1.2, 0.1]),
        ("skill.1800701".into(), vec![0.36, 4.0, 0.9]),
        ("skill.1800703".into(), vec![0.12, 0.24]),
        ("skill.1800705".into(), vec![0.5]),
        ("skill.1800706".into(), vec![0.25]),
        ("skill.1800707".into(), vec![0.28, 3.0, 1.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn finite_plan(team: &[OfficialCharacter], waves: &[&[u32]]) -> PreparedBattle {
    let mut prepared = PreparedBattle::official_dpr(team, 42, 3).unwrap();
    prepared.units.truncate(team.len());
    prepared.wave_ends.clear();
    for (wave, enemies) in waves.iter().enumerate() {
        for (position, &hp) in enemies.iter().enumerate() {
            prepared.units.push(UnitPlan {
                id: format!("enemy.{wave}.{position}"),
                combat_type: None,
                has_control_resistance: false,
                elation_bp: 0,
                kit: Kit::Enemy(&FINITE),
                stats: crate::fixture::Stats {
                    hp,
                    base_hp: hp,
                    base_speed_micros: 90_000_000,
                    ..DUMMY_STATS
                },
                starting_energy_milli: 0,
            });
        }
        prepared.wave_ends.push(prepared.units.len());
    }
    prepared
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn state(battle: &Battle<'_>) -> CipherState {
    battle.units[0].cipher_state
}

#[test]
fn registers_base_and_rejects_invalid_frozen_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = cipher(0, 106_000_000, &[]);
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[2] = 0.5;
    assert!(build(&input).is_err());
}

#[test]
fn skill_selects_one_patron_and_weakens_the_blast_targets_before_hitting() {
    let prepared = plan(&[cipher(0, 106_000_000, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(state(&battle).patron, Some(1));
    act(&mut battle, FORM, Action::Skill, Some(CENTER));

    assert_eq!(state(&battle).patron, Some(2));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 95_000);
    assert_eq!(state(&battle).tally_in_basis_points, 1_872_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(2, 1_560), (1, 780), (3, 780)]
    );
    for (slot, toughness) in [(1, 390), (2, 380), (3, 390)] {
        assert_eq!(battle.units[slot].toughness, toughness);
        assert!(
            battle.units[slot]
                .debuffs
                .iter()
                .any(|debuff| { debuff.kind == DebuffKind::Weaken && debuff.bp == 1_000 })
        );
        assert_eq!(
            battle.units[slot]
                .debuffs
                .iter()
                .any(|debuff| debuff.kind == DebuffKind::Patron),
            slot == 2
        );
    }
}

#[test]
fn real_hp_caps_the_tally_and_patron_switches_across_defeats_and_waves() {
    let prepared = finite_plan(&[cipher(0, 106_000_000, &[])], &[&[400, 300], &[700]]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(state(&battle).patron, Some(1));
    act(&mut battle, FORM, Action::Basic, Some("enemy.0.0"));
    assert_eq!(state(&battle).patron, Some(2));
    assert_eq!(state(&battle).tally_in_basis_points, 480_000);
    act(&mut battle, FORM, Action::Basic, Some("enemy.0.1"));
    assert_eq!(state(&battle).patron, Some(3));
    assert_eq!(state(&battle).tally_in_basis_points, 840_000);
    assert_eq!(battle.wave_index, 1);
}

#[test]
fn weaken_scales_finite_enemy_damage_and_expires_after_two_enemy_turns() {
    let prepared = finite_plan(&[cipher(0, 106_000_000, &[])], &[&[20_000]]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some("enemy.0.0"));
    for _ in 0..2 {
        act(&mut battle, FORM, Action::Basic, Some("enemy.0.0"));
    }
    let damage = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage { amount, .. } if event.source_id.as_deref() == Some("enemy.0.0") => {
                Some(amount)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(damage, [91, 91, 101]);
}

#[test]
fn follow_up_uses_the_patron_and_resets_only_at_ciphers_next_turn() {
    let prepared = plan(&[cipher(0, 106_000_000, &[]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, ARGENTI, Action::Basic, Some(LEFT));
    assert!(state(&battle).follow_up_used);
    assert_eq!(battle.units[0].energy_milli, 70_000);
    act(&mut battle, ARGENTI, Action::Basic, Some(LEFT));
    // The second Argenti action reaches the start of Cipher's next turn.
    assert!(!state(&battle).follow_up_used);
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert!(!state(&battle).follow_up_used);
    act(&mut battle, ARGENTI, Action::Basic, Some(LEFT));
    let follow_ups: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| (hit.actor, hit.target, hit.amount))
        .collect();
    assert_eq!(follow_ups, [(0, 2, 900); 2]);
    assert_eq!(battle.units[2].toughness, 320);
}

#[test]
fn ultimate_records_its_opening_hit_then_releases_one_tally_without_recursion() {
    let prepared = plan(&[cipher(0, 106_000_000, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[0].cipher_state.tally_in_basis_points = 12_000_000;
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some(CENTER));

    assert_eq!(state(&battle).tally_in_basis_points, 0);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.target, hit.kind, hit.amount, hit.critical))
            .collect::<Vec<_>>(),
        [
            (2, DamageKind::Direct, 720, false),
            (2, DamageKind::TrueDamage, 321, false),
            (2, DamageKind::Direct, 240, false),
            (1, DamageKind::Direct, 240, false),
            (3, DamageKind::Direct, 240, false),
            (2, DamageKind::TrueDamage, 322, false),
            (1, DamageKind::TrueDamage, 322, false),
            (3, DamageKind::TrueDamage, 321, false),
        ]
    );
    assert_eq!(battle.units[1].toughness, 380);
    assert_eq!(battle.units[2].toughness, 370);
    assert_eq!(battle.units[3].toughness, 380);
}

#[test]
fn ultimate_can_pay_the_blast_to_neighbors_after_the_main_target_dies() {
    let prepared = finite_plan(&[cipher(0, 106_000_000, &[])], &[&[1_200, 100, 10_000]]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[0].cipher_state.tally_in_basis_points = 12_000_000;
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some("enemy.0.1"));
    let true_damage = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::TrueDamage,
                amount,
                ..
            } => Some((event.target_id.as_deref().unwrap(), amount)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(true_damage, [("enemy.0.0", 455), ("enemy.0.2", 454)]);
    assert_eq!(state(&battle).patron, Some(3));
    assert_eq!(state(&battle).tally_in_basis_points, 0);
}

#[test]
fn speed_thresholds_and_eidolons_apply_to_the_record_and_follow_up() {
    for (speed, crit_bp, tally) in [
        (139_999_999, 0, 180_000),
        (140_000_000, 2_500, 270_000),
        (169_999_999, 2_500, 270_000),
        (170_000_000, 5_000, 360_000),
    ] {
        let prepared = plan(&[cipher(1, speed, &[EMPYREAN_STRIDES])]);
        let mut battle = Battle::start(&prepared, LogMode::Off);
        let rules = battle.mechanics(0).unwrap();
        assert_eq!(
            rules.crit_rate_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
            crit_bp
        );
        rules.on_ally_damage(&mut battle, 0, 0, 1, DamageKind::Direct, 100);
        assert_eq!(state(&battle).tally_in_basis_points, tally);
    }

    let prepared = plan(&[cipher(6, 106_000_000, &[]), argenti()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, ARGENTI, Action::Basic, Some(LEFT));
    assert_eq!(battle.effective_attack(0), 1_800);
    let follow_up = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::FollowUp)
        .unwrap();
    assert_eq!((follow_up.actor, follow_up.amount), (0, 8_019));
    assert!(
        battle.units[2]
            .debuffs
            .iter()
            .any(|debuff| { debuff.kind == DebuffKind::Vulnerability && debuff.bp == 3_000 })
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .count(),
        2
    );
    let tally: u64 = battle
        .dummy_damage
        .iter()
        .map(|hit| {
            u64::from(hit.amount)
                * if hit.kind == DamageKind::FollowUp {
                    3_400
                } else {
                    1_800
                }
        })
        .sum();
    assert_eq!(state(&battle).tally_in_basis_points, tally);
    battle.dummy_damage.clear();
    battle.fill_energy(0);
    act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
    let direct = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct && hit.target == 2)
        .map(|hit| u64::from(hit.amount))
        .sum::<u64>();
    let additional = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .map(|hit| u64::from(hit.amount))
        .sum::<u64>();
    assert_eq!(
        state(&battle).tally_in_basis_points,
        (tally + direct * 1_800) / 5 + additional * 1_800
    );
}

#[test]
fn tribbie_and_mem_true_damage_never_enters_the_tally() {
    let prepared = plan(&[
        cipher(0, 106_000_000, &[THREE_HUNDRED_ROGUES]),
        tribbie(),
        trailblazer(),
        argenti(),
    ]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let expected_tally = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind != DamageKind::TrueDamage)
            .map(|hit| u64::from(hit.amount) * if hit.target == 4 { 1_200 } else { 800 })
            .sum::<u64>()
    };
    battle.fill_energy(1);
    act(&mut battle, TRIBBIE, Action::Ultimate, None);
    for _ in 0..2 {
        battle.fill_energy(2);
        act(&mut battle, TRAILBLAZER, Action::Ultimate, None);
    }
    let mem = battle.active_memosprite(2).unwrap();
    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| hit.actor == mem && hit.kind == DamageKind::Direct)
    );
    assert_eq!(
        state(&battle).tally_in_basis_points,
        expected_tally(&battle)
    );

    let mem_rules = battle.mechanics(2).unwrap();
    mem_rules.on_summon_turn(&mut battle, 2);
    battle.units[0].cipher_state.tally_in_basis_points = 0;
    battle.dummy_damage.clear();
    battle.fill_energy(3);
    act(&mut battle, ARGENTI, Action::EnhancedUltimate, None);

    assert!(
        battle
            .dummy_damage
            .iter()
            .any(|hit| { hit.kind == DamageKind::TrueDamage && hit.actor == 1 })
    );
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.kind == DamageKind::TrueDamage
            && hit.actor == 3
            && hit.assists.iter().any(|(owner, _)| *owner == 2)
    }));
    assert_eq!(
        state(&battle).tally_in_basis_points,
        expected_tally(&battle)
    );
}

#[test]
fn dpr_replay_attributes_vulnerability_and_tally_without_double_credit() {
    let prepared = PreparedBattle::official_dpr(
        &[
            cipher(
                2,
                170_000_000,
                &[EMPYREAN_STRIDES, THREE_HUNDRED_ROGUES, SLEIGHT_OF_SKY],
            ),
            argenti(),
        ],
        42,
        3,
    )
    .unwrap();
    let run = || {
        calculate_dpr(
            &prepared,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    let cipher = &result.characters[0];
    assert!(cipher.assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == FORM
                    && hit.kind == DamageKind::TrueDamage
                    && hit.credits.len() == 1
                    && hit.credits[0].amount == hit.amount
            })
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(
        result.total_damage,
        result
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>()
    );
}
