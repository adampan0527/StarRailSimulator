use crate::{
    Action, Battle, CombatPath, CombatType, DamageKind, EventKind, LogMode, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind,
    battle::{Kit, resolve::gauge_for},
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
    supports_official_dpr_variant,
};

use super::*;

const WOLF: &str = "team.silver_wolf";
const ALLY: &str = "team.arlan";
const FIRST: &str = "dpr:1";
const SECOND: &str = "dpr:2";
const THUNDER: Element = Element::Thunder;

static NO_WEAKNESS: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};
static ELITE: EnemyKit = EnemyKit {
    max_toughness: 120,
    ..NO_WEAKNESS
};
static NATIVE_THUNDER: EnemyKit = EnemyKit {
    weakness: {
        let mut weakness = [false; Element::COUNT];
        weakness[THUNDER.index()] = true;
        weakness
    },
    ..NO_WEAKNESS
};

fn wolf(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let [basic, skill, ultimate, talent] = if enhanced {
        NOVAFLARE_SKILLS
    } else {
        BASE_SKILLS
    };
    let (skill_values, ultimate_values, talent_values) = if enhanced {
        (
            vec![1.96, 1.20, 3.0, 0.20, 1.0, 0.13, 2.0],
            vec![3.80, 1.20, 0.45, 3.0],
            vec![0.10, 0.12, 0.06, 1.0, 3.0],
        )
    } else {
        (
            vec![1.96, 0.85, 2.0, 0.20, 1.0, 0.10, 2.0],
            vec![3.80, 1.0, 0.45, 3.0],
            vec![0.10, 0.08, 0.06, 0.72, 3.0],
        )
    };
    OfficialCharacter {
        unit_id: WOLF.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Nihility,
            element: QUANTUM,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 125_000_000,
            speed_micros: 125_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 10_000,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (basic.into(), vec![1.0]),
            (skill.into(), skill_values),
            (ultimate.into(), ultimate_values),
            (talent.into(), talent_values),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut input = wolf(false, 0, &[]);
    input.unit_id = ALLY.into();
    input.form_id = "character.1008".into();
    input.combat_type = CombatType {
        path: CombatPath::Destruction,
        element: THUNDER,
    };
    input.stats.base_speed_micros = 120_000_000;
    input.stats.speed_micros = 120_000_000;
    input.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
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

fn finite(plan: &mut PreparedBattle, target: usize, kit: &'static EnemyKit, hp: u32) {
    let enemy = &mut plan.units[target];
    enemy.kit = Kit::Enemy(kit);
    enemy.stats.hp = hp;
    enemy.stats.base_hp = hp;
    enemy.stats.base_speed_micros = 1_000_000;
    enemy.stats.resist_bp = [2_000; Element::COUNT];
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

fn finish_enemy_turn(battle: &mut Battle<'_>, target: usize) {
    battle.units[target].normal_turns += 1;
    battle.finish_turn(target);
}

fn assert_implant(
    battle: &Battle<'_>,
    source: usize,
    target: usize,
    element: Element,
    bp: u32,
    turns: u32,
) {
    let (actual, actual_bp, actual_turns) = implant_on(battle, source, target).unwrap();
    assert_eq!(
        (actual.index(), actual_bp, actual_turns),
        (element.index(), bp, turns)
    );
}

fn hits(battle: &Battle<'_>, target: &str, kind: DamageKind) -> Vec<u32> {
    let target_index = battle
        .enemies()
        .find(|&index| battle.plan.units[index].id == target)
        .unwrap();
    if matches!(battle.plan.units[target_index].kit, Kit::Dummy { .. }) {
        return battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.target == target_index && hit.kind == kind)
            .map(|hit| hit.amount)
            .collect();
    }
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                kind: hit_kind,
                amount,
                ..
            } if hit_kind == kind && event.target_id.as_deref() == Some(target) => Some(amount),
            _ => None,
        })
        .collect()
}

