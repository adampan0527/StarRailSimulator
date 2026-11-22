use crate::{
    Battle, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage, resolve::CounterAttack},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget, GAUGE_FULL},
};

use super::*;

const MARCH: &str = "ally.march";
const HIMEKO: &str = "ally.himeko";
const NATASHA: &str = "ally.natasha";
const CENTER: &str = "dpr:2";
const SIDE: &str = "dpr:1";
const FIRE_WEAK: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: {
        let mut weaknesses = [false; Element::COUNT];
        weaknesses[Element::Fire.index()] = true;
        weaknesses
    },
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn march(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: MARCH.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: Element::Imaginary,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
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
            (BASIC.into(), vec![0.5, 1.0]),
            (SKILL.into(), vec![0.06, 0.1, 1.0]),
            (ULTIMATE.into(), vec![1.44, 2.0, 0.2]),
            (TALENT.into(), vec![7.0, 0.4, 10.0]),
            (ENHANCED.into(), vec![0.4, 0.6, 3.0, 3.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn himeko() -> OfficialCharacter {
    let mut ally = march(0, &[]);
    ally.unit_id = HIMEKO.into();
    ally.form_id = "character.1003".into();
    ally.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Fire,
    };
    ally.energy_capacity = 120;
    ally.stats.base_speed_micros = 150_000_000;
    ally.stats.speed_micros = 150_000_000;
    ally.skill_parameters = [
        ("skill.100301".into(), vec![0.5]),
        ("skill.100302".into(), vec![1.0, 0.4]),
        ("skill.100303".into(), vec![1.38, 5.0]),
        ("skill.100304".into(), vec![0.7, 3.0]),
    ]
    .into();
    ally
}

fn natasha() -> OfficialCharacter {
    let mut ally = himeko();
    ally.unit_id = NATASHA.into();
    ally.form_id = "natasha.abundance".into();
    ally.combat_type = CombatType {
        path: CombatPath::Abundance,
        element: Element::Physical,
    };
    ally.energy_capacity = 90;
    ally.stats.base_speed_micros = 140_000_000;
    ally.stats.speed_micros = 140_000_000;
    ally.skill_parameters = [
        ("skill.110501".into(), vec![0.5]),
        ("skill.110502".into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
        ("skill.110503".into(), vec![0.092, 92.0]),
        ("skill.110504".into(), vec![0.30, 0.25]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap_or_else(|| panic!("expected {actor} {action:?} -> {target:?}"));
    battle.apply(&command).unwrap();
    command
}

fn ready(battle: &mut Battle<'_>, actor: usize) {
    battle.ordinary_actor = Some(actor);
}

fn state<'b>(battle: &'b Battle<'_>) -> &'b MarchHuntState {
    &battle.units[0].march_hunt_state
}

fn march_hits(battle: &Battle<'_>) -> Vec<(DamageKind, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0)
        .map(|hit| (hit.kind, hit.amount))
        .collect()
}

#[test]
fn march_hunt_shifu_rejects_self_and_transfers_speed_to_the_new_target() {
    let plan = plan(&[march(1, &[SWAN_SOAR]), himeko(), natasha()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(MARCH));
    assert_eq!(battle.av_micros, 37_500_000);
    let before = battle.snapshot();
    let invalid = Command {
        decision_index: before.decision_index,
        actor_id: MARCH.into(),
        action: Action::Skill,
        target_id: Some(MARCH.into()),
    };
    assert!(!battle.legal_actions().contains(&invalid));
    assert_eq!(battle.apply(&invalid), Err(BattleError::InvalidTarget));
    assert_eq!(battle.snapshot(), before);

    act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.speed_micros(0), 220_000_000);
    assert_eq!(battle.speed_micros(1), 159_000_000);
    assert_eq!(battle.speed_micros(2), 140_000_000);

    ready(&mut battle, 0);
    act(&mut battle, MARCH, Action::Skill, Some(NATASHA));
    assert_eq!(state(&battle).shifu, Some(2));
    assert_eq!(battle.speed_micros(1), 150_000_000);
    assert_eq!(battle.speed_micros(2), 148_400_000);
    battle.units[2].departed = true;
    assert_eq!(battle.speed_micros(0), 200_000_000);
    ready(&mut battle, 0);
    assert!(!battle.legal_actions().iter().any(|command| {
        command.action == Action::Skill && command.target_id.as_deref() == Some(NATASHA)
    }));
}

#[test]
fn march_hunt_paths_choose_same_element_extra_damage_or_double_toughness() {
    for (shifu, mentor, toughness, hits) in [
        (
            himeko(),
            HIMEKO,
            990,
            vec![(DamageKind::Direct, 450), (DamageKind::Additional, 72)],
        ),
        (natasha(), NATASHA, 980, vec![(DamageKind::Direct, 450)]),
    ] {
        let mut march = march(0, &[]);
        march.stats.element_damage_bp[Element::Imaginary.index()] = 5_000;
        march.stats.element_damage_bp[Element::Fire.index()] = 2_000;
        let plan = plan(&[march, shifu]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, MARCH, Action::Skill, Some(mentor));
        ready(&mut battle, 0);
        act(&mut battle, MARCH, Action::Basic, Some(CENTER));
        // 500 x 1.50 and 100 x 1.20, each times 0.9 toughness and 2/3 defense.
        assert_eq!(march_hits(&battle), hits);
        assert_eq!(battle.units[3].toughness, toughness);
        assert_eq!(battle.units[0].energy_milli, 105_000);
        assert_eq!(battle.skill_points, 3);
        assert_eq!(state(&battle).charge, 1);
    }
}

#[test]
fn march_hunt_shifu_attacks_earn_once_per_action_and_e2_advances_at_seven() {
    let plan = plan(&[march(2, &[]), himeko()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
    assert_eq!(battle.ordinary_actor(), Some(HIMEKO));
    let av = battle.av_micros;
    battle.units[0].march_hunt_state.charge = 5;
    act(&mut battle, HIMEKO, Action::Skill, Some(CENTER));
    assert_eq!(state(&battle).charge, 7);
    assert_eq!(battle.ordinary_actor(), Some(MARCH));
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(
        march_hits(&battle),
        [
            (DamageKind::FollowUp, 144),
            (DamageKind::FollowUp, 216),
            (DamageKind::Additional, 60)
        ]
    );
    assert_eq!(battle.units[3].toughness, 975);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::Skill)
    );

    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
    // Keep March off the turn queue long enough for two native Shifu turns.
    battle.units[0].gauge_quanta = 4 * GAUGE_FULL;
    let march_turn = battle.units[0].normal_turns;
    for _ in 0..2 {
        act(&mut battle, HIMEKO, Action::Basic, Some(CENTER));
    }
    assert_eq!(battle.units[0].normal_turns, march_turn);
    assert_eq!(state(&battle).charge, 3);
    assert_eq!(march_hits(&battle).len(), 3);
}

#[test]
fn march_hunt_ultimate_and_follow_up_charge_independently_and_support_skill_does_not() {
    for (shifu, mentor, target) in [(himeko(), HIMEKO, None), (natasha(), NATASHA, None)] {
        let plan = plan(&[march(0, &[]), shifu]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, MARCH, Action::Skill, Some(mentor));
        if mentor == NATASHA {
            act(&mut battle, NATASHA, Action::Skill, Some(MARCH));
            assert_eq!(state(&battle).charge, 0);
        }
        battle.units[1].energy_milli = plan.units[1].kit.max_energy_milli();
        act(&mut battle, mentor, Action::Ultimate, target);
        assert_eq!(state(&battle).charge, 1);
        battle.counter_attack(
            1,
            3,
            CounterAttack {
                element: Element::Fire,
                multiplier_bp: 5_000,
                defense_multiplier_bp: 0,
                toughness: 0,
                splash: Some((5_000, 0)),
                energy_milli: 0,
            },
        );
        assert_eq!(state(&battle).charge, 2);
    }
}

#[test]
fn march_hunt_enhanced_hits_roll_sequentially_and_ultimate_boost_is_consumed() {
    let plan = plan(&[march(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].march_hunt_state.charge = 6;
    let av = battle.av_micros;
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    assert_eq!(state(&battle).charge, 7);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(MARCH));

    battle.units[0].energy_milli = 0;
    battle.units[0].march_hunt_state.charge = 10;
    battle.dummy_damage.clear();
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    // Seed 42 starts at 7415/10000, above the unempowered 60% chance.
    assert_eq!(march_hits(&battle), vec![(DamageKind::Direct, 336); 3]);
    assert_eq!(battle.units[2].toughness, 975);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 30_000);
    assert_eq!(state(&battle).charge, 3);
    assert_eq!(battle.snapshot().random_draws, 1);

    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].march_hunt_state.charge = 7;
    battle.units[0].energy_milli = 110_000;
    act(&mut battle, MARCH, Action::Ultimate, Some(CENTER));
    assert_eq!(march_hits(&battle), [(DamageKind::Direct, 1_209)]);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert!(state(&battle).ultimate_armed);
    battle.dummy_damage.clear();
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    // The first three rolls (7415, 1599, 2786) all pass 80%, for 5+3 hits.
    assert_eq!(march_hits(&battle), vec![(DamageKind::Direct, 336); 8]);
    assert_eq!(battle.snapshot().random_draws, 3);
    assert_eq!(battle.units[2].toughness, 930);
    assert_eq!(battle.units[0].energy_milli, 35_000);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(state(&battle).charge, 0);
    assert!(!state(&battle).ultimate_armed);
}

#[test]
fn march_hunt_e4_energy_and_e6_critical_damage_are_timed_to_their_actions() {
    let mut march = march(6, &[]);
    march.stats.crit_rate_bp = 10_000;
    let plan = plan(&[march]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 60_000);
    battle.units[0].energy_milli = 110_000;
    act(&mut battle, MARCH, Action::Ultimate, Some(CENTER));
    assert_eq!(march_hits(&battle), [(DamageKind::Direct, 1_296)]);
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    assert_eq!(march_hits(&battle)[1], (DamageKind::Direct, 450));
    assert_eq!(battle.units[0].energy_milli, 30_000); // 5 from Ultimate, 20 from Basic, 5 on next turn.
    assert!(state(&battle).ultimate_armed);

    battle.units[0].march_hunt_state.charge = 7;
    battle.dummy_damage.clear();
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    let hits = march_hits(&battle);
    assert!((5..=8).contains(&hits.len()));
    assert!(hits.iter().all(|hit| *hit == (DamageKind::Direct, 672)));
    assert!(!state(&battle).ultimate_armed);
}

#[test]
fn march_hunt_filigree_breaks_matched_shifu_weakness_as_imaginary() {
    for (traces, remaining) in [([].as_slice(), 10), ([FILIGREE].as_slice(), 0)] {
        let mut plan = plan(&[march(0, traces), himeko()]);
        plan.units[3].kit = Kit::Enemy(&FIRE_WEAK);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[3].toughness = 10;
        act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
        ready(&mut battle, 0);
        act(&mut battle, MARCH, Action::Basic, Some(CENTER));
        assert_eq!(battle.units[3].toughness, remaining);
        assert_eq!(battle.is_slowed(3), remaining == 0);
        let break_damage = battle.events().iter().find_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::WeaknessBreak,
                amount,
                ..
            } => Some(amount),
            _ => None,
        });
        // 3767.5533 x 0.5 Imaginary x (20 + 30)/40 x 2/3 defense.
        assert_eq!(break_damage, (remaining == 0).then_some(1_569));
    }
}

#[test]
fn march_hunt_tide_tamer_credits_break_and_critical_damage_for_two_shifu_turns() {
    let mut shifu = himeko();
    shifu.stats.crit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[march(0, &[TIDE_TAMER]), shifu], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
    ready(&mut battle, 0);
    battle.units[0].march_hunt_state.charge = 7;
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    assert_eq!(battle.support_break_effect(1), 3_600);
    battle.units[2].toughness = 10;
    ready(&mut battle, 1);
    act(&mut battle, HIMEKO, Action::Basic, Some(SIDE));
    let side_hits: Vec<&DummyDamage> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1 && hit.target == 2)
        .collect();
    assert_eq!(side_hits.len(), 2);
    assert_eq!(side_hits[0].amount, 630); // 500 x 2.10 x 0.9 x 2/3.
    assert_eq!(side_hits[0].assists, [(0, 180)]);
    assert_eq!(side_hits[1].kind, DamageKind::WeaknessBreak);
    assert_eq!(side_hits[1].amount, 8_539); // 3767.5533 x 2 x 50/40 x 2/3 x 1.36.
    assert_eq!(side_hits[1].assists, [(0, 2_260)]);
    assert_eq!(battle.support_break_effect(1), 3_600);
    // Finish the remaining Shifu turn through the usual scheduler.
    while battle.ordinary_actor() != Some(HIMEKO) {
        let actor = battle.ordinary_actor().unwrap().to_owned();
        act(&mut battle, &actor, Action::Basic, Some(CENTER));
    }
    act(&mut battle, HIMEKO, Action::Basic, Some(CENTER));
    assert_eq!(battle.support_break_effect(1), 0);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::CritDamage), 0);
}

