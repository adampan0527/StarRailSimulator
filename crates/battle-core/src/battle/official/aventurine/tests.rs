use super::{A2, A4, A6, Aventurine, BASIC, FORM, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, CombatPath, CombatType, DamageKind, Element, EventKind, LogMode,
    OfficialCharacter, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, support::AllyBuffKind},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const AV: &str = "ally.aventurine";
const ARLAN: &str = "ally.arlan";
const FU: &str = "ally.fu-xuan";
const CENTER: &str = "dpr:2";
const ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 400,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 5_000,
};

fn aventurine(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: AV.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Preservation,
            element: Element::Imaginary,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp: 4_000,
            base_hp: 2_000,
            attack: 300,
            base_attack: 300,
            defense: 2_000,
            base_defense: 1_000,
            base_aggro: 150,
            base_speed_micros: 106_000_000,
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
            (BASIC.into(), vec![if eidolon >= 3 { 1.1 } else { 1.0 }]),
            (
                SKILL.into(),
                if eidolon >= 5 {
                    vec![0.256, 356.0, 3.0, 2.0]
                } else {
                    vec![0.24, 320.0, 3.0, 2.0]
                },
            ),
            (
                ULTIMATE.into(),
                vec![
                    7.0,
                    if eidolon >= 3 { 2.916 } else { 2.7 },
                    if eidolon >= 3 { 0.162 } else { 0.15 },
                    3.0,
                ],
            ),
            (
                TALENT.into(),
                if eidolon >= 5 {
                    vec![1.0, 7.0, 0.275, 0.55, 2.0]
                } else {
                    vec![1.0, 7.0, 0.25, 0.5, 2.0]
                },
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut input = aventurine(0, &[]);
    input.unit_id = ARLAN.into();
    input.form_id = "character.1008".into();
    input.combat_type = CombatType {
        path: CombatPath::Destruction,
        element: Element::Thunder,
    };
    input.energy_capacity = 110;
    input.stats.attack = 1_000;
    input.stats.base_attack = 1_000;
    input.stats.defense = 0;
    input.stats.base_defense = 0;
    input.stats.base_speed_micros = 80_000_000;
    input.stats.speed_micros = 80_000_000;
    input.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    input
}

fn fu_xuan() -> OfficialCharacter {
    let mut input = aventurine(0, &[]);
    input.unit_id = FU.into();
    input.form_id = "character.1208".into();
    input.combat_type.element = Element::Quantum;
    input.energy_capacity = 135;
    input.stats.defense = 1_000;
    input.stats.speed_micros = 300_000_000;
    input.skill_parameters = [
        ("skill.120801".into(), vec![0.5]),
        ("skill.120802".into(), vec![0.65, 0.0, 3.0, 0.06, 0.12]),
        ("skill.120803".into(), vec![1.0]),
        ("skill.120804".into(), vec![0.18, 0.5, 0.9]),
    ]
    .into();
    input
}

fn prepare(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[400, 400, 400])
        .unwrap()
}