#[test]
fn accepts_frozen_variants_and_rejects_mixed_or_missing_parameters() {
    assert!(supports_official_dpr_variant(FORM, "base"));
    assert!(supports_official_dpr_variant(FORM, "novaflare"));
    assert!(!supports_official_dpr_variant(FORM, "unknown"));
    for enhanced in [false, true] {
        assert!(build(&wolf(enhanced, 0, &[])).is_ok());
    }
    let mut mixed = wolf(true, 0, &[]);
    mixed
        .skill_parameters
        .insert(BASE_SKILLS[0].into(), vec![1.0]);
    assert!(build(&mixed).is_err());
    let mut incomplete = wolf(false, 0, &[]);
    incomplete.skill_parameters.remove(BASE_SKILLS[3]);
    assert!(build(&incomplete).is_err());
}

#[test]
fn skill_implants_quantum_before_damage_and_a4_outlasts_all_res_down() {
    let mut prepared = plan(&[wolf(false, 0, &[BASE_TRACES[1]])], &[30]);
    finite(&mut prepared, 1, &NO_WEAKNESS, 10_000);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert!(!battle.has_weakness(1, QUANTUM));
    act(&mut battle, WOLF, Action::Skill, Some(FIRST));
    // 1960 × 0.9 × (1 - 0.20 + 0.20 + 0.10) × 1000/1500.
    assert_eq!(hits(&battle, FIRST, DamageKind::Direct), [1_293]);
    assert_eq!(battle.units[1].toughness, 10);
    assert_eq!(battle.units[0].energy_milli, 85_000);
    assert_implant(&battle, 0, 1, QUANTUM, 2_000, 3);
    assert_eq!(battle.debuff_count(1), 3);
    finish_enemy_turn(&mut battle, 1);
    finish_enemy_turn(&mut battle, 1);
    assert!(battle.has_weakness(1, QUANTUM));
    assert!(
        !battle.units[1]
            .debuffs
            .iter()
            .any(|status| status.kind == DebuffKind::AllResistanceDown)
    );
    finish_enemy_turn(&mut battle, 1);
    assert!(!battle.has_weakness(1, QUANTUM));
    assert_eq!(battle.debuff_count(1), 0);
    assert!(
        battle
            .events()
            .iter()
            .any(|event| { event.kind == EventKind::StatusExpired(StatusKind::WeaknessImplanted) })
    );
}

#[test]
fn novaflare_prioritizes_first_slot_and_ally_can_break_implanted_weakness() {
    let mut prepared = plan(&[arlan(), wolf(true, 0, &[])], &[30, 30]);
    finite(&mut prepared, 2, &NO_WEAKNESS, 10_000);
    finite(&mut prepared, 3, &NATIVE_THUNDER, 10_000);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, WOLF, Action::Skill, Some(FIRST));
    assert_implant(&battle, 1, 2, THUNDER, 2_000, 3);
    assert_eq!(battle.units[2].toughness, 30);
    // The Skill deals Quantum damage, so the implanted Lightning RES reduction is excluded.
    assert_eq!(hits(&battle, FIRST, DamageKind::Direct), [1_093]);
    battle.units[2].toughness = 10;
    act(&mut battle, ALLY, Action::Basic, Some(FIRST));
    assert_eq!(battle.units[2].toughness, 0);
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::WeaknessBroken
            && event.source_id.as_deref() == Some(ALLY)
            && event.target_id.as_deref() == Some(FIRST)
    }));
    let rules = battle.mechanics(1).unwrap();
    rules.before_action(&mut battle, 1, Action::Skill, Some(3));
    assert_implant(&battle, 1, 3, QUANTUM, 2_000, 3);
    assert!(battle.has_weakness(3, THUNDER));

    // A later cast replaces the same source's implant without adding a second debuff.
    battle.units[0].hp = 0;
    let count = battle.debuff_count(2);
    rules.before_action(&mut battle, 1, Action::Skill, Some(2));
    assert_implant(&battle, 1, 2, QUANTUM, 2_000, 3);
    assert!(!battle.has_weakness(2, THUNDER));
    assert_eq!(battle.debuff_count(2), count);
}

