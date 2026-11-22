use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const BLADE: &str = "character.1205";
const CENTER: &str = "dpr:2";

fn jade(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: Element::Quantum,
        },
        energy_capacity: ENERGY_CAP,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 5_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 103_000_000,
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
            (BASIC.into(), vec![0.9, 0.3]),
            (SKILL.into(), vec![30.0, 0.02, 0.25, 3.0]),
            (ULTIMATE.into(), vec![0.8, 2.0, 2.4]),
            (TALENT.into(), vec![0.024, 50.0, 8.0, 5.0, 1.2]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn blade() -> OfficialCharacter {
    let mut character = jade(0, &[]);
    character.unit_id = BLADE.into();
    character.form_id = BLADE.into();
    character.combat_type = CombatType {
        path: CombatPath::Destruction,
        element: Element::Wind,
    };
    character.energy_capacity = 130;
    character.stats.base_speed_micros = 97_000_000;
    character.stats.speed_micros = 97_000_000;
    character.skill_parameters = [
        ("skill.120501".into(), vec![1.0]),
        ("skill.120502".into(), vec![0.3, 3.0, 1.0, 0.4]),
        (
            "skill.120503".into(),
            vec![0.4, 1.0, 0.16, 0.4, 1.0, 0.4, 0.9],
        ),
        ("skill.120504".into(), vec![3.0, 0.44, 0.25, 1.1]),
        ("skill.120508".into(), vec![0.1, 0.4, 0.16, 1.0, 0.4]),
    ]
    .into();
    character
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[120; 3])
        .unwrap();
    for unit in &mut plan.units[team.len()..] {
        unit.stats.defense = 1_000;
    }
    plan
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
    assert!(battle.legal_actions().contains(&input));
    battle.apply(&input).unwrap();
}

#[test]
fn jade_rejects_inconsistent_locked_limits_and_non_base_variants() {
    let mut input = jade(0, &[]);
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(TALENT).unwrap()[2] = 9.0;
    assert!(build(&input).is_err());
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
}

#[test]
fn jade_basic_blast_counts_one_charge_per_target_and_restores_a_point() {
    let plan = plan(&[jade(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].jade_state.charge, 3);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(
        battle.units[1..]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>(),
        [115, 110, 115]
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(2, 405), (1, 135), (3, 135)]
    );
}

#[test]
fn jade_collector_costs_hp_once_and_counts_for_blade() {
    let plan = plan(&[jade(1, &[]), blade()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(BLADE));
    assert_eq!(battle.speed_micros(1), 127_000_000);
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert!(!battle.mechanics(0).unwrap().can_use_skill(&battle, 0));
    battle.ordinary_actor = Some(1);
    act(&mut battle, BLADE, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[1].hp, 4_900);
    assert_eq!(battle.units[0].jade_state.charge, 3);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::Additional)
        .collect();
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].actor, hits[0].target, hits[0].amount), (0, 3, 112));
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::HpSpent { amount: 100 }
                && event.source_id.as_deref() == Some(FORM)
                && event.target_id.as_deref() == Some(BLADE))
            .count(),
        1
    );
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind
                == EventKind::BladeCharge {
                    before: 0,
                    after: 1
                })
            .count(),
        1
    );

    battle.units[1].hp = 2;
    battle.ordinary_actor = Some(1);
    act(&mut battle, BLADE, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[1].hp, 1);
    battle.ordinary_actor = Some(1);
    act(&mut battle, BLADE, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[1].hp, 1);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::HpSpent { amount: 1 })
            .count(),
        1
    );
}

#[test]
fn jade_collector_uses_her_turns_and_only_grants_speed_to_the_other_ally() {
    let plan = plan(&[
        jade(0, &[REVERSE_REPO, COLLATERAL_TICKET, ASSET_FORFEITURE]),
        blade(),
    ]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.av_micros, 25_000_000);
    assert_eq!(battle.units[0].jade_state.assets, 3);
    act(&mut battle, FORM, Action::Skill, Some(BLADE));
    let rules = battle.mechanics(0).unwrap();
    let expiry = battle.units[0].jade_state.collector.unwrap().expires_at;
    battle.units[1].normal_turns = 10;
    let assets = battle.units[0].jade_state.assets;
    rules.on_ally_turn_opening(&mut battle, 0, 1, false);
    assert_eq!(battle.units[0].jade_state.assets, assets + 3);
    assert_eq!(
        battle.effective_attack(0),
        1_000 + u32::from(assets + 3) * 5
    );
    battle.units[0].normal_turns = expiry - 1;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(battle.speed_micros(1), 127_000_000);
    battle.units[0].normal_turns = expiry;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert!(rules.can_use_skill(&battle, 0));
    assert_eq!(battle.speed_micros(1), 97_000_000);

    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Skill, Some(FORM));
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].hp, 5_000);
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .count(),
        3
    );
    assert_eq!(battle.units[0].jade_state.charge, 3);
}

