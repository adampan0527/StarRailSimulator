use crate::{
    BattleError, CombatType, Command, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";
const ARGENTI: &str = "character.1302";
const FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};
const NORMAL: EnemyKit = EnemyKit {
    max_toughness: 60,
    ..FINITE
};

fn herta(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: ICE,
        },
        energy_capacity: 220,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 99_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0, 3.0, 1.0, 0.3]),
            (SKILL.into(), vec![0.7, 1.0]),
            (ULTIMATE.into(), vec![2.0, 2.0, 1.0, 0.8, 3.0, 4.0]),
            (TALENT.into(), vec![0.08, 0.04, 42.0, 1.0, 42.0, 25.0]),
            (ENHANCED_SKILL.into(), vec![0.8, 1.0, 0.4]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn argenti(speed_micros: u64) -> OfficialCharacter {
    let mut character = herta(0, &[]);
    character.unit_id = ARGENTI.into();
    character.form_id = ARGENTI.into();
    character.combat_type.element = Element::Physical;
    character.energy_capacity = 180;
    character.stats.base_speed_micros = 103_000_000;
    character.stats.speed_micros = speed_micros;
    character.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    character
}

fn plan(characters: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(characters, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let input = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&input), "{input:?}");
    battle.apply(&input).unwrap();
}

fn layers(battle: &Battle<'_>) -> Vec<u8> {
    battle
        .enemies()
        .map(|enemy| battle.units[enemy].the_herta_interpretation)
        .collect()
}

fn set_layers(battle: &mut Battle<'_>, counts: &[u8]) {
    for (enemy, &count) in battle.enemies().zip(counts) {
        set_interpretation(battle, 0, enemy, count);
    }
}

fn damage_by_enemy(battle: &Battle<'_>) -> Vec<u32> {
    battle
        .enemies()
        .map(|enemy| {
            battle
                .dummy_damage
                .iter()
                .filter(|hit| {
                    hit.target == enemy && hit.actor == 0 && hit.kind == DamageKind::Direct
                })
                .map(|hit| hit.amount)
                .sum()
        })
        .collect()
}

#[test]
fn registers_only_the_base_form_and_requires_complete_locked_parameters() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = herta(0, &[]);
    input.skill_parameters.get_mut(TALENT).unwrap()[2] = 41.0;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
}

#[test]
fn skill_spreads_twice_and_basic_only_hits_once() {
    let plan = plan(&[herta(0, &[HONESTY, LANDSCAPE])], &[60, 60, 120, 60, 60]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(layers(&battle), [1, 1, 26, 1, 1]);
    assert_eq!(battle.units[0].the_herta_state.answer, 30);
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    assert_eq!(damage_by_enemy(&battle), [1_260, 840, 420, 0, 0]);
    assert_eq!(layers(&battle), [3, 2, 27, 1, 1]);
    assert_eq!(battle.units[0].energy_milli, 149_000);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.target)
            .collect::<Vec<_>>(),
        [1, 1, 2, 1, 2, 3]
    );
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(damage_by_enemy(&battle), [1_260, 1_440, 420, 0, 0]);
    assert_eq!(layers(&battle), [3, 3, 27, 1, 1]);
    assert_eq!(battle.units[0].energy_milli, 172_000);
    assert_eq!(battle.skill_points, 3);
}

