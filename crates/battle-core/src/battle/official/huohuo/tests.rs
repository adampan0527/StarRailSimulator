use crate::{
    CombatType, DamageKind, EventKind, LogMode, OfficialStats, PreparedBattle,
    STANDARD_DPR_SCENARIO, calculate_dpr,
    fixture::{DUMMY_STATS, EnemyKit, EnemyTarget},
};

use super::{super::super::Kit, *};

const HERTA: &str = "character.1013";
const NATASHA: &str = "natasha.abundance";
const FEIXIAO: &str = "character.1220";

static ENEMY: EnemyKit = EnemyKit {
    stats: DUMMY_STATS,
    max_toughness: 120,
    weakness: [false; Element::COUNT],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 0,
};

fn huohuo(enhanced: bool, eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let id = if enhanced { NOVAFLARE } else { BASE };
    let skill = if enhanced {
        vec![0.16, 160.0, 0.128, 128.0, 1.0]
    } else {
        vec![0.14, 140.0, 0.112, 112.0, 1.0]
    };
    OfficialCharacter {
        unit_id: FORM.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Abundance,
            element: Element::Wind,
        },
        energy_capacity: MAX_ENERGY,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 1_000,
            attack: 1_000,
            base_attack: 1_000,
            defense: 500,
            base_defense: 500,
            base_aggro: 100,
            base_speed_micros: 100_000_000,
            speed_micros: 150_000_000,
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
            (id.basic.into(), vec![0.25]),
            (id.skill.into(), skill),
            (id.ultimate.into(), vec![0.15, 0.24, 2.0]),
            (
                id.talent.into(),
                vec![
                    if enhanced { 3.0 } else { 2.0 },
                    1.0,
                    0.03,
                    0.0,
                    30.0,
                    0.5,
                    6.0,
                ],
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|&id| id.into()).collect(),
    }
}

fn herta() -> OfficialCharacter {
    let mut ally = huohuo(false, 0, &[]);
    ally.unit_id = HERTA.into();
    ally.form_id = HERTA.into();
    ally.combat_type = CombatType {
        path: CombatPath::Erudition,
        element: Element::Ice,
    };
    ally.stats.speed_micros = 110_000_000;
    ally.energy_capacity = 110;
    ally.skill_parameters = [
        ("skill.101301".into(), vec![0.50]),
        ("skill.101302".into(), vec![0.50, 0.50, 0.20]),
        ("skill.101303".into(), vec![1.20]),
        ("skill.101304".into(), vec![0.50, 0.25]),
    ]
    .into();
    ally
}

fn natasha() -> OfficialCharacter {
    let mut ally = huohuo(false, 0, &[]);
    ally.unit_id = NATASHA.into();
    ally.form_id = NATASHA.into();
    ally.combat_type.element = Element::Physical;
    ally.stats.speed_micros = 100_000_000;
    ally.energy_capacity = 90;
    ally.skill_parameters = [
        ("skill.110501".into(), vec![0.5]),
        ("skill.110502".into(), vec![0.07, 0.048, 2.0, 70.0, 48.0]),
        ("skill.110503".into(), vec![0.092, 92.0]),
        ("skill.110504".into(), vec![0.30, 0.25]),
    ]
    .into();
    ally
}

fn feixiao() -> OfficialCharacter {
    let mut ally = huohuo(false, 0, &[]);
    ally.unit_id = FEIXIAO.into();
    ally.form_id = FEIXIAO.into();
    ally.combat_type.path = CombatPath::Hunt;
    ally.stats.speed_micros = 100_000_000;
    ally.energy_capacity = 0;
    ally.skill_parameters = [
        ("skill.122001".into(), vec![0.5]),
        ("skill.122002".into(), vec![2.0]),
        ("skill.122003".into(), vec![1.6, 1.0, 6.0, 7.0]),
        ("skill.122008".into(), vec![0.6, 0.3]),
        ("skill.122009".into(), vec![0.6, 0.3]),
        ("skill.122004".into(), vec![1.1, 2.0, 6.0, 12.0, 0.6, 2.0]),
    ]
    .into();
    ally
}

