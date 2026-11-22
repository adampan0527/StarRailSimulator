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

pub(super) const FORM: &str = "character.1315";
const BASIC: &str = "skill.131501";
const SKILL: &str = "skill.131502";
const ULTIMATE: &str = "skill.131503";
const TALENT: &str = "skill.131504";
const ENHANCED_BASIC: &str = "skill.131508";
const GHOST_LOAD: &str = "trace.1315101";
const ABOVE_SNAKES: &str = "trace.1315102";
const POINT_BLANK: &str = "trace.1315103";
const E2: &str = "eidolon.131502";
const ENERGY_CAP: u32 = 115;
const BASIC_TOUGHNESS: u16 = 10;
const POCKET_CAP: u8 = 3;
const POCKET_TOUGHNESS_BP: u32 = 5_000;
const BREAK_TOUGHNESS_CAP: u16 = 16 * BASIC_TOUGHNESS;

#[derive(Clone, Copy)]
struct Standoff {
    target: usize,
    expires_on_turn: u32,
    won: bool,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct BoothillState {
    standoff: Option<Standoff>,
    pockets: u8,
    last_e2_turn: Option<u32>,
}

struct Boothill {
    enhanced_basic: OfficialAbility,
    standoff_turns: u32,
    enemy_vulnerability_bp: u32,
    self_vulnerability_bp: u32,
    weakness_turns: u32,
    ultimate_delay_bp: u16,
    pocket_damage_bp: [u16; POCKET_CAP as usize],
    eidolon: u8,
    ghost_load: bool,
    above_snakes: bool,
    point_blank: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Hunt
        || input.combat_type.element != Element::Physical
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 6)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 1)?;
    if skill[2] != 2.0 || ultimate[2] != 2.0 {
        return Err(invalid("Standoff and Physical Weakness durations"));
    }
    if talent[3] != 0.5 || talent[4] != f64::from(POCKET_CAP) || talent[5] != 16.0 {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    let e4 = 1_200 * u32::from(input.eidolon >= 4);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: hit(
            basis_points(BASIC, basic[0])?,
            BASIC_TOUGHNESS,
            20_000,
            SkillPoints::Generate,
        ),
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: hit(
            basis_points(ULTIMATE, ultimate[0])?,
            30,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Boothill {
            enhanced_basic: hit(
                basis_points(ENHANCED_BASIC, enhanced[0])?,
                20,
                30_000,
                SkillPoints::Unchanged,
            ),
            standoff_turns: skill[2] as u32,
            enemy_vulnerability_bp: u32::from(basis_points(SKILL, skill[0])?) + e4,
            self_vulnerability_bp: u32::from(basis_points(SKILL, skill[1])?).saturating_sub(e4),
            weakness_turns: ultimate[2] as u32,
            ultimate_delay_bp: basis_points(ULTIMATE, ultimate[1])?,
            pocket_damage_bp: [
                basis_points(TALENT, talent[0])?,
                basis_points(TALENT, talent[1])?,
                basis_points(TALENT, talent[2])?,
            ],
            eidolon: input.eidolon,
            ghost_load: trace(GHOST_LOAD),
            above_snakes: trace(ABOVE_SNAKES),
            point_blank: trace(POINT_BLANK),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit(
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Physical,
            multiplier_bp,
            toughness,
        }],
        skill_points: points,
    }
}

fn live_standoff(battle: &Battle<'_>, owner: usize) -> Option<Standoff> {
    battle.units[owner]
        .boothill_state
        .standoff
        .filter(|duel| !duel.won && battle.is_targetable(duel.target))
}

fn clear_standoff(battle: &mut Battle<'_>, owner: usize) {
    if let Some(duel) = battle.units[owner].boothill_state.standoff.take() {
        battle.remove_debuff(owner, duel.target, DebuffKind::Standoff);
    }
}

fn clear_won_standoff(battle: &mut Battle<'_>, owner: usize) {
    if battle.units[owner]
        .boothill_state
        .standoff
        .is_some_and(|duel| duel.won)
    {
        clear_standoff(battle, owner);
    }
}

fn gain_pocket(battle: &mut Battle<'_>, owner: usize, source: usize) {
    let pockets = &mut battle.units[owner].boothill_state.pockets;
    let before = *pockets;
    let after = before.saturating_add(1).min(POCKET_CAP);
    *pockets = after;
    if before != after {
        battle.emit(
            EventKind::PocketTrickshot { before, after },
            Some(source),
            Some(owner),
        );
    }
}

impl Boothill {
    fn begin_standoff(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        battle.units[owner].boothill_state.standoff = Some(Standoff {
            target,
            expires_on_turn: battle.units[owner]
                .normal_turns
                .saturating_add(self.standoff_turns),
            won: false,
        });
        battle.apply_permanent_debuff(
            owner,
            target,
            DebuffKind::Standoff,
            self.enemy_vulnerability_bp,
        );
    }

    fn win_standoff(&self, battle: &mut Battle<'_>, owner: usize, source: usize, target: usize) {
        let Some(duel) = &mut battle.units[owner].boothill_state.standoff else {
            return;
        };
        if duel.target != target {
            return;
        }
        // A true break and a defeat can each award a stack before the same attack ends.
        duel.won = true;
        gain_pocket(battle, owner, source);
        if self.point_blank {
            battle.gain_energy(owner, owner, 10_000);
        }
        let turn = battle.normal_turns;
        if self.eidolon >= 2 && battle.units[owner].boothill_state.last_e2_turn != Some(turn) {
            battle.units[owner].boothill_state.last_e2_turn = Some(turn);
            battle.restore_skill_points(owner, 1);
            battle.apply_ally_buff_with_id(
                owner,
                owner,
                AllyBuffKind::BreakEffect,
                3_000,
                2,
                Some(E2),
            );
        }
        if battle.action.is_none() && !battle.in_follow_up() {
            clear_standoff(battle, owner);
        }
    }

