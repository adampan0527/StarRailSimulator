use crate::{
    CombatType, Command, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, analysis::DummyDamage, combat_status::DebuffKind, support::AllyBuffKind},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const ARLAN: &str = "character.1008";
const FU_XUAN: &str = "character.1208";
const CENTER: &str = "dpr:2";
const CONCERTO: &str = "summon:character.1309:concerto";

fn robin(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: Element::Physical,
        },
        energy_capacity: 160,
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
            speed_micros: 100_000_000,
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
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![0.5, 3.0]),
            (ULTIMATE.into(), vec![0.228, 90.0, 200.0, 1.2, 1.0, 1.5]),
            (TALENT.into(), vec![0.2, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn arlan(speed_micros: u64, crit_rate_bp: u32) -> OfficialCharacter {
    let mut arlan = robin(0, &[]);
    arlan.unit_id = ARLAN.into();
    arlan.form_id = ARLAN.into();
    arlan.combat_type = CombatType {
        path: CombatPath::Destruction,
        element: Element::Thunder,
    };
    arlan.energy_capacity = 110;
    arlan.stats.speed_micros = speed_micros;
    arlan.stats.base_speed_micros = speed_micros;
    arlan.stats.crit_rate_bp = crit_rate_bp;
    arlan.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    arlan
}

fn herta() -> OfficialCharacter {
    let mut herta = arlan(80_000_000, 0);
    herta.unit_id = "character.1013".into();
    herta.form_id = herta.unit_id.clone();
    herta.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    herta.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![1.0, 0.5, 0.2]),
        ("skill.101303".into(), vec![2.0]),
        ("skill.101304".into(), vec![0.5, 0.4]),
    ]
    .into();
    herta
}

fn fu_xuan() -> OfficialCharacter {
    let mut fu_xuan = arlan(200_000_000, 0);
    fu_xuan.unit_id = FU_XUAN.into();
    fu_xuan.form_id = FU_XUAN.into();
    fu_xuan.combat_type = CombatType {
        path: CombatPath::Preservation,
        element: Element::Quantum,
    };
    fu_xuan.energy_capacity = 135;
    fu_xuan.skill_parameters = [
        ("skill.120801".into(), vec![0.5]),
        ("skill.120802".into(), vec![0.65, 0.0, 3.0, 0.06, 0.12]),
        ("skill.120803".into(), vec![1.0]),
        ("skill.120804".into(), vec![0.18, 0.5, 0.9]),
    ]
    .into();
    fu_xuan.enabled_trace_ids = vec!["trace.1208103".into()];
    fu_xuan
}

fn prepared(units: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(units, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
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
        .unwrap();
    battle.apply(&command).unwrap();
}

fn concerto_hits<'b>(battle: &'b Battle<'_>) -> impl Iterator<Item = &'b DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::Additional)
}

#[test]
fn robin_concerto_replaces_her_turns_until_the_fixed_countdown_completes() {
    let mut plan = prepared(&[robin(2, &[]), arlan(200_000_000, 0)]);
    let mut three = prepared(&[robin(2, &[]), arlan(200_000_000, 0), herta()]);
    three.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&three, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(ARLAN));
    act(&mut battle, FORM, Action::Ultimate, None);
    assert!(battle.is_targetable(0));
    assert_eq!(battle.ordinary_actor(), Some(ARLAN));
    assert_eq!(battle.units[2].gauge_quanta, 0);
    assert_eq!(battle.speed_micros(1), 232_000_000);
    assert!(
        battle
            .action_order()
            .iter()
            .any(|turn| { turn.unit_id == CONCERTO && turn.remaining_av_micros == 111_111_112 })
    );
    assert!(
        !battle
            .action_order()
            .iter()
            .any(|turn| turn.unit_id == FORM)
    );
    battle.set_energy(0, 0, 160_000);
    let invalid = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action: Action::Ultimate,
        target_id: None,
    };
    assert_eq!(battle.apply(&invalid), Err(BattleError::IllegalTiming));
    assert_eq!(battle.units[0].energy_milli, 160_000);

    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.av_micros, 50_000_000);
    for _ in 0..3 {
        act(&mut battle, ARLAN, Action::Basic, Some(CENTER));
    }
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.av_micros, 161_111_112);
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.speed_micros(1), 200_000_000);
    assert_eq!(battle.units[0].energy_milli, 14_000);
    assert!(battle.units[0].summon_turn.is_none());
    assert_eq!(battle.effective_attack(1), 1_000);
}

#[test]
fn robin_entering_concerto_during_her_turn_yields_the_action_and_does_not_duplicate_it() {
    let mut plan = prepared(&[robin(0, &[A2]), arlan(100_000_000, 0)]);
    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.av_micros, 75_000_000);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.ordinary_actor(), Some(ARLAN));
    assert_eq!(battle.av_micros, 75_000_000);
    assert_eq!(battle.units[0].finished_turns, 1);
    assert_eq!(battle.units[0].normal_turns, 1);
}

