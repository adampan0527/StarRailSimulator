use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::super::Battle;
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

const BASIS_POINTS: u32 = 10_000;
const E1_SKILL_BONUS_BP: u32 = 1_000;
const E6_ULTIMATE_BONUS_BP: u32 = 2_000;
const REVIVAL_THRESHOLD_BP: u32 = 3_000;
const REVIVAL_HEAL_BP: u32 = 2_000;

struct Arlan {
    skill_hp_cost_bp: u32,
    talent_bonus_bp: u32,
    ultimate_primary_bp: u16,
    eidolon: u8,
    revival: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    let basic_bp = parameter_bp(input, "skill.100801", 1, 0)?;
    let skill_hp_cost_bp = parameter_bp(input, "skill.100802", 2, 0)?;
    let skill_bp = parameter_bp(input, "skill.100802", 2, 1)?;
    let ultimate_primary_bp = parameter_bp(input, "skill.100803", 2, 0)?;
    let ultimate_adjacent_bp = parameter_bp(input, "skill.100803", 2, 1)?;
    let talent_bonus_bp = parameter_bp(input, "skill.100804", 1, 0)?;
    if u32::from(skill_hp_cost_bp) > BASIS_POINTS || u32::from(talent_bonus_bp) > BASIS_POINTS {
        return Err(BattleError::InvalidInput(
            "character.1008 HP cost or talent bonus exceeds 100%".into(),
        ));
    }
    let max_energy_milli = input
        .energy_capacity
        .checked_mul(1_000)
        .ok_or_else(|| BattleError::InvalidInput("character.1008 energy capacity".into()))?;

    Ok(OfficialKit {
        max_energy_milli,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basic_bp, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![hit(skill_bp, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 5_000,
            effects: vec![
                hit(ultimate_primary_bp, 20),
                Effect::BlastAdjacent {
                    element: Element::Thunder,
                    multiplier_bp: ultimate_adjacent_bp,
                    toughness: 20,
                },
            ],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Arlan {
            skill_hp_cost_bp: u32::from(skill_hp_cost_bp),
            talent_bonus_bp: u32::from(talent_bonus_bp),
            ultimate_primary_bp,
            eidolon: input.eidolon,
            revival: input
                .enabled_trace_ids
                .iter()
                .any(|id| id == "trace.1008101"),
        }),
    })
}

fn parameter_bp(
    input: &OfficialCharacter,
    skill_id: &str,
    count: usize,
    index: usize,
) -> Result<u16, BattleError> {
    let ratio = input.parameters(skill_id, count)?[index];
    let bp = (ratio * f64::from(BASIS_POINTS)).round();
    if ratio <= 0.0 || !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(BattleError::InvalidInput(format!(
            "character.1008 {skill_id} parameter {} cannot be represented as basis points",
            index + 1
        )));
    }
    Ok(bp as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Thunder,
        multiplier_bp,
        toughness,
    }
}

fn hp_at_ratio(max_hp: u32, ratio_bp: u32) -> u32 {
    (u64::from(max_hp) * u64::from(ratio_bp) / u64::from(BASIS_POINTS)) as u32
}

impl Mechanics for Arlan {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Skill {
            let (_, max_hp) = battle.hp(actor);
            battle.spend_hp(actor, hp_at_ratio(max_hp, self.skill_hp_cost_bp), 1);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(actor);
        self.talent_bonus(hp, max_hp) + self.eidolon_bonus(hp, max_hp, action)
    }

    fn blast_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        multiplier_bp: u16,
    ) -> u16 {
        let (hp, max_hp) = battle.hp(actor);
        if self.eidolon >= 6 && action == Some(Action::Ultimate) && hp <= max_hp / 2 {
            return self.ultimate_primary_bp;
        }
        multiplier_bp
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, _target: usize) {
        let (hp, max_hp) = battle.hp(actor);
        if self.revival && hp <= hp_at_ratio(max_hp, REVIVAL_THRESHOLD_BP) {
            battle.heal_unit(actor, actor, hp_at_ratio(max_hp, REVIVAL_HEAL_BP));
        }
    }
}

impl Arlan {
    fn talent_bonus(&self, hp: u32, max_hp: u32) -> u32 {
        let missing_hp = u64::from(max_hp - hp);
        (missing_hp * u64::from(self.talent_bonus_bp) / u64::from(max_hp)) as u32
    }

