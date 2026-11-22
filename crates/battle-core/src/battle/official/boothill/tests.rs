use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const LEFT: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";

fn boothill(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: Element::Physical,
        },
        energy_capacity: ENERGY_CAP,
        level: 80,
        stats: OfficialStats {
            hp: 10_000,
            base_hp: 10_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 107_000_000,
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
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![0.3, 0.15, 2.0]),
            (ULTIMATE.into(), vec![4.0, 0.4, 2.0]),
            (TALENT.into(), vec![0.7, 1.2, 1.7, 0.5, 3.0, 16.0]),
            (ENHANCED_BASIC.into(), vec![2.2]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn mei() -> OfficialCharacter {
    let mut input = boothill(0, &["trace.1303101"]);
    input.unit_id = "character.1303".into();
    input.form_id = input.unit_id.clone();
    input.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Ice,
    };
    input.energy_capacity = 130;
    input.stats.speed_micros = 150_000_000;
    input.skill_parameters = [
        ("skill.130301".into(), vec![1.0]),
        ("skill.130302".into(), vec![0.32, 0.5, 3.0]),
        ("skill.130303".into(), vec![0.25, 2.0, 0.2, 0.1, 0.5]),
        ("skill.130304".into(), vec![0.1, 1.2]),
    ]
    .into();
    input
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn finite_enemy(
    plan: &mut PreparedBattle,
    target: usize,
    hp: u32,
    weakness: bool,
) -> &'static EnemyKit {
    const FINITE_STATS: crate::fixture::Stats = crate::fixture::Stats {
        level: 80,
        attack: 1_000,
        ..DUMMY_STATS
    };
    static VULNERABLE: EnemyKit = EnemyKit {
        stats: FINITE_STATS,
        max_toughness: 120,
        weakness: [true; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    static SMALL: EnemyKit = EnemyKit {
        max_toughness: 30,
        ..VULNERABLE
    };
    static RESISTANT: EnemyKit = EnemyKit {
        weakness: [false; Element::COUNT],
        ..VULNERABLE
    };
    let kit = match (plan.units[target].kit.max_toughness(), weakness) {
        (30, true) => &SMALL,
        (120, true) => &VULNERABLE,
        (120, false) => &RESISTANT,
        _ => panic!("unsupported finite test enemy"),
    };
    plan.units[target].stats = crate::fixture::Stats {
        hp,
        base_hp: hp,
        ..kit.stats
    };
    plan.units[target].kit = Kit::Enemy(kit);
    kit
}

#[test]
fn boothill_enhanced_basic_has_one_legal_target_and_skill_keeps_the_turn() {
    let plan = prepared(&[boothill(0, &[])], &[120; 2]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let av = battle.av_micros;
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusApplied(StatusKind::Standoff))
    );
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 57_500);
    assert_eq!(
        battle.legal_actions(),
        [command(&battle, FORM, Action::Basic, Some(MIDDLE))]
    );
    let rejected = command(&battle, FORM, Action::Basic, Some(LEFT));
    let snapshot = battle.snapshot();
    assert_eq!(battle.apply(&rejected), Err(BattleError::InvalidTarget));
    assert_eq!(battle.snapshot(), snapshot);

    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[2].toughness, 100);
    assert_eq!(battle.units[0].energy_milli, 87_500);
    assert_eq!(battle.skill_points, 2);
    // 1000 × 220% × 2/3 × 0.9 × 1.3.
    assert_eq!(battle.dummy_damage[0].amount, 1_716);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert!(live_standoff(&battle, 0).is_none());
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[1].toughness, 110);
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 600);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn boothill_ultimate_implants_before_its_hit_and_delays_even_outside_standoff() {
    let mut plan = prepared(&[boothill(0, &[])], &[120]);
    finite_enemy(&mut plan, 1, 100_000, false);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let av = battle.av_micros;
    let before = battle.units[1].gauge_quanta;
    battle.set_energy(0, 0, 115_000);
    act(&mut battle, FORM, Action::Ultimate, Some(LEFT));
    assert!(battle.has_weakness(1, Element::Physical));
    assert_eq!(battle.units[1].toughness, 90);
    assert_eq!(battle.units[1].gauge_quanta - before, gauge_for(4_000));
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    let implant = battle.units[1]
        .debuffs
        .iter()
        .find(|debuff| debuff.kind == DebuffKind::ImplantedWeakness(Element::Physical))
        .unwrap();
    assert_eq!(
        implant.remaining_turns(battle.units[1].normal_turns),
        Some(2)
    );
}

#[test]
fn boothill_wins_from_break_and_defeat_in_one_attack_but_refunds_e2_once() {
    let mut plan = prepared(&[boothill(2, &[POINT_BLANK])], &[30]);
    finite_enemy(&mut plan, 1, 2_000, true);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].boothill_state.pockets, 1);
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(battle.units[0].boothill_state.pockets, 3);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 115_000);
    assert_eq!(battle.effective_break_effect(0), 3_000);
    assert!(battle.units[0].boothill_state.standoff.is_none());
}

