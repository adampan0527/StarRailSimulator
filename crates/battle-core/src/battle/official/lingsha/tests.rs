use crate::{
    CombatType, Command, EventKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    StatusKind,
    battle::{Kit, analysis::DummyDamage, resolve::gauge_for, supports_official_dpr_variant},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const ALLY: &str = "character.1301";
const SUNDAY: &str = "character.1313";
const ACHERON: &str = "character.1308";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn lingsha(eidolon: u8, traces: &[&str], speed: u64) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Abundance,
            element: Element::Fire,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: speed * 1_000_000,
            speed_micros: speed * 1_000_000,
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
            (SKILL.into(), vec![0.8, 0.14, 420.0, 0.2]),
            (ULTIMATE.into(), vec![1.5, 0.12, 360.0, 0.25, 2.0, 1.0]),
            (
                TALENT.into(),
                vec![90.0, 0.75, 0.12, 360.0, 5.0, 1.0, 3.0, 0.75],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn gallagher() -> OfficialCharacter {
    let mut ally = lingsha(0, &[], 80);
    ally.form_id = ALLY.into();
    ally.unit_id = ALLY.into();
    ally.skill_parameters = [
        ("skill.130101".into(), vec![1.0]),
        ("skill.130102".into(), vec![1_600.0]),
        ("skill.130103".into(), vec![1.5, 2.0]),
        ("skill.130104".into(), vec![0.12, 640.0]),
        ("skill.130108".into(), vec![2.5, 0.15, 2.0]),
    ]
    .into();
    ally
}

fn sunday() -> OfficialCharacter {
    let mut ally = lingsha(1, &[], 200);
    ally.form_id = SUNDAY.into();
    ally.unit_id = SUNDAY.into();
    ally.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Imaginary,
    };
    ally.energy_capacity = 130;
    ally.skill_parameters = [
        ("skill.131301".into(), vec![1.0]),
        ("skill.131302".into(), vec![1.0, 0.30, 2.0, 0.50]),
        ("skill.131303".into(), vec![0.20, 0.30, 3.0, 0.12]),
        ("skill.131304".into(), vec![0.20, 3.0]),
    ]
    .into();
    ally
}

fn acheron() -> OfficialCharacter {
    let mut ally = lingsha(0, &[], 80);
    ally.form_id = ACHERON.into();
    ally.unit_id = ACHERON.into();
    ally.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Thunder,
    };
    ally.energy_capacity = 0;
    ally.skill_parameters = [
        ("skill.130801".into(), vec![0.5]),
        ("skill.130802".into(), vec![1.6, 0.6, 1.0]),
        (
            "skill.130803".into(),
            vec![0.24, 0.15, 1.2, 9.0, 0.6, 3.72, 3.0],
        ),
        ("skill.130804".into(), vec![9.0, 0.2]),
        ("skill.130814".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130815".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130816".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130817".into(), vec![1.2]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|cmd| {
            cmd.actor_id == actor && cmd.action == action && cmd.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
    command
}

fn summon(battle: &mut Battle<'_>) {
    battle
        .mechanics(0)
        .unwrap()
        .after_action(battle, 0, Action::Skill, None);
}

fn fuyuan_act(battle: &mut Battle<'_>) {
    battle.mechanics(0).unwrap().on_summon_turn(battle, 0);
}

fn follow_up_hits<'a>(battle: &'a Battle<'_>) -> impl Iterator<Item = &'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0 && hit.kind == DamageKind::FollowUp)
}

#[test]
fn lingsha_registers_base_and_validates_the_locked_summon_parameters() {
    let mut input = lingsha(0, &[], 100);
    assert!(supports_official_dpr_variant(FORM, "base"));
    assert!(!supports_official_dpr_variant(FORM, "novaflare"));
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(TALENT).unwrap()[0] = 91.0;
    assert!(matches!(build(&input), Err(BattleError::InvalidInput(_))));
}

#[test]
fn lingsha_basic_deals_fire_damage_and_the_trace_adds_ten_energy() {
    for (traces, expected_energy) in [(&[][..], 75_000), (&[SYLVAN_SMOKE][..], 85_000)] {
        let plan = plan(&[lingsha(0, traces, 200)], &[400]);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, FORM, Action::Basic, Some(LEFT));
        assert_eq!(battle.dummy_damage[0].amount, 600);
        assert_eq!(battle.units[1].toughness, 390);
        assert_eq!(battle.units[0].energy_milli, expected_energy);
        assert_eq!(battle.skill_points, 4);
        assert!(battle.units[0].summon_turn.is_none());
    }
}

