use std::sync::Arc;

use super::{
    super::{Battle, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::{Healing, MICROS},
};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1208";
const BASIC: &str = "skill.120801";
const SKILL: &str = "skill.120802";
const ULTIMATE: &str = "skill.120803";
const TALENT: &str = "skill.120804";
const A2: &str = "trace.1208101";
const A4: &str = "trace.1208102";
const A6: &str = "trace.1208103";
const BP: u32 = 10_000;
const REFRESH_ENERGY_MILLI: u32 = 20_000;
const E1_CRIT_DAMAGE_BP: u32 = 3_000;
const E2_RESCUE_BP: u32 = 7_000;
const E4_ENERGY_MILLI: u32 = 5_000;
const E6_LOSS_CAP_BP: u64 = 12_000;
const E6_BASE_DAMAGE_PER_HP_BP: u64 = 20_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct FuXuanState {
    matrix: Option<Matrix>,
    restore_charges: u8,
    blocking_action: bool,
    rescue_used: bool,
    rescuing_action: bool,
    hp_loss_tally: u64,
}

#[derive(Clone, Copy)]
struct Matrix {
    expires_after_turn: u32,
    bonus_max_hp: u32,
    control_charge: bool,
}

struct FuXuan {
    eidolon: u8,
    matrix_turns: u32,
    distribution_bp: u16,
    matrix_hp: Healing,
    crit_rate_bp: u32,
    reduction_bp: u32,
    restore_threshold_bp: u32,
    self_heal: Healing,
    healing_bonus_micros: u64,
    refresh_energy: bool,
    ultimate_heal: Option<Healing>,
    block_control: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 135 {
        return Err(BattleError::InvalidInput("Fu Xuan energy capacity".into()));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 1)?[0];
    let talent = input.parameters(TALENT, 3)?;
    let has_trace = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    if !(1.0..=10.0).contains(&skill[2]) || skill[2].fract() != 0.0 {
        return Err(BattleError::InvalidInput("Fu Xuan Matrix duration".into()));
    }
    let ultimate_heal = has_trace(A4)
        .then(|| Healing::from_data("Fu Xuan", A4, 0.05, 133.0))
        .transpose()?;
    let mechanics = FuXuan {
        eidolon: input.eidolon,
        matrix_turns: skill[2] as u32,
        distribution_bp: coefficient_bp(SKILL, skill[0], 1.0)?,
        matrix_hp: Healing::from_data("Fu Xuan", SKILL, skill[3], 0.0)?,
        crit_rate_bp: u32::from(coefficient_bp(SKILL, skill[4], 1.0)?),
        reduction_bp: u32::from(coefficient_bp(TALENT, talent[0], 1.0)?),
        restore_threshold_bp: u32::from(coefficient_bp(TALENT, talent[1], 1.0)?),
        self_heal: Healing::from_data("Fu Xuan", TALENT, talent[2], 0.0)?,
        healing_bonus_micros: u64::from(input.stats.healing_bp) * MICROS / u64::from(BP),
        refresh_energy: has_trace(A2),
        ultimate_heal,
        block_control: has_trace(A6),
    };
    let hit = |skill, value, toughness| -> Result<Effect, BattleError> {
        Ok(Effect::MaxHpHit {
            element: Element::Quantum,
            multiplier_bp: coefficient_bp(skill, value, 6.0)?,
            toughness,
        })
    };
    Ok(OfficialKit {
        max_energy_milli: 135_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(BASIC, basic, 10)?],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(ULTIMATE, ultimate, 20)?],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn coefficient_bp(id: &str, value: f64, limit: f64) -> Result<u16, BattleError> {
    if !(0.0..=limit).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "Fu Xuan {id} coefficient"
        )));
    }
    Ok((value * f64::from(BP)).round() as u16)
}

impl FuXuan {
    fn activate_matrix(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &battle.units[actor];
        let before = state
            .fu_xuan_state
            .matrix
            .map_or(0, |matrix| matrix.bonus_max_hp);
        if self.refresh_energy && state.fu_xuan_state.matrix.is_some() {
            battle.gain_energy(actor, actor, REFRESH_ENERGY_MILLI);
        }
        let old_max_hp: Vec<_> = battle
            .allies()
            .map(|ally| battle.effective_max_hp(ally))
            .collect();
        // The field's previous HP bonus does not feed back into a recast.
        let caster_hp = old_max_hp[actor].saturating_sub(before);
        let bonus_max_hp = self.matrix_hp.amount(caster_hp, 0);
        let state = &mut battle.units[actor];
        state.fu_xuan_state.matrix = Some(Matrix {
            expires_after_turn: state.normal_turns + self.matrix_turns,
            bonus_max_hp,
            control_charge: self.block_control,
        });

        for (ally, old_max_hp) in battle.allies().zip(old_max_hp) {
            let max_hp = battle.effective_max_hp(ally);
            let hp = &mut battle.units[ally].hp;
            if *hp > 0 {
                *hp = hp
                    .saturating_add(max_hp.saturating_sub(old_max_hp))
                    .min(max_hp);
            }
        }
    }

    fn expire_matrix(battle: &mut Battle<'_>, owner: usize) {
        if battle.units[owner].fu_xuan_state.matrix.take().is_some() {
            for ally in battle.allies() {
                battle.units[ally].hp = battle.units[ally].hp.min(battle.effective_max_hp(ally));
            }
        }
    }

