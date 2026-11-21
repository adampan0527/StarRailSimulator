use crate::{
    Battle, CombatType, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
    rng::Rng,
    supports_official_dpr_variant,
};

use super::*;

const CENTER: &str = "dpr:2";
const SPARKLE: &str = "character.1306";
const FREE_SKILL_TRACE: &str = "trace.11306102";

static FINITE_TARGET: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn qingque(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: QUANTUM,
        },
        energy_capacity: 140,
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
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![2.0, 0.28, 4.0]),
            (ULTIMATE.into(), vec![2.0]),
            (TALENT.into(), vec![0.72]),
            (ENHANCED_BASIC.into(), vec![2.4, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn sparkle(speed: u64) -> OfficialCharacter {
    let mut input = qingque(0, &[FREE_SKILL_TRACE, "trace.11306103"]);
    input.unit_id = SPARKLE.into();
    input.form_id = SPARKLE.into();
    input.combat_type.path = CombatPath::Harmony;
    input.energy_capacity = 110;
    input.stats.speed_micros = speed * 1_000_000;
    input.stats.base_speed_micros = input.stats.speed_micros;
    input.skill_parameters = [
        ("skill.1130601".into(), vec![1.0]),
        ("skill.1130602".into(), vec![0.24, 0.45, 2.0, 0.5]),
        ("skill.1130603".into(), vec![2.0, 6.0, 0.06, 3.0, 10.0]),
        ("skill.1130604".into(), vec![2.0, 0.04, 2.0, 3.0]),
    ]
    .into();
    input
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

fn direct_damage(battle: &Battle<'_>, actor: usize, action: Action) -> Vec<u32> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == actor && hit.kind == DamageKind::Direct && hit.action == action)
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn qingque_repeated_skills_keep_one_turn_and_cap_the_damage_bonus() {
    let plan =
        PreparedBattle::official_dpr(&[qingque(4, &[TILE_BATTLE, BIDE_TIME])], 6, 5).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let start = battle.snapshot();
    assert_eq!(start.units[0].energy_milli, 71_000);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([1, 1, 1]);
    battle.rng = Rng::new(6);

    let expected = [[0, 2, 2], [0, 3, 1], [0, 3, 1], [0, 3, 1], [0, 3, 1]];
    for (index, suits) in expected.into_iter().enumerate() {
        let cast = u8::try_from(index + 1).unwrap();
        act(&mut battle, FORM, Action::Skill, None);
        let snapshot = battle.snapshot();
        assert_eq!(battle.ordinary_actor(), Some(FORM));
        assert_eq!(snapshot.av_micros, start.av_micros);
        assert_eq!(snapshot.units[0].normal_turns, 1);
        assert_eq!(battle.units[0].finished_turns, 0);
        assert_eq!(snapshot.units[0].gauge_quanta, 0);
        assert_eq!(snapshot.skill_points, 6 - cast);
        assert_eq!(
            snapshot.units[0].energy_milli,
            71_000 + u32::from(cast) * 1_000
        );
        assert_eq!(snapshot.random_draws, u32::from(cast) * 3);
        assert_eq!(battle.units[0].qingque_state.hand, JadeHand::Tiles(suits));
        assert_eq!(battle.units[0].qingque_state.damage_stacks, cast.min(4));
    }
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    // 1000 ATK × 100% basic × (1 + 4×.38) × 2/3 DEF × .9 toughness.
    assert_eq!(direct_damage(&battle, 0, Action::Basic), [1_512]);
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(battle.units[0].finished_turns, 1);
    assert_eq!(battle.units[0].qingque_state.damage_stacks, 0);
    assert!(!battle.units[0].qingque_state.autarky);
    assert!(
        battle
            .events()
            .iter()
            .all(|event| event.kind != EventKind::ExtraTurnStarted)
    );
}

#[test]
fn qingque_one_matching_draw_ends_the_draw_sequence_and_autarky_copies_the_blast() {
    let plan =
        PreparedBattle::official_dpr(&[qingque(6, &[BIDE_TIME, WINNING_HAND])], 20, 2).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([3, 1, 0]);
    battle.rng = Rng::new(20);
    act(&mut battle, FORM, Action::Skill, None);
    let prepared = battle.snapshot();
    assert_eq!(
        prepared.random_draws, 2,
        "one matching tile, then the E4 roll"
    );
    assert_eq!(prepared.units[0].energy_milli, 72_000);
    assert_eq!(prepared.skill_points, 1);
    assert_eq!(battle.units[0].qingque_state.hand, JadeHand::Hidden);
    assert!(battle.units[0].qingque_state.autarky);
    let mut invalid = battle.legal_actions()[0].clone();
    invalid.action = Action::Skill;
    invalid.target_id = None;
    assert_eq!(battle.apply(&invalid), Err(BattleError::IllegalTiming));
    assert_eq!(battle.snapshot(), prepared);

    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.kind, hit.amount))
        .collect();
    // The E4 follow-up retains 38% skill damage and loses the 72% Hidden Hand ATK.
    assert_eq!(
        hits,
        [
            (2, DamageKind::Direct, 3_417),
            (1, DamageKind::Direct, 1_424),
            (3, DamageKind::Direct, 1_424),
            (2, DamageKind::FollowUp, 1_987),
            (1, DamageKind::FollowUp, 828),
            (3, DamageKind::FollowUp, 828),
        ]
    );
    assert_eq!(
        battle.units[1..]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [10, 80, 10]
    );
    assert_eq!(battle.snapshot().skill_points, 2);
    assert_eq!(
        battle.units[0].energy_milli, 93_000,
        "20 from the basic and 1 from the next turn's draw"
    );
    assert_eq!(battle.snapshot().units[0].speed_micros, 220_000_000);
    assert_eq!(battle.snapshot().av_micros, 95_454_546);

    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.snapshot().units[0].speed_micros, 200_000_000);
}

#[test]
fn qingque_basic_discards_the_least_common_suit_and_autarky_has_no_extra_energy() {
    let plan = PreparedBattle::official_dpr(&[qingque(4, &[])], 0, 0).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([2, 1, 1]);
    battle.units[0].qingque_state.autarky = true;
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|command| command.action != Action::Skill)
    );
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert!(battle.events().iter().any(|event| event.kind
        == EventKind::JadeTiles {
            suits: [2, 0, 1],
            hidden_hand: false,
        }));
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        [(DamageKind::Direct, 600), (DamageKind::FollowUp, 600)]
    );
    assert_eq!(battle.units[2].toughness, 100);
    assert_eq!(battle.units[0].energy_milli, 92_000);
    assert_eq!(battle.snapshot().skill_points, 1);
}