#[test]
fn jade_ultimate_enhances_exactly_two_follow_ups_that_carry_excess_charge() {
    let plan = plan(&[jade(0, &[])]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].jade_state.charge = 7;
    battle.units[0].energy_milli = 140_000;
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].jade_state.enhanced_attacks, 1);
    assert_eq!(battle.units[0].jade_state.charge, 2);
    assert_eq!(battle.units[0].jade_state.assets, 5);
    assert_eq!(battle.units[0].energy_milli, 15_000);
    for _ in 0..2 {
        battle.units[0].jade_state.charge = 5;
        battle.ordinary_actor = Some(0);
        act(&mut battle, FORM, Action::Basic, Some(CENTER));
    }
    assert_eq!(battle.units[0].jade_state.enhanced_attacks, 0);
    assert_eq!(battle.units[0].jade_state.assets, 15);
    assert_eq!(battle.units[0].jade_state.charge, 0);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::FollowUp)
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [900, 900, 900, 900, 900, 900, 540, 540, 540]
    );
}

#[test]
fn jade_eidolons_apply_to_their_own_windows() {
    let plan = plan(&[jade(6, &[]), blade()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(BLADE));
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(
        rules.resistance_penetration_bp(&battle, 0, Element::Quantum),
        2_000
    );
    assert_eq!(
        rules.resistance_penetration_bp(&battle, 0, Element::Fire),
        0
    );
    assert_eq!(
        rules.damage_bonus_bp(&battle, 0, 3, Some(Action::Basic), DamageKind::FollowUp),
        3_200
    );
    assert_eq!(
        rules.crit_rate_bonus_bp(&battle, 0, 3, Some(Action::Basic), DamageKind::Direct),
        0
    );
    Jade::gain_assets(&mut battle, 0, 15);
    assert_eq!(
        rules.crit_rate_bonus_bp(&battle, 0, 3, Some(Action::Basic), DamageKind::Direct),
        1_800
    );
    Jade::gain_assets(&mut battle, 0, 50);
    assert_eq!(
        rules.crit_damage_bonus_bp(&battle, 0, 3, Some(Action::Basic), DamageKind::Direct),
        12_000
    );

    battle.units[0].energy_milli = 140_000;
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(rules.defense_ignore_bp(&battle, 0, 3), 1_200);
    assert_eq!(battle.units[0].jade_state.charge, 3);
    assert_eq!(battle.units[0].hp, 5_000);
    assert_eq!(battle.speed_micros(0), 200_000_000);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .count(),
        3
    );
    battle.units[0].finished_turns = battle.units[0].jade_state.defense_ignore_until;
    battle.units[0].normal_turns = battle.units[0].jade_state.collector.unwrap().expires_at;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(rules.defense_ignore_bp(&battle, 0, 3), 0);
    assert_eq!(
        rules.resistance_penetration_bp(&battle, 0, Element::Quantum),
        0
    );
}

#[test]
fn jade_finite_targets_count_lethal_hits_and_only_damage_surviving_targets() {
    static ENEMY: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [true; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut plan = plan(&[jade(0, &[])]);
    plan.units[2].stats.hp = 400;
    plan.units[2].kit = Kit::Enemy(&ENEMY);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(FORM));
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(battle.units[0].jade_state.charge, 3);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Additional)
            .map(|hit| hit.target)
            .collect::<Vec<_>>(),
        [1, 3]
    );
}

#[test]
fn jade_fixed_dpr_replays_with_detectable_charge_and_damage_sources() {
    let plan = PreparedBattle::official_dpr(
        &[jade(1, &[REVERSE_REPO, COLLATERAL_TICKET]), blade()],
        42,
        3,
    )
    .unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    for kind in [
        DamageKind::Direct,
        DamageKind::FollowUp,
        DamageKind::Additional,
    ] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| hit.source_id == FORM && hit.kind == kind)
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
}
