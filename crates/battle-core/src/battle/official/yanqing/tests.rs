use crate::{
    Action, Battle, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind, battle::Kit, calculate_dpr, fixture,
};

use super::{
    BASIC, FORM, FROST_FAVORS_THE_BRAVE, GENTLE_BLADE, ICING_ON_THE_KICK, SKILL, TALENT, ULTIMATE,
    soulsteel_active, ultimate_active,
};

fn yanqing(eidolon: u8, follow_up_chance: f64) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.yanqing".into(),
        form_id: FORM.into(),
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 400_000_000,
            speed_micros: 400_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![1.1]),
            (ULTIMATE.into(), vec![0.6, 0.3, 2.1]),
            (
                TALENT.into(),
                vec![0.0, 0.15, follow_up_chance, 0.25, 0.25, 0.65],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut character = yanqing(0, 0.0);
    character.unit_id = "ally.arlan".into();
    character.form_id = "character.1008".into();
    character.energy_capacity = 110;
    character.stats.base_speed_micros = 300_000_000;
    character.stats.speed_micros = 300_000_000;
    character.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    character
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|cmd| {
            cmd.actor_id == actor
                && cmd.action == action
                && cmd.target_id.as_deref() == Some(target)
        })
        .expect("expected a legal action at this decision");
    battle.apply(&command).unwrap();
}

fn act_with_yanqing(battle: &mut Battle<'_>, action: Action, target: &str) {
    act(battle, "ally.yanqing", action, target);
}

#[test]
fn skill_grants_soulsteel_after_the_opening_attack_and_energy() {
    let plan = PreparedBattle::official_dpr(&[yanqing(2, 1.0)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act_with_yanqing(&mut battle, Action::Skill, "dpr:1");
    assert!(soulsteel_active(&battle, 0));
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert_eq!(battle.units[1].toughness, 10);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        vec![(DamageKind::Direct, 165); 4]
    );

    let mut without_detail = Battle::start(&plan, LogMode::Off);
    act_with_yanqing(&mut without_detail, Action::Skill, "dpr:1");
    act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
    act_with_yanqing(&mut without_detail, Action::Basic, "dpr:2");
    assert_eq!(battle.snapshot(), without_detail.snapshot());
    assert!(without_detail.events().is_empty());
    assert!(!soulsteel_active(&battle, 0));
    assert_eq!(battle.units[0].energy_milli, 133_000);
    assert_eq!(battle.units[2].toughness, 100);
    assert_eq!(battle.units[3].toughness, 30);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(
        battle.dummy_damage[4..]
            .iter()
            .map(|hit| (hit.kind, hit.amount))
            .collect::<Vec<_>>(),
        vec![
            (DamageKind::Direct, 150),
            (DamageKind::Direct, 75),
            (DamageKind::Direct, 75),
            (DamageKind::FollowUp, 45),
            (DamageKind::FollowUp, 105),
        ]
    );
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::FollowUp)
            .count(),
        1
    );
}

#[test]
fn follow_up_only_freezes_its_target_and_resolves_freeze_damage_next_turn() {
    let mut character = yanqing(1, 1.0);
    character.stats.effect_hit_rate_bp = 10_000;
    let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act_with_yanqing(&mut battle, Action::Skill, "dpr:1");
    act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
    assert!(!battle.is_frozen(1));
    assert!(battle.is_frozen(2));
    assert!(!battle.is_frozen(3));
    assert!(
        !battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::Additional)
    );

    act_with_yanqing(&mut battle, Action::Basic, "dpr:3");
    act_with_yanqing(&mut battle, Action::Basic, "dpr:1");
    act_with_yanqing(&mut battle, Action::Basic, "dpr:3");
    assert!(!battle.is_frozen(2));
    let hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.target == 2 && hit.kind == DamageKind::Additional)
        .expect("expected a frozen-target tick");
    assert_eq!(hit.action, Action::Basic);
    assert_eq!(hit.amount, 150);
}

#[test]
fn soulsteel_adds_crit_rate_and_damage_only_after_the_granting_skill() {
    let mut character = yanqing(0, 0.0);
    character.skill_parameters.get_mut(TALENT).unwrap()[0] = 1.0;
    let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act_with_yanqing(&mut battle, Action::Skill, "dpr:1");
    assert!(battle.dummy_damage.iter().all(|hit| !hit.critical));
    act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
    let hits = &battle.dummy_damage[4..];
    assert!(hits.iter().all(|hit| hit.critical));
    assert_eq!(
        hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [247, 123, 123]
    );
}

#[test]
fn ultimate_samples_soulsteel_and_grants_buffs_before_its_hit() {
    // Basic hits floor separately at 225 + 112 + 112 and 292 + 146 + 146.
    for (has_soulsteel, expected_ultimate, expected_next_basic) in
        [(false, 1_890, 449), (true, 2_457, 584)]
    {
        let mut character = yanqing(0, 0.0);
        character.stats.crit_rate_bp = 4_000;
        let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if has_soulsteel {
            act_with_yanqing(&mut battle, Action::Skill, "dpr:1");
        }
        battle.units[0].energy_milli = 140_000;
        act_with_yanqing(&mut battle, Action::Ultimate, "dpr:2");
        let ultimate = battle.dummy_damage.last().unwrap();
        assert!(ultimate.critical);
        assert_eq!(ultimate.amount, expected_ultimate);
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.units[2].toughness, 90);
        assert!(ultimate_active(&battle, 0));

        let hits_before = battle.dummy_damage.len();
        act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
        let subsequent_damage = battle.dummy_damage[hits_before..]
            .iter()
            .map(|hit| hit.amount)
            .sum::<u32>();
        assert_eq!(subsequent_damage, expected_next_basic);
        assert!(!ultimate_active(&battle, 0));
        assert!(!soulsteel_active(&battle, 0));
    }
}

