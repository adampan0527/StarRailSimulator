use super::*;
use crate::{
    CombatType, Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::Kit,
    calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

const RAPPA: &str = "ally.rappa";
const FUGUE: &str = "ally.fugue";
const DANCER: &str = "ally.dancer";
const FIRE: Element = Element::Fire;

fn rappa(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: RAPPA.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: IMAGINARY,
        },
        energy_capacity: 140,
        level: 80,
        stats: OfficialStats {
            hp: 1_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            base_aggro: 100,
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
            (SKILL.into(), vec![1.2]),
            (ULTIMATE.into(), vec![0.5, 0.3, 3.0]),
            (TALENT.into(), vec![10.0, 0.0, 0.6, 2.0, 0.5, 1.0]),
            (ENHANCED_BASIC.into(), vec![1.0, 0.5, 1.0, 0.5]),
            (BLAST_OPENING.into(), vec![1.0, 0.5, 1.0, 0.5]),
            (BLAST_SECOND.into(), vec![1.0, 0.5]),
            (AREA_FINISH.into(), vec![1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&trace| trace.into()).collect(),
    }
}

fn fugue() -> OfficialCharacter {
    let mut input = rappa(0, &[]);
    input.unit_id = FUGUE.into();
    input.form_id = "character.1225".into();
    input.combat_type = CombatType {
        element: FIRE,
        path: CombatPath::Nihility,
    };
    input.energy_capacity = 130;
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.122501".into(), vec![1.0]),
        ("skill.122502".into(), vec![3.0, 0.3, 1.0, 0.18, 2.0, 0.5]),
        ("skill.122503".into(), vec![2.0]),
        ("skill.122504".into(), vec![1.0, 0.4]),
        ("skill.122508".into(), vec![1.0, 0.5]),
    ]
    .into();
    input
}

fn dancer() -> OfficialCharacter {
    let mut input = rappa(0, &[]);
    input.unit_id = DANCER.into();
    input.form_id = "trailblazer.harmony".into();
    input.combat_type.path = CombatPath::Harmony;
    input.stats.speed_micros = 100_000_000;
    input.skill_parameters = [
        ("skill.800501".into(), vec![1.0]),
        ("skill.800502".into(), vec![0.5, 2.0]),
        ("skill.800503".into(), vec![3.0, 1.0, 0.3]),
        ("skill.800504".into(), vec![10.0]),
    ]
    .into();
    input
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 17, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn command(battle: &Battle<'_>, actor: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.snapshot().decision_index,
        actor_id: actor.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    battle
        .apply(&command(battle, actor, action, target))
        .unwrap();
}

fn seal(battle: &mut Battle<'_>) {
    battle.set_energy(0, 0, 140_000);
    act(battle, RAPPA, Action::Ultimate, None);
}

fn hit_as(battle: &mut Battle<'_>, actor: usize, target: usize, toughness: u16) {
    battle.begin_attack();
    battle.action = Some(Action::Basic);
    let element = battle.plan.units[actor].combat_type.unwrap().element;
    battle.attack_hit(actor, target, element, 10_000, toughness);
    battle.notify_attack(actor, Action::Basic, Some(target));
    battle.action = None;
}

fn real_breaks(battle: &Battle<'_>) -> usize {
    battle
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::WeaknessBroken)
        .count()
}

#[test]
fn rappa_builds_locked_base_rows_and_checks_enhanced_phase_consistency() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(!crate::supports_official_dpr_variant(FORM, "novaflare"));
    let mut input = rappa(6, &[A2, A4, A6]);
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(TALENT).unwrap()[2] = 0.75;
    input.skill_parameters.get_mut(TALENT).unwrap()[4] = 0.625;
    assert!(build(&input).is_ok());
    input.skill_parameters.get_mut(AREA_FINISH).unwrap()[0] = 0.4;
    assert!(build(&input).is_err());
}

