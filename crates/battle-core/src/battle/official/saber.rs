use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Ability, Effect, Element, Target},
};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
pub(super) mod tests;

pub(super) const FORM: &str = "character.1014";
const BASIC: &str = "skill.101401";
const SKILL: &str = "skill.101402";
const ULTIMATE: &str = "skill.101403";
const TALENT: &str = "skill.101404";
const ENHANCED_BASIC: &str = "skill.101408";
const KNIGHT_OF_THE_DRAGON: &str = "trace.1014101";
const BLESSING_OF_THE_LAKE: &str = "trace.1014102";
const CROWN_OF_THE_STAR: &str = "trace.1014103";

const ENERGY_CAP: u32 = 360;
const ENERGY_CAP_MILLI: u32 = ENERGY_CAP * 1_000;
const SKILL_ENERGY_MILLI: u32 = 30_000;
const ENERGY_PER_RESONANCE_MILLI: u32 = 8_000;
const RESONANCE_CAP: u8 = 99;
const ULTIMATE_RESONANCE: u8 = 3;
const SKILL_RESONANCE: u8 = 3;
const ENHANCED_BASIC_RESONANCE: u8 = 2;
const BOUNCES: u8 = 10;
const WIND: Element = Element::Wind;

#[derive(Clone, Default)]
pub(in crate::battle) struct SaberState {
    resonance: u8,
    lifetime_resonance: u8,
    skill_payment: u8,
    enhanced_basic_ready: bool,
    mana_burst: bool,
    stored_energy_milli: u32,
    wind_penetration_stacks: u8,
    ultimates_until_refund: u8,
    gilgamesh_ultimate_pending_bp: u16,
    gilgamesh_ultimate_active_bp: u16,
}

impl SaberState {
    pub(super) fn arm_gilgamesh_ultimate(&mut self, factor_bp: u16) {
        self.gilgamesh_ultimate_pending_bp = factor_bp;
    }

    pub(super) fn gilgamesh_ultimate_factor_bp(&self) -> u32 {
        u32::from(self.gilgamesh_ultimate_active_bp).max(10_000)
    }
}

struct Saber {
    enhanced_basics: [OfficialAbility; 3],
    skill_bp_per_resonance: u32,
    talent_damage_bp: u32,
    knight_of_the_dragon: bool,
    blessing_of_the_lake: bool,
    crown_of_the_star: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != WIND
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 8)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 4)?;
    if skill[2] != f64::from(SKILL_RESONANCE) {
        return Err(invalid(SKILL));
    }
    if ultimate[2] != f64::from(BOUNCES) {
        return Err(invalid(ULTIMATE));
    }
    if talent[0] != f64::from(ULTIMATE_RESONANCE)
        || talent[3] != 2.0
        || talent[4] != f64::from(ENERGY_PER_RESONANCE_MILLI / 1_000)
        || talent[6] != f64::from(RESONANCE_CAP)
        || talent[7] != 1.0
    {
        return Err(invalid(TALENT));
    }
    if enhanced[1] != f64::from(ENHANCED_BASIC_RESONANCE) {
        return Err(invalid(ENHANCED_BASIC));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    let all_targets_bp = basis_points(ENHANCED_BASIC, enhanced[0])?;
    let enhanced_basics = [
        enhanced_basic(
            all_targets_bp,
            Some(basis_points(ENHANCED_BASIC, enhanced[3])?),
        ),
        enhanced_basic(
            all_targets_bp,
            Some(basis_points(ENHANCED_BASIC, enhanced[2])?),
        ),
        enhanced_basic(all_targets_bp, None),
    ];
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP_MILLI,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic)?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: SKILL_ENERGY_MILLI,
            effects: vec![
                hit(basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: WIND,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![
                hit(basis_points(ULTIMATE, ultimate[0])?, 40),
                Effect::RandomHits {
                    element: WIND,
                    multiplier_bp: basis_points(ULTIMATE, ultimate[1])?,
                    toughness: 2,
                    count: BOUNCES,
                },
            ],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Saber {
            enhanced_basics,
            skill_bp_per_resonance: u32::from(basis_points(SKILL, skill[3])?),
            talent_damage_bp: u32::from(basis_points(TALENT, talent[2])?),
            knight_of_the_dragon: trace(KNIGHT_OF_THE_DRAGON),
            blessing_of_the_lake: trace(BLESSING_OF_THE_LAKE),
            crown_of_the_star: trace(CROWN_OF_THE_STAR),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(skill));
    }
    Ok(rounded as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: WIND,
        multiplier_bp,
        toughness,
    }
}