#[test]
fn damage_losing_hp_removes_soulsteel_and_shield_blocks_removal() {
    let mut character = yanqing(0, 0.0);
    character
        .enabled_trace_ids
        .push(FROST_FAVORS_THE_BRAVE.into());
    let enemy = fixture::enemy("fixture.target_left", 32).unwrap();
    let mut plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
    plan.units[1].kit = Kit::Enemy(enemy);
    plan.units[1].stats.attack = 500;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act_with_yanqing(&mut battle, Action::Skill, "dpr:2");
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.effect_resistance_bonus_bp(&battle, 0), 2_000);

    battle.spend_hp(0, 10, 1);
    battle.apply_official_shield(0, 0, 1_000, 3, None);
    battle.resolve_enemy(1, enemy);
    assert_eq!(battle.hp(0).0, 1_990);
    assert!(soulsteel_active(&battle, 0));
    assert!(battle.shield_amount(0) < 1_000);

    battle.apply_official_shield(0, 0, 1, 3, None);
    battle.resolve_enemy(1, enemy);
    assert!(battle.hp(0).0 < 1_990);
    assert!(!soulsteel_active(&battle, 0));
    assert_eq!(mechanics.effect_resistance_bonus_bp(&battle, 0), 0);
}

#[test]
fn a2_and_e1_are_separate_hits_and_e1_checks_before_the_attack() {
    for (starting_frozen, expected_extra) in [(false, vec![180]), (true, vec![180, 360])] {
        let mut character = yanqing(1, 0.0);
        character.enabled_trace_ids.push(ICING_ON_THE_KICK.into());
        let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if starting_frozen {
            battle.apply_skill_freeze(0, 2, 10_000, 1, 2_500, Action::Skill);
        }
        act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
        let extra = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>();
        assert_eq!(extra, expected_extra);
        assert_eq!(battle.units[2].toughness, 110);
    }

    let plan = PreparedBattle::official_dpr(&[yanqing(1, 0.0)], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].toughness = 10;
    act_with_yanqing(&mut battle, Action::Basic, "dpr:1");
    assert!(battle.is_frozen(1));
    assert!(
        !battle
            .dummy_damage
            .iter()
            .any(|hit| hit.kind == DamageKind::Additional),
        "the frost from this attack must not backdate E1"
    );
}

#[test]
fn a6_adds_speed_on_direct_and_nonprimary_crit() {
    for nonprimary in [false, true] {
        let mut character = yanqing(0, 0.0);
        character.enabled_trace_ids.push(GENTLE_BLADE.into());
        character.stats.crit_rate_bp = 10_000;
        let plan = PreparedBattle::official_dpr(&[character], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        if nonprimary {
            battle.secondary_hit(
                0,
                2,
                fixture::Element::Ice,
                3_000,
                DamageKind::Additional,
                Action::Basic,
            );
        } else {
            act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
        }
        assert_eq!(battle.snapshot().units[0].speed_micros, 440_000_000);
        assert!(battle.events().iter().any(|event| {
            event.kind == EventKind::StatusApplied(StatusKind::SpeedUp)
                && event.source_id.as_deref() == Some("ally.yanqing")
        }));
    }
}

#[test]
fn e4_penetrates_ice_resistance_only_at_or_above_eighty_percent_hp() {
    for (hp_cost, expected) in [(400, 336), (401, 300)] {
        let plan = PreparedBattle::official_dpr(&[yanqing(4, 0.0)], 42, 3).unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.spend_hp(0, hp_cost, 1);
        act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .map(|hit| hit.amount)
                .sum::<u32>(),
            expected
        );
    }
}

#[test]
fn e6_extends_both_active_buffs_after_a_teammates_defeat() {
    let enemy = fixture::enemy("fixture.target_left", 32).unwrap();
    let mut plan = PreparedBattle::official_dpr(&[yanqing(6, 0.0), arlan()], 42, 3).unwrap();
    plan.units[2].kit = Kit::Enemy(enemy);
    plan.units[2].stats.hp = 1;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act_with_yanqing(&mut battle, Action::Skill, "dpr:2");
    battle.units[0].energy_milli = 140_000;
    act_with_yanqing(&mut battle, Action::Ultimate, "dpr:2");
    act(&mut battle, "ally.arlan", Action::Basic, "dpr:1");
    assert_eq!(battle.hp(2).0, 0);

    act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
    assert!(soulsteel_active(&battle, 0));
    assert!(ultimate_active(&battle, 0));
    act(&mut battle, "ally.arlan", Action::Basic, "dpr:3");
    act_with_yanqing(&mut battle, Action::Basic, "dpr:2");
    assert!(!soulsteel_active(&battle, 0));
    assert!(!ultimate_active(&battle, 0));
}

#[test]
fn fixed_dpr_replays_follow_up_and_preserves_damage_attribution() {
    let plan = PreparedBattle::official_dpr(&[yanqing(0, 0.6)], 42, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
    );
    assert_eq!(result.characters[0].own_damage, result.total_damage);
    assert!(
        result.targets[1]
            .hits
            .iter()
            .any(|hit| { hit.kind == DamageKind::FollowUp && hit.source_id == "ally.yanqing" })
    );
}
