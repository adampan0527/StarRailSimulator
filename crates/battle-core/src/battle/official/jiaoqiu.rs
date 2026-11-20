use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        periodic::{DotScale, SkillDot},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1218";
const BASIC: &str = "skill.121801";
const SKILL: &str = "skill.121802";
const ULTIMATE: &str = "skill.121803";
const TALENT: &str = "skill.121804";
const PYRE_CLEANSE: &str = "trace.1218101";
const HEARTH_KINDLE: &str = "trace.1218102";
const SEARED_SCENT: &str = "trace.1218103";
const ROAST_TURNS: u32 = 2;
const ZONE_TURNS: u32 = 3;
const ZONE_LIMIT: u8 = 6;

#[derive(Clone, Default)]
pub(in crate::battle) struct Field {
    expires_at_turn: u32,
    triggers: u8,
    peak_stacks: u8,
}

struct Jiaoqiu {
    eidolon: u8,
    pyre_cleanse: bool,
    hearth_kindle: bool,
    seared_scent: bool,
    skill_chance_bp: u32,
    zone_chance_bp: u32,
    ultimate_vulnerability_bp: u32,
    talent_chance_bp: u32,
    stack_limit: u8,
    first_stack_bp: u32,
    extra_stack_bp: u32,
    dot_multiplier_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity or eidolon"
        )));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 6)?;
    if ultimate[3] != f64::from(ZONE_TURNS) || ultimate[4] != f64::from(ZONE_LIMIT) {
        return Err(invalid(ULTIMATE));
    }
    if talent[3] != 5.0 || talent[4] != f64::from(ROAST_TURNS) {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let eidolon = input.eidolon;
    let dot_multiplier = talent[5] + if eidolon >= 2 { 3.0 } else { 0.0 };
    let mechanics = Arc::new(Jiaoqiu {
        eidolon,
        pyre_cleanse: trace(PYRE_CLEANSE),
        hearth_kindle: trace(HEARTH_KINDLE),
        seared_scent: trace(SEARED_SCENT),
        skill_chance_bp: chance(SKILL, skill[2])?,
        zone_chance_bp: chance(ULTIMATE, ultimate[1])?,
        ultimate_vulnerability_bp: u32::from(points(ULTIMATE, ultimate[2])?),
        talent_chance_bp: chance(TALENT, talent[0])?,
        stack_limit: if eidolon == 6 { 9 } else { 5 },
        first_stack_bp: u32::from(points(TALENT, talent[1])?),
        extra_stack_bp: u32::from(points(TALENT, talent[2])?),
        dot_multiplier_bp: points(TALENT, dot_multiplier)?,
    });
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![fire_hit(points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                fire_hit(points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Fire,
                    multiplier_bp: points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![fire_hit(points(ULTIMATE, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics,
    })
}

fn fire_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Fire,
        multiplier_bp,
        toughness,
    }
}

fn invalid(skill: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: out-of-range {skill} parameters"))
}

fn points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let points = (ratio * 10_000.0).round();
    if (1.0..=f64::from(u16::MAX)).contains(&points) {
        Ok(points as u16)
    } else {
        Err(invalid(skill))
    }
}

fn chance(skill: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio <= 1.0 {
        points(skill, ratio).map(u32::from)
    } else {
        Err(invalid(skill))
    }
}

fn field_active(battle: &Battle<'_>, owner: usize) -> bool {
    let state = &battle.units[owner];
    state.hp > 0 && state.normal_turns < state.jiaoqiu_field.expires_at_turn
}

impl Jiaoqiu {
    fn stacks(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u8 {
        // The source-specific vulnerability carries the stack count and its target-turn expiry.
        let bp = battle.vulnerability_from(owner, target);
        if bp == 0 {
            0
        } else {
            let extra = bp.saturating_sub(self.first_stack_bp) / self.extra_stack_bp;
            (1 + extra).min(u32::from(self.stack_limit)) as u8
        }
    }

    fn set_stacks(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        stacks: u8,
        origin: Action,
    ) {
        let stacks = stacks.min(self.stack_limit);
        if stacks == 0
            || !battle.inflict_skill_dot(
                owner,
                target,
                SkillDot {
                    element: Element::Fire,
                    damage: DotScale::Attack(self.dot_multiplier_bp),
                    origin,
                    turns: ROAST_TURNS,
                    max_stacks: 1,
                },
            )
        {
            return;
        }
        // Roast stacks increase vulnerability and E6 RES reduction. Burn always ticks once.
        let vulnerability_bp = self.first_stack_bp + u32::from(stacks - 1) * self.extra_stack_bp;
        battle.apply_vulnerability(owner, target, vulnerability_bp, ROAST_TURNS);
        if field_active(battle, owner) {
            let field = &mut battle.units[owner].jiaoqiu_field;
            field.peak_stacks = field.peak_stacks.max(stacks);
        }
    }

    fn add_stacks(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        amount: u8,
        origin: Action,
    ) {
        let stacks = self.stacks(battle, owner, target) + amount;
        self.set_stacks(battle, owner, target, stacks, origin);
    }
}

impl Mechanics for Jiaoqiu {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.pyre_cleanse {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

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
        let peak = battle
            .enemies()
            .filter(|&enemy| battle.hp(enemy).0 > 0)
            .map(|enemy| self.stacks(battle, actor, enemy))
            .max()
            .unwrap_or(0);
        for enemy in battle.enemies() {
            if battle.hp(enemy).0 > 0 && self.stacks(battle, actor, enemy) < peak {
                self.set_stacks(battle, actor, enemy, peak, Action::Ultimate);
            }
        }
        battle.units[actor].jiaoqiu_field = Field {
            expires_at_turn: battle.units[actor].normal_turns + ZONE_TURNS,
            triggers: 0,
            peak_stacks: peak,
        };
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if battle.hp(target).0 > 0 && battle.roll_effect(actor, target, self.talent_chance_bp) {
            self.add_stacks(
                battle,
                actor,
                target,
                1 + u8::from(self.eidolon >= 1),
                action,
            );
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Skill
            && let Some(enemy) = target
            && battle.hp(enemy).0 > 0
            && battle.roll_effect(actor, enemy, self.skill_chance_bp)
        {
            self.add_stacks(battle, actor, enemy, 1, action);
        }
    }

    fn on_enemy_turn_start(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if field_active(battle, owner)
            && battle.units[owner].jiaoqiu_field.triggers < ZONE_LIMIT
            && battle.roll_effect(owner, target, self.zone_chance_bp)
        {
            self.add_stacks(battle, owner, target, 1, Action::Ultimate);
            battle.units[owner].jiaoqiu_field.triggers += 1;
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.seared_scent && field_active(battle, owner) {
            let inherited = battle.units[owner].jiaoqiu_field.peak_stacks.max(1);
            for enemy in battle.enemies() {
                self.set_stacks(battle, owner, enemy, inherited, Action::Ultimate);
            }
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if self.eidolon < 6 {
            return;
        }
        let stacks = self.stacks(battle, owner, target);
        if stacks > 0
            && let Some(recipient) = battle
                .enemies()
                .filter(|&enemy| battle.hp(enemy).0 > 0)
                .min_by_key(|&enemy| self.stacks(battle, owner, enemy))
        {
            let origin = battle.current_action().unwrap_or(Action::Skill);
            self.add_stacks(battle, owner, recipient, stacks, origin);
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.hearth_kindle && owner == target {
            let overflow = battle
                .effective_effect_hit_rate_bp(owner)
                .saturating_sub(8_000);
            (overflow / 1_500).min(4) * 6_000
        } else {
            0
        }
    }

    fn ally_target_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _attacker: usize,
        enemy: usize,
        kind: DamageKind,
    ) -> u32 {
        if self.eidolon > 0
            && kind != DamageKind::WeaknessBreak
            && self.stacks(battle, owner, enemy) > 0
        {
            4_000
        } else {
            0
        }
    }

    fn ultimate_vulnerability_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        if field_active(battle, owner) {
            self.ultimate_vulnerability_bp
        } else {
            0
        }
    }

    fn attack_reduction_bp(&self, battle: &Battle<'_>, owner: usize, _enemy: usize) -> u32 {
        if self.eidolon >= 4 && field_active(battle, owner) {
            1_500
        } else {
            0
        }
    }

    fn all_resistance_down_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.eidolon >= 6 {
            u32::from(self.stacks(battle, owner, target)) * 300
        } else {
            0
        }
    }
}