fn finite_enemy(plan: &mut PreparedBattle, target: usize) {
    plan.units[target].kit = Kit::Enemy(&ENEMY);
    plan.units[target].stats = ENEMY.stats;
    plan.units[target].stats.attack = 20_000;
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

fn heals<'a>(battle: &'a Battle<'_>) -> Vec<(&'a str, u32)> {
    battle
        .events()
        .iter()
        .filter(|event| event.source_id.as_deref() == Some(FORM))
        .filter_map(|event| match event.kind {
            EventKind::Heal { amount } => Some((event.target_id.as_deref().unwrap(), amount)),
            _ => None,
        })
        .collect()
}

#[test]
fn both_versions_read_levels_and_reject_inconsistent_parameters() {
    assert!(crate::supports_official_dpr_variant(FORM, "base"));
    assert!(crate::supports_official_dpr_variant(FORM, "novaflare"));
    assert!(!crate::supports_official_dpr_variant(FORM, "other"));
    for enhanced in [false, true] {
        let mut owner = huohuo(enhanced, 0, &[]);
        assert!(build(&owner).is_ok());
        let id = if enhanced { NOVAFLARE } else { BASE };
        owner.skill_parameters.get_mut(id.talent).unwrap()[6] = 5.0;
        assert!(build(&owner).is_err());
    }
    let mut mixed = huohuo(false, 0, &[]);
    mixed
        .skill_parameters
        .insert(NOVAFLARE.basic.into(), vec![0.25]);
    assert!(build(&mixed).is_err());
}

#[test]
fn basic_wind_hit_scales_with_max_hp_and_generates_resources() {
    let mut owner = huohuo(false, 0, &[]);
    owner.stats.attack = 100;
    owner.stats.base_attack = 100;
    let plan = PreparedBattle::official_dpr(&[owner], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Basic, Some("dpr:2"));
    let hit = &battle.dummy_damage[0];
    // 2,000 × 25% × (1,000 / (1,000 + 500)) × 90% = 300.
    assert_eq!((hit.kind, hit.amount), (DamageKind::Direct, 300));
    assert_eq!(battle.units[2].toughness, 110);
    assert_eq!(battle.skill_points, 4);
    assert_eq!(battle.units[0].energy_milli, 90_000);
}

#[test]
fn skill_heals_its_target_and_only_neighboring_slots_and_cleanses_before_healing() {
    for (enhanced, primary, adjacent) in [(false, 420, 336), (true, 480, 384)] {
        let mut plan = PreparedBattle::official_dpr(
            &[huohuo(enhanced, 0, &[]), herta(), natasha(), feixiao()],
            42,
            3,
        )
        .unwrap();
        finite_enemy(&mut plan, 4);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        for target in battle.allies() {
            battle.units[target].hp = 1_100;
            battle.apply_vulnerability(4, target, 1_000, 4);
        }
        act(&mut battle, FORM, Action::Skill, Some(HERTA));
        assert_eq!(
            heals(&battle)[..3],
            [(HERTA, primary), (FORM, adjacent), (NATASHA, adjacent)]
        );
        let first_clear = battle
            .events()
            .iter()
            .find(|event| event.kind == EventKind::StatusCleared);
        assert_eq!(first_clear.unwrap().target_id.as_deref(), Some(HERTA));
        assert_eq!(battle.skill_points, 2);
        assert_eq!(battle.units[0].energy_milli, 100_000);
        if !enhanced {
            assert_eq!(battle.hp(3).0, 1_100);
            assert_eq!(battle.vulnerability_from(4, 0), 1_000);
        }
    }
}

