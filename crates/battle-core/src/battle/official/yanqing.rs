use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1209";
const BASIC: &str = "skill.120901";
const SKILL: &str = "skill.120902";
const ULTIMATE: &str = "skill.120903";
const TALENT: &str = "skill.120904";
const ICING_ON_THE_KICK: &str = "trace.1209101";
const FROST_FAVORS_THE_BRAVE: &str = "trace.1209102";
const GENTLE_BLADE: &str = "trace.1209103";

const A2_ADDITIONAL_DAMAGE_BP: u16 = 3_000;
const A4_EFFECT_RESISTANCE_BP: u32 = 2_000;
const A6_SPEED_BP: u32 = 1_000;
const E1_ADDITIONAL_DAMAGE_BP: u16 = 6_000;
const E2_ENERGY_REGENERATION_BP: u32 = 1_000;
const E4_ICE_PENETRATION_BP: u32 = 1_200;

struct Yanqing {
    ultimate_crit_rate_bp: u32,
    ultimate_crit_damage_bp: u32,
    soulsteel_crit_rate_bp: u32,
    soulsteel_crit_damage_bp: u32,
    follow_up_chance_bp: u32,
    follow_up_damage_bp: u16,
    freeze_damage_bp: u16,
    freeze_chance_bp: u32,
    icing_on_the_kick: bool,
    frost_favors_the_brave: bool,
    gentle_blade: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }

    let [basic] = input.parameters(BASIC, 1)? else {
        return Err(invalid(BASIC));
    };
    let [skill] = input.parameters(SKILL, 1)? else {
        return Err(invalid(SKILL));
    };
    let [ultimate_crit_rate, ultimate_crit_damage, ultimate] = input.parameters(ULTIMATE, 3)?
    else {
        return Err(invalid(ULTIMATE));
    };
    let [
        soulsteel_crit_rate,
        soulsteel_crit_damage,
        follow_up_chance,
        follow_up_damage,
        freeze_damage,
        freeze_chance,
    ] = input.parameters(TALENT, 6)?
    else {
        return Err(invalid(TALENT));
    };
    let trace = |id| input.enabled_trace_ids.iter().any(|item| item == id);

    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: attack(
            BASIC,
            *basic,
            &[(5_000, 5), (2_500, 2), (2_500, 3)],
            20_000,
            SkillPoints::Generate,
        )?,
        skill: attack(SKILL, *skill, &[(2_500, 5); 4], 30_000, SkillPoints::Spend)?,
        ultimate: attack(
            ULTIMATE,
            *ultimate,
            &[(10_000, 30)],
            5_000,
            SkillPoints::Unchanged,
        )?,
        mechanics: Arc::new(Yanqing {
            ultimate_crit_rate_bp: probability_bp(ULTIMATE, *ultimate_crit_rate)?,
            ultimate_crit_damage_bp: u32::from(ratio_bp(ULTIMATE, *ultimate_crit_damage)?),
            soulsteel_crit_rate_bp: probability_bp(TALENT, *soulsteel_crit_rate)?,
            soulsteel_crit_damage_bp: u32::from(ratio_bp(TALENT, *soulsteel_crit_damage)?),
            follow_up_chance_bp: probability_bp(TALENT, *follow_up_chance)?,
            follow_up_damage_bp: ratio_bp(TALENT, *follow_up_damage)?,
            freeze_damage_bp: ratio_bp(TALENT, *freeze_damage)?,
            freeze_chance_bp: probability_bp(TALENT, *freeze_chance)?,
            icing_on_the_kick: trace(ICING_ON_THE_KICK),
            frost_favors_the_brave: trace(FROST_FAVORS_THE_BRAVE),
            gentle_blade: trace(GENTLE_BLADE),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM} {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(0.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn probability_bp(id: &str, chance: f64) -> Result<u32, BattleError> {
    if chance > 1.0 {
        return Err(invalid(id));
    }
    ratio_bp(id, chance).map(u32::from)
}

fn attack(
    id: &str,
    ratio: f64,
    hit_split: &[(u16, u16)],
    energy_milli: u32,
    skill_points: SkillPoints,
) -> Result<OfficialAbility, BattleError> {
    let total = ratio_bp(id, ratio)?;
    let mut remaining = total;
    let effects = hit_split
        .iter()
        .enumerate()
        .map(|(index, &(share_bp, toughness))| {
            let multiplier_bp = if index + 1 == hit_split.len() {
                remaining
            } else {
                (u32::from(total) * u32::from(share_bp) / 10_000) as u16
            };
            remaining -= multiplier_bp;
            Effect::Hit {
                element: Element::Ice,
                multiplier_bp,
                toughness,
            }
        })
        .collect();
    Ok(OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    })
}

fn soulsteel_active(battle: &Battle<'_>, actor: usize) -> bool {
    let unit = &battle.units[actor];
    unit.finished_turns < unit.yanqing_state.soulsteel_expires_after_turn
}

fn ultimate_active(battle: &Battle<'_>, actor: usize) -> bool {
    let unit = &battle.units[actor];
    unit.finished_turns < unit.yanqing_state.ultimate_expires_after_turn
}

impl Yanqing {
    fn finish_hit(&self, battle: &mut Battle<'_>, actor: usize, target: usize, origin: Action) {
        let was_frozen = battle.units[actor].yanqing_state.target_was_frozen;
        if self.icing_on_the_kick && battle.has_weakness(target, Element::Ice) {
            battle.secondary_hit(
                actor,
                target,
                Element::Ice,
                A2_ADDITIONAL_DAMAGE_BP,
                DamageKind::Additional,
                origin,
            );
        }
        if self.eidolon >= 1 && was_frozen {
            battle.secondary_hit(
                actor,
                target,
                Element::Ice,
                E1_ADDITIONAL_DAMAGE_BP,
                DamageKind::Additional,
                origin,
            );
        }
    }

    fn apply_crit_speed(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.gentle_blade {
            battle.apply_speed_up(actor, actor, A6_SPEED_BP, 2);
        }
    }
}

impl Mechanics for Yanqing {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let target_was_frozen = target.is_some_and(|unit| battle.is_frozen(unit));
        let soulsteel_active = soulsteel_active(battle, actor);
        let unit = &mut battle.units[actor];
        unit.yanqing_state.target_was_frozen = target_was_frozen;
        if action == Action::Ultimate {
            unit.yanqing_state.ultimate_expires_after_turn = unit.finished_turns + 1;
            unit.yanqing_state.ultimate_crit_damage = soulsteel_active;
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if let Some(target) = target {
            self.finish_hit(battle, actor, target, action);
            if soulsteel_active(battle, actor)
                && battle.hp(target).0 > 0
                && battle.roll_fixed(self.follow_up_chance_bp)
            {
                battle.units[actor].yanqing_state.target_was_frozen = battle.is_frozen(target);
                battle.follow_up_single(
                    actor,
                    target,
                    action,
                    FollowUpAttack {
                        element: Element::Ice,
                        multiplier_bp: self.follow_up_damage_bp,
                        toughness: 10,
                        energy_milli: 10_000,
                        first_hit: Some(((u32::from(self.follow_up_damage_bp) * 3 / 10) as u16, 3)),
                    },
                );
            }
        }
        if action == Action::Skill {
            // The skill grants the status after its attack and the follow-up check.
            let unit = &mut battle.units[actor];
            let expires = &mut unit.yanqing_state.soulsteel_expires_after_turn;
            *expires = (*expires).max(unit.finished_turns + 2);
        }
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
    ) {
        self.finish_hit(battle, actor, target, origin);
        battle.apply_skill_freeze(
            actor,
            target,
            self.freeze_chance_bp,
            1,
            self.freeze_damage_bp,
            origin,
        );
    }

    fn after_hp_loss(&self, battle: &mut Battle<'_>, actor: usize, _source: usize, _hp_lost: u32) {
        battle.units[actor]
            .yanqing_state
            .soulsteel_expires_after_turn = 0;
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        u32::from(soulsteel_active(battle, actor)) * self.soulsteel_crit_rate_bp
            + u32::from(ultimate_active(battle, actor)) * self.ultimate_crit_rate_bp
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        let has_ultimate_bonus = ultimate_active(battle, actor)
            && battle.units[actor].yanqing_state.ultimate_crit_damage;
        u32::from(soulsteel_active(battle, actor)) * self.soulsteel_crit_damage_bp
            + u32::from(has_ultimate_bonus) * self.ultimate_crit_damage_bp
    }

    fn after_critical_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Action,
    ) {
        self.apply_crit_speed(battle, actor);
    }

    fn after_non_primary_critical_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Action,
        _kind: DamageKind,
    ) {
        self.apply_crit_speed(battle, actor);
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(actor);
        if self.eidolon >= 4
            && element == Element::Ice
            && u64::from(hp) * 5 >= u64::from(max_hp) * 4
        {
            E4_ICE_PENETRATION_BP
        } else {
            0
        }
    }

    fn energy_regeneration_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        u32::from(self.eidolon >= 2 && soulsteel_active(battle, actor)) * E2_ENERGY_REGENERATION_BP
    }

    fn effect_resistance_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        u32::from(self.frost_favors_the_brave && soulsteel_active(battle, actor))
            * A4_EFFECT_RESISTANCE_BP
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        _target: usize,
    ) {
        if self.eidolon < 6 {
            return;
        }
        let unit = &mut battle.units[owner];
        let state = &mut unit.yanqing_state;
        if unit.finished_turns < state.soulsteel_expires_after_turn {
            state.soulsteel_expires_after_turn += 1;
        }
        if unit.finished_turns < state.ultimate_expires_after_turn {
            state.ultimate_expires_after_turn += 1;
        }
    }
}