#[test]
fn repeated_native_weakness_only_gets_all_type_res_down_and_side_note_checks_precast_count() {
    let prepared = plan(&[wolf(false, 0, &[BASE_TRACES[2]])], &[120; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    for kind in [DebuffKind::AttackDown, DebuffKind::DotVulnerability] {
        battle.apply_debuff(0, 1, kind, 1, 3);
        battle.apply_debuff(0, 2, kind, 1, 3);
    }
    battle.apply_debuff(0, 2, DebuffKind::EffectResistanceDown, 1, 3);
    let rules = battle.mechanics(0).unwrap();
    for (target, res_down) in [(1, 1_000), (2, 1_300)] {
        rules.before_action(&mut battle, 0, Action::Skill, Some(target));
        assert_implant(&battle, 0, target, QUANTUM, 0, 2);
        assert_eq!(
            battle.units[target]
                .debuffs
                .iter()
                .find(|status| status.kind == DebuffKind::AllResistanceDown)
                .unwrap()
                .bp,
            res_down
        );
    }
}

#[test]
fn novaflare_transfers_real_implant_to_elite_with_remaining_duration() {
    let mut prepared = plan(&[wolf(true, 0, &[])], &[30, 30, 120]);
    finite(&mut prepared, 1, &NO_WEAKNESS, 10_000);
    finite(&mut prepared, 2, &NO_WEAKNESS, 10_000);
    finite(&mut prepared, 3, &ELITE, 10_000);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let rules = battle.mechanics(0).unwrap();
    rules.before_action(&mut battle, 0, Action::Skill, Some(1));
    finish_enemy_turn(&mut battle, 1);
    battle.units[1].hp = 1;
    act(&mut battle, WOLF, Action::Basic, Some(FIRST));
    assert_eq!(battle.units[1].hp, 0);
    assert!(!battle.has_weakness(2, QUANTUM));
    assert_implant(&battle, 0, 3, QUANTUM, 2_000, 2);
    assert_eq!(battle.debuff_count(3), 1);
    finish_enemy_turn(&mut battle, 3);
    assert!(battle.has_weakness(3, QUANTUM));
    finish_enemy_turn(&mut battle, 3);
    assert!(!battle.has_weakness(3, QUANTUM));
}

#[test]
fn both_ultimates_reduce_defense_before_damage_and_preserve_action_targeting() {
    for enhanced in [false, true] {
        let prepared = plan(&[wolf(enhanced, 1, &[])], &[120; 2]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.fill_energy(0);
        act(
            &mut battle,
            WOLF,
            Action::Ultimate,
            (!enhanced).then_some(FIRST),
        );
        // 3800 × 0.9 × 1000 / (1000 + 500 × 0.55).
        assert_eq!(hits(&battle, FIRST, DamageKind::Direct), [2_682]);
        assert_eq!(battle.units[1].toughness, if enhanced { 100 } else { 90 });
        if enhanced {
            assert_eq!(hits(&battle, SECOND, DamageKind::Direct), [2_682]);
            assert_eq!(battle.units[0].energy_milli, 19_000);
        } else {
            assert!(hits(&battle, SECOND, DamageKind::Direct).is_empty());
            assert_eq!(battle.units[0].energy_milli, 12_000);
        }
        assert_eq!(battle.debuff_count(1), 2);
    }
}

#[test]
fn e1_caps_energy_per_cast_and_e4_caps_each_target_with_e6_damage_bonus() {
    let prepared = plan(&[wolf(true, 6, &[])], &[120; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    for kind in [
        DebuffKind::AttackDown,
        DebuffKind::Slow,
        DebuffKind::DotVulnerability,
        DebuffKind::EffectResistanceDown,
        DebuffKind::ResistanceDown(Element::Fire),
    ] {
        battle.apply_debuff(0, 1, kind, 1, 3);
    }
    battle.fill_energy(0);
    act(&mut battle, WOLF, Action::Ultimate, None);
    assert_eq!(battle.units[0].energy_milli, 40_000);
    // 3800 × 0.9 × 1.2 × 1000/1275 × (1 + 1.0 or 0.4).
    assert_eq!(hits(&battle, FIRST, DamageKind::Direct), [6_437]);
    assert_eq!(hits(&battle, SECOND, DamageKind::Direct), [4_506]);
    assert_eq!(hits(&battle, FIRST, DamageKind::Additional), [338; 5]);
    assert_eq!(hits(&battle, SECOND, DamageKind::Additional), [237; 2]);
}

#[test]
fn generate_adds_a_break_bug_before_the_attack_bug_and_extends_both() {
    let prepared = plan(&[wolf(true, 0, &[NOVAFLARE_TRACES[0]])], &[20]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[1].gauge_quanta += gauge_for(20_000);
    act(&mut battle, WOLF, Action::Skill, Some(FIRST));
    assert_eq!(battle.units[1].toughness, 0);
    assert!(battle.units[1].entanglement.is_some());
    assert_eq!(battle.debuff_count(1), 5);
    let bugs: Vec<_> = battle.units[1]
        .debuffs
        .iter()
        .filter(|status| {
            matches!(
                status.kind,
                DebuffKind::AttackDown | DebuffKind::Slow | DebuffKind::DefenseBug
            )
        })
        .map(|status| status.remaining_turns(0))
        .collect();
    assert_eq!(bugs, [Some(4); 2]);
    assert_eq!(hits(&battle, FIRST, DamageKind::WeaknessBreak), [1_419]);
}

#[test]
fn eidolon_two_is_permanent_and_novaflare_grants_ally_attack_bug() {
    let prepared = plan(&[wolf(false, 2, &[])], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.debuff_count(1), 1);
    battle.cleanse_one(1, 1);
    finish_enemy_turn(&mut battle, 1);
    assert!(battle.units[1].debuffs[0].kind == DebuffKind::EffectResistanceDown);
    assert_eq!(battle.units[1].debuffs[0].bp, 2_000);

    let prepared = plan(&[wolf(true, 2, &[]), arlan()], &[120; 2]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, WOLF, Action::Basic, Some(SECOND));
    act(&mut battle, ALLY, Action::Basic, Some(FIRST));
    assert_eq!(battle.debuff_count(2), 2);
    assert_eq!(battle.debuff_count(3), 3);
    assert_eq!(battle.vulnerability_from(0, 2), 2_000);
    let ally_hit = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.actor == 1)
        .unwrap();
    assert_eq!(ally_hit.amount, 720);
    assert_eq!(ally_hit.assists, [(0, 120)]);
}

#[test]
fn novaflare_inject_and_side_note_use_turn_start_and_effect_hit_rate() {
    for (ehr, attack) in [(999, 1_000), (1_000, 1_100), (4_999, 1_400), (7_000, 1_500)] {
        let mut input = wolf(true, 0, &NOVAFLARE_TRACES[1..]);
        input.stats.effect_hit_rate_bp = ehr;
        let prepared = plan(&[input], &[120]);
        let battle = Battle::start(&prepared, LogMode::Detailed);
        assert_eq!(battle.units[0].energy_milli, 80_000);
        assert_eq!(battle.effective_attack(0), attack);
    }
}

#[test]
fn each_version_replays_fixed_three_cycles_and_preserves_damage_credit() {
    let scenario = &STANDARD_DPR_SCENARIO;
    let totals = [false, true].map(|enhanced| {
        let traces = if enhanced {
            &NOVAFLARE_TRACES
        } else {
            &BASE_TRACES
        };
        let prepared = plan(&[wolf(enhanced, 0, traces), arlan()], &scenario.toughness);
        let calculate = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
        let result = calculate();
        assert_eq!(result, calculate());
        assert!(result.characters[0].assist_damage > 0);
        let hits = result.targets.iter().flat_map(|target| &target.hits);
        for hit in hits {
            assert_eq!(
                hit.amount,
                hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            );
        }
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|character| character.own_damage + character.assist_damage)
                .sum::<u64>()
        );
        result.total_damage
    });
    assert!(totals[1] > totals[0], "{totals:?}");
}
