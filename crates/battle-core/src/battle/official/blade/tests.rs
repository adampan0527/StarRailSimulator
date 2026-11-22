use super::*;
use crate::{
    CombatPath, CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, analysis::DummyDamage},
    calculate_dpr,
    fixture::{Ability, DUMMY_STATS, EnemyKit, EnemyTarget},
};

const CENTER: &str = "dpr:2";

fn blade(enhanced: bool, eidolon: u8, traces: [bool; 3]) -> OfficialCharacter {
    let ids = if enhanced { &NOVAFLARE } else { &BASE };
    let parameters = if enhanced {
        [
            vec![0.5],
            vec![0.3, 3.0, 1.0, 0.4, 10.0],
            vec![1.5, 1.0, 0.6, 0.5, 1.2, 0.6, 0.9],
            vec![3.0, 1.3, 0.25, 1.3],
            vec![0.1, 1.3, 0.52, 1.3, 0.4],
        ]
    } else {
        [
            vec![1.0],
            vec![0.3, 3.0, 1.0, 0.4],
            vec![0.4, 1.0, 0.16, 0.4, 1.0, 0.4, 0.9],
            vec![3.0, 0.44, 0.25, 1.1],
            vec![0.1, 0.4, 0.16, 1.0, 0.4],
        ]
    };
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: WIND,
        },
        energy_capacity: MAX_ENERGY,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 2_500,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 97_000_000,
            speed_micros: 200_000_000,
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
            ids.basic,
            ids.skill,
            ids.ultimate,
            ids.talent,
            ids.enhanced_basic,
        ]
        .into_iter()
        .map(str::to_owned)
        .zip(parameters)
        .collect(),
        eidolon,
        enabled_trace_ids: ids
            .traces
            .into_iter()
            .zip(traces)
            .filter(|&(_, enabled)| enabled)
            .map(|(id, _)| id.into())
            .collect(),
    }
}

fn prepared(character: OfficialCharacter) -> PreparedBattle {
    PreparedBattle::official_dpr(&[character], 42, 3)
        .unwrap()
        .with_dummies(&[400, 400, 400])
        .unwrap()
}

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = command(battle, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn hits<'a>(battle: &'a Battle<'_>, kind: DamageKind) -> impl Iterator<Item = &'a DummyDamage> {
    battle
        .dummy_damage
        .iter()
        .filter(move |hit| hit.kind == kind)
}

#[test]
fn blade_unenhanced_basic_uses_the_variant_stat_and_generates_a_point() {
    for (enhanced, amount) in [(false, 600), (true, 1_500)] {
        let plan = prepared(blade(enhanced, 0, [false; 3]));
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        act(&mut battle, Action::Basic, Some(CENTER));
        assert_eq!(
            hits(&battle, DamageKind::Direct).next().unwrap().amount,
            amount
        );
        assert_eq!(battle.skill_points, 4);
        assert_eq!(battle.units[0].energy_milli, 85_000);
        assert_eq!(battle.units[0].hp, 5_000);
    }
}

#[test]
fn blade_skill_keeps_one_turn_and_enhances_exactly_three_ordinary_basics() {
    for (enhanced, amounts) in [
        (false, [4_536, 1_814, 1_814]),
        (true, [5_460, 2_184, 2_184]),
    ] {
        let plan = prepared(blade(enhanced, 0, [false; 3]));
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        let before = battle.av_micros;
        act(&mut battle, Action::Skill, None);
        assert_eq!(battle.av_micros, before);
        assert_eq!(battle.units[0].finished_turns, 0);
        assert_eq!(battle.units[0].energy_milli, 65_000);
        assert_eq!(battle.units[0].hp, 3_500);
        assert_eq!(battle.skill_points, 2);
        assert_eq!(
            battle.enemy_targeting_weight(0),
            if enhanced { 13_750_000 } else { 1_250_000 }
        );
        let snapshot = battle.snapshot();
        assert_eq!(
            battle.apply(&command(&battle, Action::Skill, None)),
            Err(BattleError::IllegalTiming)
        );
        assert_eq!(battle.snapshot(), snapshot);
        act(&mut battle, Action::Basic, Some(CENTER));
        assert_eq!(
            hits(&battle, DamageKind::Direct)
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            amounts
        );
        assert_eq!(battle.units[0].hp, 3_000);
        assert_eq!(battle.units[0].energy_milli, 95_000);
        assert_eq!(battle.units[0].blade_state.charges, 2);
        assert_eq!(
            [
                battle.units[2].toughness,
                battle.units[1].toughness,
                battle.units[3].toughness
            ],
            [380, 390, 390]
        );
        act(&mut battle, Action::Basic, Some(CENTER));
        act(&mut battle, Action::Basic, Some(CENTER));
        assert_eq!(battle.skill_points, 2);
        assert_eq!(battle.units[0].finished_turns, 3);
        assert!(
            battle
                .legal_actions()
                .contains(&command(&battle, Action::Skill, None))
        );
        assert_eq!(battle.enemy_targeting_weight(0), 1_250_000);
    }
}

#[test]
fn blade_ultimate_counts_its_loss_before_damage_and_only_enhanced_a2_keeps_half_the_tally() {
    for (enhanced, expected_damage, expected_tally) in [
        (false, [6_990, 1_896, 1_896], 0),
        (true, [8_550, 2_700, 2_700], 1_250),
    ] {
        let plan = prepared(blade(enhanced, 1, [true, true, false]));
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.set_energy(0, 0, 130_000);
        act(&mut battle, Action::Ultimate, Some(CENTER));
        assert_eq!(battle.units[0].hp, 2_500);
        assert_eq!(battle.units[0].blade_state.charges, 1);
        assert_eq!(battle.units[0].blade_state.hp_tally, expected_tally);
        assert_eq!(
            hits(&battle, DamageKind::Direct)
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            expected_damage
        );
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.units[0].finished_turns, 0);
    }
}