#[test]
fn provisioning_cleanses_freeze_before_the_turn_is_skipped_and_tracks_successful_cleanses() {
    let mut plan = PreparedBattle::official_dpr(&[huohuo(false, 0, &[]), herta()], 42, 3).unwrap();
    finite_enemy(&mut plan, 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.apply_skill_freeze(2, 1, 10_000, 1, 0, Action::Basic));
    act(&mut battle, FORM, Action::Skill, Some(FORM));
    assert_eq!(battle.ordinary_actor(), Some(HERTA));
    assert!(!battle.is_frozen(1));
    assert_eq!(battle.units[0].huohuo_state.cleanses_left, 5);

    battle.notify_ally_turn_opening(1, true);
    assert_eq!(battle.units[0].huohuo_state.cleanses_left, 5);
    for _ in 0..5 {
        battle.apply_vulnerability(2, 1, 1_000, 4);
        battle.notify_ally_turn_opening(1, true);
        assert_eq!(battle.vulnerability_from(2, 1), 0);
    }
    battle.apply_vulnerability(2, 1, 1_000, 4);
    battle.units[1].hp = 1_500;
    battle.notify_ally_turn_opening(1, true);
    assert_eq!(battle.hp(1).0, 1_590);
    assert_eq!(battle.vulnerability_from(2, 1), 1_000);
    assert_eq!(battle.units[0].huohuo_state.provision_turns, 2);
}

#[test]
fn talent_heals_original_low_hp_and_enhanced_lowest_in_separate_phases() {
    for enhanced in [false, true] {
        let ids = if enhanced { NOVAFLARE } else { BASE };
        let plan = PreparedBattle::official_dpr(
            &[
                huohuo(enhanced, 0, &[ids.talent_energy]),
                herta(),
                natasha(),
            ],
            42,
            3,
        )
        .unwrap();
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle
            .mechanics(0)
            .unwrap()
            .after_action(&mut battle, 0, Action::Skill, Some(0));
        battle.units[1].hp = 970;
        battle.units[2].hp = 1;
        battle.notify_ally_action_finished(1, Action::Ultimate);
        assert_eq!(battle.units[0].energy_milli, 73_000);
        assert_eq!(battle.hp(1).0, if enhanced { 1_060 } else { 1_150 });
        assert_eq!(battle.hp(2).0, if enhanced { 181 } else { 91 });
    }
}

#[test]
fn ultimate_gives_flat_energy_only_to_energy_pools_and_enhanced_trace_uses_160_boundary() {
    for enhanced in [false, true] {
        let ids = if enhanced { NOVAFLARE } else { BASE };
        let mut ally = herta();
        ally.stats.energy_regeneration_bp = 15_000;
        let mut plan = PreparedBattle::official_dpr(
            &[
                huohuo(enhanced, 0, &[ids.control_resistance]),
                ally,
                natasha(),
                feixiao(),
            ],
            42,
            3,
        )
        .unwrap();
        for (unit, cap) in [(1, 160_000), (2, 159_000)] {
            let Kit::Official(kit) = &mut plan.units[unit].kit else {
                unreachable!()
            };
            Arc::get_mut(kit).unwrap().max_energy_milli = cap;
            plan.units[unit].starting_energy_milli = cap / 2;
        }
        plan.units[0].starting_energy_milli = 140_000;
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        battle.units[0].huohuo_state.cleanses_left = 2;
        act(&mut battle, FORM, Action::Ultimate, None);
        assert_eq!(battle.units[0].energy_milli, 5_000);
        assert_eq!(battle.units[1].energy_milli, 104_000);
        assert_eq!(battle.units[2].energy_milli, 103_350);
        assert_eq!(battle.units[3].energy_milli, 0);
        assert_eq!(battle.effective_attack(0), 1_000);
        assert_eq!(
            battle.effective_attack(1),
            if enhanced { 1_480 } else { 1_240 }
        );
        assert_eq!(battle.effective_attack(2), 1_240);
        assert_eq!(battle.effective_attack(3), 1_240);
        assert_eq!(battle.mechanics(0).unwrap().control_resistance_bp(), 3_500);
        assert_eq!(
            battle.units[0].huohuo_state.provision_turns,
            if enhanced { 3 } else { 0 }
        );
        assert_eq!(
            battle.units[0].huohuo_state.cleanses_left,
            if enhanced { 6 } else { 2 }
        );
    }
}

