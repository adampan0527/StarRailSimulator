use super::{BASIC, FORM, INTERLOCK, RELEASE, SKILL, TALENT, TRANSMISSION, ULTIMATE};
use crate::{
    Action, Battle, BattleError, Command, DamageKind, EventKind, LogMode, OfficialCharacter,
    OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, StatusKind, battle::Kit, calculate_dpr,
    fixture,
};

const MISHA: &str = "ally.misha";
const ARLAN: &str = "ally.arlan";
const HERTA: &str = "ally.herta";

fn misha(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: MISHA.into(),
        form_id: FORM.into(),
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            base_aggro: 125,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 150_000_000,
            speed_micros: 150_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            effect_resistance_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![1.0]),
            (SKILL.into(), vec![2.0, 0.8, 1.0]),
            (ULTIMATE.into(), vec![3.0, 0.6, 0.2, 0.3, 10.0]),
            (TALENT.into(), vec![2.0, 1.0]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn arlan() -> OfficialCharacter {
    let mut ally = misha(0, &[]);
    ally.unit_id = ARLAN.into();
    ally.form_id = "character.1008".into();
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 200_000_000;
    ally.skill_parameters = [
        ("skill.100801".into(), vec![0.5]),
        ("skill.100802".into(), vec![0.15, 1.2]),
        ("skill.100803".into(), vec![1.92, 0.96]),
        ("skill.100804".into(), vec![0.36]),
    ]
    .into();
    ally
}

fn herta() -> OfficialCharacter {
    let mut ally = misha(0, &[]);
    ally.unit_id = HERTA.into();
    ally.form_id = "character.1013".into();
    ally.energy_capacity = 110;
    ally.stats.speed_micros = 180_000_000;
    ally.skill_parameters = [
        ("skill.101301".into(), vec![0.5]),
        ("skill.101302".into(), vec![0.5, 0.5, 0.2]),
        ("skill.101303".into(), vec![1.2]),
        ("skill.101304".into(), vec![0.5, 0.25]),
    ]
    .into();
    ally
}

fn prepared(team: &[OfficialCharacter], seed: u32, points: u8) -> PreparedBattle {
    PreparedBattle::official_dpr(team, seed, points)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
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
        .expect("expected a legal action");
    battle.apply(&command).unwrap();
}

#[test]
fn talent_counts_skill_points_spent_by_self_and_team_and_preserves_fractional_energy() {
    let mut member = misha(0, &[]);
    member
        .skill_parameters
        .insert(TALENT.into(), vec![1.625, 1.0]);
    member.stats.energy_regeneration_bp = 11_000;
    let plan = prepared(&[member, arlan(), herta()], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    act(&mut battle, ARLAN, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.skill_points, 3);
    assert_eq!(battle.units[0].misha_state.accumulated_hits, 0);
    assert_eq!(battle.units[0].energy_milli, 50_000);

    act(&mut battle, HERTA, Action::Skill, None);
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].misha_state.accumulated_hits, 1);
    assert_eq!(battle.units[0].energy_milli, 51_787);

    act(&mut battle, MISHA, Action::Skill, Some("dpr:2"));
    assert_eq!(battle.skill_points, 1);
    assert_eq!(battle.units[0].misha_state.accumulated_hits, 3);
    assert_eq!(battle.units[0].energy_milli, 86_574);
    let own_hits: Vec<_> = battle
        .dummy_damage
        .iter()
        .filter(|hit| hit.actor == 0)
        .collect();
    assert_eq!(
        own_hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [4, 3, 5]
    );
    assert_eq!(
        own_hits.iter().map(|hit| hit.amount).collect::<Vec<_>>(),
        [1200, 480, 480]
    );
    let own_toughness: Vec<_> = battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Toughness { before, after } if event.source_id.as_deref() == Some(MISHA) => {
                Some(before - after)
            }
            _ => None,
        })
        .collect();
    assert_eq!(own_toughness, [20, 10, 10]);

    act(&mut battle, ARLAN, Action::Basic, Some("dpr:2"));
    assert_eq!(battle.skill_points, 2);
    assert_eq!(battle.units[0].misha_state.accumulated_hits, 3);
    assert_eq!(battle.units[0].energy_milli, 86_574);

    battle.units[0].misha_state.add_hits(4);
    act(&mut battle, HERTA, Action::Skill, None);
    assert_eq!(battle.units[0].misha_state.accumulated_hits, 7);
    assert_eq!(battle.units[0].energy_milli, 88_361);
}

