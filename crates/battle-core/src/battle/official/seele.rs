use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Ability, Effect, Element, Target},
};

use super::{
    super::Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1102";
const QUANTUM: Element = Element::Quantum;
const E1_HP_BP: u32 = 8_000;
const E1_CRIT_BP: u32 = 1_500;
const NOVAFLARE_E1_DEFENSE_IGNORE_BP: u32 = 2_000;

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    nightshade: &'static str,
    lacerate: &'static str,
    rippling_waves: &'static str,
}

impl KitIds {
    fn has_skill(self, input: &OfficialCharacter) -> bool {
        [self.basic, self.skill, self.ultimate, self.talent]
            .iter()
            .any(|id| input.skill_parameters.contains_key(*id))
    }
}

const BASE: KitIds = KitIds {
    basic: "skill.110201",
    skill: "skill.110202",
    ultimate: "skill.110203",
    talent: "skill.110204",
    nightshade: "trace.1102101",
    lacerate: "trace.1102102",
    rippling_waves: "trace.1102103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1110201",
    skill: "skill.1110202",
    ultimate: "skill.1110203",
    talent: "skill.1110204",
    nightshade: "trace.11102101",
    lacerate: "trace.11102102",
    rippling_waves: "trace.11102103",
};

#[derive(Clone, Default)]
pub(in crate::battle) struct SeeleState {
    speed_expires: [u32; 2],
    amplification_expires: u32,
    nightshade_expires: u32,
    nightshade_stacks: u8,
    automatic_skill_ready: bool,
    deferred_advance_bp: u16,
    marks: BTreeMap<usize, ButterflyFlurry>,
    pending_mark: Option<(usize, ButterflyFlurry)>,
}

#[derive(Clone)]
struct ButterflyFlurry {
    expires: u32,
    true_damage: u32,
    assists: Vec<(usize, u32)>,
}

