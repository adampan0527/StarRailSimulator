use super::*;

use crate::{CombatType, Command, LogMode, OfficialStats, PreparedBattle};

const OTHER: &str = "character.1106";

fn robin(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: Element::Wind,
        },
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 100_000_000,
            speed_micros: 160_000_000,
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
            (BASIC.into(), vec![0.25]),
            (SKILL.into(), vec![0.5, 6.0]),
            (ULTIMATE.into(), vec![1.0, 2.0, 0.12, 0.0]),
            (
                TALENT.into(),
                vec![0.7, 1.8, 0.0, 0.0, 50.0, 6.0, 12.0, 0.1, 0.005],
            ),
            (BIRD_SKILL.into(), vec![0.0, 0.75]),
            (
                BIRD_TALENT.into(),
                vec![0.3, 0.01, 0.04, 0.06, 0.08, 12.0, 0.0, 0.0, 140.0, 0.5],
            ),
            (ARRIVAL.into(), vec![20.0]),
            (DEPARTURE.into(), vec![0.5]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn pela() -> OfficialCharacter {
    let mut ally = robin(0, &[]);
    ally.unit_id = OTHER.into();
    ally.form_id = OTHER.into();
    ally.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Ice,
    };
    ally.energy_capacity = 110;
    ally.stats.attack = 1_500;
    ally.stats.base_attack = 1_500;
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

fn plan(eidolon: u8, traces: &[&str]) -> PreparedBattle {
    PreparedBattle::official_dpr(&[robin(eidolon, traces), pela()], 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap()
}

fn apply(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command));
    battle.resolve_command(&command).unwrap();
}

#[test]
fn birds_recruit_at_six_and_twelve_then_countdown_ends_fever() {
    let plan = plan(0, &[]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();

    apply(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.units[sprite].hp, 3_500);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(pos_state(&battle, 0).birds, 1);
    assert!(battle.units[0].summon_turn.is_none());
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 1).vulnerability_bp,
        400
    );

    for _ in 0..6 {
        rules.on_ally_attack(&mut battle, 0, 1, Action::Basic, None);
    }
    assert_eq!(pos_state(&battle, 0).birds, 2);
    assert!(battle.units[0].summon_turn.is_none());

    for _ in 0..6 {
        rules.on_ally_attack(&mut battle, 0, 1, Action::Basic, None);
    }
    assert_eq!(pos_state(&battle, 0).birds, 3);
    assert!(battle.turn_is_suspended(0));
    assert_eq!(battle.speed_micros(sprite), 288_000_000);
    let aura = rules.damage_target_aura(&battle, 0, 1);
    assert_eq!((aura.defense_down_bp, aura.vulnerability_bp), (1_600, 800));
    assert!(rules.immune_to_enemy_control(&battle, 0));
    assert!(rules.memosprite_immune_to_enemy_control(&battle, 0));

    rules.on_countdown_turn(&mut battle, 0);
    assert_eq!(pos_state(&battle, 0).vibes, 0);
    assert!(!battle.turn_is_suspended(0));
    assert!(battle.active_memosprite(0).is_none());
    assert!(battle.units[0].summon_turn.is_none());
    assert!(battle.units[0].countdown.is_none());
    assert_eq!(battle.units[0].gauge_quanta, crate::battle::GAUGE_FULL / 2);
}

#[test]
fn guest_attacks_gain_extra_vibes_and_guest_cannot_advance_other_allies() {
    let plan = plan(0, &[]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, ENERGY_MILLI);
    battle.set_energy(1, 1, 0);
    let rules = battle.mechanics(0).unwrap();
    apply(&mut battle, Action::Ultimate, Some(OTHER));
    assert_eq!(battle.units[1].energy_milli, 13_200);
    assert_eq!(battle.units[1].gauge_quanta, 0);
    rules.on_ally_attack(&mut battle, 0, 1, Action::Skill, None);
    assert_eq!(pos_state(&battle, 0).vibes, 3);

    battle.units[0].gauge_quanta = crate::battle::GAUGE_FULL;
    battle.advance_ally_immediate(1, 0);
    assert_eq!(battle.units[0].gauge_quanta, crate::battle::GAUGE_FULL);
    battle.advance_ally_immediate(1, 1);
    rules.on_ally_turn_opening(&mut battle, 0, 1, false);
    assert!(battle.ally_advance_blocked(1, 0));
    rules.on_ally_turn_opening(&mut battle, 0, 1, false);
    assert!(!battle.ally_advance_blocked(1, 0));
    battle.advance_ally_immediate(1, 0);
    assert_eq!(battle.units[0].gauge_quanta, 0);
}

#[test]
fn sustain_and_e2_share_a_turn_gate_but_a4_receives_rhythm_on_each_eligible_heal() {
    let plan = plan(2, &[DEVIATED_CHORDS, IMPROVISED_BLUES, REBUILT_HARMONY]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    let before = battle.units[0].energy_milli;
    battle.heal_unit(1, 0, 700);
    assert_eq!(pos_state(&battle, 0).vibes, 3);
    assert_eq!(pos_state(&battle, 0).rhythm, 11);
    assert_eq!(battle.units[0].energy_milli, before + 3_000);
    // The 3 newly gained Vibes make the HP-scaled flat ATK buff 860.
    assert_eq!(battle.effective_attack(1), 2_360);
    battle.heal_unit(1, 1, 700);
    assert_eq!(pos_state(&battle, 0).vibes, 3);
    rules.on_ally_attack(&mut battle, 0, 0, Action::Basic, None);
    assert_eq!(pos_state(&battle, 0).vibes, 4);
    assert_eq!(battle.effective_crit_damage_bp(0), 9_600);

    battle.turn_sequence += 1;
    rules.on_ally_attack(&mut battle, 0, 1, Action::Skill, None);
    assert_eq!(pos_state(&battle, 0).vibes, 7);
    assert_eq!(battle.units[0].energy_milli, before + 6_000);
}

#[test]
fn e6_keeps_two_ultimate_charges_and_bird_true_damage_consumes_half_its_ledger() {
    let plan = plan(6, &[]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    battle.units[0].robin_sunny_state.vibes = 12;
    apply(&mut battle, Action::Skill, None);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(pos_state(&battle, 0).vibes, 24);
    assert_eq!(battle.energy_capacity_milli(0), 280_000);
    assert_eq!(battle.units[0].energy_milli, 280_000);
    assert_eq!(battle.speed_micros(sprite), 380_160_000);
    battle.advance_to_decision();
    apply(&mut battle, Action::Ultimate, Some(OTHER));
    assert_eq!(battle.units[0].energy_milli, 145_000);
    apply(&mut battle, Action::Ultimate, Some(OTHER));
    assert_eq!(battle.units[0].energy_milli, 10_000);

    let state = &mut battle.units[0].robin_sunny_state;
    state.damage_record = 10_000;
    let vibes = state.vibes;
    rules.on_summon_turn(&mut battle, 0);
    let damage: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == sprite)
        .collect();
    assert_eq!(damage.len(), 4);
    let direct: u64 = damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct)
        .map(|hit| u64::from(hit.amount))
        .sum();
    let recorded = 10_000 + direct;
    assert_eq!(
        u64::from(damage.last().unwrap().amount),
        recorded * (1_100 + 10 * u64::from(vibes)) / 10_000
    );
    assert_eq!(pos_state(&battle, 0).damage_record, recorded - recorded / 2);
    rules.on_countdown_turn(&mut battle, 0);
    assert!(battle.units[0].energy_milli >= 140_000);
}
