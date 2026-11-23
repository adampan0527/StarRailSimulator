use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, SummonTurn, resolve::gauge_for, support::percentage_of},
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1310";
const FIRE: Element = Element::Fire;
const ENERGY_CAPACITY: u32 = 240;
const ENERGY_CAP_MILLI: u32 = ENERGY_CAPACITY * 1_000;
const COUNTDOWN_SPEED_MICROS: u64 = 70_000_000;

struct SkillIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    enhanced_basic: &'static str,
    enhanced_skill: &'static str,
    traces: [&'static str; 3],
}

const BASE: SkillIds = SkillIds {
    basic: "skill.131001",
    skill: "skill.131002",
    ultimate: "skill.131003",
    talent: "skill.131004",
    enhanced_basic: "skill.131008",
    enhanced_skill: "skill.131009",
    traces: ["trace.1310101", "trace.1310102", "trace.1310103"],
};
const NOVAFLARE: SkillIds = SkillIds {
    basic: "skill.1131001",
    skill: "skill.1131002",
    ultimate: "skill.1131003",
    talent: "skill.1131004",
    enhanced_basic: "skill.1131008",
    enhanced_skill: "skill.1131009",
    traces: ["trace.11310101", "trace.11310102", "trace.11310103"],
};

#[derive(Clone, Default)]
pub(in crate::battle) struct FireflyState {
    combustion: bool,
    countdown_delays: u8,
    next_extra_turn: u32,
    pending_skill_advance: bool,
}

struct Firefly {
    novaflare: bool,
    eidolon: u8,
    trace_a2: bool,
    trace_a4: bool,
    trace_a6: bool,
    enhanced_basic: OfficialAbility,
    enhanced_skill: OfficialAbility,
    enhanced_skill_bp: [u16; 2],
    enhanced_skill_break_ratio_bp: [u16; 2],
    enhanced_skill_break_cap_bp: u16,
    skill_hp_cost_bp: u32,
    skill_energy_bp: u32,
    skill_advance_bp: u16,
    speed_micros: u64,
    toughness_efficiency_bp: u32,
    break_vulnerability_bp: u32,
    maximum_reduction_bp: u32,
    initial_energy_bp: u32,
    low_hp_bp: u32,
    resistance_bp: u32,
    enhanced_basic_heal_bp: u32,
    enhanced_skill_heal_bp: u32,
    implant_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != FIRE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let has_variant = |ids: &SkillIds| {
        [
            ids.basic,
            ids.skill,
            ids.ultimate,
            ids.talent,
            ids.enhanced_basic,
            ids.enhanced_skill,
        ]
        .into_iter()
        .any(|id| input.skill_parameters.contains_key(id))
    };
    let (novaflare, ids) = match (has_variant(&BASE), has_variant(&NOVAFLARE)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("missing or mixed skill variants")),
    };
    let basic = input.parameters(ids.basic, 1)?;
    let skill = input.parameters(ids.skill, 4)?;
    let ultimate = input.parameters(ids.ultimate, 4)?;
    let talent = input.parameters(ids.talent, 4)?;
    let enhanced_basic = input.parameters(ids.enhanced_basic, 2)?;
    let enhanced_skill = input.parameters(ids.enhanced_skill, if novaflare { 8 } else { 7 })?;
    if skill[1] != 0.4
        || !(0.5..=0.65).contains(&skill[2])
        || skill[3] != 0.25
        || ultimate[1] != 0.5
        || !(30.0..=75.0).contains(&ultimate[2])
        || ultimate[3] != 70.0
        || talent[1] != 0.5
        || talent[2] != 0.2
        || enhanced_basic[1] != 0.2
        || enhanced_skill[2..4] != if novaflare { [0.0, 0.0] } else { [0.25, 2.0] }
        || enhanced_skill[4..7] != [0.2, 0.1, 3.6]
        || (novaflare && enhanced_skill[7] != 0.0)
    {
        return Err(invalid("inconsistent locked skill parameters"));
    }
    let [trace_a2, trace_a4, trace_a6] = ids
        .traces
        .map(|id| input.enabled_trace_ids.iter().any(|trace| trace == id));
    let points = if input.eidolon >= 1 {
        SkillPoints::Unchanged
    } else {
        SkillPoints::Spend
    };
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP_MILLI,
        basic: attack(
            ratio_bp(ids.basic, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(ratio_bp(ids.skill, skill[0])?, 20, 0, SkillPoints::Spend),
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Firefly {
            novaflare,
            eidolon: input.eidolon,
            trace_a2,
            trace_a4,
            trace_a6,
            enhanced_basic: attack(
                ratio_bp(ids.enhanced_basic, enhanced_basic[0])?,
                15,
                0,
                SkillPoints::Generate,
            ),
            enhanced_skill: OfficialAbility {
                target: Target::Enemy,
                energy_milli: 0,
                effects: vec![Effect::DirectedAttack { adjacent: true }],
                skill_points: points,
            },
            enhanced_skill_bp: [
                ratio_bp(ids.enhanced_skill, enhanced_skill[0])?,
                ratio_bp(ids.enhanced_skill, enhanced_skill[1])?,
            ],
            enhanced_skill_break_ratio_bp: [
                ratio_bp(ids.enhanced_skill, enhanced_skill[4])?,
                ratio_bp(ids.enhanced_skill, enhanced_skill[5])?,
            ],
            enhanced_skill_break_cap_bp: ratio_bp(ids.enhanced_skill, enhanced_skill[6])?,
            skill_hp_cost_bp: u32::from(ratio_bp(ids.skill, skill[1])?),
            skill_energy_bp: u32::from(ratio_bp(ids.skill, skill[2])?),
            skill_advance_bp: ratio_bp(ids.skill, skill[3])?,
            speed_micros: (ultimate[2] * 1_000_000.0).round() as u64,
            toughness_efficiency_bp: u32::from(ratio_bp(ids.ultimate, ultimate[1])?),
            break_vulnerability_bp: u32::from(ratio_bp(ids.ultimate, ultimate[0])?),
            maximum_reduction_bp: u32::from(ratio_bp(ids.talent, talent[0])?),
            initial_energy_bp: u32::from(ratio_bp(ids.talent, talent[1])?),
            low_hp_bp: u32::from(ratio_bp(ids.talent, talent[2])?),
            resistance_bp: u32::from(ratio_bp(ids.talent, talent[3])?),
            enhanced_basic_heal_bp: u32::from(ratio_bp(ids.enhanced_basic, enhanced_basic[1])?),
            // The enhanced catalog renders these two values literally in the skill text.
            enhanced_skill_heal_bp: 2_500,
            implant_turns: 2,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(skill: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

fn attack(
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::Hit {
            element: FIRE,
            multiplier_bp,
            toughness,
        }],
        skill_points: points,
    }
}

fn combustion(battle: &Battle<'_>, owner: usize) -> bool {
    battle.units[owner].firefly_state.combustion
}

fn enhanced_action(battle: &Battle<'_>, owner: usize) -> bool {
    combustion(battle, owner)
        && battle.ordinary_actor == Some(owner)
        && !battle.in_follow_up()
        && matches!(battle.action, Some(Action::Basic | Action::Skill))
}

impl Firefly {
    fn start_combustion(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner];
        state.firefly_state.combustion = true;
        state.firefly_state.countdown_delays = 0;
        state.summon_turn = Some(SummonTurn::with_action_advance(
            "complete_combustion",
            COUNTDOWN_SPEED_MICROS,
            false,
        ));
        battle.advance(owner, 10_000);
        battle.emit(
            EventKind::CompleteCombustion { active: true },
            Some(owner),
            None,
        );
    }

    fn delay_countdown(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner];
        if self.novaflare
            && self.trace_a2
            && state.firefly_state.countdown_delays < 3
            && let Some(countdown) = &mut state.summon_turn
        {
            state.firefly_state.countdown_delays += 1;
            countdown.gauge_quanta = countdown.gauge_quanta.saturating_add(gauge_for(1_000));
        }
    }

    fn grant_extra_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon < 2 || battle.in_extra_turn(owner) {
            return;
        }
        let unit = &mut battle.units[owner];
        if unit.normal_turns >= unit.firefly_state.next_extra_turn {
            unit.firefly_state.next_extra_turn = unit
                .normal_turns
                .saturating_add(if self.novaflare { 1 } else { 2 });
            battle.queue_extra_turn(owner);
        }
    }
}

