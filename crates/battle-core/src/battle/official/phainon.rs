use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle, GAUGE_FULL, SummonTurn,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

pub(super) const FORM: &str = "character.1408";
const BASIC: &str = "skill.140801";
const SKILL: &str = "skill.140802";
const ULTIMATE: &str = "skill.140803";
const TALENT: &str = "skill.140804";
const VESSEL: &str = "skill.140805";
const BLOODTHORN: &str = "skill.140808";
const CALAMITY: &str = "skill.140809";
const VERDICT: &str = "skill.140811";
const OBLIVION: &str = "trace.1408101";
const FLAMES: &str = "trace.1408102";
const VALOR: &str = "trace.1408103";
const COUNTDOWN: &str = "khaslana";
const PHYSICAL: Element = Element::Physical;
const CORE_CAP: u32 = 12;
const SCOURGE_CAP: u8 = 7;

#[derive(Clone, Default)]
pub(in crate::battle) struct PhainonState {
    coreflame: u32,
    scourge: u8,
    saved_coreflame: u32,
    remaining_turns: u8,
    defeated_enemies: u8,
    valor_stacks: u8,
    soulscorch: u8,
    sustain_turn: Option<u32>,
    verdict_damage: Option<u64>,
}

struct Phainon {
    eidolon: u8,
    oblivion: bool,
    flames: bool,
    valor: bool,
    overflow_cap: u32,
    core_on_skill: u32,
    targeted_crit_bp: u32,
    targeted_turns: u32,
    final_hit_bp: u32,
    countdown_speed_bp: u64,
    countdown_turns: u8,
    starting_scourge: u8,
    rescue_heal_bp: u32,
    early_penalty_bp: u32,
    transformed_attack_bp: u32,
    transformed_hp_bp: u32,
    exit_speed_bp: u32,
    post_attack_heal_bp: u32,
    enhanced_basic: OfficialAbility,
    thorn_scourge: u8,
    calamity: OfficialAbility,
    calamity_aoe_bp: u32,
    calamity_bounce_bp: u32,
    calamity_bounces: u8,
    calamity_reduction_bp: u32,
    soulscorch_factor_bp: u32,
    verdict: OfficialAbility,
    verdict_aoe_bp: u32,
    verdict_bounce_bp: u16,
    verdict_bounces_per_scourge: u8,
    verdict_full_cost: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 0
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != PHYSICAL
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 4)?;
    let vessel = input.parameters(VESSEL, 7)?;
    let enhanced_basic = input.parameters(BLOODTHORN, 3)?;
    let calamity = input.parameters(CALAMITY, 5)?;
    let verdict = input.parameters(VERDICT, 5)?;
    if ultimate[1..] != [12.0, 0.6, 8.0]
        || talent[1..] != [3.0, 3.0, 12.0]
        || skill[2] != 2.0
        || enhanced_basic[2] != 2.0
        || vessel[0..3] != [4.0, 0.25, 0.125]
        || vessel[5..] != [0.15, 0.2]
        || calamity[1..3] != [0.75, 4.0]
        || calamity[4] != 0.2
        || verdict[2..4] != [4.0, 4.0]
    {
        return Err(invalid("locked ability rules"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: ability(
            Target::Enemy,
            SkillPoints::Generate,
            vec![Effect::Hit {
                element: PHYSICAL,
                multiplier_bp: hit_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
        ),
        skill: blast(SkillPoints::Spend, SKILL, skill, [20, 10])?,
        ultimate: ability(Target::SelfOnly, SkillPoints::Unchanged, Vec::new()),
        mechanics: Arc::new(Phainon {
            eidolon: input.eidolon,
            oblivion: has_trace(OBLIVION),
            flames: has_trace(FLAMES),
            valor: has_trace(VALOR),
            overflow_cap: talent[2] as u32,
            core_on_skill: skill[2] as u32,
            targeted_crit_bp: bp(TALENT, talent[0])?,
            targeted_turns: talent[1] as u32,
            final_hit_bp: bp(ULTIMATE, ultimate[0])?,
            countdown_speed_bp: u64::from(bp(ULTIMATE, ultimate[2])?),
            countdown_turns: ultimate[3] as u8,
            starting_scourge: vessel[0] as u8,
            rescue_heal_bp: bp(VESSEL, vessel[1])?,
            early_penalty_bp: bp(VESSEL, vessel[2])?,
            transformed_attack_bp: bp(VESSEL, vessel[3])?,
            transformed_hp_bp: bp(VESSEL, vessel[4])?,
            exit_speed_bp: bp(VESSEL, vessel[5])?,
            post_attack_heal_bp: bp(VESSEL, vessel[6])?,
            enhanced_basic: blast(SkillPoints::Unchanged, BLOODTHORN, enhanced_basic, [30, 20])?,
            thorn_scourge: enhanced_basic[2] as u8,
            calamity: ability(Target::SelfOnly, SkillPoints::Unchanged, Vec::new()),
            calamity_aoe_bp: bp(CALAMITY, calamity[0])?,
            calamity_bounce_bp: bp(CALAMITY, calamity[3])?,
            calamity_bounces: calamity[2] as u8,
            calamity_reduction_bp: bp(CALAMITY, calamity[1])?,
            soulscorch_factor_bp: bp(CALAMITY, calamity[4])?,
            verdict: ability(Target::SelfOnly, SkillPoints::Unchanged, Vec::new()),
            verdict_aoe_bp: bp(VERDICT, verdict[0])?,
            verdict_bounce_bp: hit_bp(VERDICT, verdict[1])?,
            verdict_bounces_per_scourge: verdict[2] as u8,
            verdict_full_cost: verdict[3] as u8,
        }),
    })
}

fn invalid(rule: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {rule}"))
}

fn bp(skill: &str, value: f64) -> Result<u32, BattleError> {
    let scaled = (value * 10_000.0).round();
    if !(1.0..=200_000.0).contains(&scaled) {
        return Err(invalid(skill));
    }
    Ok(scaled as u32)
}

fn hit_bp(skill: &str, value: f64) -> Result<u16, BattleError> {
    u16::try_from(bp(skill, value)?).map_err(|_| invalid(skill))
}

fn ability(target: Target, skill_points: SkillPoints, effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli: 0,
        skill_points,
        effects,
    }
}

fn blast(
    points: SkillPoints,
    id: &str,
    row: &[f64],
    toughness: [u16; 2],
) -> Result<OfficialAbility, BattleError> {
    Ok(ability(
        Target::Enemy,
        points,
        vec![
            Effect::Hit {
                element: PHYSICAL,
                multiplier_bp: hit_bp(id, row[0])?,
                toughness: toughness[0],
            },
            Effect::BlastAdjacent {
                element: PHYSICAL,
                multiplier_bp: hit_bp(id, row[1])?,
                toughness: toughness[1],
            },
        ],
    ))
}

fn transformed(battle: &Battle<'_>, actor: usize) -> bool {
    battle.territory_owner == Some(actor)
}

fn living_enemies(battle: &Battle<'_>) -> Vec<usize> {
    battle
        .enemies()
        .filter(|&target| battle.is_targetable(target))
        .collect()
}

impl Phainon {
    fn set_coreflame(battle: &mut Battle<'_>, actor: usize, after: u32) {
        let before = std::mem::replace(&mut battle.units[actor].phainon_state.coreflame, after);
        if before != after {
            battle.emit(
                EventKind::Coreflame { before, after },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn gain_coreflame(&self, battle: &mut Battle<'_>, actor: usize, count: u32) {
        if transformed(battle, actor) {
            return;
        }
        let cap = if self.eidolon >= 6 {
            u32::MAX
        } else {
            CORE_CAP + self.overflow_cap
        };
        let after = battle.units[actor]
            .phainon_state
            .coreflame
            .saturating_add(count)
            .min(cap);
        Self::set_coreflame(battle, actor, after);
    }

    fn set_scourge(battle: &mut Battle<'_>, actor: usize, after: u8) {
        let after = after.min(SCOURGE_CAP);
        let before = std::mem::replace(&mut battle.units[actor].phainon_state.scourge, after);
        if before != after {
            battle.emit(
                EventKind::Scourge { before, after },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn gain_scourge(battle: &mut Battle<'_>, actor: usize, count: u8) {
        let after = battle.units[actor]
            .phainon_state
            .scourge
            .saturating_add(count);
        Self::set_scourge(battle, actor, after);
    }

    fn countdown_speed(&self, battle: &Battle<'_>, actor: usize) -> u64 {
        let ratio_bp = if self.eidolon >= 1 {
            (6_600 + 150 * u64::from(battle.units[actor].phainon_state.defeated_enemies)).min(8_400)
        } else {
            self.countdown_speed_bp
        };
        (battle.plan.units[actor].stats.base_speed_micros * ratio_bp / 10_000).max(1)
    }

    fn schedule(&self, battle: &mut Battle<'_>, actor: usize, remaining_gauge: u64) {
        let mut countdown = SummonTurn::new(COUNTDOWN, self.countdown_speed(battle, actor));
        countdown.gauge_quanta = remaining_gauge;
        battle.units[actor].countdown = Some(countdown);
    }

    fn enter(&self, battle: &mut Battle<'_>, actor: usize) {
        let opening_gauge = battle.units[actor].gauge_quanta;
        let before_hp = battle.effective_max_hp(actor);
        if let Some(interrupted) = battle.interrupted_turn_actor() {
            battle.close_turn_before_suspension(interrupted);
        }
        if let Some(current) = battle.ordinary_actor {
            battle.extend_active_buffs_by_one_turn(current);
            battle.close_turn_before_suspension(current);
            battle.reset_gauge(current);
        }
        let state = &mut battle.units[actor].phainon_state;
        state.saved_coreflame = state.coreflame - CORE_CAP;
        state.remaining_turns = self.countdown_turns;
        Self::set_coreflame(battle, actor, 0);
        Self::set_scourge(battle, actor, self.starting_scourge);
        battle.territory_owner = Some(actor);
        battle.cleanse_crowd_control(actor, actor);
        let hp_increase = battle.effective_max_hp(actor).saturating_sub(before_hp);
        battle.units[actor].hp = battle.units[actor].hp.saturating_add(hp_increase);
        if self.eidolon >= 1 {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::CritDamage,
                5_000,
                3,
                Some("phainon.e1"),
            );
        }
        self.schedule(battle, actor, opening_gauge);
        battle.emit(
            EventKind::StatusApplied(StatusKind::Khaslana),
            Some(actor),
            Some(actor),
        );
    }

    fn exit(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].countdown = None;
        battle.territory_owner = None;
        battle.cancel_queued_extra_turn(actor);
        let state = &mut battle.units[actor].phainon_state;
        state.remaining_turns = 0;
        state.soulscorch = 0;
        let carry = std::mem::take(&mut state.saved_coreflame);
        Self::set_scourge(battle, actor, 0);
        self.gain_coreflame(
            battle,
            actor,
            carry.saturating_add(3 * u32::from(self.oblivion)),
        );
        let cap = battle.effective_max_hp(actor);
        battle.units[actor].hp = battle.units[actor].hp.min(cap);
        for ally in battle
            .party_units()
            .filter(|&ally| battle.is_targetable(ally))
            .collect::<Vec<_>>()
        {
            battle.apply_ally_buff(
                actor,
                ally,
                AllyBuffKind::SpeedPercent,
                self.exit_speed_bp,
                1,
            );
        }
        self.stack_valor(battle, actor);
        battle.emit(
            EventKind::StatusExpired(StatusKind::Khaslana),
            Some(actor),
            Some(actor),
        );
    }

    fn stack_valor(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.valor {
            let value = &mut battle.units[actor].phainon_state.valor_stacks;
            *value = (*value + 1).min(2);
        }
    }

    fn heal(&self, battle: &mut Battle<'_>, actor: usize, ratio_bp: u32) {
        let amount = percentage_of(battle.effective_max_hp(actor), ratio_bp);
        battle.heal_unit(actor, actor, amount);
    }

    fn damage_split(&self, battle: &mut Battle<'_>, actor: usize, total_bp: u32, ultimate: bool) {
        let targets = living_enemies(battle);
        let Ok(count) = u32::try_from(targets.len()) else {
            return;
        };
        if count == 0 {
            return;
        }
        for (index, target) in targets.into_iter().enumerate() {
            let damage_bp = total_bp / count + u32::from((index as u32) < total_bp % count);
            if ultimate {
                battle.final_hit(actor, target, PHYSICAL, damage_bp, 20);
            } else {
                battle.attack_hit(actor, target, PHYSICAL, damage_bp as u16, 20);
            }
        }
    }

    fn last_hit(&self, battle: &mut Battle<'_>, actor: usize, early: bool) {
        let remaining = u32::from(battle.units[actor].phainon_state.remaining_turns);
        let scale = if early {
            10_000_u32.saturating_sub(remaining * self.early_penalty_bp)
        } else {
            10_000
        };
        let debuffs = battle.begin_debuff_action();
        battle.begin_attack();
        let previous = battle.action.replace(Action::Ultimate);
        battle.emit(EventKind::Action(Action::Ultimate), Some(actor), None);
        self.damage_split(battle, actor, percentage_of(self.final_hit_bp, scale), true);
        battle.notify_attack(actor, Action::Ultimate, None);
        battle.action = previous;
        battle.finish_debuff_action(actor, debuffs);
        battle.resolve_follow_ups();
        self.exit(battle, actor);
    }

    fn calamity(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::gain_scourge(battle, actor, living_enemies(battle).len() as u8);
        battle.units[actor].phainon_state.soulscorch = 1 + 4 * u8::from(self.eidolon >= 4);
        let actions = battle.immediately_act_enemies(actor);
        if !transformed(battle, actor) {
            return;
        }
        let state = &mut battle.units[actor].phainon_state;
        let layers = std::mem::take(&mut state.soulscorch).saturating_add(actions);
        let multiplier = 10_000 + self.soulscorch_factor_bp * u32::from(layers);
        battle.begin_attack();
        for target in living_enemies(battle) {
            battle.attack_hit(
                actor,
                target,
                PHYSICAL,
                percentage_of(self.calamity_aoe_bp, multiplier) as u16,
                5,
            );
        }
        for _ in 0..self.calamity_bounces {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.attack_hit(
                actor,
                target,
                PHYSICAL,
                percentage_of(self.calamity_bounce_bp, multiplier) as u16,
                10,
            );
        }
        battle.notify_attack(actor, Action::Skill, None);
        battle.resolve_follow_ups();
        self.heal(battle, actor, self.post_attack_heal_bp);
    }

    fn verdict(&self, battle: &mut Battle<'_>, actor: usize) {
        let cost = battle.units[actor]
            .phainon_state
            .scourge
            .min(self.verdict_full_cost);
        let remaining = battle.units[actor].phainon_state.scourge - cost;
        Self::set_scourge(battle, actor, remaining);
        while battle.cleanse_one(actor, actor) {}
        battle.units[actor].phainon_state.verdict_damage = (self.eidolon >= 6).then_some(0);
        for hit in 0..u16::from(cost * self.verdict_bounces_per_scourge) {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            // The source row assigns ten thirds of a toughness point per bounce.
            let toughness = ((hit + 1) * 10 / 3) - (hit * 10 / 3);
            battle.attack_hit(actor, target, PHYSICAL, self.verdict_bounce_bp, toughness);
        }
        if cost == self.verdict_full_cost {
            self.damage_split(battle, actor, self.verdict_aoe_bp, false);
            if self.eidolon >= 2 {
                battle.queue_extra_turn(actor);
            }
        }
        let tally = battle.units[actor].phainon_state.verdict_damage.take();
        if let Some(tally) = tally
            && let Some(target) = battle
                .enemies()
                .filter(|&target| battle.is_targetable(target))
                .max_by_key(|&target| (battle.units[target].hp, std::cmp::Reverse(target)))
        {
            let amount = (tally.saturating_mul(36) / 100).min(u64::from(u32::MAX)) as u32;
            battle.true_damage(actor, target, Action::EnhancedSkill, amount, Vec::new());
        }
        battle.notify_attack(actor, Action::EnhancedSkill, None);
        battle.resolve_follow_ups();
        self.heal(battle, actor, self.post_attack_heal_bp);
    }
}

impl Mechanics for Phainon {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        self.gain_coreflame(
            battle,
            actor,
            u32::from(self.oblivion) + 6 * u32::from(self.eidolon >= 6),
        );
        self.stack_valor(battle, actor);
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        Some(!transformed(battle, actor) && battle.units[actor].phainon_state.coreflame >= CORE_CAP)
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if !transformed(battle, actor) {
            return None;
        }
        match action {
            Action::Basic => Some(&self.enhanced_basic),
            Action::Skill => Some(&self.calamity),
            Action::EnhancedSkill if battle.units[actor].phainon_state.scourge > 0 => {
                Some(&self.verdict)
            }
            _ => None,
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
            Action::Ultimate => self.enter(battle, actor),
            Action::Skill if transformed(battle, actor) => self.calamity(battle, actor),
            Action::EnhancedSkill => self.verdict(battle, actor),
            Action::Skill => self.gain_coreflame(battle, actor, self.core_on_skill),
            Action::Basic if transformed(battle, actor) => {
                Self::gain_scourge(battle, actor, self.thorn_scourge);
                self.heal(battle, actor, self.post_attack_heal_bp);
            }
            _ => {}
        }
    }

    fn on_countdown_turn(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].phainon_state;
        state.remaining_turns -= 1;
        if state.remaining_turns == 0 {
            self.last_hit(battle, actor, false);
        } else {
            self.schedule(battle, actor, GAUGE_FULL);
            battle.queue_extra_turn(actor);
        }
    }

    fn on_ally_targeted(&self, battle: &mut Battle<'_>, actor: usize, source: usize, _: Action) {
        if actor != source && !transformed(battle, actor) {
            self.gain_coreflame(battle, actor, 1);
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::CritDamage,
                self.targeted_crit_bp,
                self.targeted_turns,
                Some("phainon.targeted"),
            );
        }
    }

    fn on_enemy_targeted(&self, battle: &mut Battle<'_>, actor: usize) {
        self.gain_coreflame(battle, actor, 1);
    }

    fn on_external_energy(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.flames {
            self.gain_coreflame(battle, actor, 1);
        }
    }

    fn on_ally_sustain(&self, battle: &mut Battle<'_>, actor: usize) {
        let turn = battle.units[actor].normal_turns;
        if self.flames && battle.units[actor].phainon_state.sustain_turn != Some(turn) {
            battle.units[actor].phainon_state.sustain_turn = Some(turn);
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::Damage,
                4_500,
                4,
                Some("phainon.sustain"),
            );
        }
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, actor: usize, _: usize, _: usize) {
        self.observe_defeat(battle, actor);
    }

    fn max_hp_multiplier_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        10_000 + self.transformed_hp_bp * u32::from(transformed(battle, actor))
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner != target {
            return 0;
        }
        5_000 * u32::from(battle.units[owner].phainon_state.valor_stacks)
            + self.transformed_attack_bp * u32::from(transformed(battle, owner))
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, actor: usize) -> bool {
        transformed(battle, actor)
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        if self.eidolon >= 2 && element == PHYSICAL && transformed(battle, actor) {
            2_000
        } else {
            0
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: DamageKind,
    ) -> u32 {
        if battle.units[actor].phainon_state.soulscorch > 0 {
            self.calamity_reduction_bp
        } else {
            0
        }
    }

    fn prevent_defeat(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        if !transformed(battle, actor) {
            return false;
        }
        self.heal(battle, actor, self.rescue_heal_bp);
        self.last_hit(battle, actor, true);
        true
    }

    fn tracks_ally_damage(&self) -> bool {
        self.eidolon >= 6
    }

    fn on_ally_damage(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: usize,
        _: DamageKind,
        amount: u32,
    ) {
        if owner == actor
            && let Some(tally) = &mut battle.units[owner].phainon_state.verdict_damage
        {
            *tally = tally.saturating_add(u64::from(amount));
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match (transformed(battle, actor), action) {
            (_, Action::Ultimate) => Some(0),
            (true, Action::EnhancedSkill)
                if battle.units[actor].phainon_state.scourge >= self.verdict_full_cost =>
            {
                Some(0)
            }
            (true, Action::Skill) => Some(1),
            (true, Action::Basic) => Some(2),
            (false, Action::Skill) => Some(1),
            _ => None,
        }
    }
}

impl Phainon {
    fn observe_defeat(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].phainon_state;
        state.defeated_enemies = (state.defeated_enemies + 1).min(12);
        let speed = self.countdown_speed(battle, actor);
        if let Some(countdown) = &mut battle.units[actor].countdown {
            countdown.speed_micros = speed;
        }
    }
}

#[cfg(test)]
mod tests;