#[test]
fn enhanced_opening_energy_e1_speed_e4_healing_and_e6_assist_have_separate_effects() {
    let mut owner = huohuo(true, 6, &[NOVAFLARE.battle_start]);
    owner.stats.healing_bp = 1_000;
    let plan = PreparedBattle::official_dpr(&[owner, herta()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert_eq!(battle.units[0].energy_milli, 100_000);
    assert_eq!(battle.units[0].huohuo_state.provision_turns, 1);
    assert_eq!(battle.speed_micros(0), 162_000_000);
    assert_eq!(battle.speed_micros(1), 122_000_000);
    battle.units[1].hp = 1_000;
    act(&mut battle, FORM, Action::Skill, Some(HERTA));
    // (320 + 160) × (1 + 0.1 + 0.2) × (1 + 0.8 × 0.5), rounded down.
    assert_eq!(
        heals(&battle).iter().find(|(id, _)| *id == HERTA),
        Some(&(HERTA, 873))
    );
    assert_eq!(battle.units[0].huohuo_state.provision_turns, 4);
    assert_eq!(battle.ally_buff_value(0, 1, AllyBuffKind::Damage), 5_000);
    for _ in 0..5 {
        battle.notify_ally_turn_opening(0, false);
    }
    assert_eq!(battle.speed_micros(0), 150_000_000);
    assert_eq!(battle.speed_micros(1), 110_000_000);
}

#[test]
fn eidolon_two_intercepts_two_lethal_attacks_and_spends_a_provision_turn_each() {
    let mut plan = PreparedBattle::official_dpr(&[herta(), huohuo(false, 2, &[])], 42, 3).unwrap();
    finite_enemy(&mut plan, 2);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FORM, Action::Skill, Some(FORM));
    for (rescues, turns) in [(1, 2), (2, 1)] {
        battle.resolve_enemy(2, &ENEMY);
        assert_eq!(battle.hp(0).0, 1_000);
        assert_eq!(battle.units[1].huohuo_state.provision_turns, turns);
        assert_eq!(battle.units[1].huohuo_state.rescues_used, rescues);
    }
    battle.resolve_enemy(2, &ENEMY);
    assert_eq!(battle.hp(0).0, 0);
    assert_eq!(
        battle.events().last().unwrap().kind,
        EventKind::UnitDefeated
    );

    let mut plan = PreparedBattle::official_dpr(&[huohuo(true, 2, &[])], 42, 3).unwrap();
    finite_enemy(&mut plan, 1);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle
        .mechanics(0)
        .unwrap()
        .after_action(&mut battle, 0, Action::Skill, Some(0));
    battle.resolve_enemy(1, &ENEMY);
    assert_eq!(battle.hp(0).0, 1_000);
    assert_eq!(battle.units[0].huohuo_state.rescues_used, 1);
}

#[test]
fn fixed_dpr_replays_both_variants_and_credits_attack_and_damage_buffs() {
    let scenario = STANDARD_DPR_SCENARIO;
    for enhanced in [false, true] {
        let ids = if enhanced { NOVAFLARE } else { BASE };
        let team = [
            huohuo(
                enhanced,
                6,
                &[ids.battle_start, ids.control_resistance, ids.talent_energy],
            ),
            herta(),
        ];
        let run = || {
            let plan = PreparedBattle::official_dpr(&team, 42, 3).unwrap();
            calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
        };
        let result = run();
        assert_eq!(result, run());
        assert!(result.characters[0].assist_damage > 0);
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.source_id == HERTA
                        && hit
                            .credits
                            .iter()
                            .any(|credit| credit.unit_id == FORM && credit.amount > 0)
                })
        );
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>()
        );
    }
}
