use std::{ops::RangeInclusive, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1302";
const BASIC: &str = "skill.130201";
const SKILL: &str = "skill.130202";
const ULTIMATE: &str = "skill.130203";
const TALENT: &str = "skill.130204";
const ENHANCED_ULTIMATE: &str = "skill.130214";
const PIETY: &str = "trace.1302101";
const GENEROSITY: &str = "trace.1302102";
const COURAGE: &str = "trace.1302103";
const E2: &str = "eidolon.130202";

const ULTIMATE_COST: u32 = 90;
const ENERGY_CAP: u32 = 180;
const ENHANCED_BOUNCES: u8 = 6;
const APOTHEOSIS_CAP: u8 = 10;
const TALENT_ENERGY: u32 = 3;

struct Argenti {
    enhanced_ultimate: OfficialAbility,
    apotheosis_crit_bp: u32,
    piety: bool,
    generosity: bool,
    courage: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != Element::Physical
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 1)?[0];
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let enhanced = input.parameters(ENHANCED_ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 3)?;
    if ultimate[1] != f64::from(ULTIMATE_COST) {
        return Err(invalid(ULTIMATE));
    }
    if enhanced[1] != f64::from(ENHANCED_BOUNCES) || enhanced[3] != f64::from(ENERGY_CAP) {
        return Err(invalid(ENHANCED_ULTIMATE));
    }
    if talent[0] != f64::from(TALENT_ENERGY) || talent[2] != f64::from(APOTHEOSIS_CAP) {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic)?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 30_000,
            effects: vec![hit(basis_points(SKILL, skill)?, 10)],
            skill_points: SkillPoints::Spend,
        },
        ultimate: ultimate_with(vec![hit(basis_points(ULTIMATE, ultimate[0])?, 20)]),
        mechanics: Arc::new(Argenti {
            enhanced_ultimate: ultimate_with(vec![
                hit(basis_points(ENHANCED_ULTIMATE, enhanced[0])?, 20),
                Effect::RandomHits {
                    element: Element::Physical,
                    multiplier_bp: basis_points(ENHANCED_ULTIMATE, enhanced[2])?,
                    toughness: 5,
                    count: ENHANCED_BOUNCES,
                },
            ]),
            apotheosis_crit_bp: u32::from(basis_points(TALENT, talent[1])?),
            piety: trace(PIETY),
            generosity: trace(GENEROSITY),
            courage: trace(COURAGE),
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
        element: Element::Physical,
        multiplier_bp,
        toughness,
    }
}

fn ultimate_with(effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target: Target::AllEnemies,
        energy_milli: 5_000,
        effects,
        skill_points: SkillPoints::Unchanged,
    }
}

impl Argenti {
    fn add_apotheosis(&self, battle: &mut Battle<'_>, actor: usize, stacks: u8) {
        let cap = APOTHEOSIS_CAP + if self.eidolon >= 4 { 2 } else { 0 };
        let before = battle.units[actor].argenti_apotheosis;
        let after = (before + stacks).min(cap);
        if before != after {
            battle.units[actor].argenti_apotheosis = after;
            battle.emit(
                EventKind::Apotheosis { before, after },
                Some(actor),
                Some(actor),
            );
        }
    }
}

impl Mechanics for Argenti {
    fn ultimate_energy_range(&self, action: Action) -> Option<RangeInclusive<u32>> {
        let max = ENERGY_CAP * 1_000;
        match action {
            Action::AssistSkill | Action::ElationSkill => None,
            Action::Ultimate => Some(ULTIMATE_COST * 1_000..=max - 1),
            Action::EnhancedUltimate => Some(max..=max),
            Action::Basic | Action::Skill | Action::EnhancedSkill => None,
        }
    }

    fn ability_override(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::EnhancedUltimate).then_some(&self.enhanced_ultimate)
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        // Save for the stronger Ultimate in the fixed, persistent three-target encounter.
        (action == Action::Ultimate).then_some(5)
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.generosity {
            let enemies = battle
                .enemies()
                .filter(|&enemy| battle.is_targetable(enemy))
                .count();
            battle.gain_energy(owner, owner, enemies as u32 * 2_000);
        }
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 4 {
            self.add_apotheosis(battle, actor, 2);
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor && self.piety {
            self.add_apotheosis(battle, actor, 1);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if self.eidolon >= 2
            && action.is_ultimate()
            && battle
                .enemies()
                .filter(|&enemy| battle.is_targetable(enemy))
                .take(3)
                .count()
                == 3
        {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::AttackPercent,
                4_000,
                1,
                Some(E2),
            );
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _: Action,
        _: u32,
    ) {
        if battle.enemies().contains(&target) {
            self.add_apotheosis(battle, actor, 1);
            battle.gain_energy(actor, actor, TALENT_ENERGY * 1_000);
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        self.apotheosis_crit_bp * u32::from(battle.units[actor].argenti_apotheosis)
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if self.eidolon >= 1 {
            400 * u32::from(battle.units[actor].argenti_apotheosis)
        } else {
            0
        }
    }

    fn ally_target_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        attacker: usize,
        enemy: usize,
        kind: DamageKind,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(enemy);
        if self.courage && owner == attacker && kind == DamageKind::Direct && hp <= max_hp / 2 {
            1_500
        } else {
            0
        }
    }

    fn ultimate_defense_ignore_bp(&self) -> u32 {
        if self.eidolon == 6 { 3_000 } else { 0 }
    }
}
