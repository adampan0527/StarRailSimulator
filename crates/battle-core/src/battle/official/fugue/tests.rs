use super::*;
use crate::{
    CombatType, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const FUGUE: &str = "ally.fugue";
const DANCER: &str = "ally.dancer";

fn fugue(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FUGUE.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: FIRE,
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
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            base_aggro: 100,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 10_000,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![3.0, 0.3, 1.0, 0.18, 2.0, 0.5]),
            (ULTIMATE.into(), vec![2.0]),
            (TALENT.into(), vec![1.0, 0.4]),
            (ENHANCED_BASIC.into(), vec![1.0, 0.5]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn trailblazer() -> OfficialCharacter {
    let mut input = fugue(0, &[]);
    input.unit_id = DANCER.into();
    input.form_id = "trailblazer.harmony".into();
    input.combat_type = CombatType {
        element: Element::Imaginary,
        path: CombatPath::Harmony,
    };
    input.energy_capacity = 140;
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.800501".into(), vec![1.0]),
        ("skill.800502".into(), vec![0.5, 2.0]),
        ("skill.800503".into(), vec![3.0, 1.0, 0.3]),
        ("skill.800504".into(), vec![10.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 17, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    battle
        .apply(&Command {
            decision_index: battle.snapshot().decision_index,
            actor_id: actor.into(),
            action,
            target_id: target.map(str::to_owned),
        })
        .unwrap();
}

fn hit_as(battle: &mut Battle<'_>, actor: usize, target: usize, element: Element, toughness: u16) {
    battle.begin_attack();
    battle.action = Some(Action::Basic);
    battle.attack_hit(actor, target, element, 10_000, toughness);
    battle.notify_attack(actor, Action::Basic, Some(target));
    battle.action = None;
}

fn extra(battle: &Battle<'_>, target: usize) -> u16 {
    battle.units[target].extra_toughness.unwrap().remaining
}

fn breaks(battle: &Battle<'_>) -> usize {
    battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::WeaknessBroken)
        .count()
}

#[test]
fn fugue_accepts_the_locked_base_variant_and_rejects_invalid_talent_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = fugue(0, &[]);
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(TALENT).unwrap()[1] = 0.5;
    assert!(build(&input).is_err());
    input.skill_parameters.get_mut(TALENT).unwrap()[1] = 0.4;
    input.skill_parameters.remove(ENHANCED_BASIC);
    assert!(build(&input).is_err());
}

#[test]
fn fugue_skill_refunds_once_enhances_basic_and_only_blesses_the_latest_target() {
    let mut ally = trailblazer();
    ally.stats.speed_micros = 60_000_000;
    ally.stats.base_speed_micros = 60_000_000;
    let prepared = plan(&[fugue(0, &[A4]), ally], &[60, 120, 60]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FUGUE, Action::Skill, Some(DANCER));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.effective_break_effect(1), 3_000);
    assert_eq!(battle.effective_break_effect(0), 3_000);
    act(&mut battle, FUGUE, Action::Basic, Some("dpr:2"));
    let attacked: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(attacked, [(3, 600), (2, 300), (4, 300)]);
    assert_eq!(battle.units[3].toughness, 110);
    assert_eq!(battle.units[2].toughness, 55);
    assert_eq!(battle.skill_points, 4);
    act(&mut battle, FUGUE, Action::Skill, Some(FUGUE));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.effective_break_effect(1), 0);
    assert_eq!(battle.effective_break_effect(0), 6_000);
    let expires = battle.units[0].fugue_state.prayer.unwrap().expires_at_turn;
    battle.units[0].normal_turns = expires;
    assert_eq!(battle.effective_break_effect(0), 3_000);
    assert!(
        battle
            .mechanics(0)
            .unwrap()
            .ability_override(&battle, 0, Action::Basic)
            .is_none()
    );
}

#[test]
fn fugue_prayer_off_element_efficiency_and_defense_down_use_actual_attacking_unit() {
    static ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 60,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[fugue(1, &[]), trailblazer()], &[60]);
    prepared.units[2].kit = Kit::Enemy(&ENEMY_KIT);
    prepared.units[2].stats.hp = 100_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FUGUE, Action::Skill, Some(DANCER));
    hit_as(&mut battle, 0, 2, FIRE, 10);
    assert_eq!(battle.units[2].toughness, 60);
    hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
    assert_eq!(battle.units[2].toughness, 53); // Floor of 10 × 150% × 50%.
    let debuffs: Vec<_> = battle.units[2]
        .debuffs
        .iter()
        .filter(|d| d.kind == DebuffKind::DefenseDown)
        .collect();
    assert_eq!(debuffs.len(), 1);
    assert_eq!((debuffs[0].source, debuffs[0].bp), (0, 1_800));
}

#[test]
fn fugue_extra_toughness_consumes_overflow_and_triggers_a_second_independent_break() {
    let prepared = plan(&[fugue(2, &[A2])], &[30]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(extra(&battle, 1), 12);
    battle.set_energy(0, 0, 0);
    hit_as(&mut battle, 0, 1, FIRE, 50);
    assert_eq!(battle.units[1].toughness, 0);
    assert_eq!(extra(&battle, 1), 0);
    assert_eq!(breaks(&battle), 2);
    assert_eq!(battle.units[0].energy_milli, 6_000);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::GaugeShifted {
            before_quanta, after_quanta,
        } if after_quanta.saturating_sub(before_quanta) == gauge_for(1_500)))
            .count(),
        2
    );
    let amounts: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind.is_break())
        .map(|hit| (hit.kind, hit.amount))
        .collect();
    // Each initial break uses the original toughness cap 30 and Fire's multiplier 2.
    assert_eq!(amounts, [(DamageKind::WeaknessBreak, 6_279); 2]);
}

