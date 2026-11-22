use super::*;

use crate::{
    CombatType, Command, DamageKind, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const OWNER: &str = FORM;
const SPRITE: &str = "trailblazer.remembrance:mem";
const HERTA: &str = "character.1013";
const SUNDAY: &str = "character.1313";
const FEIXIAO: &str = "character.1220";
const SPARKLE: &str = "character.1306";
const ENEMY: &str = "dpr:1";
const MIDDLE: &str = "dpr:2";

static FINITE_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

fn trailblazer(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: OWNER.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Remembrance,
            element: ICE,
        },
        energy_capacity: 160,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
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
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![0.6, 0.1]),
            (ULTIMATE.into(), vec![2.4, 0.4]),
            (TALENT.into(), vec![130.0, 0.8, 10.0, 640.0]),
            (JOINT_ATTACK.into(), vec![1.2, 1.2, 0.1]),
            (MEM_ATTACK.into(), vec![0.36, 4.0, 0.9]),
            (MEM_TALENT.into(), vec![0.12, 0.24]),
            (MEM_ARRIVAL.into(), vec![0.5]),
            (MEM_DEPARTURE.into(), vec![0.25]),
            (MEM_SUPPORT.into(), vec![0.28, 3.0, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn teammate(id: &str, path: CombatPath, element: Element, energy: u32) -> OfficialCharacter {
    let mut input = trailblazer(0, &[]);
    input.unit_id = id.into();
    input.form_id = id.into();
    input.combat_type = CombatType { path, element };
    input.energy_capacity = energy;
    input.stats.speed_micros = 120_000_000;
    input.stats.attack = 1_200;
    input.stats.base_attack = 1_200;
    input
}

fn herta() -> OfficialCharacter {
    let mut input = teammate(HERTA, CombatPath::Erudition, ICE, 110);
    input.skill_parameters = [
        ("skill.101301".into(), vec![1.0]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    input
}

fn sunday() -> OfficialCharacter {
    let mut input = teammate(SUNDAY, CombatPath::Harmony, Element::Imaginary, 130);
    input.eidolon = 1;
    input.skill_parameters = [
        ("skill.131301".into(), vec![1.0]),
        ("skill.131302".into(), vec![1.0, 0.30, 2.0, 0.50]),
        ("skill.131303".into(), vec![0.20, 0.30, 3.0, 0.12]),
        ("skill.131304".into(), vec![0.20, 3.0]),
    ]
    .into();
    input
}

fn feixiao() -> OfficialCharacter {
    let mut input = teammate(FEIXIAO, CombatPath::Hunt, Element::Wind, 0);
    input.skill_parameters = [
        ("skill.122001".into(), vec![0.5]),
        ("skill.122002".into(), vec![2.0]),
        ("skill.122003".into(), vec![1.6, 1.0, 6.0, 7.0]),
        ("skill.122008".into(), vec![0.6, 0.3]),
        ("skill.122009".into(), vec![0.6, 0.3]),
        ("skill.122004".into(), vec![1.1, 2.0, 6.0, 12.0, 0.6, 2.0]),
    ]
    .into();
    input
}

fn sparkle() -> OfficialCharacter {
    let mut input = teammate(SPARKLE, CombatPath::Harmony, Element::Quantum, 110);
    input.skill_parameters = [
        ("skill.130601".into(), vec![1.0]),
        ("skill.130602".into(), vec![0.24, 0.45, 1.0, 0.5]),
        ("skill.130603".into(), vec![2.0, 4.0, 0.10, 2.0]),
        ("skill.130604".into(), vec![2.0, 0.06, 2.0, 3.0]),
    ]
    .into();
    input
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

fn cast_ultimate(battle: &mut Battle<'_>) {
    battle.fill_energy(0);
    act(battle, OWNER, Action::Ultimate, None);
}

fn charge(battle: &Battle<'_>) -> u8 {
    battle.units[0].remembrance_trailblazer_state.charge
}

#[test]
fn accepts_canonical_frozen_kit_and_rejects_invalid_rows() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut invalid = trailblazer(0, &[]);
    invalid.skill_parameters.remove(MEM_SUPPORT);
    assert!(PreparedBattle::official_dpr(&[invalid], 42, 3).is_err());
    let mut invalid = trailblazer(0, &[]);
    invalid.skill_parameters.get_mut(MEM_ATTACK).unwrap()[1] = 3.0;
    assert!(PreparedBattle::official_dpr(&[invalid], 42, 3).is_err());
}

#[test]
fn sprite_has_its_own_hp_turn_slot_and_finite_enemy_targeting() {
    let mut prepared = plan(&[trailblazer(0, &[RHAPSODE])], &[120]);
    prepared.units[1].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[1].stats.hp = 10_000;
    prepared.units[1].stats.attack = 1_000;
    prepared.units[1].stats.base_attack = 1_000;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.av_micros, 35_000_000);
    assert_eq!(battle.allies().collect::<Vec<_>>(), [0]);
    assert_eq!(battle.enemies().collect::<Vec<_>>(), [1]);
    assert_eq!(battle.snapshot().units.len(), 2);

    act(&mut battle, OWNER, Action::Skill, None);
    let sprite = battle.memosprite(0).unwrap();
    assert_eq!(sprite, 2);
    assert_eq!(battle.party_units().collect::<Vec<_>>(), [0, 2]);
    assert!(!battle.turn_candidates().any(|unit| unit == sprite));
    assert_eq!(battle.enemy_targeting_weight(sprite), 1_000_000);
    assert_eq!(battle.hp(sprite), (1_440, 1_440));
    assert_eq!(battle.speed_micros(sprite), 130_000_000);
    assert_eq!(charge(&battle), 93);
    assert_eq!(
        serde_json::to_value(EventKind::MemospriteCharge {
            before: 90,
            after: 93
        })
        .unwrap(),
        serde_json::json!({"event": "memosprite_charge", "detail": {"before": 90, "after": 93}})
    );
    assert!(
        battle
            .action_order()
            .iter()
            .any(|entry| entry.unit_id == SPRITE)
    );
    assert!(
        battle
            .snapshot()
            .units
            .iter()
            .any(|unit| unit.unit_id == SPRITE && unit.is_ally)
    );
    assert!(
        battle
            .legal_actions()
            .iter()
            .all(|cmd| cmd.actor_id != SPRITE && cmd.target_id.as_deref() != Some(SPRITE))
    );

    for _ in 0..12 {
        let owner_energy = battle.units[0].energy_milli;
        battle.resolve_enemy(1, &FINITE_ENEMY);
        if battle.units[sprite].hp < 1_440 {
            assert_eq!(
                battle.units[0].energy_milli,
                (owner_energy + 10_000).min(160_000)
            );
            break;
        }
    }
    assert!(battle.units[sprite].hp < 1_440);
    assert!(
        battle
            .events()
            .iter()
            .any(|entry| entry.kind == EventKind::EnemyAttack
                && entry.target_id.as_deref() == Some(SPRITE))
    );
    let owner_hp = battle.units[0].hp;
    act(&mut battle, OWNER, Action::Skill, None);
    assert_eq!(battle.units[sprite].hp, 1_440);
    assert_eq!(battle.units[0].hp, owner_hp);
    assert_eq!(battle.skill_points, 1);

    battle.units[0].gauge_quanta = gauge_for(10_000);
    battle.true_damage(1, sprite, Action::Basic, 9_999, Vec::new());
    assert_eq!(battle.units[sprite].hp, 0);
    assert_eq!(battle.units[0].gauge_quanta, gauge_for(7_500));
    assert!(battle.units[0].summon_turn.is_none());
    assert_eq!(charge(&battle), 0);
    assert!(
        !battle
            .snapshot()
            .units
            .iter()
            .any(|unit| unit.unit_id == SPRITE)
    );

    if battle.ordinary_actor().is_none() {
        battle.advance_to_decision();
    }
    act(&mut battle, OWNER, Action::Skill, None);
    assert_eq!(battle.hp(sprite), (1_440, 1_440));
    assert!(matches!(charge(&battle), 50..=53));
}

#[test]
fn charge_uses_real_regeneration_and_support_advances_target_and_expires() {
    let prepared = plan(&[trailblazer(1, &[RHAPSODE, MAGNETS]), herta()], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, OWNER, Action::Skill, None);
    let sprite = battle.memosprite(0).unwrap();
    assert_eq!(charge(&battle), 93);

    battle.set_energy(0, 1, 109_000); // Actual +54,000, with 4,000 left over.
    assert_eq!(charge(&battle), 98);
    battle.restore_energy_flat(0, 1, 50_000); // Only 1,000 fits.
    assert_eq!(charge(&battle), 98);
    battle.restore_energy_flat(0, 0, 5_000); // Cross the next ten-energy threshold.
    assert_eq!(charge(&battle), 99);
    battle.restore_energy_flat(0, 0, 10_000);
    assert_eq!(charge(&battle), 100);
    assert_eq!(battle.gauge_quanta(sprite), 0);

    act(&mut battle, HERTA, Action::Basic, Some(ENEMY));
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert_eq!(battle.units[0].energy_milli, 135_000);
    assert_eq!(charge(&battle), 1);
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.ally_crit_damage_bonus_bp(&battle, 0, 1), 3_000);
    assert_eq!(rules.ally_crit_rate_bonus_bp(&battle, 0, 1), 1_000);
    assert_eq!(rules.memosprite_true_damage_bp(&battle, 0, 1), 3_000);

    battle.units[1].crit_rate_bonus_bp = 10_000;
    battle.attack_hit(1, 2, ICE, 10_000, 0);
    let hits = &battle.dummy_damage[battle.dummy_damage.len() - 2..];
    assert_eq!(hits[0].kind, DamageKind::Direct);
    assert!(hits[0].critical);
    assert_eq!(hits[1].kind, DamageKind::TrueDamage);
    assert_eq!(hits[1].amount, percentage_of(hits[0].amount, 3_000));
    assert_eq!(hits[1].assists, [(0, hits[1].amount)]);
    assert!(!hits[1].critical);

    let support = battle.units[1]
        .remembrance_trailblazer_state
        .support
        .unwrap();
    battle.units[1].finished_turns = support.expires_after_turn - 1;
    assert_eq!(rules.memosprite_true_damage_bp(&battle, 0, 1), 3_000);
    battle.units[1].finished_turns += 1;
    assert_eq!(rules.memosprite_true_damage_bp(&battle, 0, 1), 0);
    assert_eq!(rules.ally_crit_rate_bonus_bp(&battle, 0, 1), 0);
}

#[test]
fn epic_basic_uses_separate_sources_and_ultimate_is_guaranteed_critical() {
    let prepared = plan(&[trailblazer(6, &[EPIC_TRACE])], &[1_000; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    let sprite = battle.memosprite(0).unwrap();
    assert_eq!(battle.rng.draws, 0);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.actor, hit.amount, hit.critical))
            .collect::<Vec<_>>(),
        [(sprite, 2_592, true); 2]
    );
    assert_eq!(
        [battle.units[1].toughness, battle.units[2].toughness],
        [980; 2]
    );
    assert_eq!(charge(&battle), 90);

    cast_ultimate(&mut battle);
    cast_ultimate(&mut battle);
    assert_eq!(battle.units[0].remembrance_trailblazer_state.epic, 2);
    assert!(battle.apply_skill_freeze(1, sprite, 10_000, 1, 100, Action::Skill));
    battle.set_energy(0, 0, 0);
    battle.dummy_damage.clear();
    act(&mut battle, OWNER, Action::Basic, None);
    assert!(!battle.is_frozen(sprite));
    assert_eq!(battle.units[0].remembrance_trailblazer_state.epic, 1);
    assert_eq!(
        [battle.units[1].toughness, battle.units[2].toughness],
        [930; 2]
    );
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.actor, hit.amount))
            .collect::<Vec<_>>(),
        [(0, 720), (0, 720), (sprite, 720), (sprite, 720)]
    );
    assert_eq!(battle.skill_points, 4);
}

