use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, SummonTurn, combat_status::DebuffKind, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1204";
const BASIC: &str = "skill.120401";
const SKILL: &str = "skill.120402";
const ULTIMATE: &str = "skill.120403";
const TALENT: &str = "skill.120404";
const BATTALIA_CRUSH: &str = "trace.1204101";
const SAVANT_PROVIDENCE: &str = "trace.1204102";
const WAR_MARSHAL: &str = "trace.1204103";

const BASE_HITS: u8 = 3;
const MAX_HITS: u8 = 10;
const BASE_SPEED_MICROS: u64 = 60_000_000;
const SPEED_PER_HIT_MICROS: u64 = 10_000_000;
const SKILL_ADDED_HITS: u8 = 2;
const ULTIMATE_ADDED_HITS: u8 = 3;
const E6_VULNERABILITY_BP: u32 = 1_200;
const E6_MAX_VULNERABILITY_BP: u32 = 3 * E6_VULNERABILITY_BP;

#[derive(Clone, Default)]
pub(in crate::battle) struct JingYuanState {
    lightning_lord_hits: u8,
    lightning_lord_attacking: bool,
    crit_rate_expires_after_turn: u32,
    direct_damage_expires_after_turn: u32,
}

struct JingYuan {
    eidolon: u8,
    battalia_crush: bool,
    savant_providence: bool,
    war_marshal: bool,
    follow_up: FollowUpAttack,
    adjacent_multiplier_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 6)?;
    if skill[1] != f64::from(SKILL_ADDED_HITS) || ultimate[1] != f64::from(ULTIMATE_ADDED_HITS) {
        return Err(invalid("skill or ultimate hit count"));
    }
    if talent[0] != 60.0
        || talent[2] != 10.0
        || talent[3] != f64::from(BASE_HITS)
        || talent[4] != 0.25
        || talent[5] != f64::from(MAX_HITS)
    {
        return Err(invalid(TALENT));
    }
    let enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let adjacent_ratio = talent[4] + if input.eidolon >= 1 { 0.25 } else { 0.0 };
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: thunder_attack(
            BASIC,
            basic[0],
            10,
            20_000,
            Target::Enemy,
            SkillPoints::Generate,
        )?,
        skill: thunder_attack(
            SKILL,
            skill[0],
            10,
            30_000,
            Target::AllEnemies,
            SkillPoints::Spend,
        )?,
        ultimate: thunder_attack(
            ULTIMATE,
            ultimate[0],
            20,
            5_000,
            Target::AllEnemies,
            SkillPoints::Unchanged,
        )?,
        mechanics: Arc::new(JingYuan {
            eidolon: input.eidolon,
            battalia_crush: enabled(BATTALIA_CRUSH),
            savant_providence: enabled(SAVANT_PROVIDENCE),
            war_marshal: enabled(WAR_MARSHAL),
            follow_up: FollowUpAttack {
                element: Element::Thunder,
                multiplier_bp: basis_points(TALENT, talent[1])?,
                toughness: 5,
                energy_milli: u32::from(input.eidolon >= 4) * 2_000,
                first_hit: None,
            },
            adjacent_multiplier_bp: basis_points(TALENT, talent[1] * adjacent_ratio)?,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn thunder_attack(
    skill: &str,
    ratio: f64,
    toughness: u16,
    energy_milli: u32,
    target: Target,
    skill_points: SkillPoints,
) -> Result<OfficialAbility, BattleError> {
    Ok(OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Thunder,
            multiplier_bp: basis_points(skill, ratio)?,
            toughness,
        }],
        skill_points,
    })
}

impl JingYuan {
    fn set_hits(battle: &mut Battle<'_>, owner: usize, hits: u8) {
        let unit = &mut battle.units[owner];
        let hits = hits.min(MAX_HITS);
        unit.jing_yuan_state.lightning_lord_hits = hits;
        if let Some(summon) = &mut unit.summon_turn {
            // Speed changes preserve the action distance already covered.
            summon.speed_micros =
                BASE_SPEED_MICROS + u64::from(hits - BASE_HITS) * SPEED_PER_HIT_MICROS;
        }
    }
}

impl Mechanics for JingYuan {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].summon_turn =
            Some(SummonTurn::new("lightning-lord", BASE_SPEED_MICROS));
        Self::set_hits(battle, actor, BASE_HITS);
        if self.savant_providence {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let added_hits = match action {
            Action::Basic => return,
            Action::Skill => SKILL_ADDED_HITS,
            Action::Ultimate | Action::EnhancedUltimate => ULTIMATE_ADDED_HITS,
        };
        let unit = &mut battle.units[actor];
        if action == Action::Skill && self.war_marshal {
            unit.jing_yuan_state.crit_rate_expires_after_turn = unit.normal_turns + 2;
        }
        let hits = unit.jing_yuan_state.lightning_lord_hits + added_hits;
        Self::set_hits(battle, actor, hits);
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        if battle.is_crowd_controlled(owner) {
            return;
        }
        let state = &mut battle.units[owner].jing_yuan_state;
        state.lightning_lord_attacking = true;
        let hits = state.lightning_lord_hits;
        // Autonomous attacks use the existing Skill action field and FollowUp damage kind.
        battle.follow_up_random_blast(
            owner,
            Action::Skill,
            hits,
            Some(self.adjacent_multiplier_bp),
            self.follow_up.clone(),
        );
        battle.resolve_follow_ups();
        battle.units[owner].jing_yuan_state.lightning_lord_attacking = false;
        Self::set_hits(battle, owner, BASE_HITS);
    }

    fn damage_comes_from_summon(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _origin: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        battle.units[actor].jing_yuan_state.lightning_lord_attacking
            && (kind == DamageKind::FollowUp || kind.is_break())
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _origin: Action,
    ) {
        if self.eidolon < 6 || !battle.units[actor].jing_yuan_state.lightning_lord_attacking {
            return;
        }
        let stacks_bp = battle.units[target]
            .debuffs
            .iter()
            .find(|status| status.source == actor && status.kind == DebuffKind::Vulnerability)
            .map_or(0, |status| status.bp);
        if stacks_bp < E6_MAX_VULNERABILITY_BP {
            battle.apply_debuff(
                actor,
                target,
                DebuffKind::Vulnerability,
                stacks_bp + E6_VULNERABILITY_BP,
                0,
            );
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        let unit = &mut battle.units[actor];
        if !unit.jing_yuan_state.lightning_lord_attacking {
            return;
        }
        if self.eidolon >= 2 {
            unit.jing_yuan_state.direct_damage_expires_after_turn = unit.finished_turns + 2;
        }
        if self.eidolon >= 6 {
            for target in battle.enemies() {
                battle.remove_debuff(actor, target, DebuffKind::Vulnerability);
            }
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        let unit = &battle.units[actor];
        1_000 * u32::from(unit.finished_turns < unit.jing_yuan_state.crit_rate_expires_after_turn)
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].jing_yuan_state;
        2_500
            * u32::from(
                self.battalia_crush
                    && kind == DamageKind::FollowUp
                    && state.lightning_lord_attacking
                    && state.lightning_lord_hits >= 6,
            )
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let unit = &battle.units[actor];
        2_000
            * u32::from(
                kind == DamageKind::Direct
                    && action.is_some()
                    && unit.finished_turns < unit.jing_yuan_state.direct_damage_expires_after_turn,
            )
    }
}
