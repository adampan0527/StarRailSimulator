use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, EventKind,
    fixture::{Ability, Effect, Element, Stats, Target},
};

use super::{
    super::{Battle, SummonTurn, support::percentage_of},
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "trailblazer.remembrance";
const BASIC: &str = "skill.800701";
const SKILL: &str = "skill.800702";
const ULTIMATE: &str = "skill.800703";
const TALENT: &str = "skill.800704";
const JOINT_ATTACK: &str = "skill.800708";
const MEM_ATTACK: &str = "skill.1800701";
const MEM_TALENT: &str = "skill.1800703";
const MEM_ARRIVAL: &str = "skill.1800705";
const MEM_DEPARTURE: &str = "skill.1800706";
const MEM_SUPPORT: &str = "skill.1800707";
const RHAPSODE: &str = "trace.8007101";
const PARABLE: &str = "trace.8007102";
const MAGNETS: &str = "trace.8007103";
const EPIC_TRACE: &str = "trace.8007501";

const MEM: &str = "mem";
const ICE: Element = Element::Ice;
const FULL_CHARGE: u8 = 100;
const EPIC_CAP: u8 = 2;
const MEM_ACTION_ENERGY_MILLI: u32 = 10_000;

#[derive(Clone, Copy)]
struct Support {
    owner: usize,
    expires_after_turn: u32,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct RemembranceTrailblazerState {
    charge: u8,
    energy_remainder_milli: u32,
    summoned_once: bool,
    epic: u8,
    other_sprite_energy_turn: Option<u32>,
    support: Option<Support>,
}

struct RemembranceTrailblazer {
    eidolon: u8,
    rhapsode: bool,
    parable: bool,
    magnets: bool,
    epic_trace: bool,
    mem_hp_ratio_bp: u32,
    mem_hp_base: u32,
    mem_speed_micros: u64,
    energy_per_charge_milli: u32,
    skill_heal_bp: u32,
    skill_charge: u8,
    ultimate_damage_bp: u16,
    ultimate_charge: u8,
    joint_basic: OfficialAbility,
    joint_owner_bp: u16,
    joint_mem_bp: u16,
    joint_charge: u8,
    bounce_bp: u16,
    bounce_count: u8,
    sweep_bp: u16,
    aura_flat_bp: u32,
    aura_ratio_bp: u32,
    arrival_charge: u8,
    departure_advance_bp: u16,
    support_bp: u32,
    support_turns: u32,
    support_advance_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 160
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != ICE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }

    // Both source avatars use the canonical 8007 skill IDs and share the 18007 sprite.
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 4)?;
    let joint = input.parameters(JOINT_ATTACK, 3)?;
    let attack = input.parameters(MEM_ATTACK, 3)?;
    let aura = input.parameters(MEM_TALENT, 2)?;
    let arrival = input.parameters(MEM_ARRIVAL, 1)?;
    let departure = input.parameters(MEM_DEPARTURE, 1)?;
    let support = input.parameters(MEM_SUPPORT, 3)?;
    if talent[0] != 130.0
        || talent[2] != 10.0
        || !(400.0..=800.0).contains(&talent[3])
        || talent[3].fract() != 0.0
        || attack[1] != 4.0
        || support[1] != 3.0
        || support[2] != 1.0
    {
        return Err(invalid("memosprite constants"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 160_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: ICE,
                multiplier_bp: ratio_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: area_ability(5_000, SkillPoints::Unchanged),
        mechanics: Arc::new(RemembranceTrailblazer {
            eidolon: input.eidolon,
            rhapsode: has_trace(RHAPSODE),
            parable: has_trace(PARABLE),
            magnets: has_trace(MAGNETS),
            epic_trace: has_trace(EPIC_TRACE),
            mem_hp_ratio_bp: u32::from(ratio_bp(TALENT, talent[1])?),
            mem_hp_base: talent[3] as u32,
            mem_speed_micros: (talent[0] * 1_000_000.0) as u64,
            energy_per_charge_milli: (talent[2] * 1_000.0) as u32,
            skill_heal_bp: u32::from(ratio_bp(SKILL, skill[0])?),
            skill_charge: charge_percent(SKILL, skill[1])?,
            ultimate_damage_bp: ratio_bp(ULTIMATE, ultimate[0])?,
            ultimate_charge: charge_percent(ULTIMATE, ultimate[1])?,
            joint_basic: area_ability(30_000, SkillPoints::Generate),
            joint_owner_bp: ratio_bp(JOINT_ATTACK, joint[0])?,
            joint_mem_bp: ratio_bp(JOINT_ATTACK, joint[1])?,
            joint_charge: charge_percent(JOINT_ATTACK, joint[2])?,
            bounce_bp: ratio_bp(MEM_ATTACK, attack[0])?,
            bounce_count: attack[1] as u8,
            sweep_bp: ratio_bp(MEM_ATTACK, attack[2])?,
            aura_ratio_bp: u32::from(ratio_bp(MEM_TALENT, aura[0])?),
            aura_flat_bp: u32::from(ratio_bp(MEM_TALENT, aura[1])?),
            arrival_charge: charge_percent(MEM_ARRIVAL, arrival[0])?,
            departure_advance_bp: ratio_bp(MEM_DEPARTURE, departure[0])?,
            support_bp: u32::from(ratio_bp(MEM_SUPPORT, support[0])?),
            support_turns: support[1] as u32,
            support_advance_bp: ratio_bp(MEM_SUPPORT, support[2])?,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn charge_percent(id: &str, value: f64) -> Result<u8, BattleError> {
    let percent = value * 100.0;
    if !(1.0..=100.0).contains(&percent) || (percent - percent.round()).abs() > 1e-8 {
        return Err(invalid(id));
    }
    Ok(percent.round() as u8)
}

fn area_ability(energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    // One player action can contain separate attacks from the master and sprite.
    OfficialAbility {
        target: Target::SelfOnly,
        energy_milli,
        effects: vec![Effect::DirectedAreaAttack],
        skill_points,
    }
}

fn support_is_active(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
    battle.units[target]
        .remembrance_trailblazer_state
        .support
        .is_some_and(|support| {
            support.owner == owner
                && battle.is_targetable(target)
                && battle.units[target].finished_turns < support.expires_after_turn
        })
}

impl RemembranceTrailblazer {
    fn supported(&self, battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        if support_is_active(battle, owner, target) {
            return true;
        }
        self.eidolon >= 1
            && battle
                .memosprite_owner(target)
                .or_else(|| battle.active_memosprite(target))
                .is_some_and(|partner| support_is_active(battle, owner, partner))
    }

    fn set_charge(&self, battle: &mut Battle<'_>, owner: usize, charge: u8) {
        let charge = charge.min(FULL_CHARGE);
        let state = &mut battle.units[owner].remembrance_trailblazer_state;
        let before = std::mem::replace(&mut state.charge, charge);
        if before != charge {
            let sprite = battle.memosprite(owner);
            battle.emit(
                EventKind::MemospriteCharge {
                    before,
                    after: charge,
                },
                Some(owner),
                sprite,
            );
        }
        if before < FULL_CHARGE
            && charge == FULL_CHARGE
            && let Some(summon) = &mut battle.units[owner].summon_turn
        {
            summon.advance(10_000);
        }
    }

    fn add_charge(&self, battle: &mut Battle<'_>, owner: usize, amount: u8) {
        if amount > 0 && battle.active_memosprite(owner).is_some() {
            let charge = battle.units[owner].remembrance_trailblazer_state.charge;
            self.set_charge(battle, owner, charge.saturating_add(amount));
        }
    }

    fn set_epic(&self, battle: &mut Battle<'_>, owner: usize, epic: u8) {
        let state = &mut battle.units[owner].remembrance_trailblazer_state;
        let before = std::mem::replace(&mut state.epic, epic);
        if before != epic {
            battle.emit(
                EventKind::BasicEnhancement {
                    before,
                    after: epic,
                },
                Some(owner),
                None,
            );
        }
    }

    fn summon(&self, battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        let sprite = battle.memosprite(owner)?;
        if !battle.is_targetable(sprite) {
            let state = &mut battle.units[owner].remembrance_trailblazer_state;
            let first = !std::mem::replace(&mut state.summoned_once, true);
            let hp = percentage_of(battle.effective_max_hp(owner), self.mem_hp_ratio_bp)
                .saturating_add(self.mem_hp_base);
            battle.units[sprite].remembrance_trailblazer_state.support = None;
            battle.units[sprite].memosprite_max_hp = Some(hp);
            battle.units[sprite].memosprite_generation += 1;
            battle.units[sprite].hp = battle.effective_max_hp(sprite);
            battle.units[owner].summon_turn = Some(SummonTurn::new(MEM, self.mem_speed_micros));
            battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
            self.set_charge(
                battle,
                owner,
                self.arrival_charge + 40 * u8::from(first && self.rhapsode),
            );
        }
        Some(sprite)
    }

    fn grant_support(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize, target: usize) {
        let expires_after_turn = battle.units[target].normal_turns + self.support_turns;
        battle.units[target].remembrance_trailblazer_state.support = Some(Support {
            owner,
            expires_after_turn,
        });
        battle.emit(EventKind::TriggeredSkill, Some(sprite), Some(target));
        if target != sprite {
            battle.advance_ally(sprite, target, self.support_advance_bp);
        }
    }

    fn attack_mem(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        self.add_charge(battle, owner, 5 * u8::from(self.parable));
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        for _ in 0..self.bounce_count {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.memosprite_hit(sprite, target, ICE, self.bounce_bp, 5, Action::Skill);
        }
        for target in battle.enemies() {
            battle.memosprite_hit(sprite, target, ICE, self.sweep_bp, 10, Action::Skill);
        }
        battle.notify_attack(sprite, Action::Skill, None);
        battle.resolve_follow_ups();
    }
}

impl Mechanics for RemembranceTrailblazer {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        let hp = percentage_of(master.hp, self.mem_hp_ratio_bp).saturating_add(self.mem_hp_base);
        Some(MemospriteProfile {
            name: MEM,
            stats: Stats {
                hp,
                base_hp: hp,
                base_aggro: 100,
                base_speed_micros: self.mem_speed_micros,
                flat_speed_micros: 0,
                ..master
            },
        })
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.rhapsode {
            battle.advance(owner, 3_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            if let Some(sprite) = battle.active_memosprite(owner) {
                let amount = percentage_of(battle.effective_max_hp(sprite), self.skill_heal_bp);
                battle.heal_unit(owner, sprite, amount);
                self.add_charge(battle, owner, self.skill_charge);
            } else {
                self.summon(battle, owner);
            }
        }
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic
            && battle.active_memosprite(owner).is_some()
            && battle.units[owner].remembrance_trailblazer_state.epic > 0)
            .then_some(&self.joint_basic)
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        match battle.current_action() {
            Some(Action::Ultimate) => {
                let Some(sprite) = self.summon(battle, owner) else {
                    return;
                };
                self.add_charge(battle, owner, self.ultimate_charge);
                for target in battle.enemies() {
                    battle.memosprite_hit(
                        sprite,
                        target,
                        ICE,
                        self.ultimate_damage_bp,
                        20,
                        Action::Ultimate,
                    );
                }
                if self.epic_trace {
                    let epic = battle.units[owner].remembrance_trailblazer_state.epic;
                    self.set_epic(battle, owner, (epic + 1).min(EPIC_CAP));
                }
            }
            Some(Action::Basic) => {
                let Some(sprite) = battle.active_memosprite(owner) else {
                    return;
                };
                let epic = battle.units[owner].remembrance_trailblazer_state.epic;
                self.set_epic(battle, owner, epic - 1);
                battle.cleanse_crowd_control(owner, sprite);
                for target in battle.enemies() {
                    // The joint attack shares each enemy's ten points of toughness damage.
                    battle.attack_hit(owner, target, ICE, self.joint_owner_bp, 5);
                }
                battle.notify_attack(owner, Action::Basic, None);
                battle.begin_attack();
                for target in battle.enemies() {
                    battle.memosprite_hit(sprite, target, ICE, self.joint_mem_bp, 5, Action::Basic);
                }
                self.add_charge(battle, owner, self.joint_charge);
            }
            _ => {}
        }
    }

    fn attack_source(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        ability: Ability<'_>,
    ) -> usize {
        if action == Action::Ultimate
            || (action == Action::Basic && matches!(ability.target, Target::SelfOnly))
        {
            return battle.active_memosprite(actor).unwrap_or(actor);
        }
        actor
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        if battle.units[owner].remembrance_trailblazer_state.charge == FULL_CHARGE {
            self.set_charge(battle, owner, 0);
            let target = battle
                .party_units()
                .filter(|&target| battle.is_targetable(target))
                .max_by_key(|&target| (battle.effective_attack(target), Reverse(target)));
            if let Some(target) = target {
                self.grant_support(battle, owner, sprite, target);
            }
        } else {
            self.attack_mem(battle, owner, sprite);
        }
        battle.gain_energy(sprite, owner, MEM_ACTION_ENERGY_MILLI);
    }

    fn on_memosprite_departed(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        self.set_charge(battle, owner, 0);
        battle.units[owner]
            .remembrance_trailblazer_state
            .energy_remainder_milli = 0;
        battle.units[sprite].remembrance_trailblazer_state.support = None;
        if battle.is_targetable(owner) {
            battle.advance_ally(sprite, owner, self.departure_advance_bp);
        }
    }

    fn on_ally_energy_restored(&self, battle: &mut Battle<'_>, owner: usize, amount_milli: u32) {
        if battle.units[owner].summon_turn.is_none() {
            return;
        }
        let state = &mut battle.units[owner].remembrance_trailblazer_state;
        if state.charge == FULL_CHARGE {
            return;
        }
        let total = state.energy_remainder_milli.saturating_add(amount_milli);
        state.energy_remainder_milli = total % self.energy_per_charge_milli;
        let charge = (total / self.energy_per_charge_milli).min(u32::from(FULL_CHARGE)) as u8;
        self.add_charge(battle, owner, charge);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: Action,
    ) {
        if self.eidolon >= 4 && battle.plan.units[actor].kit.max_energy_milli() == 0 {
            self.add_charge(battle, owner, 3);
        }
    }

    fn on_memosprite_turn_finished(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        if self.eidolon < 2 || battle.memosprite_owner(sprite) == Some(owner) {
            return;
        }
        let turn = battle.units[owner].normal_turns;
        let state = &mut battle.units[owner].remembrance_trailblazer_state;
        if state.other_sprite_energy_turn != Some(turn) {
            state.other_sprite_energy_turn = Some(turn);
            battle.gain_energy(sprite, owner, 8_000);
        }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        battle.active_memosprite(owner).map_or(0, |sprite| {
            let crit = battle.effective_crit_damage_bp_without(sprite, Some(owner));
            self.aura_flat_bp + percentage_of(crit, self.aura_ratio_bp)
        })
    }

    fn ally_crit_rate_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        1_000 * u32::from(self.eidolon >= 1 && self.supported(battle, owner, target))
    }

    fn memosprite_true_damage_bp(&self, battle: &Battle<'_>, owner: usize, source: usize) -> u32 {
        if !self.supported(battle, owner, source) {
            return 0;
        }
        let energy_owner = battle.memosprite_owner(source).unwrap_or(source);
        let max_energy = battle.plan.units[energy_owner].kit.max_energy_milli();
        let magnets_bonus = if self.magnets {
            200 * (max_energy.saturating_sub(100_000) / 10_000).min(10)
        } else {
            0
        };
        let eidolon_bonus = 600 * u32::from(self.eidolon >= 4 && max_energy == 0);
        self.support_bp + magnets_bonus + eidolon_bonus
    }

    fn memosprite_guarantees_critical(&self, origin: Option<Action>) -> bool {
        self.eidolon >= 6 && origin == Some(Action::Ultimate)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Skill && battle.active_memosprite(owner).is_none()).then_some(1)
    }
}
