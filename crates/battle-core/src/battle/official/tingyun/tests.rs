use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, battle::Kit, calculate_dpr, fixture,
};

use super::{
    BASIC, FORM, JUBILANT_PASSAGE, KNELL_SUBDUAL, NOURISHED_JOVIALITY, SKILL, TALENT, ULTIMATE,
};

fn tingyun(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.tingyun".into(),
        form_id: FORM.into(),
        combat_type: crate::CombatType {
            path: crate::CombatPath::Harmony,
            element: crate::Element::Thunder,
        },
        energy_capacity: 130,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 2_000,
            base_attack: 900,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; crate::Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.2, 0.25, 3.0, 0.15]),
            (ULTIMATE.into(), vec![50.0, 2.0, 0.2]),
            (TALENT.into(), vec![0.3]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn herta() -> OfficialCharacter {
    let mut input = tingyun(0);
    input.unit_id = "ally.herta".into();
    input.form_id = "character.1013".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Erudition,
        element: crate::Element::Ice,
    };
    input.energy_capacity = 110;
    input.stats.attack = 1_200;
    input.stats.base_attack = 800;
    input.stats.base_speed_micros = 150_000_000;
    input.stats.speed_micros = 150_000_000;
    input.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    input
}

fn natasha() -> OfficialCharacter {
    let mut input = herta();
    input.unit_id = "ally.natasha".into();
    input.form_id = "natasha.abundance".into();
    input.combat_type = crate::CombatType {
        path: crate::CombatPath::Abundance,
        element: crate::Element::Physical,
    };
    input.energy_capacity = 90;
    input.skill_parameters = [
        ("skill.110501".into(), vec![0.5]),
        ("skill.110502".into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
        ("skill.110503".into(), vec![0.092, 92.0]),
        ("skill.110504".into(), vec![0.3, 0.25]),
    ]
    .into();
    input
}

fn prepared(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .expect("fixture action should be legal");
    battle.apply(&command).unwrap();
    command
}

fn basic(battle: &mut Battle<'_>) {
    let actor = battle.ordinary_actor().unwrap().to_owned();
    act(battle, &actor, Action::Basic, Some("dpr:2"));
}

#[test]
fn basic_has_two_hits_and_traces_apply_at_their_own_times() {
    let mut support = tingyun(0);
    support.enabled_trace_ids = [NOURISHED_JOVIALITY, KNELL_SUBDUAL, JUBILANT_PASSAGE]
        .map(String::from)
        .to_vec();
    support.stats.energy_regeneration_bp = 12_000;
    let plan = prepared(&[support, herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 71_000);
    basic(&mut battle);
    assert_eq!(battle.units[0].energy_milli, 95_000);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[3].toughness, 990);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [252, 588]
    );
    basic(&mut battle);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.herta"),
    );
    assert_eq!(battle.units[0].energy_milli, 130_000);
    assert_eq!(battle.speed_micros(0), 240_000_000);
    assert_eq!(battle.skill_points, 4);
}

#[test]
fn benediction_caps_attack_and_switches_off_the_previous_recipient() {
    let mut support = tingyun(0);
    support.stats.attack = 1_000;
    let plan = prepared(&[support, herta(), natasha()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.herta"),
    );
    assert_eq!(battle.effective_attack(1), 1_350);
    assert_eq!(battle.effective_attack(2), 1_200);
    while battle.ordinary_actor() != Some("ally.tingyun") {
        basic(&mut battle);
    }
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.natasha"),
    );
    assert_eq!(battle.effective_attack(1), 1_200);
    assert_eq!(battle.effective_attack(2), 1_350);
}

