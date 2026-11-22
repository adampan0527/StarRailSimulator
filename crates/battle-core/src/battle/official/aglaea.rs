use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Ability, Effect, Element, Stats, Target},
};

use super::{
    super::{Battle, SummonTurn, combat_status::DebuffKind, resolve::DamageTargetAura},
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1402";
const BASIC: &str = "skill.140201";
const SKILL: &str = "skill.140202";
const ULTIMATE: &str = "skill.140203";
const TALENT: &str = "skill.140204";
const JOINT_BASIC: &str = "skill.140208";
const GARMENTMAKER_SKILL: &str = "skill.1140201";
const GARMENTMAKER_TALENT: &str = "skill.1140203";
const GARMENTMAKER_ARRIVAL: &str = "skill.1140205";
const GARMENTMAKER_DEPARTURE: &str = "skill.1140206";
const MYOPICS_DOOM: &str = "trace.1402101";
const LAST_THREAD: &str = "trace.1402102";
const SPEEDING_SOL: &str = "trace.1402103";

const LIGHTNING: Element = Element::Thunder;
const GARMENTMAKER: &str = "garmentmaker";
const COUNTDOWN: &str = "supreme-stance";
const MICROS_PER_UNIT: u64 = 1_000_000;
const ENERGY_CAPACITY: u32 = 350;
const COUNTDOWN_SPEED_MICROS: u64 = 100 * MICROS_PER_UNIT;

#[derive(Clone, Default)]
pub(in crate::battle) struct AglaeaState {
    seam_target: Option<usize>,
    speed_stacks: u8,
    defense_stacks: u8,
}

struct Aglaea {
    eidolon: u8,
    myopics_doom: bool,
    last_thread: bool,
    speeding_sol: bool,
    healing_ratio_micros: u64,
    stance_speed_ratio_micros_per_stack: u64,
    seam_damage_bp: u16,
    sprite_speed_ratio_bp: u64,
    sprite_hp_ratio_bp: u32,
    sprite_hp_base_halves: u16,
    joint_basic: OfficialAbility,
    joint_owner_bp: [u16; 2],
    joint_sprite_bp: [u16; 2],
    sprite_attack_bp: [u16; 2],
    sprite_speed_micros_per_stack: u64,
    sprite_speed_stack_cap: u8,
    sprite_arrival_advance_bp: u16,
    sprite_departure_energy_milli: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != LIGHTNING
        || input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, combat type or eidolon"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 6)?;
    let joint = input.parameters(JOINT_BASIC, 4)?;
    let sprite_skill = input.parameters(GARMENTMAKER_SKILL, 3)?;
    let sprite_talent = input.parameters(GARMENTMAKER_TALENT, 3)?;
    let sprite_arrival = input.parameters(GARMENTMAKER_ARRIVAL, 1)?;
    let sprite_departure = input.parameters(GARMENTMAKER_DEPARTURE, 1)?;
    if ultimate[3] != 100.0
        || talent[3] != 0.35
        || !(180.0..=990.0).contains(&talent[5])
        || (talent[5] * 2.0).fract() != 0.0
        || !(44.0..=63.8).contains(&sprite_talent[0])
        || sprite_talent[2] != 6.0
        || sprite_arrival[0] != 1.0
        || sprite_departure[0] != 20.0
    {
        return Err(invalid("memosprite constants"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: LIGHTNING,
                multiplier_bp: ratio_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 20_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Aglaea {
            eidolon: input.eidolon,
            myopics_doom: has_trace(MYOPICS_DOOM),
            last_thread: has_trace(LAST_THREAD),
            speeding_sol: has_trace(SPEEDING_SOL),
            healing_ratio_micros: ratio_micros(SKILL, skill[0])?,
            stance_speed_ratio_micros_per_stack: ratio_micros(ULTIMATE, ultimate[0])?,
            seam_damage_bp: ratio_bp(TALENT, talent[0])?,
            sprite_speed_ratio_bp: u64::from(ratio_bp(TALENT, talent[3])?),
            sprite_hp_ratio_bp: u32::from(ratio_bp(TALENT, talent[4])?),
            sprite_hp_base_halves: (talent[5] * 2.0) as u16,
            joint_basic: OfficialAbility {
                target: Target::Enemy,
                energy_milli: 20_000,
                effects: vec![Effect::DirectedAttack { adjacent: true }],
                skill_points: SkillPoints::Unchanged,
            },
            joint_owner_bp: [
                ratio_bp(JOINT_BASIC, joint[0])?,
                ratio_bp(JOINT_BASIC, joint[1])?,
            ],
            joint_sprite_bp: [
                ratio_bp(JOINT_BASIC, joint[2])?,
                ratio_bp(JOINT_BASIC, joint[3])?,
            ],
            sprite_attack_bp: [
                ratio_bp(GARMENTMAKER_SKILL, sprite_skill[0])?,
                ratio_bp(GARMENTMAKER_SKILL, sprite_skill[1])?,
            ],
            sprite_speed_micros_per_stack: (sprite_talent[0] * MICROS_PER_UNIT as f64).round()
                as u64,
            sprite_speed_stack_cap: sprite_talent[2] as u8 + u8::from(input.eidolon >= 4),
            sprite_arrival_advance_bp: ratio_bp(GARMENTMAKER_ARRIVAL, sprite_arrival[0])?,
            sprite_departure_energy_milli: (sprite_departure[0] * 1_000.0) as u32,
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

fn ratio_micros(id: &str, value: f64) -> Result<u64, BattleError> {
    let scaled = (value * MICROS_PER_UNIT as f64).round();
    if !(1.0..=MICROS_PER_UNIT as f64).contains(&scaled) {
        return Err(invalid(id));
    }
    Ok(scaled as u64)
}

fn in_stance(battle: &Battle<'_>, owner: usize) -> bool {
    battle.units[owner].countdown.is_some()
}

fn is_pair_member(battle: &Battle<'_>, owner: usize, actor: usize) -> bool {
    actor == owner || battle.memosprite_owner(actor) == Some(owner)
}

impl Aglaea {
    fn garmentmaker_hp(&self, master_max_hp: u32) -> u32 {
        // The flat bonus can contain half a point at Talent level 2 or 4.
        let hp_scaled = u64::from(master_max_hp) * u64::from(self.sprite_hp_ratio_bp)
            + u64::from(self.sprite_hp_base_halves) * 5_000;
        (hp_scaled / 10_000).min(u64::from(u32::MAX)) as u32
    }

    fn summon(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.memosprite(owner) else {
            return;
        };
        let hp = self.garmentmaker_hp(battle.effective_max_hp(owner));
        let speed = battle.speed_micros(owner) * self.sprite_speed_ratio_bp / 10_000;
        {
            let state = &mut battle.units[sprite];
            state.memosprite_max_hp = Some(hp);
            state.memosprite_generation += 1;
            state.hp = hp;
        }
        battle.units[owner].summon_turn = Some(SummonTurn::new(GARMENTMAKER, speed));
        battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
        battle.advance_ally(owner, sprite, self.sprite_arrival_advance_bp);
    }

    fn enter_stance(&self, battle: &mut Battle<'_>, owner: usize) {
        if let Some(sprite) = battle.active_memosprite(owner) {
            let max_hp = battle.effective_max_hp(sprite);
            battle.restore_hp_unmodified(owner, sprite, max_hp);
        } else {
            self.summon(battle, owner);
        }
        let previous = battle.units[owner]
            .countdown
            .replace(SummonTurn::with_action_advance(
                COUNTDOWN,
                COUNTDOWN_SPEED_MICROS,
                false,
            ));
        if previous.is_none() {
            if let Some(sprite) = battle.active_memosprite(owner) {
                battle.cleanse_crowd_control(owner, sprite);
            }
            battle.emit(
                EventKind::StatusApplied(StatusKind::SupremeStance),
                Some(owner),
                Some(owner),
            );
        }
        battle.advance_ally_immediate(owner, owner);
    }

    fn mark_seam(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let previous = battle.units[owner].aglaea_state.seam_target.replace(target);
        if let Some(old) = previous.filter(|&old| old != target) {
            battle.remove_debuff(owner, old, DebuffKind::SeamStitch);
        }
        battle.apply_permanent_debuff(
            owner,
            target,
            DebuffKind::SeamStitch,
            1_500 * u32::from(self.eidolon >= 1),
        );
    }

    fn add_speed_stack(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        let stacks = &mut battle.units[owner].aglaea_state.speed_stacks;
        if *stacks < self.sprite_speed_stack_cap {
            *stacks += 1;
            battle.emit(
                EventKind::StatusApplied(StatusKind::SpeedUp),
                Some(sprite),
                Some(sprite),
            );
        }
    }

    fn joint_bonus(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.eidolon < 6 || origin != Some(Action::Basic) || kind != DamageKind::Direct {
            return 0;
        }
        let owner = battle.memosprite_owner(actor).unwrap_or(actor);
        if !in_stance(battle, owner) {
            return 0;
        }
        let speed = battle.speed_micros(actor);
        if speed > 320 * MICROS_PER_UNIT {
            6_000
        } else if speed > 240 * MICROS_PER_UNIT {
            3_000
        } else if speed > 160 * MICROS_PER_UNIT {
            1_000
        } else {
            0
        }
    }
}

impl Mechanics for Aglaea {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        let hp = self.garmentmaker_hp(master.hp);
        Some(MemospriteProfile {
            name: GARMENTMAKER,
            stats: Stats {
                hp,
                base_hp: hp,
                base_aggro: 125,
                base_speed_micros: master.base_speed_micros * self.sprite_speed_ratio_bp / 10_000,
                flat_speed_micros: master.flat_speed_micros * self.sprite_speed_ratio_bp / 10_000,
                ..master
            },
        })
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.speeding_sol {
            let minimum = ENERGY_CAPACITY * 500;
            let missing = minimum.saturating_sub(battle.units[owner].energy_milli);
            battle.restore_energy_flat(owner, owner, missing);
        }
    }

    fn tracks_active_abilities(&self) -> bool {
        self.eidolon >= 2
    }

    fn on_active_ability(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        let same_pair = is_pair_member(battle, owner, actor);
        let state = &mut battle.units[owner].aglaea_state;
        state.defense_stacks = if same_pair {
            (state.defense_stacks + 1).min(3)
        } else {
            0
        };
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Basic => {
                if battle.active_memosprite(owner).is_some()
                    && let Some(target) = target
                {
                    self.mark_seam(battle, owner, target);
                }
            }
            Action::Skill => {
                if let Some(sprite) = battle.active_memosprite(owner) {
                    let max_hp = u64::from(battle.effective_max_hp(sprite));
                    let healing = max_hp * self.healing_ratio_micros / MICROS_PER_UNIT;
                    battle.heal_unit(owner, sprite, healing as u32);
                } else {
                    self.summon(battle, owner);
                    battle.advance_ally_immediate(owner, owner);
                }
            }
            Action::Ultimate => self.enter_stance(battle, owner),
            Action::EnhancedUltimate | Action::ElationSkill => {}
        }
    }

    fn can_use_skill(&self, battle: &Battle<'_>, owner: usize) -> bool {
        !in_stance(battle, owner)
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && in_stance(battle, owner)).then_some(&self.joint_basic)
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        let neighbors: Vec<_> = battle.adjacent_enemies(target).collect();
        let [main, splash] = self.joint_owner_bp;
        battle.attack_hit(owner, target, LIGHTNING, main, 10);
        for &neighbor in &neighbors {
            battle.attack_hit(owner, neighbor, LIGHTNING, splash, 5);
        }
        battle.notify_attack(owner, Action::Basic, Some(target));
        battle.begin_attack();
        let [main, splash] = self.joint_sprite_bp;
        battle.memosprite_hit(sprite, target, LIGHTNING, main, 10, Action::Basic);
        for neighbor in neighbors {
            battle.memosprite_hit(sprite, neighbor, LIGHTNING, splash, 5, Action::Basic);
        }
    }

    fn attack_source(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Ability<'_>,
    ) -> usize {
        if action == Action::Basic && in_stance(battle, actor) {
            battle.active_memosprite(actor).unwrap_or(actor)
        } else {
            actor
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        if !is_pair_member(battle, owner, attacker) {
            return;
        }
        if let Some(marked) = battle.units[owner].aglaea_state.seam_target
            && battle.attack_targets().contains(&marked)
        {
            if attacker == owner {
                battle.secondary_hit(
                    owner,
                    marked,
                    LIGHTNING,
                    self.seam_damage_bp,
                    DamageKind::Additional,
                    origin,
                );
            }
            if self.eidolon >= 1 {
                battle.gain_energy(attacker, owner, 20_000);
            }
            if attacker == sprite && battle.is_targetable(marked) {
                self.add_speed_stack(battle, owner, sprite);
            }
        }
        if attacker == owner && self.eidolon >= 4 {
            self.add_speed_stack(battle, owner, sprite);
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        let marked = battle.units[owner]
            .aglaea_state
            .seam_target
            .filter(|&unit| battle.enemies().contains(&unit) && battle.is_targetable(unit));
        let Some(target) = marked.or_else(|| battle.random_living_enemy()) else {
            return;
        };
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), Some(target));
        let [main, splash] = self.sprite_attack_bp;
        battle.memosprite_hit(sprite, target, LIGHTNING, main, 10, Action::Skill);
        for neighbor in battle.adjacent_enemies(target) {
            battle.memosprite_hit(sprite, neighbor, LIGHTNING, splash, 5, Action::Skill);
        }
        battle.notify_attack(sprite, Action::Skill, Some(target));
        battle.gain_energy(sprite, owner, 10_000);
        battle.resolve_follow_ups();
    }

    fn on_countdown_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.emit(
            EventKind::StatusExpired(StatusKind::SupremeStance),
            Some(owner),
            Some(owner),
        );
        battle.remove_memosprite_of(owner);
    }

    fn on_memosprite_departed(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        if battle.units[owner].countdown.take().is_some() {
            battle.emit(
                EventKind::StatusExpired(StatusKind::SupremeStance),
                Some(owner),
                Some(owner),
            );
        }
        let state = &mut battle.units[owner].aglaea_state;
        state.speed_stacks = state.speed_stacks.min(u8::from(self.last_thread));
        if battle.is_targetable(owner) {
            battle.gain_energy(sprite, owner, self.sprite_departure_energy_milli);
        }
    }

    fn memosprite_immune_to_enemy_control(&self, battle: &Battle<'_>, owner: usize) -> bool {
        in_stance(battle, owner)
    }

    fn ally_flat_speed_micros(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u64 {
        let stacks = u64::from(battle.units[owner].aglaea_state.speed_stacks);
        if target == owner && in_stance(battle, owner) {
            // The intermediate Ultimate rows include half basis points per stack.
            return battle.plan.units[owner].stats.base_speed_micros
                * self.stance_speed_ratio_micros_per_stack
                * stacks
                / MICROS_PER_UNIT;
        }
        if battle.memosprite_owner(target) == Some(owner) {
            self.sprite_speed_micros_per_stack * stacks
        } else {
            0
        }
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if !self.myopics_doom || !in_stance(battle, owner) || !is_pair_member(battle, owner, target)
        {
            return 0;
        }
        let sprite_speed = battle
            .active_memosprite(owner)
            .map_or(0, |sprite| battle.speed_micros(sprite));
        let owner_speed = battle.speed_micros(owner);
        let bonus = (72 * owner_speed + 36 * sprite_speed) / (10 * MICROS_PER_UNIT);
        bonus.min(u64::from(u32::MAX)) as u32
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        attacker: usize,
    ) -> DamageTargetAura {
        DamageTargetAura {
            defense_down_bp: 1_400
                * u32::from(is_pair_member(battle, owner, attacker))
                * u32::from(battle.units[owner].aglaea_state.defense_stacks),
            ..DamageTargetAura::default()
        }
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        2_000 * u32::from(self.eidolon >= 6 && in_stance(battle, owner) && element == LIGHTNING)
    }

    fn memosprite_resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        self.resistance_penetration_bp(battle, owner, element)
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        self.joint_bonus(battle, actor, action, kind)
    }

    fn memosprite_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        sprite: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        self.joint_bonus(battle, sprite, origin, kind)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill if battle.active_memosprite(owner).is_none() => Some(1),
            _ => None,
        }
    }
}