#[test]
fn enhanced_skill_uses_primary_interpretation_in_every_phase_and_hits_far_targets() {
    let plan = plan(&[herta(0, &[HONESTY]), argenti(103_000_000)], &[60; 5]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_layers(&mut battle, &[42, 1, 0, 1, 10]);
    set_inspiration(&mut battle, 0, 1);
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    assert_eq!(
        damage_by_enemy(&battle),
        [8_568, 4_824, 4_104, 3_384, 3_384]
    );
    assert_eq!(layers(&battle), [2, 2, 1, 2, 11]);
    assert_eq!(battle.units[0].the_herta_state.inspiration, 0);
    assert_eq!(battle.units[0].energy_milli, 155_000);
    assert_eq!(battle.dummy_damage.len(), 11);
}

#[test]
fn e1_counts_half_of_the_highest_adjacent_or_primary_stacks_and_resets_to_fifteen() {
    let plan = plan(&[herta(1, &[HONESTY])], &[60; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_layers(&mut battle, &[40, 20, 2]);
    let rules = TheHerta {
        enhanced_skill: skill_attack(Effect::DirectedAreaAttack),
        skill_bp: 0,
        enhanced_skill_bp: 0,
        enhanced_finale_bp: 0,
        interpretation_main_bp: 0,
        interpretation_other_bp: 0,
        ultimate_attack_bp: 0,
        honesty: true,
        letter: false,
        landscape: false,
        eidolon: 1,
    };
    assert_eq!(rules.enhanced_layers_twice(&battle, 2), 80);
    set_layers(&mut battle, &[0, 27, 0]);
    assert_eq!(rules.enhanced_layers_twice(&battle, 2), 81);
    set_inspiration(&mut battle, 0, 1);
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    assert_eq!(damage_by_enemy(&battle), [3_312, 5_544, 3_312]);
    assert_eq!(layers(&battle), [1, 16, 1]);
    assert!(battle.events().iter().any(|event| matches!(
        event.kind,
        EventKind::Interpretation {
            before: 28,
            after: 15
        }
    )));
}

#[test]
fn letter_applies_flat_minimum_energy_additional_interpretation_and_team_crit_damage() {
    let mut character = herta(4, &[HONESTY, LETTER, LANDSCAPE]);
    character.stats.energy_regeneration_bp = 15_000;
    let mut follower = argenti(300_000_000);
    follower.stats.crit_rate_bp = 10_000;
    let plan = plan(&[character, follower], &[60, 120, 60]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.speed_micros(0), 211_880_000);
    assert_eq!(battle.speed_micros(1), 312_360_000);
    act(&mut battle, ARGENTI, Action::Basic, Some(MIDDLE));
    assert_eq!(layers(&battle), [1, 30, 1]);
    assert_eq!(battle.units[0].energy_milli, 119_000);
    assert_eq!(battle.units[0].the_herta_state.answer, 32);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.actor, 1);
    assert_eq!(hit.amount, 1_380);
    assert_eq!(hit.assists, [(0, 480)]);
}

#[test]
fn ultimate_reorders_stacks_before_damage_and_e2_advances_ordinary_turn() {
    let mut plan = plan(
        &[herta(2, &[LANDSCAPE]), argenti(350_000_000)],
        &[60, 120, 60],
    );
    plan.units[0].starting_energy_milli = ENERGY_CAP_MILLI;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_layers(&mut battle, &[30, 2, 5]);
    assert_eq!(battle.ordinary_actor(), Some(ARGENTI));
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(layers(&battle), [5, 30, 2]);
    assert_eq!(damage_by_enemy(&battle), [2_462; 3]);
    assert_eq!(battle.units[0].the_herta_state.answer, 28);
    assert_eq!(battle.units[0].the_herta_state.inspiration, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.ordinary_actor(), Some(ARGENTI));
    assert_eq!(battle.units[0].gauge_quanta, 0);
    assert!(!battle.in_extra_turn(0));
    act(&mut battle, ARGENTI, Action::Basic, Some(LEFT));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert!(battle.has_ally_buff(0, 0, AllyBuffKind::AttackPercent, ULTIMATE));
    act(&mut battle, FORM, Action::Skill, Some(MIDDLE));
    let after_advance = gauge_for(6_500);
    assert!(battle.events().iter().any(|event| matches!(
        event.kind, EventKind::GaugeShifted { after_quanta, .. } if after_quanta == after_advance
    )));
    assert_eq!(battle.units[0].the_herta_state.inspiration, 2);
}

#[test]
fn e6_ultimate_counts_each_living_target_once_and_answer_is_capped() {
    for (enemies, expected) in [(1, 9_059), (2, 7_115), (3, 5_689)] {
        let mut plan = plan(&[herta(6, &[LANDSCAPE])], &vec![60; enemies]);
        plan.units[0].starting_energy_milli = ENERGY_CAP_MILLI;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let mechanics = battle.mechanics(0).unwrap();
        for _ in 0..4 {
            for enemy in battle.enemies() {
                set_interpretation(&mut battle, 0, enemy, 0);
            }
            mechanics.on_wave_start(&mut battle, 0);
        }
        assert_eq!(battle.units[0].the_herta_state.answer, ANSWER_CAP);
        act(&mut battle, FORM, Action::Ultimate, None);
        assert_eq!(damage_by_enemy(&battle), vec![expected; enemies]);
        assert_eq!(battle.units[0].the_herta_state.inspiration, 3);
    }
}

#[test]
fn defeated_enemy_stacks_transfer_to_elites_and_final_stacks_continue_to_next_wave() {
    let mut plan = plan(&[herta(0, &[HONESTY, LANDSCAPE])], &[60; 5]);
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&NORMAL);
        enemy.stats.hp = 100;
    }
    plan.units[2].kit = Kit::Enemy(&FINITE);
    plan.units[5].kit = Kit::Enemy(&FINITE);
    plan.wave_ends = vec![4, 6];
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(layers(&battle), [1, 26, 1]);
    assert_eq!(battle.units[4].the_herta_interpretation, 0);
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(layers(&battle), [0, 28, 1]);
    assert_eq!(battle.units[0].the_herta_state.answer, 29);
    act(&mut battle, FORM, Action::Basic, Some(MIDDLE));
    assert_eq!(layers(&battle), [0, 0, 30]);
    act(&mut battle, FORM, Action::Basic, Some("dpr:3"));
    assert_eq!(battle.wave_index, 1);
    assert_eq!(layers(&battle), [1, 42]);
    assert_eq!(battle.units[0].the_herta_state.answer, 58);
    assert!(battle.units[0].the_herta_state.carried_interpretation == 0);
}