struct Seele {
    enhanced: bool,
    eidolon: u8,
    nightshade: bool,
    lacerate: bool,
    rippling_waves: bool,
    speed_bp: u32,
    speed_turns: u32,
    amplification_bp: u32,
    amplification_turns: u32,
    automatic_skill_threshold_bp: Option<u32>,
    skill_hits: Vec<Effect>,
    ultimate_bp: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let (enhanced, ids) = match (BASE.has_skill(input), NOVAFLARE.has_skill(input)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic_bp = ratio_bp(ids.basic, input.parameters(ids.basic, 1)?[0])?;
    let skill = input.parameters(ids.skill, if enhanced { 4 } else { 3 })?;
    let skill_bp = ratio_bp(ids.skill, skill[0])?;
    let ultimate_bp = ratio_bp(ids.ultimate, input.parameters(ids.ultimate, 1)?[0])?;
    let talent = input.parameters(ids.talent, 2)?;
    let speed_turns = if enhanced { 3 } else { 2 };
    let amplification_turns = if enhanced { 3 } else { 1 };
    if skill[2] != f64::from(speed_turns) || talent[1] != f64::from(amplification_turns) {
        return Err(invalid("speed or amplification duration"));
    }
    let automatic_skill_threshold_bp = skill
        .get(3)
        .map(|&ratio| ratio_bp(ids.skill, ratio))
        .transpose()?;
    if automatic_skill_threshold_bp.is_some_and(|bp| bp > 10_000) {
        return Err(invalid(ids.skill));
    }
    let trace_enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    // The action has four damage segments. The locked table supplies the total toughness.
    let skill_hits = skill_hits(skill_bp);
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: ability(vec![hit(basic_bp, 10)], 20_000, SkillPoints::Generate),
        skill: ability(skill_hits.clone(), 30_000, SkillPoints::Spend),
        ultimate: ability(vec![hit(ultimate_bp, 30)], 5_000, SkillPoints::Unchanged),
        mechanics: Arc::new(Seele {
            enhanced,
            eidolon: input.eidolon,
            nightshade: trace_enabled(ids.nightshade),
            lacerate: trace_enabled(ids.lacerate),
            rippling_waves: trace_enabled(ids.rippling_waves),
            speed_bp: ratio_bp(ids.skill, skill[1])?,
            speed_turns,
            amplification_bp: ratio_bp(ids.talent, talent[0])?,
            amplification_turns,
            automatic_skill_threshold_bp,
            skill_hits,
            ultimate_bp,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, value: f64) -> Result<u32, BattleError> {
    let bp = (value * 10_000.0).round();
    if bp > 100_000.0 {
        return Err(invalid(id));
    }
    Ok(bp as u32)
}

fn ability(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u32, toughness: u16) -> Effect {
    Effect::WideHit {
        element: QUANTUM,
        multiplier_bp,
        toughness,
    }
}

fn skill_hits(multiplier_bp: u32) -> Vec<Effect> {
    let first = multiplier_bp * 2 / 10;
    let middle = multiplier_bp / 10;
    [
        hit(first, 4),
        hit(middle, 2),
        hit(middle, 2),
        hit(multiplier_bp - first - middle * 2, 12),
    ]
    .into()
}

fn hp_at_most(battle: &Battle<'_>, unit: usize, threshold_bp: u32) -> bool {
    let (hp, max_hp) = battle.hp(unit);
    u64::from(hp) * 10_000 <= u64::from(max_hp) * u64::from(threshold_bp)
}

fn lowest_hp_ratio(battle: &Battle<'_>) -> Option<usize> {
    battle
        .enemies()
        .filter(|&unit| battle.is_targetable(unit))
        .min_by(|&left, &right| {
            let (left_hp, left_max) = battle.hp(left);
            let (right_hp, right_max) = battle.hp(right);
            (u64::from(left_hp) * u64::from(right_max))
                .cmp(&(u64::from(right_hp) * u64::from(left_max)))
                .then_with(|| left.cmp(&right))
        })
}

fn scale_damage(amount: u32, bp: u32) -> u32 {
    (u64::from(amount) * u64::from(bp) / 10_000) as u32
}

impl Seele {
    fn amplify(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor];
        state.seele_state.amplification_expires = state.normal_turns + self.amplification_turns;
    }

    fn grant_resurgence(&self, battle: &mut Battle<'_>, actor: usize) {
        if !battle.in_extra_turn(actor) {
            self.amplify(battle, actor);
            battle.queue_extra_turn(actor);
        }
    }

    fn buff_speed(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor];
        let stack_count = 1 + usize::from(self.eidolon >= 2);
        let expirations = &mut state.seele_state.speed_expires[..stack_count];
        if let Some(oldest) = expirations.iter_mut().min() {
            *oldest = state.normal_turns + self.speed_turns;
        }
    }

    fn mark<'a>(
        &self,
        battle: &'a Battle<'_>,
        actor: usize,
        target: usize,
    ) -> Option<&'a ButterflyFlurry> {
        battle.units[actor]
            .seele_state
            .marks
            .get(&target)
            .filter(|mark| mark.expires > battle.units[target].finished_turns)
    }

    fn resolve_marks(&self, battle: &mut Battle<'_>, owner: usize, source: usize, origin: Action) {
        for target in battle.attack_targets().to_vec() {
            if !battle.is_targetable(target) {
                continue;
            }
            let Some(mark) = self.mark(battle, owner, target).cloned() else {
                continue;
            };
            if self.enhanced {
                battle.true_damage(owner, target, origin, mark.true_damage, mark.assists);
            } else {
                let multiplier = scale_damage(self.ultimate_bp, 1_500) as u16;
                battle.secondary_hit(
                    owner,
                    target,
                    QUANTUM,
                    multiplier,
                    DamageKind::Additional,
                    origin,
                );
                if source == owner && !battle.is_targetable(target) {
                    self.grant_resurgence(battle, owner);
                }
            }
        }
        if source == owner
            && origin == Action::Ultimate
            && !battle.in_follow_up()
            && let Some((target, mark)) = battle.units[owner].seele_state.pending_mark.take()
            && battle.is_targetable(target)
        {
            battle.units[owner].seele_state.marks.insert(target, mark);
        }
    }
}

