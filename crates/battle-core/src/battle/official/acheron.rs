use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1308";
const BASIC: &str = "skill.130801";
const SKILL: &str = "skill.130802";
const ULTIMATE: &str = "skill.130803";
const TALENT: &str = "skill.130804";
const RAINBLADES: [&str; 3] = ["skill.130814", "skill.130815", "skill.130816"];
const STYGIAN_RESURGE: &str = "skill.130817";
const RED_ONI: &str = "trace.1308101";
const ABYSS: &str = "trace.1308102";
const THUNDER_CORE: &str = "trace.1308103";

const DREAM_CAP: u8 = 9;
const OVERFLOW_CAP: u8 = 3;
const RAINBLADE_KNOT_CAP: u8 = 3;
const THUNDER_CORE_STACK_BP: u32 = 3_000;
const THUNDER_CORE_STACK_CAP: u8 = 3;
const THUNDER_CORE_TURNS: u32 = 3;
const THUNDER_CORE_HITS: u8 = 6;
const THUNDER_CORE_HIT_BP: u16 = 2_500;

#[derive(Clone, Default)]
pub(in crate::battle) struct AcheronState {
    dreams: u8,
    overflow: u8,
    abyss_multiplier_bp: Option<u16>,
    thunder_core_stacks: u8,
    ultimate_active: bool,
}

