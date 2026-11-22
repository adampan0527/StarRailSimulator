use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, Outcome, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const GEPARD: &str = "character.1104";
const CENTER: &str = "dpr:2";
const A2: &str = "trace.1404101";
const A4: &str = "trace.1404102";
const A6: &str = "trace.1404103";

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn mydei(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: IMAGINARY,
        },
        energy_capacity: ENERGY,
        level: 80,
        stats: OfficialStats {
            hp: 5_000,
            base_hp: 2_500,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 95_000_000,
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
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.9, 0.5, 0.5]),
            (ULTIMATE.into(), vec![1.6, 1.0, 0.2, 2.0, 20.0]),
            (TALENT.into(), vec![0.25, 0.0, 150.0, 0.5, 0.5]),
            (KINGSLAYER.into(), vec![1.1, 0.66, 0.35]),
            (GODSLAYER.into(), vec![2.8, 1.68, 150.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn gepard() -> OfficialCharacter {
    let mut ally = mydei(0, &[]);
    ally.form_id = GEPARD.into();
    ally.unit_id = GEPARD.into();
    ally.combat_type = CombatType {
        path: CombatPath::Preservation,
        element: Element::Ice,
    };
    ally.energy_capacity = 100;
    ally.stats.speed_micros = 250_000_000;
    ally.skill_parameters = [
        ("skill.110401".into(), vec![1.0]),
        ("skill.110402".into(), vec![1.0, 0.65, 1.0, 0.3]),
        ("skill.110403".into(), vec![0.45, 3.0, 600.0]),
        ("skill.110404".into(), vec![0.5]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter], enemies: usize) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&vec![400; enemies])
        .unwrap()
}

fn finite(plan: &mut PreparedBattle, hp: u32, attack: u32) {
    for unit in &mut plan.units[plan.ally_count..] {
        unit.kit = Kit::Enemy(&ENEMY);
        unit.stats.hp = hp;
        unit.stats.attack = attack;
    }
}

fn command(battle: &Battle<'_>, action: Action, target: &str) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: Some(target.into()),
    }
}

fn act(battle: &mut Battle<'_>, action: Action, target: &str) {
    let command = command(battle, action, target);
    assert!(battle.legal_actions().contains(&command));
    battle.apply(&command).unwrap();
}

fn state<'a>(battle: &'a Battle<'_>, actor: usize) -> &'a MydeiState {
    &battle.units[actor].mydei_state
}

#[test]
fn basic_and_paid_blast_use_max_hp_and_the_actual_life_spent() {
    let plan = plan(&[mydei(0, &[])], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, Action::Basic, CENTER);
    assert_eq!(battle.dummy_damage[0].amount, 1_500);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 100_000);

    battle.units[0].hp = 3;
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.hp(0), (2, 5_000));
    assert_eq!(state(&battle, 0).charge_milli, 20);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 130_000);
    assert_eq!(
        battle.dummy_damage[1..]
            .iter()
            .map(|hit| hit.amount)
            .collect::<Vec<_>>(),
        [2_700, 1_500, 1_500]
    );

    battle.units[0].hp = 1;
    act(&mut battle, Action::Skill, CENTER);
    assert_eq!(battle.hp(0), (1, 5_000));
    assert_eq!(state(&battle, 0).charge_milli, 20);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::HpSpent { .. }))
            .count(),
        1
    );
}

#[test]
fn vendetta_advances_and_uses_a_separate_kingslayer_after_the_paid_skill() {
    let plan = plan(&[mydei(0, &[])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 100_000;
    battle.units[0].mydei_state.charge_milli = 80_000;
    act(&mut battle, Action::Skill, "dpr:1");

    assert!(state(&battle, 0).vendetta);
    assert_eq!(battle.hp(0), (4_469, 7_500));
    assert_eq!(battle.defense(0), 0);
    assert_eq!(state(&battle, 0).charge_milli, 62_080);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].energy_milli, 160_000);
    assert_eq!(battle.dummy_damage.len(), 2);
    assert_eq!(battle.dummy_damage[0].amount, 2_700);
    assert_eq!(battle.dummy_damage[1].amount, 4_950);
    assert_eq!(
        battle.dummy_damage[0].av_micros,
        battle.dummy_damage[1].av_micros
    );
    assert!(
        battle
            .events()
            .iter()
            .any(|event| { event.kind == EventKind::MydeiEnhancedSkill { godslayer: false } })
    );
    let actions = battle.legal_actions();
    assert_eq!(actions.len(), 2);
    assert!(
        actions
            .iter()
            .all(|command| command.action != Action::Basic)
    );
    assert!(
        actions
            .iter()
            .any(|command| command.action == Action::Ultimate)
    );
}