#[test]
fn epic_basic_triggers_tribbies_field_once_for_each_attack() {
    let tribbie_id = "character.1403";
    let mut tribbie = teammate(tribbie_id, CombatPath::Harmony, Element::Quantum, 120);
    tribbie.eidolon = 1;
    tribbie.skill_parameters = [
        ("skill.140301".into(), vec![0.3, 0.15]),
        ("skill.140302".into(), vec![0.24, 3.0]),
        ("skill.140303".into(), vec![0.3, 0.3, 0.12, 2.0]),
        ("skill.140304".into(), vec![0.18]),
    ]
    .into();
    let prepared = plan(&[trailblazer(0, &[EPIC_TRACE]), tribbie], &[1_000; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    cast_ultimate(&mut battle);
    battle.set_energy(0, 0, 0);
    battle.fill_energy(1);
    act(&mut battle, tribbie_id, Action::Ultimate, None);
    let sprite = battle.memosprite(0).unwrap();
    battle.dummy_damage.clear();

    act(&mut battle, OWNER, Action::Basic, None);

    let basic_commands = battle
        .log
        .iter()
        .flatten()
        .filter(|event| matches!(event.kind, crate::EventKind::Action(Action::Basic)))
        .count();
    assert_eq!(basic_commands, 1);
    assert_eq!(battle.skill_points, 4);
    let attacks = battle
        .dummy_damage
        .split_inclusive(|hit| hit.kind == DamageKind::TrueDamage)
        .collect::<Vec<_>>();
    assert_eq!(
        attacks.len(),
        2,
        "{:?}",
        battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.actor, hit.kind, hit.amount))
            .collect::<Vec<_>>()
    );
    for (attack, source) in attacks.into_iter().zip([0, sprite]) {
        assert_eq!(attack.len(), 5);
        assert!(attack[..2].iter().all(|hit| hit.actor == source));
        assert!(
            attack[2..4]
                .iter()
                .all(|hit| { hit.actor == 1 && hit.kind == DamageKind::Additional })
        );
        let true_damage = &attack[4];
        assert_eq!(true_damage.actor, 1);
        assert_eq!(true_damage.kind, DamageKind::TrueDamage);
        let attack_damage = attack[..4].iter().map(|hit| hit.amount).sum::<u32>();
        assert_eq!(true_damage.amount, attack_damage * 24 / 100);
    }
}

