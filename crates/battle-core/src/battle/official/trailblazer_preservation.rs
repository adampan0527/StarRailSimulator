use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "trailblazer.preservation";
const BASIC: &str = "skill.800301";
const SKILL: &str = "skill.800302";
const ULTIMATE: &str = "skill.800303";
const TALENT: &str = "skill.800304";
const ENHANCED_BASIC: &str = "skill.800308";

const MAGMA_WILL_COST: u8 = 4;
const MAGMA_WILL_CAP: u8 = 8;
const SHIELD_TURNS: u32 = 2;
const TAUNT_TURNS: u32 = 1;
const A2_REDUCTION_BP: u32 = 1_500;
const A4_HEAL_BP: u32 = 500;
const A6_ATTACK_BP: u32 = 1_500;
const A6_ENERGY_MILLI: u32 = 5_000;
const E6_DEFENSE_BP: u32 = 1_000;
const E6_STACK_CAP: u8 = 3;

#[derive(Clone, Default)]
pub(in crate::battle) struct PreservationTrailblazerState {
    magma_will: u8,
    free_enhanced_basic: bool,
    enhanced_basic_in_action: bool,
    personal_reduction_until: Option<u32>,
    shielded_turn: bool,
    defense_stacks: u8,
}

struct PreservationTrailblazer {
    basic_bp: u16,
    enhanced_main_bp: u16,
    enhanced_adjacent_bp: u16,
    enhanced_basic: OfficialAbility,
    ultimate_bp: [u16; 2],
    shield_defense_bp: u32,
    shield_flat: u32,
    skill_reduction_bp: u32,
    taunt_chance_bp: u32,
    eidolon: u8,
    team_reduction: bool,
    enhanced_heal: bool,
    shielded_attack: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Preservation
        || input.combat_type.element != Element::Fire
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let [basic] = input.parameters(BASIC, 1)? else {
        return Err(invalid(BASIC));
    };
    let [enhanced_main, enhanced_adjacent] = input.parameters(ENHANCED_BASIC, 2)? else {
        return Err(invalid(ENHANCED_BASIC));
    };
    let [reduction, taunt, taunt_turns] = input.parameters(SKILL, 3)? else {
        return Err(invalid(SKILL));
    };
    let [ultimate_attack, ultimate_defense] = input.parameters(ULTIMATE, 2)? else {
        return Err(invalid(ULTIMATE));
    };
    let [shield_defense, shield_turns, magma_will_cap, shield_flat] =
        input.parameters(TALENT, 4)?
    else {
        return Err(invalid(TALENT));
    };
    if *taunt_turns != f64::from(TAUNT_TURNS) || *reduction > 1.0 || *taunt > 1.0 {
        return Err(invalid(SKILL));
    }
    if *shield_turns != f64::from(SHIELD_TURNS)
        || *magma_will_cap != f64::from(MAGMA_WILL_CAP)
        || !(1.0..=f64::from(u32::MAX)).contains(shield_flat)
        || shield_flat.fract() != 0.0
    {
        return Err(invalid(TALENT));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let e2 = input.eidolon >= 2;
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: basic_ability(false),
        skill: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![Effect::DirectedAreaAttack],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(PreservationTrailblazer {
            basic_bp: ratio_bp(BASIC, *basic)?,
            enhanced_main_bp: ratio_bp(ENHANCED_BASIC, *enhanced_main)?,
            enhanced_adjacent_bp: ratio_bp(ENHANCED_BASIC, *enhanced_adjacent)?,
            enhanced_basic: basic_ability(true),
            ultimate_bp: [
                ratio_bp(ULTIMATE, *ultimate_attack)?,
                ratio_bp(ULTIMATE, *ultimate_defense)?,
            ],
            shield_defense_bp: u32::from(ratio_bp(TALENT, *shield_defense)?) + u32::from(e2) * 200,
            shield_flat: (*shield_flat as u32).saturating_add(u32::from(e2) * 27),
            skill_reduction_bp: u32::from(ratio_bp(SKILL, *reduction)?),
            taunt_chance_bp: u32::from(ratio_bp(SKILL, *taunt)?),
            eidolon: input.eidolon,
            team_reduction: trace("trace.8003101"),
            enhanced_heal: trace("trace.8003102"),
            shielded_attack: trace("trace.8003103"),
        }),
    })
}

fn invalid(parameter: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {parameter}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn basic_ability(enhanced: bool) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli: if enhanced { 30_000 } else { 20_000 },
        effects: vec![Effect::DirectedAttack { adjacent: enhanced }],
        skill_points: SkillPoints::Generate,
    }
}

fn enhanced_ready(battle: &Battle<'_>, actor: usize) -> bool {
    let state = &battle.units[actor].preservation_trailblazer_state;
    state.free_enhanced_basic || state.magma_will >= MAGMA_WILL_COST
}

fn set_magma_will(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let state = &mut battle.units[actor].preservation_trailblazer_state;
    let after = after.min(MAGMA_WILL_CAP);
    let before = std::mem::replace(&mut state.magma_will, after);
    if before != after {
        battle.emit(EventKind::MagmaWill { before, after }, Some(actor), None);
    }
}

fn gain_magma_will(battle: &mut Battle<'_>, actor: usize) {
    let before = battle.units[actor]
        .preservation_trailblazer_state
        .magma_will;
    set_magma_will(battle, actor, before + 1);
}