#[test]
fn fugue_extra_toughness_accepts_any_element_until_exhausted_and_restores_with_recovery() {
    static ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 30,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[fugue(0, &[]), trailblazer()], &[30]);
    prepared.units[2].kit = Kit::Enemy(&ENEMY_KIT);
    prepared.units[2].stats.hp = 100_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_toughness(0, 2, 0);
    hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
    assert_eq!(extra(&battle, 2), 2);
    hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
    assert_eq!(extra(&battle, 2), 0);
    assert_eq!(breaks(&battle), 1);
    let super_breaks = |battle: &Battle<'_>| {
        battle
            .events()
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EventKind::Damage {
                        kind: DamageKind::SuperBreak,
                        ..
                    }
                )
            })
            .count()
    };
    assert_eq!(super_breaks(&battle), 2);
    hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
    assert_eq!(super_breaks(&battle), 2);
    battle.set_toughness(2, 2, 30);
    assert_eq!(extra(&battle, 2), 12);

    battle.units[0].hp = 0;
    battle.set_toughness(2, 2, 0);
    hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
    assert_eq!(extra(&battle, 2), 12);
    assert_eq!(super_breaks(&battle), 2);
}

#[test]
fn fugue_break_trace_samples_220_percent_and_stacks_two_ally_bonuses() {
    for (personal, expected) in [(18_999, 1_200), (19_000, 3_600)] {
        let mut lead = fugue(2, &[A4, A6]);
        lead.stats.break_effect_bp = personal;
        let prepared = plan(&[lead, trailblazer()], &[30]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        hit_as(&mut battle, 1, 2, Element::Imaginary, 50);
        assert_eq!(breaks(&battle), 2);
        assert_eq!(battle.effective_break_effect(0), personal + 3_000);
        assert_eq!(battle.effective_break_effect(1), expected);
        battle.set_toughness(2, 2, 30);
        hit_as(&mut battle, 1, 2, Element::Imaginary, 50);
        assert_eq!(battle.effective_break_effect(1), expected);
    }
}

#[test]
fn fugue_e6_shares_prayer_and_ultimate_keeps_full_toughness_with_e2_advance() {
    static ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 60,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[fugue(6, &[]), trailblazer()], &[60]);
    prepared.units[2].kit = Kit::Enemy(&ENEMY_KIT);
    prepared.units[2].stats.hp = 100_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FUGUE, Action::Skill, Some(DANCER));
    assert_eq!(battle.effective_break_effect(0), 3_000);
    assert_eq!(battle.effective_break_effect(1), 3_000);
    battle.set_energy(0, 0, 130_000);
    battle.shift_gauge(0, 1, gauge_for(5_000), false);
    act(&mut battle, FUGUE, Action::Ultimate, None);
    assert_eq!(battle.units[2].toughness, 20); // 20 × (100% + E1 50% + E6 50%).
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(FUGUE)
            && event.target_id.as_deref() == Some(DANCER)
            && matches!(event.kind, EventKind::GaugeShifted {before_quanta, after_quanta}
                if before_quanta.saturating_sub(after_quanta) == gauge_for(2_400))
    }));
}

#[test]
fn fugue_e4_multiplies_break_damage_and_preserves_the_ordinary_hit() {
    let run = |eidolon| {
        let prepared = plan(&[fugue(eidolon, &[]), trailblazer()], &[60]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        act(&mut battle, FUGUE, Action::Skill, Some(DANCER));
        battle.set_toughness(0, 2, 0);
        hit_as(&mut battle, 1, 2, Element::Imaginary, 10);
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>()
    };
    let original = run(0);
    let enhanced = run(4);
    assert_eq!(original[0], enhanced[0]);
    // Both have 30% BE and 18% DEF down. At E4, E1 already increases toughness by 50%.
    assert_eq!(original[1], (DamageKind::SuperBreak, 3_473));
    assert_eq!(enhanced[1], (DamageKind::SuperBreak, 6_252));
}

#[test]
fn fugue_stacks_with_backup_dancer_without_combining_their_damage_or_credit() {
    let prepared = plan(&[fugue(0, &[]), trailblazer()], &[60]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(1, 1, 140_000);
    act(&mut battle, DANCER, Action::Ultimate, None);
    battle.set_toughness(0, 2, 0);
    hit_as(&mut battle, 0, 2, FIRE, 10);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::SuperBreak)
        .collect();
    assert_eq!(hits.len(), 2);
    assert_eq!((hits[0].amount, hits[1].amount), (3_265, 3_265));
    assert_eq!(hits[0].assists, [(1, 754)]); // 30% Break Effect from Backup Dancer.
    assert_eq!(hits[1].assists, [(1, 3_265)]); // Trailblazer provided the second trigger too.
}

#[test]
fn fugue_fixed_dpr_replays_super_break_and_balances_assists() {
    let prepared =
        PreparedBattle::official_dpr(&[fugue(4, &[A2, A4, A6]), trailblazer()], 17, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap()
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.kind == DamageKind::SuperBreak
                    && hit.credits.iter().any(|credit| credit.unit_id == FUGUE)
            })
    );
    assert_eq!(
        result
            .characters
            .iter()
            .map(|row| row.own_damage + row.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
}
