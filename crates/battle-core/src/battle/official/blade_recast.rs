use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle, SummonTurn,
        combat_status::DebuffKind,
        resolve::{FollowUpAttack, gauge_for},
        support::percentage_of,
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1507";
const BASIC: &str = "skill.150701";
const SKILL: &str = "skill.150702";
const ULTIMATE: &str = "skill.150703";
const TALENT: &str = "skill.150704";
const ENHANCED_BASIC: &str = "skill.150708";
const FOLLOW_UP: &str = "skill.150709";
const ENHANCED_ULTIMATE: &str = "skill.150714";
const OVERFLOW_TRACE: &str = "trace.1507101";
const DEFENSIVE_TRACE: &str = "trace.1507102";
const TEAM_TRACE: &str = "trace.1507103";

const ENERGY_CAP: u32 = 160;
const ENTRY_ENERGY_MILLI: u32 = 120_000;
const OVERFLOW_CAP_MILLI: u32 = 80_000;
const COUNTDOWN_SPEED_MICROS: u64 = 70_000_000;
const SKILL_ENERGY_MILLI: u32 = 30_000;
const SKILL_AREA_TOUGHNESS: u16 = 5;
const BOUNCE_TOUGHNESS: [u16; 4] = [1, 1, 1, 2];
const FIRE: Element = Element::Fire;

#[derive(Clone, Default)]
pub(in crate::battle) struct BladeRecastState {
    charge: u8,
    overflow_milli: u32,
    last_hp_charge_turn: Option<u64>,
}

