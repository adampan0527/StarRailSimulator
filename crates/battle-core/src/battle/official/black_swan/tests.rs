use crate::{
    Battle, DamageKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, periodic::SkillDot},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
    supports_official_dpr_variant,
};

use super::*;

const SWAN: &str = "team.swan";
const KAFKA: &str = "team.kafka";

fn swan(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let ids = if enhanced { NOVAFLARE } else { BASE };
    let (basic, skill, retention, talent) = if enhanced {
        (
            vec![1.0],
            vec![0.90, 3.0, 1.0, 0.208],
            0.5,
            vec![2.40, 0.65, 0.12, 0.12, 1.80, 0.0, 0.20, 50.0],
        )
    } else {
        (
            vec![0.60, 0.65, 0.65],
            vec![0.90, 1.0, 1.0, 0.208, 3.0],
            1.0,
            vec![2.40, 0.65, 0.12, 3.0, 1.80, 7.0, 0.20, 50.0],
        )
    };
    OfficialCharacter {
        unit_id: SWAN.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Nihility,
            element: crate::Element::Wind,
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
            base_speed_micros: 102_000_000,
            speed_micros: 200_000_000,
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
            (ids.basic.into(), basic),
            (ids.skill.into(), skill),
            (ids.ultimate.into(), vec![1.2, 2.0, 0.25, retention]),
            (ids.talent.into(), talent),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn kafka() -> OfficialCharacter {
    let mut input = swan(false, 0, &[]);
    input.unit_id = KAFKA.into();
    input.form_id = "character.1005".into();
    input.combat_type.element = crate::Element::Thunder;
    input.stats.speed_micros = 300_000_000;
    input.skill_parameters = [
        ("skill.100501".into(), vec![1.0]),
        ("skill.100502".into(), vec![1.6, 0.75, 0.6]),
        ("skill.100503".into(), vec![0.8, 1.0, 2.0, 2.9, 1.0]),
        ("skill.100504".into(), vec![1.4, 1.0, 2.0]),
    ]
    .into();
    input
}

fn prepared(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 0, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap();
    for unit in &mut plan.units[team.len()..] {
        unit.stats.base_speed_micros = 50_000_000;
    }
    plan
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

fn seed_arcana(battle: &mut Battle<'_>, target: usize, stacks: u32, enhanced: bool) {
    battle.units[target].black_swan.arcana = Some(Arcana {
        source: 0,
        stacks,
        origin: Action::Basic,
        all_elements: enhanced,
    });
}

fn stacks(battle: &Battle<'_>, target: usize) -> u32 {
    battle.units[target]
        .black_swan
        .arcana
        .map_or(0, |arcana| arcana.stacks)
}

fn seed_dot(battle: &mut Battle<'_>, target: usize, element: Element) {
    assert!(battle.apply_skill_dot(
        1,
        target,
        10_000,
        SkillDot {
            element,
            damage: DotScale::Attack(10_000),
            origin: Action::Skill,
            turns: 2,
            max_stacks: 1,
        },
    ));
}

fn dot_damage(battle: &Battle<'_>) -> Vec<(usize, usize, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::DamageOverTime)
        .map(|hit| {
            assert!(!hit.critical);
            (hit.actor, hit.target, hit.amount)
        })
        .collect()
}

fn enemy_turn(battle: &mut Battle<'_>, target: usize) {
    battle.units[target].normal_turns += 1;
    battle.enemy_turn = Some(target);
    battle.resolve_enemy_turn(target);
    battle.enemy_turn = None;
}

#[test]
fn only_frozen_base_or_novaflare_parameter_sets_are_accepted() {
    assert!(supports_official_dpr_variant(FORM, "base"));
    assert!(supports_official_dpr_variant(FORM, "novaflare"));
    assert!(!supports_official_dpr_variant(FORM, "custom"));

    for enhanced in [false, true] {
        let mut input = swan(enhanced, 0, &[]);
        assert!(build(&input).is_ok());
        let other = if enhanced { BASE } else { NOVAFLARE };
        input.skill_parameters.insert(other.basic.into(), vec![1.0]);
        assert!(build(&input).is_err());
    }
    let mut input = swan(true, 0, &[]);
    input.skill_parameters.remove(NOVAFLARE.talent);
    assert!(build(&input).is_err());
}

#[test]
fn base_basic_uses_preexisting_dot_types_and_skill_only_adds_trace_stacks_to_primary() {
    let plan = prepared(&[swan(false, 0, &[BASE.viscera]), kafka()], &[120; 3]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:2"));
    battle.units[1].kafka_follow_up_charges = 0;
    act(&mut battle, SWAN, Action::Basic, Some("dpr:2"));
    assert_eq!(stacks(&battle, 3), 1);
    seed_dot(&mut battle, 3, Element::Fire);
    seed_dot(&mut battle, 3, Element::Physical);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:3"));
    act(&mut battle, SWAN, Action::Skill, Some("dpr:2"));
    assert_eq!(
        (stacks(&battle, 2), stacks(&battle, 3), stacks(&battle, 4)),
        (1, 5, 1)
    );
    // 90% ATK * 1000/(1000+500) * 0.9, followed by 20/10/10 toughness loss.
    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| {
            hit.actor == 0 && hit.kind == DamageKind::Direct && hit.action == Action::Skill
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [540; 3]
    );
    assert_eq!(
        battle.units[2..=4]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [110, 80, 100],
    );
    assert!(battle.units[2..=4].iter().all(|unit| {
        unit.debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::DefenseDown && debuff.bp == 2_080)
    }));
}

#[test]
fn base_turn_ticks_other_dots_before_arcana_and_uses_the_seven_stack_defense_bypass() {
    let plan = prepared(&[swan(false, 0, &[]), kafka()], &[120; 3]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    seed_arcana(&mut battle, 3, 6, false);
    seed_dot(&mut battle, 3, Element::Fire);
    enemy_turn(&mut battle, 3);
    // The burn grants the seventh stack. (240% + 6 * 12%) * 1000 * 0.9 / 1.4;
    // neighbors receive 180% ATK with the same DEF bypass.
    assert_eq!(
        dot_damage(&battle),
        [(1, 3, 600), (0, 3, 2_005), (0, 2, 1_157), (0, 4, 1_157)]
    );
    assert_eq!(
        (stacks(&battle, 2), stacks(&battle, 3), stacks(&battle, 4)),
        (1, 1, 1)
    );

    battle.dummy_damage.clear();
    seed_arcana(&mut battle, 2, 2, false);
    enemy_turn(&mut battle, 2);
    assert_eq!(dot_damage(&battle), [(0, 2, 1_512)]);
}

#[test]
fn kafka_detonation_awards_three_base_trace_stacks_once_per_target_per_attack() {
    let plan = prepared(&[swan(false, 0, &[BASE.goblet]), kafka()], &[120; 3]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    assert_eq!(stacks(&battle, 3), 1);
    for element in [WIND, Element::Physical, Element::Fire] {
        seed_dot(&mut battle, 3, element);
    }
    battle.apply_skill_shock(1, 3, Action::Ultimate, 10_000, 10_000, 2);
    act(&mut battle, KAFKA, Action::Skill, Some("dpr:2"));
    assert_eq!(stacks(&battle, 3), 4);
    let hits = dot_damage(&battle);
    assert_eq!(hits.len(), 5);
    // Four Arcana stacks. The detonation gets no turn-start effects.
    assert_eq!(hits[4], (0, 3, 1_242));
    assert_eq!(battle.dummy_damage.last().unwrap().action, Action::Skill);

    battle.detonate_all_dots(1, 3, 7_500, Action::Skill);
    assert_eq!(stacks(&battle, 3), 4);
    battle.begin_attack();
    battle.detonate_all_dots(1, 3, 7_500, Action::Skill);
    assert_eq!(stacks(&battle, 3), 7);
}

#[test]
fn detonation_can_apply_arcana_for_a_later_hit_when_no_arcana_was_present() {
    let plan = prepared(&[swan(true, 0, &[]), kafka()], &[120]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    seed_dot(&mut battle, 2, Element::Fire);
    battle.detonate_all_dots(1, 2, 10_000, Action::Skill);
    assert_eq!(dot_damage(&battle), [(1, 2, 600)]);
    assert_eq!(stacks(&battle, 2), 1);

    battle.dummy_damage.clear();
    battle.detonate_all_dots(1, 2, 10_000, Action::Skill);
    assert_eq!(dot_damage(&battle), [(1, 2, 600), (0, 2, 1_620)]);
    assert_eq!(stacks(&battle, 2), 3);
}

#[test]
fn novaflare_overflow_deals_damage_then_trims_without_clearing_regular_stacks_on_detonation() {
    let plan = prepared(&[swan(true, 0, &[]), kafka()], &[120; 3]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    seed_arcana(&mut battle, 3, 55, true);
    battle.detonate_all_dots(1, 3, 7_500, Action::Skill);
    // (240% + 54 * 12%) * 1000 * 0.9 / 1.4 * 0.75.
    assert_eq!(dot_damage(&battle), [(0, 3, 4_281)]);
    assert_eq!(stacks(&battle, 3), 50);
    assert_eq!(stacks(&battle, 2), 0);

    battle.dummy_damage.clear();
    enemy_turn(&mut battle, 3);
    assert_eq!(
        dot_damage(&battle),
        [(0, 3, 5_322), (0, 2, 1_157), (0, 4, 1_157)]
    );
    assert_eq!(
        (stacks(&battle, 2), stacks(&battle, 3), stacks(&battle, 4)),
        (1, 25, 1)
    );
}

#[test]
fn novaflare_traces_add_five_stacks_on_attacked_targets_and_refresh_defense_down() {
    let plan = prepared(
        &[
            swan(true, 0, &[NOVAFLARE.viscera, NOVAFLARE.goblet]),
            kafka(),
        ],
        &[120; 3],
    );
    let mut battle = Battle::start(&plan, LogMode::Off);
    assert!((2..=4).all(|enemy| stacks(&battle, enemy) == 1));
    assert!((2..=4).all(|enemy| battle.debuff_count(enemy) == 2));
    battle.cleanse_one(2, 2);
    assert_eq!(battle.debuff_count(2), 1);

    act(&mut battle, KAFKA, Action::Basic, Some("dpr:3"));
    battle.units[1].kafka_follow_up_charges = 0;
    act(&mut battle, SWAN, Action::Basic, Some("dpr:1"));
    assert_eq!(stacks(&battle, 2), 6);
    assert_eq!(stacks(&battle, 3), 1);
    assert_eq!(battle.debuff_count(2), 2);
    battle.set_energy(0, 0, 120_000);
    act(&mut battle, SWAN, Action::Ultimate, None);
    assert!((2..=4).all(|enemy| battle.debuff_count(enemy) == 3));
    assert!(stacks(&battle, 2) > 11);
    assert!(stacks(&battle, 3) > 6);

    for turn in 1..=3 {
        battle.units[2].normal_turns = turn;
        battle.expire_debuffs(2);
        let count = match turn {
            1 => 3,
            2 => 2,
            _ => 1,
        };
        assert_eq!(battle.debuff_count(2), count);
    }
    battle.cleanse_one(2, 2);
    assert_eq!(battle.debuff_count(2), 0);
    assert!(!battle.has_elemental_dot(2, WIND));
}

#[test]
fn epiphany_maps_statuses_once_and_preserves_one_base_tick_or_both_novaflare_ticks() {
    for enhanced in [false, true] {
        let plan = prepared(&[swan(enhanced, 0, &[]), kafka()], &[120]);
        let mut battle = Battle::start(&plan, LogMode::Off);
        seed_arcana(&mut battle, 2, 8, enhanced);
        assert_eq!(battle.debuff_count(2), 1);
        assert_eq!(battle.has_elemental_dot(2, Element::Fire), enhanced);
        battle.set_energy(0, 0, 120_000);
        act(&mut battle, SWAN, Action::Ultimate, None);
        assert_eq!(battle.debuff_count(2), 2);
        assert!(
            DOT_ELEMENTS
                .into_iter()
                .all(|element| battle.has_elemental_dot(2, element))
        );
        assert_eq!(
            battle.dummy_damage.last().unwrap().amount,
            if enhanced { 900 } else { 720 }
        );

        battle.detonate_dots(1, 2, Element::Fire, 5_000, Action::Ultimate);
        assert_eq!(dot_damage(&battle).len(), 1);
        let after_detonation = stacks(&battle, 2);
        enemy_turn(&mut battle, 2);
        assert!(stacks(&battle, 2) > after_detonation);
        enemy_turn(&mut battle, 2);
        assert_eq!(battle.debuff_count(2), 1);
        assert_eq!(battle.has_elemental_dot(2, Element::Fire), enhanced);
        if enhanced {
            assert!(stacks(&battle, 2) > after_detonation);
        } else {
            assert_eq!(stacks(&battle, 2), 1);
        }
    }
}

#[test]
fn e1_elemental_resistance_and_e6_ally_attack_stacks_are_source_attributed() {
    let plan = prepared(&[swan(true, 6, &[]), kafka()], &[120]);
    let mut battle = Battle::start(&plan, LogMode::Off);
    // Novaflare E2 applies 30 stacks. E6 doubles those and then stacks on a teammate attack.
    assert_eq!(stacks(&battle, 2), 60);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
    assert_eq!(stacks(&battle, 2), 62);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.amount, 750);
    assert_eq!(hit.assists, [(0, 150)]);
    battle.units[0].hp = 0;
    battle.detonate_all_dots(1, 2, 10_000, Action::Skill);
    assert_eq!(stacks(&battle, 2), 62);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.amount, 6_248);
    assert!(hit.assists.is_empty());
}

#[test]
fn e4_original_epiphany_shares_one_energy_trigger_and_enhanced_grants_unconditional_vulnerability()
{
    for enhanced in [false, true] {
        let mut input = swan(enhanced, 4, &[]);
        input.stats.effect_hit_rate_bp = 90_000;
        let mut plan = prepared(&[input, kafka()], &[120]);
        plan.units[2].stats.effect_resistance_bp = 10_000;
        let mut battle = Battle::start(&plan, LogMode::Off);
        // Reset the E2 state so no natural DoT roll can affect the fixture.
        battle.units[2].black_swan.arcana = None;
        battle.set_energy(0, 0, 120_000);
        act(&mut battle, SWAN, Action::Ultimate, None);
        assert_eq!(
            battle.dummy_damage.last().unwrap().amount,
            if enhanced { 1_044 } else { 720 }
        );
        assert_eq!(battle.roll_effect(0, 2, 10_000), !enhanced);
        enemy_turn(&mut battle, 2);
        assert_eq!(battle.units[0].energy_milli, 13_000);
        enemy_turn(&mut battle, 2);
        assert_eq!(
            battle.units[0].energy_milli,
            if enhanced { 21_000 } else { 13_000 }
        );
        assert!(!battle.roll_effect(0, 2, 10_000));
    }
}

#[test]
fn base_e2_spreads_to_living_neighbors_on_defeat() {
    const ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut plan = prepared(&[swan(false, 2, &[]), kafka()], &[120; 3]);
    plan.units[3].kit = Kit::Enemy(&ENEMY);
    plan.units[3].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Off);
    seed_arcana(&mut battle, 3, 1, false);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.hp(3).0, 0);
    assert_eq!(stacks(&battle, 2), 6);
    assert_eq!(stacks(&battle, 4), 6);
}

#[test]
fn candle_gives_original_personal_damage_and_attributes_enhanced_team_damage() {
    for enhanced in [false, true] {
        let trace = if enhanced {
            NOVAFLARE.candle
        } else {
            BASE.candle
        };
        let mut input = swan(enhanced, 0, &[trace]);
        input.stats.effect_hit_rate_bp = 20_000;
        let plan = prepared(&[input, kafka()], &[120]);
        let mut battle = Battle::start(&plan, LogMode::Off);
        act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
        let ally = &battle.dummy_damage[0];
        assert_eq!(ally.amount, if enhanced { 1_032 } else { 600 });
        assert_eq!(ally.assists, if enhanced { vec![(0, 432)] } else { vec![] });

        battle.units[1].kafka_follow_up_charges = 0;
        act(&mut battle, SWAN, Action::Basic, Some("dpr:1"));
        assert_eq!(
            battle.dummy_damage[1].amount,
            if enhanced { 1_032 } else { 619 }
        );
    }
}

#[test]
fn effective_e3_and_e5_level_values_flow_through_actions_and_arcana() {
    let mut input = swan(false, 5, &[]);
    input
        .skill_parameters
        .insert(BASE.basic.into(), vec![0.66, 0.68, 0.68]);
    input
        .skill_parameters
        .insert(BASE.skill.into(), vec![0.99, 1.0, 1.0, 0.22, 3.0]);
    input
        .skill_parameters
        .insert(BASE.ultimate.into(), vec![1.296, 2.0, 0.27, 1.0]);
    input.skill_parameters.insert(
        BASE.talent.into(),
        vec![2.64, 0.68, 0.132, 3.0, 1.98, 7.0, 0.20, 50.0],
    );
    let plan = prepared(&[input, kafka()], &[120]);

    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
    battle.units[1].kafka_follow_up_charges = 0;
    act(&mut battle, SWAN, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.dummy_damage[1].amount, 396);
    battle.set_energy(0, 0, 120_000);
    act(&mut battle, SWAN, Action::Ultimate, None);
    assert_eq!(battle.dummy_damage[2].amount, 972);
    enemy_turn(&mut battle, 2);
    // First stack of Lv.12 talent: 264% * 0.9 / 1.5 * 1.27 * 1.25 (E1).
    assert_eq!(dot_damage(&battle), [(0, 2, 2_514)]);

    let mut battle = Battle::start(&plan, LogMode::Off);
    act(&mut battle, KAFKA, Action::Basic, Some("dpr:1"));
    act(&mut battle, SWAN, Action::Skill, Some("dpr:1"));
    assert_eq!(battle.dummy_damage[1].amount, 594);
    assert!(
        battle.units[2]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::DefenseDown && debuff.bp == 2_200)
    );
}

#[test]
fn fixed_dpr_replays_novaflare_and_preserves_arcana_and_candle_attribution() {
    let toughness = &STANDARD_DPR_SCENARIO.toughness;
    let mut input = swan(
        true,
        0,
        &[NOVAFLARE.viscera, NOVAFLARE.goblet, NOVAFLARE.candle],
    );
    input.stats.effect_hit_rate_bp = 12_000;
    let plan = prepared(&[input, kafka()], toughness);
    let result = calculate_dpr(&plan, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, toughness, STANDARD_DPR_SCENARIO.cycles).unwrap()
    );
    let swan_credit = result
        .characters
        .iter()
        .find(|unit| unit.unit_id == SWAN)
        .unwrap();
    assert!(swan_credit.assist_damage > 0);
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| { hit.source_id == SWAN && hit.kind == DamageKind::DamageOverTime })
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>(),
    );
}