#[test]
fn ultimate_sets_the_godslayer_target_and_the_extra_turn_resumes_its_interrupted_actor() {
    for (eidolon, amounts) in [(0, vec![12_600, 7_560, 7_560]), (1, vec![13_950; 5])] {
        let plan = plan(&[gepard(), mydei(eidolon, &[])], 5);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.ordinary_actor(), Some(GEPARD));
        Mydei::set_vendetta(&mut battle, 1, true);
        battle.units[1].hp = 4_000;
        battle.units[1].mydei_state.charge_milli = 140_000;
        battle.units[1].energy_milli = 160_000;

        act(&mut battle, Action::Ultimate, "dpr:3");

        assert_eq!(battle.hp(1), (5_500, 7_500));
        assert_eq!(battle.units[1].energy_milli, 15_000);
        assert_eq!(battle.units[1].normal_turns, 0);
        assert_eq!(state(&battle, 1).charge_milli, 10_000);
        assert_eq!(state(&battle, 1).marked_target, None);
        assert_eq!(battle.ordinary_actor(), Some(GEPARD));
        assert_eq!(battle.skill_points, 3);
        assert_eq!(
            battle.dummy_damage[..3]
                .iter()
                .map(|hit| hit.amount)
                .collect::<Vec<_>>(),
            [7_200, 4_500, 4_500]
        );
        let godslayer = &battle.dummy_damage[3..];
        assert_eq!(godslayer[0].target, 4);
        assert_eq!(
            godslayer.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
            amounts
        );
        assert!(
            godslayer
                .iter()
                .all(|hit| hit.action == Action::Skill && hit.kind == DamageKind::Direct)
        );
        assert_eq!(battle.units[4].toughness, 350);
        assert_eq!(battle.units[3].toughness, 360);
        let extra_start = battle
            .events()
            .iter()
            .position(|event| event.kind == EventKind::ExtraTurnStarted)
            .unwrap();
        let ultimate_taunt = battle
            .events()
            .iter()
            .rposition(|event| event.kind == EventKind::StatusApplied(StatusKind::Taunt))
            .unwrap();
        assert!(ultimate_taunt < extra_start);
    }
}

#[test]
fn a6_uses_the_opening_hp_and_counts_only_actual_damage_for_its_bonus() {
    let mut input = mydei(0, &[A6]);
    input.stats.hp = 8_199;
    let mut plan = plan(&[input], 1);
    finite(&mut plan, 40_000, 1_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(state(&battle, 0).chiton_steps, 40);
    Mydei::set_vendetta(&mut battle, 0, true);
    let rules = battle.mechanics(0).unwrap();

    assert_eq!(
        rules.crit_rate_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
        4_800
    );
    assert_eq!(rules.incoming_healing_bonus_bp(&battle, 0), 3_000);
    assert_eq!(battle.hp(0), (12_298, 12_298));
    battle.apply_official_shield(0, 0, 1_000, 1, Some(Action::Skill));
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(state(&battle, 0).charge_milli, 0);
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.hp(0), (11_298, 12_298));
    assert_eq!(state(&battle, 0).charge_milli, 16_262);

    battle.spend_hp(0, 1_000, 1);
    assert_eq!(state(&battle, 0).charge_milli, 24_393);
    battle.heal_unit(0, 0, 1_000);
    assert_eq!(battle.hp(0), (11_598, 12_298));
    assert_eq!(state(&battle, 0).charge_milli, 24_393);
}

#[test]
fn a2_preserves_vendetta_three_times_and_a4_blocks_control_only_during_vendetta() {
    let mut plan = plan(&[mydei(0, &[A2, A4])], 1);
    finite(&mut plan, 40_000, 100_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Mydei::set_vendetta(&mut battle, 0, true);
    let rules = battle.mechanics(0).unwrap();
    battle.apply_break_freeze(1, 0, Action::Skill);
    assert!(!battle.is_frozen(0));
    assert!(!battle.apply_debuff(1, 0, DebuffKind::SkillImprisonment, 1_000, 1));

    for preserved in 1..=3 {
        battle.resolve_enemy(1, &ENEMY);
        assert!(state(&battle, 0).vendetta);
        assert_eq!(battle.hp(0), (3_750, 7_500));
        assert_eq!(state(&battle, 0).preserved_vendettas, preserved);
        assert!(state(&battle, 0).charge_milli > 0);
    }
    rules.on_unit_action_finished(&mut battle, 0, 1);
    assert!(state(&battle, 0).godslayer_queued);
    battle.resolve_enemy(1, &ENEMY);
    assert!(!state(&battle, 0).vendetta);
    assert_eq!(battle.hp(0), (2_500, 5_000));
    assert_eq!(state(&battle, 0).charge_milli, 0);
    assert!(!battle.start_extra_turn());
    assert_eq!(battle.defense(0), 500);
    battle.apply_break_freeze(1, 0, Action::Skill);
    assert!(battle.is_frozen(0));
    battle.resolve_enemy(1, &ENEMY);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::UnitDefeated)
    );
}

