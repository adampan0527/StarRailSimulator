use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        elation::AhaInstant,
        resolve::ElationHit,
        support::{OffenseContext, percentage_of},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::Healing,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1503";
const BASIC: &str = "skill.150301";
const SKILL: &str = "skill.150302";
const ULTIMATE: &str = "skill.150303";
const TALENT: &str = "skill.150304";
const MOON_BASIC: &str = "skill.150308";
const WAVE_BASIC: &str = "skill.150310";
const ELATION: &str = "skill.150320";
const ICE: Element = Element::Ice;

#[derive(Clone, Copy)]
struct DeepLearning {
    archetype: usize,
    charges: u8,
}

#[derive(Clone, Copy)]
struct NextAttack {
    multiplier_bp: u32,
    instant: AhaInstant,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct PearlState {
    learning: Option<DeepLearning>,
    refund_target: Option<usize>,
    next_attacks: Vec<(usize, NextAttack)>,
    repellency_spent: u32,
    latitude_granted: u32,
    resurrections: u8,
}

struct Pearl {
    eidolon: u8,
    vision: bool,
    latitude: bool,
    firewall: bool,
    healing_bp: u32,
    skill_banger: u32,
    skill_healing: Healing,
    learning_charges: u8,
    learning_hit_bp: u32,
    ultimate_banger: u32,
    advance: [u16; 3],
    extra_punchline: u32,
    extra_banger: u32,
    low_hp_threshold_bp: u32,
    block_bp: u32,
    banger_cap: u32,
    reduction_bp: u32,
    repellency_per_banger: u32,
    moon_basic: OfficialAbility,
    wave_basic: OfficialAbility,
    moon_healing: Healing,
    wave_healing: Healing,
    moon_hit_bp: u32,
    elation_multipliers: [u32; 4],
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id} combat value"))
}