#[test]
fn ultimate_spends_cap_plus_e1_snapshot_then_resets_to_three_base_hits() {
    for (eidolon, first_count, second_count) in [(0, 10, 3), (1, 13, 5)] {
        let mut plan = prepared(&[misha(eidolon, &[])], 42, 3);
        plan.units[2].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        plan.units[2].stats.hp = 10;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].misha_state.add_hits(30);
        assert_eq!(battle.units[0].misha_state.accumulated_hits, 7);
        battle.fill_energy(0);
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));

        let finite_hits: Vec<_> = battle
            .events()
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    EventKind::Damage {
                        kind: DamageKind::Direct,
                        ..
                    }
                )
            })
            .map(|event| event.target_id.as_deref().unwrap())
            .collect();
        assert_eq!(finite_hits, ["dpr:2"]);
        assert_eq!(battle.dummy_damage.len() + finite_hits.len(), first_count);
        assert!(battle.dummy_damage.iter().any(|hit| hit.target == 1));
        assert!(battle.dummy_damage.iter().any(|hit| hit.target == 3));
        assert_eq!(battle.units[2].hp, 0);
        assert_eq!(battle.units[0].energy_milli, 15_000);
        assert_eq!(battle.skill_points, 3);
        assert_eq!(battle.units[0].misha_state.accumulated_hits, 0);

        battle.fill_energy(0);
        let before = battle.dummy_damage.len();
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:3"));
        assert_eq!(battle.dummy_damage.len() - before, second_count);
        assert_eq!(battle.dummy_damage[before].target, 3);
        let combined_toughness_loss = 2_000 - battle.units[1].toughness - battle.units[3].toughness;
        assert_eq!(
            combined_toughness_loss,
            (first_count + second_count - 1) as u16 * 5 + 5
        );
    }
}

#[test]
fn release_boosts_the_first_freeze_and_interlock_boosts_later_hits_only_during_cast() {
    for interlock in [false, true] {
        let mut member = misha(0, &[RELEASE]);
        if interlock {
            member.enabled_trace_ids.push(INTERLOCK.into());
        }
        // The first bounce selects the left enemy and its freeze roll is 26.23%.
        let plan = prepared(&[member], 11, 3);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.fill_energy(0);
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));

        assert_eq!(
            battle
                .dummy_damage
                .iter()
                .map(|hit| hit.target)
                .collect::<Vec<_>>(),
            [2, 1, 2]
        );
        assert!(battle.is_frozen(2));
        assert_eq!(battle.is_frozen(1), interlock);
        assert!(!battle.is_frozen(3));
        assert_eq!(battle.effective_effect_hit_rate_bp(0), 0);
    }
}

#[test]
fn freeze_precedes_the_hit_and_its_turn_start_damage_can_crit_with_transmission() {
    let mut member = misha(0, &[RELEASE, TRANSMISSION]);
    member.stats.crit_rate_bp = 10_000;
    let plan = prepared(&[member], 42, 3);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.fill_energy(0);
    act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));

    let first_hit = &battle.dummy_damage[0];
    assert_eq!(first_hit.amount, 648);
    assert!(first_hit.critical);
    let freeze = battle
        .events()
        .iter()
        .position(|event| event.kind == EventKind::StatusApplied(StatusKind::Frozen))
        .unwrap();
    let toughness = battle
        .events()
        .iter()
        .position(|event| matches!(event.kind, EventKind::Toughness { .. }))
        .unwrap();
    assert!(freeze < toughness);

    act(&mut battle, MISHA, Action::Basic, Some("dpr:1"));
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 4);
    assert_eq!(snapshot.units[0].energy_milli, 25_000);
    assert!(!battle.is_frozen(2));
    let tick = battle
        .dummy_damage
        .iter()
        .find(|hit| hit.target == 2 && hit.kind == DamageKind::Additional)
        .unwrap();
    assert_eq!(tick.action, Action::Ultimate);
    assert!(tick.critical);
    assert_eq!(tick.amount, 324);
}

#[test]
fn release_freeze_skips_the_first_action_of_a_finite_attacking_enemy() {
    for (traces, attacks) in [(&[][..], true), (&[RELEASE][..], false)] {
        let mut plan = prepared(&[misha(0, traces)], 16, 3);
        plan.units[2].kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.fill_energy(0);
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));
        act(&mut battle, MISHA, Action::Basic, Some("dpr:3"));
        assert_eq!(
            battle.events().iter().any(|event| {
                event.source_id.as_deref() == Some("dpr:2") && event.kind == EventKind::EnemyAttack
            }),
            attacks
        );
    }
}

