use super::{A2, A4, A6, BASIC, FORM, OfficialCharacter, SKILL, TALENT, ULTIMATE};
use crate::{
    Action, Battle, CombatPath, CombatType, DamageKind, Element, EventKind, LogMode, OfficialStats,
    PreparedBattle, battle::Kit, calculate_dpr, fixture,
};

const FU: &str = "ally.fu-xuan";
const LYNX: &str = "ally.lynx";

fn fu_xuan(eidolon: u8, traces: &[&str]) -> OfficialCharacter {
    let raised_skill = eidolon >= 3;
    let raised_ultimate = eidolon >= 5;
    OfficialCharacter {
        unit_id: FU.into(),
        form_id: FORM.into(),
        combat_type: CombatType {
            path: CombatPath::Preservation,
            element: Element::Quantum,
        },
        energy_capacity: 135,
        level: 80,
        stats: OfficialStats {
            hp: 4_000,
            base_hp: 1_474,
            base_aggro: 150,
            attack: 465,
            base_attack: 465,
            defense: 1_000,
            base_defense: 606,
            base_speed_micros: 200_000_000,
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
            (BASIC.into(), vec![if raised_ultimate { 0.55 } else { 0.5 }]),
            (
                SKILL.into(),
                if raised_skill {
                    vec![0.65, 0.0, 3.0, 0.066, 0.132]
                } else {
                    vec![0.65, 0.0, 3.0, 0.06, 0.12]
                },
            ),
            (
                ULTIMATE.into(),
                vec![if raised_ultimate { 1.08 } else { 1.0 }],
            ),
            (
                TALENT.into(),
                if raised_skill {
                    vec![0.196, 0.5, 0.92]
                } else {
                    vec![0.18, 0.5, 0.9]
                },
            ),
        ]
        .into(),
        eidolon,
        enabled_trace_ids: traces.iter().map(|id| (*id).into()).collect(),
    }
}

fn lynx() -> OfficialCharacter {
    let mut ally = fu_xuan(0, &[]);
    ally.unit_id = LYNX.into();
    ally.form_id = "character.1110".into();
    ally.combat_type.path = CombatPath::Abundance;
    ally.energy_capacity = 100;
    ally.stats.hp = 2_000;
    ally.stats.defense = 0;
    ally.stats.base_defense = 0;
    ally.stats.base_speed_micros = 125_000_000;
    ally.stats.speed_micros = 125_000_000;
    ally.skill_parameters = [
        ("skill.111001".into(), vec![0.25]),
        (
            "skill.111002".into(),
            vec![0.05, 50.0, 2.0, 0.08, 80.0, 5.0],
        ),
        ("skill.111003".into(), vec![1.0, 0.09, 90.0]),
        ("skill.111004".into(), vec![2.0, 0.024, 24.0, 0.03, 30.0]),
    ]
    .into();
    ally
}

fn finite_enemy(
    plan: &mut PreparedBattle,
    target: usize,
    attack: u32,
) -> &'static fixture::EnemyKit {
    let kit = fixture::enemy("fixture.target_left", 32).unwrap();
    let unit = &mut plan.units[target];
    unit.kit = Kit::Enemy(kit);
    unit.stats.level = 80;
    unit.stats.attack = attack;
    unit.stats.crit_rate_bp = 0;
    unit.stats.base_speed_micros = 50_000_000;
    unit.stats.hp = 10_000;
    kit
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = battle
        .legal_actions()
        .into_iter()
        .find(|cmd| {
            cmd.actor_id == actor && cmd.action == action && cmd.target_id.as_deref() == target
        })
        .unwrap();
    battle.apply(&command).unwrap();
}

