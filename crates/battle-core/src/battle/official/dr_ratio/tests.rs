use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{
        Kit,
        combat_status::DebuffKind,
        periodic::{DotScale, SkillDot},
    },
    calculate_dpr,
    fixture::{self, Element},
};

use super::{BASIC, DEDUCTION, FOLLY, FORM, INFERENCE, SKILL, SUMMATION, TALENT, ULTIMATE};

const RATIO: &str = "ally.dr_ratio";
const YANQING: &str = "ally.yanqing";
const JIAOQIU: &str = "ally.jiaoqiu";

fn dr_ratio(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: RATIO.into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Hunt,
            element: crate::Element::Imaginary,
        },
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 400_000_000,
            speed_micros: 400_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.75]),
            (ULTIMATE.into(), vec![1.44, 2.0]),
            (TALENT.into(), vec![1.35, 0.4, 0.2]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn yanqing() -> OfficialCharacter {
    let mut ally = dr_ratio(0, &[]);
    ally.unit_id = YANQING.into();
    ally.form_id = "character.1209".into();
    ally.combat_type.element = crate::Element::Ice;
    ally.stats.base_speed_micros = 500_000_000;
    ally.stats.speed_micros = 500_000_000;
    ally.skill_parameters = [
        ("skill.120901".into(), vec![0.5]),
        ("skill.120902".into(), vec![1.1]),
        ("skill.120903".into(), vec![0.6, 0.3, 2.1]),
        (
            "skill.120904".into(),
            vec![0.0, 0.15, 0.0, 0.25, 0.25, 0.65],
        ),
    ]
    .into();
    ally
}

fn jiaoqiu() -> OfficialCharacter {
    let mut ally = dr_ratio(0, &[]);
    ally.unit_id = JIAOQIU.into();
    ally.form_id = "character.1218".into();
    ally.combat_type = crate::CombatType {
        path: crate::CombatPath::Nihility,
        element: crate::Element::Fire,
    };
    ally.energy_capacity = 100;
    ally.stats.base_speed_micros = 500_000_000;
    ally.stats.speed_micros = 500_000_000;
    ally.skill_parameters = [
        ("skill.121801".into(), vec![0.5]),
        ("skill.121802".into(), vec![0.75, 0.45, 1.0]),
        ("skill.121803".into(), vec![0.6, 0.5, 0.09, 3.0, 6.0]),
        (
            "skill.121804".into(),
            vec![1.0, 0.075, 0.025, 5.0, 2.0, 0.9],
        ),
    ]
    .into();
    ally
}

fn prepare(team: &[OfficialCharacter], seed: u32) -> PreparedBattle {
    PreparedBattle::official_dpr(team, seed, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    let cmd = battle
        .legal_actions()
        .into_iter()
        .find(|cmd| {
            cmd.actor_id == actor
                && cmd.action == action
                && cmd.target_id.as_deref() == Some(target)
        })
        .unwrap();
    battle.apply(&cmd).unwrap();
}

fn apply_debuffs(battle: &mut Battle<'_>, target: usize, count: usize) {
    let kinds = [
        DebuffKind::AttackDown,
        DebuffKind::DotVulnerability,
        DebuffKind::EffectResistanceDown,
        DebuffKind::ResistanceDown(Element::Fire),
        DebuffKind::ResistanceDown(Element::Ice),
        DebuffKind::ResistanceDown(Element::Thunder),
    ];
    for &kind in &kinds[..count] {
        assert!(battle.apply_debuff(0, target, kind, 1_000, 10));
    }
}

fn hits(battle: &Battle<'_>, actor: usize) -> Vec<(Action, DamageKind, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == actor)
        .map(|hit| (hit.action, hit.kind, hit.amount))
        .collect()
}

#[test]
fn normal_actions_use_locked_ratios_toughness_and_resources() {
    let plan = prepare(&[dr_ratio(0, &[])], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, RATIO, Action::Basic, "dpr:2");
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(battle.units[2].toughness, 990);
    act(&mut battle, RATIO, Action::Skill, "dpr:2");
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.units[2].toughness, 970);
    battle.fill_energy(0);
    act(&mut battle, RATIO, Action::Ultimate, "dpr:2");
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[2].toughness, 940);
    assert_eq!(
        hits(&battle, 0),
        [
            (Action::Basic, DamageKind::Direct, 300),
            (Action::Skill, DamageKind::Direct, 450),
            (Action::Ultimate, DamageKind::Direct, 864),
        ]
    );
    assert_eq!(battle.debuff_count(2), 0);
}

