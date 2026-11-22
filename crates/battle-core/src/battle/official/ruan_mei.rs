use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, resolve::DamageTargetAura, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1303";
const BASIC: &str = "skill.130301";
const SKILL: &str = "skill.130302";
const ULTIMATE: &str = "skill.130303";
const TALENT: &str = "skill.130304";
const A2: &str = "trace.1303101";
const A4: &str = "trace.1303102";
const A6: &str = "trace.1303103";
const E4: &str = "eidolon.130304";
const BREAK_EFFECT_AURA_BP: u32 = 2_000;
const A6_THRESHOLD_BP: u32 = 12_000;
const A6_BREAK_STEP_BP: u32 = 1_000;
const A6_DAMAGE_STEP_BP: u32 = 600;
const A6_CAP_BP: u32 = 3_600;

#[derive(Clone, Copy, Default)]
pub(in crate::battle) struct RuanMeiState {
    overtones_until_turn: u32,
    field_until_turn: u32,
}

struct RuanMei {
    damage_bonus_bp: u32,
    break_efficiency_bp: u32,
    skill_turns: u32,
    penetration_bp: u32,
    field_turns: u32,
    rebloom_break_effect_bp: u32,
    rebloom_base_delay_bp: u32,
    rebloom_damage_bp: u16,
    speed_bp: u32,
    break_damage_bp: u16,
    break_effect_aura: bool,
    start_energy: bool,
    break_effect_to_damage: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != Element::Ice
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let [basic] = input.parameters(BASIC, 1)? else {
        return Err(invalid(BASIC));
    };
    let [damage_bonus, break_efficiency, skill_turns] = input.parameters(SKILL, 3)? else {
        return Err(invalid(SKILL));
    };
    let [
        penetration,
        field_turns,
        rebloom_break_effect,
        rebloom_base_delay,
        rebloom_damage,
    ] = input.parameters(ULTIMATE, 5)?
    else {
        return Err(invalid(ULTIMATE));
    };
    let [speed, break_damage] = input.parameters(TALENT, 2)? else {
        return Err(invalid(TALENT));
    };
    if *break_efficiency != 0.5 || *skill_turns != 3.0 {
        return Err(invalid(SKILL));
    }
    if *field_turns != 2.0 || *rebloom_break_effect != 0.2 || *rebloom_base_delay != 0.1 {
        return Err(invalid(ULTIMATE));
    }
    let talent_damage_bp = basis_points(TALENT, *break_damage)?;
    let talent_damage_bp = talent_damage_bp
        .checked_add(20_000 * u16::from(input.eidolon >= 6))
        .ok_or_else(|| invalid(TALENT))?;
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Ice,
                multiplier_bp: basis_points(BASIC, *basic)?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(RuanMei {
            damage_bonus_bp: u32::from(basis_points(SKILL, *damage_bonus)?),
            break_efficiency_bp: u32::from(basis_points(SKILL, *break_efficiency)?),
            skill_turns: *skill_turns as u32,
            penetration_bp: u32::from(basis_points(ULTIMATE, *penetration)?),
            field_turns: *field_turns as u32 + u32::from(input.eidolon >= 6),
            rebloom_break_effect_bp: u32::from(basis_points(ULTIMATE, *rebloom_break_effect)?),
            rebloom_base_delay_bp: u32::from(basis_points(ULTIMATE, *rebloom_base_delay)?),
            rebloom_damage_bp: basis_points(ULTIMATE, *rebloom_damage)?,
            speed_bp: u32::from(basis_points(TALENT, *speed)?),
            break_damage_bp: talent_damage_bp,
            break_effect_aura: trace(A2),
            start_energy: trace(A4),
            break_effect_to_damage: trace(A6),
            eidolon: input.eidolon,
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

fn overtones_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    battle.is_targetable(owner) && unit.normal_turns < unit.ruan_mei_state.overtones_until_turn
}

fn field_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    battle.is_targetable(owner) && unit.normal_turns < unit.ruan_mei_state.field_until_turn
}

fn has_rebloom(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
    battle.units[target]
        .debuffs
        .iter()
        .any(|status| status.source == owner && status.kind == DebuffKind::ThanatoplumRebloom)
}

impl Mechanics for RuanMei {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let unit = &mut battle.units[owner];
        match action {
            Action::Skill => {
                unit.ruan_mei_state.overtones_until_turn =
                    unit.normal_turns.saturating_add(self.skill_turns);
            }
            Action::Ultimate => {
                unit.ruan_mei_state.field_until_turn =
                    unit.normal_turns.saturating_add(self.field_turns);
            }
            Action::Basic | Action::EnhancedUltimate => {}
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if self.start_energy && owner == actor && !extra {
            battle.gain_energy(owner, owner, 5_000);
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
        _element: Element,
    ) -> u32 {
        if !battle.allies().contains(&actor) || !overtones_active(battle, owner) {
            return 0;
        }
        let extra = if self.break_effect_to_damage {
            let excess = battle
                .effective_break_effect(owner)
                .saturating_sub(A6_THRESHOLD_BP);
            (excess / A6_BREAK_STEP_BP * A6_DAMAGE_STEP_BP).min(A6_CAP_BP)
        } else {
            0
        };
        self.damage_bonus_bp + extra
    }

    fn ally_toughness_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _actor: usize) -> u32 {
        self.break_efficiency_bp * u32::from(overtones_active(battle, owner))
    }