    fn eidolon_bonus(&self, hp: u32, max_hp: u32, action: Option<Action>) -> u32 {
        if hp > max_hp / 2 {
            return 0;
        }
        match action {
            Some(Action::Skill) if self.eidolon >= 1 => E1_SKILL_BONUS_BP,
            Some(Action::Ultimate) if self.eidolon >= 6 => E6_ULTIMATE_BONUS_BP,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, LogMode, OfficialStats, PreparedBattle, STANDARD_DPR_SCENARIO, calculate_dpr,
    };

    fn mechanics(eidolon: u8) -> Arlan {
        Arlan {
            skill_hp_cost_bp: 1_500,
            talent_bonus_bp: 7_200,
            ultimate_primary_bp: 32_000,
            eidolon,
            revival: false,
        }
    }

    #[test]
    fn talent_scales_with_missing_hp_including_odd_max_hp() {
        let arlan = mechanics(0);
        assert_eq!(arlan.talent_bonus(997, 997), 0);
        assert_eq!(arlan.talent_bonus(847, 997), 1_083);
        assert_eq!(arlan.talent_bonus(1, 997), 7_192);
    }

    #[test]
    fn eidolon_bonuses_apply_to_their_actions_at_or_below_half_hp() {
        let arlan = mechanics(6);
        for action in [Action::Basic, Action::Skill, Action::Ultimate] {
            assert_eq!(arlan.eidolon_bonus(501, 1_000, Some(action)), 0);
        }
        assert_eq!(arlan.eidolon_bonus(500, 1_001, Some(Action::Basic)), 0);
        assert_eq!(arlan.eidolon_bonus(500, 1_001, Some(Action::Skill)), 1_000);
        assert_eq!(
            arlan.eidolon_bonus(499, 1_000, Some(Action::Ultimate)),
            2_000
        );
        assert_eq!(arlan.eidolon_bonus(1, 1_000, None), 0);
        assert_eq!(
            mechanics(5).eidolon_bonus(1, 1_000, Some(Action::Ultimate)),
            0
        );
    }

    fn character(eidolon: u8) -> OfficialCharacter {
        OfficialCharacter {
            unit_id: "ally.arlan".into(),
            form_id: "character.1008".into(),
            energy_capacity: 110,
            level: 80,
            stats: OfficialStats {
                hp: 1_000,
                base_hp: 1_000,
                attack: 1_000,
                base_attack: 1_000,
                defense: 500,
                base_defense: 500,
                base_speed_micros: 200_000_000,
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
                ("skill.100801".into(), vec![0.5]),
                ("skill.100802".into(), vec![0.15, 1.2]),
                ("skill.100803".into(), vec![1.92, 0.96]),
                ("skill.100804".into(), vec![0.36]),
            ]
            .into(),
            eidolon,
            enabled_trace_ids: Vec::new(),
        }
    }

    fn plan(eidolon: u8) -> PreparedBattle {
        PreparedBattle::official_dpr(&[character(eidolon)], 42, 0).unwrap()
    }

    fn command(decision_index: u32, action: Action, target: &str) -> Command {
        Command {
            decision_index,
            actor_id: "ally.arlan".into(),
            action,
            target_id: Some(target.into()),
        }
    }

    #[test]
    fn basic_generates_a_skill_point_and_keeps_full_hp() {
        let plan = plan(0);
        let command = command(0, Action::Basic, "dpr:2");
        let battle = Battle::replay(&plan, &[command], LogMode::Detailed).unwrap();
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.units[0].hp, 1_000);
        assert_eq!(snapshot.units[0].energy_milli, 75_000);
        assert_eq!(snapshot.units[2].toughness, 110);
        assert_eq!(snapshot.skill_points, 1);
        assert_eq!(battle.dummy_damage[0].amount, 300);
    }

