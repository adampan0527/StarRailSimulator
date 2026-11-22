use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, analysis::DummyDamage, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget, KILL_ENERGY_MILLI},
};

use super::{BASE, FORM, NOVAFLARE, OfficialCharacter, RETRIBUTION};
use crate::battle::support::AllyBuffKind;

const WELT: &str = "team.welt";
const ALLY: &str = "team.arlan";
const FIRST: &str = "dpr:1";

fn welt(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let ids = if enhanced { NOVAFLARE } else { BASE };
    let mut ultimate = vec![1.5, if enhanced { 0.12 } else { 0.4 }, 1.0, 0.1];
    let mut talent = vec![if enhanced { 1.0 } else { 0.6 }];
    if enhanced {
        ultimate.extend([0.04, 8.0, 2.0]);
        talent.extend([0.4, 0.05]);
    }
    OfficialCharacter {
        unit_id: WELT.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Imaginary,
        },
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 125_000_000,
            speed_micros: 125_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 10_000,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (ids.basic.into(), vec![1.0]),
            (ids.skill.into(), vec![0.72, 0.75, 0.1, 2.0]),
            (ids.ultimate.into(), ultimate),
            (ids.talent.into(), talent),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn ally() -> OfficialCharacter {
    let mut character = welt(false, 0, &[]);
    character.unit_id = ALLY.into();
    character.form_id = "character.1008".into();
    character.combat_type = crate::CombatType {
        path: crate::CombatPath::Destruction,
        element: crate::Element::Thunder,
    };
    character.energy_capacity = 110;
    character.stats.base_speed_micros = 120_000_000;
    character.stats.speed_micros = 120_000_000;
    character.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    character
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 0, 3)
        .unwrap()
        .with_dummies(toughness)
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

fn damage(battle: &Battle<'_>, kind: DamageKind) -> Vec<u32> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == kind)
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn validates_complete_and_consistent_skill_variants() {
    for variant in ["base", "novaflare"] {
        assert!(crate::supports_official_dpr_variant(FORM, variant));
    }
    assert!(!crate::supports_official_dpr_variant(FORM, "other"));
    let mut input = welt(true, 0, &[]);
    input.skill_parameters.insert(BASE.basic.into(), vec![1.0]);
    assert!(PreparedBattle::official_dpr(&[input], 0, 3).is_err());
    let mut input = welt(false, 0, &[]);
    input.skill_parameters.remove(BASE.talent);
    assert!(PreparedBattle::official_dpr(&[input], 0, 3).is_err());
}

#[test]
fn skill_bounces_use_per_hit_toughness_and_energy_and_check_slow_before_each_hit() {
    for (enhanced, segments, talent) in [(false, 3, 360), (true, 5, 600)] {
        let prepared = plan(&[welt(enhanced, 0, &[])], &[120]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        act(&mut battle, WELT, Action::Skill, Some(FIRST));
        assert_eq!(damage(&battle, DamageKind::Direct), vec![432; segments]);
        assert_eq!(
            damage(&battle, DamageKind::Additional),
            vec![talent; segments - 1]
        );
        assert_eq!(battle.units[1].toughness, 120 - 10 * segments as u16);
        assert_eq!(battle.units[0].energy_milli, 90_000);
        assert_eq!(battle.skill_points, 2);
        assert_eq!(battle.speed_micros(1), 90_000_000);
    }
}

#[test]
fn base_ultimate_keeps_skill_slow_and_break_imprisonment_separate() {
    let mut input = welt(
        false,
        2,
        &[BASE.retribution, BASE.judgement, BASE.punishment],
    );
    input.stats.break_effect_bp = 5_000;
    let prepared = plan(&[input], &[20]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    // Only the last 10 toughness remains. A pre-existing slow enables both Talent hits.
    battle.apply_speed_down(0, 1, 1_000, 2);
    battle.units[1].toughness = 10;
    let gauge = battle.units[1].gauge_quanta;
    battle.fill_energy(0);
    act(&mut battle, WELT, Action::Ultimate, None);
    assert_eq!(damage(&battle, DamageKind::Direct), [504, 672]);
    assert_eq!(damage(&battle, DamageKind::Additional), [537, 537]);
    assert_eq!(battle.units[0].energy_milli, 21_000);
    assert_eq!(
        battle.units[1].gauge_quanta - gauge,
        gauge_for(2_500 + 4_500 + 4_000)
    );
    assert_eq!(battle.speed_micros(1), 70_000_000);
    assert_eq!(battle.vulnerability_from(0, 1), 1_200);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusApplied(StatusKind::Imprisoned))
    );
    assert_eq!(battle.units[0].welt_state.honor_charges, 2);
    act(&mut battle, WELT, Action::Basic, Some(FIRST));
    assert_eq!(battle.units[0].welt_state.honor_charges, 1);
    assert_eq!(damage(&battle, DamageKind::Additional).len(), 4);
}

#[test]
fn finite_enemy_kill_redirects_later_bounces_without_extra_hits_on_defeated_target() {
    static FINITE_ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[welt(false, 6, &[])], &[120; 2]);
    prepared.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[1].stats.hp = 1;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, WELT, Action::Skill, Some(FIRST));
    assert_eq!(battle.hp(1).0, 0);
    assert_eq!(damage(&battle, DamageKind::Direct), [432; 3]);
    assert_eq!(damage(&battle, DamageKind::Additional), [360; 2]);
    assert_eq!(battle.units[2].toughness, 90);
    assert_eq!(battle.units[0].energy_milli, 106_000 + KILL_ENERGY_MILLI);

    let mut prepared = plan(&[welt(true, 0, &[])], &[120]);
    prepared.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[1].stats.hp = 1;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, WELT, Action::Skill, Some(FIRST));
    assert_eq!(battle.units[0].energy_milli, 66_000 + KILL_ENERGY_MILLI);
}