    fn ally_break_effect_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> u32 {
        BREAK_EFFECT_AURA_BP * u32::from(self.break_effect_aura)
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        self.speed_bp
            * u32::from(
                actor != owner && battle.allies().contains(&actor) && battle.is_targetable(owner),
            )
    }

    fn ally_target_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> u32 {
        4_000
            * u32::from(
                self.eidolon >= 2
                    && battle.is_targetable(owner)
                    && battle.units[enemy].toughness == 0,
            )
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        if !battle.allies().contains(&actor) || !field_active(battle, owner) {
            return DamageTargetAura::default();
        }
        DamageTargetAura {
            resistance_down_bp: self.penetration_bp,
            defense_down_bp: 2_000 * u32::from(self.eidolon >= 1),
            ..DamageTargetAura::default()
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        _action: Action,
        _primary: Option<usize>,
    ) {
        if !field_active(battle, owner) {
            return;
        }
        for target in battle.attack_targets().to_vec() {
            if !battle.units[target].rebloom_spent && !has_rebloom(battle, owner, target) {
                battle.apply_permanent_debuff(owner, target, DebuffKind::ThanatoplumRebloom, 0);
            }
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        target: usize,
        action: Action,
    ) {
        if !battle.is_targetable(owner) {
            return;
        }
        if self.eidolon >= 4 {
            battle.apply_ally_buff_with_id(
                owner,
                owner,
                AllyBuffKind::BreakEffect,
                10_000,
                3,
                Some(E4),
            );
        }
        battle.break_status_hit_scaled(
            owner,
            target,
            Element::Ice,
            action,
            DamageKind::WeaknessBreak,
            self.break_damage_bp,
        );
    }

    fn prevent_toughness_recovery(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
    ) -> bool {
        if !has_rebloom(battle, owner, target) {
            return false;
        }
        battle.remove_debuff(owner, target, DebuffKind::ThanatoplumRebloom);
        battle.units[target].rebloom_spent = true;
        let break_effect_bp = battle.effective_break_effect(owner);
        let scaled_delay =
            u64::from(break_effect_bp) * u64::from(self.rebloom_break_effect_bp) / 10_000;
        let delay =
            (u64::from(self.rebloom_base_delay_bp) + scaled_delay).min(u64::from(u16::MAX)) as u16;
        battle.delay_action(owner, target, delay);
        battle.break_status_hit_scaled(
            owner,
            target,
            Element::Ice,
            Action::Ultimate,
            DamageKind::WeaknessBreak,
            self.rebloom_damage_bp,
        );
        true
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate if !field_active(battle, owner) => Some(0),
            Action::Skill if !overtones_active(battle, owner) => Some(1),
            _ => None,
        }
    }
}
