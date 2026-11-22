use crate::{
    CombatType, EventKind, LogMode, OfficialStats, Outcome, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::*;

const LEFT: &str = "dpr:1";
const CENTER: &str = "dpr:2";
const ARGENTI: &str = "character.1302";
const ACHERON: &str = "character.1308";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn tribbie(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Harmony,
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
            base_aggro: 100,
            base_speed_micros: 96_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (
                BASIC.into(),
                if eidolon >= 3 {
                    vec![0.33, 0.165]
                } else {
                    vec![0.3, 0.15]
                },
            ),
            (
                SKILL.into(),
                vec![if eidolon >= 5 { 0.264 } else { 0.24 }, 3.0],
            ),
            (
                ULTIMATE.into(),
                if eidolon >= 3 {
                    vec![0.33, 0.33, 0.132, 2.0]
                } else {
                    vec![0.3, 0.3, 0.12, 2.0]
                },
            ),
            (TALENT.into(), vec![if eidolon >= 5 { 0.198 } else { 0.18 }]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn argenti() -> OfficialCharacter {
    let mut ally = tribbie(0, &[]);
    ally.unit_id = ARGENTI.into();
    ally.form_id = ARGENTI.into();
    ally.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Physical,
    };
    ally.energy_capacity = 180;
    ally.stats.speed_micros = 103_000_000;
    ally.skill_parameters = [
        ("skill.130201".into(), vec![1.0]),
        ("skill.130202".into(), vec![1.2]),
        ("skill.130203".into(), vec![1.6, 90.0]),
        ("skill.130204".into(), vec![3.0, 0.025, 10.0]),
        ("skill.130214".into(), vec![2.8, 6.0, 0.95, 180.0]),
    ]
    .into();
    ally
}

fn acheron() -> OfficialCharacter {
    let mut ally = tribbie(0, &[]);
    ally.unit_id = ACHERON.into();
    ally.form_id = ACHERON.into();
    ally.combat_type = CombatType {
        path: CombatPath::Nihility,
        element: Element::Thunder,
    };
    ally.energy_capacity = 0;
    ally.stats.speed_micros = 101_000_000;
    ally.skill_parameters = [
        ("skill.130801".into(), vec![0.5]),
        ("skill.130802".into(), vec![1.6, 0.6, 1.0]),
        (
            "skill.130803".into(),
            vec![0.24, 0.15, 1.2, 9.0, 0.6, 3.72, 3.0],
        ),
        ("skill.130804".into(), vec![9.0, 0.2]),
        ("skill.130814".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130815".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130816".into(), vec![0.24, 0.15, 1.2]),
        ("skill.130817".into(), vec![1.2]),
    ]
    .into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400; 3])
        .unwrap();
    for unit in &mut plan.units[team.len()..] {
        unit.stats.defense = 1_000;
    }
    plan
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn open_zone(battle: &mut Battle<'_>) {
    battle.units[0].energy_milli = 120_000;
    act(battle, FORM, Action::Ultimate, None);
}

fn damage_hits(battle: &Battle<'_>, kind: DamageKind) -> Vec<(usize, usize, u32)> {
    battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == kind)
        .map(|hit| (hit.actor, hit.target, hit.amount))
        .collect()
}

#[test]
fn base_form_validates_frozen_parameters_and_basic_scales_from_hp() {
    let mut input = tribbie(0, &[]);
    input.stats.attack = 4_000;
    let plan = plan(&[input]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some(CENTER));
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 80_000);
    assert_eq!(
        damage_hits(&battle, DamageKind::Direct),
        [(0, 2, 675), (0, 1, 337), (0, 3, 337)]
    );
    assert_eq!(
        battle.units[1..]
            .iter()
            .map(|enemy| enemy.toughness)
            .collect::<Vec<_>>(),
        [395, 390, 395]
    );
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut invalid = tribbie(0, &[]);
    invalid.skill_parameters.get_mut(SKILL).unwrap()[1] = 2.0;
    assert!(build(&invalid).is_err());
}