impl Mechanics for Firefly {
    fn can_use_ultimate(&self, battle: &Battle<'_>, owner: usize) -> bool {
        !combustion(battle, owner)
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if !combustion(battle, owner) {
            return None;
        }
        match action {
            Action::AssistSkill | Action::ElationSkill => None,
            Action::Basic => Some(&self.enhanced_basic),
            Action::Skill | Action::EnhancedSkill => Some(&self.enhanced_skill),
            Action::Ultimate | Action::EnhancedUltimate => None,
        }
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        let minimum = percentage_of(ENERGY_CAP_MILLI, self.initial_energy_bp);
        if battle.units[owner].energy_milli < minimum {
            battle.set_energy(owner, owner, minimum);
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let unit = &mut battle.units[owner];
        unit.firefly_state.combustion = false;
        unit.summon_turn = None;
        battle.emit(
            EventKind::CompleteCombustion { active: false },
            Some(owner),
            None,
        );
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        if combustion(battle, owner) {
            self.on_summon_turn(battle, owner);
        }
    }

    fn on_ally_energy_full(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            while battle.cleanse_one(owner, owner) {}
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if !action.is_ultimate()
            && !battle.in_extra_turn(owner)
            && std::mem::take(&mut battle.units[owner].firefly_state.pending_skill_advance)
        {
            battle.advance(owner, self.skill_advance_bp);
        }
        let (_, max_hp) = battle.hp(owner);
        match (combustion(battle, owner), action) {
            (false, Action::Skill) => {
                let amount = percentage_of(max_hp, self.skill_hp_cost_bp);
                battle.spend_hp(owner, amount, 1);
            }
            (true, Action::Basic | Action::Skill) => {
                let percent = if action == Action::Basic {
                    self.enhanced_basic_heal_bp
                } else {
                    self.enhanced_skill_heal_bp
                };
                battle.heal_unit(owner, owner, percentage_of(max_hp, percent));
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Ultimate => self.start_combustion(battle, owner),
            Action::Skill if !combustion(battle, owner) => {
                battle.restore_energy_flat(
                    owner,
                    owner,
                    percentage_of(ENERGY_CAP_MILLI, self.skill_energy_bp),
                );
                if battle.in_extra_turn(owner) && battle.interrupted_turn_actor() == Some(owner) {
                    battle.units[owner].firefly_state.pending_skill_advance = true;
                } else {
                    battle.advance(owner, self.skill_advance_bp);
                }
            }
            _ => {}
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let neighbors: Vec<_> = battle.adjacent_enemies(target).collect();
        battle.implant_weakness(owner, target, FIRE, self.implant_turns);
        if self.novaflare {
            for &neighbor in &neighbors {
                battle.implant_weakness(owner, neighbor, FIRE, self.implant_turns);
            }
        }
        for (target, splash) in std::iter::once((target, false))
            .chain(neighbors.into_iter().map(|neighbor| (neighbor, true)))
        {
            let i = usize::from(splash);
            battle.attack_hit_with_break_effect(
                owner,
                target,
                FIRE,
                self.enhanced_skill_bp[i],
                if splash { 15 } else { 30 },
                (
                    self.enhanced_skill_break_ratio_bp[i],
                    self.enhanced_skill_break_cap_bp,
                ),
            );
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        _target: usize,
        _action: Action,
    ) {
        if owner == source && enhanced_action(battle, owner) {
            self.delay_countdown(battle, owner);
            self.grant_extra_turn(battle, owner);
        }
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if enhanced_action(battle, owner) {
            self.grant_extra_turn(battle, owner);
        }
    }

    fn ally_flat_speed_micros(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u64 {
        self.speed_micros * u64::from(owner == target && combustion(battle, owner))
    }

    fn toughness_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if combustion(battle, owner)
            && kind == DamageKind::Direct
            && matches!(action, Some(Action::Basic | Action::Skill))
        {
            self.toughness_efficiency_bp + 5_000 * u32::from(self.eidolon >= 6)
        } else {
            0
        }
    }

    fn off_element_toughness_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u16 {
        5_500
            * u16::from(
                owner == actor && !self.novaflare && self.trace_a2 && combustion(battle, owner),
            )
    }

    fn own_break_vulnerability_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        self.break_vulnerability_bp * u32::from(enhanced_action(battle, owner))
    }

    fn break_effect_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        2_500 * u32::from(self.novaflare && self.trace_a2 && combustion(battle, owner))
    }

    fn break_effect_from_attack_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        attack: Option<u32>,
    ) -> u32 {
        if self.trace_a6 {
            let attack = attack.unwrap_or_else(|| battle.effective_attack(owner));
            attack.saturating_sub(1_800) / 10 * 80
        } else {
            0
        }
    }

    fn provides_super_break(&self) -> bool {
        self.trace_a4
    }

    fn super_break_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
        _target: usize,
    ) -> u16 {
        if owner != actor || !self.trace_a4 || !enhanced_action(battle, owner) {
            return 0;
        }
        self.super_break_multiplier_for_effect(0, battle.effective_break_effect(owner))
    }

