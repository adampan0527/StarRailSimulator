use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1006";
const BASE_SKILLS: [&str; 4] = [
    "skill.100601",
    "skill.100602",
    "skill.100603",
    "skill.100604",
];
const NOVAFLARE_SKILLS: [&str; 4] = [
    "skill.1100601",
    "skill.1100602",
    "skill.1100603",
    "skill.1100604",
];
const BASE_TRACES: [&str; 3] = ["trace.1006101", "trace.1006102", "trace.1006103"];
const NOVAFLARE_TRACES: [&str; 3] = ["trace.11006101", "trace.11006102", "trace.11006103"];
const QUANTUM: Element = Element::Quantum;
const MAX_EIDOLON_DEBUFFS: usize = 5;

struct SilverWolf {
    enhanced: bool,
    eidolon: u8,
    generate: bool,
    inject: bool,
    side_note: bool,
    implant_chance_bp: u32,
    implant_res_down_bp: u32,
    all_res_chance_bp: u32,
    all_res_down_bp: u32,
    defense_chance_bp: u32,
    defense_down_bp: u32,
    bug_chance_bp: u32,
    bugs: [(DebuffKind, u32); 3],
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let has_variant = |ids: [&str; 4]| {
        ids.iter()
            .any(|id| input.skill_parameters.contains_key(*id))
    };
    let (enhanced, [basic_id, skill_id, ultimate_id, talent_id], traces) =
        match (has_variant(BASE_SKILLS), has_variant(NOVAFLARE_SKILLS)) {
            (true, false) => (false, BASE_SKILLS, BASE_TRACES),
            (false, true) => (true, NOVAFLARE_SKILLS, NOVAFLARE_TRACES),
            _ => return Err(invalid("skill variant")),
        };
    let basic = input.parameters(basic_id, 1)?;
    let skill = input.parameters(skill_id, 7)?;
    let ultimate = input.parameters(ultimate_id, 4)?;
    let talent = input.parameters(talent_id, 5)?;
    if skill[2] != if enhanced { 3.0 } else { 2.0 } || skill[6] != 2.0 {
        return Err(invalid(skill_id));
    }
    if ultimate[3] != 3.0 {
        return Err(invalid(ultimate_id));
    }
    if talent[4] != 3.0 {
        return Err(invalid(talent_id));
    }
    let [generate, inject, side_note] =
        traces.map(|id| input.enabled_trace_ids.iter().any(|trace| trace == id));

