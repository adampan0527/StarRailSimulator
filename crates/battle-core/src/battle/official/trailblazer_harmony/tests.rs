use super::*;
use crate::{
    CombatType, Command, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const TRAILBLAZER: &str = "ally.harmony-trailblazer";
const MEI: &str = "ally.ruan-mei";
const ENEMY: &str = "dpr:1";

fn trailblazer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: TRAILBLAZER.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
            element: IMAGINARY,
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
            base_aggro: 100,
            base_speed_micros: 100_000_000,
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
            (BASIC.into(), vec![if eidolon >= 5 { 1.1 } else { 1.0 }]),
            (
                SKILL.into(),
                vec![if eidolon >= 3 { 0.55 } else { 0.5 }, 2.0],
            ),
            (
                ULTIMATE.into(),
                vec![3.0, 1.0, if eidolon >= 5 { 0.33 } else { 0.3 }],
            ),
            (TALENT.into(), vec![if eidolon >= 3 { 11.0 } else { 10.0 }]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn ruan_mei() -> OfficialCharacter {
    let mut input = trailblazer(0, &["trace.1303101"]);
    input.unit_id = MEI.into();
    input.form_id = "character.1303".into();
    input.combat_type.element = Element::Ice;
    input.energy_capacity = 130;
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.130301".into(), vec![1.0]),
        ("skill.130302".into(), vec![0.32, 0.5, 3.0]),
        ("skill.130303".into(), vec![0.25, 2.0, 0.2, 0.1, 0.5]),
        ("skill.130304".into(), vec![0.1, 1.2]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    battle
        .apply(&Command {
            decision_index: battle.snapshot().decision_index,
            actor_id: TRAILBLAZER.into(),
            action,
            target_id: target.map(String::from),
        })
        .unwrap();
}

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.set_energy(0, 0, 140_000);
    act(battle, Action::Ultimate, None);
}

#[test]
fn harmony_trailblazer_accepts_the_canonical_form_and_rejects_inconsistent_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = trailblazer(0, &[]);
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[1] = 2.0;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
    input.skill_parameters.get_mut(ULTIMATE).unwrap()[1] = 1.0;
    input.energy_capacity = 130;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
}

#[test]
fn harmony_trailblazer_basic_and_e6_skill_use_locked_damage_toughness_and_energy() {
    let prepared = plan(&[trailblazer(6, &[SHUFFLE])], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(0, 0, 0);
    act(&mut battle, Action::Basic, Some(ENEMY));
    assert_eq!(battle.dummy_damage[0].amount, 660);
    assert_eq!(battle.units[1].toughness, 110);
    assert_eq!(battle.units[0].energy_milli, 25_000);
    assert_eq!(battle.skill_points, 4);

    act(&mut battle, Action::Skill, Some(ENEMY));
    assert_eq!(battle.units[1].toughness, 60);
    assert_eq!(battle.units[0].energy_milli, 77_500);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.action == Action::Skill)
            .count(),
        7
    );
    assert!(battle.dummy_damage[1..].iter().all(|hit| hit.amount == 330));

    act(&mut battle, Action::Skill, Some(ENEMY));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 130_000);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .energy_regeneration_bonus_bp(&battle, 0),
        0
    );
}

#[test]
fn harmony_trailblazer_converts_only_the_segments_after_the_first_actual_break() {
    let prepared = plan(&[trailblazer(0, &[SHUFFLE, DANCE, HAT])], &[20]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    act(&mut battle, Action::Skill, Some(ENEMY));
    assert_eq!(battle.units[0].energy_milli, 45_000);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|e| e.kind == EventKind::WeaknessBroken)
            .count(),
        1
    );
    assert!(battle.events().iter().any(|e| {
        e.source_id.as_deref() == Some(TRAILBLAZER)
            && e.target_id.as_deref() == Some(ENEMY)
            && matches!(e.kind, EventKind::GaugeShifted { before_quanta, after_quanta }
                if after_quanta - before_quanta == gauge_for(3_000))
    }));
    let break_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind.is_break())
        .collect();
    assert_eq!(break_hits.len(), 2);
    assert_eq!(break_hits[0].kind, DamageKind::WeaknessBreak);
    assert_eq!(break_hits[1].kind, DamageKind::SuperBreak);
    // Four eligible touches of 5, 30% BE, one target's 60%, and 1000/1500 DEF.
    assert_eq!(break_hits[1].amount, 10_448);
    assert!(!break_hits[1].critical);
}

