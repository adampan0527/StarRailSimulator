use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const FIRST: &str = "dpr:1";
const REMEMBRANCE: &str = "trailblazer.remembrance";

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        form_id: FORM.into(),
        unit_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: PHYSICAL,
        },
        level: 80,
        energy_capacity: 0,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 125,
            base_speed_micros: 100_000_000,
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
            (BASIC, vec![1.0]),
            (SKILL, vec![3.0, 1.2, 2.0]),
            (ULTIMATE, vec![9.6, 12.0, 0.6, 8.0]),
            (TALENT, vec![0.3, 3.0, 3.0, 12.0]),
            (VESSEL, vec![4.0, 0.25, 0.125, 0.8, 2.7, 0.15, 0.2]),
            (BLOODTHORN, vec![2.5, 0.75, 2.0]),
            (CALAMITY, vec![0.4, 0.75, 4.0, 0.3, 0.2]),
            (VERDICT, vec![4.5, 0.45, 4.0, 4.0, 11.7]),
        ]
        .into_iter()
        .map(|(id, values)| (id.into(), values))
        .collect(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn remembrance() -> OfficialCharacter {
    let mut ally = character(0, &[]);
    ally.unit_id = REMEMBRANCE.into();
    ally.form_id = REMEMBRANCE.into();
    ally.combat_type = CombatType {
        path: CombatPath::Remembrance,
        element: Element::Ice,
    };
    ally.energy_capacity = 160;
    ally.stats.speed_micros = 300_000_000;
    ally.skill_parameters = [
        ("skill.800701", vec![1.0]),
        ("skill.800702", vec![0.6, 0.1]),
        ("skill.800703", vec![2.4, 0.4]),
        ("skill.800704", vec![130.0, 0.8, 10.0, 640.0]),
        ("skill.800708", vec![1.2, 1.2, 0.1]),
        ("skill.1800701", vec![0.36, 4.0, 0.9]),
        ("skill.1800703", vec![0.12, 0.24]),
        ("skill.1800705", vec![0.5]),
        ("skill.1800706", vec![0.25]),
        ("skill.1800707", vec![0.28, 3.0, 1.0]),
    ]
    .into_iter()
    .map(|(id, values)| (id.into(), values))
    .collect();
    ally
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = Command {
        decision_index: battle.decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    };
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.apply(&command).unwrap();
}

fn transform(battle: &mut Battle<'_>, coreflame: u32) {
    Phainon::set_coreflame(battle, 0, coreflame);
    act(battle, FORM, Action::Ultimate, None);
}

#[test]
fn waits_for_twelve_coreflames_and_uses_real_support_or_enemy_targeting() {
    let prepared = plan(
        &[character(0, &[OBLIVION, FLAMES, VALOR]), remembrance()],
        &[1_000; 2],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[0].phainon_state.coreflame, 1);
    assert_eq!(battle.effective_attack(0), 1_500);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action.is_ultimate())
    );
    battle.notify_ally_targets(1, Some(0), Target::Ally, Action::Skill);
    battle.restore_energy_flat(1, 0, 0);
    battle.notify_ally_targets(0, None, Target::SelfOnly, Action::Skill);
    assert_eq!(battle.units[0].phainon_state.coreflame, 3);
    battle.heal_unit(1, 0, 100);
    battle.apply_official_shield(1, 0, 100, 2, Some(Action::Skill));
    assert_eq!(battle.ally_buff_value(0, 0, AllyBuffKind::Damage), 4_500);
    act(&mut battle, REMEMBRANCE, Action::Basic, Some(FIRST));
    act(&mut battle, FORM, Action::Skill, Some(FIRST));
    assert_eq!(battle.units[0].phainon_state.coreflame, 5);
    assert_eq!(battle.units[2].toughness, 970);
}