fn damage_log<'a>(battle: &'a Battle<'_>) -> Vec<(&'a str, u32, u32, u32)> {
    battle
        .events()
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::Damage {
                amount,
                hp_loss,
                absorbed,
                ..
            } => Some((
                event.target_id.as_deref().unwrap(),
                amount,
                hp_loss,
                absorbed,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn matrix_splits_before_defense_resistance_and_each_recipients_shields() {
    let mut plan = PreparedBattle::official_dpr(&[lynx(), fu_xuan(0, &[])], 42, 3).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 1_000);
    plan.units[0].stats.resist_bp[Element::Physical.index()] = 2_000;
    plan.units[1].stats.resist_bp[Element::Physical.index()] = 1_000;
    let mut battle = Battle::start(&plan, LogMode::Detailed);

    battle.resolve_enemy(2, enemy);
    // Talent already reduces 1000 * 0.8 to 656 before any Matrix.
    assert_eq!(damage_log(&battle), [(LYNX, 656, 656, 0)]);
    act(&mut battle, FU, Action::Skill, None);
    battle.apply_official_shield(0, 0, 250, 2, None);
    battle.apply_official_shield(0, 1, 100, 2, None);
    let fu_energy = battle.units[1].energy_milli;

    battle.resolve_enemy(2, enemy);
    // Ally: 1000 * .35 * 1 * .8 * .82 = 229.6; Fu Xuan: 1000 * .65 * .5 * .9 * .82 = 239.85.
    assert_eq!(
        damage_log(&battle)[1..],
        [(LYNX, 229, 0, 229), (FU, 239, 139, 100)]
    );
    assert_eq!(battle.hp(0), (1_584, 2_240));
    assert_eq!(battle.hp(1), (4_101, 4_240));
    assert_eq!(battle.shield_amount(0), 21);
    assert_eq!(battle.units[1].energy_milli, fu_energy);
}

#[test]
fn matrix_hp_and_crit_bonuses_refresh_without_compounding_and_expire_on_caster_turn() {
    let mut fu = fu_xuan(1, &[A2]);
    fu.stats.energy_regeneration_bp = 11_000;
    let plan = PreparedBattle::official_dpr(&[fu, lynx()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[0].energy_milli = 0;
    battle.units[1].hp = 1_000;
    act(&mut battle, FU, Action::Skill, None);
    assert_eq!(battle.hp(0), (4_240, 4_240));
    assert_eq!(battle.hp(1), (1_240, 2_240));
    assert_eq!(battle.effective_crit_damage_bp(1), 8_000);
    assert_eq!(battle.units[0].energy_milli, 33_000);

    act(&mut battle, LYNX, Action::Basic, Some("dpr:2"));
    act(&mut battle, FU, Action::Skill, None);
    assert_eq!(battle.hp(0), (4_240, 4_240));
    assert_eq!(battle.hp(1), (1_240, 2_240));
    assert_eq!(battle.units[0].energy_milli, 88_000);

    while battle.units[0].finished_turns < 5 {
        let actor = battle.ordinary_actor().unwrap().to_owned();
        act(&mut battle, &actor, Action::Basic, Some("dpr:2"));
    }
    assert_eq!(battle.hp(0), (4_000, 4_000));
    assert_eq!(battle.hp(1), (1_240, 2_000));
    assert_eq!(battle.effective_crit_damage_bp(1), 5_000);
}

#[test]
fn restores_missing_hp_using_charges_and_ultimate_heals_only_other_allies() {
    let mut plan = PreparedBattle::official_dpr(&[fu_xuan(0, &[A4]), lynx()], 42, 3).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 2_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FU, Action::Skill, None);
    battle.units[0].hp = 2_940;
    battle.units[1].hp = 500;
    battle.resolve_enemy(2, enemy);
    // HP 2,940 minus 820 hits exactly 50%. Restore floor(2,120 * .90) = 1,908.
    assert_eq!(battle.hp(0).0, 4_028);
    assert_eq!(battle.units[0].fu_xuan_state.restore_charges, 0);

    battle.units[0].hp = 2_000;
    battle.resolve_enemy(2, enemy);
    assert_eq!(battle.hp(0).0, 1_180);
    battle.units[0].energy_milli = 135_000;
    act(&mut battle, FU, Action::Ultimate, None);
    assert_eq!(battle.hp(1).0, 845); // .05 * 4,240 + 133
    assert_eq!(battle.hp(0).0, 3_934); // 1,180 + .9 * (4,240 - 1,180)
    assert_eq!(battle.units[0].fu_xuan_state.restore_charges, 0);
    for _ in 0..3 {
        battle.units[0].energy_milli = 135_000;
        act(&mut battle, FU, Action::Ultimate, None);
    }
    assert_eq!(battle.units[0].fu_xuan_state.restore_charges, 2);
}

#[test]
fn e2_rescues_two_lethal_recipients_in_one_action_and_only_once_per_battle() {
    let mut plan = PreparedBattle::official_dpr(&[lynx(), fu_xuan(2, &[])], 42, 3).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 1_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FU, Action::Skill, None);
    for unit in &mut battle.units[..2] {
        unit.hp = 1;
    }

    battle.resolve_enemy(2, enemy);
    assert_eq!(battle.hp(0), (1_568, 2_240));
    assert_eq!(battle.hp(1), (2_968, 4_240));
    assert_eq!(battle.units[1].fu_xuan_state.restore_charges, 1);
    assert!(
        battle
            .events()
            .iter()
            .all(|event| event.kind != EventKind::UnitDefeated)
    );

    battle.units[1].hp = 1;
    battle.resolve_enemy(2, enemy);
    assert_eq!(battle.hp(1), (0, 4_000));
    assert_eq!(battle.hp(0), (1_281, 2_000));
    assert_eq!(battle.effective_crit_damage_bp(0), 5_000);
}

#[test]
fn a6_blocks_all_controls_in_one_action_before_effect_res_and_refreshes_on_skill() {
    let mut ly = lynx();
    ly.stats.effect_resistance_bp = 10_000;
    let mut plan = PreparedBattle::official_dpr(&[fu_xuan(0, &[A6]), ly], 42, 3).unwrap();
    finite_enemy(&mut plan, 2, 200);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    act(&mut battle, FU, Action::Skill, None);

    battle.enemy_action = Some(2);
    assert!(!battle.apply_skill_freeze(2, 1, 10_000, 1, 1_000, Action::Basic));
    battle.apply_skill_imprisonment(2, 0, 1_000, 2_000);
    assert!(!battle.is_crowd_controlled(0));
    battle.finish_enemy_action();

    battle.enemy_action = Some(2);
    assert!(battle.apply_skill_freeze(2, 0, 10_000, 1, 1_000, Action::Basic));
    battle.units[0].hp = 1_000;
    battle.finish_enemy_action();
    assert_eq!(battle.units[0].fu_xuan_state.restore_charges, 1);
    assert_eq!(battle.hp(0).0, 1_000);

    assert!(battle.clear_freeze(0));
    act(&mut battle, LYNX, Action::Basic, Some("dpr:2"));
    act(&mut battle, FU, Action::Skill, None);
    battle.enemy_action = Some(2);
    assert!(!battle.apply_skill_freeze(2, 0, 10_000, 1, 1_000, Action::Basic));
    battle.finish_enemy_action();
    assert_eq!(battle.units[0].fu_xuan_state.restore_charges, 0);
    assert_eq!(battle.hp(0).0, 3_916);
}

#[test]
fn e4_counts_an_attacked_ally_even_if_shield_absorbs_both_shares() {
    let mut fu = fu_xuan(6, &[]);
    fu.stats.energy_regeneration_bp = 12_000;
    let mut plan = PreparedBattle::official_dpr(&[lynx(), fu], 42, 3).unwrap();
    let enemy = finite_enemy(&mut plan, 2, 1_000);
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.units[1].energy_milli = 0;
    act(&mut battle, FU, Action::Skill, None);
    battle.apply_official_shield(0, 0, 500, 2, None);
    battle.apply_official_shield(0, 1, 500, 2, None);

    battle.resolve_enemy(2, enemy);
    assert_eq!(battle.hp(1), (4_264, 4_264));
    assert_eq!(battle.units[1].energy_milli, 42_000);
    assert_eq!(battle.units[1].fu_xuan_state.hp_loss_tally, 0);
    battle.units[0].departed = true;
    battle.resolve_enemy(2, enemy);
    assert_eq!(battle.units[1].energy_milli, 54_000);
    assert_eq!(battle.units[1].fu_xuan_state.hp_loss_tally, 163);
}

#[test]
fn e6_adds_capped_net_hp_loss_as_base_damage_then_clears_the_tally() {
    let plan = PreparedBattle::official_dpr(&[fu_xuan(6, &[]), lynx()], 42, 3).unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    battle.spend_hp(1, 100, 1);
    assert_eq!(battle.units[0].fu_xuan_state.hp_loss_tally, 0);
    act(&mut battle, FU, Action::Skill, None);
    battle.spend_hp(1, 5_000, 1);
    assert_eq!(battle.units[0].fu_xuan_state.hp_loss_tally, 2_163);
    battle.spend_hp(0, 5_000, 1);
    assert_eq!(battle.units[0].fu_xuan_state.hp_loss_tally, 5_116);

    battle.units[0].energy_milli = 135_000;
    act(&mut battle, FU, Action::Ultimate, None);
    assert_eq!(battle.units[0].fu_xuan_state.hp_loss_tally, 0);
    assert_eq!(battle.dummy_damage.len(), 3);
    for hit in &battle.dummy_damage {
        // (4,264 * 1.08 + 5,116 * 2) * (1000/1500) * .90, optional E1 crit.
        assert_eq!(hit.amount, if hit.critical { 16_024 } else { 8_902 });
    }

    battle.units[0].energy_milli = 135_000;
    act(&mut battle, FU, Action::Ultimate, None);
    let direct = battle.dummy_damage[3..]
        .iter()
        .filter(|hit| hit.kind == DamageKind::Direct);
    assert_eq!(direct.clone().count(), 3);
    for hit in direct {
        assert_eq!(hit.amount, if hit.critical { 4_973 } else { 2_763 });
    }
}

#[test]
fn fixed_dpr_attributes_matrix_to_fu_xuan_and_conserves_every_hit() {
    for eidolon in 0..=6 {
        let plan = PreparedBattle::official_dpr(&[fu_xuan(eidolon, &[A2, A4, A6]), lynx()], 42, 3)
            .unwrap();
        let scenario = crate::STANDARD_DPR_SCENARIO;
        let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
        assert_eq!(
            result,
            calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap()
        );
        let hits: Vec<_> = result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .collect();
        assert!(hits.iter().any(|hit| hit.source_id == FU
            && hit.action == Action::Ultimate
            && hit.kind == DamageKind::Direct));
        assert!(hits.iter().any(|hit| {
            hit.source_id == LYNX
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == FU && credit.amount > 0)
        }));
        for hit in hits {
            assert_eq!(
                hit.amount,
                hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            );
        }
    }
}
