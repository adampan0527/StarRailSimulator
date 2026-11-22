use super::{BASE, FORM, NOVAFLARE, QUANTUM};
use crate::{
    Action, Battle, CombatPath, CombatType, Command, DamageKind, EventKind, LogMode,
    OfficialCharacter, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::{Kit, resolve::gauge_for, support::AllyBuffKind},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget, GAUGE_FULL, KILL_ENERGY_MILLI},
};

const SEELE: &str = "team.seele";
const ALLY: &str = "team.ally";
const FIRST: &str = "dpr:1";
const SECOND: &str = "dpr:2";
const THIRD: &str = "dpr:3";

static FINITE: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [true; Element::COUNT],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 0,
};

fn seele(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let (ids, skill, ultimate, turns) = if enhanced {
        (NOVAFLARE, vec![3.6, 0.25, 3.0, 0.5], 7.2, 3.0)
    } else {
        (BASE, vec![2.2, 0.25, 2.0], 4.25, 1.0)
    };
    OfficialCharacter {
        unit_id: SEELE.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Hunt,
            element: QUANTUM,
        },
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp: 10_000,
            base_hp: 10_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 75,
            base_speed_micros: 115_000_000,
            speed_micros: 115_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (ids.basic.into(), vec![1.0]),
            (ids.skill.into(), skill),
            (ids.ultimate.into(), vec![ultimate]),
            (ids.talent.into(), vec![0.8, turns]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn ally(speed: u64) -> OfficialCharacter {
    let mut unit = seele(false, 0, &[]);
    unit.unit_id = ALLY.into();
    unit.form_id = "character.1008".into();
    unit.combat_type = CombatType {
        path: CombatPath::Destruction,
        element: Element::Thunder,
    };
    unit.energy_capacity = 110;
    unit.skill_parameters = [
        ("skill.100801".into(), vec![1.0]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    unit.stats.base_speed_micros = speed;
    unit.stats.speed_micros = speed;
    unit
}

fn plan(team: &[OfficialCharacter], toughness: &[u16]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(toughness)
        .unwrap()
}

fn finite(prepared: &mut PreparedBattle, target: usize, max_hp: u32) {
    prepared.units[target].kit = Kit::Enemy(&FINITE);
    prepared.units[target].stats.hp = max_hp;
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: &str) -> Command {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|command| {
            command.actor_id == actor
                && command.action == action
                && command.target_id.as_deref() == Some(target)
        })
        .unwrap();
    battle.apply(&command).unwrap();
    command
}

fn extra_turns(battle: &Battle<'_>) -> usize {
    battle
        .events()
        .iter()
        .filter(|event| {
            event.kind == EventKind::ExtraTurnStarted && event.source_id.as_deref() == Some(SEELE)
        })
        .count()
}

#[test]
fn seele_both_variants_complete_fixed_three_cycle_dpr() {
    let scenario = STANDARD_DPR_SCENARIO;
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(crate::supports_official_dpr_variant(FORM, "novaflare"));
    assert!(!crate::supports_official_dpr_variant(FORM, "other"));
    for (enhanced, expected) in [(false, 10_684), (true, 17_061)] {
        let prepared = PreparedBattle::official_dpr(&[seele(enhanced, 0, &[])], 42, 3).unwrap();
        let evaluate = || calculate_dpr(&prepared, &scenario.toughness, scenario.cycles).unwrap();
        let dpr = evaluate();
        assert_eq!(dpr, evaluate());
        assert_eq!(dpr.total_damage, expected);
        assert_eq!(dpr.dpr, expected as f64 / 3.0);
        assert_eq!(dpr.targets[1].hits.len(), 14);
        for hit in &dpr.targets[1].hits {
            assert_eq!(hit.kind, DamageKind::Direct);
            assert_eq!(hit.credits[0].amount, hit.amount);
            assert_eq!(hit.source_id, SEELE);
        }
        assert_eq!(dpr.targets[0].damage + dpr.targets[2].damage, 0);
    }

    let mut mixed = seele(false, 0, &[]);
    mixed
        .skill_parameters
        .insert(NOVAFLARE.basic.into(), vec![1.0]);
    assert!(PreparedBattle::official_dpr(&[mixed], 42, 3).is_err());
}

#[test]
fn seele_basic_kill_grants_zero_av_and_extra_skill_keeps_own_turn_timers_and_resources() {
    let mut prepared = plan(
        &[seele(false, 4, &[BASE.rippling_waves]), ally(110_000_000)],
        &[120; 3],
    );
    finite(&mut prepared, 2, 1);
    finite(&mut prepared, 3, 1);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.ordinary_actor(), Some(SEELE));
    let av = battle.av_micros;
    let basic = act(&mut battle, SEELE, Action::Basic, FIRST);

    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(SEELE));
    assert_eq!(extra_turns(&battle), 1);
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.units[0].finished_turns, 1);
    assert_eq!(battle.units[0].gauge_quanta, GAUGE_FULL - gauge_for(2_000));
    assert_eq!(
        battle.units[0].energy_milli,
        80_000 + KILL_ENERGY_MILLI + 15_000
    );
    assert_eq!(battle.skill_points, 4);

    let next_gauge = battle.units[0].gauge_quanta;
    let ally_gauge = battle.units[1].gauge_quanta;
    let skill = act(&mut battle, SEELE, Action::Skill, SECOND);
    assert_eq!(battle.units[3].hp, 0);
    assert_eq!(extra_turns(&battle), 1);
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.units[0].finished_turns, 1);
    assert_eq!(battle.units[0].seele_state.amplification_expires, 2);
    assert_eq!(battle.units[0].energy_milli, 120_000);
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.ordinary_actor(), Some(ALLY));
    assert_eq!(battle.av_micros, av + ally_gauge.div_ceil(110_000_000));
    assert_eq!(
        battle.units[0].gauge_quanta,
        next_gauge - (battle.av_micros - av) * 143_750_000
    );
    let replay = Battle::replay(&prepared, &[basic, skill], LogMode::Detailed).unwrap();
    assert_eq!(replay.snapshot(), battle.snapshot());
    assert_eq!(replay.events(), battle.events());
}