struct BladeRecast {
    enhanced_basic: OfficialAbility,
    enhanced_ultimate: OfficialAbility,
    skill_area_bp: u16,
    bounce_bp: u16,
    skill_hp_cost_bp: u32,
    follow_up_hp_cost_bp: u32,
    ultimate_hp_cost_bp: u32,
    fury_crit_rate_bp: u32,
    fury_crit_damage_bp: u32,
    rescue_bp: u32,
    bind_defense_down_bp: u32,
    bind_vulnerability_bp: u16,
    bind_turns: u32,
    talent_energy_milli: u32,
    has_overflow_trace: bool,
    has_defensive_trace: bool,
    has_team_trace: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != FIRE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let enhanced_basic = input.parameters(ENHANCED_BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 8)?;
    let talent = input.parameters(TALENT, 2)?;
    let follow_up = input.parameters(FOLLOW_UP, 1)?[0];
    let enhanced_ultimate = input.parameters(ENHANCED_ULTIMATE, 1)?[0];
    if skill[1] != BOUNCE_TOUGHNESS.len() as f64 {
        return Err(invalid(SKILL));
    }
    if ultimate[4] != 70.0 || ultimate[7] != 2.0 {
        return Err(invalid(ULTIMATE));
    }
    let energy = (talent[1] * 1_000.0).round();
    if talent[0] != 9.0 || !(1.0..=f64::from(ENERGY_CAP * 1_000)).contains(&energy) {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    let skill_area_bp = ratio(SKILL, skill[0])?;
    let mechanics = BladeRecast {
        enhanced_basic: hp_ability(
            Target::Enemy,
            ratio(ENHANCED_BASIC, enhanced_basic)?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        enhanced_ultimate: hp_ability(
            Target::AllEnemies,
            ratio(
                ENHANCED_ULTIMATE,
                enhanced_ultimate * if input.eidolon == 6 { 1.5 } else { 1.0 },
            )?,
            20,
            5_000,
            SkillPoints::Unchanged,
        ),
        skill_area_bp,
        bounce_bp: ratio(SKILL, skill[2])?,
        skill_hp_cost_bp: u32::from(ratio(SKILL, skill[3])?),
        follow_up_hp_cost_bp: u32::from(ratio(FOLLOW_UP, follow_up)?),
        ultimate_hp_cost_bp: u32::from(ratio(ULTIMATE, ultimate[0])?),
        fury_crit_rate_bp: u32::from(ratio(ULTIMATE, ultimate[1])?),
        fury_crit_damage_bp: u32::from(ratio(ULTIMATE, ultimate[2])?),
        rescue_bp: u32::from(ratio(ULTIMATE, ultimate[5])?),
        bind_defense_down_bp: u32::from(ratio(ULTIMATE, ultimate[6])?),
        bind_vulnerability_bp: ratio(ULTIMATE, ultimate[3])?,
        bind_turns: 2,
        talent_energy_milli: energy as u32,
        has_overflow_trace: trace(OVERFLOW_TRACE),
        has_defensive_trace: trace(DEFENSIVE_TRACE),
        has_team_trace: trace(TEAM_TRACE),
        eidolon: input.eidolon,
    };
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: hp_ability(
            Target::Enemy,
            ratio(BASIC, basic)?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: hp_ability(
            Target::AllEnemies,
            skill_area_bp,
            SKILL_AREA_TOUGHNESS,
            SKILL_ENERGY_MILLI,
            SkillPoints::Unchanged,
        ),
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio(id: &str, fraction: f64) -> Result<u16, BattleError> {
    let bp = (fraction * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hp_ability(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::MaxHpHit {
            element: FIRE,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn zone_active(battle: &Battle<'_>, owner: usize) -> bool {
    battle.units[owner].countdown.is_some()
}

fn has_other_nihility(battle: &Battle<'_>, owner: usize) -> bool {
    battle.allies().any(|ally| {
        ally != owner
            && battle.plan.units[ally]
                .combat_type
                .is_some_and(|combat| combat.path == CombatPath::Nihility)
    })
}

fn set_charge(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].blade_recast_state.charge, after);
    if before != after {
        battle.emit(
            EventKind::BladeRecastCharge { before, after },
            Some(owner),
            None,
        );
    }
}

impl BladeRecast {
    fn charge_cap(&self) -> u8 {
        if self.eidolon >= 2 { 7 } else { 9 }
    }

    fn can_follow_up(&self, battle: &Battle<'_>, owner: usize) -> bool {
        zone_active(battle, owner)
            && battle.units[owner].hp > 1
            && !battle.is_crowd_controlled(owner)
            && battle.units[owner].blade_recast_state.charge == self.charge_cap()
    }

    fn queue_follow_up(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.special_follow_up_target(battle, owner).is_some()
            && !battle.has_pending_special_follow_up(owner)
        {
            battle.queue_special_follow_up(
                owner,
                Action::Skill,
                FollowUpAttack {
                    element: FIRE,
                    multiplier_bp: self.skill_area_bp,
                    toughness: SKILL_AREA_TOUGHNESS,
                    energy_milli: SKILL_ENERGY_MILLI,
                    first_hit: None,
                },
            );
        }
    }

    fn gain_charge(&self, battle: &mut Battle<'_>, owner: usize) {
        let before = battle.units[owner].blade_recast_state.charge;
        set_charge(battle, owner, (before + 1).min(self.charge_cap()));
        self.queue_follow_up(battle, owner);
    }

    fn hp_charge(&self, battle: &mut Battle<'_>, owner: usize) {
        let turn = battle.turn_sequence;
        let state = &mut battle.units[owner].blade_recast_state;
        if self.eidolon == 6
            && state.last_hp_charge_turn != Some(turn)
            && zone_active(battle, owner)
        {
            battle.units[owner].blade_recast_state.last_hp_charge_turn = Some(turn);
            self.gain_charge(battle, owner);
        }
    }

    fn apply_bind(&self, battle: &mut Battle<'_>, owner: usize, enemy: usize) {
        battle.apply_debuff(
            owner,
            enemy,
            DebuffKind::BalefireBind {
                vulnerability_bp: self.bind_vulnerability_bp,
            },
            self.bind_defense_down_bp,
            self.bind_turns,
        );
    }

    fn raise_energy_floor(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.has_overflow_trace && battle.units[owner].energy_milli < ENTRY_ENERGY_MILLI {
            battle.set_energy(owner, owner, ENTRY_ENERGY_MILLI);
        }
    }

    fn start_zone(&self, battle: &mut Battle<'_>, owner: usize) {
        for enemy in battle.enemies() {
            self.apply_bind(battle, owner, enemy);
        }
        let cost = percentage_of(battle.effective_max_hp(owner), self.ultimate_hp_cost_bp);
        battle.spend_hp(owner, cost, 1);
        battle.units[owner].countdown = Some(SummonTurn::with_action_advance(
            "无量忿怒",
            COUNTDOWN_SPEED_MICROS,
            false,
        ));
        battle.emit(
            EventKind::StatusApplied(StatusKind::InfiniteFury),
            Some(owner),
            Some(owner),
        );
        self.queue_follow_up(battle, owner);
    }

    fn end_zone(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].countdown = None;
        self.raise_energy_floor(battle, owner);
        battle.emit(
            EventKind::StatusExpired(StatusKind::InfiniteFury),
            Some(owner),
            Some(owner),
        );
    }

    fn skill_bounces(&self, battle: &mut Battle<'_>, owner: usize, follow_up: bool) {
        for toughness in BOUNCE_TOUGHNESS {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            if follow_up {
                battle.follow_up_hit_with_max_hp(
                    owner,
                    target,
                    Action::Skill,
                    FIRE,
                    self.bounce_bp,
                    toughness,
                );
            } else {
                battle.attack_hit_with_max_hp(
                    owner,
                    target,
                    FIRE,
                    (0, self.bounce_bp),
                    0,
                    toughness,
                );
            }
        }
    }
}

impl Mechanics for BladeRecast {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        self.raise_energy_floor(battle, owner);
    }

    fn on_countdown_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        self.end_zone(battle, owner);
    }

    fn can_use_skill(&self, battle: &Battle<'_>, owner: usize) -> bool {
        zone_active(battle, owner) && battle.units[owner].hp > 1
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if !zone_active(battle, owner) {
            return None;
        }
        match action {
            Action::Basic => Some(&self.enhanced_basic),
            Action::Ultimate => Some(&self.enhanced_ultimate),
            _ => None,
        }
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill => Some(1),
            Action::Basic => Some(3),
            _ => None,
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let cost = percentage_of(battle.effective_max_hp(owner), self.skill_hp_cost_bp);
                battle.spend_hp(owner, cost, 1);
            }
            Action::Ultimate if !zone_active(battle, owner) => self.start_zone(battle, owner),
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Basic => {
                if let Some(enemy) = target {
                    battle.apply_debuff(owner, enemy, DebuffKind::Taunt, 0, 1);
                }
            }
            Action::Skill => self.skill_bounces(battle, owner, false),
            Action::Ultimate if self.has_overflow_trace => {
                let stored =
                    std::mem::take(&mut battle.units[owner].blade_recast_state.overflow_milli);
                battle.restore_energy_flat(owner, owner, stored);
            }
            _ => {}
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: Option<usize>,
    ) -> bool {
        for target in battle.enemies() {
            battle.attack_hit_with_max_hp(
                owner,
                target,
                FIRE,
                (0, self.skill_area_bp),
                0,
                SKILL_AREA_TOUGHNESS,
            );
        }
        self.skill_bounces(battle, owner, false);
        battle.notify_attack(owner, Action::Skill, None);
        true
    }

    fn taunts_enemy(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> bool {
        battle.units[enemy]
            .debuffs
            .iter()
            .any(|status| status.source == owner && status.kind == DebuffKind::Taunt)
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        _: Action,
        _: Option<usize>,
    ) {
        if zone_active(battle, owner) && battle.is_targetable(owner) {
            let targets: Vec<_> = battle.attack_targets().to_vec();
            for enemy in targets {
                self.apply_bind(battle, owner, enemy);
            }
            self.gain_charge(battle, owner);
        }
    }

    fn special_follow_up_target(&self, battle: &Battle<'_>, owner: usize) -> Option<usize> {
        self.can_follow_up(battle, owner)
            .then(|| battle.enemies().find(|&enemy| battle.is_targetable(enemy)))
            .flatten()
    }

    fn resolve_special_follow_up(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        origin: Action,
    ) {
        set_charge(battle, owner, 0);
        battle.restore_energy_flat(owner, owner, self.talent_energy_milli);
        let cost = percentage_of(battle.effective_max_hp(owner), self.follow_up_hp_cost_bp);
        battle.spend_hp(owner, cost, 1);
        for enemy in battle.enemies() {
            battle.follow_up_hit_with_max_hp(
                owner,
                enemy,
                origin,
                FIRE,
                self.skill_area_bp,
                SKILL_AREA_TOUGHNESS,
            );
        }
        self.skill_bounces(battle, owner, true);
        if self.eidolon >= 1
            && let Some(timer) = &mut battle.units[owner].countdown
        {
            timer.gauge_quanta = timer.gauge_quanta.saturating_add(gauge_for(1_500));
        }
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
        _: bool,
    ) {
        if owner == target && self.has_defensive_trace && zone_active(battle, owner) {
            self.apply_bind(battle, owner, attacker);
            self.gain_charge(battle, owner);
        }
    }

    fn after_hp_loss(&self, battle: &mut Battle<'_>, owner: usize, _: usize, _: u32) {
        self.hp_charge(battle, owner);
    }

    fn after_hp_spent(&self, battle: &mut Battle<'_>, owner: usize, _: u32) {
        self.hp_charge(battle, owner);
    }

    fn tracks_action_end_reactions(&self) -> bool {
        true
    }

    fn react_to_action_end(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        self.queue_follow_up(battle, owner);
        if battle.has_pending_special_follow_up(owner) {
            battle.resolve_follow_ups();
        }
    }

    fn prevent_defeat(&self, battle: &mut Battle<'_>, owner: usize) -> bool {
        if !zone_active(battle, owner) {
            return false;
        }
        self.end_zone(battle, owner);
        let restored = percentage_of(battle.effective_max_hp(owner), self.rescue_bp).max(1);
        battle.restore_hp_unmodified(owner, owner, restored);
        true
    }

    fn on_energy_overflow(&self, battle: &mut Battle<'_>, owner: usize, amount_milli: u32) {
        if self.has_overflow_trace {
            let state = &mut battle.units[owner].blade_recast_state;
            state.overflow_milli = state
                .overflow_milli
                .saturating_add(amount_milli)
                .min(OVERFLOW_CAP_MILLI);
        }
    }

    fn on_ally_energy_full(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if self.has_overflow_trace && owner == target {
            while battle.cleanse_one(owner, owner) {}
        }
    }

    fn aggro_multiplier_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.has_defensive_trace && zone_active(battle, owner) {
            110_000
        } else {
            10_000
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: DamageKind,
    ) -> u32 {
        if self.has_defensive_trace && zone_active(battle, owner) {
            5_000
        } else {
            0
        }
    }

    fn incoming_healing_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.has_defensive_trace && zone_active(battle, owner) {
            5_000
        } else {
            0
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if zone_active(battle, owner) {
            self.fury_crit_rate_bp
        } else {
            0
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if zone_active(battle, owner) {
            self.fury_crit_damage_bp
        } else {
            0
        }
    }

    fn all_resistance_down_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if self.eidolon >= 1 && zone_active(battle, owner) {
            2_000
        } else {
            0
        }
    }

    fn grants_ultimate_follow_up(&self) -> bool {
        self.eidolon >= 2
    }

    fn ally_follow_up_damage_bonus_bp(&self) -> u32 {
        if self.eidolon >= 2 { 7_500 } else { 0 }
    }

    fn ally_ultimate_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.has_team_trace && zone_active(battle, owner) && has_other_nihility(battle, owner) {
            7_500
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _: Element,
    ) -> u32 {
        if !self.has_team_trace || !zone_active(battle, owner) {
            return 0;
        }
        5_000
            + 5_000 * u32::from(self.eidolon >= 4)
            + 7_500 * u32::from(owner == target && !has_other_nihility(battle, owner))
    }
}
