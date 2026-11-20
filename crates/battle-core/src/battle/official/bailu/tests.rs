use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, battle::Kit, calculate_dpr, fixture,
};

use super::{BASIC, OfficialCharacter, SKILL, TALENT, ULTIMATE};

fn bailu(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.bailu".into(),
        form_id: "character.1211".into(),
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 1_300,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: Some(0),
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.078, 78.0, 0.15, 2.0]),
            (ULTIMATE.into(), vec![0.09, 90.0, 2.0]),
            (TALENT.into(), vec![0.036, 36.0, 0.12, 120.0, 2.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn natasha() -> OfficialCharacter {
    let mut ally = bailu(0, &[]);
    ally.unit_id = "ally.natasha".into();
    ally.form_id = "natasha.abundance".into();
    ally.energy_capacity = 90;
    ally.stats.attack = 1_500;
    ally.stats.base_speed_micros = 95_000_000;
    ally.stats.speed_micros = 95_000_000;
    ally.skill_parameters = [
        ("skill.110501".into(), vec![0.5]),
        ("skill.110502".into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
        ("skill.110503".into(), vec![0.092, 92.0]),
        ("skill.110504".into(), vec![0.30, 0.25]),
    ]
    .into();
    ally
}

fn gepard() -> OfficialCharacter {
    let mut ally = natasha();
    ally.unit_id = "ally.gepard".into();
    ally.form_id = "character.1104".into();
    ally.energy_capacity = 100;
    ally.skill_parameters = [
        ("skill.110401".into(), vec![0.5]),
        ("skill.110402".into(), vec![1.0, 0.65, 1.0, 0.3]),
        ("skill.110403".into(), vec![0.3, 3.0, 150.0]),
        ("skill.110404".into(), vec![0.25]),
    ]
    .into();
    ally
}

fn finite_enemy(
    plan: &mut PreparedBattle,
    target: usize,
    attack: u32,
) -> &'static fixture::EnemyKit {
    let kit = fixture::enemy("fixture.target_left", 32).unwrap();
    plan.units[target].kit = Kit::Enemy(kit);
    plan.units[target].stats = kit.stats;
    plan.units[target].stats.attack = attack;
    plan.units[target].stats.hp = 10_000;
    kit
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .unwrap();
    battle.apply(&command).unwrap();
}

fn heal_log<'a>(battle: &'a Battle<'_>) -> Vec<(&'a str, u32)> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Heal { amount } if event.source_id.as_deref() == Some("ally.bailu") => {
                Some((event.target_id.as_deref().unwrap(), amount))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn skill_heals_initial_target_and_two_seeded_living_targets_at_decaying_amounts() {
    let mut caster = bailu(0, &[]);
    caster.stats.healing_bp = 2_000;
    let plan = PreparedBattle::official_dpr(&[caster, natasha(), gepard()], 42, 1).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 500;
    battle.units[1].hp = 500;
    battle.units[2].hp = 0;
    let mut repeated = battle.clone();

    for session in [&mut battle, &mut repeated] {
        act(session, Action::Skill, Some("ally.natasha"));
    }
    // (2,000×7.8%+78)×1.2, then ×0.85 and ×0.85², truncated per heal.
    assert_eq!(
        heal_log(&battle),
        [
            ("ally.natasha", 280),
            ("ally.natasha", 238),
            ("ally.bailu", 202)
        ]
    );
    assert_eq!(battle.snapshot(), repeated.snapshot());
    assert_eq!(battle.events(), repeated.events());
    assert_eq!(battle.snapshot().random_draws, 2);
    assert_eq!(battle.snapshot().skill_points, 0);
    assert_eq!(battle.snapshot().units[0].energy_milli, 80_000);
}

#[test]
fn overheal_raises_current_and_base_scaled_max_hp_and_expires_at_turn_end() {
    let plan =
        PreparedBattle::official_dpr(&[bailu(0, &["trace.1211101"]), natasha()], 42, 0).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 100_000;
    battle.units[1].hp = 1_719;
    act(&mut battle, Action::Ultimate, None);

    // Bailu's own overheal adds 10% of 1,300 base HP before the next ally is healed.
    // Natasha needed exactly floor(2,130×9%+90) = 281 and receives no Max HP buff.
    assert_eq!(
        heal_log(&battle),
        [("ally.bailu", 0), ("ally.natasha", 281)]
    );
    assert_eq!(battle.hp(0), (2_130, 2_130));
    assert_eq!(battle.hp(1), (2_000, 2_000));
    assert_eq!(battle.snapshot().units[0].max_hp, 2_130);
    assert_eq!(battle.snapshot().units[0].energy_milli, 5_000);
    while battle.units[0].finished_turns < 3 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(battle.hp(0), (2_000, 2_000));
}

#[test]
fn invigoration_mitigates_and_heals_three_hits_including_a_fully_shielded_hit() {
    let mut ally = natasha();
    ally.stats.energy_regeneration_bp = 15_000;
    let traces = ["trace.1211102", "trace.1211103"];
    let mut plan = PreparedBattle::official_dpr(&[ally, bailu(1, &traces)], 42, 0).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 200);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 1_600;
    battle.units[0].energy_milli = 0;
    battle.units[1].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    battle.apply_official_shield(1, 0, 91, 1, Some(Action::Ultimate));

    for _ in 0..3 {
        battle.resolve_enemy(2, enemy);
    }
    assert_eq!(battle.hp(0), (2_000, 2_000));
    // Three 10-point enemy hits × 150% ERR, then a fixed 8 energy on depletion.
    assert_eq!(battle.snapshot().units[0].energy_milli, 53_000);
    assert!(battle.units[0].invigoration.is_none());
    battle.resolve_enemy(2, enemy);

    let hits: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                amount, absorbed, ..
            } => Some((amount, absorbed)),
            _ => None,
        })
        .collect();
    // 200×520/(520+500) with and without the 10% trace reduction.
    assert_eq!(hits, [(91, 91), (91, 0), (91, 0), (101, 0)]);
    assert_eq!(
        heal_log(&battle),
        [
            ("ally.natasha", 270),
            ("ally.bailu", 0),
            ("ally.natasha", 108),
            ("ally.natasha", 108),
            ("ally.natasha", 96)
        ]
    );
}