    fn super_break_multiplier_for_effect(&self, _multiplier_bp: u16, amount: u32) -> u16 {
        if self.novaflare {
            if amount >= 30_000 {
                15_000
            } else if amount >= 15_000 {
                10_000
            } else {
                0
            }
        } else if amount >= 36_000 {
            5_000
        } else if amount >= 20_000 {
            3_500
        } else {
            0
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _source: usize,
        _kind: DamageKind,
    ) -> u32 {
        if combustion(battle, owner) {
            return self.maximum_reduction_bp;
        }
        let (hp, max_hp) = battle.hp(owner);
        let span = max_hp.saturating_sub(percentage_of(max_hp, self.low_hp_bp));
        let missing = max_hp.saturating_sub(hp).min(span);
        (u64::from(missing) * u64::from(self.maximum_reduction_bp) / u64::from(span.max(1))) as u32
    }

    fn effect_resistance_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        (self.resistance_bp + 5_000 * u32::from(self.eidolon >= 4))
            * u32::from(combustion(battle, owner))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _target: usize) -> u32 {
        1_500
            * u32::from(
                self.eidolon >= 1
                    && enhanced_action(battle, actor)
                    && battle.action == Some(Action::Skill),
            )
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        2_000 * u32::from(self.eidolon >= 6 && element == FIRE && combustion(battle, owner))
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::AssistSkill | Action::ElationSkill => None,
            Action::Ultimate => Some(0),
            Action::Skill | Action::EnhancedSkill => Some(2),
            Action::Basic => Some(3),
            Action::EnhancedUltimate => None,
        }
    }
}
