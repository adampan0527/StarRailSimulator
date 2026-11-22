use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        resolve::DamageTargetAura,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1306";
const QUANTUM: Element = Element::Quantum;
const ENERGY_CAP: u32 = 110;
const E1_ATTACK_BP: u32 = 4_000;
const E1_SPEED_BP: u32 = 1_500;
const E1_SPEED_TURNS: u32 = 2;
const E6_CRIT_RATIO_BP: u32 = 3_000;

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    almanac: &'static str,
    artificial_flower: &'static str,
    nocturne: &'static str,
}

impl KitIds {
    fn has_combat_data(self, input: &OfficialCharacter) -> bool {
        [self.basic, self.skill, self.ultimate, self.talent]
            .iter()
            .any(|id| input.skill_parameters.contains_key(*id))
    }
}

const BASE: KitIds = KitIds {
    basic: "skill.130601",
    skill: "skill.130602",
    ultimate: "skill.130603",
    talent: "skill.130604",
    almanac: "trace.1306101",
    artificial_flower: "trace.1306102",
    nocturne: "trace.1306103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1130601",
    skill: "skill.1130602",
    ultimate: "skill.1130603",
    talent: "skill.1130604",
    almanac: "trace.11306101",
    artificial_flower: "trace.11306102",
    nocturne: "trace.11306103",
};

#[derive(Clone, Default)]
pub(in crate::battle) struct SparkleState {
    allies: [SparkleAlly; 4],
    overflow: u8,
    free_skill: bool,
    extra_turn_points_spent: u8,
}

#[derive(Clone, Copy, Default)]
struct SparkleAlly {
    dreamdiver: Dreamdiver,
    cipher_expires: u32,
    talent_stacks: u32,
    talent_expires: u32,
    points_spent: u8,
}

#[derive(Clone, Copy, Default)]
struct Dreamdiver {
    crit_bp: u32,
    expires: u32,
    applied_at: u32,
}

struct Sparkle {
    enhanced: bool,
    eidolon: u8,
    almanac: bool,
    artificial_flower: bool,
    nocturne: bool,
    skill_crit_ratio_bp: u32,
    skill_crit_flat_bp: u32,
    skill_turns: u32,
    advance_bp: u16,
    cipher_turns: u32,
    cipher_stack_bp: u32,
    recovered_points: u8,
    overflow_cap: u8,
    talent_turns: u32,
    talent_stack_bp: u32,
    talent_stack_cap: u32,
    extra_skill_point_cap: u8,
    free_skill: OfficialAbility,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let (enhanced, ids) = match (
        BASE.has_combat_data(input),
        NOVAFLARE.has_combat_data(input),
    ) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic = input.parameters(ids.basic, 1)?;
    let skill = input.parameters(ids.skill, 4)?;
    let ultimate = input.parameters(ids.ultimate, if enhanced { 5 } else { 4 })?;
    let talent = input.parameters(ids.talent, 4)?;
    let (skill_turns, recovered_points, cipher_turns) =
        if enhanced { (2, 6, 3) } else { (1, 4, 2) };
    if skill[2] != f64::from(skill_turns)
        || skill[3] != 0.5
        || ultimate[1] != f64::from(recovered_points)
        || ultimate[3] != f64::from(cipher_turns)
        || (enhanced && ultimate[4] != 10.0)
        || talent[0] != 2.0
        || talent[2] != 2.0
        || talent[3] != 3.0
    {
        return Err(invalid(
            "skill duration, action advance or skill point parameters",
        ));
    }
    let enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let almanac = enabled(ids.almanac);
    let eidolon = input.eidolon;
    let fourth_eidolon = u8::from(eidolon >= 4);
    let dreamdiver = |skill_points| OfficialAbility {
        target: Target::Ally,
        energy_milli: 30_000,
        effects: Vec::new(),
        skill_points,
    };

    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: if almanac { 30_000 } else { 20_000 },
            effects: vec![Effect::Hit {
                element: QUANTUM,
                multiplier_bp: basis_points(ids.basic, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: dreamdiver(SkillPoints::Spend),
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Sparkle {
            enhanced,
            eidolon,
            almanac,
            artificial_flower: enabled(ids.artificial_flower),
            nocturne: enabled(ids.nocturne),
            skill_crit_ratio_bp: u32::from(basis_points(ids.skill, skill[0])?),
            skill_crit_flat_bp: u32::from(basis_points(ids.skill, skill[1])?),
            skill_turns,
            advance_bp: basis_points(ids.skill, skill[3])?,
            cipher_turns: cipher_turns + u32::from(!enhanced && eidolon >= 1),
            cipher_stack_bp: u32::from(basis_points(ids.ultimate, ultimate[2])?),
            recovered_points: recovered_points + fourth_eidolon,
            overflow_cap: ultimate.get(4).copied().unwrap_or(0.0) as u8,
            talent_turns: talent[0] as u32,
            talent_stack_bp: u32::from(basis_points(ids.talent, talent[1])?),
            talent_stack_cap: talent[3] as u32,
            extra_skill_point_cap: talent[2] as u8 + fourth_eidolon,
            free_skill: dreamdiver(SkillPoints::Unchanged),
        }),
    })
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(id));
    }
    Ok(rounded as u16)
}

