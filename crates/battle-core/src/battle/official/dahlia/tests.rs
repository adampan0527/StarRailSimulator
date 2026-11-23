use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle,
    battle::{Kit, resolve::gauge_for},
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const DAHLIA: &str = "ally.dahlia";
const PARTNER: &str = "ally.partner";
const FUGUE: &str = "ally.fugue";

fn dahlia(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: DAHLIA.into(),
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
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![1.6, 3.0, 0.5]),
            (ULTIMATE.into(), vec![3.0, 4.0, 0.18]),
            (TALENT.into(), vec![0.3, 5.0, 2.0, 35.0, 0.6]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn harmony() -> OfficialCharacter {
    let mut unit = dahlia(0, &[]);
    unit.unit_id = PARTNER.into();
    unit.form_id = "trailblazer.harmony".into();
    unit.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Imaginary,
    };
    unit.energy_capacity = 140;
    unit.stats.speed_micros = 100_000_000;
    unit.stats.break_effect_bp = 3_000;
    unit.skill_parameters = [
        ("skill.800501".into(), vec![1.0]),
        ("skill.800502".into(), vec![0.5, 2.0]),
        ("skill.800503".into(), vec![3.0, 1.0, 0.3]),
        ("skill.800504".into(), vec![10.0]),
    ]
    .into();
    unit
}

fn fugue() -> OfficialCharacter {
    let mut unit = dahlia(0, &[]);
    unit.unit_id = FUGUE.into();
    unit.form_id = "character.1225".into();
    unit.stats.speed_micros = 100_000_000;
    unit.skill_parameters = [
        ("skill.122501".into(), vec![1.0]),
        ("skill.122502".into(), vec![3.0, 0.3, 1.0, 0.18, 2.0, 0.5]),
        ("skill.122503".into(), vec![2.0]),
        ("skill.122504".into(), vec![1.0, 0.4]),
        ("skill.122508".into(), vec![1.0, 0.5]),
    ]
    .into();
    unit
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 17, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    battle
        .apply(&Command {
            decision_index: battle.snapshot().decision_index,
            actor_id: DAHLIA.into(),
            action,
            target_id: target.map(str::to_owned),
        })
        .unwrap();
}

fn basic(battle: &mut Battle<'_>, actor: usize, target: usize, element: Element) {
    battle.begin_attack();
    battle.action = Some(Action::Basic);
    battle.attack_hit(actor, target, element, 10_000, 10);
    battle.notify_attack(actor, Action::Basic, Some(target));
    battle.resolve_follow_ups();
    battle.action = None;
}

#[test]
fn frozen_base_skill_sets_the_zone_and_ultimate_divides_the_total_damage() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let plan = prepared(&[dahlia(0, &[])], &[200; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    act(&mut battle, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 130_000);
    for enemy in 1..4 {
        assert_eq!(battle.units[enemy].toughness, 185);
    }
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [960; 3],
    );
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.skill_points, 2);
    for enemy in 1..4 {
        assert_eq!(battle.units[enemy].toughness, 155);
        assert_eq!(battle.units[enemy].debuffs[0].remaining_turns(0), Some(4));
    }
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct && hit.action == Action::Ultimate)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [638; 3],
    );
}

#[test]
fn zone_opens_other_super_break_sources_before_break_and_credits_dahlia() {
    let plan = prepared(&[dahlia(0, &[]), harmony(), fugue()], &[200]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(partner(&battle, 0), Some(1));
    basic(&mut battle, 2, 3, FIRE);
    assert_eq!(battle.dummy_damage.len(), 1);
    act(&mut battle, Action::Skill, Some("dpr:1"));
    battle.dummy_damage.clear();
    basic(&mut battle, 2, 3, FIRE);
    assert_eq!(battle.units[3].toughness, 160);
    let first = &battle.dummy_damage[1];
    assert_eq!(
        (first.kind, first.actor, first.amount),
        (DamageKind::SuperBreak, 2, 3_390)
    );
    assert_eq!(first.assists, [(0, 3_390)]);

    let zone_end = battle.units[0].dahlia_state.zone_expires_on.unwrap();
    battle.units[0].normal_turns = zone_end;
    battle.notify_ally_turn_opening(0, false);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusExpired(StatusKind::DahliaZone))
    );
    battle.dummy_damage.clear();
    basic(&mut battle, 2, 3, FIRE);
    assert_eq!(battle.units[3].toughness, 150);
    assert_eq!(battle.dummy_damage.len(), 1);
    battle.set_toughness(2, 3, 0);
    basic(&mut battle, 2, 3, FIRE);
    assert_eq!(battle.dummy_damage.last().unwrap().amount, 2_511);
}

