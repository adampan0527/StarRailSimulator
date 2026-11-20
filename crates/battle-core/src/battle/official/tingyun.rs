use std::{cmp::Reverse, sync::Arc};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1202";
const BASIC: &str = "skill.120201";
const SKILL: &str = "skill.120202";
const ULTIMATE: &str = "skill.120203";
const TALENT: &str = "skill.120204";
const NOURISHED_JOVIALITY: &str = "trace.1202101";
const KNELL_SUBDUAL: &str = "trace.1202102";
const JUBILANT_PASSAGE: &str = "trace.1202103";
const BP: u32 = 10_000;
const RATIO_SCALE: u32 = 1_000_000;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity"
        )));
    }
    let basic = multiplier_bp(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 1)?;
    let has_trace = |trace| input.enabled_trace_ids.iter().any(|id| id == trace);
    let benediction_damage_bp = multiplier_bp(
        SKILL,
        skill[0] + if input.eidolon >= 4 { 0.20 } else { 0.0 },
    )?;
    let mechanics = Tingyun {
        attack_ratio_micros: attack_ratio(SKILL, skill[1])?,
        attack_cap_micros: attack_ratio(SKILL, skill[3])?,
        benediction_turns: turns(SKILL, skill[2])?,
        benediction_damage_bp,
        talent_damage_bp: multiplier_bp(TALENT, talent[0])?,
        ultimate_energy_milli: energy(ULTIMATE, ultimate[0])?
            + if input.eidolon >= 6 { 10_000 } else { 0 },
        ultimate_turns: turns(ULTIMATE, ultimate[1])?,
        ultimate_damage_bp: u32::from(multiplier_bp(ULTIMATE, ultimate[2])?),
        speed_trace: has_trace(NOURISHED_JOVIALITY),
        basic_damage_trace: has_trace(KNELL_SUBDUAL),
        energy_trace: has_trace(JUBILANT_PASSAGE),
        eidolon: input.eidolon,
    };
    // Keep the total multiplier unchanged when splitting the two displayed hits.
    let first = (u32::from(basic) * 3 / 10) as u16;
    let basic_hit = |multiplier_bp, toughness| Effect::Hit {
        element: Element::Thunder,
        multiplier_bp,
        toughness,
    };
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![basic_hit(first, 3), basic_hit(basic - first, 7)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::Ally,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn multiplier_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * f64::from(BP)).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {skill} multiplier"
        )));
    }
    Ok(value as u16)
}

fn attack_ratio(skill: &str, ratio: f64) -> Result<u32, BattleError> {
    if !(0.0..=1.0).contains(&ratio) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {skill} attack ratio"
        )));
    }
    Ok((ratio * f64::from(RATIO_SCALE)).round() as u32)
}

fn turns(skill: &str, value: f64) -> Result<u32, BattleError> {
    if value.fract() != 0.0 || !(1.0..=f64::from(u8::MAX)).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {skill} duration"
        )));
    }
    Ok(value as u32)
}

fn energy(skill: &str, value: f64) -> Result<u32, BattleError> {
    if value.fract() != 0.0 || !(1.0..=130.0).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {skill} energy restored"
        )));
    }
    Ok(value as u32 * 1_000)
}

fn scale_attack(attack: u32, ratio_micros: u32) -> u32 {
    (u64::from(attack) * u64::from(ratio_micros) / u64::from(RATIO_SCALE)) as u32
}

struct Tingyun {
    attack_ratio_micros: u32,
    attack_cap_micros: u32,
    benediction_turns: u32,
    benediction_damage_bp: u16,
    talent_damage_bp: u16,
    ultimate_energy_milli: u32,
    ultimate_turns: u32,
    ultimate_damage_bp: u32,
    speed_trace: bool,
    basic_damage_trace: bool,
    energy_trace: bool,
    eidolon: u8,
}

fn recipient(battle: &Battle<'_>, owner: usize) -> Option<usize> {
    let state = &battle.units[owner].tingyun_state;
    state.benediction_target.filter(|&target| {
        battle.hp(target).0 > 0
            && battle.units[target].finished_turns < state.benediction_expires_after_turn
    })
}

