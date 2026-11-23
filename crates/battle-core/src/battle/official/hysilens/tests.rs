use crate::{
    CombatType, Command, DamageKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    calculate_dpr,
};

use super::*;

const HERTA: &str = "character.1013";

fn hysilens(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: PHYSICAL,
        },
        level: 80,
        energy_capacity: ENERGY_CAPACITY,
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
            element_damage_bp: [0; Element::COUNT],
            elation_bp: 0,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0, 1.0, 3.0]),
            (SKILL.into(), vec![1.4, 1.0, 0.2, 3.0]),
            (ULTIMATE.into(), vec![2.0, 3.0, 0.25, 0.8, 8.0, 0.15]),
            (TALENT.into(), vec![1.0, 0.25, 0.2, 0.25, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn herta() -> OfficialCharacter {
    let mut friend = hysilens(0, &[]);
    friend.form_id = HERTA.into();
    friend.unit_id = HERTA.into();
    friend.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    friend.stats.speed_micros = 120_000_000;
    friend.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    friend
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000, 1_000, 1_000])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn dot_hit(battle: &mut Battle<'_>, source: usize, target: usize) {
    battle.periodic_hit(
        source,
        target,
        PeriodicHit {
            element: Element::Fire,
            damage: DotScale::Attack(10_000),
            stacks: 1,
            ratio_bp: 10_000,
            origin: Action::Skill,
        },
    );
}

#[test]
fn validates_frozen_base_and_supports_level_one_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut low = hysilens(0, &[GLADIUS, BANQUET, FIDDLE]);
    low.skill_parameters
        .insert(BASIC.into(), vec![0.5, 1.0, 3.0]);
    low.skill_parameters
        .insert(SKILL.into(), vec![0.7, 1.0, 0.1, 3.0]);
    low.skill_parameters
        .insert(ULTIMATE.into(), vec![1.2, 3.0, 0.15, 0.32, 8.0, 0.15]);
    low.skill_parameters
        .insert(TALENT.into(), vec![1.0, 0.1, 0.2, 0.1, 2.0]);
    let prepared = plan(std::slice::from_ref(&low));
    assert!(
        calculate_dpr(&prepared, &[1_000; 3], STANDARD_DPR_SCENARIO.cycles)
            .unwrap()
            .total_damage
            > 0
    );
    low.skill_parameters.get_mut(ULTIMATE).unwrap()[4] = 7.0;
    assert!(PreparedBattle::official_dpr(&[low], 42, 3).is_err());
}

