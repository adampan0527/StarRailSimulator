use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1225";
const BASIC: &str = "skill.122501";
const SKILL: &str = "skill.122502";
const ULTIMATE: &str = "skill.122503";
const TALENT: &str = "skill.122504";
const ENHANCED_BASIC: &str = "skill.122508";
const A2: &str = "trace.1225101";
const A4: &str = "trace.1225102";
const A6: &str = "trace.1225103";
const FIRE: Element = Element::Fire;
const FOXIAN_EFFICIENCY_BP: u32 = 5_000;

#[derive(Clone, Copy, Default)]
pub(in crate::battle) struct FugueState {
    prayer: Option<FoxianPrayer>,
    refunded_first_skill: bool,
    last_break_bonus_bp: [u32; 4],
}

#[derive(Clone, Copy)]
struct FoxianPrayer {
    target: usize,
    expires_at_turn: u32,
}

struct Fugue {
    eidolon: u8,
    prayer_turns: u32,
    prayer_break_effect_bp: u32,
    defense_chance_bp: u32,
    defense_down_bp: u32,
    defense_turns: u32,
    off_element_bp: u16,
    super_break_bp: u16,
    extra_toughness_bp: u32,
    break_delay: bool,
    refund_skill: bool,
    share_break: bool,
    enhanced_basic: OfficialAbility,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != FIRE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 6)?;
    let ultimate = input.parameters(ULTIMATE, 1)?;
    let talent = input.parameters(TALENT, 2)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 2)?;
    if skill[0] != 3.0 || skill[2] != 1.0 || skill[4] != 2.0 || skill[5] != 0.5 {
        return Err(invalid(SKILL));
    }
    if talent[1] != 0.4 {
        return Err(invalid(TALENT));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ULTIMATE, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Fugue {
            eidolon: input.eidolon,
            prayer_turns: skill[0] as u32,
            prayer_break_effect_bp: u32::from(basis_points(SKILL, skill[1])?),
            defense_chance_bp: u32::from(basis_points(SKILL, skill[2])?),
            defense_down_bp: u32::from(basis_points(SKILL, skill[3])?),
            defense_turns: skill[4] as u32,
            off_element_bp: basis_points(SKILL, skill[5])?,
            super_break_bp: basis_points(TALENT, talent[0])?,
            extra_toughness_bp: u32::from(basis_points(TALENT, talent[1])?),
            break_delay: has_trace(A2),
            refund_skill: has_trace(A4),
            share_break: has_trace(A6),
            enhanced_basic: OfficialAbility {
                target: Target::Enemy,
                energy_milli: 20_000,
                effects: vec![
                    hit(basis_points(ENHANCED_BASIC, enhanced[0])?, 10),
                    Effect::BlastAdjacent {
                        element: FIRE,
                        multiplier_bp: basis_points(ENHANCED_BASIC, enhanced[1])?,
                        toughness: 5,
                    },
                ],
                skill_points: SkillPoints::Generate,
            },
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: FIRE,
        multiplier_bp,
        toughness,
    }
}

fn active_prayer(battle: &Battle<'_>, owner: usize) -> Option<FoxianPrayer> {
    let unit = &battle.units[owner];
    unit.fugue_state
        .prayer
        .filter(|prayer| battle.is_targetable(owner) && unit.normal_turns < prayer.expires_at_turn)
}

impl Fugue {
    fn blesses(&self, battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        active_prayer(battle, owner).is_some_and(|prayer| {
            battle.allies().contains(&target) && (self.eidolon >= 6 || prayer.target == target)
        })
    }

    fn share_break_effect(&self, battle: &mut Battle<'_>, owner: usize) {
        let above_threshold = battle.effective_break_effect(owner) >= 22_000;
        let value = if above_threshold { 1_800 } else { 600 };
        for ally in battle.allies().filter(|&ally| ally != owner) {
            if !battle.is_targetable(ally) {
                continue;
            }
            let previous = if battle.ally_buff_value(owner, ally, AllyBuffKind::BreakEffect) > 0 {
                battle.units[owner].fugue_state.last_break_bonus_bp[ally]
            } else {
                0
            };
            battle.units[owner].fugue_state.last_break_bonus_bp[ally] = value;
            battle.apply_ally_buff_with_id(
                owner,
                ally,
                AllyBuffKind::BreakEffect,
                previous + value,
                2,
                Some(A6),
            );
        }
    }
}