fn finite(plan: &mut PreparedBattle, index: usize, attack: u32, hp: u32) {
    let unit = &mut plan.units[index];
    unit.kit = Kit::Enemy(&ENEMY);
    unit.stats.level = 80;
    unit.stats.attack = attack;
    unit.stats.base_attack = attack;
    unit.stats.crit_rate_bp = 0;
    unit.stats.hp = hp;
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

fn bets(battle: &Battle<'_>, owner: usize) -> u8 {
    battle.units[owner].aventurine_state.blind_bets
}

fn fortification(battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
    battle
        .official_shield(owner, target, Action::Skill)
        .map_or(0, |shield| shield.amount)
}

#[test]
fn fortification_sources_stack_to_twice_the_skill_and_refresh_each_recipients_turns() {
    let plan = prepare(&[aventurine(0, &[A4]), arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.shield_amount(0), 800); // .24 * 2,000 + 320
    assert_eq!(battle.shield_amount(1), 800);

    battle.apply_official_shield(1, 1, 1_500, 3, Some(Action::Ultimate));
    battle.deplete_shields(1, 100);
    assert_eq!(fortification(&battle, 0, 1), 700);
    assert_eq!(battle.shield_amount(1), 1_400);
    act(&mut battle, AV, Action::Skill, None);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(fortification(&battle, 0, 0), 1_600);
    assert_eq!(fortification(&battle, 0, 1), 1_500);
    act(&mut battle, AV, Action::Skill, None);
    assert_eq!(fortification(&battle, 0, 0), 1_600);
    assert_eq!(fortification(&battle, 0, 1), 1_600);

    for turn in 1..=3 {
        battle.units[1].normal_turns = turn;
        battle.finish_turn(1);
        assert_eq!(
            fortification(&battle, 0, 1),
            if turn < 3 { 1_600 } else { 0 }
        );
        assert_eq!(fortification(&battle, 0, 0), 1_600);
    }
}

#[test]
fn e1_shield_is_granted_after_the_ultimate_hit_and_debuff_applies_before_it() {
    let mut input = aventurine(1, &[]);
    input.stats.crit_rate_bp = 10_000;
    let plan = prepare(&[input, arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 110_000;
    act(&mut battle, AV, Action::Ultimate, Some(CENTER));
    let hit = &battle.dummy_damage[0];
    // 2000 * 2.7 * 0.6 * (1 + .50 + .15), with E1's 20% applied only afterward.
    assert_eq!(hit.amount, 5_346);
    assert_eq!(battle.units[3].toughness, 370);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(fortification(&battle, 0, 0), 800);
    assert_eq!(fortification(&battle, 0, 1), 800);
    assert_eq!(battle.effective_crit_damage_bp(1), 7_000);
    assert_eq!(battle.debuff_count(3), 1);
    assert_eq!(battle.debuff_count(2), 0);
    for event in battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::StatusApplied(StatusKind::Unnerved))
    {
        assert_eq!(event.target_id.as_deref(), Some(CENTER));
    }
}

#[test]
fn e2_and_unnerved_are_distinct_sources_of_attributed_team_damage_until_expiry() {
    let mut partner = arlan();
    partner.stats.crit_rate_bp = 10_000;
    let plan = prepare(&[aventurine(2, &[A4]), partner]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, AV, Action::Basic, Some(CENTER));
    // E2 applies before the basic: 2,000 * .60 * 1.12.
    assert_eq!(battle.dummy_damage[0].amount, 1_344);
    battle.units[0].energy_milli = 110_000;
    act(&mut battle, AV, Action::Ultimate, Some(CENTER));
    assert_eq!(battle.debuff_count(3), 2);
    act(&mut battle, AV, Action::Basic, Some("dpr:1"));
    act(&mut battle, ARLAN, Action::Basic, Some(CENTER));
    let hit = battle.dummy_damage.last().unwrap();
    // 1,000 * .60 * 1.12 * (1 + .50 + .20 + .15) vs 1,000 * .60 * 1.50.
    assert_eq!(hit.amount, 1_243);
    assert_eq!(hit.assists, [(0, 343)]);

    let expiration = battle.units[3]
        .debuffs
        .iter()
        .filter_map(|status| status.remaining_turns(battle.units[3].normal_turns))
        .max()
        .unwrap();
    assert!(expiration > 0);
    battle.units[3].normal_turns += expiration;
    battle.finish_turn(3);
    assert_eq!(battle.debuff_count(3), 0);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| event.kind == EventKind::StatusExpired(StatusKind::Unnerved))
    );
}

#[test]
fn leverage_changes_crit_with_live_defense_and_preserves_counterfactual_attribution() {
    let mut input = aventurine(0, &[A2]);
    input.stats.defense = 1_700;
    input.stats.crit_rate_bp = 7_000;
    let plan = prepare(&[input, arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();
    assert_eq!(mechanics.crit_rate_from_defense_bp(1_699), 0);
    assert_eq!(mechanics.crit_rate_from_defense_bp(1_700), 200);
    assert_eq!(mechanics.crit_rate_from_defense_bp(4_000), 4_800);
    assert_eq!(mechanics.crit_rate_from_defense_bp(10_000), 4_800);

    // Seed 42 first roll is between 7,200 and 8,000. Only the buffed hit is critical.
    battle.apply_ally_buff(1, 0, AllyBuffKind::DefensePercent, 4_000, 2);
    act(&mut battle, AV, Action::Basic, Some(CENTER));
    let hit = &battle.dummy_damage[0];
    assert!(hit.critical);
    assert_eq!(hit.amount, 1_890); // 2,100 * .60 * 1.50
    assert_eq!(hit.assists, [(1, 870)]); // Unbuffed: 1,700 * .60
}

#[test]
fn breaking_own_shield_counts_only_the_original_enemy_attack_and_drains_the_queue() {
    let mut plan = prepare(&[aventurine(0, &[A4])]);
    finite(&mut plan, 1, 300, 20_000);
    finite(&mut plan, 2, 2_000, 20_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    // Both shielded hits give two bets. The first leaves 700 shield and the second breaks it.
    battle.units[0].energy_milli = 0;
    for _ in 0..3 {
        battle.resolve_enemy(1, &ENEMY);
    }
    assert_eq!(fortification(&battle, 0, 0), 500);
    assert_eq!(bets(&battle, 0), 6);
    let from = battle.events().len();
    battle.resolve_enemy(2, &ENEMY);
    let new = &battle.events()[from..];
    let depleted = new
        .iter()
        .position(|event| event.kind == EventKind::ShieldDepleted)
        .unwrap();
    let charged = new
        .iter()
        .position(|event| {
            event.kind
                == EventKind::BlindBet {
                    before: 6,
                    after: 8,
                }
        })
        .unwrap();
    let consumed = new
        .iter()
        .position(|event| {
            event.kind
                == EventKind::BlindBet {
                    before: 8,
                    after: 1,
                }
        })
        .unwrap();
    let follow_up = new
        .iter()
        .position(|event| event.kind == EventKind::FollowUp)
        .unwrap();
    assert!(depleted < charged && charged < consumed && consumed < follow_up);
    assert_eq!(
        new.iter()
            .filter(|event| matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::FollowUp,
                    ..
                }
            ))
            .count()
            + battle
                .dummy_damage
                .iter()
                .filter(|hit| hit.kind == DamageKind::FollowUp)
                .count(),
        7
    );
    assert_eq!(battle.units[0].energy_milli, 27_000);
    assert_eq!(fortification(&battle, 0, 0), 0);
    assert_eq!(bets(&battle, 0), 1);

    battle.apply_official_shield(0, 0, 2_000, 2, Some(Action::Ultimate));
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(bets(&battle, 0), 1);
}

#[test]
fn fu_xuans_shared_damage_does_not_add_a_second_blind_bet() {
    let mut plan = prepare(&[arlan(), fu_xuan(), aventurine(0, &[A4])]);
    finite(&mut plan, 3, 1_000, 20_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FU, Action::Skill, None);

    let from = battle.events().len();
    battle.resolve_enemy(3, &ENEMY);
    let events = &battle.events()[from..];
    for target in [ARLAN, FU] {
        assert!(events.iter().any(|event| {
            event.target_id.as_deref() == Some(target)
                && matches!(event.kind, EventKind::Damage { absorbed, .. } if absorbed > 0)
        }));
    }
    assert_eq!(bets(&battle, 2), 1);
}

#[test]
fn bingo_counts_each_shielded_teammate_attack_once_and_resets_on_aventurines_turn() {
    let plan = prepare(&[aventurine(0, &[A4, A6]), arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.deplete_shields(1, 800);
    battle.follow_up(1, Element::Thunder, 1_000, 0, Action::Basic, 0);
    battle.resolve_follow_ups();
    assert_eq!(bets(&battle, 0), 0);

    battle.apply_official_shield(0, 1, 400, 3, Some(Action::Skill));
    for _ in 0..4 {
        battle.follow_up(1, Element::Thunder, 1_000, 0, Action::Basic, 0);
    }
    battle.resolve_follow_ups();
    assert_eq!(bets(&battle, 0), 3); // Each follow-up hits all three enemies.

    act(&mut battle, AV, Action::Basic, Some(CENTER));
    assert_eq!(battle.ordinary_actor(), Some(AV));
    battle.follow_up(1, Element::Thunder, 1_000, 0, Action::Basic, 0);
    battle.resolve_follow_ups();
    assert_eq!(bets(&battle, 0), 4);
    Aventurine::change_blind_bets(&mut battle, 0, 0, 3, Action::Basic);
    battle.notify_attack(0, Action::Basic, None);
    battle.resolve_follow_ups();
    assert_eq!(bets(&battle, 0), 0);
    assert_eq!(fortification(&battle, 0, 0), 1_040);
    assert_eq!(fortification(&battle, 0, 1), 880);
}

#[test]
fn control_guard_checks_the_wearer_and_cools_down_for_two_own_turns() {
    let mut plan = prepare(&[aventurine(0, &[A4]), arlan()]);
    finite(&mut plan, 2, 100, 20_000);
    plan.units[2].stats.effect_hit_rate_bp = 10_000;
    plan.units[0].stats.effect_resistance_bp = 0;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();

    assert_eq!(mechanics.effect_resistance_aura_bp(&battle, 0, 1), 5_000);
    assert!(!battle.apply_skill_freeze(2, 0, 10_000, 1, 1_000, Action::Basic));
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 1_000, Action::Basic));
    assert!(battle.apply_skill_freeze(2, 0, 10_000, 1, 1_000, Action::Basic));
    battle.clear_freeze(0);
    battle.clear_freeze(1);
    battle.units[0].normal_turns += 1;
    assert!(battle.roll_status(2, 0, 10_000, true));
    battle.units[0].normal_turns += 1;
    assert!(!battle.roll_status(2, 0, 10_000, true));

    battle.deplete_shields(0, 800);
    battle.deplete_shields(1, 800);
    battle.apply_official_shield(1, 0, 300, 3, Some(Action::Ultimate));
    assert_eq!(mechanics.effect_resistance_aura_bp(&battle, 0, 1), 0);
    battle.units[0].normal_turns += 2;
    assert!(battle.roll_status(2, 0, 10_000, true));
}

#[test]
fn e4_applies_defense_before_ten_individual_hits_and_energy_tracks_actual_hits() {
    let plan = prepare(&[aventurine(4, &[A6]), arlan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    Aventurine::change_blind_bets(&mut battle, 0, 0, 6, Action::Basic);
    battle.units[0].energy_milli = 110_000;
    act(&mut battle, AV, Action::Ultimate, Some(CENTER));
    assert_eq!(bets(&battle, 0), 3); // 6 + seed 42's 6 rolls over the 10 cap, then costs 7.
    assert_eq!(battle.units[0].energy_milli, 15_000);
    assert_eq!(battle.defense(0), 2_400);
    let hit_amounts = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::FollowUp)
        .map(|hit| hit.amount)
        .collect::<Vec<_>>();
    assert_eq!(hit_amounts, [360; 10]); // .25 * (2000 + .40 * 1000) * .60
    assert_eq!(
        battle
            .enemies()
            .map(|target| u32::from(battle.units[target].toughness))
            .sum::<u32>(),
        1_140
    );
    assert_eq!(fortification(&battle, 0, 0), 1_336); // E1 800 + A6 floor(2400 * .072 + 96) * 2
    assert_eq!(fortification(&battle, 0, 1), 1_068);

    let start = battle.units[0].normal_turns;
    battle.units[0].normal_turns = start + 1;
    battle.finish_turn(0);
    assert_eq!(battle.defense(0), 2_400);
    battle.units[0].normal_turns = start + 2;
    battle.finish_turn(0);
    assert_eq!(battle.defense(0), 2_000);

    let mut plan = prepare(&[aventurine(4, &[A4])]);
    finite(&mut plan, 1, 100, 200);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[2].hp = 0;
    battle.units[3].hp = 0;
    Aventurine::change_blind_bets(&mut battle, 0, 0, 6, Action::Basic);
    battle.units[0].energy_milli = 0;
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.units[1].hp, 0);
    assert_eq!(battle.units[0].energy_milli, 16_000); // Enemy hit, one bounce, and a kill.
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::FollowUp,
                    ..
                }
            ))
            .count(),
        1
    );
}

