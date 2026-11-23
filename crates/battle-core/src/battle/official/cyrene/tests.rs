use super::*;
use crate::{CombatType, Command, DamageKind, LogMode, OfficialStats, PreparedBattle};

fn character(
    form: &str,
    path: CombatPath,
    element: Element,
    energy_capacity: u32,
    rows: &[(&str, &[f64])],
) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: form.into(),
        form_id: form.into(),
        combat_type: CombatType { path, element },
        energy_capacity,
        level: 80,
        stats: OfficialStats {
            hp: 2_000,
            base_hp: 2_000,
            attack: 500,
            base_attack: 500,
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
            elation_bp: 0,
        },
        skill_parameters: rows
            .iter()
            .map(|&(id, values)| (id.into(), values.to_vec()))
            .collect(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

pub(in crate::battle::official) fn cyrene(eidolon: u8) -> OfficialCharacter {
    let mut role = character(
        FORM,
        CombatPath::Remembrance,
        Element::Ice,
        0,
        &[
            ("skill.141501", &[0.5, 1.0]),
            ("skill.141502", &[0.18, 2.0, 3.0]),
            ("skill.141503", &[1.0, 24.0, 0.375, 12.0]),
            ("skill.141504", &[1.0, 0.15, 27.0, 24.0, 12.0]),
            ("skill.141508", &[0.3, 3.0, 0.3]),
            ("skill.141514", &[12.0]),
            ("skill.1141501", &[0.6, 12.0]),
            ("skill.1141502", &[12.0, 0.4, 2.0]),
            ("skill.1141503", &[0.24, 0.0]),
            ("skill.1141513", &[0.16, 0.72, 1.0]),
            ("skill.1141514", &[70.0, 0.72, 0.36]),
            ("skill.1141515", &[1.0, 0.12, 3.0]),
            ("skill.1141516", &[2.0, 1.0]),
            ("skill.1141517", &[0.0, 0.0024, 2.0, 0.0, 0.0048, 2.0]),
            ("skill.1141518", &[3.0, 0.4, 0.6, 1.0]),
            ("skill.1141519", &[0.72, 24.0]),
            ("skill.1141520", &[0.36, 0.2, 0.12]),
            (
                "skill.1141521",
                &[0.16, 0.15, 5.0, 0.1, 12.0, 0.12, 0.72, 6.0, 4.0],
            ),
            ("skill.1141522", &[1.2, 0.6, 0.8, 60.0]),
            ("skill.1141523", &[0.3, 1.0]),
            ("skill.1141524", &[0.18, 1.0, 0.12]),
            ("skill.1141525", &[0.24, 2.0, 3.0, 0.8, 1.5]),
            ("skill.1141526", &[0.6, 3.0]),
        ],
    );
    role.eidolon = eidolon;
    role
}

fn evernight() -> OfficialCharacter {
    character(
        super::super::evernight::FORM,
        CombatPath::Remembrance,
        Element::Ice,
        240,
        &[
            ("skill.141301", &[0.5]),
            ("skill.141302", &[0.18, 2.0, 2.0, 0.5, 12.0, 0.1]),
            ("skill.141309", &[0.3, 2.0, 2.0, 0.5, 12.0, 0.1]),
            ("skill.141303", &[1.5, 2.0, 0.45, 0.225]),
            ("skill.141304", &[2.0, 0.45, 2.0, 160.0, 0.5, 16.0, 0.05]),
            ("skill.1141301", &[0.5, 0.1, 4.0, 1.0]),
            ("skill.1141303", &[0.5, 3.0]),
            ("skill.1141306", &[0.1, 0.01, 40.0]),
            ("skill.1141307", &[0.12, 0.06, 16.0]),
        ],
    )
}

fn herta() -> OfficialCharacter {
    let mut ally = character(
        "character.1013",
        CombatPath::Erudition,
        Element::Ice,
        110,
        &[
            ("skill.101301", &[1.0]),
            ("skill.101302", &[0.75, 0.5, 0.2]),
            ("skill.101303", &[1.6]),
            ("skill.101304", &[0.5, 0.325]),
        ],
    );
    ally.unit_id = "herta".into();
    ally
}

fn plan(team: &[OfficialCharacter]) -> PreparedBattle {
    PreparedBattle::official_dpr(team, 42, 3)
        .unwrap()
        .with_dummies(&[1_000; 3])
        .unwrap()
}

fn command(battle: &Battle<'_>, actor_id: &str, action: Action, target: Option<&str>) -> Command {
    Command {
        decision_index: battle.decision_index,
        actor_id: actor_id.into(),
        action,
        target_id: target.map(str::to_owned),
    }
}

fn act(battle: &mut Battle<'_>, actor: &str, action: Action, target: Option<&str>) {
    let command = command(battle, actor, action, target);
    assert!(battle.legal_actions().contains(&command), "{command:?}");
    battle.resolve_command(&command).unwrap();
}

#[test]
fn first_ultimate_fills_sabers_energy_without_creating_overflow() {
    let saber = super::super::saber::tests::saber(0, &["trace.1014102"], false);
    let saber_id = saber.unit_id.clone();
    let prepared = plan(&[cyrene(0), saber]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    assert_eq!(battle.units[1].energy_milli, 216_000);

    set_recollection(&mut battle, 0, 24);
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[1].energy_milli, 360_000);

    act(&mut battle, &saber_id, Action::Ultimate, None);
    assert_eq!(battle.units[1].energy_milli, 5_000);
}

#[test]
fn first_ultimate_refills_team_and_spends_twelve_then_story_takes_a_free_turn() {
    let prepared = plan(&[cyrene(1), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let ult = command(&battle, FORM, Action::EnhancedUltimate, Some("herta"));
    set_recollection(&mut battle, 0, 23);
    assert!(!battle.legal_actions().contains(&ult));
    set_recollection(&mut battle, 0, 24);
    assert!(battle.legal_actions().contains(&ult));
    battle.resolve_command(&ult).unwrap();

    let sprite = battle.active_memosprite(0).unwrap();
    assert_eq!(battle.speed_micros(sprite), 0);
    assert_eq!(battle.hp(sprite), (2_480, 2_480));
    assert_eq!(battle.units[0].cyrene_state.recollection, 12);
    assert_eq!(battle.units[0].cyrene_state.story, 2);
    assert_eq!(battle.units[1].energy_milli, 110_000);
    assert_eq!(battle.units[sprite].normal_turns, 1);
    assert!(
        !battle
            .legal_actions()
            .iter()
            .any(|action| action.actor_id == FORM && action.action == Action::Skill)
    );
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(
        rules.ally_damage_bonus_bp(&battle, 0, 1, Element::Ice),
        5_500
    );

    battle.units[1].normal_turns += 2;
    assert_eq!(
        rules.ally_damage_bonus_bp(&battle, 0, 1, Element::Ice),
        1_500
    );
    act(&mut battle, FORM, Action::Ultimate, None);
    assert_eq!(battle.units[0].cyrene_state.recollection, 6);
    assert_eq!(battle.units[0].cyrene_state.story, 0);
    assert_eq!(battle.units[sprite].normal_turns, 3);
    assert_eq!(
        battle
            .dummy_damage
            .iter()
            .filter(|hit| hit.actor == sprite && hit.kind == DamageKind::Direct)
            .count(),
        18
    );
}

#[test]
fn true_damage_is_separate_and_its_stacks_follow_future_and_distinct_ode_targets() {
    let mut cyrene = cyrene(2);
    cyrene.enabled_trace_ids = vec!["trace.1415101".into(), "trace.1415102".into()];
    let prepared = plan(&[cyrene, evernight(), herta()]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let evey = battle.active_memosprite(1).unwrap();
    let before = battle.units[0].cyrene_state.recollection;
    battle.notify_active_ability(1);
    battle.notify_active_ability(1);
    battle.notify_active_ability(evey);
    battle.notify_active_ability(evey);
    assert_eq!(battle.units[0].cyrene_state.recollection, before + 3);
    let contributors = &battle.units[0].cyrene_state.contributors;
    assert_eq!(contributors.len(), 2);
    assert!(contributors.contains(&1) && contributors.contains(&evey));

    set_recollection(&mut battle, 0, 24);
    act(&mut battle, FORM, Action::EnhancedUltimate, Some("herta"));
    let before = battle.dummy_damage.len();
    battle.attack_hit(2, 3, Element::Ice, 10_000, 0);
    let [direct, truth] = &battle.dummy_damage[before..] else {
        panic!("one true hit must follow one direct hit");
    };
    assert_eq!(truth.amount, percentage_of(direct.amount, 2_400));
    assert_eq!(truth.kind, DamageKind::TrueDamage);
    assert_eq!(truth.assists, [(0, truth.amount)]);
}

#[test]
fn time_ode_uses_frozen_evernight_conditions_and_leaves_ultimate_damage_outside_sprite_bonus() {
    let mut companion = evernight();
    companion.eidolon = 2;
    let mut invalid = evernight();
    invalid.skill_parameters.get_mut("skill.141309").unwrap()[2] = 3.0;
    assert!(super::super::evernight::build(&invalid).is_err());
    let prepared = plan(&[cyrene(0), companion]);
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    let evey = battle.active_memosprite(1).unwrap();
    set_recollection(&mut battle, 0, 24);
    act(
        &mut battle,
        FORM,
        Action::EnhancedUltimate,
        Some(super::super::evernight::FORM),
    );
    let rules = battle.mechanics(0).unwrap();
    assert_eq!(
        rules.ally_damage_bonus_bp(&battle, 0, evey, Element::Ice),
        3_300
    );
    battle.action = Some(Action::Ultimate);
    assert_eq!(
        rules.ally_damage_bonus_bp(&battle, 0, evey, Element::Ice),
        1_500
    );
    battle.action = None;

    let companion = battle.mechanics(1).unwrap();
    let before = battle.effective_crit_damage_bp(evey);
    let owner_crit_before = battle.effective_crit_damage_bp(1);
    companion.before_action(&mut battle, 1, Action::Skill, None);
    assert_eq!(
        battle.effective_crit_damage_bp(evey) - before,
        percentage_of(battle.effective_crit_damage_bp(1), 3_000)
            + battle.effective_crit_damage_bp(1)
            - owner_crit_before
    );
    assert!(matches!(
        battle.events().last().unwrap().kind,
        EventKind::Memoria { before, after } if after == before + 4
    ));
    for action in [Action::Skill, Action::Ultimate] {
        let previous_events = battle.events().len();
        companion.after_action(&mut battle, 1, action, None);
        let gains = battle.events()[previous_events..]
            .iter()
            .filter_map(|event| match event.kind {
                EventKind::Memoria { before, after } => Some(after - before),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(gains, [3], "{action:?}");
    }
}
