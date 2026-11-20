use std::sync::Arc;

use super::{
    super::{Battle, resolve::CounterAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.100101";
const SKILL: &str = "skill.100102";
const ULTIMATE: &str = "skill.100103";
const TALENT: &str = "skill.100104";
const PURIFY: &str = "trace.1001101";
const REINFORCE: &str = "trace.1001102";
const ICE_SPELL: &str = "trace.1001103";
const MICROS: u64 = 1_000_000;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 {
        return Err(BattleError::InvalidInput(
            "March 7th energy capacity".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 2)?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let shield = ShieldStrength::new(skill[0], skill[3])?;
    let freeze_chance_bp = u32::from(basis_points(ULTIMATE, ultimate[1])?)
        + if has_trace(ICE_SPELL) { 1_500 } else { 0 };
    let e4 = input.eidolon >= 4;
    let mechanics = MarchPreservation {
        cleanse: has_trace(PURIFY),
        shield,
        shield_turns: u32::from(count(SKILL, skill[1])?) + u32::from(has_trace(REINFORCE)),
        freeze_chance_bp,
        frozen_turns: u32::from(count(ULTIMATE, ultimate[2])?),
        frozen_damage_bp: basis_points(ULTIMATE, ultimate[3])?,
        counter_multiplier_bp: basis_points(TALENT, talent[0])?,
        counter_defense_multiplier_bp: if e4 { 3_000 } else { 0 },
        counters_per_turn: count(TALENT, talent[1])? + u8::from(e4),
        eidolon: input.eidolon,
    };
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: ice_attack(
            Target::Enemy,
            basis_points(BASIC, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: ice_attack(
            Target::AllEnemies,
            basis_points(ULTIMATE, ultimate[0])?,
            20,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(mechanics),
    })
}

fn ice_attack(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Ice,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "March 7th {id} multiplier"
        )));
    }
    Ok(value as u16)
}

fn count(id: &str, value: f64) -> Result<u8, BattleError> {
    if !(1.0..f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(BattleError::InvalidInput(format!(
            "March 7th {id} duration or count"
        )));
    }
    Ok(value as u8)
}

struct ShieldStrength {
    ratio_micros: u64,
    flat_micros: u64,
}

impl ShieldStrength {
    fn new(defense_ratio: f64, flat: f64) -> Result<Self, BattleError> {
        if !(0.0..=10.0).contains(&defense_ratio) || !(0.0..=1_000_000.0).contains(&flat) {
            return Err(BattleError::InvalidInput(
                "March 7th shield strength".into(),
            ));
        }
        Ok(Self {
            ratio_micros: (defense_ratio * MICROS as f64).round() as u64,
            flat_micros: (flat * MICROS as f64).round() as u64,
        })
    }

    fn amount(&self, defense: u32) -> u32 {
        let value = (u64::from(defense) * self.ratio_micros + self.flat_micros) / MICROS;
        u32::try_from(value).unwrap_or(u32::MAX)
    }
}

struct MarchPreservation {
    cleanse: bool,
    shield: ShieldStrength,
    shield_turns: u32,
    freeze_chance_bp: u32,
    frozen_turns: u32,
    frozen_damage_bp: u16,
    counter_multiplier_bp: u16,
    counter_defense_multiplier_bp: u16,
    counters_per_turn: u8,
    eidolon: u8,
}

impl Mechanics for MarchPreservation {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon < 2 {
            return;
        }
        let target = battle
            .allies()
            .filter(|&ally| battle.hp(ally).0 > 0)
            .min_by(|&left, &right| {
                let (left_hp, left_max) = battle.hp(left);
                let (right_hp, right_max) = battle.hp(right);
                (u64::from(left_hp) * u64::from(right_max))
                    .cmp(&(u64::from(right_hp) * u64::from(left_max)))
                    .then_with(|| left.cmp(&right))
            });
        if let Some(target) = target {
            let shield = ShieldStrength {
                ratio_micros: 240_000,
                flat_micros: 320 * MICROS,
            };
            battle.apply_official_shield(
                actor,
                target,
                shield.amount(battle.defense(actor)),
                3,
                None,
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
        match (action, target) {
            (Action::Skill, Some(target)) => {
                if self.cleanse {
                    battle.cleanse_one(actor, target);
                }
                let amount = self.shield.amount(battle.defense(actor));
                battle.apply_official_shield(
                    actor,
                    target,
                    amount,
                    self.shield_turns,
                    Some(action),
                );
            }
            (Action::Ultimate, None) => {
                for target in battle.enemies() {
                    let frozen = battle.apply_skill_freeze(
                        actor,
                        target,
                        self.freeze_chance_bp,
                        self.frozen_turns,
                        self.frozen_damage_bp,
                        action,
                    );
                    if frozen && self.eidolon >= 1 {
                        battle.gain_energy(actor, actor, 6_000);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.eidolon >= 6 && battle.has_official_shield(owner, actor, Action::Skill) {
            let (_, max_hp) = battle.hp(actor);
            let amount = max_hp / 25 + 106;
            battle.heal_unit(owner, actor, amount);
        }
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        _target: usize,
        was_shielded: bool,
    ) {
        if !was_shielded
            || battle.hp(attacker).0 == 0
            || !battle.spend_counter_charge(owner, self.counters_per_turn)
        {
            return;
        }
        battle.counter_attack(
            owner,
            attacker,
            CounterAttack {
                element: Element::Ice,
                multiplier_bp: self.counter_multiplier_bp,
                defense_multiplier_bp: self.counter_defense_multiplier_bp,
                toughness: 10,
                splash: None,
                energy_milli: 10_000,
            },
        );
    }
}
