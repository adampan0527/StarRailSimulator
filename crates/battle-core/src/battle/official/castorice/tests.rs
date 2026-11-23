use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const TARGET: &str = "dpr:2";
const HERTA: &str = "character.1013";
const SPRITE: &str = "character.1407:netherwing";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

fn castorice(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: QUANTUM,
        },
        energy_capacity: 0,
        level: 80,
        stats: OfficialStats {
            hp: 10_000,
            base_hp: 10_000,
            attack: 100,
            base_attack: 100,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 200_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            elation_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.3, 0.5, 0.3]),
            (ULTIMATE.into(), vec![165.0, 3.0, 1.0, 0.2, 0.0]),
            (TALENT.into(), vec![0.0, 0.2, 3.0, 3.0]),
            (JOINT.into(), vec![0.4, 0.3, 0.5]),
            (CLAW.into(), vec![0.4]),
            (BREATH.into(), vec![0.25, 0.24, 0.28, 0.34, 0.25]),
            (BACKUP.into(), vec![5.0]),
            (ROAR.into(), vec![0.1, 3.0]),
            (DETONATION.into(), vec![0.4, 6.0, 0.06, 800.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn herta() -> OfficialCharacter {
    let mut member = castorice(0, &[]);
    member.unit_id = HERTA.into();
    member.form_id = HERTA.into();
    member.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    member.energy_capacity = 110;
    member.stats.hp = 12_000;
    member.stats.base_hp = 12_000;
    member.stats.base_speed_micros = 100_000_000;
    member.stats.speed_micros = 100_000_000;
    member.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    member
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn resolve(battle: &mut Battle<'_>, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: FORM.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

fn summon(battle: &mut Battle<'_>) -> usize {
    Castorice::set_newbud(battle, 0, NEWBUD_CAP);
    resolve(battle, Action::Ultimate, None);
    battle.active_memosprite(0).unwrap()
}

#[test]
fn validates_newbud_resource_and_uses_no_energy_pool() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    let mut invalid = castorice(0, &[]);
    invalid.energy_capacity = 100;
    assert!(PreparedBattle::official_dpr(&[invalid], 42, 3).is_err());
    let mut invalid = castorice(0, &[]);
    invalid.skill_parameters.remove(BREATH);
    assert!(PreparedBattle::official_dpr(&[invalid], 42, 3).is_err());
    let prepared = plan(&[castorice(0, &[])]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.gain_energy(0, 0, 999_000);
    assert_eq!(battle.units[0].energy_milli, 0);
    assert_eq!(battle.units[0].castorice_state.newbud, 10_200);
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|cmd| !cmd.action.is_ultimate())
    );
}

#[test]
fn skill_records_actual_loss_with_no_sp_and_trace_speed_follows_current_hp() {
    let prepared = plan(&[castorice(0, &[INVERTED_TORCH]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.speed_micros(0), 280_000_000);
    battle.units[1].hp = 2;
    battle.skill_points = 0;
    resolve(&mut battle, Action::Skill, Some(TARGET));
    assert_eq!([battle.units[0].hp, battle.units[1].hp], [7_000, 2]);
    assert_eq!(battle.units[0].castorice_state.newbud, 13_200);
    assert_eq!(battle.skill_points, 0);
    assert_eq!(
        battle
            .enemies()
            .map(|i| battle.units[i].toughness)
            .collect::<Vec<_>>(),
        [990, 980, 990]
    );
    battle.units[0].hp = 4_999;
    assert_eq!(battle.speed_micros(0), 200_000_000);
}

#[test]
fn overhealing_caps_each_target_and_resets_after_an_action() {
    let prepared = plan(&[castorice(4, &[DARK_TIDE]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.incoming_healing_amount(1, 1_000), 1_200);
    battle.heal_unit(1, 0, 5_000);
    battle.heal_unit(1, 0, 5_000);
    battle.heal_unit(1, 1, 1_000);
    assert_eq!(battle.units[0].castorice_state.newbud, 15_480);
    battle.notify_unit_action_finished(1);
    battle.heal_unit(1, 0, 5_000);
    assert_eq!(battle.units[0].castorice_state.newbud, 19_560);
}

#[test]
fn joint_skill_is_two_attacks_with_one_command_and_heals_dragon() {
    let prepared = plan(&[castorice(2, &[]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = summon(&mut battle);
    assert_eq!(battle.hp(sprite), (34_000, 34_000));
    assert_eq!(battle.speed_micros(sprite), 165_000_000);
    assert_eq!(battle.gauge_quanta(sprite), 0);
    battle.units[sprite].hp = 20_000;
    let attack_before = battle.attack_index;
    let decision_before = battle.decision_index;
    resolve(&mut battle, Action::Skill, None);
    assert_eq!(battle.attack_index - attack_before, 2);
    assert_eq!(battle.decision_index - decision_before, 1);
    assert_eq!(battle.units[sprite].hp, 28_800);
    assert_eq!(battle.units[0].castorice_state.newbud, STARTING_NEWBUD);
    assert_eq!(battle.units[0].castorice_state.ardent_will, 2);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| hit.actor)
            .collect::<Vec<_>>(),
        [0, 0, 0, sprite, sprite, sprite]
    );
    assert_eq!(
        battle
            .enemies()
            .map(|i| battle.units[i].toughness)
            .collect::<Vec<_>>(),
        [980; 3]
    );
}

#[test]
fn consecutive_breaths_are_one_attack_and_e2_adds_two_free_repeats() {
    for (eidolon, breaths, final_hits) in [(0, 4, 6), (2, 6, 6), (6, 6, 9)] {
        let prepared = plan(&[castorice(eidolon, &[DARK_TIDE, WEST_WIND]), herta()]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        let sprite = summon(&mut battle);
        battle.units[1].hp = 5_000;
        let attack_before = battle.attack_index;
        battle.mechanics(0).unwrap().on_summon_turn(&mut battle, 0);
        assert_eq!(battle.attack_index - attack_before, 2);
        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .filter(|hit| hit.actor == sprite)
                .count(),
            breaths * 3 + final_hits
        );
        assert_eq!(battle.active_memosprite(0), None);
        assert_eq!(battle.units[0].castorice_state.newbud, 0);
        assert_eq!(battle.units[1].hp, 6_400 + 280 * u32::from(eidolon >= 4));
    }
}

#[test]
fn hit_uses_owner_max_hp_and_credits_both_hp_and_sprite_crit_from_an_ally() {
    let prepared = plan(&[castorice(0, &[]), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = summon(&mut battle);
    battle.apply_max_hp_increase(1, 0, 2_000, 3);
    battle.apply_ally_buff(1, sprite, AllyBuffKind::CritRate, 10_000, 3);
    let target = battle.enemies().next().unwrap();
    battle.begin_attack();
    battle.memosprite_hp_hit(sprite, target, QUANTUM, 10_000, 0, Action::Skill);
    let hit = battle.dummy_damage.last().unwrap();
    // 12,000 owner HP, 50% CRIT DMG, 10% roar, 20% resistance reduction, 500 defense.
    assert_eq!(
        (hit.actor, hit.amount, hit.critical),
        (sprite, 14_256, true)
    );
    assert_eq!(hit.assists, [(1, 6_336)]);
}

#[test]
fn backup_cannot_be_randomly_targeted_and_absorbs_actual_lethal_overflow() {
    let mut prepared = plan(&[castorice(0, &[]), herta()]);
    let enemy = 2;
    prepared.units[enemy].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = summon(&mut battle);
    for _ in 0..10 {
        battle.resolve_enemy(enemy, &FINITE_ENEMY);
    }
    let attacks = battle
        .log
        .iter()
        .flatten()
        .filter(|event| matches!(event.kind, EventKind::EnemyAttack));
    assert_eq!(attacks.clone().count(), 10);
    assert!(
        attacks
            .map(|event| event.target_id.as_deref())
            .all(|target| target != Some(SPRITE))
    );
    battle.units[1].hp = 100;
    battle.units[sprite].hp = 700;
    battle.true_damage(enemy, 1, Action::Skill, 200, Vec::new());
    assert_eq!((battle.units[1].hp, battle.units[sprite].hp), (1, 294));
    battle.true_damage(enemy, 1, Action::Skill, 200, Vec::new());
    assert_eq!((battle.units[1].hp, battle.units[sprite].hp), (1, 1));
    battle.notify_unit_action_finished(enemy);
    assert!(battle.active_memosprite(0).is_none());
}

#[test]
fn dpr_replay_keeps_netherwing_as_source_and_credits_owner_and_territory() {
    let mut owner = castorice(2, &[DARK_TIDE, INVERTED_TORCH, WEST_WIND]);
    owner.stats.hp = 28_000;
    owner.stats.base_hp = 28_000;
    let mut friend = herta();
    friend.stats.hp = 28_000;
    friend.stats.base_hp = 28_000;
    let prepared = plan(&[owner, friend]);
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = result.targets.iter().flat_map(|target| &target.hits);
    assert!(
        hits.clone().any(|hit| hit.source_id == SPRITE
            && hit.credits.iter().any(|credit| credit.unit_id == FORM))
    );
    assert!(
        hits.filter(|hit| hit.source_id == HERTA)
            .any(|hit| hit.credits.iter().any(|credit| credit.unit_id == FORM))
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}

#[test]
fn mooncocoon_waits_for_healing_and_triggers_once_per_battle() {
    let mut prepared = plan(&[castorice(0, &[]), herta()]);
    let enemy = 2;
    prepared.units[enemy].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[0].hp = 100;
    battle.units[1].hp = 100;
    battle.true_damage(enemy, 0, Action::Skill, 200, Vec::new());
    battle.true_damage(enemy, 1, Action::Skill, 200, Vec::new());
    assert_eq!((battle.units[0].hp, battle.units[1].hp), (1, 1));
    battle.heal_unit(0, 0, 123);
    battle.notify_unit_action_finished(enemy);
    assert_eq!((battle.units[0].hp, battle.units[1].hp), (124, 0));
    battle.true_damage(enemy, 0, Action::Skill, 200, Vec::new());
    assert_eq!(battle.units[0].hp, 0);
}

#[test]
fn sixth_eidolon_detonates_when_lifetime_ends_and_depletes_non_quantum_toughness() {
    let mut prepared = plan(&[castorice(6, &[])]);
    let enemy = 1;
    prepared.units[enemy].kit = Kit::Enemy(&FINITE_ENEMY);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = summon(&mut battle);
    battle.begin_attack();
    battle.memosprite_hp_hit(sprite, enemy, QUANTUM, 1_000, 10, Action::Skill);
    assert_eq!(battle.units[enemy].toughness, 990);
    let rules = battle.mechanics(0).unwrap();
    battle.units[sprite].normal_turns = 2;
    rules.after_battle_turn(&mut battle, 0, sprite, false);
    assert_eq!(battle.active_memosprite(0), Some(sprite));
    battle.units[sprite].normal_turns = 3;
    rules.after_battle_turn(&mut battle, 0, sprite, false);
    assert_eq!(battle.active_memosprite(0), None);
    assert_eq!(
        battle.dummy_damage.len() + usize::from((990 - battle.units[enemy].toughness) / 5),
        9
    );
}

#[test]
fn finite_wave_clear_preserves_dragon_and_its_next_turn_acceleration() {
    let mut prepared = plan(&[castorice(0, &[INVERTED_TORCH])]);
    let enemy = 1;
    prepared.units[enemy].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.wave_ends = vec![2, 4];
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = summon(&mut battle);
    battle.units[enemy].hp = 1;
    battle.units[sprite].normal_turns = 1;
    battle.mechanics(0).unwrap().on_summon_turn(&mut battle, 0);
    assert_eq!(battle.units[enemy].hp, 0);
    assert_eq!(battle.units[sprite].hp, 25_500);
    battle.finish_turn(sprite);
    battle.resolve_wave_or_end();
    assert_eq!(battle.wave_index, 1);
    assert_eq!(battle.active_memosprite(0), Some(sprite));
    assert_eq!(battle.speed_micros(sprite), 330_000_000);
}