#[test]
fn territory_freezes_a_real_memosprite_and_restores_its_preexisting_gauge() {
    let prepared = plan(
        &[character(0, &[OBLIVION, VALOR]), remembrance()],
        &[1_000; 2],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, REMEMBRANCE, Action::Skill, None);
    let sprite = battle.active_memosprite(1).unwrap();
    let sprite_gauge = battle.gauge_quanta(sprite);
    let friend_gauge = battle.gauge_quanta(1);
    let friend_turns = battle.units[1].normal_turns;
    let sprite_turns = battle.units[sprite].normal_turns;
    transform(&mut battle, 15);
    assert_eq!(battle.territory_owner, Some(0));
    assert!(!battle.is_targetable(1));
    assert!(!battle.is_targetable(sprite));
    assert_eq!(battle.hp(0), (3_700, 3_700));
    assert_eq!(battle.effective_attack(0), 2_300);
    assert_eq!(battle.units[0].countdown.unwrap().speed_micros, 60_000_000);
    assert_eq!(battle.units[0].phainon_state.remaining_turns, 7);
    let before_av = battle.av_micros;
    for _ in 0..6 {
        act(&mut battle, FORM, Action::Basic, Some(FIRST));
    }
    assert!(battle.av_micros > before_av);
    assert_eq!(battle.gauge_quanta(sprite), sprite_gauge);
    assert_eq!(battle.gauge_quanta(1), friend_gauge);
    assert_eq!(battle.units[1].normal_turns, friend_turns);
    assert_eq!(battle.units[sprite].normal_turns, sprite_turns);
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.skill_points, 2);
    act(&mut battle, FORM, Action::Basic, Some(FIRST));
    assert_eq!(battle.territory_owner, None);
    assert_eq!(battle.active_memosprite(1), Some(sprite));
    assert_eq!(battle.units[0].phainon_state.coreflame, 6);
    assert_eq!(battle.hp(0), (1_000, 1_000));
    assert_eq!(battle.effective_attack(0), 2_000);
    let ending_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.action == Action::Ultimate && hit.actor == 0)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(ending_hits, [6_624, 6_624]);
}

#[test]
fn territory_pauses_the_aha_clock_while_all_elation_allies_are_absent() {
    let yao = super::super::yao_guang::tests::yao(0, &[]);
    let prepared = plan(&[character(0, &[]), yao], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let aha_gauge = battle.aha.as_ref().unwrap().gauge_quanta;
    transform(&mut battle, 12);
    assert_eq!(battle.territory_owner, Some(0));
    assert_eq!(battle.aha_speed_micros(), None);

    let transformed_av = battle.av_micros;
    for _ in 0..6 {
        act(&mut battle, FORM, Action::Basic, Some(FIRST));
    }
    assert!(battle.av_micros > transformed_av);
    assert_eq!(battle.aha.as_ref().unwrap().gauge_quanta, aha_gauge);
    assert!(
        battle
            .log
            .iter()
            .flatten()
            .all(|event| !matches!(event.kind, EventKind::AhaInstant { .. }))
    );
    act(&mut battle, FORM, Action::Basic, Some(FIRST));
    assert_eq!(battle.territory_owner, None);
    assert!(battle.aha_speed_micros().is_some());
}

#[test]
fn cerydra_copies_one_full_verdict_while_temporarily_absent() {
    let cerydra = super::super::cerydra::tests::character(0, &[]);
    let prepared = plan(&[character(6, &[]), cerydra], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    super::super::cerydra::tests::grant_merit(&mut battle, 1, 0, 8);
    transform(&mut battle, 12);
    assert_eq!(battle.territory_owner, Some(0));
    assert!(!battle.is_targetable(1));
    let av = battle.av_micros;
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.units[0].phainon_state.scourge, 0);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(super::super::cerydra::tests::charge(&battle, 1), 2);
    let hits = &battle.dummy_damage;
    assert_eq!(hits.len(), 36);
    assert_eq!(
        hits.iter()
            .filter(|hit| hit.kind == DamageKind::TrueDamage)
            .count(),
        2
    );
    assert!(
        hits[..18]
            .iter()
            .all(|hit| hit.assists == [(1, hit.amount)])
    );
    assert_eq!(
        battle
            .log
            .iter()
            .flatten()
            .filter(|event| event.kind == EventKind::ExtraTurnStarted)
            .count(),
        2
    );
    assert_eq!(
        battle
            .log
            .iter()
            .flatten()
            .filter(|event| event.kind == EventKind::TriggeredSkill)
            .count(),
        1
    );
}

#[test]
fn cerydra_does_not_copy_calamitys_forced_enemy_actions() {
    let cerydra = super::super::cerydra::tests::character(0, &[]);
    let prepared = plan(&[character(0, &[]), cerydra], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    super::super::cerydra::tests::grant_merit(&mut battle, 1, 0, 6);
    transform(&mut battle, 12);
    assert!(!battle.is_targetable(1));
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(super::super::cerydra::tests::charge(&battle, 1), 7);
    assert_eq!(battle.dummy_damage.len(), 5);
    assert!(
        battle
            .log
            .iter()
            .flatten()
            .all(|event| event.kind != EventKind::TriggeredSkill)
    );
}

#[test]
fn enhanced_skill_spends_scourge_ignores_skill_points_and_uses_the_e2_extra_turn() {
    let prepared = plan(&[character(6, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle, 12);
    assert!(battle.in_extra_turn(0));
    let av = battle.av_micros;
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.units[0].phainon_state.scourge, 0);
    assert_eq!(battle.units[0].phainon_state.remaining_turns, 7);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|command| command.action == Action::EnhancedSkill)
    );
    assert_eq!(battle.skill_points, 3);
    let (last, prior) = battle.dummy_damage.split_last().unwrap();
    assert_eq!(prior.len(), 17);
    assert_eq!(last.kind, DamageKind::TrueDamage);
    assert_eq!(
        last.amount,
        prior.iter().map(|hit| hit.amount).sum::<u32>() * 36 / 100
    );
    assert_eq!(battle.units[1].toughness, 927);
}

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 1_000,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

#[test]
fn calamity_forces_each_enemy_turn_and_uses_during_attack_damage_reduction() {
    let mut prepared = plan(&[character(0, &[])], &[1_000; 2]);
    for index in 1..=2 {
        prepared.units[index].kit = Kit::Enemy(&FINITE_ENEMY);
        prepared.units[index].stats.hp = 100_000;
        prepared.units[index].stats.attack = 1_000;
    }
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle, 12);
    assert!(battle.has_weakness(1, PHYSICAL));
    let av = battle.av_micros;
    act(&mut battle, FORM, Action::Skill, None);
    let log = battle.log.as_ref().unwrap();
    let enemy_hits: Vec<_> = log
        .iter()
        .filter(|event| event.av_micros == av && event.target_id.as_deref() == Some(FORM))
        .filter_map(|event| match event.kind {
            EventKind::Damage { amount, .. } => Some(amount),
            _ => None,
        })
        .collect();
    // Usual unbroken target damage is 1,000 × 0.6 × 0.85; Soulscorch retains 25%.
    assert_eq!(enemy_hits, [127, 127]);
    assert_eq!(
        log.iter()
            .filter(|event| event.av_micros == av && matches!(event.kind, EventKind::EnemyAttack))
            .count(),
        2
    );
    assert_eq!(battle.units[0].phainon_state.scourge, 6);
    assert_eq!(battle.units[0].phainon_state.soulscorch, 0);
    assert!(battle.units[1].toughness < 1_000);
    assert!(battle.units[2].toughness < 1_000);
}