    #[test]
    fn skill_uses_no_skill_points_and_applies_talent_after_hp_cost() {
        let plan = plan(0);
        let mut battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.snapshot().units[0].energy_milli, 55_000);
        let skill = command(0, Action::Skill, "dpr:2");
        assert!(battle.legal_actions().contains(&skill));
        battle.apply(&skill).unwrap();
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.units[0].hp, 850);
        assert_eq!(snapshot.units[0].energy_milli, 85_000);
        assert_eq!(snapshot.units[2].toughness, 100);
        assert_eq!(snapshot.skill_points, 0);
        // 1000 ATK × 1.2 × (1 + 0.15 × 0.36) × 1000/(1000+500) × 0.9.
        assert_eq!(battle.dummy_damage[0].amount, 758);

        for (decision, hp) in (1..).zip([700, 550, 400, 250, 100, 1, 1]) {
            battle
                .apply(&command(decision, Action::Skill, "dpr:2"))
                .unwrap();
            assert_eq!(battle.snapshot().units[0].hp, hp);
            assert_eq!(battle.snapshot().skill_points, 0);
        }
    }

    #[test]
    fn ultimate_above_half_hp_keeps_the_default_blast_multipliers() {
        let plan = plan(6);
        let commands = [
            command(0, Action::Skill, "dpr:2"),
            command(1, Action::Skill, "dpr:2"),
            command(2, Action::Ultimate, "dpr:2"),
        ];
        let battle = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
        let hits: Vec<_> = battle.dummy_damage[2..]
            .iter()
            .map(|hit| (hit.target, hit.amount))
            .collect();
        assert_eq!(battle.snapshot().units[0].hp, 700);
        assert_eq!(hits, [(2, 1_276), (1, 638), (3, 638)]);
    }

    #[test]
    fn dpr_policy_uses_skill_at_zero_skill_points() {
        let plan = plan(0);
        let scenario = STANDARD_DPR_SCENARIO;
        let dpr = calculate_dpr(&plan, &scenario.toughness, scenario.cycles).unwrap();
        let first_hit = &dpr.targets[1].hits[0];
        assert_eq!(first_hit.source_id, "ally.arlan");
        assert_eq!(first_hit.action, Action::Skill);
        assert_eq!(first_hit.amount, 758);
        assert!(
            dpr.targets[0]
                .hits
                .iter()
                .any(|hit| hit.action == Action::Ultimate)
        );
    }

    #[test]
    fn revival_only_heals_after_defeating_a_target_with_low_hp_and_trace_enabled() {
        use super::super::super::Kit;
        use crate::{EventKind, fixture};

        for (trace_enabled, skills, expected_hp) in
            [(true, 4, 400), (false, 5, 250), (true, 5, 450)]
        {
            let mut arlan = character(0);
            if trace_enabled {
                arlan.enabled_trace_ids.push("trace.1008101".into());
            }
            let mut plan = PreparedBattle::official_dpr(&[arlan], 42, 0).unwrap();
            // Replace one dummy with a killable foe that cannot act in this replay.
            let target = &mut plan.units[1];
            target.kit = Kit::Enemy(fixture::enemy("fixture.target_left", 32).unwrap());
            target.stats.hp = 1;
            target.stats.base_speed_micros = 1_000_000;
            let commands: Vec<_> = (0..skills)
                .map(|decision| command(decision, Action::Skill, "dpr:2"))
                .chain([command(skills, Action::Basic, "dpr:1")])
                .collect();
            let battle = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
            assert!(
                battle
                    .events()
                    .iter()
                    .any(|event| event.kind == EventKind::UnitDefeated)
            );
            assert_eq!(battle.snapshot().units[0].hp, expected_hp);
        }
    }

    #[test]
    fn e6_blast_replay_restores_energy_and_only_hits_neighboring_slots() {
        let plan = plan(6);
        let commands = [
            command(0, Action::Skill, "dpr:2"),
            command(1, Action::Skill, "dpr:2"),
            command(2, Action::Skill, "dpr:2"),
            command(3, Action::Skill, "dpr:2"),
            command(4, Action::Ultimate, "dpr:1"),
        ];
        let battle = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.units[0].hp, 400);
        assert_eq!(snapshot.units[0].energy_milli, 5_000);
        assert_eq!(snapshot.units[1].toughness, 10);
        assert_eq!(snapshot.units[2].toughness, 20);
        assert_eq!(snapshot.units[3].toughness, 30);
        let hits: Vec<_> = battle
            .dummy_damage
            .iter()
            .map(|hit| (hit.target, hit.amount))
            .collect();
        // E1 fourth skill and E6 both ult hits use the post-cost 40% HP state.
        assert_eq!(hits[3..], [(2, 947), (1, 1_631), (2, 1_631)]);

        let repeated = Battle::replay(&plan, &commands, LogMode::Detailed).unwrap();
        assert_eq!(repeated.snapshot(), snapshot);
        assert_eq!(repeated.events(), battle.events());
    }
}
