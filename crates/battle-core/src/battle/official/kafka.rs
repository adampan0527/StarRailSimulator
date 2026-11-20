use std::sync::Arc;

use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, periodic::DotEvent, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1005";
const THUNDER: Element = Element::Thunder;
const PLUNDER_ENERGY_MILLI: u32 = 5_000;
const E1_CHANCE_BP: u32 = 10_000;
const E1_VULNERABILITY_BP: u32 = 3_000;
const E1_TURNS: u32 = 2;
const E4_ENERGY_MILLI: u32 = 2_000;
const E6_SHOCK_BP: u16 = 15_600;
const E6_SHOCK_TURNS: u32 = 1;
const NOVAFLARE_ATTACK_THRESHOLD_BP: u32 = 7_500;
const NOVAFLARE_ATTACK_BONUS_BP: u32 = 10_000;
const NOVAFLARE_FOLLOW_UP_DETONATION_BP: u16 = 8_000;

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    torture: &'static str,
    plunder: &'static str,
    thorns: &'static str,
}

const BASE: KitIds = KitIds {
    basic: "skill.100501",
    skill: "skill.100502",
    ultimate: "skill.100503",
    talent: "skill.100504",
    torture: "trace.1005101",
    plunder: "trace.1005102",
    thorns: "trace.1005103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1100501",
    skill: "skill.1100502",
    ultimate: "skill.1100503",
    talent: "skill.1100504",
    torture: "trace.11005101",
    plunder: "trace.11005102",
    thorns: "trace.11005103",
};

struct Kafka {
    enhanced: bool,
    eidolon: u8,
    torture: bool,
    plunder: bool,
    thorns: bool,
    skill_detonation_bp: u16,
    splash_detonation_bp: Option<u16>,
    ultimate_detonation_bp: u16,
    shock_bp: u16,
    ultimate_shock_chance_bp: u32,
    ultimate_shock_turns: u32,
    follow_up: FollowUpAttack,
    follow_up_shock_chance_bp: u32,
    follow_up_shock_turns: u32,
    follow_up_recovery: u8,
    follow_up_cap: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let has_variant = |ids: KitIds| {
        [ids.basic, ids.skill, ids.ultimate, ids.talent]
            .into_iter()
            .any(|id| input.skill_parameters.contains_key(id))
    };
    let (enhanced, ids) = match (has_variant(BASE), has_variant(NOVAFLARE)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic = input.parameters(ids.basic, 1)?;
    let skill = input.parameters(ids.skill, if enhanced { 4 } else { 3 })?;
    let ultimate = input.parameters(ids.ultimate, 5)?;
    let talent = input.parameters(ids.talent, if enhanced { 5 } else { 3 })?;
    let traced = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let thorns = traced(ids.thorns);
    let thorns_chance = if !enhanced && thorns { 3_000 } else { 0 };
    let e6 = input.eidolon >= 6;
    let shock_bp = basis_points(ids.ultimate, ultimate[3])?
        .checked_add(if e6 { E6_SHOCK_BP } else { 0 })
        .ok_or_else(|| invalid("Shock damage multiplier"))?;
    let extra_shock_turns = if e6 { E6_SHOCK_TURNS } else { 0 };
    let (follow_up_recovery, follow_up_cap) = if enhanced {
        (
            fixed_count(ids.talent, talent[3], 1)?,
            fixed_count(ids.talent, talent[4], 2)?,
        )
    } else {
        (1, 1)
    };

    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(ids.basic, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                hit(basis_points(ids.skill, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: THUNDER,
                    multiplier_bp: basis_points(ids.skill, skill[2])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ids.ultimate, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Kafka {
            enhanced,
            eidolon: input.eidolon,
            torture: traced(ids.torture),
            plunder: traced(ids.plunder),
            thorns,
            skill_detonation_bp: basis_points(ids.skill, skill[1])?,
            splash_detonation_bp: skill
                .get(3)
                .map(|&value| basis_points(ids.skill, value))
                .transpose()?,
            ultimate_detonation_bp: basis_points(ids.ultimate, ultimate[4])?,
            shock_bp,
            ultimate_shock_chance_bp: u32::from(basis_points(ids.ultimate, ultimate[1])?)
                + thorns_chance,
            ultimate_shock_turns: u32::from(fixed_count(ids.ultimate, ultimate[2], 2)?)
                + extra_shock_turns,
            follow_up: FollowUpAttack {
                element: THUNDER,
                multiplier_bp: basis_points(ids.talent, talent[0])?,
                toughness: 10,
                energy_milli: 10_000,
                first_hit: None,
            },
            follow_up_shock_chance_bp: u32::from(basis_points(ids.talent, talent[1])?)
                + thorns_chance,
            follow_up_shock_turns: u32::from(fixed_count(ids.talent, talent[2], 2)?)
                + extra_shock_turns,
            follow_up_recovery,
            follow_up_cap,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(skill));
    }
    Ok(rounded as u16)
}

fn fixed_count(skill: &str, value: f64, expected: u8) -> Result<u8, BattleError> {
    if value != f64::from(expected) {
        return Err(invalid(skill));
    }
    Ok(expected)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: THUNDER,
        multiplier_bp,
        toughness,
    }
}

impl Kafka {
    fn restore_follow_up(&self, battle: &mut Battle<'_>, owner: usize, count: u8) {
        let charges = &mut battle.units[owner].kafka_follow_up_charges;
        *charges = (*charges + count).min(self.follow_up_cap);
    }

    fn apply_e1(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if self.eidolon >= 1
            && battle.hp(target).0 > 0
            && battle.roll_effect(actor, target, E1_CHANCE_BP)
        {
            battle.apply_dot_vulnerability(actor, target, E1_VULNERABILITY_BP, E1_TURNS);
        }
    }

    fn detonate_skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.detonate_all_dots(actor, target, self.skill_detonation_bp, Action::Skill);
        if let Some(bp) = self.splash_detonation_bp {
            for neighbor in battle.adjacent_enemies(target) {
                battle.detonate_all_dots(actor, neighbor, bp, Action::Skill);
            }
        }
    }

    fn detonate_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        for target in battle.enemies() {
            battle.apply_skill_shock(
                actor,
                target,
                Action::Ultimate,
                self.ultimate_shock_chance_bp,
                self.shock_bp,
                self.ultimate_shock_turns,
            );
        }
        for target in battle.enemies() {
            if self.enhanced || self.torture {
                battle.detonate_all_dots(
                    actor,
                    target,
                    self.ultimate_detonation_bp,
                    Action::Ultimate,
                );
            } else {
                battle.detonate_dots(
                    actor,
                    target,
                    THUNDER,
                    self.ultimate_detonation_bp,
                    Action::Ultimate,
                );
            }
        }
        if self.enhanced && self.thorns {
            self.restore_follow_up(battle, actor, 1);
        }
    }
}