    fn restore_if_needed(&self, battle: &mut Battle<'_>, actor: usize) {
        let (hp, max_hp) = battle.hp(actor);
        if hp == 0
            || hp > percentage_of(max_hp, self.restore_threshold_bp)
            || battle.units[actor].fu_xuan_state.restore_charges == 0
            || battle.is_crowd_controlled(actor)
        {
            return;
        }
        battle.units[actor].fu_xuan_state.restore_charges -= 1;
        let amount = self
            .self_heal
            .amount(max_hp - hp, self.healing_bonus_micros);
        battle.heal_unit(actor, actor, amount);
    }

    fn finish_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].fu_xuan_state;
        state.hp_loss_tally = 0;
        state.restore_charges = (state.restore_charges + 1).min(2);
        if let Some(heal) = self.ultimate_heal {
            let amount = heal.amount(battle.effective_max_hp(actor), self.healing_bonus_micros);
            for ally in battle.allies().filter(|&ally| ally != actor) {
                if battle.is_targetable(ally) {
                    battle.heal_unit(actor, ally, amount);
                }
            }
        }
        self.restore_if_needed(battle, actor);
    }

    fn hp_loss_cap(battle: &Battle<'_>, actor: usize) -> u64 {
        u64::from(battle.effective_max_hp(actor)) * E6_LOSS_CAP_BP / u64::from(BP)
    }
}

impl Mechanics for FuXuan {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].fu_xuan_state.restore_charges = 1;
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Basic => {}
            Action::Skill => self.activate_matrix(battle, actor),
            Action::Ultimate | Action::EnhancedUltimate => self.finish_ultimate(battle, actor),
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner != actor {
            return;
        }
        let state = &battle.units[actor];
        if state
            .fu_xuan_state
            .matrix
            .is_some_and(|matrix| matrix.expires_after_turn <= state.normal_turns)
        {
            Self::expire_matrix(battle, actor);
        }
        self.restore_if_needed(battle, actor);
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::expire_matrix(battle, actor);
    }

    fn after_enemy_action(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner].fu_xuan_state;
        state.blocking_action = false;
        state.rescuing_action = false;
        self.restore_if_needed(battle, owner);
    }

    fn after_hp_loss(&self, battle: &mut Battle<'_>, actor: usize, _: u32) {
        // Wait for all shares of one enemy action before checking for a killing blow.
        if battle.enemy_action.is_none() {
            self.restore_if_needed(battle, actor);
        }
    }

    fn record_team_hp_loss(&self, battle: &mut Battle<'_>, owner: usize, hp_lost: u32) {
        if self.eidolon >= 6 && battle.units[owner].fu_xuan_state.matrix.is_some() {
            let cap = Self::hp_loss_cap(battle, owner);
            let state = &mut battle.units[owner].fu_xuan_state;
            state.hp_loss_tally = (state.hp_loss_tally + u64::from(hp_lost)).min(cap);
        }
    }

    fn block_enemy_control(&self, battle: &mut Battle<'_>, owner: usize, attacker: usize) -> bool {
        let action = battle.enemy_action == Some(attacker);
        let state = &mut battle.units[owner].fu_xuan_state;
        if action && state.blocking_action {
            return true;
        }
        if let Some(matrix) = &mut state.matrix
            && std::mem::take(&mut matrix.control_charge)
        {
            state.blocking_action = action;
            return true;
        }
        false
    }

    fn prevent_team_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
    ) -> bool {
        let action = battle.enemy_action == Some(attacker);
        let state = &mut battle.units[owner].fu_xuan_state;
        if self.eidolon < 2
            || state.matrix.is_none()
            || (state.rescue_used && !(action && state.rescuing_action))
        {
            return false;
        }
        state.rescue_used = true;
        state.rescuing_action = action;
        let amount = percentage_of(battle.effective_max_hp(target), E2_RESCUE_BP);
        battle.heal_unit(owner, target, amount.max(1));
        true
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        target: usize,
        _: bool,
    ) {
        if self.eidolon >= 4
            && owner != target
            && battle.units[owner].fu_xuan_state.matrix.is_some()
        {
            battle.gain_energy(owner, owner, E4_ENERGY_MILLI);
        }
    }

    fn ally_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        battle.units[owner]
            .fu_xuan_state
            .matrix
            .map_or(0, |matrix| matrix.bonus_max_hp)
    }

    fn ally_crit_rate_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if battle.units[owner].fu_xuan_state.matrix.is_some() {
            self.crit_rate_bp
        } else {
            0
        }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if self.eidolon >= 1 && battle.units[owner].fu_xuan_state.matrix.is_some() {
            E1_CRIT_DAMAGE_BP
        } else {
            0
        }
    }

    fn ally_damage_reduction_bp(&self, _: &Battle<'_>, _: usize) -> u32 {
        self.reduction_bp
    }

    fn damage_distribution_bp(&self, battle: &Battle<'_>, owner: usize) -> u16 {
        if battle.units[owner].fu_xuan_state.matrix.is_some() {
            self.distribution_bp
        } else {
            0
        }
    }

    fn flat_damage_base_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u64 {
        if self.eidolon < 6 || action != Some(Action::Ultimate) || kind != DamageKind::Direct {
            return 0;
        }
        battle.units[actor]
            .fu_xuan_state
            .hp_loss_tally
            .min(Self::hp_loss_cap(battle, actor))
            * E6_BASE_DAMAGE_PER_HP_BP
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        let state = &battle.units[actor];
        match action {
            Action::Skill
                if state
                    .fu_xuan_state
                    .matrix
                    .is_none_or(|matrix| matrix.expires_after_turn <= state.normal_turns) =>
            {
                Some(2)
            }
            _ => None,
        }
    }
}