#[test]
fn boothill_gains_from_teammates_and_overflow_still_refunds_energy_and_skill_points() {
    let plan = prepared(&[boothill(2, &[POINT_BLANK]), mei()], &[30; 2]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].boothill_state.pockets = 3;
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[0].energy_milli, 97_500);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].boothill_state.pockets, 3);

    // Let the slower ally take its pending turn and start the second Standoff.
    act(&mut battle, "character.1303", Action::Basic, Some(MIDDLE));
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    battle.units[0].energy_milli = 0;
    battle.action = Some(Action::Basic);
    battle.attack_hit(1, 3, Element::Ice, 1_000, 30);
    battle.notify_attack(1, Action::Basic, Some(3));
    battle.action = None;
    assert_eq!(battle.units[0].energy_milli, 10_000);
    assert_eq!(battle.units[0].boothill_state.pockets, 3);
    assert!(live_standoff(&battle, 0).is_none());
    assert_eq!(battle.skill_points, 4);
}

#[test]
fn boothill_standoff_taunts_only_the_designated_enemy_and_e4_changes_both_multipliers() {
    for (eidolon, vulnerability) in [(0, 1.15), (4, 1.03)] {
        let plan = &mut prepared(&[mei(), boothill(eidolon, &[ABOVE_SNAKES])], &[120; 2]);
        let dueled = finite_enemy(plan, 2, 100_000, true);
        let other = finite_enemy(plan, 3, 100_000, true);
        let mut battle = Battle::start(plan, LogMode::Detailed);
        act(&mut battle, FORM, Action::Skill, Some(LEFT));
        battle.resolve_enemy(2, dueled);
        assert_eq!(battle.units[0].hp, 10_000);
        let hit = (1_000.0_f64 * (2.0 / 3.0) * vulnerability).floor() as u32;
        assert_eq!(battle.units[1].hp, 10_000 - hit);
        battle.resolve_enemy(3, other);
        assert!(battle.units[0].hp < 10_000);
        battle.attack_hit(3, 1, Element::Physical, 10_000, 0);
        assert_eq!(battle.units[1].hp, 10_000 - hit - 466);
        let mark = battle.units[2]
            .debuffs
            .iter()
            .find(|debuff| debuff.kind == DebuffKind::Standoff)
            .unwrap();
        assert_eq!(mark.bp, if eidolon >= 4 { 4_200 } else { 3_000 });
    }
}

#[test]
fn boothill_new_pocket_applies_to_the_extra_break_and_e6_caps_each_victim() {
    let plan = prepared(&[boothill(6, &[])], &[200, 240, 160]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    battle.units[2].toughness = 30;
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(battle.units[0].boothill_state.pockets, 2);
    assert_eq!(battle.units[1].toughness, 200);
    assert_eq!(battle.units[3].toughness, 160);
    // Physical x2, 16% DEF ignore factor 1000/1420, E4 mark x1.42. True
    // break uses original 240 toughness. The talent caps max toughness at
    // 160, uses the new 120% ratio and E2's +30% Break Effect, and adds
    // separate 40%/70% E6 hits.
    let own = 3767.5533_f64 * 2.0 * 1000.0 / 1420.0;
    let true_break = (own * 260.0 / 40.0 * 1.42).floor() as u32;
    let talent = own * 180.0 / 40.0 * 1.2 * 1.3;
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::WeaknessBreak)
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(
        hits,
        [
            (2, true_break),
            (2, (talent * 1.42).floor() as u32),
            (2, (talent * 1.42 * 0.4).floor() as u32),
            (1, (talent * 0.7).floor() as u32),
            (3, (talent * 0.7).floor() as u32),
        ]
    );
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::WeaknessBroken)
            .count(),
        1
    );
}

#[test]
fn boothill_e1_and_ghost_load_use_caps_and_credit_a_teammates_break_effect() {
    let plan = prepared(&[boothill(1, &[GHOST_LOAD]), mei()], &[120]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.crit_from_break_effect_bp(500), (50, 250));
    assert_eq!(mechanics.crit_from_break_effect_bp(30_000), (3_000, 15_000));
    assert_eq!(mechanics.crit_from_break_effect_bp(50_000), (3_000, 15_000));
    // Fix the roll at a critical hit to isolate the converted 10% CRIT DMG
    // from Ruan Mei's +20% Break Effect and the 16% defense ignore.
    battle.units[0].crit_rate_bonus_bp = 10_000;
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    let hit = &battle.dummy_damage[0];
    assert!(hit.critical);
    assert_eq!(hit.amount, 1_014);
    assert_eq!(hit.assists, [(1, 64)]);
}

#[test]
fn boothill_fixed_dpr_replays_with_break_assists_and_rotates_to_the_larger_target() {
    let plan = prepared(
        &[boothill(0, &[GHOST_LOAD, POINT_BLANK]), mei()],
        &[30, 120, 30],
    );
    let scenario = STANDARD_DPR_SCENARIO;
    let report = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        report,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    let hits = || report.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == FORM
            && hit.kind == DamageKind::WeaknessBreak
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == "character.1303" && credit.amount > 0)
    }));
    for target in &report.targets[..2] {
        assert!(
            target
                .hits
                .iter()
                .any(|hit| hit.source_id == FORM && hit.kind == DamageKind::WeaknessBreak)
        );
    }
    for hit in hits() {
        assert_eq!(
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>(),
            hit.amount
        );
    }
}

#[test]
fn boothill_rejects_inconsistent_locked_skill_parameters() {
    let mut input = boothill(0, &[]);
    input.skill_parameters.get_mut(TALENT).unwrap()[5] = 17.0;
    assert!(build(&input).is_err());
    let mut input = boothill(0, &[]);
    input.skill_parameters.remove(ENHANCED_BASIC);
    assert!(build(&input).is_err());
}