impl Mechanics for Kafka {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        self.restore_follow_up(battle, actor, 1);
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            self.restore_follow_up(battle, owner, self.follow_up_recovery);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if self.enhanced && self.eidolon >= 1 {
            // The debuff takes effect before this action's DoT detonation.
            let targets = battle.attack_targets().to_vec();
            for target in targets {
                self.apply_e1(battle, actor, target);
            }
        }
        match (action, target) {
            (Action::Skill, Some(target)) => self.detonate_skill(battle, actor, target),
            (Action::Ultimate, _) => self.detonate_ultimate(battle, actor),
            _ => {}
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        if owner == source
            || (!self.enhanced && (origin != Action::Basic || battle.in_follow_up()))
            || battle.units[owner].kafka_follow_up_charges == 0
        {
            return;
        }
        let Some(&target) = battle.attack_targets().first() else {
            return;
        };
        if battle.hp(target).0 == 0 {
            return;
        }
        battle.units[owner].kafka_follow_up_charges -= 1;
        battle.follow_up_single(owner, target, origin, self.follow_up.clone());
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
    ) {
        self.apply_e1(battle, actor, target);
        battle.apply_skill_shock(
            actor,
            target,
            origin,
            self.follow_up_shock_chance_bp,
            self.shock_bp,
            self.follow_up_shock_turns,
        );
        if self.enhanced && self.thorns {
            battle.detonate_all_dots(actor, target, NOVAFLARE_FOLLOW_UP_DETONATION_BP, origin);
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if self.plunder && battle.has_shock(target) {
            battle.gain_energy(owner, owner, PLUNDER_ENERGY_MILLI);
        }
    }

    fn on_enemy_dot_damage(&self, battle: &mut Battle<'_>, owner: usize, event: DotEvent) {
        if self.eidolon >= 4
            && event.source == owner
            && event.element == THUNDER
            && !event.from_break
        {
            battle.gain_energy(owner, owner, E4_ENERGY_MILLI);
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, _owner: usize, target: usize) -> u32 {
        if self.enhanced
            && self.torture
            && battle.effective_effect_hit_rate_bp(target) >= NOVAFLARE_ATTACK_THRESHOLD_BP
        {
            NOVAFLARE_ATTACK_BONUS_BP
        } else {
            0
        }
    }

    fn ally_dot_damage_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        match (self.eidolon >= 2, self.enhanced) {
            (false, _) => 0,
            (true, false) => 2_500,
            (true, true) => 3_300,
        }
    }
}