#[test]
fn talent_checks_fixed_chance_at_each_debuff_threshold() {
    // SplitMix64 first rolls are 3867 (seed 5), 4314 (4), 7415 (42) and 8833 (0).
    for (seed, count, succeeds) in [
        (5, 0, true),
        (4, 0, false),
        (4, 1, true),
        (42, 1, false),
        (42, 2, true),
        (0, 2, false),
        (0, 3, true),
    ] {
        let mut ratio = dr_ratio(0, &[]);
        ratio.stats.effect_hit_rate_bp = 10_000;
        let mut plan = prepare(&[ratio], seed);
        plan.units[2].stats.effect_resistance_bp = 10_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply_debuffs(&mut battle, 2, count);
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        let expected = (Action::Skill, DamageKind::FollowUp, 810);
        assert_eq!(
            hits(&battle, 0).contains(&expected),
            succeeds,
            "{seed}/{count}"
        );
        assert_eq!(
            battle.units[0].energy_milli,
            if succeeds { 105_000 } else { 100_000 }
        );
        assert_eq!(battle.units[2].toughness, if succeeds { 970 } else { 980 });
        assert_eq!(battle.snapshot().random_draws, u32::from(count < 3));
    }
}

#[test]
fn inference_uses_effect_chance_and_supplies_a_debuff_before_the_talent_roll() {
    for hit_rate in [0, 10_000] {
        let mut ratio = dr_ratio(0, &[INFERENCE]);
        ratio.stats.effect_hit_rate_bp = hit_rate;
        let mut plan = prepare(&[ratio], 12);
        plan.units[2].stats.effect_resistance_bp = 5_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply_debuffs(&mut battle, 2, 2);
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        let applies = hit_rate > 0;
        assert_eq!(battle.debuff_count(2), if applies { 3 } else { 2 });
        assert_eq!(hits(&battle, 0).len(), if applies { 2 } else { 1 });
        assert_eq!(battle.snapshot().random_draws, if applies { 0 } else { 2 });
        if applies {
            let events = battle.events();
            let application = events.iter().position(|event| {
                event.kind == EventKind::StatusApplied(StatusKind::EffectResistanceDown)
            });
            let follow_up = events
                .iter()
                .position(|event| event.kind == EventKind::FollowUp);
            assert!(application.unwrap() < follow_up.unwrap());
            battle.units[2].normal_turns = 1;
            battle.expire_debuffs(2);
            assert_eq!(battle.debuff_count(2), 3);
            battle.units[2].normal_turns = 2;
            battle.expire_debuffs(2);
            assert_eq!(battle.debuff_count(2), 2);
        }
    }
}

#[test]
fn debuffs_count_distinct_statuses_instead_of_stacks_or_roast_components() {
    let plan = prepare(&[dr_ratio(0, &[]), jiaoqiu()], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, JIAOQIU, Action::Skill, "dpr:2");
    let target = 3;
    assert_eq!(battle.vulnerability_from(1, target), 1_000);
    assert!(battle.has_elemental_dot(target, Element::Fire));
    assert_eq!(battle.debuff_count(target), 1);
    for _ in 0..2 {
        battle.apply_speed_down(0, target, 1_000, 2);
        battle.apply_skill_shock(0, target, Action::Basic, 10_000, 3_000, 2);
        assert!(battle.apply_skill_dot(
            0,
            target,
            10_000,
            SkillDot {
                element: Element::Wind,
                damage: DotScale::Attack(2_000),
                origin: Action::Skill,
                turns: 2,
                max_stacks: 5,
            },
        ));
    }
    assert_eq!(battle.dot_stacks(target, Element::Wind), 2);
    assert_eq!(battle.debuff_count(target), 4);
    battle.apply_break_dot(0, target, Element::Wind, Action::Skill);
    battle.apply_break_shock(0, target, Action::Skill);
    battle.apply_break_freeze(0, target, Action::Skill);
    battle.apply_entanglement(0, target, Action::Skill, 0);
    assert_eq!(battle.debuff_count(target), 8);
}