#[test]
fn mem_normal_skill_bounces_and_tracks_toughness_without_using_skill_points() {
    let prepared = plan(&[trailblazer(0, &[PARABLE])], &[1_000; 3]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, OWNER, Action::Skill, None);
    act(&mut battle, OWNER, Action::Basic, Some(MIDDLE));
    let sprite = battle.memosprite(0).unwrap();
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert_eq!(charge(&battle), 61);
    assert_eq!(battle.units[0].energy_milli, 140_000);
    assert_eq!(battle.skill_points, 3);
    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == sprite)
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 7);
    assert_eq!(
        hits[..4].iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [216; 4]
    );
    assert_eq!(
        hits[4..]
            .iter()
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>(),
        [(1, 540), (2, 540), (3, 540)]
    );
    assert_eq!(
        (1..=3)
            .map(|target| battle.units[target].toughness)
            .map(u32::from)
            .sum::<u32>(),
        2_940
    );
}

#[test]
fn e2_checks_real_other_memosprites_and_e4_counts_zero_energy_actions() {
    let mut prepared = plan(&[trailblazer(4, &[]), feixiao()], &[1_000]);
    let mut other = prepared.units.last().unwrap().clone();
    other.id = format!("{FEIXIAO}:fixture");
    other.kit = Kit::Memosprite { owner: 1 };
    prepared.units.push(other);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, OWNER, Action::Skill, None);
    let sprite = battle.memosprite(0).unwrap();
    let other = battle.memosprite(1).unwrap();

    battle.notify_memosprite_turn_finished(sprite);
    assert_eq!(battle.units[0].energy_milli, 110_000);
    battle.notify_memosprite_turn_finished(other);
    battle.notify_memosprite_turn_finished(other);
    assert_eq!(battle.units[0].energy_milli, 118_000);
    battle.units[0].normal_turns += 1;
    battle.notify_memosprite_turn_finished(other);
    assert_eq!(battle.units[0].energy_milli, 126_000);
    assert_eq!(charge(&battle), 54);

    act(&mut battle, FEIXIAO, Action::Basic, Some(ENEMY));
    assert_eq!(charge(&battle), 57);
    battle.units[1].remembrance_trailblazer_state.support = Some(Support {
        owner: 0,
        expires_after_turn: 4,
    });
    battle.units[other].hp = 1_000;
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(rules.memosprite_true_damage_bp(&battle, 0, 1), 3_400);
    assert_eq!(rules.memosprite_true_damage_bp(&battle, 0, other), 3_400);
    assert_eq!(rules.ally_crit_rate_bonus_bp(&battle, 0, other), 1_000);
}

