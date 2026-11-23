use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1313";
const BASIC: &str = "skill.131301";
const SKILL: &str = "skill.131302";
const ULTIMATE: &str = "skill.131303";
const TALENT: &str = "skill.131304";
const REST_DAYS_LONGING: &str = "trace.1313101";
const EXALTED_SWEEP: &str = "trace.1313102";
const HAVEN_IN_PALM: &str = "trace.1313103";

const ENERGY_CAP: u32 = 130;
const ADVANCE_BP: u16 = 10_000;
const E1_CHARACTER_DEF_IGNORE_BP: u32 = 1_600;
const E1_SUMMON_DEF_IGNORE_BP: u32 = 4_000;
const E2_DAMAGE_BP: u32 = 3_000;
const E6_OVERFLOW_RATIO_BP: u32 = 20_000;

#[derive(Clone, Copy)]
struct Beatified {
    target: usize,
    sprite: Option<(usize, u32)>,
    bonus_crit_damage_bp: u32,
    turns: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct SundayState {
    beatified: Option<Beatified>,
    first_ultimate_refunded: bool,
    talent_stacks: [u8; 4],
}

struct Sunday {
    eidolon: u8,
    rest_days_longing: bool,
    exalted_sweep: bool,
    haven_in_palm: bool,
    damage_bp: u32,
    summon_damage_bp: u32,
    damage_turns: u32,
    energy_restore_bp: u32,
    beatified_ratio_bp: u32,
    beatified_flat_bp: u32,
    beatified_turns: u8,
    crit_rate_bp: u32,
    crit_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != Element::Imaginary
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 2)?;
    if skill[0] != 1.0 || skill[2] != 2.0 {
        return Err(invalid(SKILL));
    }
    if ultimate[0] != 0.2 || ultimate[2] != 3.0 {
        return Err(invalid(ULTIMATE));
    }
    if talent[1] != 3.0 {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Imaginary,
                multiplier_bp: basis_points(BASIC, basic)?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::OtherAlly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Sunday {
            eidolon: input.eidolon,
            rest_days_longing: trace(REST_DAYS_LONGING),
            exalted_sweep: trace(EXALTED_SWEEP),
            haven_in_palm: trace(HAVEN_IN_PALM),
            damage_bp: u32::from(basis_points(SKILL, skill[1])?),
            summon_damage_bp: u32::from(basis_points(SKILL, skill[3])?),
            damage_turns: skill[2] as u32,
            energy_restore_bp: u32::from(basis_points(ULTIMATE, ultimate[0])?),
            beatified_ratio_bp: u32::from(basis_points(ULTIMATE, ultimate[1])?),
            beatified_flat_bp: u32::from(basis_points(ULTIMATE, ultimate[3])?),
            beatified_turns: ultimate[2] as u8,
            crit_rate_bp: u32::from(basis_points(TALENT, talent[0])?),
            crit_turns: talent[1] as u32 + u32::from(input.eidolon >= 6),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn eligible_for_advance(battle: &Battle<'_>, target: usize) -> bool {
    battle.plan.units[target]
        .combat_type
        .is_none_or(|combat| combat.path != CombatPath::Harmony)
}

fn has_combat_summon(battle: &Battle<'_>, target: usize) -> bool {
    battle
        .allies()
        .any(|owner| battle.has_advanceable_summon(owner, target))
}

fn preferred_target(battle: &Battle<'_>, actor: usize) -> Option<usize> {
    battle
        .allies()
        .filter(|&target| target != actor && battle.is_targetable(target))
        .max_by_key(|&target| {
            (
                eligible_for_advance(battle, target),
                has_combat_summon(battle, target),
                battle.plan.units[target].stats.attack,
                Reverse(target),
            )
        })
}

impl Sunday {
    fn beatified(battle: &Battle<'_>, owner: usize, target: usize) -> Option<Beatified> {
        battle.units[owner]
            .sunday_state
            .beatified
            .filter(|blessing| {
                blessing.target == target
                    || blessing.sprite == Some((target, battle.units[target].memosprite_generation))
            })
    }

    fn apply_talent(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let stacks = if self.eidolon >= 6
            && battle.has_ally_buff(actor, target, AllyBuffKind::CritRate, TALENT)
        {
            (battle.units[actor].sunday_state.talent_stacks[target] + 1).min(3)
        } else {
            1
        };
        battle.units[actor].sunday_state.talent_stacks[target] = stacks;
        for recipient in Self::buff_targets(battle, target).into_iter().flatten() {
            battle.apply_ally_buff_with_id(
                actor,
                recipient,
                AllyBuffKind::CritRate,
                self.crit_rate_bp * u32::from(stacks),
                self.crit_turns,
                Some(TALENT),
            );
            if self.eidolon >= 6 {
                battle.apply_ally_buff_with_id(
                    actor,
                    recipient,
                    AllyBuffKind::CritOverflowRatio,
                    E6_OVERFLOW_RATIO_BP,
                    self.crit_turns,
                    Some(TALENT),
                );
            }
        }
    }

    fn buff_targets(battle: &Battle<'_>, target: usize) -> [Option<usize>; 2] {
        [Some(target), battle.active_memosprite(target)]
    }

    fn skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if self.haven_in_palm {
            battle.cleanse_one(actor, target);
        }
        let extra = u32::from(has_combat_summon(battle, target)) * self.summon_damage_bp;
        for recipient in Self::buff_targets(battle, target).into_iter().flatten() {
            battle.apply_ally_buff_with_id(
                actor,
                recipient,
                AllyBuffKind::Damage,
                self.damage_bp + extra,
                self.damage_turns,
                Some(SKILL),
            );
        }
        self.apply_talent(battle, actor, target);
        if self.eidolon >= 1 {
            for recipient in Self::buff_targets(battle, target).into_iter().flatten() {
                for (summon, bp) in [
                    (false, E1_CHARACTER_DEF_IGNORE_BP),
                    (true, E1_SUMMON_DEF_IGNORE_BP),
                ] {
                    battle.apply_ally_buff_with_id(
                        actor,
                        recipient,
                        AllyBuffKind::DefenseIgnore { summon },
                        bp,
                        2,
                        Some(SKILL),
                    );
                }
            }
        }
        if Self::beatified(battle, actor, target).is_some() {
            battle.restore_skill_points(actor, 1);
        }
        if eligible_for_advance(battle, target) {
            battle.advance_ally_immediate(actor, target);
            for owner in battle.allies() {
                if battle.has_advanceable_summon(owner, target)
                    && let Some(summon) = &mut battle.units[owner].summon_turn
                {
                    // Equal advanced AV puts the chosen ally before the summon.
                    summon.advance(ADVANCE_BP);
                }
            }
        }
    }

    fn ultimate(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let cap = battle.plan.units[target].kit.max_energy_milli();
        let restored = percentage_of(cap, self.energy_restore_bp);
        let minimum = 40_000 * u32::from(self.rest_days_longing);
        battle.restore_energy_flat(actor, target, restored.max(minimum));
        let bonus_crit_damage_bp = percentage_of(
            battle.effective_crit_damage_bp(actor),
            self.beatified_ratio_bp,
        )
        .saturating_add(self.beatified_flat_bp);
        let sprite = battle
            .active_memosprite(target)
            .map(|sprite| (sprite, battle.units[sprite].memosprite_generation));
        let state = &mut battle.units[actor].sunday_state;
        state.beatified = Some(Beatified {
            target,
            sprite,
            bonus_crit_damage_bp,
            turns: self.beatified_turns,
        });
        if self.eidolon >= 2 && !state.first_ultimate_refunded {
            state.first_ultimate_refunded = true;
            battle.restore_skill_points(actor, 2);
        }
        if self.eidolon >= 6 {
            self.apply_talent(battle, actor, target);
        }
    }
}

impl Mechanics for Sunday {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.exalted_sweep {
            battle.gain_energy(actor, actor, 25_000);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner != actor || extra {
            return;
        }
        let beatified = &mut battle.units[actor].sunday_state.beatified;
        if let Some(blessing) = beatified {
            blessing.turns -= 1;
            if blessing.turns == 0 {
                *beatified = None;
            }
        }
        if self.eidolon >= 4 {
            battle.gain_energy(actor, actor, 8_000);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].sunday_state.beatified = None;
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.skill(battle, actor, target),
            (Action::Ultimate, Some(target)) => self.ultimate(battle, actor, target),
            _ => {}
        }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        Self::beatified(battle, owner, target).map_or(0, |blessing| blessing.bonus_crit_damage_bp)
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _element: Element,
    ) -> u32 {
        E2_DAMAGE_BP
            * u32::from(self.eidolon >= 2 && Self::beatified(battle, owner, target).is_some())
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if target != preferred_target(battle, actor) || target.is_none() {
            return None;
        }
        match action {
            Action::AssistSkill | Action::ElationSkill => None,
            Action::Ultimate => Some(0),
            Action::Skill | Action::EnhancedSkill => Some(2),
            Action::Basic | Action::EnhancedUltimate => None,
        }
    }
}
