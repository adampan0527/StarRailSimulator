use super::{BASIC, EFFICIENCY, ICING, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, Command, DamageKind, EventKind, LogMode, OfficialCharacter, OfficialStats,
    PreparedBattle, STANDARD_DPR_SCENARIO,
    battle::setup::{Kit, UnitPlan},
    calculate_dpr,
    fixture::{DUMMY_STATS, Element, EnemyKit, EnemyTarget},
};

static ICE_WEAK_ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 30,
    weakness: {
        let mut weaknesses = [false; Element::COUNT];
        weaknesses[Element::Ice.index()] = true;
        weaknesses
    },
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};

fn character(eidolon: u8) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: "ally.herta".into(),
        form_id: "character.1013".into(),
        level: 80,
        energy_capacity: 110,
        stats: OfficialStats {
            hp: 2_000,
            attack: 1_000,
            base_attack: 600,
            defense: 500,
            base_defense: 500,
            base_speed_micros: 100_000_000,
            speed_micros: 200_000_000,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: 0,
            thunder_damage_bp: None,
            break_effect_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            (BASIC.into(), vec![0.5]),
            (SKILL.into(), vec![0.5, 0.5, 0.2]),
            (ULTIMATE.into(), vec![1.2]),
            (TALENT.into(), vec![0.5, 0.25]),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: Vec::new(),
    }
}

fn finite_plan(input: OfficialCharacter, hit_points: &[u32]) -> PreparedBattle {
    let mut plan = PreparedBattle::official_dpr(&[input], 42, 3).unwrap();
    plan.units.truncate(plan.ally_count);
    for (i, &hp) in hit_points.iter().enumerate() {
        let mut stats = ICE_WEAK_ENEMY.stats;
        stats.hp = hp;
        plan.units.push(UnitPlan {
            id: format!("enemy.{i}"),
            kit: Kit::Enemy(&ICE_WEAK_ENEMY),
            stats,
            starting_energy_milli: 0,
        });
    }
    plan.wave_ends = vec![plan.units.len()];
    plan
}

fn command(battle: &Battle<'_>, action: Action, target: Option<&str>) -> Command {
    battle
        .legal_actions()
        .into_iter()
        .find(|command| command.action == action && command.target_id.as_deref() == target)
        .expect("expected a legal Herta action")
}

