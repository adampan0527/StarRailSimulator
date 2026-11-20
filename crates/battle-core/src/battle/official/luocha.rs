use std::sync::Arc;

use super::{
    super::{Battle, combat_status::DebuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1203";
const BASIC: &str = "skill.120301";
const SKILL: &str = "skill.120302";
const ULTIMATE: &str = "skill.120303";
const TALENT: &str = "skill.120304";
const CLEANSING_REVIVAL: &str = "trace.1203101";
const SANCTIFIED: &str = "trace.1203102";
const THROUGH_THE_VALLEY: &str = "trace.1203103";
const BP: u32 = 10_000;
const MICROS: u64 = 1_000_000;
const SKILL_ENERGY_MILLI: u32 = 30_000;
const E2_HP_THRESHOLD_BP: u32 = 5_000;
const SHIELD: AttackAmount = AttackAmount {
    ratio_micros: 180_000,
    flat_micros: 240 * MICROS,
};
const TEAM_HEAL: AttackAmount = AttackAmount {
    ratio_micros: 70_000,
    flat_micros: 93 * MICROS,
};

#[derive(Clone, Default)]
pub(in crate::battle) struct LuochaState {
    abyss_flowers: u32,
    field_expires_after_turn: u32,
    auto_ready_after_turn: u32,
    field_attack: bool,
}

struct Luocha {
    skill_heal: AttackAmount,
    field_heal: AttackAmount,
    healing_bp: u32,
    skill_threshold_bp: u32,
    skill_cooldown_turns: u32,
    field_flowers: u32,
    field_turns: u32,
    cleanse: bool,
    team_heal: bool,
    control_resistance: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "Luocha energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 4)?;
    if !(0.0..=1.0).contains(&skill[2]) || ultimate[1] != 1.0 {
        return Err(BattleError::InvalidInput(
            "Luocha automatic skill HP threshold or ultimate dispel count".into(),
        ));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: imaginary_attack(
            BASIC,
            basic[0],
            Target::Enemy,
            10,
            20_000,
            SkillPoints::Generate,
        )?,
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: SKILL_ENERGY_MILLI,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: imaginary_attack(
            ULTIMATE,
            ultimate[0],
            Target::AllEnemies,
            20,
            5_000,
            SkillPoints::Unchanged,
        )?,
        mechanics: Arc::new(Luocha {
            skill_heal: AttackAmount::from_data(SKILL, skill[0], skill[1])?,
            field_heal: AttackAmount::from_data(TALENT, talent[1], talent[3])?,
            healing_bp: input.stats.healing_bp,
            skill_threshold_bp: (skill[2] * f64::from(BP)).round() as u32,
            skill_cooldown_turns: positive_count(SKILL, skill[3])?,
            field_flowers: positive_count(TALENT, talent[0])?,
            field_turns: positive_count(TALENT, talent[2])?,
            cleanse: trace(CLEANSING_REVIVAL),
            team_heal: trace(SANCTIFIED),
            control_resistance: trace(THROUGH_THE_VALLEY),
            eidolon: input.eidolon,
        }),
    })
}

fn imaginary_attack(
    skill: &str,
    ratio: f64,
    target: Target,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> Result<OfficialAbility, BattleError> {
    let multiplier_bp = (ratio * f64::from(BP)).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&multiplier_bp) {
        return Err(BattleError::InvalidInput(format!(
            "Luocha {skill} multiplier"
        )));
    }
    Ok(OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Imaginary,
            multiplier_bp: multiplier_bp as u16,
            toughness,
        }],
        skill_points,
    })
}

fn positive_count(skill: &str, value: f64) -> Result<u32, BattleError> {
    if !(1.0..=30.0).contains(&value) || value.fract() != 0.0 {
        return Err(BattleError::InvalidInput(format!("Luocha {skill} count")));
    }
    Ok(value as u32)
}

#[derive(Clone, Copy)]
struct AttackAmount {
    ratio_micros: u64,
    flat_micros: u64,
}

impl AttackAmount {
    fn from_data(skill: &str, ratio: f64, flat: f64) -> Result<Self, BattleError> {
        if !(0.0..=10.0).contains(&ratio) || !(0.0..=1_000_000.0).contains(&flat) {
            return Err(BattleError::InvalidInput(format!(
                "Luocha {skill} heal coefficient"
            )));
        }
        Ok(Self {
            ratio_micros: (ratio * MICROS as f64).round() as u64,
            flat_micros: (flat * MICROS as f64).round() as u64,
        })
    }

    fn amount(self, attack: u32, bonus_bp: u32) -> u32 {
        let base_micros =
            u128::from(attack) * u128::from(self.ratio_micros) + u128::from(self.flat_micros);
        let scaled = base_micros * (u128::from(BP) + u128::from(bonus_bp));
        u32::try_from(scaled / (u128::from(MICROS) * u128::from(BP))).unwrap_or(u32::MAX)
    }
}