#[test]
fn rappa_basic_and_area_skill_resolve_energy_sp_and_toughness() {
    let prepared = plan(&[rappa(0, &[])], &[120, 120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(0, 0, 0);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 20_000);
    assert_eq!(
        (battle.units[1].toughness, battle.units[2].toughness),
        (110, 120)
    );
    act(&mut battle, RAPPA, Action::Skill, None);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].energy_milli, 50_000);
    assert_eq!(
        (battle.units[1].toughness, battle.units[2].toughness),
        (100, 110)
    );
    let hits = battle
        .dummy_damage
        .iter()
        .map(|hit| (hit.target, hit.amount))
        .collect::<Vec<_>>();
    assert_eq!(hits, [(1, 600), (1, 720), (2, 720)]);
}

#[test]
fn rappa_ultimate_preserves_the_ordinary_turn_and_e1_refunds_only_on_exit() {
    let prepared = plan(&[rappa(1, &[])], &[600]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let before = battle.snapshot();
    seal(&mut battle);
    assert!(battle.in_extra_turn(0));
    assert_eq!(battle.snapshot().av_micros, before.av_micros);
    assert_eq!(battle.units[0].energy_milli, 5_000);
    assert_eq!(battle.effective_break_effect(0), 3_000);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .defense_ignore_bp(&battle, 0, 1),
        1_500
    );
    battle.set_energy(0, 0, 140_000);
    let decision_index = battle.snapshot().decision_index;
    for action in [Action::Skill, Action::Ultimate] {
        assert!(matches!(
            battle.apply(&command(&battle, RAPPA, action, None)),
            Err(BattleError::IllegalTiming)
        ));
    }
    assert_eq!(battle.snapshot().decision_index, decision_index);
    battle.set_energy(0, 0, 5_000);
    for (ink, energy) in [(2, 25_000), (1, 45_000), (0, 85_000)] {
        act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
        assert_eq!(battle.units[0].rappa_state.ink, ink);
        assert_eq!(battle.units[0].energy_milli, energy);
        assert_eq!(battle.skill_points, 3);
        if ink == 2 {
            assert!(!battle.in_extra_turn(0));
            assert_eq!(battle.snapshot().av_micros, before.av_micros);
            assert_eq!(battle.units[0].normal_turns, before.units[0].normal_turns);
        }
    }
    assert_eq!(battle.effective_break_effect(0), 0);
    assert_eq!(
        battle
            .mechanics(0)
            .unwrap()
            .defense_ignore_bp(&battle, 0, 1),
        0
    );
}

#[test]
fn rappa_first_two_phases_blast_and_e2_only_adds_primary_toughness() {
    for (eidolon, primary) in [(0, 80), (2, 66)] {
        let prepared = plan(&[rappa(eidolon, &[])], &[120, 120, 120, 120]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        seal(&mut battle);
        act(&mut battle, RAPPA, Action::Basic, Some("dpr:2"));
        let remaining = battle.units[1..]
            .iter()
            .map(|unit| unit.toughness)
            .collect::<Vec<_>>();
        assert_eq!(remaining, [96, primary, 96, 110]);
        let direct_hits = battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.kind == DamageKind::Direct)
            .map(|hit| (hit.target, hit.amount))
            .collect::<Vec<_>>();
        let main = if eidolon == 0 { 600 } else { 631 };
        let adjacent = if eidolon == 0 { 300 } else { 315 };
        assert_eq!(
            direct_hits,
            [
                (2, main),
                (1, adjacent),
                (3, adjacent),
                (2, main),
                (1, adjacent),
                (3, adjacent),
                (1, main),
                (2, main),
                (3, main),
                (4, main),
            ]
        );
        assert_eq!(real_breaks(&battle), 0);
    }
}

#[test]
fn rappa_enhanced_hit_reduces_nonweak_toughness_and_its_talent_uses_full_toughness() {
    static ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[rappa(0, &[])], &[120]);
    prepared.units[1].kit = Kit::Enemy(&ENEMY_KIT);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, RAPPA, Action::Skill, None);
    assert_eq!(battle.units[1].toughness, 120);
    seal(&mut battle);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[1].toughness, 100); // 7 + 7 + 3 + 3 with per-phase rounding.
    assert!(!battle.has_weakness(1, IMAGINARY));
    assert_eq!(real_breaks(&battle), 0);
}

