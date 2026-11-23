use super::*;

use crate::{
    CombatType, Command, DamageKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const MEM: &str = "trailblazer.remembrance";

fn character(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        form_id: FORM.into(),
        unit_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: Element::Ice,
        },
        level: 80,
        energy_capacity: 240,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 500,
            base_attack: 500,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
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
            (BASIC, vec![0.5]),
            (SKILL, vec![0.24, 2.0, 2.0, 0.5, 12.0, 0.1]),
            (ULTIMATE, vec![2.0, 2.0, 0.6, 0.3]),
            (TALENT, vec![2.0, 0.6, 2.0, 160.0, 0.5, 16.0, 0.05]),
            (EVEY_SKILL, vec![0.5, 0.1, 4.0, 1.0]),
            (EVEY_TALENT, vec![0.5, 3.0]),
            (EVEY_DEPARTURE, vec![0.1, 0.01, 40.0]),
            (EVEY_BURST, vec![0.12, 0.06, 16.0]),
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
    ally.unit_id = MEM.into();
    ally.form_id = MEM.into();
    ally.energy_capacity = 160;
    ally.stats.crit_rate_bp = 10_000;
    ally.stats.speed_micros = 180_000_000;
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

fn memoria(battle: &Battle<'_>) -> u32 {
    battle.units[0].evernight_state.memoria
}

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, FORM, Action::Ultimate, None);
}

#[test]
fn validates_frozen_conditions_and_uses_immediate_independent_opening() {
    let mut bad = character(0, &[]);
    bad.skill_parameters.get_mut(EVEY_BURST).unwrap()[2] = 15.0;
    assert!(build(&bad).is_err());
    let prepared = plan(&[character(0, &[])], &[1_000]);
    let battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.hp(sprite), (1_000, 1_000));
    assert_eq!(battle.speed_micros(sprite), 160_000_000);
    assert_eq!(battle.enemy_targeting_weight(sprite), 125 * 40_000);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(memoria(&battle), 1);
    let hit = &battle.dummy_damage[0];
    assert_eq!((hit.actor, hit.amount, hit.av_micros), (sprite, 450, 0));
    assert_eq!(battle.units[1].toughness, 990);
}

#[test]
fn zero_sp_skill_pays_current_hp_and_heals_the_existing_sprite() {
    let prepared = plan(&[character(0, &[])], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.spend_hp(sprite, 600, 1);
    let previous_memoria = memoria(&battle);
    battle.skill_points = 0;
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.skill_points, 0);
    assert_eq!(battle.hp(0), (1_800, 2_000));
    assert_eq!(battle.hp(sprite), (900, 1_000));
    assert_eq!(memoria(&battle), previous_memoria + 5);
    assert_eq!(battle.units[sprite].memosprite_generation, 1);
    let crit = battle.effective_crit_damage_bp(0);
    assert_eq!(crit, 11_000);
    assert_eq!(battle.effective_crit_damage_bp(sprite), 11_000 + 2_640);
}

#[test]
fn threshold_advances_evey_once_and_burst_consumes_memoria_with_distinct_targets() {
    let prepared = plan(&[character(0, &[])], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.units[0].evernight_state.last_target = Some(2);
    Evernight::set_memoria(&mut battle, 0, 14);
    battle.spend_hp(0, 1, 1);
    assert_eq!(memoria(&battle), 16);
    assert_eq!(battle.gauge_quanta(sprite), 0);
    assert!(battle.units[0].evernight_state.burst_advance_used);
    battle.units[1..4]
        .iter_mut()
        .for_each(|unit| unit.toughness = 1_000);
    battle.dummy_damage.clear();
    let rules = battle.mechanics(0).unwrap();
    rules.on_summon_turn(&mut battle, 0);
    let hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.amount))
        .collect();
    assert_eq!(hits, [(2, 1_728), (1, 864), (3, 864)]);
    assert_eq!(battle.units[2].toughness, 970);
    assert_eq!(battle.units[1].toughness, 980);
    assert_eq!(memoria(&battle), 2);
    assert!(battle.active_memosprite(0).is_none());
    assert!(!battle.units[0].evernight_state.burst_advance_used);
    assert_eq!(rules.speed_bonus_bp(&battle, 0), 2_600);
}

#[test]
fn ultimate_uses_eveys_hp_and_accumulates_charges_until_owner_turn_start() {
    let prepared = plan(&[character(2, &[])], &[1_000; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.dummy_damage.clear();
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].evernight_state.darkest_riddle, Some(4));
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.dummy_damage.len(), 2);
    assert!(
        battle.dummy_damage.iter().all(|hit| {
            (hit.actor, hit.action, hit.amount) == (sprite, Action::Ultimate, 2_340)
        })
    );
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].evernight_state.darkest_riddle, Some(8));
    let rules = battle.mechanics(0).unwrap();
    battle.units[0].evernight_state.darkest_riddle = Some(1);
    Evernight::set_memoria(&mut battle, 0, 16);
    rules.on_summon_turn(&mut battle, 0);
    assert_eq!(battle.units[0].evernight_state.darkest_riddle, Some(0));
    assert!(rules.immune_to_enemy_control(&battle, 0));
    rules.on_ally_turn_opening(&mut battle, 0, 0, true);
    assert_eq!(battle.units[0].evernight_state.darkest_riddle, Some(0));
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(battle.units[0].evernight_state.darkest_riddle, None);
    assert!(!rules.immune_to_enemy_control(&battle, 0));
    assert_eq!(rules.speed_bonus_bp(&battle, 0), 0);
}