#[test]
fn sunday_advances_and_buffs_the_sprite_within_its_own_slot() {
    let prepared = plan(&[trailblazer(0, &[]), sunday()], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, OWNER, Action::Skill, None);
    let sprite = battle.memosprite(0).unwrap();
    assert_eq!(battle.ordinary_actor(), Some(SUNDAY));
    assert!(battle.gauge_quanta(sprite) > 0);
    act(&mut battle, SUNDAY, Action::Skill, Some(OWNER));
    assert_eq!(battle.gauge_quanta(sprite), 0);
    assert_eq!(battle.ordinary_actor(), Some(OWNER));
    assert!(battle.has_ally_buff(
        1,
        sprite,
        crate::battle::support::AllyBuffKind::Damage,
        "skill.131302"
    ));
    act(&mut battle, OWNER, Action::Basic, Some(ENEMY));
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.actor == sprite
            && hit
                .assists
                .iter()
                .any(|&(owner, amount)| owner == 1 && amount > 0)
    }));
}

#[test]
fn finite_support_true_damage_is_a_separate_lethal_hit() {
    let mut prepared = plan(&[trailblazer(0, &[RHAPSODE]), herta()], &[120]);
    prepared.units[2].kit = Kit::Enemy(&FINITE_ENEMY);
    prepared.units[2].stats.hp = 850;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, OWNER, Action::Skill, None);
    battle.units[1].remembrance_trailblazer_state.support = Some(Support {
        owner: 0,
        expires_after_turn: 4,
    });
    act(&mut battle, HERTA, Action::Basic, Some(ENEMY));
    assert_eq!(battle.snapshot().outcome, crate::Outcome::Victory);
    assert_eq!(battle.units[1].energy_milli, 85_000);
    let hits = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind,
                amount,
                hp_loss,
                ..
            } if event.source_id.as_deref() == Some(HERTA) => Some((kind, amount, hp_loss)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        [
            (DamageKind::Direct, 720, 720),
            (DamageKind::TrueDamage, 201, 130)
        ]
    );
}