    Ok(OfficialKit {
        max_energy_milli: 110_000,
        basic: ability(
            Target::Enemy,
            SkillPoints::Generate,
            20_000,
            ratio_bp(basic_id, basic[0])?,
            10,
        ),
        skill: ability(
            Target::Enemy,
            SkillPoints::Spend,
            30_000,
            ratio_bp(skill_id, skill[0])?,
            20,
        ),
        ultimate: ability(
            if enhanced {
                Target::AllEnemies
            } else {
                Target::Enemy
            },
            SkillPoints::Unchanged,
            5_000,
            ratio_bp(ultimate_id, ultimate[0])?,
            if enhanced { 20 } else { 30 },
        ),
        mechanics: Arc::new(SilverWolf {
            enhanced,
            eidolon: input.eidolon,
            generate,
            inject,
            side_note,
            implant_chance_bp: u32::from(ratio_bp(skill_id, skill[1])?),
            implant_res_down_bp: u32::from(ratio_bp(skill_id, skill[3])?),
            all_res_chance_bp: u32::from(ratio_bp(skill_id, skill[4])?),
            all_res_down_bp: u32::from(ratio_bp(skill_id, skill[5])?),
            defense_chance_bp: u32::from(ratio_bp(ultimate_id, ultimate[1])?),
            defense_down_bp: u32::from(ratio_bp(ultimate_id, ultimate[2])?),
            bug_chance_bp: u32::from(ratio_bp(talent_id, talent[3])?),
            bugs: [
                (
                    DebuffKind::AttackDown,
                    u32::from(ratio_bp(talent_id, talent[0])?),
                ),
                (
                    DebuffKind::DefenseBug,
                    u32::from(ratio_bp(talent_id, talent[1])?),
                ),
                (DebuffKind::Slow, u32::from(ratio_bp(talent_id, talent[2])?)),
            ],
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

fn ability(
    target: Target,
    skill_points: SkillPoints,
    energy_milli: u32,
    multiplier_bp: u16,
    toughness: u16,
) -> OfficialAbility {
    OfficialAbility {
        target,
        skill_points,
        energy_milli,
        effects: vec![Effect::Hit {
            element: QUANTUM,
            multiplier_bp,
            toughness,
        }],
    }
}

fn implant_on(battle: &Battle<'_>, source: usize, target: usize) -> Option<(Element, u32, u32)> {
    let state = &battle.units[target];
    state.debuffs.iter().find_map(|status| match status.kind {
        DebuffKind::ImplantedWeakness(element) if status.source == source => status
            .remaining_turns(state.normal_turns)
            .map(|turns| (element, status.bp, turns)),
        _ => None,
    })
}

impl SilverWolf {
    fn select_weakness(&self, battle: &mut Battle<'_>, target: usize) -> Option<Element> {
        let mut elements = Vec::new();
        for element in battle
            .allies()
            .filter(|&ally| battle.is_targetable(ally))
            .filter_map(|ally| battle.plan.units[ally].combat_type.map(|kind| kind.element))
        {
            if !elements.contains(&element) {
                elements.push(element);
            }
        }
        let native = &battle.plan.units[target].kit;
        let missing: Vec<_> = elements
            .iter()
            .copied()
            .filter(|&element| !native.matches_weakness(element))
            .collect();
        // A successful repeated Weakness counts as one debuff without changing native RES.
        let candidates = if missing.is_empty() {
            &elements
        } else {
            &missing
        };
        let first = battle.plan.units[0].combat_type.map(|kind| kind.element);
        if self.enhanced
            && battle.is_targetable(0)
            && first.is_some_and(|element| candidates.contains(&element))
        {
            return first;
        }
        match candidates.as_slice() {
            [] => None,
            [element] => Some(*element),
            elements => Some(elements[battle.rng.below(elements.len() as u64) as usize]),
        }
    }

    fn implant(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        element: Element,
        turns: u32,
    ) {
        let native_weakness = battle.plan.units[target].kit.matches_weakness(element);
        battle.apply_debuff(
            owner,
            target,
            DebuffKind::ImplantedWeakness(element),
            self.implant_res_down_bp * u32::from(!native_weakness),
            turns,
        );
    }

    fn apply_skill_debuffs(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let a6 = !self.enhanced && self.side_note && battle.debuff_count(target) >= 3;
        if battle.roll_effect(owner, target, self.implant_chance_bp)
            && let Some(element) = self.select_weakness(battle, target)
        {
            let turns = 2 + u32::from(self.enhanced || self.inject);
            self.implant(battle, owner, target, element, turns);
        }
        if battle.roll_effect(owner, target, self.all_res_chance_bp) {
            battle.apply_debuff(
                owner,
                target,
                DebuffKind::AllResistanceDown,
                self.all_res_down_bp + 300 * u32::from(a6),
                2,
            );
        }
    }

    fn apply_ultimate_debuff(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if battle.roll_effect(owner, target, self.defense_chance_bp) {
            battle.apply_defense_down(owner, target, self.defense_down_bp, 3);
        }
    }

    fn apply_bug(&self, battle: &mut Battle<'_>, owner: usize, target: usize, chance_bp: u32) {
        if !battle.is_targetable(target) || !battle.roll_effect(owner, target, chance_bp) {
            return;
        }
        let missing: Vec<_> = self
            .bugs
            .iter()
            .filter(|&&(kind, _)| {
                !battle.units[target]
                    .debuffs
                    .iter()
                    .any(|status| status.source == owner && status.kind == kind)
            })
            .copied()
            .collect();
        let candidates = if missing.is_empty() {
            &self.bugs[..]
        } else {
            &missing
        };
        let index = if candidates.len() == 1 {
            0
        } else {
            battle.rng.below(candidates.len() as u64) as usize
        };
        let (kind, bp) = candidates[index];
        battle.apply_debuff(owner, target, kind, bp, 3 + u32::from(self.generate));
    }
}

impl Mechanics for SilverWolf {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.enhanced && self.inject {
            battle.gain_energy(actor, actor, 20_000);
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if actor == owner && self.enhanced && self.inject {
            battle.gain_energy(owner, owner, 5_000);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 2 {
            let kind = if self.enhanced {
                DebuffKind::Vulnerability
            } else {
                DebuffKind::EffectResistanceDown
            };
            for target in battle.enemies() {
                battle.apply_permanent_debuff(owner, target, kind, 2_000);
            }
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.apply_skill_debuffs(battle, actor, target),
            (Action::Ultimate, Some(target)) => self.apply_ultimate_debuff(battle, actor, target),
            (Action::Ultimate, None) if self.enhanced => {
                for target in battle.enemies() {
                    if battle.is_targetable(target) {
                        self.apply_ultimate_debuff(battle, actor, target);
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
        _target: Option<usize>,
    ) {
        if action != Action::Ultimate || self.eidolon == 0 {
            return;
        }
        // The action's pre-hit DEF reduction and Weakness Break count, its Talent Bug comes later.
        let targets: Vec<_> = battle
            .attack_targets()
            .iter()
            .map(|&target| (target, battle.debuff_count(target).min(MAX_EIDOLON_DEBUFFS)))
            .collect();
        let energy_triggers = targets.iter().map(|&(_, count)| count).sum::<usize>();
        battle.gain_energy(
            actor,
            actor,
            7_000 * energy_triggers.min(MAX_EIDOLON_DEBUFFS) as u32,
        );
        if self.eidolon >= 4 {
            for (target, count) in targets {
                for _ in 0..count {
                    battle.secondary_hit(
                        actor,
                        target,
                        QUANTUM,
                        2_000,
                        DamageKind::Additional,
                        action,
                    );
                }
            }
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        _origin: Action,
    ) {
        let targets = battle.attack_targets().to_vec();
        for target in targets {
            if owner == source {
                self.apply_bug(battle, owner, target, self.bug_chance_bp);
            }
            if self.enhanced && self.eidolon >= 2 {
                self.apply_bug(battle, owner, target, 10_000);
            }
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _source: usize,
        target: usize,
        _origin: Action,
    ) {
        if self.generate {
            self.apply_bug(
                battle,
                owner,
                target,
                if self.enhanced { 10_000 } else { 6_500 },
            );
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if !self.enhanced {
            return;
        }
        let Some((element, bp, turns)) = implant_on(battle, owner, target) else {
            return;
        };
        if bp == 0 || turns == 0 {
            return;
        }
        if let Some(recipient) = battle
            .enemies()
            .filter(|&other| {
                battle.is_targetable(other) && implant_on(battle, owner, other).is_none()
            })
            .min_by_key(|&other| (!battle.is_elite(other), other))
        {
            self.implant(battle, owner, recipient, element, turns);
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.enhanced && self.side_note && owner == target {
            (battle.effective_effect_hit_rate_bp(owner) / 1_000 * 1_000).min(5_000)
        } else {
            0
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        if self.eidolon == 6 {
            2_000 * battle.debuff_count(target).min(MAX_EIDOLON_DEBUFFS) as u32
        } else {
            0
        }
    }
}