#[test]
fn e2_caps_effective_heal_per_action_and_godslayer_suppresses_charge_gain() {
    let plan = plan(&[gepard(), mydei(2, &[])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Mydei::set_vendetta(&mut battle, 1, true);
    let rules = battle.mechanics(1).unwrap();
    assert_eq!(rules.defense_ignore_bp(&battle, 1, 2), 1_500);
    battle.units[1].hp = 4_000;
    battle.heal_unit(0, 1, 6_000);
    assert_eq!(state(&battle, 1).charge_milli, 18_666);
    battle.heal_unit(0, 1, 6_000);
    assert_eq!(state(&battle, 1).charge_milli, 18_666);

    for _ in 0..3 {
        battle.spend_hp(1, 4_000, 1);
        battle.heal_unit(0, 1, 4_000);
    }
    assert_eq!(state(&battle, 1).healing_charge_milli, 40_000);
    assert_eq!(state(&battle, 1).charge_milli, 199_999);
    rules.on_unit_action_finished(&mut battle, 1, 0);
    assert_eq!(state(&battle, 1).healing_charge_milli, 0);
    assert!(state(&battle, 1).godslayer_queued);

    assert!(battle.start_extra_turn());
    rules.before_action(&mut battle, 1, Action::Skill, Some(2));
    assert_eq!(state(&battle, 1).charge_milli, 49_999);
    battle.spend_hp(1, 1_000, 1);
    battle.heal_unit(0, 1, 1_000);
    assert_eq!(state(&battle, 1).charge_milli, 49_999);
    assert_eq!(state(&battle, 1).healing_charge_milli, 0);
    rules.on_unit_action_finished(&mut battle, 1, 1);
    battle.spend_hp(1, 1_000, 1);
    assert_eq!(state(&battle, 1).charge_milli, 63_332);
}

#[test]
fn e4_heals_after_a_shielded_enemy_attack_and_taunt_expires_after_two_enemy_turns() {
    let mut plan = plan(&[gepard(), mydei(4, &[])], 4);
    finite(&mut plan, 40_000, 500);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Mydei::set_vendetta(&mut battle, 1, true);
    let rules = battle.mechanics(1).unwrap();
    assert_eq!(
        rules.crit_damage_bonus_bp(&battle, 1, 3, None, DamageKind::Direct),
        3_000
    );
    battle.units[1].energy_milli = 160_000;
    act(&mut battle, Action::Ultimate, CENTER);
    assert!(rules.taunts_enemy(&battle, 1, 2));
    assert!(rules.taunts_enemy(&battle, 1, 3));
    assert!(rules.taunts_enemy(&battle, 1, 4));
    assert!(!rules.taunts_enemy(&battle, 1, 5));

    battle.units[1].hp = 4_000;
    battle.apply_official_shield(0, 1, 2_000, 1, Some(Action::Ultimate));
    let charge_before = state(&battle, 1).charge_milli;
    battle.resolve_enemy(3, &ENEMY);
    assert_eq!(battle.hp(1), (4_750, 7_500));
    assert_eq!(state(&battle, 1).charge_milli - charge_before, 4_000);
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::EnemyAttack && event.target_id.as_deref() == Some(FORM)
    }));

    for turns in 1..=2 {
        battle.units[3].normal_turns = turns;
        battle.finish_turn(3);
        assert_eq!(rules.taunts_enemy(&battle, 1, 3), turns == 1);
    }
}

