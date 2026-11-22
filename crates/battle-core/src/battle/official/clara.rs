use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::CounterAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110701";
const SKILL: &str = "skill.110702";
const ULTIMATE: &str = "skill.110703";
const TALENT: &str = "skill.110704";
const KINSHIP: &str = "trace.1107101";
const UNDER_PROTECTION: &str = "trace.1107102";
const REVENGE: &str = "trace.1107103";
const BP: u32 = 10_000;
const COUNTER_TOUGHNESS: u16 = 10;
const COUNTER_ENERGY_MILLI: u32 = 5_000;

struct Clara {
    eidolon: u8,
    kinship: bool,
    under_protection: bool,
    revenge: bool,
    marked_hit_bp: u16,
    counter_bp: u16,
    enhanced_counter_bp: u16,
    enhanced_charges: u8,
    damage_reduction_bp: u32,
    ultimate_damage_reduction_bp: u32,
    ultimate_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "Clara energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 3)?;
    let counter_bp = basis_points(TALENT, talent[1])?;
    let enhanced_counter_bp = counter_bp
        .checked_add(basis_points(ULTIMATE, ultimate[1])?)
        .ok_or_else(|| invalid_parameter(ULTIMATE))?;
    let ultimate_turns = integer_parameter(ULTIMATE, ultimate[2])?;
    let charges = integer_parameter(ULTIMATE, ultimate[4])?;
    let damage_reduction_bp = reduction_bp(TALENT, talent[2])?;
    let ultimate_damage_reduction_bp = reduction_bp(ULTIMATE, ultimate[3])?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let mechanics = Clara {
        eidolon: input.eidolon,
        kinship: has_trace(KINSHIP),
        under_protection: has_trace(UNDER_PROTECTION),
        revenge: has_trace(REVENGE),
        marked_hit_bp: basis_points(SKILL, skill[1])?,
        counter_bp,
        enhanced_counter_bp,
        enhanced_charges: charges + u8::from(input.eidolon >= 6),
        damage_reduction_bp,
        ultimate_damage_reduction_bp,
        ultimate_turns: u32::from(ultimate_turns),
    };
    Ok(OfficialKit {
        max_energy_milli: input.energy_capacity * 1_000,
        basic: attack(
            Target::Enemy,
            basis_points(BASIC, basic[0])?,
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            Target::AllEnemies,
            basis_points(SKILL, skill[0])?,
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn attack(
    target: Target,
    multiplier_bp: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Physical,
            multiplier_bp,
            toughness: 10,
        }],
        skill_points,
    }
}

fn invalid_parameter(skill: &str) -> BattleError {
    BattleError::InvalidInput(format!("Clara {skill} has an out-of-range parameter"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * f64::from(BP)).round();
    if (1.0..=f64::from(u16::MAX)).contains(&rounded) {
        Ok(rounded as u16)
    } else {
        Err(invalid_parameter(skill))
    }
}

fn reduction_bp(skill: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio > 1.0 {
        return Err(invalid_parameter(skill));
    }
    Ok((ratio * f64::from(BP)).round() as u32)
}

fn integer_parameter(skill: &str, value: f64) -> Result<u8, BattleError> {
    if (1.0..f64::from(u8::MAX)).contains(&value) && value.fract() == 0.0 {
        Ok(value as u8)
    } else {
        Err(invalid_parameter(skill))
    }
}

impl Mechanics for Clara {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::ElationSkill => {}
            Action::Ultimate | Action::EnhancedUltimate => {
                let state = &mut battle.units[actor];
                state.counter_state.enhanced_charges = self.enhanced_charges;
                state.counter_state.ult_expires_after_turn =
                    state.normal_turns + self.ultimate_turns;
                if self.eidolon >= 2 {
                    battle.apply_attack_bonus(actor, 3_000, 2);
                }
            }
            Action::Skill | Action::EnhancedSkill if self.eidolon == 0 => {
                battle.units[actor].counter_state.marked.clear()
            }
            Action::Basic | Action::Skill | Action::EnhancedSkill => {}
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if action == Action::Skill && battle.units[actor].counter_state.marked.contains(&target) {
            battle.secondary_hit(
                actor,
                target,
                Element::Physical,
                self.marked_hit_bp,
                DamageKind::Additional,
                action,
            );
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            battle.units[owner].counter_state.recently_hit = false;
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        source: usize,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[owner];
        let mut remaining_bp = BP;
        if battle.enemies().contains(&source)
            && matches!(kind, DamageKind::Direct | DamageKind::FollowUp)
        {
            remaining_bp = remaining_bp * (BP - self.damage_reduction_bp) / BP;
        }
        if state.finished_turns < state.counter_state.ult_expires_after_turn {
            remaining_bp = remaining_bp * (BP - self.ultimate_damage_reduction_bp) / BP;
        }
        if state.counter_state.recently_hit {
            remaining_bp = remaining_bp * 7_000 / BP;
        }
        BP - remaining_bp
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
        _was_shielded: bool,
    ) {
        let self_hit = owner == target;
        if self_hit {
            if self.eidolon >= 4 {
                battle.units[owner].counter_state.recently_hit = true;
            }
            if self.kinship && battle.roll_fixed(3_500) {
                battle.cleanse_one(owner, owner);
            }
        }
        let enhanced = battle.units[owner].counter_state.enhanced_charges > 0;
        if !enhanced && !self_hit && (self.eidolon < 6 || !battle.roll_fixed(5_000)) {
            return;
        }
        if battle.hp(attacker).0 == 0 || battle.is_frozen(owner) {
            return;
        }
        let state = &mut battle.units[owner].counter_state;
        if !state.marked.contains(&attacker) {
            state.marked.push(attacker);
        }
        let (multiplier_bp, splash) = if enhanced {
            state.enhanced_charges -= 1;
            (
                self.enhanced_counter_bp,
                Some((self.enhanced_counter_bp / 2, COUNTER_TOUGHNESS)),
            )
        } else {
            (self.counter_bp, None)
        };
        battle.counter_attack(
            owner,
            attacker,
            CounterAttack {
                element: Element::Physical,
                multiplier_bp,
                defense_multiplier_bp: 0,
                toughness: COUNTER_TOUGHNESS,
                splash,
                energy_milli: COUNTER_ENERGY_MILLI,
            },
        );
    }

    fn control_resistance_bp(&self) -> u32 {
        if self.under_protection { 3_500 } else { 0 }
    }

    fn damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.revenge && kind == DamageKind::FollowUp {
            3_000
        } else {
            0
        }
    }
}