#[test]
fn qingque_ultimate_outside_her_turn_stores_tiles_for_the_next_turn() {
    let plan = PreparedBattle::official_dpr(&[sparkle(250), qingque(2, &[])], 0, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let start = battle.snapshot();
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    assert_eq!(start.units[1].energy_milli, 71_000);
    assert_eq!(start.random_draws, 1);
    assert!(battle.events().iter().any(|event| {
        matches!(event.kind, EventKind::JadeTiles { .. })
            && event.source_id.as_deref() == Some(SPARKLE)
            && event.target_id.as_deref() == Some(FORM)
    }));

    battle.units[1].energy_milli = 140_000;
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    assert_eq!(
        battle.effective_attack(1),
        1_450,
        "only Sparkle's 45% ATK trace"
    );
    assert_eq!(
        battle.units[1].qingque_state.hand,
        JadeHand::Tiles([4, 0, 0])
    );
    assert_eq!(battle.units[1].energy_milli, 5_000);
    assert_eq!(battle.snapshot().random_draws, 1);
    assert_eq!(direct_damage(&battle, 1, Action::Ultimate), [1_914; 3]);

    act(&mut battle, SPARKLE, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.units[1].qingque_state.hand, JadeHand::Hidden);
    assert_eq!(battle.effective_attack(1), 2_170);
    assert_eq!(battle.units[1].energy_milli, 5_000);
    assert_eq!(battle.snapshot().random_draws, 1);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(
        battle.snapshot().skill_points,
        4,
        "E2 enhanced basic does not refund SP"
    );
}

#[test]
fn qingque_ultimate_in_her_turn_uses_skill_stacks_then_completes_the_current_hand() {
    let plan = PreparedBattle::official_dpr(&[qingque(1, &[BIDE_TIME])], 0, 2).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([2, 1, 1]);
    battle.rng = Rng::new(0);
    battle.units[0].energy_milli = 140_000;
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(
        battle.units[0].qingque_state.hand,
        JadeHand::Tiles([2, 0, 2])
    );
    act(&mut battle, FORM, Action::Ultimate, None);
    // The hit has one 38% skill stack and 10% E1 damage, then gains Hidden Hand.
    assert_eq!(direct_damage(&battle, 0, Action::Ultimate), [1_776; 3]);
    assert_eq!(battle.units[0].qingque_state.hand, JadeHand::Hidden);
    assert_eq!(battle.units[0].qingque_state.damage_stacks, 1);
    assert_eq!(battle.snapshot().av_micros, 50_000_000);
    assert_eq!(battle.units[0].energy_milli, 5_000);

    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(
        direct_damage(&battle, 0, Action::Basic),
        [3_417, 1_424, 1_424]
    );
    assert_eq!(battle.snapshot().skill_points, 1);
    assert!(matches!(
        battle.units[0].qingque_state.hand,
        JadeHand::Tiles([0..=1, 0..=1, 0..=1])
    ));
}

#[test]
fn qingque_ultimate_cannot_overwrite_hidden_hand_during_an_interruption() {
    let plan = PreparedBattle::official_dpr(&[qingque(0, &[]), sparkle(150)], 0, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    battle.units[0].qingque_state.hand = JadeHand::Hidden;
    battle.units[0].energy_milli = 140_000;
    battle.queue_extra_turn(1);
    assert!(battle.start_extra_turn());

    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].qingque_state.hand, JadeHand::Hidden);
    act(&mut battle, SPARKLE, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.units[0].qingque_state.hand, JadeHand::Hidden);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(
        direct_damage(&battle, 0, Action::Basic),
        [3_124, 1_302, 1_302]
    );
}