#[test]
fn lingsha_first_skill_summons_and_advances_fuyuan_on_its_own_clock() {
    let plan = plan(&[lingsha(0, &[], 100)], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 1_000;
    let skill = act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.units[0].hp, 2_000);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 2);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().speed_micros,
        90_000_000
    );
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(follow_up_hits(&battle).count(), 4);
    assert!(follow_up_hits(&battle).all(|hit| hit.av_micros == 188_888_889 && hit.amount == 450));
    let replay = Battle::replay(&plan, &[skill], LogMode::Detailed).unwrap();
    let damage = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.av_micros, hit.target, hit.action, hit.amount))
            .collect::<Vec<_>>()
    };
    assert_eq!(damage(&replay), damage(&battle));
}

#[test]
fn lingsha_replenishment_preserves_progress_and_removes_the_exhausted_summon() {
    let plan = plan(&[lingsha(0, &[], 200)], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    summon(&mut battle);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 3);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        gauge_for(8_000)
    );
    summon(&mut battle);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 5);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        gauge_for(6_000)
    );
    for remaining in (0..5).rev() {
        fuyuan_act(&mut battle);
        assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, remaining);
    }
    assert!(battle.units[0].summon_turn.is_none());
    summon(&mut battle);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 3);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        gauge_for(8_000)
    );
}

#[test]
fn lingsha_ultimate_sets_befog_before_break_and_credits_the_cross_attack() {
    let plan = plan(&[lingsha(2, &[], 200), gallagher()], &[40, 400]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.set_energy(0, 0, 110_000);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert!(battle.units[0].summon_turn.is_none());
    assert_eq!(battle.effective_break_effect(0), 4_000);
    assert_eq!(battle.effective_break_effect(1), 4_000);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    let befog = battle.units[2]
        .debuffs
        .iter()
        .find(|status| status.kind == DebuffKind::Befog)
        .unwrap();
    assert_eq!(befog.bp, 2_500);
    assert_eq!(befog.remaining_turns(battle.units[2].normal_turns), Some(2));
    battle.action = Some(Action::Basic);
    battle.attack_hit(1, 2, Element::Fire, 10_000, 10);
    battle.action = None;
    let break_hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 1 && hit.kind == DamageKind::WeaknessBreak)
        .unwrap();
    assert_eq!(break_hit.target, 2);
    assert_eq!(break_hit.assists.len(), 1);
    assert_eq!(break_hit.assists[0].0, 0);
    assert!(break_hit.assists[0].1 > 0);
    assert_eq!(battle.dummy_damage[0].amount, 900);

    summon(&mut battle);
    battle.set_energy(0, 0, 110_000);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].summon_turn.unwrap().gauge_quanta, 0);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 3);
    let befog_events = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::StatusApplied(StatusKind::Befog))
        .count();
    assert_eq!(befog_events, 4);
}

#[test]
fn lingsha_befog_application_and_refresh_each_add_one_slashed_dream() {
    let plan = plan(&[lingsha(0, &[], 200), acheron()], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..2 {
        battle.set_energy(0, 0, 110_000);
        act(&mut battle, FORM, Action::Ultimate, None);
    }
    let thresholds: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::SlashedDream { before, after, .. }
                if event.source_id.as_deref() == Some(FORM)
                    && event.target_id.as_deref() == Some(ACHERON) =>
            {
                Some((before, after))
            }
            _ => None,
        })
        .collect();
    assert_eq!(thresholds, [(0, 1), (1, 2)]);
    assert!(battle.enemies().all(|target| {
        battle.units[target]
            .debuffs
            .iter()
            .filter(|debuff| debuff.kind == DebuffKind::Befog)
            .count()
            == 1
    }));
}

#[test]
fn lingsha_e6_extra_hits_prefer_live_fire_weakness_and_use_one_attack() {
    let plan = plan(&[lingsha(6, &[], 200)], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].toughness = 0;
    battle.units[2].toughness = 0;
    summon(&mut battle);
    fuyuan_act(&mut battle);
    let targets: Vec<_> = follow_up_hits(&battle).map(|hit| hit.target).collect();
    assert_eq!(targets, [1, 2, 3, 3, 3, 3, 3, 3]);
    // E1 multiplies 10 by 1.5, and each E6 hit reduces 7 after flooring 5 * 1.5.
    assert_eq!(battle.units[3].toughness, 342);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::FollowUp)
            .count(),
        1
    );
    assert_eq!(battle.units[0].energy_milli, 55_000);
}