#[test]
fn e6_counts_other_shielded_teammates_including_shields_from_other_sources() {
    let plan = prepare(&[aventurine(6, &[]), arlan(), fu_xuan()]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let mechanics = battle.mechanics(0).unwrap();
    let damage_bonus = |battle: &Battle<'_>| {
        mechanics.damage_bonus_bp(battle, 0, 3, Some(Action::Basic), DamageKind::Direct)
    };
    battle.apply_official_shield(0, 0, 500, 3, Some(Action::Skill));
    assert_eq!(damage_bonus(&battle), 0);
    battle.apply_official_shield(1, 1, 500, 3, Some(Action::Ultimate));
    battle.apply_official_shield(0, 2, 500, 3, Some(Action::Skill));
    assert_eq!(damage_bonus(&battle), 10_000);
    battle.deplete_shields(1, 500);
    assert_eq!(damage_bonus(&battle), 5_000);
}

#[test]
fn controlled_aventurine_retains_up_to_ten_bets_and_follows_up_after_recovery() {
    let mut plan = prepare(&[aventurine(0, &[])]);
    finite(&mut plan, 1, 100, 20_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(1, 0, 10_000, 1, 1_000, Action::Basic));
    battle.apply_official_shield(0, 0, 800, 3, Some(Action::Skill));
    for _ in 0..6 {
        battle.resolve_enemy(1, &ENEMY);
    }
    assert_eq!(bets(&battle, 0), 10);
    assert!(
        battle
            .events()
            .iter()
            .all(|event| event.kind != EventKind::FollowUp)
    );

    assert!(battle.clear_freeze(0));
    battle.finish_turn(0);
    assert_eq!(bets(&battle, 0), 3);
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
fn fixed_dpr_replays_all_eidolons_with_conserved_attribution() {
    let scenario = STANDARD_DPR_SCENARIO;
    for eidolon in 0..=6 {
        let mut partner = arlan();
        partner.stats.crit_rate_bp = 10_000;
        let plan =
            PreparedBattle::official_dpr(&[aventurine(eidolon, &[A2, A4, A6]), partner], 42, 3)
                .unwrap();
        let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
        assert_eq!(
            result,
            calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
        );
        let hits = result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits.clone().any(|hit| hit.source_id == AV
            && hit.action == Action::Ultimate
            && hit.kind == DamageKind::Direct));
        assert!(hits.clone().any(|hit| {
            hit.source_id == ARLAN
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == AV && credit.amount > 0)
        }));
        for hit in hits {
            assert_eq!(
                hit.amount,
                hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            );
        }
    }
}