#[test]
fn repeated_ultimate_extends_invigoration_without_refilling_and_e2_boosts_later_heals() {
    let mut plan = PreparedBattle::official_dpr(&[natasha(), bailu(2, &[])], 42, 0).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 200);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 500;
    battle.units[1].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    battle.resolve_enemy(2, enemy);
    battle.units[1].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    let status = battle.units[0].invigoration.unwrap();
    assert_eq!(status.expires_after_turn, 3);
    assert_eq!(status.remaining_heals, 1);
    battle.resolve_enemy(2, enemy);
    assert!(battle.units[0].invigoration.is_none());
    // First ultimate 270, later heals with E2: floor(108×1.15)=124, floor(270×1.15)=310.
    assert_eq!(
        heal_log(&battle)
            .into_iter()
            .filter_map(|(target, amount)| (target == "ally.natasha").then_some(amount))
            .collect::<Vec<_>>(),
        [270, 124, 310, 124]
    );
}

#[test]
fn invigoration_expires_on_the_recipients_own_turn_and_refunds_only_at_full_hp() {
    let plan = PreparedBattle::official_dpr(&[natasha(), bailu(1, &[])], 42, 0).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 0;
    battle.units[1].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    battle.units[1].hp = 1_990;
    while battle.units[0].finished_turns < 2 || battle.units[1].finished_turns < 3 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert!(battle.units.iter().all(|unit| unit.invigoration.is_none()));
    assert_eq!(battle.snapshot().units[0].energy_milli, 48_000);
    let refunded: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Energy {
                before_milli,
                after_milli,
            } if after_milli.checked_sub(before_milli) == Some(8_000) => event.target_id.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(refunded, ["ally.natasha"]);
}