#[test]
fn harmony_trailblazer_keeps_each_targets_segments_separate_and_uses_the_live_enemy_count() {
    let prepared = plan(&[trailblazer(0, &[DANCE])], &[120, 120, 120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    for target in [1, 2] {
        battle.set_toughness(0, target, 0);
    }
    battle.begin_attack();
    battle.action = Some(Action::Skill);
    for (target, toughness) in [(1, 10), (2, 5), (1, 5), (3, 5)] {
        battle.attack_hit(0, target, IMAGINARY, 5_000, toughness);
    }
    battle.notify_attack(0, Action::Skill, Some(1));
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::SuperBreak)
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(hits, [(1, 6_856), (2, 2_285)]);
    assert_eq!(battle.units[3].toughness, 115);
}

#[test]
fn harmony_trailblazer_does_not_convert_an_unmatched_element_or_attack_a_dead_target() {
    let mut prepared = plan(&[trailblazer(0, &[])], &[120]);
    const ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    prepared.units[1].kit = Kit::Enemy(&ENEMY_KIT);
    prepared.units[1].stats.hp = 20_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    battle.set_toughness(0, 1, 0);
    act(&mut battle, Action::Skill, Some(ENEMY));
    assert!(!battle.events().iter().any(|e| matches!(
        e.kind,
        EventKind::Damage {
            kind: DamageKind::SuperBreak,
            ..
        }
    )));
    assert_eq!(battle.units[0].energy_milli, 35_000);

    battle.units[1].hp = 1;
    act(&mut battle, Action::Skill, Some(ENEMY));
    assert_eq!(battle.units[1].hp, 0);
    // Only the first hit lands, so six attack energy and ten kill energy.
    assert_eq!(battle.units[0].energy_milli, 51_000);
}

#[test]
fn harmony_trailblazer_super_break_can_defeat_a_finite_enemy_without_retriggering_break() {
    const ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [true; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[trailblazer(0, &[])], &[120]);
    prepared.units[1].kit = Kit::Enemy(&ENEMY_KIT);
    prepared.units[1].stats.hp = 3_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    battle.set_toughness(0, 1, 0);
    act(&mut battle, Action::Basic, Some(ENEMY));
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(battle.units[0].energy_milli, 35_000);
    assert!(
        !battle
            .events()
            .iter()
            .any(|e| e.kind == EventKind::WeaknessBroken)
    );
    assert!(battle.events().iter().any(|e| matches!(
        e.kind,
        EventKind::Damage {
            kind: DamageKind::SuperBreak,
            amount: 3_265,
            hp_loss: 2_334,
            critical: false,
            ..
        }
    )));
}

#[test]
fn harmony_trailblazer_e4_shares_live_break_effect_and_dancer_expires_on_opening() {
    let mut lead = trailblazer(4, &[]);
    lead.stats.break_effect_bp = 10_000;
    let prepared = plan(&[lead, ruan_mei()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.effective_break_effect(0), 12_000);
    assert_eq!(battle.effective_break_effect(1), 3_800);
    cast_ultimate(&mut battle);
    assert_eq!(battle.effective_break_effect(0), 15_000);
    assert_eq!(battle.effective_break_effect(1), 7_250);
    assert_eq!(
        battle.units[0]
            .harmony_trailblazer_state
            .backup_dancer_until_turn,
        4
    );
    battle.units[0].normal_turns = 3;
    assert_eq!(battle.effective_break_effect(1), 7_250);
    battle.units[0].normal_turns = 4;
    assert_eq!(battle.effective_break_effect(1), 3_800);
}

#[test]
fn harmony_trailblazer_super_break_attributes_ruan_meis_efficiency_and_besotted() {
    use crate::battle::combat_status::DebuffKind;
    let mut lead = trailblazer(0, &[]);
    lead.stats.break_effect_bp = 10_000;
    let prepared = plan(&[lead, ruan_mei()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    let mei = battle.mechanics(1).unwrap();
    mei.after_action(&mut battle, 1, Action::Skill, None);
    battle.apply_permanent_debuff(1, 2, DebuffKind::Besotted, 1_200);
    battle.set_toughness(0, 2, 0);
    assert!(
        battle.units[2]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::Besotted)
    );
    act(&mut battle, Action::Basic, Some(ENEMY));
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.kind, DamageKind::SuperBreak);
    // Ruan Mei adds 5 toughness, 20% BE, and 12% Break vulnerability.
    assert_eq!(hit.amount, 10_549);
    assert_eq!(hit.assists, [(1, 4_773)]);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|e| e.kind == EventKind::StatusApplied(StatusKind::Besotted))
            .count(),
        1
    );
}

#[test]
fn harmony_trailblazer_fixed_dpr_replays_and_preserves_super_break_attribution() {
    let prepared = plan(
        &[trailblazer(6, &[DANCE, SHUFFLE, HAT]), ruan_mei()],
        &[30, 120, 30],
    );
    let result = calculate_dpr(&prepared, &[30, 120, 30], STANDARD_DPR_SCENARIO.cycles).unwrap();
    let replay = calculate_dpr(&prepared, &[30, 120, 30], STANDARD_DPR_SCENARIO.cycles).unwrap();
    assert_eq!(result, replay);
    let hits: Vec<_> = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .filter(|hit| hit.kind == DamageKind::SuperBreak)
        .collect();
    assert!(hits.iter().any(|hit| {
        hit.source_id == MEI
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == TRAILBLAZER && credit.amount > 0)
    }));
    assert!(hits.iter().all(|hit| !hit.critical
        && hit.credits.iter().map(|credit| credit.amount).sum::<u32>() == hit.amount));
}
