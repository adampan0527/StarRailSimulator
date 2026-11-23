use super::*;

use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle,
    battle::Kit,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const PELA: &str = "character.1106";
const CENTER: &str = "dpr:2";
const A2: &str = "trace.8003101";
const A4: &str = "trace.8003102";
const A6: &str = "trace.8003103";

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn trailblazer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Preservation,
            element: Element::Fire,
        },
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 150,
            base_speed_micros: 95_000_000,
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
            (BASIC.into(), vec![0.5]),
            (ENHANCED_BASIC.into(), vec![0.9, 0.36]),
            (SKILL.into(), vec![0.4, 1.0, 1.0]),
            (ULTIMATE.into(), vec![0.5, 0.75]),
            (TALENT.into(), vec![0.04, 2.0, 8.0, 20.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn pela() -> OfficialCharacter {
    let mut ally = trailblazer(0, &[]);
    ally.unit_id = PELA.into();
    ally.form_id = PELA.into();
    ally.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Ice,
    };
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 120_000_000;
    ally.skill_parameters = [
        ("skill.110601".into(), vec![0.5]),
        ("skill.110602".into(), vec![1.05, 1.0]),
        ("skill.110603".into(), vec![1.0, 0.3, 2.0, 0.6]),
        ("skill.110604".into(), vec![5.0]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn magma_will(battle: &Battle<'_>, actor: usize) -> u8 {
    battle.units[actor]
        .preservation_trailblazer_state
        .magma_will
}

fn direct_damage(battle: &Battle<'_>) -> Vec<u32> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn ordinary_basic_gains_will_and_paid_enhancement_consumes_it_before_the_blast() {
    let plan = plan(&[trailblazer(1, &[A4])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_magma_will(&mut battle, 0, 3);
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(magma_will(&battle, 0), 4);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(direct_damage(&battle), [375]);
    assert_eq!(battle.units[2].toughness, 390);
    assert_eq!(battle.shield_amount(0), 40);

    battle.units[0].hp = 4_000;
    let start = battle.events().len();
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(magma_will(&battle, 0), 0);
    assert_eq!(battle.units[0].energy_milli, 110_000);
    assert_eq!(battle.skill_points, 5);
    assert_eq!(direct_damage(&battle), [375, 690, 216, 216]);
    assert_eq!(battle.units[2].toughness, 370);
    assert_eq!(battle.units[1].toughness, 390);
    assert_eq!(battle.units[3].toughness, 390);
    assert_eq!(battle.hp(0), (4_250, 5_000));
    let events = &battle.events()[start..];
    let spend = events
        .iter()
        .position(|event| {
            event.kind
                == EventKind::MagmaWill {
                    before: 4,
                    after: 0,
                }
        })
        .unwrap();
    let hit = events
        .iter()
        .position(|event| matches!(event.kind, EventKind::Toughness { .. }))
        .unwrap();
    assert!(spend < hit);
}

#[test]
fn ultimate_uses_both_stats_and_grants_exactly_one_free_enhanced_basic() {
    let plan = plan(&[trailblazer(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 120_000;
    set_magma_will(&mut battle, 0, 6);
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(direct_damage(&battle), [525; 3]);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.skill_points, 3);
    assert!((1..=3).all(|enemy| battle.units[enemy].toughness == 380));
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(magma_will(&battle, 0), 6);
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(magma_will(&battle, 0), 2);
    assert_eq!(battle.units[0].energy_milli, 65_000);
    assert_eq!(
        direct_damage(&battle),
        [525, 525, 525, 540, 216, 216, 540, 216, 216]
    );
}

#[test]
fn skill_taunt_respects_effect_resistance_and_reduces_shielded_hits() {
    let mut plan = plan(&[pela(), trailblazer(0, &[A2])]);
    for enemy in &mut plan.units[2..] {
        enemy.kit = Kit::Enemy(&ENEMY);
        enemy.stats.attack = 1_500;
    }
    plan.units[3].stats.effect_resistance_bp = 10_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    assert_eq!(magma_will(&battle, 1), 1);
    assert_eq!(battle.skill_points, 2);
    let rules = battle.mechanics(1).unwrap();
    assert!(rules.taunts_enemy(&battle, 1, 2));
    assert!(!rules.taunts_enemy(&battle, 1, 3));
    assert!(rules.taunts_enemy(&battle, 1, 4));
    assert_eq!(battle.shield_amount(0), 40);
    assert_eq!(battle.shield_amount(1), 40);

    battle.apply_official_shield(1, 1, 1_000, 1, Some(Action::Skill));
    let start = battle.events().len();
    battle.resolve_enemy(2, &ENEMY);
    battle.resolve_enemy(3, &ENEMY);
    let hits = battle.events()[start..]
        .iter()
        .filter_map(|event| {
            if let EventKind::Damage {
                amount,
                absorbed,
                hp_loss,
                ..
            } = event.kind
            {
                Some((event.target_id.as_deref(), amount, absorbed, hp_loss))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    // The level 32 attacker has 520 attacker defense points against 500 DEF.
    // The expected hits combine that ratio with 0.60 × 0.85 or 0.85.
    assert_eq!(
        hits,
        [(Some(FORM), 390, 390, 0), (Some(PELA), 650, 40, 610)]
    );
    assert_eq!(magma_will(&battle, 1), 2);

    battle.units[2].normal_turns = 1;
    battle.finish_turn(2);
    assert!(!rules.taunts_enemy(&battle, 1, 2));
    assert!(rules.taunts_enemy(&battle, 1, 4));
    battle.units[0].normal_turns = 1;
    battle.finish_turn(0);
    assert_eq!(battle.support_damage_reduction_bp(0), 0);
    battle.units[1].normal_turns = 2;
    rules.on_ally_turn_opening(&mut battle, 1, 1, false);
    assert_eq!(
        rules.incoming_damage_reduction_bp(&battle, 1, 2, DamageKind::Direct),
        0
    );
}

#[test]
fn e2_e4_e6_and_shield_trace_resolve_at_the_expected_turn_boundaries() {
    let plan = plan(&[trailblazer(6, &[A6])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(magma_will(&battle, 0), 4);
    assert_eq!(battle.units[0].energy_milli, 60_000);
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(battle.defense(0), 550);
    assert_eq!(battle.shield_amount(0), 80);
    assert_eq!(battle.units[0].energy_milli, 95_000);

    let start = battle.events().len();
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(direct_damage(&battle), [690, 216, 216, 427]);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.units[0].shields.len(), 1);
    assert!(
        battle.events()[start..]
            .iter()
            .any(|event| event.kind == EventKind::StatusApplied(crate::StatusKind::Shield))
    );

    for _ in 0..3 {
        battle.units[0].energy_milli = 120_000;
        act(&mut battle, Action::Ultimate, None);
    }
    assert_eq!(battle.defense(0), 650);
    assert_eq!(battle.shield_amount(0), 86);
    assert_eq!(battle.units[0].shields.len(), 1);

    let rules = battle.mechanics(0).unwrap();
    for _ in 0..10 {
        rules.after_enemy_attack(&mut battle, 0);
    }
    assert_eq!(magma_will(&battle, 0), 8);
    let next = battle.units[0].normal_turns + 2;
    battle.units[0].normal_turns = next - 1;
    battle.finish_turn(0);
    assert!(battle.shield_amount(0) > 0);
    battle.units[0].normal_turns = next;
    battle.finish_turn(0);
    assert_eq!(battle.shield_amount(0), 0);
}