#[test]
fn seele_ultimate_kill_interrupts_and_resumes_allied_turn_at_same_av() {
    let prepared_team = [seele(false, 0, &[BASE.lacerate]), ally(125_000_000)];
    let mut prepared = plan(&prepared_team, &[120; 2]);
    finite(&mut prepared, 2, 1);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let av = battle.av_micros;
    let seele_gauge = battle.units[0].gauge_quanta;
    assert_eq!(battle.ordinary_actor(), Some(ALLY));
    battle.fill_energy(0);
    act(&mut battle, SEELE, Action::Ultimate, FIRST);
    assert_eq!(battle.ordinary_actor(), Some(SEELE));
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.units[0].energy_milli, 5_000 + KILL_ENERGY_MILLI);

    let order = battle.action_order();
    assert_eq!(order[0].unit_id, SEELE);
    assert_eq!(order[0].remaining_av_micros, 0);
    assert_eq!(order[1].unit_id, ALLY);
    assert_eq!(order[1].remaining_av_micros, 0);
    assert_eq!(order[2].unit_id, SEELE);
    assert_eq!(
        order[2].remaining_av_micros,
        seele_gauge.div_ceil(115_000_000)
    );

    act(&mut battle, SEELE, Action::Basic, SECOND);
    assert_eq!(battle.dummy_damage[0].amount, 1_296);
    assert_eq!(battle.ordinary_actor(), Some(ALLY));
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.units[0].gauge_quanta, seele_gauge);
    assert_eq!(battle.units[0].normal_turns, 0);
    assert_eq!(battle.units[0].finished_turns, 0);
    assert_eq!(battle.units[1].normal_turns, 1);
    assert_eq!(battle.units[1].finished_turns, 0);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::TurnStarted
                && event.source_id.as_deref() == Some(ALLY))
            .count(),
        1
    );
}