#[test]
fn qingque_autarky_does_not_retarget_after_a_lethal_basic_and_a2_stays_consumed_across_waves() {
    let mut plan = PreparedBattle::official_dpr(&[qingque(6, &[TILE_BATTLE])], 20, 2).unwrap();
    plan.units[1].kit = Kit::Enemy(&FINITE_TARGET);
    plan.units[1].stats.hp = 100;
    plan.wave_ends = vec![2, 4];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([3, 1, 0]);
    battle.rng = Rng::new(20);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.snapshot().skill_points, 2);
    act(&mut battle, FORM, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.snapshot().wave_index, 1);
    assert_eq!(battle.snapshot().skill_points, 3);
    assert!(
        battle
            .events()
            .iter()
            .all(|event| event.kind != EventKind::FollowUp)
    );
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(
        battle.snapshot().skill_points,
        2,
        "A2 was spent in the first wave"
    );
}

#[test]
fn qingque_skill_spends_feed_sparkles_single_turn_and_dpr_replays() {
    let plan = PreparedBattle::official_dpr(
        &[
            qingque(6, &[TILE_BATTLE, BIDE_TIME, WINNING_HAND]),
            sparkle(150),
        ],
        6,
        7,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.snapshot().skill_point_cap, 7);
    battle.units[0].qingque_state.hand = JadeHand::Tiles([1, 1, 1]);
    battle.rng = Rng::new(6);
    let sparkle_rules = battle.mechanics(1).unwrap();
    for _ in 0..2 {
        act(&mut battle, FORM, Action::Skill, None);
        assert!(
            sparkle_rules
                .ability_override(&battle, 1, Action::Skill)
                .is_none()
        );
    }
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.snapshot().skill_points, 5);
    assert!(
        sparkle_rules
            .ability_override(&battle, 1, Action::Skill)
            .is_some()
    );
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    let before = battle.snapshot().skill_points;
    act(&mut battle, SPARKLE, Action::Skill, Some(FORM));
    assert_eq!(battle.snapshot().skill_points, before);

    let run = || {
        calculate_dpr(
            &plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(run(), result);
    assert!(result.characters[0].own_damage > 0);
    assert!(result.characters[1].assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == FORM
                    && hit.action == Action::Basic
                    && hit.kind == DamageKind::FollowUp
            })
    );
    assert_eq!(
        result
            .characters
            .iter()
            .map(|role| role.own_damage + role.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
}

#[test]
fn qingque_repeated_skills_do_not_end_an_already_active_extra_turn() {
    let plan = PreparedBattle::official_dpr(&[sparkle(250), qingque(4, &[])], 6, 7).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    battle.units[1].qingque_state.hand = JadeHand::Tiles([1, 1, 1]);
    battle.rng = Rng::new(6);
    battle.queue_extra_turn(1);
    assert!(battle.start_extra_turn());

    for _ in 0..3 {
        act(&mut battle, FORM, Action::Skill, None);
        assert!(battle.in_extra_turn(1));
    }
    assert_eq!(battle.units[1].normal_turns, 0);
    assert!(
        battle
            .mechanics(0)
            .unwrap()
            .ability_override(&battle, 0, Action::Skill)
            .is_some()
    );
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert!(!battle.in_extra_turn(1));
    assert_eq!(battle.ordinary_actor(), Some(SPARKLE));
    assert_eq!(battle.snapshot().av_micros, 40_000_000);
}

#[test]
fn qingque_rejects_malformed_skill_parameters_and_unregistered_variants() {
    let mut input = qingque(0, &[]);
    input.skill_parameters.get_mut(SKILL).unwrap()[2] = 5.0;
    assert!(matches!(
        PreparedBattle::official_dpr(&[input], 0, 3),
        Err(BattleError::InvalidInput(_))
    ));
    assert!(supports_official_dpr_variant(FORM, "base"));
    assert!(!supports_official_dpr_variant(FORM, "novaflare"));
}