#[test]
fn weightless_attributes_debuff_and_attacker_stacks_and_caps_delay_per_enemy_turn() {
    let prepared = plan(
        &[welt(true, 4, &[NOVAFLARE.retribution]), ally()],
        &[120; 2],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    battle.units[2].gauge_quanta += gauge_for(10_000);
    let gauge = battle.units[2].gauge_quanta;
    battle.fill_energy(0);
    act(&mut battle, WELT, Action::Ultimate, None);
    assert_eq!(battle.units[2].gauge_quanta - gauge, gauge_for(1_200));
    assert!(damage(&battle, DamageKind::Additional).is_empty());
    assert_eq!(battle.speed_micros(2), 85_000_000);
    act(&mut battle, WELT, Action::Basic, Some(FIRST));
    act(&mut battle, ALLY, Action::Basic, Some(FIRST));

    // 1,000 × 0.9 × 1.30 × 1000 / (1000 + 500 × 0.6) = 900.
    let ally_hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 1)
        .unwrap();
    assert_eq!(ally_hit.amount, 900);
    assert_eq!(ally_hit.assists, [(0, 300)]);
    assert!(battle.has_ally_buff(0, 1, AllyBuffKind::Damage, RETRIBUTION));
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::Damage), 1_000);
    let rules = battle.mechanics(0).unwrap();
    // The first two attacks and 6 of the next 12 attacks delay only their own target.
    let gauge = battle.units[2].gauge_quanta;
    for _ in 0..12 {
        rules.on_attack_target(&mut battle, 0, 2);
        rules.on_attack_finished(&mut battle, 0, 1, Action::Basic);
    }
    assert_eq!(battle.units[2].gauge_quanta - gauge, gauge_for(2_400));
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::Damage), 10_000);
    let other = battle.units[3].gauge_quanta;
    rules.on_attack_target(&mut battle, 0, 3);
    assert_eq!(battle.units[3].gauge_quanta - other, gauge_for(400));
    battle.units[2].normal_turns += 1;
    battle.finish_turn(2);
    rules.on_attack_target(&mut battle, 0, 2);
    assert_eq!(battle.units[2].gauge_quanta - gauge, gauge_for(2_800));
    battle.units[2].normal_turns += 1;
    battle.finish_turn(2);
    assert!(!battle.has_weightless(0, 2));
    assert_eq!(battle.speed_micros(2), 100_000_000);
    battle.units[1].normal_turns += 1;
    battle.finish_turn(1);
    assert!(battle.has_ally_buff(0, 1, AllyBuffKind::Damage, RETRIBUTION));
    battle.units[1].normal_turns += 1;
    battle.finish_turn(1);
    assert!(!battle.has_ally_buff(0, 1, AllyBuffKind::Damage, RETRIBUTION));
}