#[test]
fn rappa_gains_two_elite_charges_per_true_break_including_fugue_toughness() {
    let prepared = plan(&[rappa(0, &[A2]), fugue()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(0, 0, 0);
    hit_as(&mut battle, 1, 2, 170);
    assert_eq!(real_breaks(&battle), 2);
    assert_eq!(battle.units[2].extra_toughness.unwrap().remaining, 0);
    assert_eq!(battle.units[0].rappa_state.charge, 4);
    assert_eq!(battle.units[0].energy_milli, 20_000);
    hit_as(&mut battle, 1, 2, 170);
    assert_eq!(real_breaks(&battle), 2);
    assert_eq!(battle.units[0].rappa_state.charge, 4);
    assert_eq!(battle.units[0].energy_milli, 20_000);
}

#[test]
fn rappa_consumes_direct_phase_charge_and_keeps_talent_toughness_break_charge() {
    let prepared = plan(&[rappa(0, &[])], &[37, 36]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    // Leave the adjacent enemy with 21 Toughness. The third direct phase breaks it.
    battle.set_toughness(0, 2, 21);
    seal(&mut battle);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    assert_eq!(real_breaks(&battle), 2);
    let changes = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::RappaCharge { before, after } => Some((before, after)),
            _ => None,
        })
        .collect::<Vec<_>>();
    // 21 direct Toughness on the neighbor, 37 direct + 4 talent on the primary.
    assert_eq!(changes, [(0, 1), (1, 2), (2, 0)]);

    let prepared = plan(&[rappa(0, &[])], &[41, 36]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_toughness(0, 2, 21);
    seal(&mut battle);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    assert_eq!(real_breaks(&battle), 2);
    assert_eq!(battle.units[0].rappa_state.charge, 1);
    assert_eq!(battle.units[1].toughness, 0);
}

#[test]
fn rappa_last_ink_groups_all_toughness_and_credits_each_super_break_provider() {
    let prepared = plan(&[rappa(0, &[A4]), fugue(), dancer()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.set_energy(2, 2, 140_000);
    act(&mut battle, DANCER, Action::Ultimate, None);
    battle.set_toughness(0, 3, 0);
    battle.units[3].extra_toughness.as_mut().unwrap().remaining = 0;
    seal(&mut battle);
    set_ink(&mut battle, 0, 1);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    let hits = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.kind == DamageKind::SuperBreak)
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 3);
    // 40 Toughness at 60% self / 100% Fugue / 100% Trailblazer, with 60% BE and 2/3 DEF.
    assert_eq!(
        hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [9_644, 16_074, 16_074]
    );
    assert_eq!(hits[0].assists, [(2, 1_808)]);
    // Removing Backup Dancer's Break Effect changes Fugue's hit as well.
    assert_eq!(hits[1].assists, [(2, 3_014), (1, 13_060)]);
    assert_eq!(hits[2].assists, [(2, 16_074)]);
    assert_eq!(battle.units[0].rappa_state.ink, 0);
    assert_eq!(battle.units[0].rappa_state.charge, 0);
}

#[test]
fn rappa_withered_leaf_is_capped_two_turn_break_vulnerability() {
    for (attack, expected) in [(2_399, 200), (2_799, 500), (4_000, 1_000)] {
        let mut input = rappa(0, &[A6]);
        input.stats.attack = attack;
        let prepared = plan(&[input, dancer()], &[120]);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        hit_as(&mut battle, 1, 2, 120);
        let debuff = battle.units[2]
            .debuffs
            .iter()
            .find(|status| status.kind == DebuffKind::WitheredLeaf)
            .unwrap();
        assert_eq!(
            (
                debuff.source,
                debuff.bp,
                debuff.remaining_turns(battle.units[2].normal_turns)
            ),
            (0, expected, Some(2))
        );
        // The triggering Break receives the mark only after its own damage has settled.
        let hit = battle
            .dummy_damage
            .iter()
            .find(|hit| hit.kind == DamageKind::WeaknessBreak)
            .unwrap();
        assert_eq!(hit.amount, 4_395);
        assert!(hit.assists.is_empty());
        battle.break_hit_capped(1, 2, IMAGINARY, Action::Basic, 10_000, 120);
        let marked = battle.dummy_damage.last().unwrap();
        let expected_damage = match attack {
            2_399 => 4_483,
            2_799 => 4_615,
            _ => 4_835,
        };
        assert_eq!(marked.amount, expected_damage);
        assert_eq!(marked.assists, [(0, expected_damage - 4_395)]);
    }
}

#[test]
fn rappa_e4_team_speed_is_temporary_and_e6_uses_fifteen_full_charges() {
    let mut input = rappa(6, &[A2]);
    input.skill_parameters.get_mut(TALENT).unwrap()[2] = 0.75;
    input.skill_parameters.get_mut(TALENT).unwrap()[4] = 0.625;
    let prepared = plan(&[input, dancer()], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[0].rappa_state.charge, 5);
    set_charge(&mut battle, 0, 14);
    hit_as(&mut battle, 1, 2, 120);
    assert_eq!(battle.units[0].rappa_state.charge, 15);
    assert_eq!(battle.speed_micros(1), 100_000_000);
    seal(&mut battle);
    assert_eq!(battle.speed_micros(0), 212_000_000);
    assert_eq!(battle.speed_micros(1), 112_000_000);
    set_ink(&mut battle, 0, 1);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:1"));
    assert_eq!(battle.units[0].rappa_state.charge, 5);
    assert_eq!(battle.speed_micros(1), 100_000_000);
    assert!(battle.dummy_damage.iter().any(|hit| {
        hit.actor == 0
            && hit.kind == DamageKind::WeaknessBreak
            && hit.amount == 60_900
            && !hit.critical
    }));
}

#[test]
fn rappa_finite_targets_removed_mid_attack_are_not_hit_again() {
    static ENEMY_KIT: EnemyKit = EnemyKit {
        stats: DUMMY_STATS,
        max_toughness: 120,
        weakness: [false; Element::COUNT],
        target: EnemyTarget::FirstLiving,
        hit_energy_milli: 0,
    };
    let mut prepared = plan(&[rappa(0, &[A4])], &[120, 120, 120, 120]);
    for target in 1..=4 {
        prepared.units[target].kit = Kit::Enemy(&ENEMY_KIT);
    }
    prepared.units[2].stats.hp = 500;
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    seal(&mut battle);
    act(&mut battle, RAPPA, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.units[2].hp, 0);
    assert_eq!(battle.units[2].toughness, 120);
    let remaining = battle.units[1..]
        .iter()
        .map(|unit| unit.toughness)
        .collect::<Vec<_>>();
    assert_eq!(remaining, [108, 120, 108, 114]);
    assert_eq!(battle.units[0].rappa_state.ink, 2);
}

#[test]
fn rappa_fixed_dpr_with_fugue_and_trailblazer_replays_and_balances_credit() {
    let prepared =
        PreparedBattle::official_dpr(&[rappa(0, &[A2, A4, A6]), fugue(), dancer()], 17, 3).unwrap();
    let scenario = STANDARD_DPR_SCENARIO;
    let result = calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(
        result,
        calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap()
    );
    let rappa_hits = result
        .targets
        .iter()
        .flat_map(|target| &target.hits)
        .filter(|hit| hit.source_id == RAPPA && hit.kind == DamageKind::SuperBreak)
        .collect::<Vec<_>>();
    assert!(
        rappa_hits
            .iter()
            .any(|hit| hit.credits.iter().any(|credit| credit.unit_id == RAPPA))
    );
    assert!(
        rappa_hits
            .iter()
            .any(|hit| hit.credits.iter().any(|credit| credit.unit_id == FUGUE))
    );
    assert!(
        rappa_hits
            .iter()
            .any(|hit| hit.credits.iter().any(|credit| credit.unit_id == DANCER))
    );
    assert_eq!(
        result
            .characters
            .iter()
            .map(|row| row.own_damage + row.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
}
