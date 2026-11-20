use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110601";
const SKILL: &str = "skill.110602";
const ULTIMATE: &str = "skill.110603";
const TALENT: &str = "skill.110604";
const A2: &str = "trace.1106101";
const A4: &str = "trace.1106102";
const A6: &str = "trace.1106103";
const BASH_DAMAGE_BP: u32 = 2_000;
const SECRET_STRATEGY_EFFECT_HIT_RATE_BP: u32 = 1_000;
const WIPE_OUT_DAMAGE_BP: u32 = 2_000;
const E1_DEFEAT_ENERGY_MILLI: u32 = 5_000;
const E2_SPEED_UP_BP: u32 = 1_000;
const E2_DURATION: u32 = 2;
const E4_BASE_CHANCE_BP: u32 = 10_000;
const E4_ICE_RESISTANCE_DOWN_BP: u32 = 1_200;
const E4_DURATION: u32 = 2;
const E6_MULTIPLIER_BP: u16 = 4_000;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "character.1106 energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 1)?;
    if skill[1] != 1.0
        || !(0.0..=1.0).contains(&ultimate[0])
        || !(0.0..=1.0).contains(&ultimate[1])
        || !(1.0..=f64::from(u8::MAX)).contains(&ultimate[2])
        || ultimate[2].fract() != 0.0
        || !(0.0..=110.0).contains(&talent[0])
    {
        return Err(BattleError::InvalidInput(
            "character.1106 dispel count, Exposed chance or duration, or Talent energy".into(),
        ));
    }
    let enabled = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let basic_hit = ice_hit(BASIC, basic[0] / 2.0, 5)?;
    Ok(OfficialKit {
        max_energy_milli: input.energy_capacity * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![basic_hit, basic_hit],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![ice_hit(SKILL, skill[0], 20)?],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![ice_hit(ULTIMATE, ultimate[3], 20)?],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Pela {
            exposed_chance_bp: (ultimate[0] * 10_000.0).round() as u32,
            exposed_defense_down_bp: (ultimate[1] * 10_000.0).round() as u32,
            exposed_turns: ultimate[2] as u32,
            talent_energy_milli: (talent[0] * 1_000.0).round() as u32,
            eidolon: input.eidolon,
            bash: enabled(A2),
            secret_strategy: enabled(A4),
            wipe_out: enabled(A6),
        }),
    })
}

fn ice_hit(id: &str, multiplier: f64, toughness: u16) -> Result<Effect, BattleError> {
    let multiplier_bp = (multiplier * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&multiplier_bp) {
        return Err(BattleError::InvalidInput(format!(
            "character.1106 {id} damage multiplier"
        )));
    }
    Ok(Effect::Hit {
        element: Element::Ice,
        multiplier_bp: multiplier_bp as u16,
        toughness,
    })
}

struct Pela {
    exposed_chance_bp: u32,
    exposed_defense_down_bp: u32,
    exposed_turns: u32,
    talent_energy_milli: u32,
    eidolon: u8,
    bash: bool,
    secret_strategy: bool,
    wipe_out: bool,
}

impl Mechanics for Pela {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Ultimate, _) => {
                for enemy in battle.enemies() {
                    if battle.hp(enemy).0 > 0
                        && battle.roll_effect(actor, enemy, self.exposed_chance_bp)
                    {
                        battle.apply_defense_down(
                            actor,
                            enemy,
                            self.exposed_defense_down_bp,
                            self.exposed_turns,
                        );
                    }
                }
            }
            (Action::Skill, Some(enemy)) => {
                if battle.dispel_one(actor, enemy) {
                    if self.eidolon >= 2 {
                        battle.apply_speed_up(actor, actor, E2_SPEED_UP_BP, E2_DURATION);
                    }
                    if self.wipe_out {
                        battle.arm_next_attack_bonus(actor, WIPE_OUT_DAMAGE_BP);
                    }
                }
                if self.eidolon >= 4 && battle.roll_effect(actor, enemy, E4_BASE_CHANCE_BP) {
                    battle.apply_resistance_down(
                        actor,
                        enemy,
                        Element::Ice,
                        E4_ICE_RESISTANCE_DOWN_BP,
                        E4_DURATION,
                    );
                }
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let targets = target.map_or_else(|| battle.enemies(), |enemy| enemy..enemy + 1);
        let mut trigger_talent = false;
        for enemy in targets {
            if battle.hp(enemy).0 == 0 || !battle.is_debuffed(enemy) {
                continue;
            }
            trigger_talent = true;
            if self.eidolon >= 6 {
                battle.secondary_hit(
                    actor,
                    enemy,
                    Element::Ice,
                    E6_MULTIPLIER_BP,
                    DamageKind::Additional,
                    action,
                );
            }
        }
        if trigger_talent {
            battle.gain_energy(actor, actor, self.talent_energy_milli);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        if self.bash && battle.is_debuffed(target) {
            BASH_DAMAGE_BP
        } else {
            0
        }
    }

    fn effect_hit_rate_aura_bp(&self) -> u32 {
        if self.secret_strategy {
            SECRET_STRATEGY_EFFECT_HIT_RATE_BP
        } else {
            0
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        _target: usize,
    ) {
        if self.eidolon >= 1 {
            battle.gain_energy(owner, owner, E1_DEFEAT_ENERGY_MILLI);
        }
    }
}