#[test]
fn enhanced_eidolon_one_triggers_once_per_target_and_judgement_once_per_cast() {
    let mut character = welt(true, 6, &[NOVAFLARE.judgement, NOVAFLARE.punishment]);
    character.stats.crit_rate_bp = 7_000;
    character.stats.effect_hit_rate_bp = 8_000;
    let prepared = plan(&[character], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.effective_attack(0), 1_800);
    battle.fill_energy(0);
    act(&mut battle, WELT, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 10_000);
    assert!(damage(&battle, DamageKind::Additional).is_empty());
    battle.dummy_damage.clear();
    act(&mut battle, WELT, Action::Skill, Some(FIRST));
    let (direct, additional): (Vec<_>, Vec<_>) = battle
        .dummy_damage
        .iter()
        .partition(|hit| hit.kind == DamageKind::Direct);
    assert_eq!(direct.len(), 5);
    assert_eq!(additional.len(), 7);
    assert!(
        direct
            .iter()
            .chain(&additional)
            .all(|hit: &&DummyDamage| hit.critical)
    );
    assert_eq!(battle.units[0].energy_milli, 55_000);
}

#[test]
fn weightless_applies_even_if_enemy_resists_imprisonment_and_skill_slow() {
    let mut prepared = plan(&[welt(true, 0, &[])], &[120]);
    prepared.units[1].stats.effect_resistance_bp = 10_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, WELT, Action::Skill, Some(FIRST));
    assert!(!battle.is_slowed(1));
    assert!(damage(&battle, DamageKind::Additional).is_empty());
    let gauge = battle.units[1].gauge_quanta;
    battle.fill_energy(0);
    act(&mut battle, WELT, Action::Ultimate, None);
    assert_eq!(battle.units[1].gauge_quanta, gauge);
    assert!(battle.has_weightless(0, 1));
    assert_eq!(battle.speed_micros(1), 95_000_000);
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| { event.kind == EventKind::StatusApplied(StatusKind::Imprisoned) })
    );
}

#[test]
fn punishment_attack_bonus_counts_complete_hit_rate_steps_and_caps_at_eighty_percent() {
    for (effect_hit_rate_bp, attack) in [
        (4_000, 1_000),
        (4_999, 1_000),
        (5_000, 1_200),
        (7_999, 1_600),
        (8_000, 1_800),
        (9_000, 1_800),
    ] {
        let mut character = welt(true, 0, &[NOVAFLARE.punishment]);
        character.stats.effect_hit_rate_bp = effect_hit_rate_bp;
        let prepared = plan(&[character], &[120]);
        let battle = Battle::start(&prepared, LogMode::Off);
        assert_eq!(battle.effective_attack(0), attack);
    }
}

#[test]
fn fixed_dpr_replays_both_welt_versions_with_conserved_team_credit() {
    let toughness = &STANDARD_DPR_SCENARIO.toughness;
    let totals = [false, true].map(|enhanced| {
        let ids = if enhanced { NOVAFLARE } else { BASE };
        let inputs = [
            welt(
                enhanced,
                0,
                &[ids.retribution, ids.judgement, ids.punishment],
            ),
            ally(),
        ];
        let prepared = plan(&inputs, toughness);
        let summary = || calculate_dpr(&prepared, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap();
        let result = summary();
        assert_eq!(result, summary());
        assert!(result.characters[0].assist_damage > 0);
        assert_eq!(
            result
                .targets
                .iter()
                .map(|target| target.damage)
                .sum::<u64>(),
            result.total_damage
        );
        assert_eq!(
            result
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>(),
            result.total_damage
        );
        result.total_damage
    });
    assert!(totals[1] > totals[0], "{totals:?}");
}
