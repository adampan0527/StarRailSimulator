use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1004";
const IMAGINARY: Element = Element::Imaginary;
const RETRIBUTION: &str = "welt.retribution";

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    retribution: &'static str,
    judgement: &'static str,
    punishment: &'static str,
}

const BASE: KitIds = KitIds {
    basic: "skill.100401",
    skill: "skill.100402",
    ultimate: "skill.100403",
    talent: "skill.100404",
    retribution: "trace.1004101",
    judgement: "trace.1004102",
    punishment: "trace.1004103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1100401",
    skill: "skill.1100402",
    ultimate: "skill.1100403",
    talent: "skill.1100404",
    retribution: "trace.11004101",
    judgement: "trace.11004102",
    punishment: "trace.11004103",
};

#[derive(Clone, Default)]
pub(in crate::battle) struct WeltState {
    honor_charges: u8,
    target_was_slowed: bool,
    own_additional_hit: bool,
    weightless_delays: BTreeMap<usize, (u32, u8)>,
}

struct Weightless {
    delay_bp: u16,
    delays_per_turn: u8,
    turns: u32,
    defense_down_bp: u16,
    slow_bp: u16,
}

struct Welt {
    eidolon: u8,
    retribution: bool,
    judgement: bool,
    punishment: bool,
    weightless: Option<Weightless>,
    skill_energy_per_hit: u32,
    slow_chance_bp: u32,
    slow_bp: u32,
    slow_turns: u32,
    imprison_chance_bp: u32,
    imprison_delay_bp: u16,
    imprison_slow_bp: u32,
    talent_bp: u16,
    basic_extra_bp: u16,
    skill_extra_bp: u16,
    ultimate_extra_bp: u16,
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
    let basic_bp = ratio_bp(ids.basic, input.parameters(ids.basic, 1)?[0])?;
    let skill = input.parameters(ids.skill, 4)?;
    let skill_bp = ratio_bp(ids.skill, skill[0])?;
    let ultimate = input.parameters(ids.ultimate, if enhanced { 7 } else { 4 })?;
    let ultimate_bp = ratio_bp(ids.ultimate, ultimate[0])?;
    let talent = input.parameters(ids.talent, if enhanced { 3 } else { 1 })?;
    let weightless = if enhanced {
        if ultimate[5] != 8.0 || ultimate[6] != 2.0 {
            return Err(invalid(ids.ultimate));
        }
        Some(Weightless {
            delay_bp: ratio_bp(ids.ultimate, ultimate[4])?,
            delays_per_turn: 8,
            turns: 2,
            defense_down_bp: ratio_bp(ids.talent, talent[1])?,
            slow_bp: ratio_bp(ids.talent, talent[2])?,
        })
    } else {
        None
    };
    if skill[3] != 2.0 {
        return Err(invalid(ids.skill));
    }
    let trace_enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let judgement = trace_enabled(ids.judgement);
    let punishment = trace_enabled(ids.punishment);
    let extra_ultimate_energy = match (enhanced, judgement, punishment) {
        (false, true, _) => 10_000,
        (true, _, true) => 5_000,
        _ => 0,
    };
    let bounces = if enhanced {
        4
    } else {
        2 + u8::from(input.eidolon == 6)
    };

    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            skill_points: SkillPoints::Generate,
            energy_milli: 20_000,
            effects: vec![hit(basic_bp, 10)],
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            skill_points: SkillPoints::Spend,
            // Each completed bounce grants energy even when a previous target was defeated.
            energy_milli: 0,
            effects: vec![
                hit(skill_bp, 10),
                Effect::RandomHits {
                    element: IMAGINARY,
                    multiplier_bp: skill_bp,
                    toughness: 10,
                    count: bounces,
                },
            ],
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            skill_points: SkillPoints::Unchanged,
            energy_milli: 5_000 + extra_ultimate_energy,
            // The two hits each trigger Time Distortion on a slowed target.
            effects: vec![
                hit(ultimate_bp / 2, 10),
                hit(ultimate_bp - ultimate_bp / 2, 10),
            ],
        },
        mechanics: Arc::new(Welt {
            eidolon: input.eidolon,
            retribution: trace_enabled(ids.retribution),
            judgement,
            punishment,
            weightless,
            skill_energy_per_hit: if enhanced { 6_000 } else { 10_000 },
            slow_chance_bp: u32::from(ratio_bp(ids.skill, skill[1])?)
                + 3_500 * u32::from(!enhanced && input.eidolon >= 4),
            slow_bp: u32::from(ratio_bp(ids.skill, skill[2])?),
            slow_turns: 2,
            imprison_chance_bp: u32::from(ratio_bp(ids.ultimate, ultimate[2])?),
            imprison_delay_bp: ratio_bp(ids.ultimate, ultimate[1])?,
            imprison_slow_bp: u32::from(ratio_bp(ids.ultimate, ultimate[3])?),
            talent_bp: ratio_bp(ids.talent, talent[0])?,
            basic_extra_bp: scale_bp(basic_bp, if enhanced { 8_000 } else { 5_000 }),
            skill_extra_bp: scale_bp(skill_bp, if enhanced { 12_000 } else { 8_000 }),
            ultimate_extra_bp: scale_bp(ultimate_bp, 4_000),
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

fn scale_bp(multiplier: u16, factor: u16) -> u16 {
    u16::try_from(u32::from(multiplier) * u32::from(factor) / 10_000).unwrap_or(u16::MAX)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: IMAGINARY,
        multiplier_bp,
        toughness,
    }
}

fn additional(
    battle: &mut Battle<'_>,
    actor: usize,
    target: usize,
    multiplier_bp: u16,
    action: Action,
) {
    battle.units[actor].welt_state.own_additional_hit = true;
    battle.secondary_hit(
        actor,
        target,
        IMAGINARY,
        multiplier_bp,
        DamageKind::Additional,
        action,
    );
    battle.units[actor].welt_state.own_additional_hit = false;
}

impl Welt {
    fn use_honor_charge(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        if self.weightless.is_some() {
            return self.judgement;
        }
        let charges = &mut battle.units[actor].welt_state.honor_charges;
        if *charges == 0 {
            return false;
        }
        *charges -= 1;
        true
    }

    fn has_e6_crit(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        self.weightless.is_some()
            && self.eidolon == 6
            && matches!(action, Some(Action::Skill | Action::Ultimate))
            && match kind {
                DamageKind::Direct => battle.units[actor].welt_state.target_was_slowed,
                DamageKind::Additional => {
                    battle.units[actor].welt_state.own_additional_hit && battle.is_slowed(target)
                }
                _ => false,
            }
    }
}

impl Mechanics for Welt {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.weightless.is_some() && self.retribution {
            battle.gain_energy(actor, actor, 30_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if self.weightless.is_none() && self.retribution && action == Action::Ultimate {
            for target in battle
                .enemies()
                .filter(|&target| battle.is_targetable(target))
                .collect::<Vec<_>>()
            {
                if battle.roll_effect(actor, target, 10_000) {
                    battle.apply_vulnerability(actor, target, 1_200, 2);
                }
            }
        }
    }

    fn before_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _action: Action,
    ) {
        battle.units[actor].welt_state.target_was_slowed = battle.is_slowed(target);
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        let slowed = battle.units[actor].welt_state.target_was_slowed;
        if slowed && battle.is_targetable(target) {
            additional(battle, actor, target, self.talent_bp, action);
            if self.eidolon >= 2 {
                battle.gain_energy(actor, actor, 3_000);
            }
        }
        if action == Action::Skill {
            battle.gain_energy(actor, actor, self.skill_energy_per_hit);
            if battle.is_targetable(target)
                && battle.roll_effect(actor, target, self.slow_chance_bp)
            {
                battle.apply_speed_down(actor, target, self.slow_bp, self.slow_turns);
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
        match action {
            Action::Basic | Action::Skill | Action::EnhancedSkill => {
                if let Some(target) = target
                    && self.use_honor_charge(battle, actor)
                {
                    let multiplier = if action == Action::Basic {
                        self.basic_extra_bp
                    } else {
                        self.skill_extra_bp
                    };
                    additional(battle, actor, target, multiplier, action);
                }
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                if self.weightless.is_none() && self.eidolon >= 1 {
                    battle.units[actor].welt_state.honor_charges = 2;
                }
                let targets = battle.attack_targets().to_vec();
                for target in targets {
                    if battle.is_targetable(target)
                        && battle.roll_status(actor, target, self.imprison_chance_bp, true)
                    {
                        battle.apply_skill_imprisonment(
                            actor,
                            target,
                            self.imprison_slow_bp,
                            self.imprison_delay_bp,
                        );
                    }
                }
            }
        }
    }

    fn on_attack_target(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let Some(weightless) = &self.weightless else {
            return;
        };
        if !battle.has_weightless(owner, target) {
            return;
        }
        let turn = battle.units[target].normal_turns;
        let (counted_turn, delays) = battle.units[owner]
            .welt_state
            .weightless_delays
            .entry(target)
            .or_insert((turn, 0));
        if *counted_turn != turn {
            *counted_turn = turn;
            *delays = 0;
        }
        if *delays < weightless.delays_per_turn {
            *delays += 1;
            battle.delay_action(owner, target, weightless.delay_bp);
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        action: Action,
    ) {
        let Some(weightless) = &self.weightless else {
            return;
        };
        let own_action = owner == source && !battle.in_follow_up();
        let targets = battle.attack_targets().to_vec();
        if own_action && self.eidolon >= 1 && matches!(action, Action::Skill | Action::Ultimate) {
            for &target in &targets {
                if battle.has_weightless(owner, target) {
                    additional(battle, owner, target, self.ultimate_extra_bp, action);
                }
            }
        }
        if self.retribution
            && targets
                .iter()
                .any(|&target| battle.has_weightless(owner, target))
        {
            let stacked =
                (battle.ally_buff_value(owner, source, AllyBuffKind::Damage) + 1_000).min(10_000);
            battle.apply_ally_buff_with_id(
                owner,
                source,
                AllyBuffKind::Damage,
                stacked,
                2,
                Some(RETRIBUTION),
            );
        }
        if own_action && action == Action::Ultimate {
            for target in targets {
                battle.apply_debuff(
                    owner,
                    target,
                    DebuffKind::Weightless {
                        defense_down_bp: weightless.defense_down_bp,
                        resistance_down_bp: 3_000 * u16::from(self.eidolon >= 4),
                    },
                    u32::from(weightless.slow_bp),
                    weightless.turns,
                );
            }
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.weightless.is_some() && self.punishment && owner == target {
            let excess_bp = battle
                .effective_effect_hit_rate_bp(owner)
                .saturating_sub(4_000);
            (excess_bp / 1_000 * 2_000).min(8_000)
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
        2_000
            * u32::from(
                self.weightless.is_none() && self.punishment && battle.units[target].toughness == 0,
            )
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        3_000 * u32::from(self.has_e6_crit(battle, actor, target, action, kind))
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        6_000 * u32::from(self.has_e6_crit(battle, actor, target, action, kind))
    }
}