fn preferred_recipient(battle: &Battle<'_>, owner: usize) -> usize {
    battle
        .allies()
        .filter(|&unit| unit != owner && battle.hp(unit).0 > 0)
        .max_by_key(|&unit| (battle.plan.units[unit].stats.attack, Reverse(unit)))
        .unwrap_or(owner)
}

impl Mechanics for Tingyun {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let Some(target) = target else { return };
        match action {
            Action::Skill => {
                let cap = scale_attack(battle.effective_attack(actor), self.attack_cap_micros);
                let expires = battle.units[target].normal_turns + self.benediction_turns;
                let state = &mut battle.units[actor].tingyun_state;
                if state.benediction_target != Some(target) {
                    state.e2_refund_turn = None;
                }
                state.benediction_target = Some(target);
                state.benediction_expires_after_turn = expires;
                state.benediction_attack_cap = cap;
                if self.speed_trace {
                    battle.apply_speed_up(actor, actor, 2_000, 1);
                }
            }
            Action::Ultimate => {
                battle.restore_energy_flat(actor, target, self.ultimate_energy_milli);
                battle.apply_ally_buff(
                    actor,
                    target,
                    AllyBuffKind::Damage,
                    self.ultimate_damage_bp,
                    self.ultimate_turns,
                );
            }
            Action::Basic => {}
        }
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if recipient(battle, owner) != Some(target) {
            return 0;
        }
        scale_attack(
            battle.plan.units[target].stats.base_attack,
            self.attack_ratio_micros,
        )
        .min(battle.units[owner].tingyun_state.benediction_attack_cap)
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
    ) {
        let Some(target) = recipient(battle, owner) else {
            return;
        };
        if actor != owner && actor != target {
            return;
        }
        let mut enemies = Vec::new();
        for &enemy in battle.attack_targets() {
            if !enemies.contains(&enemy) {
                enemies.push(enemy);
            }
        }
        let Some(&first) = enemies.first() else {
            return;
        };
        if actor == owner {
            battle.secondary_hit_with_credit(
                target,
                first,
                Element::Thunder,
                self.talent_damage_bp,
                origin,
                owner,
            );
        }
        if actor == target {
            let selected = battle.random_index(enemies.len());
            battle.secondary_hit_with_credit(
                target,
                enemies[selected],
                Element::Thunder,
                self.benediction_damage_bp,
                origin,
                owner,
            );
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if self.eidolon >= 1
            && action == Action::Ultimate
            && recipient(battle, owner) == Some(actor)
        {
            battle.apply_ally_buff(owner, actor, AllyBuffKind::SpeedPercent, 2_000, 1);
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        killer: usize,
        _target: usize,
    ) {
        if self.eidolon < 2 || recipient(battle, owner) != Some(killer) {
            return;
        }
        let turn = battle.units[killer].normal_turns;
        let state = &mut battle.units[owner].tingyun_state;
        if state.e2_refund_turn != Some(turn) {
            state.e2_refund_turn = Some(turn);
            battle.gain_energy(owner, killer, 5_000);
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.energy_trace && owner == actor {
            battle.gain_energy(owner, owner, 5_000);
        }
    }

    fn damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.basic_damage_trace && action == Some(Action::Basic) && kind == DamageKind::Direct {
            4_000
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        let target = target?;
        let current = recipient(battle, actor);
        let preferred = preferred_recipient(battle, actor);
        match action {
            Action::Skill => Some(if current.is_none() && target == preferred {
                2
            } else {
                4
            }),
            Action::Ultimate => {
                let target_has_room = target == actor
                    || battle.units[target].energy_milli
                        < battle.plan.units[target].kit.max_energy_milli();
                Some(
                    if target == current.unwrap_or(preferred) && target_has_room {
                        0
                    } else {
                        4
                    },
                )
            }
            Action::Basic => None,
        }
    }
}