#[test]
fn benediction_expires_at_the_end_of_the_third_recipient_turn() {
    let plan = prepared(&[tingyun(0), herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.herta"),
    );
    assert_eq!(battle.effective_attack(1), 1_400);
    for completed in 1..=3 {
        while battle.ordinary_actor() != Some("ally.herta") {
            basic(&mut battle);
        }
        assert_eq!(battle.effective_attack(1), 1_400);
        basic(&mut battle);
        assert_eq!(battle.units[1].finished_turns, completed);
    }
    assert_eq!(battle.effective_attack(1), 1_200);
}

#[test]
fn both_additional_damage_triggers_use_the_recipients_own_lightning_and_crit() {
    let mut carry = herta();
    carry.stats.crit_rate_bp = 10_000;
    carry.stats.element_damage_bp[carry.combat_type.element.index()] = 8_000;
    carry.stats.element_damage_bp[crate::Element::Thunder.index()] = 3_000;
    let plan = prepared(&[tingyun(4), carry]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.herta"),
    );
    basic(&mut battle);
    basic(&mut battle);

    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .map(|hit| (hit.actor, hit.target, hit.critical, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(hits, [(1, 3, true, 655), (1, 3, true, 491)]);
    assert_eq!(battle.units[3].toughness, 980);
}

#[test]
fn area_attack_triggers_a_single_replayable_additional_hit() {
    let plan = prepared(&[tingyun(0), herta()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let commands = [
        act(
            &mut battle,
            "ally.tingyun",
            Action::Skill,
            Some("ally.herta"),
        ),
        act(&mut battle, "ally.herta", Action::Skill, None),
    ];
    let additional_targets = |battle: &Battle<'_>| {
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>()
    };
    let hits = additional_targets(&battle);
    assert_eq!(hits.len(), 1);
    assert!((2..=4).contains(&hits[0].0));
    assert_eq!(hits[0].1, 168);
    let replay = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
    assert_eq!(hits, additional_targets(&replay));
}

#[test]
fn ultimate_restores_flat_energy_and_damage_buff_expires_on_recipient_turns() {
    for (eidolon, after_milli) in [(0, 60_000), (6, 70_000)] {
        let mut carry = herta();
        carry.stats.energy_regeneration_bp = 15_000;
        let mut plan = prepared(&[tingyun(eidolon), carry]);
        plan.units[0].starting_energy_milli = 130_000;
        plan.units[1].starting_energy_milli = 10_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(
            &mut battle,
            "ally.tingyun",
            Action::Ultimate,
            Some("ally.herta"),
        );
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.units[1].energy_milli, after_milli);
        for _ in 0..3 {
            while battle.ordinary_actor() != Some("ally.herta") {
                basic(&mut battle);
            }
            basic(&mut battle);
        }
        let hits = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == 1)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>();
        assert_eq!(hits, [432, 432, 360]);
    }
}

#[test]
fn e1_stacks_with_existing_speed_up_after_a_non_attacking_ultimate() {
    let mut plan = prepared(&[tingyun(1), natasha()]);
    plan.units[1].starting_energy_milli = 90_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.natasha"),
    );
    battle.apply_speed_up(1, 1, 1_000, 2);
    act(&mut battle, "ally.natasha", Action::Ultimate, None);
    assert_eq!(battle.speed_micros(1), 195_000_000);
    assert!(battle.dummy_damage.is_empty());
    for _ in 0..2 {
        while battle.ordinary_actor() != Some("ally.natasha") {
            basic(&mut battle);
        }
        basic(&mut battle);
    }
    assert_eq!(battle.speed_micros(1), 165_000_000);
}

#[test]
fn e2_refunds_one_of_multiple_defeats_by_the_blessed_unit() {
    let mut carry = herta();
    carry.stats.energy_regeneration_bp = 15_000;
    let mut plan = prepared(&[tingyun(2), carry]);
    plan.units[1].starting_energy_milli = 110_000;
    for enemy in &mut plan.units[2..4] {
        enemy.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        enemy.stats.hp = 1;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(
        &mut battle,
        "ally.tingyun",
        Action::Skill,
        Some("ally.herta"),
    );
    act(&mut battle, "ally.herta", Action::Ultimate, None);
    assert_eq!(battle.units[1].energy_milli, 60_000);
    let transfers = battle.events().iter().filter(|event| {
        event.source_id.as_deref() == Some("ally.tingyun")
            && event.target_id.as_deref() == Some("ally.herta")
            && matches!(event.kind, EventKind::Energy { .. })
    });
    assert_eq!(transfers.count(), 1);
}

#[test]
fn dpr_assigns_both_kinds_of_damage_to_the_correct_contributors() {
    let team = [tingyun(6), herta()];
    let plan = PreparedBattle::official_dpr(&team, 42, 3).unwrap();
    let cycles = STANDARD_DPR_SCENARIO.cycles;
    let result = calculate_dpr(&plan, &[1_000; 3], cycles).unwrap();
    let hits = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .filter(|hit| hit.kind == DamageKind::Additional && hit.source_id == "ally.herta")
        .collect::<Vec<_>>();
    assert!(!hits.is_empty());
    for hit in hits {
        let credited = hit
            .credits
            .iter()
            .find(|credit| credit.unit_id == "ally.tingyun");
        assert_eq!(
            credited.map(|credit| credit.amount),
            Some(hit.amount),
            "{hit:?}"
        );
    }
    assert!(result.characters[0].own_damage > 0);
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, calculate_dpr(&plan, &[1_000; 3], cycles).unwrap());
}