#[test]
fn a_lethal_first_enhanced_hit_transfers_only_reset_layers_and_counts_as_a_hit() {
    let mut plan = plan(&[herta(2, &[HONESTY, LANDSCAPE])], &[60; 2]);
    for enemy in &mut plan.units[1..] {
        enemy.kit = Kit::Enemy(&NORMAL);
    }
    plan.units[1].stats.hp = 100;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_layers(&mut battle, &[40, 3]);
    act(&mut battle, FORM, Action::Skill, Some(LEFT));
    assert_eq!(layers(&battle), [0, 20]);
    assert_eq!(battle.units[0].energy_milli, 156_000);
    assert_eq!(battle.units[0].the_herta_state.answer, 30);
    let direct_targets: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| {
            matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::Direct,
                    ..
                }
            )
            .then_some(event.target_id.as_deref())
            .flatten()
        })
        .collect();
    assert_eq!(direct_targets, [LEFT, MIDDLE, MIDDLE, MIDDLE]);
}

#[test]
fn fixed_dpr_repeats_and_the_enhanced_target_prefers_highest_interpretation() {
    let mut plan = plan(&[herta(2, &[HONESTY, LETTER, LANDSCAPE])], &[120; 3]);
    plan.units[0].starting_energy_milli = ENERGY_CAP_MILLI;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    set_layers(&mut battle, &[40, 1, 1]);
    let run = || calculate_dpr(&plan, &[120; 3], STANDARD_DPR_SCENARIO.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert!(result.total_damage > 0);
    assert!(result.targets.iter().all(|target| target.damage > 0));
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(
        mechanics.dpr_priority(&battle, 0, Action::Skill, Some(1)),
        Some(1)
    );
    assert_eq!(
        mechanics.dpr_priority(&battle, 0, Action::Skill, Some(2)),
        Some(2)
    );
}