#[test]
fn blade_ultimate_sets_low_hp_exactly_without_incoming_healing_or_charge() {
    let plan = prepared(blade(true, 0, [true, true, false]));
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 100;
    battle.units[0].blade_state.hp_tally = 1_000;
    battle.set_energy(0, 0, 130_000);
    act(&mut battle, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.units[0].hp, 2_500);
    assert_eq!(battle.units[0].blade_state.charges, 0);
    assert_eq!(battle.units[0].blade_state.hp_tally, 500);
}

#[test]
fn blade_enhanced_a4_counts_restored_hp_and_caps_the_tally_with_damage() {
    let plan = prepared(blade(true, 0, [true, true, false]));
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 4_900;
    battle.units[0].blade_state.hp_tally = 4_400;
    battle.heal_unit(0, 0, 2_000);
    battle.heal_unit(0, 0, 2_000);
    assert_eq!(battle.hp(0), (5_000, 5_000));
    assert_eq!(battle.units[0].blade_state.hp_tally, 4_425);
    assert_eq!(battle.units[0].blade_state.charges, 0);
    battle.set_energy(0, 0, 130_000);
    act(&mut battle, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.units[0].blade_state.hp_tally, 2_250);
    assert_eq!(
        hits(&battle, DamageKind::Direct).next().unwrap().amount,
        7_740
    );
    // Any tally carried over from a higher temporary HP cap uses the current cap.
    battle.units[0].blade_state.hp_tally = 4_900;
    battle.set_energy(0, 0, 130_000);
    act(&mut battle, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.units[0].blade_state.hp_tally, 2_250);
}

#[test]
fn blade_enhanced_e1_adds_tally_to_primary_basic_and_e2_only_crits_in_hellscape() {
    let mut character = blade(true, 2, [false; 3]);
    character.stats.crit_rate_bp = 8_500;
    let plan = prepared(character);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].blade_state.hp_tally = 1_000;
    assert_eq!(
        battle.mechanics(0).unwrap().crit_rate_bonus_bp(
            &battle,
            0,
            2,
            Some(Action::Basic),
            DamageKind::Direct
        ),
        0
    );
    act(&mut battle, Action::Skill, None);
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].blade_state.hp_tally, 3_000);
    assert_eq!(
        hits(&battle, DamageKind::Direct)
            .map(|hit| (hit.amount, hit.critical))
            .collect::<Vec<_>>(),
        [(13_860, true), (3_276, true), (3_276, true)]
    );
}

#[test]
fn blade_e4_requires_a_threshold_crossing_and_observes_direct_ally_hp_costs() {
    let mut arlan = blade(false, 0, [false; 3]);
    arlan.unit_id = "ally.arlan".into();
    arlan.form_id = "character.1008".into();
    arlan.energy_capacity = 110;
    arlan.combat_type.element = Element::Thunder;
    arlan.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    let plan = PreparedBattle::official_dpr(&[blade(true, 4, [false; 3]), arlan], 42, 3)
        .unwrap()
        .with_dummies(&[400])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 2_600;
    assert_eq!(battle.spend_ally_hp(1, 0, 100, 1), 100);
    assert_eq!(battle.hp(0), (3_000, 5_500));
    assert_eq!(battle.spend_ally_hp(1, 0, 250, 1), 250);
    assert_eq!(battle.hp(0), (3_250, 6_000));
    assert_eq!(battle.spend_ally_hp(1, 0, 20_000, 1), 3_249);
    assert_eq!(battle.spend_ally_hp(1, 0, 1, 1), 0);
    assert_eq!(battle.hp(0), (1, 6_000));
    assert_eq!(battle.units[0].blade_state.e4_stacks, 2);
    assert_eq!(battle.units[0].blade_state.hp_tally, 3_599);
    assert_eq!(battle.units[0].blade_state.charges, 3);
}