fn enhanced_basic(multiplier_bp: u16, extra_bp: Option<u16>) -> OfficialAbility {
    let mut effects = vec![hit(multiplier_bp, 20)];
    if let Some(extra_bp) = extra_bp {
        effects.push(hit(extra_bp, 0));
    }
    OfficialAbility {
        target: Target::AllEnemies,
        energy_milli: 30_000,
        effects,
        skill_points: SkillPoints::Generate,
    }
}

fn update_resonance(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let state = &mut battle.units[owner].saber_state;
    let before = std::mem::replace(&mut state.resonance, after);
    state.lifetime_resonance = (state.lifetime_resonance + after.saturating_sub(before)).min(15);
    if before != after {
        battle.emit(
            EventKind::SaberCoreResonance { before, after },
            Some(owner),
            Some(owner),
        );
    }
}

fn add_resonance(battle: &mut Battle<'_>, owner: usize, gained: u8) {
    let after = battle.units[owner]
        .saber_state
        .resonance
        .saturating_add(gained)
        .min(RESONANCE_CAP);
    update_resonance(battle, owner, after);
}

fn skill_can_fill_energy(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    let resonance = unit.saber_state.resonance;
    resonance > 0
        && unit
            .energy_milli
            .saturating_add(battle.adjusted_energy_gain(owner, SKILL_ENERGY_MILLI))
            .saturating_add(u32::from(resonance) * ENERGY_PER_RESONANCE_MILLI)
            >= ENERGY_CAP_MILLI
}

fn buff_turns(battle: &Battle<'_>, owner: usize) -> u32 {
    let unit = &battle.units[owner];
    // Count the current turn when it has already opened and still has a normal action.
    2 - u32::from(unit.normal_turns > unit.finished_turns)
}

impl Saber {
    fn try_mana_burst(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &battle.units[owner].saber_state;
        if state.mana_burst && !state.enhanced_basic_ready && skill_can_fill_energy(battle, owner) {
            battle.units[owner].saber_state.mana_burst = false;
            battle.restore_skill_points(owner, 1);
            battle.advance_ally_immediate(owner, owner);
        }
    }

    fn finish_ultimate(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner].saber_state;
        state.enhanced_basic_ready = true;
        if self.eidolon >= 4 {
            state.wind_penetration_stacks = (state.wind_penetration_stacks + 1).min(3);
        }
        let stored = std::mem::take(&mut state.stored_energy_milli);
        if stored > 0 {
            battle.restore_energy_flat(owner, owner, stored);
        }
        if self.eidolon >= 6 {
            let state = &mut battle.units[owner].saber_state;
            if state.ultimates_until_refund == 0 {
                state.ultimates_until_refund = 2;
                battle.restore_energy_flat(owner, owner, 300_000);
            } else {
                state.ultimates_until_refund -= 1;
            }
        }
    }
}