#[test]
fn sunday_ultimate_covers_only_the_sprite_present_at_application() {
    let prepared = plan(&[trailblazer(0, &[]), sunday()], &[1_000]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.fill_energy(1);
    act(&mut battle, SUNDAY, Action::Ultimate, Some(OWNER));
    act(&mut battle, OWNER, Action::Skill, None);
    let sprite = battle.memosprite(0).unwrap();
    let rules = battle.mechanics(1).unwrap();
    assert!(rules.ally_crit_damage_bonus_bp(&battle, 1, 0) > 0);
    assert_eq!(rules.ally_crit_damage_bonus_bp(&battle, 1, sprite), 0);

    battle.fill_energy(1);
    act(&mut battle, SUNDAY, Action::Ultimate, Some(OWNER));
    assert!(rules.ally_crit_damage_bonus_bp(&battle, 1, sprite) > 0);
    battle.true_damage(2, sprite, Action::Basic, 10_000, Vec::new());
    act(&mut battle, SUNDAY, Action::Basic, Some(ENEMY));
    act(&mut battle, OWNER, Action::Skill, None);
    assert_eq!(battle.units[sprite].memosprite_generation, 2);
    assert_eq!(rules.ally_crit_damage_bonus_bp(&battle, 1, sprite), 0);
    assert!(rules.ally_crit_damage_bonus_bp(&battle, 1, 0) > 0);
}

#[test]
fn fixed_dpr_keeps_three_targets_reports_sprite_sources_and_conserves_credit() {
    let scenario = STANDARD_DPR_SCENARIO;
    let prepared = plan(
        &[
            trailblazer(6, &[RHAPSODE, PARABLE, MAGNETS, EPIC_TRACE]),
            feixiao(),
            sparkle(),
            sunday(),
        ],
        &scenario.toughness,
    );
    let result = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    let repeated = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(result, repeated);
    assert_eq!(result.targets.len(), 3);
    assert_eq!(result.characters.len(), 4);
    assert_eq!(result.total_damage, 180_219);
    assert_eq!(
        result
            .characters
            .iter()
            .map(|role| role.own_damage + role.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == SPRITE
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == OWNER && credit.amount > 0)
            })
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == FEIXIAO
                    && hit.kind == DamageKind::TrueDamage
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == OWNER && credit.amount == hit.amount)
            })
    );
}