#[test]
fn summation_stacks_before_damage_persists_and_respects_eidolon_one() {
    for (eidolon, enabled, initial, after_skill, cap, first_skill_damage, later_basic_damage) in [
        (0, true, 0, 3, 6, 742, 540),
        (1, true, 4, 7, 10, 832, 600),
        (1, false, 0, 0, 0, 675, 450),
    ] {
        let traces = if enabled { &[SUMMATION][..] } else { &[] };
        let mut ratio = dr_ratio(eidolon, traces);
        ratio.stats.crit_rate_bp = 10_000;
        let plan = prepare(&[ratio], 42);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.units[0].dr_ratio_state.summation_stacks, initial);
        apply_debuffs(&mut battle, 2, 3);
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        assert_eq!(hits(&battle, 0)[0].2, first_skill_damage);
        assert_eq!(battle.units[0].dr_ratio_state.summation_stacks, after_skill);
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        assert_eq!(battle.units[0].dr_ratio_state.summation_stacks, cap);
        let mechanics = battle.mechanics(0).unwrap();
        assert_eq!(
            mechanics.crit_rate_bonus_bp(&battle, 0, 2, Some(Action::Skill), DamageKind::Direct),
            u32::from(cap) * 250,
        );
        act(&mut battle, RATIO, Action::Basic, "dpr:1");
        assert_eq!(hits(&battle, 0).last().unwrap().2, later_basic_damage);
    }
}

#[test]
fn deduction_e2_e4_e6_use_own_damage_types_limits_and_energy() {
    for (eidolon, count, direct, follow, extras, energy) in [
        (0, 2, 450, 810, vec![], 113_750),
        (0, 3, 585, 1_053, vec![], 113_750),
        (2, 4, 630, 1_134, vec![168; 4], 113_750),
        (4, 5, 675, 1_215, vec![180; 4], 132_500),
        (6, 6, 675, 1_620, vec![180; 4], 132_500),
    ] {
        let mut ratio = dr_ratio(eidolon, &[DEDUCTION]);
        ratio.stats.energy_regeneration_bp = 12_500;
        let plan = prepare(&[ratio], 42);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        apply_debuffs(&mut battle, 2, count);
        act(&mut battle, RATIO, Action::Skill, "dpr:2");
        let mut expected = vec![
            (Action::Skill, DamageKind::Direct, direct),
            (Action::Skill, DamageKind::FollowUp, follow),
        ];
        expected.extend(
            extras
                .into_iter()
                .map(|amount| (Action::Skill, DamageKind::Additional, amount)),
        );
        assert_eq!(hits(&battle, 0), expected);
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(battle.units[2].toughness, 970);
    }
}

#[test]
fn folly_replaces_its_target_and_counts_ally_attacks_instead_of_hits() {
    for (eidolon, max_triggers, final_energy) in [(0, 2, 35_000), (6, 3, 85_000)] {
        let plan = prepare(&[dr_ratio(eidolon, &[]), yanqing()], 42);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.fill_energy(0);
        act(&mut battle, RATIO, Action::Ultimate, "dpr:1");
        battle.fill_energy(0);
        act(&mut battle, RATIO, Action::Ultimate, "dpr:2");
        assert_eq!(battle.debuff_count(3), 0);
        act(&mut battle, YANQING, Action::Basic, "dpr:1");
        assert_eq!(
            battle.units[0].dr_ratio_state.mark.unwrap().remaining,
            max_triggers
        );
        act(&mut battle, RATIO, Action::Basic, "dpr:2");
        assert_eq!(
            battle.units[0].dr_ratio_state.mark.unwrap().remaining,
            max_triggers
        );
        for remaining in (1..max_triggers).rev() {
            act(&mut battle, YANQING, Action::Skill, "dpr:2");
            assert_eq!(
                battle.units[0].dr_ratio_state.mark.unwrap().remaining,
                remaining
            );
            act(&mut battle, RATIO, Action::Basic, "dpr:1");
        }
        act(&mut battle, YANQING, Action::Skill, "dpr:2");
        assert!(battle.units[0].dr_ratio_state.mark.is_none());
        let ratio_follows = hits(&battle, 0)
            .into_iter()
            .filter(|&(_, kind, _)| kind == DamageKind::FollowUp)
            .collect::<Vec<_>>();
        assert_eq!(ratio_follows.len(), usize::from(max_triggers));
        assert!(
            ratio_follows
                .iter()
                .all(|&(origin, _, _)| origin == Action::Skill)
        );
        assert_eq!(
            battle.units[0].energy_milli,
            final_energy + u32::from(max_triggers - 1) * 20_000
        );
        assert_eq!(
            battle
                .events()
                .iter()
                .filter(|event| event.kind == EventKind::StatusExpired(FOLLY))
                .map(|event| event.target_id.as_deref())
                .collect::<Vec<_>>(),
            [Some("dpr:1"), Some("dpr:2")]
        );
    }
}