impl Sparkle {
    fn cipher_active(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        if !battle.allies().contains(&target) {
            return false;
        }
        let ally = &battle.units[owner].sparkle_state.allies[target];
        battle.units[target].finished_turns < ally.cipher_expires
    }

    fn dreamdiver_active(&self, battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        if !battle.allies().contains(&target) {
            return false;
        }
        let buff = battle.units[owner].sparkle_state.allies[target].dreamdiver;
        let unit = &battle.units[target];
        buff.crit_bp > 0
            && (unit.finished_turns < buff.expires
                || (!self.enhanced && self.artificial_flower && unit.normal_turns == buff.expires))
    }

    fn talent_stacks(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let timer = if self.enhanced { owner } else { target };
        if !battle.allies().contains(&timer) {
            return 0;
        }
        let state = &battle.units[timer];
        let talent = &battle.units[owner].sparkle_state.allies[timer];
        if state.finished_turns < talent.talent_expires {
            talent.talent_stacks
        } else {
            0
        }
    }

    fn talent_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let cipher = if Self::cipher_active(battle, owner, target) {
            self.cipher_stack_bp
        } else {
            0
        };
        self.talent_stacks(battle, owner, target) * (self.talent_stack_bp + cipher)
    }

    fn speed_up(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.enhanced && self.eidolon >= 1 {
            battle.apply_ally_buff(
                actor,
                actor,
                AllyBuffKind::SpeedPercent,
                E1_SPEED_BP,
                E1_SPEED_TURNS,
            );
        }
    }

    fn apply_skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        // A previous application from this same skill never increases its own snapshot.
        let crit_damage_bp = battle
            .effective_crit_damage_bp(actor)
            .saturating_sub(self.ally_crit_damage_bonus_bp(battle, actor, actor));
        let ratio_bp = self.skill_crit_ratio_bp
            + if self.eidolon >= 6 {
                E6_CRIT_RATIO_BP
            } else {
                0
            };
        let crit_bp = self.skill_crit_flat_bp + percentage_of(crit_damage_bp, ratio_bp);
        for ally in battle.allies() {
            if battle.is_targetable(ally)
                && (ally == target
                    || (self.eidolon >= 6 && Self::cipher_active(battle, actor, ally)))
            {
                battle.units[actor].sparkle_state.allies[ally].dreamdiver = Dreamdiver {
                    crit_bp,
                    expires: battle.units[ally].normal_turns + self.skill_turns,
                    applied_at: battle.decision_index,
                };
            }
        }
        battle.units[actor].sparkle_state.free_skill = false;
        self.speed_up(battle, actor);
        if actor != target {
            battle.advance_ally(actor, target, self.advance_bp);
        }
    }

    fn apply_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let restored = battle.restore_skill_points(actor, self.recovered_points);
        let bank = &mut battle.units[actor].sparkle_state.overflow;
        *bank = (*bank + self.recovered_points - restored).min(self.overflow_cap);

        for ally in battle.allies() {
            if battle.units[ally].hp > 0 {
                let expires = battle.units[ally].normal_turns + self.cipher_turns;
                battle.units[actor].sparkle_state.allies[ally].cipher_expires = expires;
            }
        }

        if self.eidolon >= 6 {
            self.spread_dreamdiver(battle, actor);
        }
    }

    fn spread_dreamdiver(&self, battle: &mut Battle<'_>, actor: usize) {
        let latest = battle
            .allies()
            .filter(|&ally| {
                battle.is_targetable(ally) && self.dreamdiver_active(battle, actor, ally)
            })
            .max_by_key(|&ally| {
                (
                    battle.units[actor].sparkle_state.allies[ally]
                        .dreamdiver
                        .applied_at,
                    Reverse(ally),
                )
            });
        let Some(source) = latest else {
            return;
        };
        let buff = battle.units[actor].sparkle_state.allies[source].dreamdiver;
        let remaining = buff
            .expires
            .saturating_sub(battle.units[source].finished_turns);
        for ally in battle.allies() {
            if ally != source
                && battle.is_targetable(ally)
                && Self::cipher_active(battle, actor, ally)
            {
                battle.units[actor].sparkle_state.allies[ally].dreamdiver = Dreamdiver {
                    expires: battle.units[ally].normal_turns + remaining,
                    ..buff
                };
            }
        }
    }

    fn restore_overflow(battle: &mut Battle<'_>, owner: usize) {
        let overflow = battle.units[owner].sparkle_state.overflow;
        if overflow > 0 {
            let restored = battle.restore_skill_points(owner, overflow);
            battle.units[owner].sparkle_state.overflow -= restored;
        }
    }
}