#[test]
fn seele_novaflare_automatic_skill_hits_half_hp_once_and_can_defeat() {
    let mut prepared = plan(
        &[seele(true, 0, &[NOVAFLARE.nightshade]), ally(125_000_000)],
        &[120; 3],
    );
    finite(&mut prepared, 2, 5_000);
    finite(&mut prepared, 3, 10_000);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[2].hp = 3_100;
    act(&mut battle, ALLY, Action::Basic, FIRST);
    assert_eq!(battle.units[2].hp, 340);
    assert_eq!(battle.units[2].toughness, 90);
    assert_eq!(battle.units[0].energy_milli, 60_000);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.speed_micros(0), 143_750_000);
    assert_eq!(battle.ordinary_actor(), Some(SEELE));

    // Seele's own ordinary turn reset the automatic cast. Its kill grants Resurgence.
    battle.units[3].hp = 2_100;
    act(&mut battle, SEELE, Action::Basic, SECOND);
    assert_eq!(battle.units[3].hp, 0);
    assert_eq!(battle.units[0].energy_milli, 80_000 + KILL_ENERGY_MILLI);
    assert_eq!(battle.skill_points, 5);
    assert_eq!(extra_turns(&battle), 1);
    assert_eq!(battle.units[0].seele_state.nightshade_stacks, 1);
    assert_eq!(
        battle
            .events()
            .iter()
            .filter(|event| event.kind == EventKind::TriggeredSkill)
            .count(),
        2
    );
}

#[test]
fn seele_automatic_skill_falls_back_by_hp_ratio_after_a_defeated_target() {
    let mut prepared = plan(&[seele(true, 0, &[]), ally(125_000_000)], &[120; 3]);
    finite(&mut prepared, 2, 1);
    finite(&mut prepared, 3, 30_000);
    finite(&mut prepared, 4, 10_000);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.units[3].hp = 9_000;
    battle.units[4].hp = 4_000;
    act(&mut battle, ALLY, Action::Basic, FIRST);
    let cast = battle
        .events()
        .iter()
        .find(|event| event.kind == EventKind::TriggeredSkill)
        .unwrap();
    assert_eq!(cast.target_id.as_deref(), Some(SECOND));
    assert_eq!(battle.units[3].hp, 6_840);
    assert_eq!(battle.units[4].hp, 4_000);
    assert_eq!(extra_turns(&battle), 0);
}

#[test]
fn seele_e1_conditions_aggro_speed_stacks_and_personal_defense_ignore() {
    let traces = [BASE.nightshade];
    let prepared = plan(&[seele(false, 2, &traces)], &[120]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.enemy_targeting_weight(0), 750_000);
    battle.units[0].hp = 5_000;
    assert_eq!(battle.enemy_targeting_weight(0), 375_000);
    act(&mut battle, SEELE, Action::Skill, FIRST);
    assert_eq!(battle.speed_micros(0), 143_750_000);
    act(&mut battle, SEELE, Action::Skill, FIRST);
    assert_eq!(battle.speed_micros(0), 172_500_000);
    act(&mut battle, SEELE, Action::Basic, FIRST);
    assert_eq!(battle.speed_micros(0), 143_750_000);
    act(&mut battle, SEELE, Action::Basic, FIRST);
    assert_eq!(battle.speed_micros(0), 115_000_000);

    for enhanced in [false, true] {
        let mut prepared = plan(&[seele(enhanced, 1, &[])], &[120]);
        finite(&mut prepared, 1, 10_000);
        let mut battle = Battle::start(&prepared, LogMode::Off);
        let rules = battle.mechanics(0).unwrap();
        battle.units[1].hp = 8_001;
        assert_eq!(
            rules.crit_rate_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
            0
        );
        assert_eq!(rules.defense_ignore_bp(&battle, 0, 1), 0);
        battle.units[1].hp = 8_000;
        assert_eq!(
            rules.crit_rate_bonus_bp(&battle, 0, 1, None, DamageKind::Direct),
            1_500
        );
        assert_eq!(
            rules.defense_ignore_bp(&battle, 0, 1),
            if enhanced { 2_000 } else { 0 }
        );
    }
}

#[test]
fn seele_e6_novaflare_snapshots_ultimate_true_damage_and_assist() {
    let prepared = plan(
        &[seele(true, 6, &[NOVAFLARE.lacerate]), ally(110_000_000)],
        &[120],
    );
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    battle.apply_ally_buff(1, 0, AllyBuffKind::Damage, 2_000, 1);
    battle.fill_energy(0);
    act(&mut battle, SEELE, Action::Ultimate, FIRST);
    assert_eq!(battle.dummy_damage[0].amount, 10_800);
    assert_eq!(battle.dummy_damage[0].assists, [(1, 1_080)]);
    assert_eq!(battle.dummy_damage.len(), 1);
    act(&mut battle, SEELE, Action::Basic, FIRST);
    let hit = battle.dummy_damage.last().unwrap();
    assert_eq!(hit.kind, DamageKind::TrueDamage);
    assert_eq!(hit.amount, 3_240);
    assert_eq!(hit.assists, [(1, 324)]);
    assert_eq!(hit.action, Action::Basic);
    assert!(!hit.critical);
    assert_eq!(battle.units[2].toughness, 80);
}