#[test]
fn blade_e6_executes_after_the_skill_when_fourth_charge_arrives_and_a6_adds_energy() {
    let plan = prepared(blade(true, 6, [false, true, true]));
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    for _ in 0..3 {
        battle.spend_hp(0, 100, 1);
    }
    act(&mut battle, Action::Skill, None);
    assert_eq!(battle.units[0].blade_state.charges, 0);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(battle.units[0].hp, 4_700);
    assert_eq!(battle.units[0].blade_state.hp_tally, 2_175);
    assert_eq!(
        hits(&battle, DamageKind::FollowUp)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [8_640; 3]
    );
    assert_eq!(battle.units[0].finished_turns, 0);
}

#[test]
fn blade_original_traces_heal_on_a_broken_target_and_only_boost_low_hp_healing() {
    let plan = prepared(blade(false, 0, [true, true, true]));
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[2].toughness = 0;
    act(&mut battle, Action::Skill, None);
    act(&mut battle, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].hp, 3_350);
    battle.units[0].hp = 2_400;
    battle.heal_unit(0, 0, 100);
    assert_eq!(battle.units[0].hp, 2_520);
    battle.heal_unit(0, 0, 100);
    assert_eq!(battle.units[0].hp, 2_620);
    assert_eq!(battle.units[0].blade_state.hp_tally, 2_000);
}

#[test]
fn blade_records_multiple_enemy_hits_as_one_charge_and_shields_avoid_hp_loss() {
    let mut plan = prepared(blade(true, 0, [false; 3]));
    plan.units[1].kit = Kit::Enemy(&EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 400,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 10_000,
    });
    plan.units[1].stats.attack = 1_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let strike = Effect::Hit {
        element: Element::Physical,
        multiplier_bp: 10_000,
        toughness: 0,
    };
    let effects = [strike; 2];
    let attack = Ability {
        target: Target::Enemy,
        energy_milli: 0,
        effects: &effects,
    };
    battle.apply_official_shield(0, 0, 5_000, 1, Some(Action::Skill));
    battle.enemy_action = Some(1);
    battle.resolve_ability(1, Some(0), attack);
    battle.finish_enemy_action();
    assert_eq!(battle.hp(0), (5_000, 5_000));
    assert_eq!(battle.units[0].blade_state.charges, 0);
    battle.deplete_shields(0, 5_000);
    battle.enemy_action = Some(1);
    battle.resolve_ability(1, Some(0), attack);
    battle.finish_enemy_action();
    assert_eq!(battle.units[0].blade_state.charges, 1);
    assert_eq!(
        battle.units[0].blade_state.hp_tally,
        5_000 - battle.units[0].hp
    );
    assert!(battle.units[0].blade_state.hp_tally > 0);
    battle.enemy_action = Some(1);
    battle.resolve_ability(1, Some(0), attack);
    battle.finish_enemy_action();
    assert_eq!(battle.units[0].blade_state.charges, 2);
}

fn bailu() -> OfficialCharacter {
    let mut bailu = blade(false, 0, [false; 3]);
    bailu.unit_id = "ally.bailu".into();
    bailu.form_id = "character.1211".into();
    bailu.energy_capacity = 100;
    bailu.combat_type.path = CombatPath::Abundance;
    bailu.combat_type.element = Element::Thunder;
    bailu.skill_parameters = [
        ("skill.121101".into(), vec![1.0]),
        ("skill.121102".into(), vec![0.112, 280.0, 0.15, 2.0]),
        ("skill.121103".into(), vec![0.135, 360.0, 2.0]),
        ("skill.121104".into(), vec![0.054, 144.0, 0.12, 120.0, 2.0]),
    ]
    .into();
    bailu
}

#[test]
fn blade_mixed_hit_credits_only_the_max_hp_gain_to_its_provider() {
    let plan = PreparedBattle::official_dpr(&[blade(false, 0, [false; 3]), bailu()], 42, 3)
        .unwrap()
        .with_dummies(&[400, 400, 400])
        .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_max_hp_increase(1, 0, 300, 2);
    act(&mut battle, Action::Skill, None);
    act(&mut battle, Action::Basic, Some(CENTER));
    let primary = hits(&battle, DamageKind::Direct).next().unwrap();
    assert_eq!(primary.amount, 4_788);
    assert_eq!(primary.assists, [(1, 252)]);
}

#[test]
fn blade_fixed_dpr_is_reproducible_in_both_variants() {
    for enhanced in [false, true] {
        let plan =
            PreparedBattle::official_dpr(&[blade(enhanced, 6, [true; 3]), bailu()], 42, 3).unwrap();
        let run = || {
            calculate_dpr(
                &plan,
                &STANDARD_DPR_SCENARIO.toughness,
                STANDARD_DPR_SCENARIO.cycles,
            )
            .unwrap()
        };
        let first = run();
        assert_eq!(first, run());
        assert_eq!(
            first.total_damage,
            first
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>()
        );
        assert!(
            first
                .targets
                .iter()
                .flat_map(|enemy| &enemy.hits)
                .any(|hit| hit.source_id == FORM && hit.kind == DamageKind::FollowUp)
        );
    }
}
