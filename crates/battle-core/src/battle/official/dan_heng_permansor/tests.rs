use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, combat_status::DebuffKind, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget, GAUGE_FULL},
};

use super::*;

const HERTA: &str = "character.1013";
const SUNDAY: &str = "character.1313";

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Preservation,
            element: PHYSICAL,
        },
        energy_capacity: 135,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_200,
            base_attack: 1_200,
            defense: 500,
            base_defense: 500,
            base_aggro: 150,
            base_speed_micros: 100_000_000,
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
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![0.2, 400.0, 3.0, 3.0]),
            (
                ULTIMATE.into(),
                vec![3.0, 0.8, 2.0, 0.2, 400.0, 3.0, 3.0, 0.8],
            ),
            (TALENT.into(), vec![0.1, 200.0, 3.0, 3.0, 165.0, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn herta() -> OfficialCharacter {
    let mut ally = character(0, &[]);
    ally.unit_id = HERTA.into();
    ally.form_id = HERTA.into();
    ally.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 130_000_000;
    ally.stats.attack = 1_000;
    ally.stats.base_attack = 1_000;
    ally.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    ally
}

fn sunday() -> OfficialCharacter {
    let mut ally = character(0, &[]);
    ally.unit_id = SUNDAY.into();
    ally.form_id = SUNDAY.into();
    ally.combat_type = CombatType {
        path: CombatPath::Harmony,
        element: Element::Imaginary,
    };
    ally.energy_capacity = 130;
    ally.stats.speed_micros = 140_000_000;
    ally.stats.attack = 800;
    ally.stats.base_attack = 800;
    ally.skill_parameters = [
        ("skill.131301".into(), vec![1.0]),
        ("skill.131302".into(), vec![1.0, 0.30, 2.0, 0.50]),
        ("skill.131303".into(), vec![0.20, 0.30, 3.0, 0.12]),
        ("skill.131304".into(), vec![0.20, 3.0]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        target_id: target.map(str::to_owned),
        action,
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

fn ult(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, FORM, Action::Ultimate, None);
}

#[test]
fn recognizes_base_and_accepts_fractional_shield_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut low = character(0, &[]);
    low.skill_parameters
        .insert(SKILL.into(), vec![0.182, 332.5, 3.0, 3.0]);
    low.skill_parameters
        .insert(TALENT.into(), vec![0.091, 166.25, 3.0, 3.0, 165.0, 1.0]);
    let prepared = plan(std::slice::from_ref(&low));
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(FORM));
    assert_eq!(battle.shield_amount(0), 550);
    battle.mechanics(0).unwrap().on_summon_turn(&mut battle, 0);
    assert_eq!(battle.shield_amount(0), 825);
    low.skill_parameters.get_mut(TALENT).unwrap()[4] = 160.0;
    assert!(build(&low).is_err());
}

#[test]
fn binds_one_holder_updates_attack_and_reuses_the_same_capped_shield() {
    let prepared = plan(&[character(0, &[BOND_ATTACK]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 97_500);
    assert_eq!(battle.shield_amount(0), 640);
    assert_eq!(battle.shield_amount(1), 640);
    assert_eq!(battle.effective_attack(1), 1_180);
    let dragon = battle.units[0].summon_turn.unwrap();
    assert_eq!(dragon.speed_micros, 165_000_000);
    assert_eq!(dragon.gauge_quanta, GAUGE_FULL);
    battle.units[0].summon_turn.as_mut().unwrap().advance(2_500);
    let rules = battle.mechanics(0).unwrap();
    rules.after_action(&mut battle, 0, Action::Skill, Some(0));
    assert_eq!(bondmate(&battle, 0), Some(0));
    assert_eq!(battle.effective_attack(1), 1_000);
    assert_eq!(battle.effective_attack(0), 1_380);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        GAUGE_FULL - gauge_for(2_500)
    );
    for _ in 0..4 {
        rules.after_action(&mut battle, 0, Action::Skill, Some(0));
    }
    assert_eq!(battle.shield_amount(0), 3 * 676);
    assert_eq!(battle.units[0].shields.len(), 1);
}

#[test]
fn an_actual_bondmate_attack_advances_the_dragon_and_restores_energy_once() {
    let prepared = plan(&[character(0, &[SYLVANITY]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.av_micros, 30_000_000);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    battle.advance_to_decision();
    assert_eq!(battle.ordinary_actor, Some(1));
    let gauge = battle.units[0].summon_turn.unwrap().gauge_quanta;
    let energy = battle.units[0].energy_milli;
    act(&mut battle, HERTA, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli - energy, 6_000);
    assert_eq!(
        battle.units[0].summon_turn.unwrap().gauge_quanta,
        gauge - gauge_for(1_500)
    );
}

#[test]
fn dragon_cleanses_one_debuff_per_target_and_shields_the_weakest_again() {
    let prepared = plan(&[character(0, &[DRAGON_BLESSING]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    battle.deplete_shields(1, 250);
    for target in 0..2 {
        battle.apply_debuff(2, target, DebuffKind::AttackDown, 1_000, 3);
        battle.apply_debuff(2, target, DebuffKind::DefenseDown, 1_000, 3);
    }
    battle.mechanics(0).unwrap().on_summon_turn(&mut battle, 0);
    assert_eq!(battle.units[0].debuffs.len(), 1);
    assert_eq!(battle.units[1].debuffs.len(), 1);
    assert_eq!(battle.shield_amount(0), 640 + 320);
    assert_eq!(battle.shield_amount(1), 390 + 320 + 160);
    assert!(battle.dummy_damage.is_empty());
}

#[test]
fn e2_ultimate_advances_four_dragon_turns_with_scaled_shields_and_distinct_damage() {
    let prepared = plan(&[character(2, &[DRAGON_BLESSING]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    battle.advance_to_decision();
    ult(&mut battle);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.units[0].summon_turn.unwrap().gauge_quanta, 0);
    assert_eq!(battle.units[0].permansor_state.empowered_actions, 4);
    assert_eq!(
        battle.ally_buff_value(0, 1, AllyBuffKind::ResistancePenetration),
        1_800
    );
    let direct = &battle.dummy_damage;
    assert_eq!(direct.len(), 3);
    assert!(
        direct
            .iter()
            .all(|hit| hit.actor == 0 && hit.kind == DamageKind::Direct)
    );
    battle.dummy_damage.clear();
    let rules = battle.mechanics(0).unwrap();
    rules.on_summon_turn(&mut battle, 0);
    assert_eq!(battle.units[0].permansor_state.empowered_actions, 3);
    assert_eq!(battle.shield_amount(0), 1_920);
    assert_eq!(battle.shield_amount(1), 1_920);
    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 7);
    assert!(
        hits[..3]
            .iter()
            .all(|hit| hit.actor == 0 && hit.kind == DamageKind::FollowUp)
    );
    assert!(hits[3..].iter().all(|hit| {
        hit.actor == 1 && hit.kind == DamageKind::Additional && hit.assists == [(0, hit.amount)]
    }));
    assert_eq!(hits[3].amount, 2 * hits[6].amount);
    for enemy in 2..5 {
        assert_eq!(battle.units[enemy].toughness, 960);
    }
    for _ in 0..3 {
        rules.on_summon_turn(&mut battle, 0);
    }
    battle.dummy_damage.clear();
    rules.on_summon_turn(&mut battle, 0);
    assert!(battle.dummy_damage.is_empty());
}

#[test]
fn sunday_recognizes_the_bondmates_linked_summon_and_advances_it() {
    let prepared = plan(&[character(0, &[]), herta(), sunday()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    assert!(battle.has_advanceable_summon(0, 1));
    assert!(!battle.has_advanceable_summon(0, 0));
    battle.advance_to_decision();
    assert_eq!(battle.ordinary_actor, Some(2));
    act(&mut battle, SUNDAY, Action::Skill, Some(HERTA));
    assert_eq!(battle.units[0].summon_turn.unwrap().gauge_quanta, 0);
    assert_eq!(battle.ally_buff_value(2, 1, AllyBuffKind::Damage), 8_000);
}

#[test]
fn finite_defeat_removes_the_bond_without_leaving_an_orphan_dragon() {
    static FINITE: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 1_000,
        weakness: [true; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[character(6, &[DRAGON_BLESSING]), herta()]);
    prepared.units[2].kit = Kit::Enemy(&FINITE);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.ally_damage_reduction_bp_for_target(&battle, 0, 0), 0);
    assert_eq!(
        rules.ally_damage_reduction_bp_for_target(&battle, 0, 1),
        2_000
    );
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 1).defense_down_bp,
        1_200
    );
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 0).vulnerability_bp,
        2_000
    );
    battle.confirm_defeat(2, 1);
    assert!(bondmate(&battle, 0).is_none());
    assert!(battle.units[0].summon_turn.is_none());
    assert!(rules.damage_target_aura(&battle, 0, 0) == DamageTargetAura::default());
}

#[test]
fn fixed_dpr_preserves_follow_up_and_bondmate_provenance_and_credit() {
    let scenario = STANDARD_DPR_SCENARIO;
    let prepared = plan(&[
        character(6, &[BOND_ATTACK, SYLVANITY, DRAGON_BLESSING]),
        herta(),
        sunday(),
    ]);
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == FORM && hit.kind == DamageKind::FollowUp));
    for action in [Action::Skill, Action::Ultimate] {
        assert!(hits().any(|hit| {
            hit.source_id == HERTA
                && hit.action == action
                && hit.kind == DamageKind::Additional
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == FORM && credit.amount > 0)
        }));
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|ally| ally.own_damage + ally.assist_damage)
            .sum::<u64>()
    );
}