#[test]
fn e6_starts_transformed_and_spends_one_hundred_charge_without_using_a_normal_turn() {
    let plan = plan(&[gepard(), mydei(6, &[])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(GEPARD));
    assert_eq!(battle.hp(1), (7_500, 7_500));
    assert_eq!(state(&battle, 1).charge_milli, 0);
    battle.units[1].mydei_state.charge_milli = 80_000;
    battle.units[1].energy_milli = 160_000;
    act(&mut battle, Action::Ultimate, "dpr:1");
    assert_eq!(state(&battle, 1).charge_milli, 0);
    assert_eq!(battle.units[1].normal_turns, 0);
    assert_eq!(battle.ordinary_actor(), Some(GEPARD));
    assert!(
        battle
            .events()
            .iter()
            .any(|event| { event.kind == EventKind::MydeiEnhancedSkill { godslayer: true } })
    );
}

#[test]
fn vendetta_multiplies_a_teammates_hp_buff_in_damage_and_attribution() {
    let plan = plan(&[gepard(), mydei(0, &[])], 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Mydei::set_vendetta(&mut battle, 1, true);
    battle.apply_max_hp_increase(0, 1, 1_000, 2);
    assert_eq!(battle.hp(1), (9_000, 9_000));
    battle.ordinary_actor = Some(1);
    act(&mut battle, Action::Skill, "dpr:1");

    let first = &battle.dummy_damage[0];
    assert_eq!(first.amount, 5_940);
    assert_eq!(first.assists, [(0, 990)]);
}

#[test]
fn finite_victory_and_dpr_replay_share_the_automatic_actions_and_target_rng() {
    let team = [gepard(), mydei(6, &[A2, A4])];
    let mut finite_plan = plan(&team, 3);
    finite(&mut finite_plan, 15_000, 1_000);
    finite_plan.units[1].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&finite_plan, LogMode::Detailed);
    let mut commands = Vec::new();
    while battle.outcome() == Outcome::Ongoing && commands.len() < 60 {
        let action = battle
            .legal_actions()
            .into_iter()
            .min_by_key(|command| {
                (
                    command.actor_id != FORM || command.action != Action::Ultimate,
                    command.target_id.as_deref() != Some(CENTER),
                )
            })
            .unwrap();
        battle.apply(&action).unwrap();
        commands.push(action);
    }
    assert_eq!(battle.outcome(), Outcome::Victory);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::MydeiEnhancedSkill { godslayer: true })
    );
    let replay = Battle::replay(&finite_plan, &commands, LogMode::Detailed).unwrap();
    assert_eq!(replay.snapshot(), battle.snapshot());
    assert_eq!(replay.events(), battle.events());
    assert_eq!(
        Battle::replay(&finite_plan, &commands, LogMode::Off)
            .unwrap()
            .snapshot(),
        battle.snapshot()
    );

    let dpr_plan = plan(&team, 3);
    let calculate = || {
        calculate_dpr(
            &dpr_plan,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let first = calculate();
    assert_eq!(first, calculate());
    assert_eq!(
        first.total_damage,
        first
            .characters
            .iter()
            .map(|row| row.own_damage + row.assist_damage)
            .sum::<u64>()
    );
    assert!(first.targets.iter().flat_map(|row| &row.hits).any(|hit| {
        hit.source_id == FORM && hit.action == Action::Skill && hit.kind == DamageKind::Direct
    }));
}

#[test]
fn ultimate_windows_keep_one_locked_skill_target_and_reject_alternate_commands() {
    let mut plan = plan(&[mydei(6, &[])], 3);
    plan.units[0].starting_energy_milli = 160_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let actions = battle.legal_actions();
    let skills = actions
        .iter()
        .filter(|command| command.action == Action::Skill)
        .collect::<Vec<_>>();
    assert_eq!(skills.len(), 1);
    assert_eq!(
        actions
            .iter()
            .filter(|command| command.action == Action::Ultimate)
            .count(),
        3
    );
    let chosen = skills[0].target_id.as_deref().unwrap();
    let alternate = if chosen == "dpr:1" { "dpr:2" } else { "dpr:1" };
    let before = battle.snapshot();
    assert_eq!(
        battle.apply(&command(&battle, Action::Skill, alternate)),
        Err(BattleError::InvalidTarget)
    );
    assert_eq!(battle.snapshot(), before);
    act(&mut battle, Action::Ultimate, alternate);
    let next = battle
        .events()
        .iter()
        .find(|event| event.kind == EventKind::MydeiEnhancedSkill { godslayer: false })
        .unwrap();
    assert_eq!(next.target_id.as_deref(), Some(chosen));
}

#[test]
fn e6_can_queue_two_extra_turns_without_spending_the_interrupted_turn() {
    let plan = plan(&[gepard(), mydei(6, &[])], 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].mydei_state.charge_milli = 199_000;
    battle.units[1].energy_milli = 160_000;
    act(&mut battle, Action::Ultimate, CENTER);

    let enhanced = battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::MydeiEnhancedSkill { godslayer: true })
        .collect::<Vec<_>>();
    assert_eq!(enhanced.len(), 2);
    assert_eq!(enhanced[0].target_id.as_deref(), Some(CENTER));
    assert_eq!(enhanced[0].av_micros, enhanced[1].av_micros);
    assert_eq!(battle.units[1].normal_turns, 0);
    assert_eq!(battle.units[1].energy_milli, 25_000);
    assert_eq!(battle.ordinary_actor(), Some(GEPARD));
    assert_eq!(state(&battle, 1).charge_milli, 0);
    assert!(!state(&battle, 1).godslayer_queued);
}
