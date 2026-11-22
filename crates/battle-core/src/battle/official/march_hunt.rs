use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, CombatType, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "march_7th.hunt";
const BASIC: &str = "skill.122401";
const SKILL: &str = "skill.122402";
const ULTIMATE: &str = "skill.122403";
const TALENT: &str = "skill.122404";
const ENHANCED: &str = "skill.122408";
const SWAN_SOAR: &str = "trace.1224101";
const FILIGREE: &str = "trace.1224102";
const TIDE_TAMER: &str = "trace.1224103";
const CHARGE_THRESHOLD: u8 = 7;
const CHARGE_CAP: u8 = 10;

#[derive(Clone, Default)]
pub(in crate::battle) struct MarchHuntState {
    shifu: Option<usize>,
    charge: u8,
    ultimate_armed: bool,
    last_follow_up_turn: Option<u32>,
}

struct MarchHunt {
    enhanced: OfficialAbility,
    empowered: OfficialAbility,
    shifu_speed_bp: u16,
    shifu_additional_bp: u16,
    shifu_toughness_bp: u16,
    enhanced_damage_bp: u16,
    swan_soar: bool,
    filigree: bool,
    tide_tamer: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110
        || input.combat_type.path != CombatPath::Hunt
        || input.combat_type.element != Element::Imaginary
    {
        return Err(invalid("energy capacity or combat type"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 3)?;
    let enhanced = input.parameters(ENHANCED, 4)?;
    if basic[1] != 1.0
        || talent[0] != f64::from(CHARGE_THRESHOLD)
        || talent[2] != f64::from(CHARGE_CAP)
        || enhanced[2] != 3.0
        || enhanced[3] != 3.0
        || ultimate[1] != 2.0
        || enhanced[1] > 1.0
        || enhanced[1] + ultimate[2] > 1.0
    {
        return Err(invalid("charge, enhanced hit count or chance"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let enhanced_multiplier = basis_points(ENHANCED, enhanced[0])?;
    Ok(OfficialKit {
        max_energy_milli: 110_000,
        basic: single_attack(
            basis_points(BASIC, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: OfficialAbility {
            target: Target::OtherAlly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: single_attack(
            basis_points(ULTIMATE, ultimate[0])?,
            30,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(MarchHunt {
            enhanced: enhanced_attack(enhanced_multiplier, 3, basis_points(ENHANCED, enhanced[1])?),
            empowered: enhanced_attack(
                enhanced_multiplier,
                5,
                basis_points(ULTIMATE, enhanced[1] + ultimate[2])?,
            ),
            shifu_speed_bp: basis_points(SKILL, skill[0])?,
            shifu_additional_bp: basis_points(SKILL, skill[1])?,
            shifu_toughness_bp: basis_points(SKILL, skill[2])?,
            enhanced_damage_bp: basis_points(TALENT, talent[1])?,
            swan_soar: trace(SWAN_SOAR),
            filigree: trace(FILIGREE),
            tide_tamer: trace(TIDE_TAMER),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(&format!("{skill} multiplier")));
    }
    Ok(bp as u16)
}

fn single_attack(
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Imaginary,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn enhanced_attack(multiplier_bp: u16, initial_hits: u8, chance_bp: u16) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli: 30_000,
        effects: vec![Effect::RepeatingSingleHits {
            element: Element::Imaginary,
            multiplier_bp,
            toughness: 5,
            initial_hits,
            extra_hits: 3,
            extra_chance_bp: u32::from(chance_bp),
        }],
        skill_points: SkillPoints::Unchanged,
    }
}

impl MarchHuntState {
    fn enhanced(&self) -> bool {
        self.charge >= CHARGE_THRESHOLD
    }
}

impl MarchHunt {
    fn active_shifu(battle: &Battle<'_>, actor: usize) -> Option<usize> {
        battle.units[actor]
            .march_hunt_state
            .shifu
            .filter(|&shifu| battle.is_targetable(shifu))
    }

    fn shifu_combat_type(battle: &Battle<'_>, actor: usize) -> Option<CombatType> {
        Self::active_shifu(battle, actor).and_then(|shifu| battle.plan.units[shifu].combat_type)
    }

    fn add_charge(battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].march_hunt_state;
        let was_enhanced = state.enhanced();
        state.charge = (state.charge + 1).min(CHARGE_CAP);
        if !was_enhanced && state.enhanced() {
            battle.advance_immediate(actor);
        }
    }

    fn add_shifu_hit(&self, battle: &mut Battle<'_>, actor: usize, target: usize, origin: Action) {
        if let Some(shifu) = Self::shifu_combat_type(battle, actor)
            && !toughness_shifu(shifu.path)
        {
            battle.secondary_hit(
                actor,
                target,
                shifu.element,
                self.shifu_additional_bp,
                DamageKind::Additional,
                origin,
            );
        }
    }
}

fn toughness_shifu(path: CombatPath) -> bool {
    match path {
        CombatPath::Harmony
        | CombatPath::Nihility
        | CombatPath::Preservation
        | CombatPath::Abundance => true,
        CombatPath::Hunt
        | CombatPath::Destruction
        | CombatPath::Erudition
        | CombatPath::Remembrance
        | CombatPath::Elation => false,
    }
}

impl Mechanics for MarchHunt {
    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !battle.units[actor].march_hunt_state.enhanced()
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        let state = &battle.units[actor].march_hunt_state;
        (action == Action::Basic && state.enhanced()).then_some(if state.ultimate_armed {
            &self.empowered
        } else {
            &self.enhanced
        })
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::ElationSkill => {}
            Action::Skill => battle.units[actor].march_hunt_state.shifu = target,
            Action::Ultimate | Action::EnhancedUltimate => {
                battle.units[actor].march_hunt_state.ultimate_armed = true;
            }
            Action::Basic => {
                let state = &mut battle.units[actor].march_hunt_state;
                if state.enhanced() {
                    state.charge -= CHARGE_THRESHOLD;
                    state.ultimate_armed = false;
                    if self.tide_tamer
                        && let Some(shifu) = Self::active_shifu(battle, actor)
                    {
                        battle.apply_ally_buff(actor, shifu, AllyBuffKind::CritDamage, 6_000, 2);
                        battle.apply_ally_buff(actor, shifu, AllyBuffKind::BreakEffect, 3_600, 2);
                    }
                } else {
                    Self::add_charge(battle, actor);
                }
            }
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _: u32,
    ) {
        if action == Action::Basic {
            self.add_shifu_hit(battle, actor, target, action);
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        if battle.units[owner].march_hunt_state.shifu != Some(source) {
            return;
        }
        // An Ultimate always counts through the action hook, including an Ultimate with no attack.
        if battle.in_follow_up() || !origin.is_ultimate() {
            Self::add_charge(battle, owner);
        }
        if self.eidolon < 2
            || battle.in_follow_up()
            || origin.is_ultimate()
            || battle.is_frozen(owner)
            || battle.attack_targets().is_empty()
            || !battle.enemies().any(|target| battle.is_targetable(target))
        {
            return;
        }
        let unit = &mut battle.units[owner];
        if unit.march_hunt_state.last_follow_up_turn == Some(unit.normal_turns) {
            return;
        }
        unit.march_hunt_state.last_follow_up_turn = Some(unit.normal_turns);
        battle.follow_up_single_or_random(
            owner,
            battle.current_target(),
            origin,
            FollowUpAttack {
                element: Element::Imaginary,
                multiplier_bp: 6_000,
                toughness: 5,
                energy_milli: 5_000,
                first_hit: Some((2_400, 2)),
            },
        );
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        action: Action,
    ) {
        if action.is_ultimate() && battle.units[owner].march_hunt_state.shifu == Some(source) {
            Self::add_charge(battle, owner);
        }
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
    ) {
        self.add_shifu_hit(battle, actor, target, origin);
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _: Action) {
        Self::add_charge(battle, actor);
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.swan_soar {
            battle.advance(actor, 2_500);
        }
    }

    fn alternate_weakness(&self, battle: &Battle<'_>, actor: usize) -> Option<Element> {
        if self.filigree {
            Self::shifu_combat_type(battle, actor).map(|shifu| shifu.element)
        } else {
            None
        }
    }

    fn toughness_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if ((kind == DamageKind::Direct && action == Some(Action::Basic))
            || kind == DamageKind::FollowUp)
            && Self::shifu_combat_type(battle, actor)
                .is_some_and(|shifu| toughness_shifu(shifu.path))
        {
            u32::from(self.shifu_toughness_bp)
        } else {
            0
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if battle.units[actor].march_hunt_state.enhanced() {
            u32::from(self.enhanced_damage_bp)
        } else {
            0
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].march_hunt_state;
        if self.eidolon >= 6
            && state.ultimate_armed
            && state.enhanced()
            && action == Some(Action::Basic)
            && kind == DamageKind::Direct
        {
            5_000
        } else {
            0
        }
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if self.eidolon >= 1 && Self::active_shifu(battle, actor).is_some() {
            1_000
        } else {
            0
        }
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if battle.units[owner].march_hunt_state.shifu == Some(target) {
            u32::from(self.shifu_speed_bp)
        } else {
            0
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.eidolon >= 4 && owner == actor {
            battle.gain_energy(actor, actor, 5_000);
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if action != Action::Skill || Self::active_shifu(battle, actor).is_some() {
            return None;
        }
        let preferred = battle
            .allies()
            .filter(|&unit| unit != actor && battle.is_targetable(unit))
            .max_by_key(|&unit| (battle.speed_micros(unit), std::cmp::Reverse(unit)));
        if preferred.is_some() && preferred == target {
            Some(2)
        } else {
            None
        }
    }
}