#[test]
fn a2_a4_and_e6_apply_each_gain_and_refund_exactly_once() {
    let prepared = plan(
        &[character(6, &[DARK_THE_NIGHT, ROUSE_THE_FLAME])],
        &[1_000],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(memoria(&battle), 13);
    assert_eq!(battle.hp(sprite), (950, 1_000));
    assert_eq!(battle.units[0].energy_milli, 215_000);
    act(&mut battle, FORM, Action::Skill, None);
    assert_eq!(battle.hp(0), (1_710, 2_000));
    assert!(battle.active_memosprite(0).is_none());
    assert_eq!(battle.skill_points, 4);
    assert_eq!(memoria(&battle), 16);
    assert!(battle.log.iter().flatten().any(|event| matches!(
        event.kind,
        EventKind::Memoria {
            before: 35,
            after: 0
        }
    )));
}

static FINITE: EnemyKit = EnemyKit {
    stats: Stats {
        hp: 30_000,
        attack: 100,
        ..DUMMY_STATS
    },
    max_toughness: 1_000,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

#[test]
fn real_received_attack_counts_once_per_unit_but_separate_hp_costs_count() {
    let mut prepared = plan(&[character(0, &[])], &[1_000]);
    prepared.units[1].kit = Kit::Enemy(&FINITE);
    prepared.units[1].stats = FINITE.stats;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    let start = memoria(&battle);
    battle.enemy_action = Some(1);
    battle.begin_attack();
    for target in [0, 0, sprite, sprite] {
        battle.attack_hit(1, target, Element::Physical, 10_000, 0);
    }
    assert!(battle.units[0].hp < 2_000 && battle.units[sprite].hp < 1_000);
    assert_eq!(memoria(&battle), start + 4);
    battle.begin_attack();
    battle.attack_hit(1, sprite, Element::Physical, 10_000, 0);
    battle.spend_hp(sprite, 1, 1);
    battle.spend_hp(sprite, 1, 1);
    assert_eq!(memoria(&battle), start + 10);
}

#[test]
fn skill_support_reaches_real_memosprites_and_e1_e4_keep_sources_separate() {
    let prepared = plan(
        &[character(4, &[KINDLE_THE_MORN]), remembrance()],
        &[1_000; 2],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    act(&mut battle, MEM, Action::Skill, None);
    let evey = battle.active_memosprite(0).unwrap();
    let mem = battle.active_memosprite(1).unwrap();
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.ally_crit_damage_bonus_bp(&battle, 0, 1), 0);
    assert_eq!(
        rules.ally_crit_damage_bonus_bp(&battle, 0, mem),
        3_600 + 1_500
    );
    assert_eq!(rules.ally_toughness_bonus_bp(&battle, 0, evey), 5_000);
    assert_eq!(rules.ally_toughness_bonus_bp(&battle, 0, mem), 2_500);
    assert_eq!(rules.ally_memosprite_damage_factor_bp(&battle, 0), 13_000);
    battle.units[3].hp = 0;
    assert_eq!(rules.ally_memosprite_damage_factor_bp(&battle, 0), 15_000);
}

#[test]
fn evey_uses_its_summoned_max_hp_and_credits_a_teammates_hp_buff() {
    use crate::battle::support::AllyBuffKind;

    let prepared = plan(&[character(0, &[]), remembrance()], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let sprite = battle.active_memosprite(0).unwrap();
    battle.units[sprite].memosprite_max_hp = Some(1_200);
    battle.apply_ally_buff(1, sprite, AllyBuffKind::MaxHpFlat, 400, 2);
    assert_eq!(battle.effective_max_hp(sprite), 1_600);
    battle.dummy_damage.clear();
    battle.begin_attack();
    battle.memosprite_hp_hit(sprite, 2, Element::Ice, 5_000, 0, Action::Skill);
    let hit = &battle.dummy_damage[0];
    assert_eq!((hit.actor, hit.amount), (sprite, 720));
    assert_eq!(hit.assists, [(1, 180)]);
}

#[test]
fn dpr_replays_ice_attacks_and_credits_evernight_on_a_separate_sprite() {
    let prepared = plan(
        &[
            character(2, &[DARK_THE_NIGHT, ROUSE_THE_FLAME, KINDLE_THE_MORN]),
            remembrance(),
        ],
        &STANDARD_DPR_SCENARIO.toughness,
    );
    let scenario = STANDARD_DPR_SCENARIO;
    let run = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == "character.1413:evey"
        && hit.action == Action::Ultimate
        && hit.kind == DamageKind::Direct));
    assert!(hits().any(|hit| {
        hit.source_id == "trailblazer.remembrance:mem"
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == FORM && credit.amount > 0)
    }));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}