#[test]
fn robin_skill_duration_uses_her_turns_and_a6_awards_extra_energy() {
    let plan = prepared(&[robin(0, &[A2, A6]), arlan(100_000_000, 0)]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli, 115_000);
    assert_eq!(battle.units[0].robin_state.skill_turns, 3);
    for turns in [2, 1, 0] {
        assert_eq!(battle.ordinary_actor(), Some(ARLAN));
        act(&mut battle, ARLAN, Action::Basic, Some(CENTER));
        assert_eq!(battle.ordinary_actor(), Some(FORM));
        assert_eq!(battle.units[0].robin_state.skill_turns, turns);
        if turns > 0 {
            act(&mut battle, FORM, Action::Basic, Some(CENTER));
        }
    }
    let own_basic = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 0)
        .unwrap();
    assert_eq!(own_basic.amount, 900);
    assert_eq!(battle.units[0].energy_milli, 160_000);
}

#[test]
fn robin_concerto_creates_one_separate_fixed_critical_hit_per_attack() {
    let mut plan = prepared(&[robin(0, &[]), arlan(200_000_000, 0)]);
    plan.units[0].starting_energy_milli = 160_000;
    plan.units[1].starting_energy_milli = 110_000;
    plan.units[0].stats.crit_damage_bp = 24_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    // Applied before the cast, +428 scales both Robin and the attacking ally.
    assert_eq!(battle.effective_attack(0), 1_428);
    assert_eq!(battle.effective_attack(1), 1_428);
    battle.apply_ally_buff(1, 0, AllyBuffKind::CritDamage, 60_000, 3);
    let draws = battle.rng.draws;
    act(&mut battle, ARLAN, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.rng.draws, draws);
    assert_eq!(battle.units[0].energy_milli, 7_000);
    let hits: Vec<_> = concerto_hits(&battle).collect();
    assert_eq!(hits.len(), 1);
    let hit = hits[0];
    // 1428 × 1.2 × 2.5 × 1000 / 1500 × 0.9.
    assert_eq!((hit.amount, hit.critical), (2_570, true));
    assert_eq!(hit.target, 3);
    assert_eq!(hit.action, Action::Ultimate);
    assert!(hit.assists.is_empty());
    assert_eq!(battle.units[3].toughness, 380);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 1)
            .count(),
        3
    );
}

#[test]
fn robin_e1_and_a4_enter_single_hit_attribution_without_affecting_fixed_crit_damage() {
    let mut plan = prepared(&[robin(1, &[A4]), arlan(200_000_000, 10_000)]);
    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    battle.follow_up(1, Element::Thunder, 10_000, 0, Action::Basic, 0);
    battle.resolve_follow_ups();
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 1 && hit.kind == DamageKind::FollowUp)
        .collect();
    assert_eq!(hits.len(), 3);
    for hit in hits {
        // Robin supplies +428 ATK, +45% follow-up CRIT DMG and 24% RES PEN.
        assert_eq!(hit.amount, 2_071);
        assert_eq!(hit.assists, [(0, 1_171)]);
    }
    assert_eq!(concerto_hits(&battle).next().unwrap().amount, 3_187);
}

#[test]
fn robin_e6_uses_eight_charges_and_leaves_the_ninth_hit_at_normal_crit_damage() {
    let mut plan = prepared(&[robin(6, &[]), arlan(200_000_000, 0)]);
    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Ultimate, None);
    for _ in 0..9 {
        battle.follow_up(1, Element::Thunder, 1_000, 0, Action::Basic, 0);
    }
    battle.resolve_follow_ups();
    let amounts: Vec<_> = concerto_hits(&battle).map(|hit| hit.amount).collect();
    assert_eq!(&amounts[..8], &[8_924; 8]);
    assert_eq!(&amounts[8..], &[3_187]);
    assert_eq!(battle.units[0].energy_milli, 32_000);
}

#[test]
fn robin_e4_cleanses_only_control_and_concerto_prevents_incoming_control() {
    let mut plan = prepared(&[robin(4, &[]), arlan(200_000_000, 0)]);
    plan.units[0].starting_energy_milli = 160_000;
    plan.units[2].kit = Kit::Enemy(&EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 400,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    });
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_debuff(2, 1, DebuffKind::AttackDown, 1_000, 3));
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 10_000, Action::Basic));
    assert!(battle.units[1].frozen.is_some());
    act(&mut battle, FORM, Action::Ultimate, None);
    assert!(battle.units[1].frozen.is_none());
    assert!(
        battle.units[1]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::AttackDown)
    );
    assert!(!battle.apply_skill_freeze(2, 0, 10_000, 1, 10_000, Action::Basic));
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .effect_resistance_aura_bp(&battle, 0, 1),
        5_000
    );
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusExpired(StatusKind::Frozen)
            && event.target_id.as_deref() == Some(ARLAN)
    }));
}

#[test]
fn robin_own_immunity_preserves_fu_xuans_one_use_team_guard() {
    let mut plan = prepared(&[fu_xuan(), robin(0, &[])]);
    plan.units[1].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FU_XUAN, Action::Skill, None);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert!(battle.resists_enemy_control(2, 1));
    assert!(battle.resists_enemy_control(2, 0));
    assert!(!battle.resists_enemy_control(2, 0));
    assert!(battle.resists_enemy_control(2, 1));
}

#[test]
fn robin_fixed_dpr_replays_and_attributes_buffs_and_added_damage() {
    let run = || {
        let scenario = STANDARD_DPR_SCENARIO;
        let plan = PreparedBattle::official_dpr(
            &[robin(2, &[A2, A4, A6]), arlan(120_000_000, 10_000)],
            42,
            3,
        )
        .unwrap();
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == FORM
                    && hit.action == Action::Ultimate
                    && hit.kind == DamageKind::Additional
                    && hit.critical
            })
    );
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
}
