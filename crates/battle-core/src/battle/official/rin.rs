use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::JointFollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints, archer,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1508";
const BASIC: &str = "skill.150801";
const SKILL: &str = "skill.150802";
const ULTIMATE: &str = "skill.150803";
const TALENT: &str = "skill.150804";
const JOINT: &str = "skill.150805";
const ENHANCED_SKILL: &str = "skill.150809";
const ELEGANT_CONDUCT: &str = "trace.1508101";
const LADYLIKE_POISE: &str = "trace.1508102";
const INBOUND_PROSPERITY: &str = "trace.1508103";
const ENERGY_CAPACITY: u32 = 160;
const QUANTUM: Element = Element::Quantum;
const GEM_MAX: u16 = 999;
const GEM_COST: u16 = 3;
const BOUNCE_LIMIT: u16 = 33;
const GEM_THRESHOLD: u16 = 15;
const POINT_THRESHOLD: u8 = 7;
const POINTS_RETAINED: u8 = 2;
const GEMS_PER_CONVERTED_POINT: u16 = 2;
const SHADOW_THRESHOLD: u16 = 30;

#[derive(Clone, Default)]
pub(in crate::battle) struct RinState {
    gems: u16,
    shadow_gems: u16,
    joint_used: bool,
}

struct Rin {
    enhanced_skill: OfficialAbility,
    bounce_bp: u16,
    ultimate_other_bp: u16,
    vulnerability_bp: u32,
    crit_damage_bp: u32,
    rin_joint: JointFollowUpAttack,
    archer_joint: JointFollowUpAttack,
    elegant_conduct: bool,
    ladylike_poise: bool,
    inbound_prosperity: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != QUANTUM
        || input.eidolon > 6
    {
        return Err(invalid("energy, combat type or eidolon"));
    }

    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 1)?[0];
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 6)?;
    let joint = input.parameters(JOINT, 4)?;
    let enhanced = input.parameters(ENHANCED_SKILL, 8)?;
    if ultimate[2..4] != [12.0, 1.0] || ultimate[5] != 3.0 {
        return Err(invalid(ULTIMATE));
    }
    if talent[..2] != [20.0, 2.0]
        || talent[3..]
            != [
                f64::from(POINT_THRESHOLD),
                f64::from(GEM_THRESHOLD),
                f64::from(GEM_MAX),
            ]
    {
        return Err(invalid(TALENT));
    }
    if joint[1..3] != [4.0, 3.0] {
        return Err(invalid(JOINT));
    }
    if enhanced[1] != f64::from(GEM_COST)
        || enhanced[3..6]
            != [
                f64::from(POINTS_RETAINED),
                f64::from(GEMS_PER_CONVERTED_POINT),
                f64::from(BOUNCE_LIMIT),
            ]
    {
        return Err(invalid(ENHANCED_SKILL));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: attack(
            Target::Enemy,
            SkillPoints::Generate,
            20_000,
            u32::from(ratio_bp(BASIC, basic)?),
            10,
        ),
        skill: attack(
            Target::Enemy,
            SkillPoints::Spend,
            30_000,
            u32::from(ratio_bp(SKILL, skill)?),
            20,
        ),
        ultimate: attack(
            Target::Enemy,
            SkillPoints::Unchanged,
            5_000,
            wide_ratio_bp(ULTIMATE, ultimate[0])?,
            30,
        ),
        mechanics: Arc::new(Rin {
            enhanced_skill: attack(
                Target::AllEnemies,
                SkillPoints::Unchanged,
                30_000,
                u32::from(ratio_bp(ENHANCED_SKILL, enhanced[0])?),
                20,
            ),
            bounce_bp: ratio_bp(ENHANCED_SKILL, enhanced[2])?,
            ultimate_other_bp: ratio_bp(ULTIMATE, ultimate[1])?,
            vulnerability_bp: u32::from(ratio_bp(ULTIMATE, ultimate[4])?),
            crit_damage_bp: u32::from(ratio_bp(TALENT, talent[2])?),
            rin_joint: joint_attack(ratio_bp(JOINT, joint[0])?, 10_000),
            archer_joint: joint_attack(ratio_bp(JOINT, joint[3])?, 0),
            elegant_conduct: trace(ELEGANT_CONDUCT),
            ladylike_poise: trace(LADYLIKE_POISE),
            inbound_prosperity: trace(INBOUND_PROSPERITY),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("Rin Tohsaka invalid {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    u16::try_from(wide_ratio_bp(id, ratio)?).map_err(|_| invalid(id))
}

fn wide_ratio_bp(id: &str, ratio: f64) -> Result<u32, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u32::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u32)
}

fn attack(
    target: Target,
    skill_points: SkillPoints,
    energy_milli: u32,
    multiplier_bp: u32,
    toughness: u16,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::WideHit {
            element: QUANTUM,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn joint_attack(multiplier_bp: u16, energy_milli: u32) -> JointFollowUpAttack {
    JointFollowUpAttack {
        element: QUANTUM,
        multiplier_bp: u32::from(multiplier_bp),
        toughness: 10,
        energy_milli,
    }
}

fn is_archer(battle: &Battle<'_>, actor: usize) -> bool {
    battle.plan.units[actor].official_form.as_deref() == Some(archer::FORM)
}

fn set_gems(battle: &mut Battle<'_>, actor: usize, gems: u16) {
    let after = gems.min(GEM_MAX);
    let before = std::mem::replace(&mut battle.units[actor].rin_state.gems, after);
    if before != after {
        battle.emit(EventKind::RinGemEnergy { before, after }, Some(actor), None);
    }
}

fn gain_gems(battle: &mut Battle<'_>, actor: usize, amount: u16) {
    set_gems(battle, actor, battle.units[actor].rin_state.gems + amount);
}

fn set_shadow_gems(battle: &mut Battle<'_>, actor: usize, after: u16) {
    let before = std::mem::replace(&mut battle.units[actor].rin_state.shadow_gems, after);
    if before != after {
        battle.emit(EventKind::RinShadowGem { before, after }, Some(actor), None);
    }
}

fn enhanced_ready(battle: &Battle<'_>, actor: usize) -> bool {
    let state = &battle.units[actor].rin_state;
    state.shadow_gems > 0 || state.gems >= GEM_THRESHOLD || battle.skill_points >= POINT_THRESHOLD
}

impl Rin {
    fn refresh_speed(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.ladylike_poise {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::SpeedPercent,
                2_000,
                3,
                Some(LADYLIKE_POISE),
            );
        }
    }

    fn on_skill_point_change(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        points: u8,
    ) {
        if points == 0 || !battle.is_targetable(owner) || !battle.is_party_unit(actor) {
            return;
        }
        gain_gems(battle, owner, u16::from(points));
        let cap = if owner == actor && self.eidolon >= 4 {
            2
        } else {
            1
        };
        let current = battle.ally_buff_value(owner, actor, AllyBuffKind::CritDamage);
        let amount = (current + self.crit_damage_bp).min(cap * self.crit_damage_bp);
        battle.apply_ally_buff_with_id(
            owner,
            actor,
            AllyBuffKind::CritDamage,
            amount,
            2,
            Some(TALENT),
        );
    }

    fn bounce(&self, battle: &mut Battle<'_>, actor: usize, copied: bool) {
        let state = &battle.units[actor].rin_state;
        let shadow = state.shadow_gems > 0;
        let available = if shadow {
            state.shadow_gems
        } else {
            state.gems
        };
        if shadow && !copied {
            set_shadow_gems(battle, actor, 0);
        }
        let mut spent = 0;
        for _ in 0..(available / GEM_COST).min(BOUNCE_LIMIT) {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            if !shadow && !copied {
                set_gems(battle, actor, battle.units[actor].rin_state.gems - GEM_COST);
                spent += GEM_COST;
            }
            battle.attack_hit(actor, target, QUANTUM, self.bounce_bp, 2);
        }
        if self.eidolon >= 1 && spent >= SHADOW_THRESHOLD {
            set_shadow_gems(battle, actor, spent);
        }
    }
}

impl Mechanics for Rin {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        set_gems(battle, actor, 20);
        self.refresh_speed(battle, actor);
        if self.elegant_conduct {
            for target in battle.allies() {
                if target == actor || is_archer(battle, target) {
                    battle.apply_ally_buff_with_id(
                        actor,
                        target,
                        AllyBuffKind::AttackPercent,
                        15_000,
                        u32::MAX,
                        Some(ELEGANT_CONDUCT),
                    );
                    battle.apply_ally_buff_with_id(
                        actor,
                        target,
                        AllyBuffKind::ElementResistancePenetration(QUANTUM),
                        1_500,
                        u32::MAX,
                        Some(ELEGANT_CONDUCT),
                    );
                }
            }
        }
    }

    fn skill_point_cap_bonus(&self) -> u8 {
        2 * u8::from(self.elegant_conduct)
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.set_skill_points(actor, battle.skill_points.min(battle.skill_point_cap()));
    }

    fn on_ally_skill_points_restored(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        points: u8,
    ) {
        self.on_skill_point_change(battle, owner, actor, points);
    }

    fn on_ally_skill_points_spent(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        points: u8,
    ) {
        self.on_skill_point_change(battle, owner, actor, points);
    }

    fn can_use_skill_action(&self, battle: &Battle<'_>, actor: usize, action: Action) -> bool {
        (action == Action::EnhancedSkill) == enhanced_ready(battle, actor)
    }

    fn ability_override(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::EnhancedSkill).then_some(&self.enhanced_skill)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::EnhancedSkill if battle.units[actor].rin_state.shadow_gems == 0 => {
                let points = battle.skill_points.saturating_sub(POINTS_RETAINED);
                if points > 0 {
                    battle.spend_skill_points(actor, points);
                    gain_gems(battle, actor, u16::from(points) * GEMS_PER_CONVERTED_POINT);
                }
            }
            Action::Ultimate => {
                battle.restore_skill_points(actor, 1);
                let gems =
                    12 * u16::from(self.inbound_prosperity) + 24 * u16::from(self.eidolon >= 6);
                gain_gems(battle, actor, gems);
                for target in battle.enemies() {
                    if battle.is_targetable(target) {
                        battle.apply_vulnerability(actor, target, self.vulnerability_bp, 3);
                    }
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
        match action {
            Action::EnhancedSkill => {
                self.bounce(battle, actor, false);
                self.refresh_speed(battle, actor);
            }
            Action::Ultimate => {
                for enemy in battle.enemies().filter(|&enemy| Some(enemy) != target) {
                    battle.attack_hit(actor, enemy, QUANTUM, self.ultimate_other_bp, 20);
                }
                if self.eidolon >= 6 {
                    battle.queue_extra_turn(actor);
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
        if battle.current_action() != Some(Action::EnhancedSkill) {
            return false;
        }
        battle.resolve_ability(actor, None, self.enhanced_skill.resolve().0);
        self.bounce(battle, actor, true);
        battle.notify_attack(actor, Action::EnhancedSkill, None);
        true
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        if origin != Action::Skill
            || battle.in_follow_up()
            || battle.nested_skill.is_some()
            || battle.units[owner].rin_state.joint_used
            || !is_archer(battle, actor)
            || !battle.is_targetable(owner)
            || battle.is_crowd_controlled(owner)
            || !battle.is_targetable(actor)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
            || (battle.skill_points > 3 && !battle.units[actor].archer_state.circuit_is_full())
        {
            return;
        }
        battle.units[owner].rin_state.joint_used = true;
        battle.queue_joint_follow_up(
            [(owner, self.rin_joint), (actor, self.archer_joint)],
            origin,
            4,
        );
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        if owner == actor {
            battle.units[owner].rin_state.joint_used = false;
        }
    }

    fn damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        3_000
            * u32::from(
                self.eidolon >= 2
                    && kind == DamageKind::Direct
                    && matches!(origin, Some(Action::Skill | Action::EnhancedSkill)),
            )
    }

    fn ally_skill_damage_factor_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
    ) -> u32 {
        3_000 * u32::from(self.eidolon >= 2 && battle.is_targetable(owner))
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, _: Element) -> u32 {
        2_000 * u32::from(self.eidolon >= 6)
    }
}