#[test]
fn opening_zone_increases_max_hp_before_ultimate_and_follows_owner_turns() {
    let plan = plan(&[tribbie(0, &[GLASS_BALL_WITH_WINGS]), argenti()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].hp = 4_200;
    open_zone(&mut battle);
    assert_eq!(battle.effective_max_hp(0), 5_900);
    assert_eq!(battle.units[0].hp, 5_100);
    assert_eq!(battle.effective_max_hp(1), 5_000);
    assert_eq!(
        damage_hits(&battle, DamageKind::Direct),
        [(0, 2, 1_035), (0, 3, 1_035), (0, 4, 1_035)]
    );
    assert_eq!(
        damage_hits(&battle, DamageKind::Additional),
        [(0, 2, 414); 3]
    );
    battle.apply_max_hp_increase(1, 1, 1_000, 2);
    assert_eq!(battle.effective_max_hp(0), 5_990);

    let rules = battle.mechanics(0).unwrap();
    battle.units[1].normal_turns += 2;
    rules.on_ally_turn_opening(&mut battle, 0, 1, false);
    rules.on_ally_turn_opening(&mut battle, 0, 0, true);
    assert!(zone_active(&battle, 0));
    battle.units[0].normal_turns += 1;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert!(zone_active(&battle, 0));
    battle.units[0].hp = battle.effective_max_hp(0);
    battle.units[0].normal_turns += 1;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert!(!zone_active(&battle, 0));
    assert_eq!(battle.effective_max_hp(0), 5_000);
    assert_eq!(battle.units[0].hp, 5_000);
}

#[test]
fn skill_at_e4_credits_tribbie_for_all_type_penetration_and_defense_ignore() {
    let mut plan = plan(&[tribbie(4, &[]), argenti()]);
    for enemy in &mut plan.units[2..] {
        enemy.stats.resist_bp[Element::Physical.index()] = 2_000;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, None);
    battle.ordinary_actor = Some(1);
    act(&mut battle, ARGENTI, Action::Basic, Some(CENTER));
    assert_eq!(battle.units[0].energy_milli, 90_000);
    assert_eq!(damage_hits(&battle, DamageKind::Direct), [(1, 3, 514)]);
    assert_eq!(battle.dummy_damage[0].assists, [(0, 154)]);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(
        rules.damage_target_aura(&battle, 0, 1).defense_down_bp,
        1_800
    );
    battle.units[0].normal_turns = battle.units[0].tribbie_state.numinosity_until;
    assert!(rules.damage_target_aura(&battle, 0, 1) == DamageTargetAura::default());
}

#[test]
fn e2_adds_one_zone_hit_and_e1_uses_all_attack_damage_once() {
    let plan = plan(&[tribbie(2, &[PEBBLE_AT_CROSSROADS]), argenti()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 90_000);
    open_zone(&mut battle);
    battle.dummy_damage.clear();
    battle.units[2].hp = 2_000;
    battle.units[3].hp = 4_000;
    battle.units[4].hp = 9_000;
    battle.units[1].energy_milli = 180_000;
    act(&mut battle, ARGENTI, Action::EnhancedUltimate, None);
    let attack_hits = &battle.dummy_damage[..14];
    let other_total: u32 = attack_hits[..9].iter().map(|hit| hit.amount).sum();
    assert!(
        attack_hits[..9]
            .iter()
            .all(|hit| hit.actor == 1 && hit.kind == DamageKind::Direct)
    );
    assert!(attack_hits[9..13].iter().all(|hit| hit.actor == 0
        && hit.target == 4
        && hit.kind == DamageKind::Additional
        && hit.amount == 421));
    assert_eq!(attack_hits[13].kind, DamageKind::TrueDamage);
    assert_eq!(attack_hits[13].target, 4);
    assert_eq!(attack_hits[13].actor, 0);
    assert_eq!(
        attack_hits[13].amount,
        percentage_of(other_total + 4 * 421, 2_400)
    );
    assert_eq!(battle.units[0].energy_milli, 14_500);
}

#[test]
fn each_other_ult_slot_triggers_once_until_tribbie_ult_and_e6_triggers_her_own() {
    let plan = plan(&[tribbie(6, &[LAMB_OUTSIDE_THE_WALL]), argenti()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].energy_milli = 90_000;
    act(&mut battle, ARGENTI, Action::Ultimate, None);
    let hits = damage_hits(&battle, DamageKind::FollowUp);
    assert_eq!(hits, [(0, 2, 3_693), (0, 3, 3_693), (0, 4, 3_693)]);
    assert_eq!(battle.units[0].tribbie_state.lamb_stacks, 1);
    battle.units[1].energy_milli = 180_000;
    act(&mut battle, ARGENTI, Action::EnhancedUltimate, None);
    assert_eq!(damage_hits(&battle, DamageKind::FollowUp), hits);

    open_zone(&mut battle);
    assert_eq!(damage_hits(&battle, DamageKind::FollowUp).len(), 6);
    assert_eq!(battle.units[0].tribbie_state.lamb_stacks, 2);
    battle.units[1].energy_milli = 180_000;
    act(&mut battle, ARGENTI, Action::EnhancedUltimate, None);
    assert_eq!(damage_hits(&battle, DamageKind::FollowUp).len(), 9);
    assert_eq!(battle.units[0].tribbie_state.lamb_stacks, 3);
    let rules = battle.mechanics(0).unwrap();
    battle.units[0].normal_turns = battle.units[0].tribbie_state.lamb_until;
    rules.on_ally_turn_opening(&mut battle, 0, 0, false);
    assert_eq!(battle.units[0].tribbie_state.lamb_stacks, 0);
}

#[test]
fn a_lethal_hit_counts_but_zone_retargets_only_to_a_surviving_hit_enemy() {
    let mut plan = plan(&[tribbie(1, &[]), argenti()]);
    plan.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    plan.units[2].stats.hp = 5_000;
    plan.units[3].stats.hp = 4_000;
    plan.units[4].stats.hp = 9_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    open_zone(&mut battle);
    battle.units[2].hp = 1;
    battle.dummy_damage.clear();
    battle.ordinary_actor = Some(0);
    act(&mut battle, FORM, Action::Basic, Some(LEFT));
    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(
        damage_hits(&battle, DamageKind::Additional),
        [(0, 3, 351); 2]
    );
    assert_eq!(damage_hits(&battle, DamageKind::TrueDamage), [(0, 3, 484)]);
}

#[test]
fn ally_ultimate_that_clears_a_wave_sends_follow_up_to_the_new_wave() {
    let mut plan = plan(&[tribbie(0, &[]), argenti()]);
    plan.wave_ends = vec![4, 5];
    for enemy in &mut plan.units[2..] {
        enemy.kit = Kit::Enemy(&FINITE_ENEMY);
        enemy.stats.hp = 100;
    }
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, ARGENTI, Action::Ultimate, None);
    assert_eq!(battle.outcome, Outcome::Victory);
    assert!(battle.events().iter().any(|event| {
        event.source_id.as_deref() == Some(FORM)
            && event.target_id.as_deref() == Some("dpr:3")
            && matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::FollowUp,
                    ..
                }
            )
    }));
}

#[test]
fn a_new_zone_applies_one_acheron_dream_and_refresh_does_not_reapply() {
    let plan = plan(&[tribbie(0, &[]), acheron()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    open_zone(&mut battle);
    open_zone(&mut battle);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| {
                event.source_id.as_deref() == Some(FORM)
                    && matches!(event.kind, EventKind::SlashedDream { .. })
            })
            .count(),
        1
    );
}

#[test]
fn fixed_dpr_replays_with_all_damage_kinds_and_conserved_attribution() {
    let scenario = STANDARD_DPR_SCENARIO;
    let plan = PreparedBattle::official_dpr(
        &[
            tribbie(
                2,
                &[
                    LAMB_OUTSIDE_THE_WALL,
                    GLASS_BALL_WITH_WINGS,
                    PEBBLE_AT_CROSSROADS,
                ],
            ),
            argenti(),
        ],
        42,
        3,
    )
    .unwrap();
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    for kind in [
        DamageKind::Direct,
        DamageKind::FollowUp,
        DamageKind::Additional,
        DamageKind::TrueDamage,
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