#[test]
fn follow_up_converts_every_bounce_and_the_attack_once() {
    let plan = prepared(&[dahlia(0, &[A4]), harmony()], &[250]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, Some("dpr:1"));
    battle.set_energy(0, 0, 40_000);
    battle.dummy_damage.clear();
    basic(&mut battle, 1, 2, Element::Imaginary);
    assert_eq!(battle.units[2].toughness, 198);
    assert_eq!(battle.units[0].energy_milli, 50_000);
    let dahlia_hits = battle.dummy_damage.iter().filter(|hit| hit.actor == 0);
    assert_eq!(
        dahlia_hits
            .clone()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .count(),
        5
    );
    assert_eq!(
        dahlia_hits
            .filter(|hit| hit.kind == DamageKind::SuperBreak)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [2_034, 2_034, 2_034, 2_034, 2_034, 3_051]
    );
    assert_eq!(battle.skill_points, 2);
    battle.dummy_damage.clear();
    basic(&mut battle, 1, 2, Element::Imaginary);
    assert!(battle.dummy_damage.iter().all(|hit| hit.actor == 1));
    battle.units[0].normal_turns += 1;
    basic(&mut battle, 1, 2, Element::Imaginary);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn e1_reduces_each_enemy_once_and_uses_the_frozen_minimum_and_maximum() {
    let plan = prepared(&[dahlia(1, &[]), harmony(), fugue()], &[39, 1_201, 202]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Skill, Some("dpr:2"));
    assert_eq!(
        battle.units[3..6]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [14, 886, 137]
    );
    assert_eq!(battle.units[5].toughness_reduction_remainder, 50);
    let center = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.target == 4 && hit.kind == DamageKind::SuperBreak)
        .unwrap();
    assert_eq!(center.amount, 71_206);
    basic(&mut battle, 0, 4, FIRE);
    assert_eq!(battle.units[4].toughness, 871);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.super_break_multiplier_bp(&battle, 0, 1, 4), 10_000);
    assert_eq!(rules.super_break_multiplier_bp(&battle, 0, 2, 4), 6_000);
    battle.confirm_defeat(0, 4);
    assert!(!battle.units[4].dahlia_state.eidolon_toughness_used);
}

#[test]
fn a6_grants_speed_and_fixed_fire_toughness_once_per_implanted_target() {
    let plan = prepared(&[dahlia(0, &[A6]), fugue()], &[80; 2]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(1, 1, 64_000);
    battle.begin_attack();
    battle.action = Some(Action::Skill);
    for target in [2, 2, 3] {
        battle.implant_weakness(1, target, FIRE, 2);
    }
    for target in 2..4 {
        battle.attack_hit(1, target, FIRE, 10_000, 10);
    }
    battle.notify_attack(1, Action::Skill, Some(2));
    battle.action = None;
    assert_eq!(battle.units[1].energy_milli, 65_000);
    assert_eq!(
        (battle.units[2].toughness, battle.units[3].toughness),
        (50, 50)
    );
    assert_eq!(battle.speed_micros(1), 130_000_000);
    assert!(battle.has_ally_buff(0, 1, AllyBuffKind::SpeedPercent, A6));
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind
                == EventKind::Energy {
                    before_milli: 64_000,
                    after_milli: 65_000,
                })
            .count(),
        1
    );
}

#[test]
fn fixed_fire_toughness_contributes_to_another_super_break_source() {
    let plan = prepared(&[dahlia(0, &[A6]), fugue()], &[80]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_toughness(1, 2, 0);
    battle.begin_attack();
    battle.action = Some(Action::Skill);
    battle.implant_weakness(1, 2, FIRE, 2);
    battle.attack_hit(1, 2, FIRE, 10_000, 10);
    battle.notify_attack(1, Action::Skill, Some(2));
    let conversion = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.kind == DamageKind::SuperBreak && hit.amount == 7_535)
        .unwrap();
    assert_eq!(conversion.actor, 1);
    assert_eq!(conversion.assists, [(0, 5_024)]);
}

#[test]
fn wilt_implants_only_dancers_elements_and_e2_covers_a_later_wave() {
    static ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 200,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut plan = prepared(&[dahlia(2, &[]), harmony(), fugue()], &[200; 2]);
    plan.units[3].kit = Kit::Enemy(&ENEMY);
    plan.units[4].kit = Kit::Enemy(&ENEMY);
    plan.wave_ends = vec![4, 5];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.has_weakness(3, FIRE));
    assert!(battle.has_weakness(3, Element::Imaginary));
    assert!(!battle.has_weakness(3, Element::Physical));
    assert_eq!(battle.units[3].debuffs[0].remaining_turns(0), Some(3));
    assert!(battle.units[4].debuffs.is_empty());
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .damage_target_aura(&battle, 0, 1)
            .resistance_down_bp,
        2_000
    );
    battle.confirm_defeat(3, 1);
    assert_eq!(partner(&battle, 0), Some(2));
    battle.confirm_defeat(0, 3);
    battle.resolve_wave_or_end();
    assert!(battle.has_weakness(4, FIRE));
    assert!(!battle.has_weakness(4, Element::Imaginary));
    assert_eq!(battle.units[4].debuffs[0].remaining_turns(0), Some(3));
}

#[test]
fn a2_refreshes_after_sustain_and_e6_advances_both_dancers_after_e4_bounces() {
    let plan = prepared(&[dahlia(6, &[A2]), harmony(), fugue()], &[1_000; 2]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.effective_break_effect(0), 15_000);
    assert_eq!(battle.effective_break_effect(1), 26_600);
    assert_eq!(battle.effective_break_effect(2), 8_600);
    battle.units[1].normal_turns = 1;
    battle.expire_ally_buffs(1);
    assert_eq!(battle.effective_break_effect(1), 18_000);
    battle.notify_ally_sustain(2, 0);
    assert_eq!(battle.effective_break_effect(1), 26_600);
    battle.units[1].normal_turns = 2;
    battle.notify_ally_sustain(2, 0);
    battle.units[1].normal_turns = 4;
    battle.expire_ally_buffs(1);
    assert_eq!(battle.effective_break_effect(1), 18_000);

    for actor in 0..3 {
        battle.units[actor].gauge_quanta = gauge_for(5_000);
    }
    basic(&mut battle, 1, 3, Element::Imaginary);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .count(),
        10
    );
    for target in [3, 4] {
        assert!(
            battle.units[target]
                .debuffs
                .iter()
                .any(|debuff| debuff.kind == DebuffKind::Vulnerability && debuff.bp == 1_200)
        );
    }
    assert_eq!(battle.units[0].gauge_quanta, gauge_for(3_000));
    assert_eq!(battle.units[1].gauge_quanta, gauge_for(3_000));
    assert_eq!(battle.units[2].gauge_quanta, gauge_for(5_000));
}