impl Mechanics for Seele {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].seele_state.automatic_skill_ready = self.enhanced;
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            self.on_extra_turn_start(battle, actor);
        }
    }

    fn on_extra_turn_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].seele_state.automatic_skill_ready = self.enhanced;
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action != Action::Ultimate && !battle.in_extra_turn(actor) {
            let pending = std::mem::take(&mut battle.units[actor].seele_state.deferred_advance_bp);
            if pending > 0 {
                battle.advance(actor, pending);
            }
        }
        match action {
            Action::ElationSkill => {}
            Action::Basic => {}
            Action::Skill | Action::EnhancedSkill => self.buff_speed(battle, actor),
            Action::Ultimate | Action::EnhancedUltimate => self.amplify(battle, actor),
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Basic && self.rippling_waves {
            if battle.interrupted_turn_actor() == Some(actor) && battle.in_extra_turn(actor) {
                // The pending ordinary action has not reset its gauge yet.
                let pending = &mut battle.units[actor].seele_state.deferred_advance_bp;
                *pending = (*pending + 2_000).min(10_000);
            } else {
                battle.advance(actor, 2_000);
            }
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _action: Action,
        _hp_before: u32,
    ) {
        if battle.units[target].hp == 0
            && (!self.enhanced || self.mark(battle, actor, target).is_none())
        {
            self.grant_resurgence(battle, actor);
        }
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, _: usize) {
        if self.eidolon >= 4 {
            battle.gain_energy(actor, actor, 15_000);
        }
        if self.enhanced && self.nightshade {
            let state = &mut battle.units[actor];
            let nightshade = &mut state.seele_state;
            if nightshade.nightshade_expires <= state.finished_turns {
                nightshade.nightshade_stacks = 0;
            }
            nightshade.nightshade_stacks = (nightshade.nightshade_stacks + 1).min(3);
            nightshade.nightshade_expires = state.normal_turns + 3;
        }
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, _: usize, target: usize) {
        if self.enhanced && self.mark(battle, owner, target).is_some() {
            self.grant_resurgence(battle, owner);
        }
    }

    fn capture_ultimate_damage(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        amount: u32,
        assists: &[(usize, u32)],
    ) {
        if self.eidolon < 6 {
            return;
        }
        let expires = battle.units[target].normal_turns + if self.enhanced { 3 } else { 1 };
        let state = &mut battle.units[actor].seele_state;
        let mark = ButterflyFlurry {
            expires,
            true_damage: scale_damage(amount, 3_000),
            assists: assists
                .iter()
                .map(|&(owner, amount)| (owner, scale_damage(amount, 3_000)))
                .filter(|&(_, amount)| amount > 0)
                .collect(),
        };
        state.pending_mark = Some((target, mark));
    }

    fn additional_debuff_count(&self, battle: &Battle<'_>, owner: usize, target: usize) -> usize {
        usize::from(battle.is_targetable(owner) && self.mark(battle, owner, target).is_some())
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        if self.eidolon >= 6 {
            self.resolve_marks(battle, owner, source, origin);
        }
        let Some(threshold) = self.automatic_skill_threshold_bp else {
            return;
        };
        if !battle.units[owner].seele_state.automatic_skill_ready {
            return;
        }
        let mut hit_targets = battle
            .attack_targets()
            .iter()
            .copied()
            .filter(|&target| hp_at_most(battle, target, threshold));
        let Some(first) = hit_targets.next() else {
            return;
        };
        let target = std::iter::once(first)
            .chain(hit_targets)
            .find(|&target| battle.is_targetable(target))
            .or_else(|| lowest_hp_ratio(battle));
        if let Some(target) = target {
            battle.units[owner].seele_state.automatic_skill_ready = false;
            battle.queue_triggered_skill(owner, target);
        }
    }

    fn on_triggered_skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let target = if battle.is_targetable(target) {
            Some(target)
        } else {
            lowest_hp_ratio(battle)
        };
        if let Some(target) = target {
            self.buff_speed(battle, actor);
            battle.perform_triggered_skill(
                actor,
                target,
                Ability {
                    target: Target::Enemy,
                    energy_milli: 0,
                    effects: &self.skill_hits,
                },
            );
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let unit = &battle.units[actor];
        let state = &unit.seele_state;
        let amplified = state.amplification_expires > unit.finished_turns
            && (self.enhanced || matches!(kind, DamageKind::Direct | DamageKind::Additional));
        let nightshade = if state.nightshade_expires > unit.finished_turns {
            u32::from(state.nightshade_stacks) * 5_000
        } else {
            0
        };
        nightshade + u32::from(amplified) * self.amplification_bp
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let state = &battle.units[actor];
        state
            .seele_state
            .speed_expires
            .iter()
            .filter(|&&expiry| expiry > state.finished_turns)
            .count() as u32
            * self.speed_bp
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        let unit = &battle.units[actor];
        if self.lacerate
            && element == QUANTUM
            && unit.seele_state.amplification_expires > unit.finished_turns
        {
            if self.enhanced { 2_500 } else { 2_000 }
        } else {
            0
        }
    }

    fn aggro_multiplier_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if !self.enhanced && self.nightshade && hp_at_most(battle, actor, 5_000) {
            5_000
        } else {
            10_000
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
        u32::from(self.eidolon >= 1 && hp_at_most(battle, target, E1_HP_BP)) * E1_CRIT_BP
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, _actor: usize, target: usize) -> u32 {
        u32::from(self.enhanced && self.eidolon >= 1 && hp_at_most(battle, target, E1_HP_BP))
            * NOVAFLARE_E1_DEFENSE_IGNORE_BP
    }
}