fn ratio_bp(id: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn counter(id: &str, value: f64) -> Result<u8, BattleError> {
    if !(1.0..=f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u8)
}

fn basic_ability(multiplier_bp: u16, enhanced: bool) -> OfficialAbility {
    OfficialAbility {
        target: if enhanced {
            Target::AllEnemies
        } else {
            Target::Enemy
        },
        effects: vec![Effect::Hit {
            element: ICE,
            multiplier_bp,
            toughness: if enhanced { 30 } else { 10 },
        }],
        energy_milli: if enhanced { 30_000 } else { 20_000 },
        skill_points: SkillPoints::Generate,
    }
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != ICE
        || input.energy_capacity != 180
    {
        return Err(invalid("combat type or energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 8)?;
    let talent = input.parameters(TALENT, 5)?;
    let moon = input.parameters(MOON_BASIC, 5)?;
    let wave = input.parameters(WAVE_BASIC, 4)?;
    let elation = input.parameters(ELATION, 4)?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    Ok(OfficialKit {
        max_energy_milli: 180_000,
        basic: basic_ability(ratio_bp(BASIC, basic[0])?, false),
        skill: OfficialAbility {
            target: Target::AllAllies,
            effects: Vec::new(),
            energy_milli: 30_000,
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::OtherAlly,
            effects: Vec::new(),
            energy_milli: 5_000,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Pearl {
            eidolon: input.eidolon,
            vision: has_trace("trace.1503101"),
            latitude: has_trace("trace.1503102"),
            firewall: has_trace("trace.1503103"),
            healing_bp: input.stats.healing_bp,
            skill_banger: u32::from(counter(SKILL, skill[0])?),
            skill_healing: Healing::from_data(FORM, SKILL, skill[1], skill[2])?,
            learning_charges: counter(ULTIMATE, ultimate[0])?,
            learning_hit_bp: u32::from(ratio_bp(ULTIMATE, ultimate[1])?),
            ultimate_banger: u32::from(counter(ULTIMATE, ultimate[2])?),
            advance: [
                ratio_bp(ULTIMATE, ultimate[3])?,
                ratio_bp(ULTIMATE, ultimate[4])?,
                ratio_bp(ULTIMATE, ultimate[5])?,
            ],
            extra_punchline: u32::from(counter(ULTIMATE, ultimate[6])?),
            extra_banger: u32::from(counter(ULTIMATE, ultimate[7])?),
            low_hp_threshold_bp: u32::from(ratio_bp(TALENT, talent[0])?),
            block_bp: u32::from(ratio_bp(TALENT, talent[1])?),
            banger_cap: u32::from(counter(TALENT, talent[2])?),
            reduction_bp: u32::from(ratio_bp(TALENT, talent[3])?),
            repellency_per_banger: u32::from(counter(TALENT, talent[4])?),
            moon_basic: basic_ability(ratio_bp(MOON_BASIC, moon[0])?, true),
            wave_basic: basic_ability(ratio_bp(WAVE_BASIC, wave[0])?, true),
            moon_healing: Healing::from_data(FORM, MOON_BASIC, moon[1], moon[3])?,
            wave_healing: Healing::from_data(FORM, WAVE_BASIC, wave[1], wave[3])?,
            moon_hit_bp: u32::from(ratio_bp(MOON_BASIC, moon[4])?),
            elation_multipliers: [
                u32::from(ratio_bp(ELATION, elation[0])?),
                u32::from(ratio_bp(ELATION, elation[1])?),
                u32::from(ratio_bp(ELATION, elation[2])?),
                u32::from(ratio_bp(ELATION, elation[3])?),
            ],
        }),
    })
}

fn is_elation(battle: &Battle<'_>, actor: usize) -> bool {
    battle.plan.units[actor]
        .combat_type
        .is_some_and(|kind| kind.path == CombatPath::Elation)
}

fn lowest_hp(battle: &Battle<'_>) -> Option<usize> {
    battle
        .party_units()
        .filter(|&actor| battle.is_targetable(actor))
        .min_by(|&left, &right| {
            let (left_hp, left_max) = battle.hp(left);
            let (right_hp, right_max) = battle.hp(right);
            (u64::from(left_hp) * u64::from(right_max))
                .cmp(&(u64::from(right_hp) * u64::from(left_max)))
        })
}

impl Pearl {
    fn heal_party(&self, battle: &mut Battle<'_>, actor: usize, healing: Healing) {
        let lowest = lowest_hp(battle);
        let defense = battle.effective_defense(actor);
        let vision_bonus = if self.vision {
            let bonus = battle.offense_buffs(
                OffenseContext {
                    actor,
                    max_hp_source: actor,
                    enemy: None,
                    element: ICE,
                    kind: DamageKind::Elation,
                    break_damage: false,
                    follow_up: false,
                    ultimate: false,
                    direct_skill: false,
                    summon: false,
                },
                |_| true,
            );
            percentage_of(
                battle
                    .intrinsic_elation_bp(actor)
                    .saturating_add(bonus.elation_bp),
                2_000,
            )
        } else {
            0
        };
        let amount = healing.amount(
            defense,
            u64::from(self.healing_bp.saturating_add(vision_bonus)) * 100,
        );
        for ally in battle.party_units() {
            if !battle.is_targetable(ally) {
                continue;
            }
            if self.latitude {
                battle.cleanse_one(actor, ally);
            }
            battle.heal_unit(actor, ally, amount);
            if Some(ally) == lowest {
                battle.heal_unit(actor, ally, amount);
            }
        }
    }

    fn end_learning(battle: &mut Battle<'_>, actor: usize) {
        if let Some(learning) = battle.units[actor].pearl_state.learning.take() {
            battle.emit(
                EventKind::StatusExpired(StatusKind::DeepLearning),
                Some(actor),
                Some(learning.archetype),
            );
        }
    }

    fn ultimate(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.grant_certified_banger(actor, self.ultimate_banger);
        if battle.units[actor]
            .pearl_state
            .learning
            .is_some_and(|learning| learning.archetype != target)
        {
            Self::end_learning(battle, actor);
        }
        let refund_target = (self.firewall && is_elation(battle, target)).then_some(target);
        let state = &mut battle.units[actor].pearl_state;
        state.learning = Some(DeepLearning {
            archetype: target,
            charges: self.learning_charges,
        });
        state.refund_target = refund_target;
        battle.emit(
            EventKind::StatusApplied(StatusKind::DeepLearning),
            Some(actor),
            Some(target),
        );
        let count = battle.elation_team_size();
        let advance = self.advance[count.saturating_sub(1).min(2)];
        for ally in battle.allies() {
            if battle.is_targetable(ally)
                && (ally == target
                    || (self.eidolon >= 2 && ally != actor && is_elation(battle, ally)))
            {
                battle.advance_ally(actor, ally, advance);
            }
        }
        if count >= 4 {
            battle.queue_granted_extra_turn(actor, target);
        }
    }

    fn enhanced_basic(&self, battle: &mut Battle<'_>, actor: usize, learning: DeepLearning) {
        let moon = is_elation(battle, learning.archetype);
        self.heal_party(
            battle,
            actor,
            if moon {
                self.moon_healing
            } else {
                self.wave_healing
            },
        );
        let points = battle.certified_banger(actor).unwrap_or(0);
        for target in battle.enemies() {
            if !battle.is_targetable(target) {
                continue;
            }
            if moon && points > 0 {
                battle.elation_hit(
                    actor,
                    target,
                    ElationHit {
                        element: ICE,
                        multiplier_bp: self.moon_hit_bp,
                        punchline: points,
                        toughness: 0,
                        origin: Action::Basic,
                        factor_bp: 10_000,
                        assisted_by: None,
                    },
                );
            }
            let borrowed_bp =
                self.learning_hit_bp * u32::from(moon) + 24_000 * u32::from(self.eidolon >= 6);
            if borrowed_bp > 0 && battle.is_targetable(learning.archetype) {
                battle.elation_hit_using_actor_stats(
                    learning.archetype,
                    target,
                    ElationHit {
                        element: ICE,
                        multiplier_bp: borrowed_bp,
                        punchline: points,
                        toughness: 0,
                        origin: Action::Basic,
                        factor_bp: 10_000,
                        assisted_by: Some(actor),
                    },
                );
            }
        }
        if let Some(current) = &mut battle.units[actor].pearl_state.learning {
            current.charges -= 1;
            if current.charges == 0 {
                Self::end_learning(battle, actor);
            }
        }
    }
}

impl Mechanics for Pearl {
    fn elation_participant(&self) -> Option<u16> {
        Some(104)
    }

    fn scales_with_defense(&self) -> bool {
        true
    }

    fn permanent_banger_cap(&self, battle: &Battle<'_>) -> Option<u32> {
        Some(if battle.elation_ascension().is_some() {
            75
        } else {
            self.banger_cap
        })
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if action != Action::Basic {
            return None;
        }
        let learning = battle.units[actor].pearl_state.learning?;
        Some(if is_elation(battle, learning.archetype) {
            &self.moon_basic
        } else {
            &self.wave_basic
        })
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Basic => {
                if let Some(learning) = battle.units[actor].pearl_state.learning {
                    self.enhanced_basic(battle, actor, learning);
                }
            }
            Action::Skill => {
                battle.grant_certified_banger(actor, self.skill_banger);
                self.heal_party(battle, actor, self.skill_healing);
            }
            Action::Ultimate => {
                if let Some(target) = target {
                    self.ultimate(battle, actor, target);
                }
            }
            _ => {}
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _: Option<usize>,
    ) -> bool {
        battle.grant_certified_banger(actor, self.skill_banger);
        self.heal_party(battle, actor, self.skill_healing);
        true
    }

    fn absorb_ally_damage(&self, battle: &mut Battle<'_>, owner: usize, damage: u32) -> u32 {
        let spent = battle.units[owner].pearl_state.repellency_spent;
        let capacity = battle
            .certified_banger(owner)
            .unwrap_or(0)
            .saturating_mul(self.repellency_per_banger)
            .saturating_sub(spent);
        let blocked = capacity.min(percentage_of(damage, self.block_bp));
        let used = spent + blocked;
        battle.units[owner].pearl_state.repellency_spent = used % self.repellency_per_banger;
        battle.spend_certified_banger(owner, used / self.repellency_per_banger);
        blocked
    }

    fn ally_damage_reduction_bp_for_target(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(target);
        if battle.is_targetable(owner) && hp <= percentage_of(max_hp, self.low_hp_threshold_bp) {
            self.reduction_bp
        } else {
            0
        }
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let defense = battle.effective_defense(actor);
        if self.vision && defense >= 2_400 {
            3_200 + ((defense - 2_400).min(3_600) / 100) * 300
        } else {
            0
        }
    }

    fn effect_resistance_aura_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        5_000
            * u32::from(
                self.latitude
                    && battle.is_party_unit(target)
                    && battle.certified_banger(owner).unwrap_or(0) > 0,
            )
    }

    fn on_ally_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        if !self.latitude || !battle.is_party_unit(actor) {
            return;
        }
        let earned = &mut battle.units[owner].pearl_state.latitude_granted;
        if owner == actor {
            *earned = 0;
        }
        if *earned < 50 {
            *earned += 5;
            battle.grant_certified_banger(owner, 5);
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        let refund = &mut battle.units[owner].pearl_state.refund_target;
        if *refund == Some(actor) && action.is_ultimate() {
            *refund = None;
            battle.restore_energy_flat(actor, owner, 90_000);
        }
    }

    fn on_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize) {
        if battle.units[owner]
            .pearl_state
            .learning
            .is_some_and(|learning| !battle.is_targetable(learning.archetype))
        {
            Self::end_learning(battle, owner);
        }
        battle.enforce_permanent_banger_caps();
    }

    fn after_granted_extra_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if battle.is_targetable(owner) {
            let factor = 1 + u32::from(self.eidolon >= 2);
            battle.start_extra_turn_elation(
                actor,
                self.extra_banger * factor,
                self.extra_punchline * factor,
            );
        }
    }

    fn after_granted_extra_turn_end(&self, battle: &mut Battle<'_>, _: usize) {
        let retained_bp = battle
            .elation_ascension()
            .map_or(0, |ascension| u32::from(ascension.innate_level) * 1_000);
        battle.finish_extra_turn_elation(retained_bp, retained_bp);
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        let index = battle.elation_team_size().saturating_sub(1).min(3);
        let multiplier_bp = self.elation_multipliers[index] * (1 + u32::from(self.eidolon >= 4));
        battle.units[actor].pearl_state.next_attacks = battle
            .party_units()
            .filter(|&ally| battle.is_targetable(ally))
            .map(|ally| {
                (
                    ally,
                    NextAttack {
                        multiplier_bp,
                        instant,
                    },
                )
            })
            .collect();
        battle.gain_energy(actor, actor, 5_000);
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        if !battle.is_targetable(owner) || !battle.is_party_unit(actor) {
            return;
        }
        let element_source = battle.memosprite_owner(actor).unwrap_or(actor);
        let Some(combat) = battle.plan.units[element_source].combat_type else {
            return;
        };
        let attacks = &mut battle.units[owner].pearl_state.next_attacks;
        let Some(index) = attacks.iter().position(|&(unit, _)| unit == actor) else {
            return;
        };
        let (_, next) = attacks.swap_remove(index);
        for target in battle.enemies() {
            if battle.is_targetable(target) && battle.attack_targets().contains(&target) {
                battle.elation_hit(
                    actor,
                    target,
                    ElationHit {
                        element: combat.element,
                        multiplier_bp: next.multiplier_bp,
                        punchline: next.instant.punchline,
                        toughness: 0,
                        origin,
                        factor_bp: next.instant.skill_damage_factor_bp,
                        assisted_by: Some(owner),
                    },
                );
            }
        }
    }

    fn ally_elation_bp(&self, battle: &Battle<'_>, _: usize, _: usize) -> u32 {
        if self.eidolon < 1 {
            return 0;
        }
        match battle.elation_team_size() {
            0 | 1 => 0,
            2 => 1_000,
            3 => 2_000,
            _ => 6_000,
        }
    }

    fn prevent_team_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        target: usize,
    ) -> bool {
        let used = &mut battle.units[owner].pearl_state.resurrections;
        if self.eidolon < 1 || *used >= 2 {
            return false;
        }
        *used += 1;
        let amount = percentage_of(battle.effective_max_hp(target), 5_000).max(1);
        battle.restore_hp_unmodified(owner, target, amount) > 0
    }

    fn ally_merrymake_bp(&self, _: &Battle<'_>, _: usize, _: usize) -> u32 {
        1_500 * u32::from(self.eidolon >= 2)
    }

    fn ally_resistance_penetration_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 6 && battle.units[owner].pearl_state.learning.is_some())
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => {
                let preferred = battle
                    .allies()
                    .filter(|&ally| ally != actor && battle.is_targetable(ally))
                    .max_by_key(|&ally| {
                        (
                            is_elation(battle, ally),
                            battle.intrinsic_elation_bp(ally),
                            Reverse(ally),
                        )
                    });
                Some(u8::from(target != preferred))
            }
            Action::Skill
                if battle.certified_banger(actor).unwrap_or(0) < 15
                    && battle.units[actor].pearl_state.learning.is_none() =>
            {
                Some(2)
            }
            _ => None,
        }
    }
}