#[test]
fn ally_follow_up_counts_as_an_attack_but_additional_damage_does_not() {
    let mut ally = yanqing();
    ally.skill_parameters.get_mut("skill.120904").unwrap()[2] = 1.0;
    ally.enabled_trace_ids.push("trace.1209101".into());
    let plan = prepare(&[dr_ratio(0, &[]), ally], 42);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, YANQING, Action::Skill, "dpr:2");
    act(&mut battle, RATIO, Action::Basic, "dpr:1");
    battle.fill_energy(0);
    act(&mut battle, RATIO, Action::Ultimate, "dpr:2");

    let start = battle.events().len();
    act(&mut battle, YANQING, Action::Basic, "dpr:2");
    let follows = battle.events()[start..]
        .iter()
        .filter(|event| event.kind == EventKind::FollowUp)
        .map(|event| event.source_id.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(follows, [Some(YANQING), Some(RATIO), Some(RATIO)]);
    let yanqing_hits = hits(&battle, 1);
    assert_eq!(
        yanqing_hits
            .iter()
            .filter(|&&(origin, kind, _)| origin == Action::Basic && kind == DamageKind::Additional)
            .count(),
        2
    );
    assert!(battle.units[0].dr_ratio_state.mark.is_none());
}

#[test]
fn skill_follows_up_on_random_survivor_after_defeat_and_mark_does_not_transfer() {
    let mut plan = prepare(&[dr_ratio(0, &[]), yanqing()], 42);
    plan.units[2].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
    plan.units[2].stats.hp = 301;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, YANQING, Action::Basic, "dpr:3");
    apply_debuffs(&mut battle, 2, 3);
    act(&mut battle, RATIO, Action::Skill, "dpr:1");
    assert_eq!(battle.hp(2).0, 0);
    assert_eq!(battle.snapshot().random_draws, 1);
    assert_eq!(battle.units[0].energy_milli, 115_000);
    assert_eq!(battle.dummy_damage.last().unwrap().target, 4);
    assert_eq!(
        battle.dummy_damage.last().unwrap().kind,
        DamageKind::FollowUp
    );

    plan.units[2].stats.hp = 1_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    act(&mut battle, RATIO, Action::Ultimate, "dpr:1");
    assert!(battle.units[0].dr_ratio_state.mark.is_some());
    act(&mut battle, YANQING, Action::Basic, "dpr:1");
    assert_eq!(battle.hp(2).0, 0);
    assert!(battle.units[0].dr_ratio_state.mark.is_none());
    assert!(
        !battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::FollowUp)
    );
}

#[test]
fn replay_and_dpr_preserve_follow_up_source_origin_and_attribution() {
    let plan = prepare(&[dr_ratio(6, &[INFERENCE]), jiaoqiu()], 42);
    let mut logged = Battle::start(&plan, LogMode::Detailed);
    let mut quiet = Battle::start(&plan, LogMode::Off);
    for battle in [&mut logged, &mut quiet] {
        battle.fill_energy(0);
        act(battle, RATIO, Action::Ultimate, "dpr:2");
        act(battle, JIAOQIU, Action::Skill, "dpr:2");
        act(battle, RATIO, Action::Skill, "dpr:2");
    }
    assert_eq!(logged.snapshot(), quiet.snapshot());
    assert_eq!(hits(&logged, 0), hits(&quiet, 0));
    assert!(quiet.events().is_empty());
    let hit = logged
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 0 && hit.kind == DamageKind::FollowUp)
        .unwrap();
    assert_eq!(hit.action, Action::Skill);
    assert_eq!(hit.target, 3);
    assert_eq!(hit.amount, 1_336);
    assert_eq!(hit.assists, [(1, 121)]);

    let run = || calculate_dpr(&plan, &[1_000; 3], STANDARD_DPR_SCENARIO.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits.clone().any(|hit| {
        hit.kind == DamageKind::FollowUp && hit.source_id == RATIO && hit.credits.len() > 1
    }));
    for hit in hits {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>(),
    );
}