#[test]
fn seele_e6_ally_kill_only_grants_resurgence_for_novaflare() {
    for enhanced in [false, true] {
        let mut prepared = plan(&[seele(enhanced, 6, &[]), ally(125_000_000)], &[120; 3]);
        finite(&mut prepared, 2, 20_000);
        let mut battle = Battle::start(&prepared, LogMode::Detailed);
        battle.fill_energy(0);
        // Keep the target above half HP throughout the opening Ultimate.
        act(&mut battle, SEELE, Action::Ultimate, FIRST);
        battle.units[2].hp = 800;
        act(&mut battle, ALLY, Action::Basic, FIRST);
        assert_eq!(battle.units[2].hp, 0);
        assert_eq!(extra_turns(&battle), usize::from(enhanced));
        let expected = if enhanced {
            DamageKind::TrueDamage
        } else {
            DamageKind::Additional
        };
        assert!(battle.events().iter().any(|event| {
            event.source_id.as_deref() == Some(SEELE)
                && matches!(event.kind, EventKind::Damage { kind, .. } if kind == expected)
        }));
        assert_eq!(
            battle.units[0].energy_milli,
            5_000 + KILL_ENERGY_MILLI + 15_000
        );
        if enhanced {
            assert_eq!(battle.ordinary_actor(), Some(SEELE));
            act(&mut battle, SEELE, Action::Basic, THIRD);
        }
    }
}

#[test]
fn seele_extra_basic_advances_next_natural_turn_when_own_ordinary_turn_is_suspended() {
    let mut prepared = plan(&[seele(false, 0, &[BASE.rippling_waves])], &[120; 2]);
    finite(&mut prepared, 1, 1);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let av = battle.av_micros;
    battle.fill_energy(0);
    act(&mut battle, SEELE, Action::Ultimate, FIRST);
    assert!(battle.in_extra_turn(0));
    act(&mut battle, SEELE, Action::Basic, SECOND);
    assert_eq!(battle.av_micros, av);
    assert_eq!(battle.ordinary_actor(), Some(SEELE));
    assert!(!battle.in_extra_turn(0));
    assert_eq!(battle.units[0].normal_turns, 1);
    assert_eq!(battle.units[0].finished_turns, 0);

    act(&mut battle, SEELE, Action::Skill, SECOND);
    let next = (GAUGE_FULL - gauge_for(2_000)).div_ceil(143_750_000);
    assert_eq!(battle.av_micros, av + next);
    assert_eq!(battle.units[0].finished_turns, 1);
}

#[test]
fn seele_novaflare_nightshade_caps_and_expires_in_ordinary_turns() {
    let mut prepared = plan(&[seele(true, 0, &[NOVAFLARE.nightshade])], &[120; 5]);
    for target in 1..=4 {
        finite(&mut prepared, target, 1);
    }
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    act(&mut battle, SEELE, Action::Basic, FIRST);
    assert!(battle.in_extra_turn(0));
    assert_eq!(battle.units[0].seele_state.nightshade_stacks, 2);

    act(&mut battle, SEELE, Action::Basic, THIRD);
    assert!(battle.in_extra_turn(0));
    assert_eq!(battle.units[0].seele_state.nightshade_stacks, 3);
    assert_eq!(battle.units[0].seele_state.nightshade_expires, 4);
    assert_eq!(battle.units[0].finished_turns, 1);
    act(&mut battle, SEELE, Action::Basic, "dpr:5");
    assert!(!battle.in_extra_turn(0));
    assert_eq!(extra_turns(&battle), 2);

    for actor_turn in 2..=5 {
        act(&mut battle, SEELE, Action::Basic, "dpr:5");
        let base_with_toughness = 600;
        let amplification_and_nightshade = 1_380;
        assert_eq!(
            battle.dummy_damage.last().unwrap().amount,
            if actor_turn < 5 {
                base_with_toughness + amplification_and_nightshade
            } else {
                base_with_toughness
            },
        );
    }
}
