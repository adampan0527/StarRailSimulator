use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "trailblazer.destruction";
const BASIC: &str = "skill.800101";
const SKILL: &str = "skill.800102";
const TALENT: &str = "skill.800104";
const SINGLE_ULTIMATE: &str = "skill.800108";
const BLAST_ULTIMATE: &str = "skill.800109";
const READY_FOR_BATTLE: &str = "trace.8001101";
const PERSEVERANCE: &str = "trace.8001102";
const FIGHTING_WILL: &str = "trace.8001103";
const ENERGY_CAPACITY: u32 = 120;
const TALENT_STACK_CAP: u8 = 2;
const E1_ENERGY_MILLI: u32 = 10_000;
const E2_HEAL_BP: u32 = 500;
const E4_CRIT_RATE_BP: u32 = 2_500;
const PHYSICAL: Element = Element::Physical;

#[derive(Clone, Default)]
pub(in crate::battle) struct DestructionTrailblazerState {
    talent_stacks: u8,
    ultimate_kill_energy_available: bool,
    hit_physical_weakness: bool,
}

struct DestructionTrailblazer {
    blast_ultimate: OfficialAbility,
    talent_attack_bp: u32,
    ready_for_battle: bool,
    perseverance: bool,
    fighting_will: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != PHYSICAL
        || input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    // Both avatar IDs resolve to the 8001 skill IDs in the frozen profile catalog.
    let basic = ratio_bp(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = ratio_bp(SKILL, input.parameters(SKILL, 1)?[0])?;
    let single = ratio_bp(SINGLE_ULTIMATE, input.parameters(SINGLE_ULTIMATE, 1)?[0])?;
    let blast = input.parameters(BLAST_ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    if talent[1] != f64::from(TALENT_STACK_CAP) {
        return Err(invalid(TALENT));
    }
    let has_trace = |trace| {
        input
            .enabled_trace_ids
            .iter()
            .any(|enabled| enabled == trace)
    };
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: attack(vec![hit(basic, 10)], 20_000, SkillPoints::Generate),
        skill: attack(
            vec![hit(skill, 20), adjacent_hit(skill, 10)],
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(vec![hit(single, 30)], 5_000, SkillPoints::Unchanged),
        mechanics: Arc::new(DestructionTrailblazer {
            blast_ultimate: attack(
                vec![
                    hit(ratio_bp(BLAST_ULTIMATE, blast[0])?, 20),
                    adjacent_hit(ratio_bp(BLAST_ULTIMATE, blast[1])?, 20),
                ],
                5_000,
                SkillPoints::Unchanged,
            ),
            talent_attack_bp: u32::from(ratio_bp(TALENT, talent[0])?),
            ready_for_battle: has_trace(READY_FOR_BATTLE),
            perseverance: has_trace(PERSEVERANCE),
            fighting_will: has_trace(FIGHTING_WILL),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: PHYSICAL,
        multiplier_bp,
        toughness,
    }
}

fn adjacent_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::BlastAdjacent {
        element: PHYSICAL,
        multiplier_bp,
        toughness,
    }
}

fn add_talent_stack(battle: &mut Battle<'_>, actor: usize) {
    let stacks = &mut battle.units[actor]
        .destruction_trailblazer_state
        .talent_stacks;
    *stacks = (*stacks + 1).min(TALENT_STACK_CAP);
}

impl Mechanics for DestructionTrailblazer {
    fn ability_override(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::EnhancedUltimate).then_some(&self.blast_ultimate)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        _: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        let target = target.filter(|_| action.is_ultimate())?;
        let has_both_neighbors = battle
            .adjacent_enemies(target)
            .filter(|&enemy| battle.is_targetable(enemy))
            .count()
            == 2;
        Some(u8::from(
            (action == Action::EnhancedUltimate) != has_both_neighbors,
        ))
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.ready_for_battle {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        battle.units[actor]
            .destruction_trailblazer_state
            .ultimate_kill_energy_available = self.eidolon >= 1 && action.is_ultimate();
    }

    fn before_attack_hit(&self, battle: &mut Battle<'_>, actor: usize, target: usize, _: Action) {
        if self.eidolon >= 2 && battle.has_weakness(target, PHYSICAL) {
            battle.units[actor]
                .destruction_trailblazer_state
                .hit_physical_weakness = true;
        }
    }

    fn on_attack_finished(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: Action) {
        if owner == actor
            && std::mem::take(
                &mut battle.units[actor]
                    .destruction_trailblazer_state
                    .hit_physical_weakness,
            )
        {
            let amount = percentage_of(battle.effective_attack(actor), E2_HEAL_BP);
            battle.heal_unit(actor, actor, amount);
        }
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            battle.units[owner]
                .destruction_trailblazer_state
                .ultimate_kill_energy_available = false;
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        _: usize,
        _: Action,
    ) {
        if owner == source {
            add_talent_stack(battle, owner);
        }
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if !battle.enemies().contains(&target) {
            return;
        }
        if self.eidolon >= 6 {
            add_talent_stack(battle, actor);
        }
        if std::mem::take(
            &mut battle.units[actor]
                .destruction_trailblazer_state
                .ultimate_kill_energy_available,
        ) {
            battle.gain_energy(actor, actor, E1_ENERGY_MILLI);
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner != target {
            return 0;
        }
        self.talent_attack_bp
            * u32::from(
                battle.units[owner]
                    .destruction_trailblazer_state
                    .talent_stacks,
            )
    }

    fn defense_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if !self.perseverance {
            return 0;
        }
        1_000
            * u32::from(
                battle.units[actor]
                    .destruction_trailblazer_state
                    .talent_stacks,
            )
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.fighting_will
            && kind == DamageKind::Direct
            && matches!(action, Some(Action::Skill | Action::EnhancedUltimate))
            && battle.current_target() == Some(target)
        {
            2_500
        } else {
            0
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _: usize,
        target: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.eidolon >= 4
            && kind == DamageKind::Direct
            && battle.units[target].toughness == 0
            && battle.plan.units[target].kit.max_toughness() > 0
        {
            E4_CRIT_RATE_BP
        } else {
            0
        }
    }
}