impl Mechanics for Fugue {
    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        for target in battle.enemies() {
            let maximum = battle.plan.units[target].kit.max_toughness();
            let extra = percentage_of(u32::from(maximum), self.extra_toughness_bp) as u16;
            battle.grant_extra_toughness(owner, target, extra);
        }
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && active_prayer(battle, actor).is_some())
            .then_some(&self.enhanced_basic)
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::ElationSkill => {}
            Action::Skill | Action::EnhancedSkill => {
                let Some(target) = target else {
                    return;
                };
                let unit = &mut battle.units[owner];
                unit.fugue_state.prayer = Some(FoxianPrayer {
                    target,
                    expires_at_turn: unit.normal_turns.saturating_add(self.prayer_turns),
                });
                if self.refund_skill && !unit.fugue_state.refunded_first_skill {
                    unit.fugue_state.refunded_first_skill = true;
                    battle.restore_skill_points(owner, 1);
                }
            }
            Action::Ultimate if self.eidolon >= 2 => {
                for ally in battle.allies() {
                    if battle.is_targetable(ally) {
                        battle.advance_ally(owner, ally, 2_400);
                    }
                }
            }
            Action::Basic | Action::Ultimate | Action::EnhancedUltimate => {}
        }
    }

    fn on_ally_first_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: usize,
    ) {
        if self.blesses(battle, owner, actor)
            && battle.roll_effect(owner, target, self.defense_chance_bp)
        {
            battle.apply_debuff(
                owner,
                target,
                DebuffKind::DefenseDown,
                self.defense_down_bp,
                self.defense_turns,
            );
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: usize,
        _action: Action,
    ) {
        if !battle.is_targetable(owner) || !battle.allies().contains(&actor) {
            return;
        }
        if self.break_delay && battle.is_targetable(target) {
            battle.delay_action(owner, target, 1_500);
        }
        if self.eidolon >= 2 {
            battle.gain_energy(owner, owner, 3_000);
        }
        if self.share_break {
            self.share_break_effect(battle, owner);
        }
    }

    fn ignores_weakness(&self, action: Option<Action>, kind: DamageKind) -> bool {
        action == Some(Action::Ultimate) && kind == DamageKind::Direct
    }

    fn provides_super_break(&self) -> bool {
        true
    }

    fn super_break_multiplier_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
        _target: usize,
    ) -> u16 {
        self.super_break_bp
    }

    fn break_effect_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        if self.refund_skill { 3_000 } else { 0 }
    }

    fn toughness_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        if self.eidolon >= 6 {
            FOXIAN_EFFICIENCY_BP
        } else {
            0
        }
    }

    fn ally_break_effect_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        if self.blesses(battle, owner, actor) {
            self.prayer_break_effect_bp
        } else {
            0
        }
    }

    fn ally_toughness_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        if self.eidolon >= 1 && self.blesses(battle, owner, actor) {
            FOXIAN_EFFICIENCY_BP
        } else {
            0
        }
    }

    fn off_element_toughness_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u16 {
        if self.blesses(battle, owner, actor) {
            self.off_element_bp
        } else {
            0
        }
    }

    fn ally_break_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        if self.eidolon >= 4 && self.blesses(battle, owner, actor) {
            2_000
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::ElationSkill => None,
            Action::Ultimate => Some(0),
            Action::Skill | Action::EnhancedSkill => {
                let current = active_prayer(battle, owner);
                let needs_prayer =
                    current.is_none_or(|prayer| !battle.is_targetable(prayer.target));
                let preferred = battle
                    .allies()
                    .filter(|&ally| battle.is_targetable(ally))
                    .max_by_key(|&ally| (battle.effective_break_effect(ally), ally != owner));
                Some(if needs_prayer && target == preferred {
                    1
                } else {
                    5
                })
            }
            Action::Basic | Action::EnhancedUltimate => None,
        }
    }
}