impl Mechanics for Saber {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.blessing_of_the_lake && battle.units[owner].energy_milli < 216_000 {
            battle.set_energy(owner, owner, 216_000);
        }
        add_resonance(battle, owner, 1);
        battle.units[owner].saber_state.mana_burst = self.knight_of_the_dragon;
        self.try_mana_burst(battle, owner);
    }

    fn on_energy_overflow(&self, battle: &mut Battle<'_>, owner: usize, amount_milli: u32) {
        if self.blessing_of_the_lake {
            let cap = if self.eidolon >= 6 { 200_000 } else { 120_000 };
            let stored = &mut battle.units[owner].saber_state.stored_energy_milli;
            *stored = stored.saturating_add(amount_milli).min(cap);
        }
    }

    fn can_use_skill(&self, battle: &Battle<'_>, owner: usize) -> bool {
        !battle.units[owner].saber_state.enhanced_basic_ready
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if action != Action::Basic || !battle.units[owner].saber_state.enhanced_basic_ready {
            return None;
        }
        let enemies = battle
            .enemies()
            .filter(|&enemy| battle.is_targetable(enemy))
            .take(3)
            .count();
        Some(&self.enhanced_basics[enemies.saturating_sub(1)])
    }

    fn before_ally_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<usize>,
        _: Ability<'_>,
    ) {
        if battle.action.is_some_and(Action::is_ultimate) && battle.is_targetable(owner) {
            add_resonance(battle, owner, ULTIMATE_RESONANCE);
            battle.apply_ally_buff_with_id(
                owner,
                owner,
                AllyBuffKind::Damage,
                self.talent_damage_bp,
                buff_turns(battle, owner),
                Some(TALENT),
            );
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Ultimate => {
                let state = &mut battle.units[owner].saber_state;
                state.gilgamesh_ultimate_active_bp =
                    std::mem::take(&mut state.gilgamesh_ultimate_pending_bp);
            }
            Action::Skill => {
                if skill_can_fill_energy(battle, owner) {
                    let state = &mut battle.units[owner].saber_state;
                    state.skill_payment = state.resonance;
                } else {
                    add_resonance(battle, owner, SKILL_RESONANCE);
                }
                if self.crown_of_the_star {
                    battle.apply_ally_buff_with_id(
                        owner,
                        owner,
                        AllyBuffKind::CritDamage,
                        5_000,
                        buff_turns(battle, owner),
                        Some(CROWN_OF_THE_STAR),
                    );
                }
            }
            Action::Basic if battle.units[owner].saber_state.enhanced_basic_ready => {
                add_resonance(battle, owner, ENHANCED_BASIC_RESONANCE);
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Ultimate => {
                battle.units[owner].saber_state.gilgamesh_ultimate_active_bp = 0;
                self.finish_ultimate(battle, owner);
            }
            Action::Skill => {
                let payment = std::mem::take(&mut battle.units[owner].saber_state.skill_payment);
                if payment > 0 {
                    update_resonance(battle, owner, 0);
                    battle.restore_energy_flat(
                        owner,
                        owner,
                        u32::from(payment) * ENERGY_PER_RESONANCE_MILLI,
                    );
                }
            }
            Action::Basic => {
                let state = &mut battle.units[owner].saber_state;
                if std::mem::take(&mut state.enhanced_basic_ready) {
                    state.mana_burst = self.knight_of_the_dragon;
                }
            }
            _ => {}
        }
        if self.eidolon >= 1 && matches!(action, Action::Basic | Action::Skill) {
            add_resonance(battle, owner, 1);
        }
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        self.try_mana_burst(battle, owner);
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if action == Some(Action::Skill) && kind == DamageKind::Direct {
            let per_stack = self.skill_bp_per_resonance + if self.eidolon >= 2 { 700 } else { 0 };
            return base + u32::from(battle.units[owner].saber_state.skill_payment) * per_stack;
        }
        base
    }

    fn damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.eidolon >= 1 && action == Some(Action::Ultimate) && kind == DamageKind::Direct {
            6_000
        } else {
            0
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if self.knight_of_the_dragon { 2_000 } else { 0 }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if self.crown_of_the_star {
            400 * u32::from(battle.units[owner].saber_state.lifetime_resonance.min(8))
        } else {
            0
        }
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if self.eidolon >= 2 {
            100 * u32::from(battle.units[owner].saber_state.lifetime_resonance)
        } else {
            0
        }
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        if self.eidolon >= 4 && element == WIND {
            800 + 400 * u32::from(battle.units[owner].saber_state.wind_penetration_stacks)
        } else {
            0
        }
    }

    fn ultimate_resistance_penetration_bp(&self) -> u32 {
        if self.eidolon >= 6 { 2_000 } else { 0 }
    }
}