#[test]
fn e2_roll_uses_interlock_and_shreds_defense_before_that_hits_damage() {
    for (traces, expected) in [(&[RELEASE][..], 360), (&[RELEASE, INTERLOCK][..], 380)] {
        // The guaranteed first freeze consumes no draw. The E2 roll is 31.62%.
        let plan = prepared(&[misha(2, traces)], 11, 3);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.fill_energy(0);
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));

        // With E2 applied, 600 raw × 90% × 1000/(1000 + 500 × 84%) floors to 380.
        assert_eq!(battle.dummy_damage[0].amount, expected);
        if expected == 380 {
            let shred = battle
                .events()
                .iter()
                .position(|event| {
                    event.kind == EventKind::StatusApplied(StatusKind::DefenseDown)
                        && event.target_id.as_deref() == Some("dpr:2")
                })
                .unwrap();
            let toughness = battle
                .events()
                .iter()
                .position(|event| matches!(event.kind, EventKind::Toughness { .. }))
                .unwrap();
            assert!(shred < toughness);
            let debuffs = &battle.units[2].debuffs;
            assert_eq!(debuffs.len(), 1);
            assert_eq!(debuffs[0].bp, 1_600);
            while battle.units[2].normal_turns < 2 {
                act(&mut battle, MISHA, Action::Basic, Some("dpr:1"));
            }
            assert_eq!(battle.units[2].debuffs.len(), 1);
            while battle.units[2].normal_turns < 3 {
                act(&mut battle, MISHA, Action::Basic, Some("dpr:1"));
            }
            assert!(battle.units[2].debuffs.is_empty());
        }
    }
}

#[test]
fn e4_adds_six_pct_to_multiplier_and_e6_buff_expires_before_later_refund() {
    for (eidolon, ultimate_damage, basic_damage, refund) in [(4, 396, 600, 0), (6, 514, 780, 1)] {
        let mut member = misha(eidolon, &[]);
        if eidolon == 6 {
            member
                .skill_parameters
                .insert(TALENT.into(), vec![2.2, 1.0]);
        }
        let mut plan = prepared(&[member], 42, 0);
        for enemy in &mut plan.units[1..] {
            enemy.stats.effect_resistance_bp = 10_000;
        }
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.fill_energy(0);
        act(&mut battle, MISHA, Action::Ultimate, Some("dpr:2"));
        assert_eq!(battle.dummy_damage[0].amount, ultimate_damage);
        let before = battle.snapshot();
        let invalid = Command {
            decision_index: before.decision_index,
            actor_id: MISHA.into(),
            action: Action::Skill,
            target_id: Some("dpr:2".into()),
        };
        assert_eq!(
            battle.apply(&invalid),
            Err(BattleError::InsufficientSkillPoints)
        );
        assert_eq!(battle.snapshot(), before);

        act(&mut battle, MISHA, Action::Basic, Some("dpr:2"));
        assert_eq!(battle.dummy_damage.last().unwrap().amount, basic_damage);
        assert_eq!(battle.units[0].misha_state.accumulated_hits, 0);
        let before_skill = battle.dummy_damage.len();
        let before_events = battle.events().len();
        act(&mut battle, MISHA, Action::Skill, Some("dpr:2"));
        assert_eq!(battle.dummy_damage[before_skill].amount, 1_200);
        assert_eq!(battle.units[0].misha_state.accumulated_hits, 2);
        assert_eq!(battle.skill_points, refund);
        assert_eq!(
            battle.units[0].energy_milli,
            if eidolon == 6 { 57_200 } else { 57_000 }
        );
        let point_changes: Vec<_> = battle.events()[before_events..]
            .iter()
            .filter_map(|event| match event.kind {
                EventKind::SkillPoints { before, after } => Some((before, after)),
                _ => None,
            })
            .collect();
        assert_eq!(
            point_changes,
            if eidolon == 6 {
                vec![(1, 0), (0, 1)]
            } else {
                vec![(1, 0)]
            }
        );

        if eidolon == 6 {
            act(&mut battle, MISHA, Action::Skill, Some("dpr:2"));
            assert_eq!(battle.skill_points, 0);
            assert_eq!(battle.units[0].misha_state.accumulated_hits, 4);
        }
    }
}

#[test]
fn fixed_dpr_repeats_bounces_and_attributes_e2_assists() {
    let mut member = misha(6, &[RELEASE, INTERLOCK, TRANSMISSION]);
    member.stats.effect_hit_rate_bp = 50_000;
    let scenario = STANDARD_DPR_SCENARIO;
    let plan = PreparedBattle::official_dpr(&[member, arlan()], 42, 3).unwrap();
    let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let repeated = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    assert_eq!(result, repeated);
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result
            .characters
            .iter()
            .map(|member| member.own_damage + member.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == MISHA
                    && hit.action == Action::Ultimate
                    && hit.kind == DamageKind::Additional
            })
    );
}