impl PreservationTrailblazer {
    fn use_skill(&self, battle: &mut Battle<'_>, actor: usize) {
        let unit = &mut battle.units[actor];
        unit.preservation_trailblazer_state.personal_reduction_until = Some(unit.normal_turns + 1);
        gain_magma_will(battle, actor);
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy)
                && battle.roll_status(actor, enemy, self.taunt_chance_bp, false)
            {
                battle.apply_debuff(actor, enemy, DebuffKind::Taunt, 0, TAUNT_TURNS);
            }
        }
        if self.team_reduction {
            for ally in battle.party_units() {
                if battle.is_targetable(ally) {
                    battle.apply_ally_buff(
                        actor,
                        ally,
                        AllyBuffKind::DamageReduction,
                        A2_REDUCTION_BP,
                        1,
                    );
                }
            }
        }
    }

    fn begin_basic(battle: &mut Battle<'_>, actor: usize) {
        if !enhanced_ready(battle, actor) {
            return;
        }
        let state = &mut battle.units[actor].preservation_trailblazer_state;
        state.enhanced_basic_in_action = true;
        if !std::mem::take(&mut state.free_enhanced_basic) {
            let remaining = state.magma_will - MAGMA_WILL_COST;
            set_magma_will(battle, actor, remaining);
        }
    }

    fn finish_enhanced_basic(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.enhanced_heal {
            let amount = percentage_of(battle.effective_max_hp(actor), A4_HEAL_BP);
            battle.heal_unit(actor, actor, amount);
        }
        self.gain_defense_stack(battle, actor);
    }

    fn gain_defense_stack(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 6 {
            let state = &mut battle.units[actor].preservation_trailblazer_state;
            state.defense_stacks = (state.defense_stacks + 1).min(E6_STACK_CAP);
        }
    }

    fn shield_team(&self, battle: &mut Battle<'_>, actor: usize) {
        let defense = battle.defense(actor);
        let amount =
            percentage_of(defense, self.shield_defense_bp).saturating_add(self.shield_flat);
        for ally in battle.party_units() {
            // All three actions refresh the same unstacked talent shield.
            battle.apply_official_shield(actor, ally, amount, SHIELD_TURNS, None);
        }
    }

    fn start_shielded_turn(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.shielded_attack && battle.shield_amount(actor) > 0 {
            battle.units[actor]
                .preservation_trailblazer_state
                .shielded_turn = true;
            battle.gain_energy(actor, actor, A6_ENERGY_MILLI);
        }
    }
}

impl Mechanics for PreservationTrailblazer {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 4 {
            set_magma_will(battle, actor, MAGMA_WILL_COST);
        }
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && enhanced_ready(battle, actor)).then_some(&self.enhanced_basic)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Basic {
            Self::begin_basic(battle, actor);
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        match battle.current_action() {
            Some(Action::Basic)
                if battle.units[actor]
                    .preservation_trailblazer_state
                    .enhanced_basic_in_action =>
            {
                let defense_bp = if self.eidolon >= 1 { 5_000 } else { 0 };
                battle.attack_hit_with_defense(
                    actor,
                    target,
                    Element::Fire,
                    [self.enhanced_main_bp, defense_bp],
                    20,
                );
                for neighbor in battle.adjacent_enemies(target) {
                    battle.attack_hit(
                        actor,
                        neighbor,
                        Element::Fire,
                        self.enhanced_adjacent_bp,
                        10,
                    );
                }
            }
            Some(Action::Basic) => {
                let defense_bp = if self.eidolon >= 1 { 2_500 } else { 0 };
                battle.attack_hit_with_defense(
                    actor,
                    target,
                    Element::Fire,
                    [self.basic_bp, defense_bp],
                    10,
                );
            }
            Some(Action::Ultimate) => {
                battle.attack_hit_with_defense(actor, target, Element::Fire, self.ultimate_bp, 20);
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Basic => {
                if std::mem::take(
                    &mut battle.units[actor]
                        .preservation_trailblazer_state
                        .enhanced_basic_in_action,
                ) {
                    self.finish_enhanced_basic(battle, actor);
                } else {
                    gain_magma_will(battle, actor);
                }
            }
            Action::Skill => self.use_skill(battle, actor),
            Action::Ultimate => {
                battle.units[actor]
                    .preservation_trailblazer_state
                    .free_enhanced_basic = true;
                self.gain_defense_stack(battle, actor);
            }
            Action::EnhancedSkill | Action::EnhancedUltimate | Action::ElationSkill => return,
        }
        self.shield_team(battle, actor);
    }

    fn after_enemy_attack(&self, battle: &mut Battle<'_>, actor: usize) {
        gain_magma_will(battle, actor);
    }

    fn taunts_enemy(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> bool {
        battle.units[enemy]
            .debuffs
            .iter()
            .any(|debuff| debuff.kind == DebuffKind::Taunt && debuff.source == owner)
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner == actor && !extra {
            let unit = &mut battle.units[actor];
            if unit
                .preservation_trailblazer_state
                .personal_reduction_until
                .is_some_and(|end| unit.normal_turns >= end)
            {
                unit.preservation_trailblazer_state.personal_reduction_until = None;
            }
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            self.start_shielded_turn(battle, actor);
        }
    }

    fn on_extra_turn_start(&self, battle: &mut Battle<'_>, actor: usize) {
        self.start_shielded_turn(battle, actor);
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        if owner == actor {
            battle.units[actor]
                .preservation_trailblazer_state
                .shielded_turn = false;
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: DamageKind,
    ) -> u32 {
        if battle.units[owner]
            .preservation_trailblazer_state
            .personal_reduction_until
            .is_some()
        {
            self.skill_reduction_bp
        } else {
            0
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner == target
            && battle.units[owner]
                .preservation_trailblazer_state
                .shielded_turn
        {
            A6_ATTACK_BP
        } else {
            0
        }
    }

    fn defense_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        u32::from(
            battle.units[actor]
                .preservation_trailblazer_state
                .defense_stacks,
        ) * E6_DEFENSE_BP
    }
}