#[test]
fn march_hunt_fixed_dpr_and_detailed_replay_are_reproducible() {
    let solo = PreparedBattle::official_dpr(&[march(0, &[])], 42, 3).unwrap();
    let result = calculate_dpr(&solo, &[30, 120, 30], std::num::NonZeroU32::MIN).unwrap();
    assert_eq!(result.total_damage, 600); // Two 50% ATK hits on the unbroken middle dummy.
    assert_eq!(result.dpr, 600.0);

    let plan = PreparedBattle::official_dpr(
        &[
            march(6, &[SWAN_SOAR, FILIGREE, TIDE_TAMER]),
            natasha(),
            himeko(),
        ],
        42,
        3,
    )
    .unwrap();
    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == MARCH
                    && matches!(hit.action, Action::Basic | Action::Skill)
                    && hit.kind == DamageKind::FollowUp
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
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let commands = [
        act(&mut battle, MARCH, Action::Skill, Some(HIMEKO)),
        act(&mut battle, HIMEKO, Action::Basic, Some(CENTER)),
    ];
    let replay = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
    assert_eq!(battle.events(), replay.events());
    assert_eq!(battle.snapshot(), replay.snapshot());
}

#[test]
fn march_hunt_e2_retargets_after_shifu_kills_the_primary_enemy() {
    let mut plan = plan(&[march(2, &[]), himeko()]);
    plan.units[3].kit = Kit::Enemy(&FIRE_WEAK);
    plan.units[3].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, MARCH, Action::Skill, Some(HIMEKO));
    act(&mut battle, HIMEKO, Action::Basic, Some(CENTER));

    // Two live dummies remain, and the first seed-42 roll selects the second of them.
    let follow_up = battle
        .events()
        .iter()
        .find(|event| {
            event.source_id.as_deref() == Some(MARCH) && event.kind == EventKind::FollowUp
        })
        .unwrap();
    assert_eq!(follow_up.target_id.as_deref(), Some("dpr:3"));
    assert_eq!(battle.units[3].hp, 0);
    assert_eq!(battle.units[4].toughness, 995);
    assert_eq!(state(&battle).charge, 2);
    assert_eq!(battle.snapshot().random_draws, 1);
}

#[test]
fn march_hunt_enhanced_attack_stops_on_defeat_without_a_continuation_roll() {
    let mut plan = plan(&[march(0, &[])]);
    plan.units[2].kit = Kit::Enemy(&FIRE_WEAK);
    plan.units[2].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].march_hunt_state.charge = 7;
    act(&mut battle, MARCH, Action::Basic, Some(CENTER));
    assert_eq!(state(&battle).charge, 0);
    assert_eq!(battle.snapshot().random_draws, 0);
    assert_eq!(battle.units[0].energy_milli, 95_000); // 55 initial + 10 defeat + 30 attack.
    assert_eq!(battle.units[1].toughness, 1_000);
    assert_eq!(battle.units[3].toughness, 1_000);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    EventKind::Damage {
                        kind: DamageKind::Direct,
                        ..
                    }
                )
            })
            .count(),
        1
    );
}