#[test]
fn entry_and_ultimate_zones_grant_sp_and_expire_at_owner_turn_start() {
    let prepared = plan(&[hysilens(4, &[GLADIUS])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.skill_points, 4);
    assert!(zone_active(&battle, 0));
    let rules = battle.mechanics(0).unwrap();
    let aura = rules.damage_target_aura(&battle, 0, 0);
    assert_eq!(
        (aura.defense_down_bp, aura.resistance_down_bp),
        (2_500, 2_000)
    );
    assert_eq!(rules.attack_reduction_bp(&battle, 0, 1), 1_500);
    battle.fill_energy(0);
    act(&mut battle, Action::Ultimate, None);
    assert_eq!(battle.skill_points, 5);
    assert!(zone_active(&battle, 0));

    battle.units[0].normal_turns = 4;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert!(!zone_active(&battle, 0));
    assert!(rules.damage_target_aura(&battle, 0, 0) == DamageTargetAura::default());
}

#[test]
fn skill_applies_vulnerability_before_damage_and_only_one_talent_status_per_target() {
    let prepared = plan(&[hysilens(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    for target in battle.enemies() {
        assert_eq!(battle.units[target].toughness, 990);
        assert_eq!(battle.standalone_skill_dot_count(target), 1);
    }
    let damage = battle
        .dummy_damage
        .iter()
        .map(|hit| hit.amount)
        .collect::<Vec<_>>();
    assert_eq!(damage, [1_008; 3]);
}

#[test]
fn an_attack_cannot_repeat_the_talent_and_e1_fills_two_independent_sets() {
    let prepared = plan(&[hysilens(1, &[]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let target = 2;
    battle.action = Some(Action::Basic);
    for expected in [2, 4, 6, 8, 8] {
        battle.begin_attack();
        for _ in 0..3 {
            battle.attack_hit(1, target, Element::Ice, 100, 0);
        }
        assert_eq!(battle.standalone_skill_dot_count(target), expected);
    }
    for element in TALENT_ELEMENTS {
        assert!(battle.has_owned_skill_dot(0, target, element, false));
        assert!(battle.has_owned_skill_dot(0, target, element, true));
    }
    battle.dummy_damage.clear();
    for expected in [8, 0] {
        battle.tick_skill_dots(target);
        assert_eq!(battle.standalone_skill_dot_count(target), expected);
    }
    assert_eq!(battle.dummy_damage.len(), 16);
    assert!(battle.dummy_damage.iter().all(|hit| {
        hit.actor == 0 && hit.kind == DamageKind::DamageOverTime && hit.amount == 174
    }));
}

#[test]
fn ultimate_triggers_existing_statuses_at_one_and_a_half_ratio_before_zone_echoes() {
    let prepared = plan(&[hysilens(0, &[BANQUET]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let target = 2;
    assert!(battle.apply_skill_dot(
        1,
        target,
        10_000,
        SkillDot {
            element: Element::Fire,
            damage: DotScale::Attack(10_000),
            origin: Action::Skill,
            turns: 2,
            max_stacks: 1,
        },
    ));
    battle.fill_energy(0);
    act(&mut battle, Action::Ultimate, None);
    let sequence = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.target == target)
        .map(|hit| (hit.actor, hit.kind, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(
        sequence,
        [
            (0, DamageKind::Direct, 1_309),
            (1, DamageKind::DamageOverTime, 981),
            (0, DamageKind::DamageOverTime, 245),
            (0, DamageKind::DamageOverTime, 523),
            (0, DamageKind::DamageOverTime, 523),
        ]
    );
    assert_eq!(battle.standalone_skill_dot_count(target), 2);
}

#[test]
fn zone_uses_a_fresh_nonrecursive_cap_per_enemy_and_batch() {
    for (eidolon, limit, expected_hit) in [(0, 8, 523), (6, 12, 911)] {
        let prepared = plan(&[hysilens(eidolon, &[GLADIUS]), herta()]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        for (timing, expected_origin) in [
            (DotTiming::Detonation, Action::Skill),
            (DotTiming::TurnStart(2), Action::Ultimate),
        ] {
            battle.dummy_damage.clear();
            battle.dot_timing = timing;
            for target in [2, 3] {
                for _ in 0..14 {
                    dot_hit(&mut battle, 1, target);
                }
            }
            battle.finish_dot_batch(timing);
            assert!(!battle.pending_dot_batch);
            for target in [2, 3] {
                let echoes = battle
                    .dummy_damage
                    .iter()
                    .filter(|hit| hit.target == target && hit.actor == 0)
                    .collect::<Vec<_>>();
                assert_eq!(echoes.len(), limit);
                assert!(
                    echoes
                        .iter()
                        .all(|hit| hit.amount == expected_hit && hit.action == expected_origin),
                    "{:?}",
                    echoes.iter().map(|hit| hit.amount).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn e1_multiplier_has_separate_attribution_and_e2_shares_hit_rate_conversion_in_zone() {
    let mut character = hysilens(2, &[GLADIUS, FIDDLE]);
    character.stats.effect_hit_rate_bp = 8_750;
    let prepared = plan(&[character, herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 1, PHYSICAL), 3_000);
    rules.on_defeated(&mut battle, 0);
    assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 1, PHYSICAL), 0);
    assert_eq!(rules.ally_damage_bonus_bp(&battle, 0, 0, PHYSICAL), 3_000);
    dot_hit(&mut battle, 1, 2);
    let hit = &battle.dummy_damage[0];
    assert_eq!(hit.amount, 696);
    assert_eq!(hit.assists, [(0, 96)]);
    battle.confirm_defeat(2, 0);
    dot_hit(&mut battle, 1, 2);
    assert_eq!(battle.dummy_damage[1].amount, 600);
}

#[test]
fn fixed_dpr_replays_source_and_support_credits_without_changing_the_total() {
    let prepared = plan(&[hysilens(6, &[GLADIUS, BANQUET, FIDDLE]), herta()]);
    let run = || calculate_dpr(&prepared, &[1_000; 3], STANDARD_DPR_SCENARIO.cycles).unwrap();
    let report = run();
    assert_eq!(report, run());
    let hits = || report.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.kind == DamageKind::DamageOverTime));
    assert!(hits().any(
        |hit| hit.source_id == HERTA && hit.credits.iter().any(|credit| credit.unit_id == FORM)
    ));
    assert_eq!(
        report.total_damage,
        report
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>(),
    );
}