    fn proc_talent(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let state = &battle.units[owner].boothill_state;
        if state.standoff.is_none_or(|duel| duel.target != target)
            || battle.units[target].toughness != 0
        {
            return;
        }
        let Some(ratio) = state
            .pockets
            .checked_sub(1)
            .map(|slot| self.pocket_damage_bp[usize::from(slot)])
        else {
            return;
        };
        let sides = battle.adjacent_enemies(target);
        self.pocket_break(battle, owner, target, ratio);
        if self.eidolon >= 6 {
            self.pocket_break(battle, owner, target, (u32::from(ratio) * 4 / 10) as u16);
            for adjacent in sides {
                self.pocket_break(battle, owner, adjacent, (u32::from(ratio) * 7 / 10) as u16);
            }
        }
    }

    fn pocket_break(&self, battle: &mut Battle<'_>, actor: usize, target: usize, ratio_bp: u16) {
        battle.break_hit_capped(
            actor,
            target,
            Element::Physical,
            Action::Basic,
            ratio_bp,
            BREAK_TOUGHNESS_CAP,
        );
    }

    fn preferred_duel(&self, battle: &Battle<'_>, owner: usize) -> Option<usize> {
        let enemies = battle
            .enemies()
            .filter(|&target| battle.is_targetable(target));
        if battle.units[owner].boothill_state.pockets < POCKET_CAP {
            enemies.min_by_key(|&target| {
                let toughness = battle.units[target].toughness;
                (
                    toughness == 0,
                    !battle.has_weakness(target, Element::Physical),
                    toughness,
                )
            })
        } else {
            enemies.min_by_key(|&target| {
                (
                    battle.units[target].toughness != 0,
                    !battle.has_weakness(target, Element::Physical),
                    Reverse(battle.plan.units[target].kit.max_toughness()),
                )
            })
        }
    }
}

impl Mechanics for Boothill {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 1 {
            gain_pocket(battle, owner, owner);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner == actor
            && !extra
            && battle.units[owner]
                .boothill_state
                .standoff
                .is_some_and(|duel| duel.expires_on_turn <= battle.units[owner].normal_turns)
        {
            clear_standoff(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        clear_standoff(battle, owner);
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        live_standoff(battle, actor).is_none()
    }

    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && live_standoff(battle, actor).is_some())
            .then_some(&self.enhanced_basic)
    }

    fn forced_action_target(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<usize> {
        if action == Action::Basic {
            return live_standoff(battle, actor).map(|duel| duel.target);
        }
        None
    }

    fn taunts_enemy(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> bool {
        live_standoff(battle, owner).is_some_and(|duel| duel.target == enemy)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.begin_standoff(battle, actor, target),
            (Action::Ultimate, Some(target)) => {
                battle.implant_weakness(actor, target, Element::Physical, self.weakness_turns);
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
        match (action, target) {
            (Action::Basic, Some(target)) => self.proc_talent(battle, actor, target),
            (Action::Ultimate, Some(target)) if battle.is_targetable(target) => {
                battle.delay_action(actor, target, self.ultimate_delay_bp);
            }
            _ => {}
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
        _: Action,
    ) {
        self.win_standoff(battle, owner, source, target);
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, source: usize, target: usize) {
        self.win_standoff(battle, owner, source, target);
    }

    fn on_attack_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize, _: Action) {
        clear_won_standoff(battle, owner);
    }

    fn on_ally_action_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize, _: Action) {
        clear_won_standoff(battle, owner);
    }

    fn toughness_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if action == Some(Action::Basic)
            && kind == DamageKind::Direct
            && live_standoff(battle, actor).is_some()
        {
            u32::from(battle.units[actor].boothill_state.pockets) * POCKET_TOUGHNESS_BP
        } else {
            0
        }
    }

    fn crit_from_break_effect_bp(&self, break_effect_bp: u32) -> (u32, u32) {
        if self.ghost_load {
            (
                (break_effect_bp / 10).min(3_000),
                (break_effect_bp / 2).min(15_000),
            )
        } else {
            (0, 0)
        }
    }

    fn incoming_vulnerability_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        source: usize,
        kind: DamageKind,
    ) -> u32 {
        if kind == DamageKind::Direct && self.taunts_enemy(battle, owner, source) {
            self.self_vulnerability_bp
        } else {
            0
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        source: usize,
        _: DamageKind,
    ) -> u32 {
        if self.above_snakes
            && battle.enemies().contains(&source)
            && live_standoff(battle, owner).is_some_and(|duel| duel.target != source)
        {
            3_000
        } else {
            0
        }
    }

    fn defense_ignore_bp(&self, _: &Battle<'_>, _: usize, _: usize) -> u32 {
        1_600 * u32::from(self.eidolon >= 1)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Skill | Action::EnhancedSkill => {
                Some(if target == self.preferred_duel(battle, owner) {
                    1
                } else {
                    5
                })
            }
            Action::Ultimate => {
                let duel = live_standoff(battle, owner);
                let preferred = duel
                    .map(|duel| duel.target)
                    .or_else(|| self.preferred_duel(battle, owner));
                if target != preferred {
                    Some(5)
                } else if duel.is_none()
                    && battle.ordinary_actor == Some(owner)
                    && battle.skill_points > 0
                {
                    Some(2)
                } else {
                    Some(0)
                }
            }
            Action::Basic | Action::EnhancedUltimate => None,
        }
    }
}