struct Acheron {
    eidolon: u8,
    red_oni: bool,
    abyss: bool,
    thunder_core: bool,
    talent_res_down_bp: u32,
    rainblade_bp: u16,
    knot_aoe_bp: [u16; RAINBLADE_KNOT_CAP as usize],
    resurge_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 0
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != Element::Thunder
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 7)?;
    let talent = input.parameters(TALENT, 2)?;
    if skill[2] != 1.0 || ultimate[3] != f64::from(DREAM_CAP) || talent[0] != ultimate[3] {
        return Err(invalid("Slashed Dream and Crimson Knot parameters"));
    }

    for id in RAINBLADES {
        if input.parameters(id, 3)? != &ultimate[..3] {
            return Err(invalid(id));
        }
    }
    if input.parameters(STYGIAN_RESURGE, 1)?[0] != ultimate[2] {
        return Err(invalid(STYGIAN_RESURGE));
    }
    let rainblade_bp = ratio_bp(ULTIMATE, ultimate[0])?;
    let knot_base = ultimate[1];
    let knot_step = (ultimate[4] - knot_base) / f64::from(RAINBLADE_KNOT_CAP);
    let knot_aoe_bp = [
        ratio_bp(ULTIMATE, knot_base + knot_step)?,
        ratio_bp(ULTIMATE, knot_base + 2.0 * knot_step)?,
        ratio_bp(ULTIMATE, ultimate[4])?,
    ];
    let resurge_bp = ratio_bp(ULTIMATE, ultimate[2])?;
    if (ultimate[5] - ultimate[6] - 3.0 * ultimate[0]).abs() > 0.000_1
        || (ultimate[6] - ultimate[2] - 3.0 * ultimate[4]).abs() > 0.000_1
        || knot_base <= 0.0
        || knot_step <= 0.0
        || talent[1] > 1.0
    {
        return Err(invalid("Ultimate or Talent parameters"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: attack(
            SkillPoints::Generate,
            vec![hit(ratio_bp(BASIC, basic)?, 10)],
        ),
        skill: attack(
            SkillPoints::Spend,
            vec![
                hit(ratio_bp(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Thunder,
                    multiplier_bp: ratio_bp(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
        ),
        ultimate: attack(SkillPoints::Unchanged, vec![Effect::DirectedAreaAttack]),
        mechanics: Arc::new(Acheron {
            eidolon: input.eidolon,
            red_oni: trace(RED_ONI),
            abyss: trace(ABYSS),
            thunder_core: trace(THUNDER_CORE),
            talent_res_down_bp: u32::from(ratio_bp(TALENT, talent[1])?),
            rainblade_bp,
            knot_aoe_bp,
            resurge_bp,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn attack(skill_points: SkillPoints, effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli: 0,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Thunder,
        multiplier_bp,
        toughness,
    }
}

impl Acheron {
    fn set_dreams(battle: &mut Battle<'_>, source: usize, owner: usize, after: u8, overflow: u8) {
        let state = &mut battle.units[owner].acheron_state;
        let before = std::mem::replace(&mut state.dreams, after);
        let overflow_before = std::mem::replace(&mut state.overflow, overflow);
        if before != after || overflow_before != overflow {
            battle.emit(
                EventKind::SlashedDream {
                    before,
                    after,
                    overflow_before,
                    overflow_after: overflow,
                },
                Some(source),
                Some(owner),
            );
        }
    }

    fn set_knots(battle: &mut Battle<'_>, owner: usize, target: usize, after: u8) {
        let before = std::mem::replace(&mut battle.units[target].crimson_knots, after);
        if before != after {
            battle.emit(
                EventKind::CrimsonKnot { before, after },
                Some(owner),
                Some(target),
            );
        }
    }

    fn add_knots(battle: &mut Battle<'_>, owner: usize, target: usize, added: u8) {
        Self::set_knots(
            battle,
            owner,
            target,
            battle.units[target].crimson_knots + added,
        );
    }

    fn gain_dreams(
        &self,
        battle: &mut Battle<'_>,
        source: usize,
        owner: usize,
        target: Option<usize>,
        points: u8,
    ) {
        let state = &battle.units[owner].acheron_state;
        let gained = points.min(DREAM_CAP - state.dreams);
        let overflow = if self.red_oni {
            (state.overflow + points - gained).min(OVERFLOW_CAP)
        } else {
            0
        };
        Self::set_dreams(battle, source, owner, state.dreams + gained, overflow);
        if let Some(target) = target {
            Self::add_knots(battle, owner, target, gained);
        }
    }

    fn most_marked(battle: &Battle<'_>, candidates: impl Iterator<Item = usize>) -> Option<usize> {
        candidates
            .filter(|&target| battle.is_targetable(target))
            .min_by_key(|&target| Reverse(battle.units[target].crimson_knots))
    }

    fn stack_thunder_core(&self, battle: &mut Battle<'_>, actor: usize) {
        if !self.thunder_core {
            return;
        }
        let kind = AllyBuffKind::Damage;
        let existing = if battle.has_ally_buff(actor, actor, kind, THUNDER_CORE) {
            battle.units[actor].acheron_state.thunder_core_stacks
        } else {
            0
        };
        let stacks = (existing + 1).min(THUNDER_CORE_STACK_CAP);
        battle.units[actor].acheron_state.thunder_core_stacks = stacks;
        battle.apply_ally_buff_with_id(
            actor,
            actor,
            kind,
            u32::from(stacks) * THUNDER_CORE_STACK_BP,
            THUNDER_CORE_TURNS,
            Some(THUNDER_CORE),
        );
    }

    fn rainblade(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let marks = battle.units[target].crimson_knots;
        let removed = marks.min(RAINBLADE_KNOT_CAP);
        // Reserve the stacks used by this cut before it can defeat the target.
        // The defeat listener transfers only stacks the cut did not consume.
        Self::set_knots(battle, actor, target, marks - removed);
        battle.attack_hit(actor, target, Element::Thunder, self.rainblade_bp, 5);
        if removed > 0 {
            self.stack_thunder_core(battle, actor);
            let multiplier_bp = self.knot_aoe_bp[usize::from(removed - 1)];
            for enemy in battle.enemies() {
                battle.attack_hit(actor, enemy, Element::Thunder, multiplier_bp, 0);
            }
        }
    }

    fn abyss_multiplier(&self, battle: &Battle<'_>, actor: usize) -> Option<u16> {
        if !self.abyss {
            return None;
        }
        let others = battle
            .allies()
            .filter(|&ally| {
                ally != actor
                    && battle.plan.units[ally]
                        .combat_type
                        .is_some_and(|kind| kind.path == CombatPath::Nihility)
            })
            .count();
        match others {
            0 => None,
            1 if self.eidolon < 2 => Some(11_500),
            _ => Some(16_000),
        }
    }
}

impl Mechanics for Acheron {
    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        Some(battle.units[actor].acheron_state.dreams == DREAM_CAP)
    }

    fn observes_enemy_debuffs(&self) -> bool {
        true
    }

    fn on_enemy_debuffs_applied(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        targets: &[usize],
    ) {
        if source == owner && battle.action == Some(Action::Ultimate) && !battle.in_follow_up() {
            return;
        }
        let target = Self::most_marked(battle, targets.iter().copied())
            .or_else(|| Self::most_marked(battle, battle.enemies()));
        self.gain_dreams(battle, source, owner, target, 1);
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].acheron_state.abyss_multiplier_bp =
            self.abyss_multiplier(battle, actor);
        if self.red_oni {
            let target = battle.random_living_enemy();
            self.gain_dreams(battle, actor, actor, target, 5);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 4 {
            for target in battle.enemies() {
                battle.apply_permanent_debuff(
                    owner,
                    target,
                    DebuffKind::UltimateVulnerability,
                    800,
                );
            }
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.eidolon >= 2 && owner == actor {
            let target = Self::most_marked(battle, battle.enemies());
            self.gain_dreams(battle, actor, actor, target, 1);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::ElationSkill => {}
            Action::Skill | Action::EnhancedSkill => {
                self.gain_dreams(battle, actor, actor, target, 1)
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                let overflow = battle.units[actor].acheron_state.overflow;
                Self::set_dreams(battle, actor, actor, 0, overflow);
            }
            Action::Basic => {}
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.units[actor].acheron_state.ultimate_active = true;
        self.rainblade(battle, actor, target);
        for _ in 1..RAINBLADES.len() {
            let Some(next) = Self::most_marked(battle, battle.enemies()) else {
                break;
            };
            // Keep the commanded target when it ties for the most remaining knots.
            let next = if battle.is_targetable(target)
                && battle.units[target].crimson_knots == battle.units[next].crimson_knots
            {
                target
            } else {
                next
            };
            self.rainblade(battle, actor, next);
        }
        for enemy in battle.enemies() {
            battle.attack_hit(actor, enemy, Element::Thunder, self.resurge_bp, 5);
        }
        for enemy in battle.enemies() {
            Self::set_knots(battle, actor, enemy, 0);
        }
        if self.thunder_core {
            for _ in 0..THUNDER_CORE_HITS {
                let Some(enemy) = battle.random_living_enemy() else {
                    break;
                };
                battle.attack_hit(actor, enemy, Element::Thunder, THUNDER_CORE_HIT_BP, 0);
            }
        }
        battle.units[actor].acheron_state.ultimate_active = false;

        let overflow = battle.units[actor].acheron_state.overflow;
        if overflow > 0 {
            Self::set_dreams(battle, actor, actor, overflow, 0);
            if let Some(target) = battle.random_living_enemy() {
                Self::add_knots(battle, actor, target, overflow);
            }
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        let knots = battle.units[target].crimson_knots;
        if knots > 0 {
            Self::set_knots(battle, owner, target, 0);
            if let Some(destination) = Self::most_marked(battle, battle.enemies()) {
                Self::add_knots(battle, owner, destination, knots);
            }
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        1_800 * u32::from(self.eidolon >= 1 && battle.is_debuffed(target))
    }

    fn action_damage_factor_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u16 {
        if kind == DamageKind::Direct && action.is_some() {
            return battle.units[actor]
                .acheron_state
                .abyss_multiplier_bp
                .unwrap_or(10_000);
        }
        10_000
    }

    fn all_resistance_down_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        self.talent_res_down_bp * u32::from(battle.units[owner].acheron_state.ultimate_active)
    }

    fn ignores_weakness(&self, action: Option<Action>, kind: DamageKind) -> bool {
        kind == DamageKind::Direct
            && action.is_some_and(|action| self.primary_attack_counts_as_ultimate(action))
    }

    fn primary_attack_counts_as_ultimate(&self, action: Action) -> bool {
        action == Action::Ultimate || self.eidolon == 6
    }

    fn ultimate_resistance_penetration_bp(&self) -> u32 {
        2_000 * u32::from(self.eidolon == 6)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if action != Action::Ultimate {
            return None;
        }
        let target = target?;
        let most_marked = Self::most_marked(battle, battle.enemies())?;
        let fewer_knots =
            battle.units[target].crimson_knots < battle.units[most_marked].crimson_knots;
        Some(u8::from(fewer_knots))
    }
}
