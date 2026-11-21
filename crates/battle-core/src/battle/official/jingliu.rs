use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::super::{
    Battle,
    support::{AllyBuffKind, percentage_of},
};
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1212";
const ICE: Element = Element::Ice;
const ENERGY_CAPACITY: u32 = 140;
const TRANSMIGRATION_THRESHOLD: u8 = 2;
const MOONLIGHT_CAP: u8 = 5;
const HEALTH_EVENTS_PER_SYZYGY: u8 = 20;
const E1_CRIT_DAMAGE: &str = "jingliu.moon_crashes_tianguan";

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    enhanced_skill: &'static str,
    deathrealm: &'static str,
    sword_champion: &'static str,
    frost_wraith: &'static str,
}

impl KitIds {
    fn has_skill(self, input: &OfficialCharacter) -> bool {
        [
            self.basic,
            self.skill,
            self.ultimate,
            self.talent,
            self.enhanced_skill,
        ]
        .iter()
        .any(|id| input.skill_parameters.contains_key(*id))
    }
}

const BASE: KitIds = KitIds {
    basic: "skill.121201",
    skill: "skill.121202",
    ultimate: "skill.121203",
    talent: "skill.121204",
    enhanced_skill: "skill.121209",
    deathrealm: "trace.1212101",
    sword_champion: "trace.1212102",
    frost_wraith: "trace.1212103",
};

const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1121201",
    skill: "skill.1121202",
    ultimate: "skill.1121203",
    talent: "skill.1121204",
    enhanced_skill: "skill.1121209",
    deathrealm: "trace.11212101",
    sword_champion: "trace.11212102",
    frost_wraith: "trace.11212103",
};

#[derive(Clone, Default)]
pub(in crate::battle) struct JingliuState {
    syzygy: u8,
    moonlight: u8,
    transformed: bool,
    health_events: u8,
    last_damage_attack: [Option<u32>; 4],
    converted_attack: u32,
    amplified_skill_ready: bool,
    amplified_skill: bool,
    defense_ignore_ready: bool,
    defense_ignore: bool,
    transformed_attack: bool,
}

enum Scaling {
    Attack {
        hp_to_attack_bp: u32,
        attack_cap_bp: u32,
    },
    MaxHp {
        moonlight_crit_damage_bp: u32,
    },
}