#[test]
fn lethal_hit_runs_scaled_finisher_and_returns_from_territory() {
    let mut prepared = plan(&[character(0, &[OBLIVION])], &[1_000]);
    prepared.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[1].stats.hp = 100_000;
    prepared.units[1].stats.attack = 1_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle, 12);
    battle.units[0].hp = 1;
    let before = battle.units[1].hp;
    battle.resolve_enemy(1, &FINITE_ENEMY);
    assert_eq!(battle.territory_owner, None);
    assert!(!battle.has_weakness(1, PHYSICAL));
    assert_eq!(battle.units[0].hp, 925);
    assert_eq!(before - battle.units[1].hp, 1_296);
}

#[test]
fn defeating_a_wave_keeps_territory_and_updates_e1_countdown_speed() {
    let mut prepared = plan(&[character(2, &[])], &[1_000; 2]);
    prepared.wave_ends = vec![2, 3];
    for index in 1..=2 {
        prepared.units[index].kit = Kit::Enemy(&FINITE_ENEMY);
        prepared.units[index].stats.hp = if index == 1 { 1 } else { 100_000 };
    }
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    transform(&mut battle, 12);
    let av = battle.av_micros;
    assert_eq!(battle.units[0].countdown.unwrap().speed_micros, 66_000_000);
    act(&mut battle, FORM, Action::EnhancedSkill, None);
    assert_eq!(battle.wave_index, 1);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.territory_owner, Some(0));
    assert!(battle.has_weakness(2, PHYSICAL));
    assert_eq!(battle.units[0].countdown.unwrap().speed_micros, 67_500_000);
    assert_eq!(battle.units[0].phainon_state.remaining_turns, 7);
}

#[test]
fn fixed_dpr_replays_the_transformation_with_conserved_attribution() {
    let prepared = plan(
        &[character(6, &[OBLIVION, FLAMES, VALOR]), remembrance()],
        &[30, 120, 30],
    );
    let run = || {
        calculate_dpr(
            &prepared,
            &STANDARD_DPR_SCENARIO.toughness,
            STANDARD_DPR_SCENARIO.cycles,
        )
        .unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.source_id == FORM
                && hit.action == Action::EnhancedSkill
                && hit.kind == DamageKind::TrueDamage)
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|row| row.own_damage + row.assist_damage)
            .sum::<u64>()
    );
}
