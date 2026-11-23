use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind,
    fixture::{Effect, Element, Stats, Target},
};

use super::{
    super::{
        Battle, SummonTurn,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1407";
const DRAGON: &str = "netherwing";
const QUANTUM: Element = Element::Quantum;
const NEWBUD_CAP: u32 = 34_000;
const STARTING_NEWBUD: u32 = 10_200;
const HEAL_CONVERSION_CAP: u32 = 4_080;
const BASIC: &str = "skill.140701";
const SKILL: &str = "skill.140702";
const ULTIMATE: &str = "skill.140703";
const TALENT: &str = "skill.140704";
const JOINT: &str = "skill.140709";
const CLAW: &str = "skill.1140701";
const BREATH: &str = "skill.1140702";
const BACKUP: &str = "skill.1140703";
const ROAR: &str = "skill.1140705";
const DETONATION: &str = "skill.1140706";
const DARK_TIDE: &str = "trace.1407101";
const INVERTED_TORCH: &str = "trace.1407102";
const WEST_WIND: &str = "trace.1407103";

#[derive(Clone, Default)]
pub(in crate::battle) struct CastoriceState {
    newbud: u32,
    cyrene_detonation_bp: u32,
    talent_stacks: u32,
    talent_until: u32,
    dragon_turn_at_summon: u32,
    breath_tier: usize,
    breath_stacks: u32,
    ardent_will: u8,
    joint_grants_newbud: bool,
    heal_conversion: BTreeMap<usize, u32>,
    dragon_exhausted: bool,
    mooncocoon_used: bool,
    mooncocoon_targets: Vec<(usize, usize)>,
}

struct Castorice {
    eidolon: u8,
    dark_tide: bool,
    inverted_torch: bool,
    west_wind: bool,
    cost_bp: u32,
    joint_cost_bp: u32,
    joint: OfficialAbility,
    joint_dragon_bp: u16,
    dragon_speed: u64,
    dragon_lifetime_turns: u32,
    dragon_hp: u32,
    territory_resistance_bp: u32,
    talent_bp: u32,
    talent_max_stacks: u32,
    talent_turns: u32,
    claw_bp: u16,
    breath_cost_bp: u32,
    breath_lethal_threshold_bp: u32,
    breath_bp: [u16; 3],
    overflow_ratio: u32,
    roar_damage_bp: u32,
    roar_turns: u32,
    detonation_bp: u16,
    detonation_hits: u32,
    detonation_heal_bp: u32,
    detonation_flat_heal: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.level != 80
        || input.energy_capacity != 0
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != QUANTUM
    {
        return Err(invalid("level, resource, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 4)?;
    let joint = input.parameters(JOINT, 3)?;
    let claw = input.parameters(CLAW, 1)?;
    let breath = input.parameters(BREATH, 5)?;
    let backup = input.parameters(BACKUP, 1)?;
    let roar = input.parameters(ROAR, 2)?;
    let detonation = input.parameters(DETONATION, 4)?;
    if ultimate[0] != 165.0
        || ultimate[1] != 3.0
        || ultimate[2] != 1.0
        || talent[2] != 3.0
        || talent[3] != 3.0
        || breath[0] != 0.25
        || breath[4] != 0.25
        || backup[0] != 5.0
        || roar[1] != 3.0
        || detonation[1] != 6.0
        || !(400.0..=1_500.0).contains(&detonation[3])
        || detonation[3].fract() != 0.0
    {
        return Err(invalid("memosprite constants"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let hp_hit = |id, value, toughness| {
        Ok(Effect::MaxHpHit {
            element: QUANTUM,
            multiplier_bp: ratio(id, value)?,
            toughness,
        })
    };
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![hp_hit(BASIC, basic[0], 10)?],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![
                hp_hit(SKILL, skill[1], 20)?,
                Effect::MaxHpBlastAdjacent {
                    element: QUANTUM,
                    multiplier_bp: ratio(SKILL, skill[2])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 0,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Castorice {
            eidolon: input.eidolon,
            dark_tide: has_trace(DARK_TIDE),
            inverted_torch: has_trace(INVERTED_TORCH),
            west_wind: has_trace(WEST_WIND),
            cost_bp: u32::from(ratio(SKILL, skill[0])?),
            joint_cost_bp: u32::from(ratio(JOINT, joint[0])?),
            joint: OfficialAbility {
                target: Target::AllEnemies,
                energy_milli: 0,
                // The frozen table reports a combined 20 toughness for the joint action.
                effects: vec![hp_hit(JOINT, joint[1], 10)?],
                skill_points: SkillPoints::Unchanged,
            },
            joint_dragon_bp: ratio(JOINT, joint[2])?,
            dragon_speed: ultimate[0] as u64 * 1_000_000,
            dragon_lifetime_turns: ultimate[1] as u32,
            dragon_hp: NEWBUD_CAP,
            territory_resistance_bp: u32::from(ratio(ULTIMATE, ultimate[3])?),
            talent_bp: u32::from(ratio(TALENT, talent[1])?),
            talent_max_stacks: talent[2] as u32,
            talent_turns: talent[3] as u32,
            claw_bp: ratio(CLAW, claw[0])?,
            breath_cost_bp: u32::from(ratio(BREATH, breath[0])?),
            breath_lethal_threshold_bp: u32::from(ratio(BREATH, breath[4])?),
            breath_bp: [
                ratio(BREATH, breath[1])?,
                ratio(BREATH, breath[2])?,
                ratio(BREATH, breath[3])?,
            ],
            overflow_ratio: backup[0] as u32,
            roar_damage_bp: u32::from(ratio(ROAR, roar[0])?),
            roar_turns: roar[1] as u32,
            detonation_bp: ratio(DETONATION, detonation[0])?,
            detonation_hits: detonation[1] as u32,
            detonation_heal_bp: u32::from(ratio(DETONATION, detonation[2])?),
            detonation_flat_heal: detonation[3] as u32,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("invalid Castorice {detail}"))
}

fn ratio(id: &str, value: f64) -> Result<u16, BattleError> {
    if !(0.0..=5.0).contains(&value) {
        return Err(invalid(id));
    }
    Ok((value * 10_000.0).round() as u16)
}

impl Castorice {
    fn set_newbud(battle: &mut Battle<'_>, owner: usize, newbud: u32) {
        let overflow = super::cyrene::active_ode_parameters(battle, owner, FORM).is_some();
        let after = newbud.min(NEWBUD_CAP * (1 + u32::from(overflow)));
        let before = std::mem::replace(&mut battle.units[owner].castorice_state.newbud, after);
        if before != after {
            battle.emit(EventKind::Newbud { before, after }, Some(owner), None);
        }
    }

    fn gain_newbud(battle: &mut Battle<'_>, owner: usize, amount: u32) {
        let newbud = battle.units[owner]
            .castorice_state
            .newbud
            .saturating_add(amount);
        Self::set_newbud(battle, owner, newbud);
    }

    fn convert_to_newbud_or_dragon_hp(battle: &mut Battle<'_>, owner: usize, amount: u32) {
        if let Some(sprite) = battle.active_memosprite(owner) {
            if !battle.units[owner].castorice_state.dragon_exhausted {
                battle.restore_hp_unmodified(owner, sprite, amount);
            }
        } else {
            Self::gain_newbud(battle, owner, amount);
        }
    }

    fn summon(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.memosprite(owner) else {
            return;
        };
        battle.units[sprite].memosprite_max_hp = Some(self.dragon_hp);
        battle.units[sprite].memosprite_generation += 1;
        battle.units[sprite].hp = battle.effective_max_hp(sprite);
        battle.units[owner].summon_turn = Some(SummonTurn::new(DRAGON, self.dragon_speed));
        let normal_turns = battle.units[sprite].normal_turns;
        let state = &mut battle.units[owner].castorice_state;
        state.dragon_turn_at_summon = normal_turns;
        state.breath_tier = 0;
        state.breath_stacks = 0;
        state.dragon_exhausted = false;
        state.ardent_will = 2 * u8::from(self.eidolon >= 2);
        state.joint_grants_newbud = self.eidolon >= 2;
        battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
        for target in battle.party_units() {
            if battle.is_targetable(target) {
                battle.apply_ally_buff(
                    owner,
                    target,
                    AllyBuffKind::Damage,
                    self.roar_damage_bp,
                    self.roar_turns,
                );
            }
        }
        if self.eidolon >= 2 {
            battle.advance_ally(owner, owner, 10_000);
        }
        battle.advance_ally(owner, sprite, 10_000);
    }

    fn hp_factor(&self, battle: &Battle<'_>, target: usize) -> u32 {
        if self.eidolon == 0 {
            return 10_000;
        }
        let (hp, max_hp) = battle.hp(target);
        if hp <= percentage_of(max_hp, 5_000) {
            14_000
        } else if hp <= percentage_of(max_hp, 8_000) {
            12_000
        } else {
            10_000
        }
    }

    fn dragon_hit(
        &self,
        battle: &mut Battle<'_>,
        sprite: usize,
        target: usize,
        multiplier_bp: u16,
        toughness: u16,
    ) {
        let multiplier_bp = percentage_of(u32::from(multiplier_bp), self.hp_factor(battle, target));
        battle.memosprite_hp_hit(
            sprite,
            target,
            QUANTUM,
            multiplier_bp,
            toughness,
            Action::Skill,
        );
    }

    fn sweep(&self, battle: &mut Battle<'_>, sprite: usize, multiplier_bp: u16) {
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                self.dragon_hit(battle, sprite, target, multiplier_bp, 10);
            }
        }
    }

    fn detonate(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        let multiplier = u32::from(self.detonation_bp)
            + battle.units[owner].castorice_state.cyrene_detonation_bp;
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        for _ in 0..self.detonation_hits + 3 * u32::from(self.eidolon == 6) {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.memosprite_hp_hit(
                sprite,
                target,
                QUANTUM,
                percentage_of(multiplier, self.hp_factor(battle, target)),
                5,
                Action::Skill,
            );
        }
        battle.notify_attack(sprite, Action::Skill, None);
        let healing = percentage_of(battle.effective_max_hp(owner), self.detonation_heal_bp)
            .saturating_add(self.detonation_flat_heal);
        // Healing resolves before departure, so it contributes no Newbud.
        battle.units[owner].castorice_state.dragon_exhausted = true;
        for target in battle.party_units() {
            if target != sprite && battle.is_targetable(target) {
                battle.heal_unit(sprite, target, healing);
            }
        }
        battle.remove_memosprite_of(owner);
        battle.resolve_follow_ups();
    }

    fn breathe(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        let max_hp = battle.effective_max_hp(sprite);
        let cost = percentage_of(max_hp, self.breath_cost_bp);
        let lethal_threshold = percentage_of(max_hp, self.breath_lethal_threshold_bp);
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        let mut detonate = false;
        // Notify attack-based effects only once, after the entire breath chain.
        loop {
            let state = &mut battle.units[owner].castorice_state;
            let tier = state.breath_tier;
            state.breath_tier = (tier + 1).min(self.breath_bp.len() - 1);
            state.breath_stacks = (state.breath_stacks + u32::from(self.west_wind)).min(6);
            if state.ardent_will > 0 {
                state.ardent_will -= 1;
            } else if battle.units[sprite].hp <= lethal_threshold {
                let hp = battle.units[sprite].hp;
                battle.spend_hp(sprite, hp.saturating_sub(1), 1);
                detonate = true;
            } else {
                battle.spend_hp(sprite, cost, 1);
            }
            self.sweep(battle, sprite, self.breath_bp[tier]);
            if detonate || !battle.enemies().any(|target| battle.is_targetable(target)) {
                break;
            }
        }
        battle.notify_attack(sprite, Action::Skill, None);
        if detonate {
            self.detonate(battle, owner, sprite);
        } else {
            if self.inverted_torch && !battle.enemies().any(|target| battle.is_targetable(target)) {
                battle.apply_ally_buff(owner, sprite, AllyBuffKind::SpeedPercent, 10_000, 1);
            }
            battle.resolve_follow_ups();
        }
    }

    fn take_party_hp(&self, battle: &mut Battle<'_>, owner: usize, cost_bp: u32) {
        let dragon = battle.memosprite(owner);
        for target in battle.party_units() {
            if Some(target) != dragon && battle.is_targetable(target) {
                let amount = percentage_of(battle.units[target].hp, cost_bp);
                battle.spend_ally_hp(owner, target, amount, 1);
            }
        }
    }
}

impl Mechanics for Castorice {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        Some(MemospriteProfile {
            name: DRAGON,
            stats: Stats {
                hp: self.dragon_hp,
                base_hp: self.dragon_hp,
                base_speed_micros: self.dragon_speed,
                flat_speed_micros: 0,
                base_aggro: 100,
                ..master
            },
        })
    }

    fn memosprite_is_backup(&self) -> bool {
        true
    }

    fn memosprite_uses_owner_max_hp(&self) -> bool {
        true
    }

    fn memosprite_ignores_weakness(&self) -> bool {
        self.eidolon == 6
    }

    fn memosprite_resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        self.resistance_penetration_bp(battle, owner, element)
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        Self::set_newbud(battle, owner, STARTING_NEWBUD);
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, owner: usize) -> Option<bool> {
        Some(
            battle.units[owner].castorice_state.newbud >= NEWBUD_CAP
                && battle.active_memosprite(owner).is_none(),
        )
    }

    fn activate_special_ultimate(&self, battle: &mut Battle<'_>, owner: usize) {
        Self::gain_newbud(battle, owner, NEWBUD_CAP);
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Skill && battle.active_memosprite(owner).is_some())
            .then_some(&self.joint)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let in_joint = battle.active_memosprite(owner).is_some();
                let cost = if in_joint {
                    self.joint_cost_bp
                } else {
                    self.cost_bp
                };
                self.take_party_hp(battle, owner, cost);
                if in_joint
                    && std::mem::take(&mut battle.units[owner].castorice_state.joint_grants_newbud)
                {
                    Self::gain_newbud(battle, owner, STARTING_NEWBUD);
                }
            }
            Action::Ultimate => {
                let overflow = battle.units[owner]
                    .castorice_state
                    .newbud
                    .saturating_sub(NEWBUD_CAP);
                let bonus = super::cyrene::active_ode_parameters(battle, owner, FORM).map_or(
                    0,
                    |(_, values)| {
                        let enemies = battle
                            .enemies()
                            .filter(|&enemy| battle.is_targetable(enemy))
                            .count();
                        let overflow_bp = u64::from(overflow) * 100 * u64::from(values[0])
                            / u64::from(NEWBUD_CAP);
                        overflow_bp as u32 + values[1] * u32::from(enemies <= values[2] as usize)
                    },
                );
                battle.units[owner].castorice_state.cyrene_detonation_bp = bonus;
                Self::set_newbud(battle, owner, 0);
                self.summon(battle, owner);
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
        if action == Action::Skill
            && let Some(sprite) = battle.active_memosprite(owner)
        {
            battle.notify_attack(owner, Action::Skill, None);
            battle.begin_attack();
            self.sweep(battle, sprite, self.joint_dragon_bp);
        }
    }

    fn attack_source(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _: crate::fixture::Ability<'_>,
    ) -> usize {
        if action == Action::Skill {
            battle.active_memosprite(owner).unwrap_or(owner)
        } else {
            owner
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        let low_hp =
            battle.units[sprite].hp <= percentage_of(battle.effective_max_hp(sprite), 2_500);
        let another_wave = battle.wave_index + 1 < battle.plan.wave_ends.len();
        let turns_alive = battle.units[sprite].normal_turns
            - battle.units[owner].castorice_state.dragon_turn_at_summon;
        if another_wave && low_hp && turns_alive < self.dragon_lifetime_turns {
            battle.begin_attack();
            battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
            self.sweep(battle, sprite, self.claw_bp);
            battle.notify_attack(sprite, Action::Skill, None);
            battle.resolve_follow_ups();
        } else {
            self.breathe(battle, owner, sprite);
        }
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, extra: bool) {
        if extra {
            return;
        }
        let normal_turns = battle.units[actor].normal_turns;
        if actor == owner {
            let state = &mut battle.units[owner].castorice_state;
            if normal_turns >= state.talent_until {
                state.talent_stacks = 0;
            }
        } else if Some(actor) == battle.active_memosprite(owner) {
            let state = &mut battle.units[owner].castorice_state;
            state.breath_stacks = 0;
            if normal_turns - state.dragon_turn_at_summon >= self.dragon_lifetime_turns {
                self.detonate(battle, owner, actor);
            }
        }
    }

    fn on_party_hp_lost(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        amount: u32,
        _: Option<u32>,
    ) {
        let expires_after_turn = battle.units[owner].normal_turns + self.talent_turns;
        let state = &mut battle.units[owner].castorice_state;
        state.talent_stacks = (state.talent_stacks + 1).min(self.talent_max_stacks);
        state.talent_until = expires_after_turn;
        if Some(target) != battle.memosprite(owner) {
            Self::convert_to_newbud_or_dragon_hp(battle, owner, amount);
        }
    }

    fn absorb_fatal_hp_overflow(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        overflow: u32,
    ) -> bool {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return false;
        };
        if target == sprite || battle.units[owner].castorice_state.dragon_exhausted {
            return false;
        }
        let cost = overflow.saturating_mul(self.overflow_ratio);
        battle.units[owner].castorice_state.dragon_exhausted = cost >= battle.units[sprite].hp;
        battle.spend_ally_hp(owner, sprite, cost, 1);
        true
    }

    fn on_party_healed(&self, battle: &mut Battle<'_>, owner: usize, target: usize, amount: u32) {
        if !self.dark_tide || Some(target) == battle.memosprite(owner) {
            return;
        }
        let spent = battle.units[owner]
            .castorice_state
            .heal_conversion
            .entry(target)
            .or_default();
        let amount = amount.min(HEAL_CONVERSION_CAP.saturating_sub(*spent));
        *spent += amount;
        if amount > 0 {
            Self::convert_to_newbud_or_dragon_hp(battle, owner, amount);
        }
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if battle.units[owner].castorice_state.dragon_exhausted
            && let Some(sprite) = battle.active_memosprite(owner)
        {
            self.detonate(battle, owner, sprite);
        }
        battle.units[owner].castorice_state.heal_conversion.clear();
        let targets = std::mem::take(&mut battle.units[owner].castorice_state.mooncocoon_targets);
        for (target, attacker) in targets {
            if battle.units[target].hp == 1 && battle.shield_amount(target) == 0 {
                battle.confirm_defeat(attacker, target);
            }
        }
    }

    fn prevent_team_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
    ) -> bool {
        let state = &mut battle.units[owner].castorice_state;
        if state.mooncocoon_used && state.mooncocoon_targets.is_empty() {
            return false;
        }
        state.mooncocoon_used = true;
        if !state
            .mooncocoon_targets
            .iter()
            .any(|&(unit, _)| unit == target)
        {
            state.mooncocoon_targets.push((target, attacker));
            battle.emit(EventKind::TriggeredSkill, Some(owner), Some(target));
        }
        battle.units[target].hp = 1;
        true
    }

    fn ally_incoming_healing_bonus_bp(&self, _: &Battle<'_>, _: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 4)
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        let (hp, max_hp) = battle.hp(owner);
        4_000 * u32::from(self.inverted_torch && hp >= max_hp.div_ceil(2))
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _: Element,
    ) -> u32 {
        let own_dragon = Some(target) == battle.active_memosprite(owner);
        if target != owner && !own_dragon {
            return 0;
        }
        let state = &battle.units[owner].castorice_state;
        let talent = self.talent_bp * state.talent_stacks;
        talent + 3_000 * state.breath_stacks * u32::from(own_dragon)
    }

    fn elemental_resistance_down_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Element,
    ) -> u32 {
        self.territory_resistance_bp * u32::from(battle.active_memosprite(owner).is_some())
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, element: Element) -> u32 {
        2_000 * u32::from(self.eidolon == 6 && element == QUANTUM)
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
        multiplier_bp: u32,
    ) -> u32 {
        if action == Some(Action::Skill)
            && kind == DamageKind::Direct
            && battle.active_memosprite(owner).is_some()
        {
            percentage_of(multiplier_bp, self.hp_factor(battle, target))
        } else {
            multiplier_bp
        }
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Ultimate).then_some(0)
    }
}