impl Mechanics for Sparkle {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        self.speed_up(battle, actor);
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.set_skill_points(actor, battle.skill_points.min(battle.skill_point_cap()));
    }

    fn skill_point_cap_bonus(&self) -> u8 {
        self.extra_skill_point_cap
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Skill && battle.units[actor].sparkle_state.free_skill)
            .then_some(&self.free_skill)
    }

    fn on_ally_skill_points_spent(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        points: u8,
    ) {
        for ally in battle.allies() {
            if (!self.enhanced || ally == owner) && battle.units[ally].hp > 0 {
                let stacks = self.talent_stacks(battle, owner, ally) + u32::from(points);
                let expires = battle.units[ally].normal_turns + self.talent_turns;
                let ally = &mut battle.units[owner].sparkle_state.allies[ally];
                ally.talent_stacks = stacks.min(self.talent_stack_cap);
                ally.talent_expires = expires;
            }
        }
        if self.enhanced && self.almanac && self.dreamdiver_active(battle, owner, actor) {
            battle.gain_energy(owner, owner, 1_000);
        }
        if self.enhanced && self.artificial_flower {
            let extra_turn = battle.in_extra_turn(actor);
            let state = &mut battle.units[owner].sparkle_state;
            let spent = if extra_turn {
                &mut state.extra_turn_points_spent
            } else {
                &mut state.allies[actor].points_spent
            };
            *spent = spent.saturating_add(points);
            if *spent >= 3 {
                state.free_skill = true;
            }
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.apply_skill(battle, actor, target),
            (Action::Ultimate, _) => self.apply_ultimate(battle, actor),
            _ => {}
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        battle.units[owner].sparkle_state.allies[actor].points_spent = 0;
        Self::restore_overflow(battle, owner);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if !action.is_ultimate()
            && battle.in_extra_turn(actor)
            && battle
                .mechanics(actor)
                .is_none_or(|rules| !rules.keeps_turn(action))
        {
            battle.units[owner].sparkle_state.extra_turn_points_spent = 0;
            Self::restore_overflow(battle, owner);
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let cipher = if self.eidolon >= 1 && Self::cipher_active(battle, owner, target) {
            E1_ATTACK_BP
        } else {
            0
        };
        let quantum = |unit: usize| {
            battle.plan.units[unit]
                .combat_type
                .is_some_and(|combat| combat.element == QUANTUM)
        };
        cipher
            + if !self.nocturne {
                0
            } else if self.enhanced {
                4_500
            } else {
                let extra = if quantum(target) {
                    let count = battle.allies().filter(|&ally| quantum(ally)).count();
                    [0, 500, 1_500, 3_000][count.min(3)]
                } else {
                    0
                };
                1_500 + extra
            }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.dreamdiver_active(battle, owner, target) {
            battle.units[owner].sparkle_state.allies[target]
                .dreamdiver
                .crit_bp
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _element: Element,
    ) -> u32 {
        if self.enhanced {
            0
        } else {
            self.talent_bonus_bp(battle, owner, target)
        }
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        if !battle.allies().contains(&actor) {
            return DamageTargetAura::default();
        }
        let stacks = self.talent_stacks(battle, owner, actor);
        DamageTargetAura {
            vulnerability_bp: if self.enhanced {
                self.talent_bonus_bp(battle, owner, actor)
            } else {
                0
            },
            defense_down_bp: if self.eidolon >= 2 {
                stacks * if self.enhanced { 1_000 } else { 800 }
            } else {
                0
            },
            resistance_down_bp: if self.enhanced
                && self.nocturne
                && self.dreamdiver_active(battle, owner, actor)
            {
                1_000
            } else {
                0
            },
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::ElationSkill => None,
            Action::Basic => None,
            Action::Ultimate | Action::EnhancedUltimate => Some(0),
            Action::Skill | Action::EnhancedSkill => {
                let preferred = battle
                    .allies()
                    .filter(|&unit| unit != actor && battle.is_targetable(unit))
                    .max_by_key(|&unit| (battle.plan.units[unit].stats.attack, Reverse(unit)));
                Some(if preferred.is_some() && target == preferred {
                    2
                } else {
                    4
                })
            }
        }
    }
}