struct Jingliu {
    scaling: Scaling,
    eidolon: u8,
    deathrealm: bool,
    sword_champion: bool,
    frost_wraith: bool,
    hp_cost_bp: u32,
    crit_rate_bp: u32,
    enhanced_skill: OfficialAbility,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != ICE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let (novaflare, ids) = match (BASE.has_skill(input), NOVAFLARE.has_skill(input)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic = input.parameters(ids.basic, 1)?;
    let skill = input.parameters(ids.skill, 2)?;
    let ultimate = input.parameters(ids.ultimate, 4)?;
    let enhanced_skill = input.parameters(ids.enhanced_skill, 3)?;
    let talent = input.parameters(ids.talent, if novaflare { 8 } else { 7 })?;
    if skill[1] != 1.0 || ultimate[1] != 1.0 || enhanced_skill[1] != 1.0 {
        return Err(invalid("Syzygy gains or cost"));
    }
    if talent[4] != f64::from(TRANSMIGRATION_THRESHOLD)
        || talent[5] != 1.0
        || (novaflare
            && (talent[3] != f64::from(MOONLIGHT_CAP)
                || talent[7] != f64::from(HEALTH_EVENTS_PER_SYZYGY)))
    {
        return Err(invalid("transmigration and Moonlight thresholds"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let sword_champion = trace(ids.sword_champion);
    let bonus_energy = |energy| {
        if novaflare && sword_champion {
            energy
        } else {
            0
        }
    };
    let scaling = if novaflare {
        Scaling::MaxHp {
            moonlight_crit_damage_bp: u32::from(ratio_bp(ids.talent, talent[2])?),
        }
    } else {
        let eidolon = u32::from(input.eidolon >= 4);
        Scaling::Attack {
            hp_to_attack_bp: u32::from(ratio_bp(ids.talent, talent[2])?) + eidolon * 9_000,
            attack_cap_bp: u32::from(ratio_bp(ids.talent, talent[3])?) + eidolon * 3_000,
        }
    };
    let hp_cost_bp = u32::from(ratio_bp(ids.talent, talent[1])?);
    let crit_rate_bp = u32::from(ratio_bp(ids.talent, talent[6])?);
    if hp_cost_bp > 10_000 || crit_rate_bp > 10_000 {
        return Err(invalid(ids.talent));
    }

    let basic_bp = ratio_bp(ids.basic, basic[0])?;
    let skill_bp = ratio_bp(ids.skill, skill[0])?;
    let ultimate_bp = ratio_bp(ids.ultimate, ultimate[0])?;
    let ultimate_adjacent_bp = ratio_bp(ids.ultimate, ultimate[2])?;
    let enhanced_bp = ratio_bp(ids.enhanced_skill, enhanced_skill[0])?;
    let enhanced_adjacent_bp = ratio_bp(ids.enhanced_skill, enhanced_skill[2])?;
    let hit = |multiplier_bp, toughness| ice_hit(novaflare, multiplier_bp, toughness);
    let blast = |multiplier_bp, toughness| ice_adjacent(novaflare, multiplier_bp, toughness);

    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: attack(vec![hit(basic_bp, 10)], 20_000, SkillPoints::Generate),
        skill: attack(
            vec![hit(skill_bp, 20)],
            20_000 + bonus_energy(15_000),
            SkillPoints::Spend,
        ),
        ultimate: attack(
            vec![hit(ultimate_bp, 20), blast(ultimate_adjacent_bp, 20)],
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Jingliu {
            scaling,
            eidolon: input.eidolon,
            deathrealm: trace(ids.deathrealm),
            sword_champion,
            frost_wraith: trace(ids.frost_wraith),
            hp_cost_bp,
            crit_rate_bp,
            enhanced_skill: attack(
                vec![hit(enhanced_bp, 20), blast(enhanced_adjacent_bp, 10)],
                30_000 + bonus_energy(8_000),
                SkillPoints::Unchanged,
            ),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn ice_hit(max_hp: bool, multiplier_bp: u16, toughness: u16) -> Effect {
    if max_hp {
        Effect::MaxHpHit {
            element: ICE,
            multiplier_bp,
            toughness,
        }
    } else {
        Effect::Hit {
            element: ICE,
            multiplier_bp,
            toughness,
        }
    }
}

fn ice_adjacent(max_hp: bool, multiplier_bp: u16, toughness: u16) -> Effect {
    if max_hp {
        Effect::MaxHpBlastAdjacent {
            element: ICE,
            multiplier_bp,
            toughness,
        }
    } else {
        Effect::BlastAdjacent {
            element: ICE,
            multiplier_bp,
            toughness,
        }
    }
}

impl Jingliu {
    fn novaflare(&self) -> bool {
        matches!(self.scaling, Scaling::MaxHp { .. })
    }

    fn syzygy_cap(&self) -> u8 {
        3 + u8::from(self.novaflare()) + u8::from(self.eidolon >= 6)
    }

    fn gain_syzygy(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].jingliu_state;
        if state.syzygy == self.syzygy_cap() {
            if self.novaflare() && self.frost_wraith {
                state.defense_ignore_ready = true;
            }
            return;
        }

        let before = state.syzygy;
        state.syzygy += 1;
        let enters = !state.transformed && state.syzygy == TRANSMIGRATION_THRESHOLD;
        if enters {
            let e6_stacks = if self.novaflare() { 2 } else { 1 };
            state.transformed = true;
            state.syzygy += u8::from(self.novaflare()) + e6_stacks * u8::from(self.eidolon >= 6);
        }
        let after = state.syzygy;
        battle.emit(
            EventKind::Syzygy { before, after },
            Some(actor),
            Some(actor),
        );
        if enters {
            battle.advance_immediate(actor);
        }
    }

    fn spend_syzygy(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].jingliu_state;
        let before = state.syzygy;
        state.syzygy -= 1;
        let after = state.syzygy;
        let cleared = if after == 0 {
            state.transformed = false;
            std::mem::take(&mut state.moonlight)
        } else {
            0
        };
        battle.emit(
            EventKind::Syzygy { before, after },
            Some(actor),
            Some(actor),
        );
        if cleared > 0 {
            battle.emit(
                EventKind::Moonlight {
                    before: cleared,
                    after: 0,
                },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn consume_teammate_hp(&self, battle: &mut Battle<'_>, actor: usize) {
        let mut consumed = 0;
        for ally in battle.allies() {
            if ally != actor && battle.is_targetable(ally) {
                let (_, max_hp) = battle.hp(ally);
                let cost = percentage_of(max_hp, self.hp_cost_bp);
                consumed += battle.spend_ally_hp(actor, ally, cost, 1);
            }
        }
        if let Scaling::Attack {
            hp_to_attack_bp,
            attack_cap_bp,
        } = self.scaling
        {
            let cap = percentage_of(battle.plan.units[actor].stats.base_attack, attack_cap_bp);
            battle.units[actor].jingliu_state.converted_attack =
                percentage_of(consumed, hp_to_attack_bp).min(cap);
        }
    }

    fn eidolon_one_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: usize,
    ) {
        if !battle.is_targetable(target) {
            return;
        }
        if self.novaflare() {
            battle.secondary_hit_from_hp(actor, target, ICE, 8_000, action);
        } else if battle.attack_targets().len() == 1 {
            battle.secondary_hit(actor, target, ICE, 10_000, DamageKind::Additional, action);
        }
    }
}

impl Mechanics for Jingliu {
    fn can_use_basic(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !battle.units[actor].jingliu_state.transformed
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Skill && battle.units[actor].jingliu_state.transformed)
            .then_some(&self.enhanced_skill)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        let state = &mut battle.units[actor].jingliu_state;
        state.transformed_attack = state.transformed;
        let enhanced_skill = action == Action::Skill && state.transformed;
        state.amplified_skill = enhanced_skill && std::mem::take(&mut state.amplified_skill_ready);
        state.defense_ignore = std::mem::take(&mut state.defense_ignore_ready);
        if state.transformed {
            self.consume_teammate_hp(battle, actor);
        }
        if self.eidolon >= 1 && (enhanced_skill || action == Action::Ultimate) {
            let crit_damage = if self.novaflare() { 3_600 } else { 2_400 };
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::CritDamage,
                crit_damage,
                1,
                Some(E1_CRIT_DAMAGE),
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
        let enhanced_skill =
            action == Action::Skill && battle.units[actor].jingliu_state.transformed_attack;
        if self.eidolon >= 1
            && (enhanced_skill || action == Action::Ultimate)
            && let Some(target) = target
        {
            self.eidolon_one_hit(battle, actor, action, target);
        }
        let state = &mut battle.units[actor].jingliu_state;
        state.converted_attack = 0;
        state.amplified_skill = false;
        state.defense_ignore = false;
        state.transformed_attack = false;
        match action {
            Action::Skill if enhanced_skill => self.spend_syzygy(battle, actor),
            Action::Skill => {
                if !self.novaflare() && self.sword_champion {
                    battle.advance(actor, 1_000);
                }
                self.gain_syzygy(battle, actor);
            }
            Action::Ultimate => {
                if self.eidolon >= 2 {
                    battle.units[actor].jingliu_state.amplified_skill_ready = true;
                }
                self.gain_syzygy(battle, actor);
            }
            Action::Basic | Action::EnhancedUltimate => {}
        }
    }

    fn on_ally_health_event(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        attack_index: Option<u32>,
    ) {
        if !self.novaflare() {
            return;
        }
        let state = &mut battle.units[owner].jingliu_state;
        if let Some(index) = attack_index {
            let last = &mut state.last_damage_attack[target];
            if last.replace(index) == Some(index) {
                return;
            }
        }
        state.health_events += 1;
        let accrues = state.health_events == HEALTH_EVENTS_PER_SYZYGY;
        if accrues {
            state.health_events = 0;
        }
        if state.transformed && state.moonlight < MOONLIGHT_CAP {
            let before = state.moonlight;
            state.moonlight += 1;
            let after = state.moonlight;
            battle.emit(
                EventKind::Moonlight { before, after },
                Some(owner),
                Some(target),
            );
        }
        if accrues {
            self.gain_syzygy(battle, owner);
        }
    }

    fn effect_resistance_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        u32::from(self.deathrealm && battle.units[actor].jingliu_state.transformed) * 3_500
    }

    fn bonus_attack_flat(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        battle.units[actor].jingliu_state.converted_attack
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        u32::from(battle.units[actor].jingliu_state.transformed) * self.crit_rate_bp
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].jingliu_state;
        if !state.transformed {
            return 0;
        }
        match self.scaling {
            Scaling::Attack { .. } => u32::from(self.eidolon >= 6) * 5_000,
            Scaling::MaxHp {
                moonlight_crit_damage_bp,
            } => {
                u32::from(state.moonlight)
                    * (moonlight_crit_damage_bp + u32::from(self.eidolon >= 4) * 2_000)
            }
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].jingliu_state;
        let ultimate_trace = if self.novaflare() {
            self.deathrealm
        } else {
            self.frost_wraith
        };
        if kind != DamageKind::Direct {
            return 0;
        }
        match action {
            Some(Action::Skill) if state.amplified_skill => 8_000,
            Some(Action::Ultimate) if ultimate_trace && state.transformed_attack => 2_000,
            _ => 0,
        }
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _: usize) -> u32 {
        u32::from(battle.units[actor].jingliu_state.defense_ignore) * 2_500
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        u32::from(
            self.novaflare()
                && self.eidolon >= 6
                && element == ICE
                && battle.units[actor].jingliu_state.transformed,
        ) * 3_000
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        if action != Action::Ultimate {
            return None;
        }
        let state = &battle.units[actor].jingliu_state;
        let ready = state.transformed || (state.syzygy == 1 && battle.skill_points == 0);
        Some(if ready { 0 } else { 4 })
    }
}