fn field_active(battle: &Battle<'_>, owner: usize) -> bool {
    let state = &battle.units[owner];
    state.hp > 0 && state.finished_turns < state.luocha_state.field_expires_after_turn
}

impl Luocha {
    fn gain_flower(&self, battle: &mut Battle<'_>, owner: usize) {
        if field_active(battle, owner) {
            return;
        }
        let owner = &mut battle.units[owner];
        let state = &mut owner.luocha_state;
        state.abyss_flowers += 1;
        if state.abyss_flowers == self.field_flowers {
            state.abyss_flowers = 0;
            state.field_expires_after_turn = owner.normal_turns + self.field_turns;
        }
    }

    fn skill_effect(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let (hp, max_hp) = battle.hp(target);
        let lower_e2_branch =
            u64::from(hp) * u64::from(BP) < u64::from(max_hp) * u64::from(E2_HP_THRESHOLD_BP);
        let attack = battle.effective_attack(owner);
        let mut healing_bp = self.healing_bp;
        if self.eidolon >= 2 {
            if lower_e2_branch {
                healing_bp = healing_bp.saturating_add(3_000);
            } else {
                battle.apply_official_shield(
                    owner,
                    target,
                    SHIELD.amount(attack, 0),
                    2,
                    Some(Action::Skill),
                );
            }
        }
        if self.cleanse {
            battle.cleanse_one(owner, target);
        }
        battle.heal_unit(owner, target, self.skill_heal.amount(attack, healing_bp));
        self.gain_flower(battle, owner);
    }

    fn auto_ready(&self, battle: &Battle<'_>, owner: usize) -> bool {
        let state = &battle.units[owner];
        state.finished_turns >= state.luocha_state.auto_ready_after_turn && !battle.is_frozen(owner)
    }

    fn can_receive_auto_heal(&self, battle: &Battle<'_>, target: usize) -> bool {
        let (hp, max_hp) = battle.hp(target);
        hp > 0
            && u64::from(hp) * u64::from(BP)
                <= u64::from(max_hp) * u64::from(self.skill_threshold_bp)
    }

    fn auto_heal(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let state = &mut battle.units[owner];
        state.luocha_state.auto_ready_after_turn = state.normal_turns + self.skill_cooldown_turns;
        self.skill_effect(battle, owner, target);
        battle.gain_energy(owner, owner, SKILL_ENERGY_MILLI);
    }
}

impl Mechanics for Luocha {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action != Action::Ultimate {
            return;
        }
        for target in battle.enemies() {
            if battle.hp(target).0 == 0 {
                continue;
            }
            battle.dispel_one(actor, target);
            if self.eidolon >= 6 {
                battle.apply_debuff(actor, target, DebuffKind::AllResistanceDown, 2_000, 2);
            }
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.skill_effect(battle, actor, target),
            (Action::Ultimate, _) => self.gain_flower(battle, actor),
            _ => {}
        }
    }

    fn after_ally_hp_loss(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if self.auto_ready(battle, owner) && self.can_receive_auto_heal(battle, target) {
            self.auto_heal(battle, owner, target);
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if actor == owner
            && self.auto_ready(battle, owner)
            && let Some(target) = battle
                .allies()
                .find(|&target| self.can_receive_auto_heal(battle, target))
        {
            self.auto_heal(battle, owner, target);
        }
    }

    fn on_attack_target(&self, battle: &mut Battle<'_>, owner: usize, _target: usize) {
        if field_active(battle, owner) {
            battle.units[owner].luocha_state.field_attack = true;
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        _origin: Action,
    ) {
        // The attack that opened a new field has already landed.
        if !std::mem::take(&mut battle.units[owner].luocha_state.field_attack) {
            return;
        }
        let attack = battle.effective_attack(owner);
        for target in battle.allies() {
            let (hp, max_hp) = battle.hp(target);
            if hp == 0 || hp == max_hp {
                continue;
            }
            let heal = if target == attacker {
                self.field_heal
            } else if self.team_heal {
                TEAM_HEAL
            } else {
                continue;
            };
            battle.heal_unit(owner, target, heal.amount(attack, self.healing_bp));
        }
    }

    fn control_resistance_bp(&self) -> u32 {
        if self.control_resistance { 7_000 } else { 0 }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        if self.eidolon >= 1 && field_active(battle, owner) {
            2_000
        } else {
            0
        }
    }

    fn enemy_damage_reduction_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.eidolon >= 4 && field_active(battle, owner) {
            1_200
        } else {
            0
        }
    }
}
