use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1207";
const BASIC: &str = "skill.120701";
const SKILL: &str = "skill.120702";
const ULTIMATE: &str = "skill.120703";
const TALENT: &str = "skill.120704";
const ARCHERION: &str = "trace.1207101";
const BOWMASTER: &str = "trace.1207102";
const MAJESTAS: &str = "trace.1207103";
const MAX_BOWSTRINGS: u8 = 2;

#[derive(Clone, Default)]
pub(in crate::battle) struct YukongState {
    bowstrings: u8,
    empowered_bowstrings: bool,
    preserve_bowstrings_this_turn: bool,
    talent_ready_at: u32,
    debuff_resist_ready_after_turn: u32,
    energy_refunded: [bool; 4],
}

struct Yukong {
    eidolon: u8,
    attack_bp: u16,
    ultimate_crit_rate_bp: u16,
    ultimate_crit_damage_bp: u16,
    enhanced_basic: OfficialAbility,
    talent_damage_bp: u16,
    archerion: bool,
    bowmaster: bool,
    majestas: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130 {
        return Err(invalid("energy capacity"));
    }
    let basic = multiplier(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 3)?;
    if skill[0] != f64::from(MAX_BOWSTRINGS) || talent[1] != 1.0 || talent[2] != 1.0 {
        return Err(invalid(
            "bowstring cap, talent toughness ratio, or cooldown",
        ));
    }
    let has_trace = |trace| input.enabled_trace_ids.iter().any(|id| id == trace);
    let basic_ability = |toughness| OfficialAbility {
        target: Target::Enemy,
        energy_milli: 20_000,
        effects: vec![imaginary_hit(basic, toughness)],
        skill_points: SkillPoints::Generate,
    };
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: basic_ability(10),
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: vec![],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 5_000,
            effects: vec![imaginary_hit(multiplier(ULTIMATE, ultimate[0])?, 30)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Yukong {
            eidolon: input.eidolon,
            attack_bp: multiplier(SKILL, skill[1])?,
            ultimate_crit_rate_bp: multiplier(ULTIMATE, ultimate[1])?,
            ultimate_crit_damage_bp: multiplier(ULTIMATE, ultimate[2])?,
            enhanced_basic: basic_ability(20),
            talent_damage_bp: multiplier(TALENT, talent[0])?,
            archerion: has_trace(ARCHERION),
            bowmaster: has_trace(BOWMASTER),
            majestas: has_trace(MAJESTAS),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn multiplier(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(&format!("{skill} multiplier")));
    }
    Ok(bp as u16)
}

fn imaginary_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Imaginary,
        multiplier_bp,
        toughness,
    }
}

impl Yukong {
    fn talent_ready(battle: &Battle<'_>, actor: usize) -> bool {
        let unit = &battle.units[actor];
        unit.normal_turns >= unit.yukong_state.talent_ready_at
    }
}

impl Mechanics for Yukong {
    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && Self::talent_ready(battle, actor))
            .then_some(&self.enhanced_basic)
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            for ally in battle.allies() {
                battle.apply_ally_buff(actor, ally, AllyBuffKind::SpeedPercent, 1_000, 2);
            }
        }
        if self.eidolon >= 2 {
            for ally in battle.allies() {
                let energy_cap = battle.plan.units[ally].kit.max_energy_milli();
                if battle.units[ally].hp > 0
                    && energy_cap > 0
                    && battle.units[ally].energy_milli == energy_cap
                {
                    self.on_ally_energy_full(battle, actor, ally);
                }
            }
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Ultimate {
            let state = &mut battle.units[actor].yukong_state;
            if self.eidolon >= 6 {
                state.bowstrings = (state.bowstrings + 1).min(MAX_BOWSTRINGS);
            }
            state.empowered_bowstrings = state.bowstrings > 0;
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
        if action == Action::Basic && Self::talent_ready(battle, actor) {
            let unit = &mut battle.units[actor];
            unit.yukong_state.talent_ready_at = unit.normal_turns + 2;
            battle.secondary_hit(
                actor,
                target,
                Element::Imaginary,
                self.talent_damage_bp,
                DamageKind::Additional,
                action,
            );
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let state = &mut battle.units[actor].yukong_state;
        match action {
            Action::Skill => {
                state.bowstrings = MAX_BOWSTRINGS;
                state.preserve_bowstrings_this_turn = true;
            }
            Action::Ultimate | Action::EnhancedUltimate => state.energy_refunded = [false; 4],
            Action::Basic => {}
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        _action: Action,
    ) {
        if self.majestas && battle.units[owner].yukong_state.bowstrings > 0 {
            battle.gain_energy(owner, owner, 2_000);
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        let state = &mut battle.units[owner].yukong_state;
        if owner == actor && std::mem::take(&mut state.preserve_bowstrings_this_turn) {
            return;
        }
        state.bowstrings = state.bowstrings.saturating_sub(1);
        if state.bowstrings == 0 {
            state.empowered_bowstrings = false;
        }
    }

    fn on_ally_energy_full(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.eidolon >= 2
            && !std::mem::replace(
                &mut battle.units[owner].yukong_state.energy_refunded[actor],
                true,
            )
        {
            battle.gain_energy(owner, owner, 5_000);
        }
    }

    fn prevent_debuff(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        let unit = &mut battle.units[actor];
        if self.archerion && unit.finished_turns >= unit.yukong_state.debuff_resist_ready_after_turn
        {
            unit.yukong_state.debuff_resist_ready_after_turn = unit.finished_turns + 2;
            true
        } else {
            false
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        if battle.units[owner].yukong_state.bowstrings > 0 {
            u32::from(self.attack_bp)
        } else {
            0
        }
    }

    fn ally_crit_rate_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        if battle.units[owner].yukong_state.empowered_bowstrings {
            u32::from(self.ultimate_crit_rate_bp)
        } else {
            0
        }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        if battle.units[owner].yukong_state.empowered_bowstrings {
            u32::from(self.ultimate_crit_damage_bp)
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        element: Element,
    ) -> u32 {
        let personal =
            self.eidolon >= 4 && owner == target && battle.units[owner].yukong_state.bowstrings > 0;
        3_000 * u32::from(personal)
            + 1_200 * u32::from(self.bowmaster && element == Element::Imaginary)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        let has_bowstrings = battle.units[actor].yukong_state.bowstrings > 0;
        Some(match action {
            Action::Ultimate | Action::EnhancedUltimate if has_bowstrings || self.eidolon >= 6 => 0,
            Action::Skill if !has_bowstrings => 2,
            Action::Basic => 3,
            Action::Skill | Action::Ultimate | Action::EnhancedUltimate => 4,
        })
    }
}