#[test]
fn lethal_hit_uses_self_survival_first_then_bailu_battle_limited_charges() {
    for (eidolon, expected_rescues) in [(0, 1), (6, 2)] {
        let mut plan =
            PreparedBattle::official_dpr(&[gepard(), bailu(eidolon, &[])], 42, 0).unwrap();
        let enemy = finite_enemy(&mut plan, 2, 2_000);
        let mut battle = Battle::start(&plan, LogMode::Detailed);

        battle.units[0].hp = 1;
        battle.resolve_enemy(2, enemy);
        assert_eq!(battle.hp(0).0, 500);
        for _ in 0..expected_rescues {
            battle.resolve_enemy(2, enemy);
            assert_eq!(battle.hp(0).0, 360);
            assert!(
                !battle
                    .events()
                    .iter()
                    .any(|event| event.kind == EventKind::UnitDefeated)
            );
        }
        battle.resolve_enemy(2, enemy);
        assert_eq!(battle.hp(0).0, 0);
        assert_eq!(
            battle.events().last().unwrap().kind,
            EventKind::UnitDefeated
        );
        assert_eq!(
            heal_log(&battle),
            vec![("ally.gepard", 360); expected_rescues]
        );

        battle.units[1].hp = 1;
        battle.resolve_enemy(2, enemy);
        assert_eq!(battle.hp(1).0, 0);
    }
}

#[test]
fn frozen_bailu_cannot_rescue_a_teammate() {
    let mut plan = PreparedBattle::official_dpr(&[natasha(), bailu(6, &[])], 42, 0).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 2_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 0, Action::Basic));
    battle.units[0].hp = 1;
    battle.resolve_enemy(2, enemy);

    assert_eq!(battle.hp(0).0, 0);
    assert_eq!(battle.units[1].bailu_state.revives_used, 0);
    assert!(heal_log(&battle).is_empty());
}

#[test]
fn a2_max_hp_increase_is_attributed_on_allied_hp_scaled_damage() {
    let mut ally = natasha();
    ally.eidolon = 6;
    let team = [bailu(0, &["trace.1211101"]), ally];
    let plan = PreparedBattle::official_dpr(&team, 42, 0).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 100_000;
    act(&mut battle, Action::Ultimate, None);
    while battle.ordinary_actor != Some(1) {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    act(&mut battle, Action::Basic, Some("dpr:2"));
    let hit = battle.dummy_damage.last().unwrap();

    // E6 scales by 40% of max HP, with 0.6 from defense and toughness.
    assert_eq!(hit.kind, DamageKind::Additional);
    assert_eq!(hit.amount, 511);
    assert_eq!(hit.assists, [(0, 31)]);
}

#[test]
fn e4_each_bounce_contributes_one_damage_stack_and_fixed_dpr_attributes_the_bonus() {
    let plan = PreparedBattle::official_dpr(&[bailu(4, &[]), natasha()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..2 {
        act(&mut battle, Action::Skill, Some("ally.natasha"));
    }
    act(&mut battle, Action::Basic, Some("dpr:2"));
    let hit = &battle.dummy_damage[0];
    // At 3 stacks: (1500×50%)×(1000/1500)×0.9×1.3 = 585, unbuffed 450.
    assert_eq!(hit.amount, 585);
    assert_eq!(hit.assists, [(0, 135)]);
    assert_eq!(battle.snapshot().random_draws, 4);
    while battle.units[1].finished_turns < 2 {
        act(&mut battle, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(battle.ally_buff_value(0, 1, super::AllyBuffKind::Damage), 0);

    let scenario = STANDARD_DPR_SCENARIO;
    let calculate = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let first = calculate();
    assert_eq!(first, calculate());
    assert!(first.characters[0].assist_damage > 0);
    assert!(first.targets[1].hits.iter().any(|hit| {
        hit.kind == DamageKind::Direct
            && hit.source_id == "ally.bailu"
            && hit.action == Action::Basic
    }));
    assert_eq!(
        first.total_damage,
        first
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