fn damage_events(battle: &Battle<'_>) -> Vec<(DamageKind, u32)> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage { kind, amount, .. }
                if event.source_id.as_deref() == Some("ally.herta") =>
            {
                Some((kind, amount))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn fixed_dpr_applies_the_high_hp_trace_without_triggering_follow_ups() {
    let mut herta = character(0);
    herta.enabled_trace_ids.push(EFFICIENCY.into());
    let scenario = STANDARD_DPR_SCENARIO;
    let plan = PreparedBattle::official_dpr(&[herta], 42, 3).unwrap();
    let run = || calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
    let result = run();
    assert_eq!(result, run());
    assert_eq!(result.targets.len(), 3);
    for target in &result.targets {
        let first = &target.hits[0];
        let second = &target.hits[1];
        assert_eq!(first.source_id, "ally.herta");
        assert_eq!(first.action, Action::Skill);
        assert_eq!(second.action, Action::Skill);
        // 1,000 ATK × 50% multiplier × (1 + 20% + 25%) × 2/3 DEF × 90% toughness,
        // split into 30% and 70% hits and truncated individually.
        assert_eq!((first.amount, second.amount), (130, 304));
        assert!(
            target
                .hits
                .iter()
                .all(|hit| hit.kind != DamageKind::FollowUp)
        );
    }
    assert_eq!(result.total_damage, result.characters[0].own_damage);
}

#[test]
fn half_hp_crossing_chains_after_the_full_attack_and_replays() {
    let plan = finite_plan(character(0), &[720, 900, 1_400]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    let skill = command(&battle, Action::Skill, None);
    battle.apply(&skill).unwrap();

    // The skill puts the first enemy at exactly 50%, and the first spin puts the
    // second enemy below 50%. Each spin deals 1,000 × 25% × 2/3 × 90% = 150.
    let mut expected = vec![(DamageKind::Direct, 108); 3];
    expected.extend([(DamageKind::Direct, 252); 3]);
    expected.extend([(DamageKind::FollowUp, 150); 6]);
    assert_eq!(damage_events(&battle), expected);
    let snapshot = battle.snapshot();
    assert_eq!(snapshot.skill_points, 2);
    assert_eq!(snapshot.units[0].energy_milli, 55_000 + 30_000 + 2 * 5_000);
    assert_eq!(snapshot.units[1].hp, 60);
    assert_eq!(snapshot.units[2].hp, 240);
    assert_eq!(snapshot.units[3].hp, 740);
    assert!(
        snapshot.units[1..]
            .iter()
            .all(|enemy| enemy.toughness == 10)
    );

    let replay = Battle::replay(&plan, &[skill], LogMode::Detailed).unwrap();
    assert_eq!(replay.events(), battle.events());
}

#[test]
fn the_second_skill_hit_checks_hp_again_before_dealing_damage() {
    let plan = finite_plan(character(0), &[1_000]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 600;
    let skill = command(&battle, Action::Skill, None);
    battle.apply(&skill).unwrap();
    // The first 30% hit pushes 600 HP to 492, so the 70% hit loses the DMG bonus.
    assert_eq!(
        damage_events(&battle),
        [
            (DamageKind::Direct, 108),
            (DamageKind::Direct, 210),
            (DamageKind::FollowUp, 150),
        ]
    );
}

#[test]
fn e1_basic_checks_pre_hit_hp_and_does_not_add_toughness_damage() {
    let plan = finite_plan(character(1), &[1_200]);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].hp = 600;
    let basic = command(&battle, Action::Basic, Some("enemy.0"));
    battle.apply(&basic).unwrap();
    assert_eq!(
        damage_events(&battle),
        [(DamageKind::Direct, 300), (DamageKind::Additional, 240)]
    );
    assert_eq!(battle.snapshot().skill_points, 4);
    assert_eq!(battle.snapshot().units[0].energy_milli, 75_000);
    assert_eq!(battle.snapshot().units[1].toughness, 20);

    let mut crossing = Battle::start(&plan, LogMode::Detailed);
    crossing.units[1].hp = 601;
    crossing.apply(&basic).unwrap();
    assert_eq!(
        damage_events(&crossing),
        [(DamageKind::Direct, 300), (DamageKind::FollowUp, 150)]
    );
}

#[test]
fn icing_and_e6_apply_in_their_own_damage_windows() {
    let mut herta = character(6);
    herta.enabled_trace_ids.push(ICING.into());
    let mut plan = finite_plan(herta, &[1_000, 2_500]);
    plan.units[0].starting_energy_milli = 110_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.apply_break_freeze(0, 2, Action::Basic);
    let ultimate = command(&battle, Action::Ultimate, None);
    battle.apply(&ultimate).unwrap();

    // E6 begins after the ultimate. ATK becomes 1,000 + 25% × 600 = 1,150;
    // E4 gives the resulting follow-up 10% DMG, for 189.75 damage per enemy.
    assert_eq!(
        damage_events(&battle),
        [
            (DamageKind::Direct, 720),
            (DamageKind::Direct, 864),
            (DamageKind::FollowUp, 189),
            (DamageKind::FollowUp, 189),
        ]
    );
    assert_eq!(battle.units[0].crit_rate_bonus_bp, 300);
    assert_eq!(battle.snapshot().units[0].energy_milli, 10_000);
    assert_eq!(battle.snapshot().skill_points, 3);
}

#[test]
fn e2_keeps_five_stacks_when_the_next_wave_crosses_half_hp() {
    let mut herta = character(2);
    herta.stats.crit_rate_bp = 10_000;
    let mut plan = finite_plan(herta, &[700; 6]);
    plan.wave_ends = vec![6, 7];
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    let skill = command(&battle, Action::Skill, None);
    battle.apply(&skill).unwrap();
    assert_eq!(battle.snapshot().wave_index, 1);
    assert_eq!(battle.units[0].crit_rate_bonus_bp, 1_500);

    let skill = command(&battle, Action::Skill, None);
    battle.apply(&skill).unwrap();
    assert_eq!(battle.units[0].crit_rate_bonus_bp, 1_500);
}