#[test]
fn lingsha_fuyuan_cleanses_each_living_ally_and_e4_heals_the_lowest_ratio() {
    let mut ally = gallagher();
    ally.stats.hp = 4_000;
    ally.stats.base_hp = 4_000;
    let plan = plan(&[lingsha(4, &[], 200), ally], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    summon(&mut battle);
    battle.units[0].hp = 1_000;
    battle.units[1].hp = 1_600;
    battle.apply_debuff(2, 0, DebuffKind::Slow, 1_000, 2);
    battle.apply_debuff(2, 1, DebuffKind::AttackDown, 1_000, 2);
    battle.apply_debuff(2, 1, DebuffKind::DefenseDown, 1_000, 2);
    fuyuan_act(&mut battle);
    assert_eq!(battle.units[0].hp, 1_480);
    assert_eq!(battle.units[1].hp, 2_480);
    assert!(battle.units[0].debuffs.is_empty());
    assert_eq!(battle.units[1].debuffs.len(), 1);
}

#[test]
fn lingsha_emergency_heals_after_finite_enemy_damage_and_does_not_spend_an_action() {
    let mut plan = plan(&[lingsha(0, &[EMBER_ECHO], 200)], &[400; 3]);
    plan.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[1].stats = crate::fixture::Stats {
        level: 80,
        attack: 600,
        ..DUMMY_STATS
    };
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    summon(&mut battle);
    let gauge = battle.units[0].summon_turn.unwrap().gauge_quanta;
    battle.units[0].hp = 1_300;
    battle.resolve_enemy(1, &FINITE_ENEMY);
    let hit_count = follow_up_hits(&battle).count();
    assert!(hit_count > 0);
    assert_eq!(battle.units[0].hp, 1_380);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 3);
    assert_eq!(battle.units[0].summon_turn.unwrap().gauge_quanta, gauge);

    battle.units[0].finished_turns = 1;
    battle.spend_ally_hp(0, 0, 480, 1);
    battle.notify_ally_action_finished(0, Action::Skill);
    assert_eq!(follow_up_hits(&battle).count(), hit_count);
    battle.units[0].finished_turns = 2;
    battle.spend_ally_hp(0, 0, 1, 1);
    battle.notify_ally_action_finished(0, Action::Skill);
    assert!(follow_up_hits(&battle).count() > hit_count);
    assert_eq!(battle.units[0].hp, 1_379);
    assert_eq!(battle.units[0].lingsha_state.fuyuan_actions, 3);
}

#[test]
fn lingsha_break_effect_conversion_caps_and_is_attributed_to_its_source() {
    let plan = plan(&[lingsha(0, &[VERMILION_WAFT], 200), gallagher()], &[400]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_ally_buff(1, 0, AllyBuffKind::BreakEffect, 25_000, 3);
    assert_eq!(battle.effective_attack(0), 1_500);
    battle.units[0].hp = 500;
    act(&mut battle, FORM, Action::Skill, None);
    // 600 * 1.5 * 0.8 and floor((1500 * 0.14 + 420) * 1.20).
    assert_eq!(battle.dummy_damage[0].amount, 720);
    assert_eq!(battle.dummy_damage[0].assists, [(1, 240)]);
    assert_eq!(battle.units[0].hp, 1_256);
}

#[test]
fn lingsha_fixed_dpr_replays_and_conserves_follow_up_and_break_assists() {
    let plan = PreparedBattle::official_dpr(
        &[
            lingsha(6, &[VERMILION_WAFT, SYLVAN_SMOKE, EMBER_ECHO], 150),
            gallagher(),
        ],
        42,
        3,
    )
    .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    let mut damage = 0;
    let mut seen_follow_up = false;
    let mut seen_assist = false;
    for hit in result.targets.iter().flat_map(|target| &target.hits) {
        assert_eq!(
            u64::from(hit.amount),
            hit.credits
                .iter()
                .map(|credit| u64::from(credit.amount))
                .sum::<u64>()
        );
        damage += u64::from(hit.amount);
        seen_follow_up |= hit.source_id == FORM && hit.kind == DamageKind::FollowUp;
        seen_assist |=
            hit.source_id == ALLY && hit.credits.iter().any(|credit| credit.unit_id == FORM);
    }
    assert_eq!(result.total_damage, damage);
    assert!(seen_follow_up && seen_assist);
}

#[test]
fn lingsha_fuyuan_counts_for_sundays_summon_advance_and_defense_ignore() {
    let mut caster = lingsha(0, &[], 100);
    caster.stats.crit_damage_bp = 0;
    let plan = plan(&[caster, sunday()], &[400; 3]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    summon(&mut battle);
    let av = battle.av_micros;
    act(&mut battle, SUNDAY, Action::Skill, Some(FORM));
    assert_eq!(battle.ordinary_actor(), Some(FORM));
    assert_eq!(battle.units[0].summon_turn.unwrap().gauge_quanta, 0);
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    let hits: Vec<_> = follow_up_hits(&battle).collect();
    assert_eq!(hits.len(), 4);
    for hit in hits {
        // 1000 × 0.75 × 1.8 × 0.9 / (1 + 0.5 × 0.6).
        assert_eq!((hit.av_micros, hit.amount), (av, 934));
        assert_eq!(hit.assists, [(1, 484)]);
    }
}
