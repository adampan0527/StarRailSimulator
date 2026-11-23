use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        elation::{AhaInstant, ElationAscension, ElationDebt},
        resolve::ElationHit,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1511";
const BASIC: &str = "skill.151101";
const SKILL: &str = "skill.151102";
const ULTIMATE: &str = "skill.151103";
const TALENT: &str = "skill.151104";
const ELATION_SKILL: &str = "skill.151120";
const FACES: &str = "skill.151129";
const DEBT: &str = "skill.151131";
const PRANK: &str = "trace.1511101";
const CARNIVAL: &str = "trace.1511102";
const BOX: &str = "trace.1511103";
const QUANTUM: Element = Element::Quantum;

#[derive(Clone, Default)]
pub(in crate::battle) struct AeonAhaState {
    wishpower: u8,
    emanator: Option<usize>,
    faces_turns: u8,
    threshold_triggered: bool,
    sustain_triggered: bool,
    used_ultimate: bool,
}

struct AeonAha {
    eidolon: u8,
    prank: bool,
    carnival: bool,
    box_trace: bool,
    basic_bp: u32,
    skill_bp: u32,
    emanator_elation_bp: u32,
    ult_area_bp: u32,
    ult_bounce_bp: u32,
    ult_advance_bp: u16,
    elation_bp: u32,
    truth_bp: u32,
    threshold_crit_bp: u32,
    reclaim_bp: u32,
    faces_defense_ignore_bp: u32,
    faces_crit_bp: u32,
    debt_ratio_bp: u32,
    debt_cap_bp: u32,
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id} combat value"))
}

fn ratio(id: &str, value: f64) -> Result<u32, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u32)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != QUANTUM
        || input.energy_capacity != 0
    {
        return Err(invalid("combat type or resource"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 6)?;
    let ult = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 11)?;
    let elation = input.parameters(ELATION_SKILL, 2)?;
    let faces = input.parameters(FACES, 3)?;
    let debt = input.parameters(DEBT, 3)?;
    if basic[1] != 1.0 || skill[2..] != [0.0, 0.0, 1.0, 1.0] {
        return Err(invalid("basic or skill"));
    }
    if ult[0] != 8.0 || ult[2] != 5.0 || ult[4..] != [0.2, 3.0] {
        return Err(invalid(ULTIMATE));
    }
    if talent[..8] != [1.0, 40.0, 1.0, 0.0, 20.0, 8.0, 1.0, 8.0] || talent[9..] != [3.0, 10.0] {
        return Err(invalid(TALENT));
    }
    let expected_faces = if input.eidolon >= 2 {
        [0.1, 0.6, 0.2]
    } else {
        [0.1, 0.3, 0.12]
    };
    if faces != expected_faces {
        return Err(invalid(FACES));
    }

    let ability = |target, skill_points| OfficialAbility {
        target,
        effects: vec![Effect::DirectedAreaAttack],
        energy_milli: 0,
        skill_points,
    };
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: ability(Target::Enemy, SkillPoints::Generate),
        skill: ability(Target::OtherAlly, SkillPoints::Spend),
        ultimate: ability(Target::SelfOnly, SkillPoints::Unchanged),
        mechanics: Arc::new(AeonAha {
            eidolon: input.eidolon,
            prank: trace(PRANK),
            carnival: trace(CARNIVAL),
            box_trace: trace(BOX),
            basic_bp: ratio(BASIC, basic[0])?,
            skill_bp: ratio(SKILL, skill[1])?,
            emanator_elation_bp: ratio(SKILL, skill[0])?,
            ult_area_bp: ratio(ULTIMATE, ult[1])?,
            ult_bounce_bp: ratio(ULTIMATE, ult[3])?,
            ult_advance_bp: ratio(ULTIMATE, ult[4])? as u16,
            elation_bp: ratio(ELATION_SKILL, elation[0])?,
            truth_bp: ratio(ELATION_SKILL, elation[1])?,
            threshold_crit_bp: ratio(TALENT, talent[8])?,
            reclaim_bp: ratio(FACES, faces[0])?,
            faces_defense_ignore_bp: ratio(FACES, faces[1])?,
            faces_crit_bp: ratio(FACES, faces[2])?,
            debt_ratio_bp: ratio(DEBT, debt[0])?,
            debt_cap_bp: ratio(DEBT, debt[2])?,
        }),
    })
}

fn set_wishpower(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[actor].aeon_aha_state.wishpower, after);
    if before != after {
        battle.emit(EventKind::Wishpower { before, after }, Some(actor), None);
    }
}

fn grant_wishpower(battle: &mut Battle<'_>, actor: usize, count: u8) {
    let current = battle.units[actor].aeon_aha_state.wishpower;
    set_wishpower(battle, actor, current.saturating_add(count).min(10));
}

fn faces_active(battle: &Battle<'_>, actor: usize) -> bool {
    battle.units[actor].aeon_aha_state.faces_turns > 0
}

pub(super) fn deal_elation_truth(battle: &mut Battle<'_>, actor: usize, damage: u64) {
    let targets: Vec<_> = battle
        .enemies()
        .filter(|&enemy| battle.is_targetable(enemy))
        .collect();
    let count = targets.len() as u64;
    for (index, target) in targets.into_iter().enumerate() {
        let amount = damage / count + u64::from((index as u64) < damage % count);
        if amount > 0 {
            battle.true_damage(
                actor,
                target,
                Action::ElationSkill,
                amount.min(u64::from(u32::MAX)) as u32,
                Vec::new(),
            );
        }
    }
}

impl AeonAha {
    fn hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        multiplier_bp: u32,
        toughness: u16,
        origin: Action,
    ) {
        battle.elation_hit(
            actor,
            target,
            ElationHit {
                element: QUANTUM,
                multiplier_bp,
                punchline: battle.certified_banger(actor).unwrap_or(0),
                toughness,
                origin,
                factor_bp: 10_000,
                assisted_by: None,
            },
        );
    }

    fn select_emanator(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let former = battle.units[actor].aeon_aha_state.emanator.replace(target);
        if former == Some(target) {
            return;
        }
        if let Some(former) = former {
            battle.emit(
                EventKind::StatusExpired(StatusKind::EmanatorOfElation),
                Some(actor),
                Some(former),
            );
        }
        battle.emit(
            EventKind::StatusApplied(StatusKind::EmanatorOfElation),
            Some(actor),
            Some(target),
        );
    }

    fn skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        self.select_emanator(battle, actor, target);
        battle.emit(EventKind::FollowUp, Some(actor), None);
        let emanator_element = battle.plan.units[target]
            .combat_type
            .map(|kind| kind.element);
        for enemy in battle.enemies() {
            if !battle.is_targetable(enemy) {
                continue;
            }
            self.hit(battle, actor, enemy, self.skill_bp, 10, Action::Skill);
            if self.prank
                && let Some(element) = emanator_element
                && battle.is_targetable(enemy)
            {
                battle.elation_hit(
                    actor,
                    enemy,
                    ElationHit {
                        element,
                        multiplier_bp: 2_000,
                        punchline: battle.certified_banger(actor).unwrap_or(0),
                        toughness: 0,
                        origin: Action::Skill,
                        factor_bp: 10_000,
                        assisted_by: None,
                    },
                );
            }
        }
        if self.prank {
            battle.restore_skill_points(actor, 1);
        }
    }

    fn prepare_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].aeon_aha_state;
        let remaining = state.wishpower - 8;
        state.faces_turns = 3;
        state.sustain_triggered = false;
        let first = !std::mem::replace(&mut state.used_ultimate, true);
        set_wishpower(battle, actor, remaining);
        battle.emit(
            EventKind::StatusApplied(StatusKind::FacesOfElation),
            Some(actor),
            Some(actor),
        );
        if self.eidolon == 6 && first {
            battle.advance_aha(10_000);
            battle.add_punchline(actor, 100);
        } else {
            battle.advance_aha(self.ult_advance_bp);
        }
    }
}

impl Mechanics for AeonAha {
    fn elation_participant(&self) -> Option<u16> {
        Some(u16::MAX)
    }

    fn aha_base_speed_micros(&self, battle: &Battle<'_>, actor: usize) -> Option<u64> {
        Some(battle.plan.units[actor].stats.base_speed_micros)
    }

    fn elation_ascension(&self, battle: &Battle<'_>, actor: usize) -> Option<ElationAscension> {
        faces_active(battle, actor).then_some(ElationAscension {
            innate_level: if self.eidolon >= 2 { 2 } else { 1 },
            reclaimed_bp: self.reclaim_bp,
        })
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        Some(battle.units[actor].aeon_aha_state.wishpower >= 8)
    }

    fn activate_special_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let wishpower = battle.units[actor].aeon_aha_state.wishpower;
        set_wishpower(battle, actor, wishpower.max(8));
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            grant_wishpower(battle, actor, 5);
        }
        if self.box_trace {
            battle.advance(actor, 3_000);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon != 6 {
            return;
        }
        let rule = self.elation_debt().expect("Aeon Aha has Elation Debt");
        for target in battle.enemies() {
            if matches!(battle.plan.units[target].kit, super::super::Kit::Enemy(_)) {
                battle.apply_elation_debt(actor, target, rule, 0);
            }
        }
    }

    fn elation_debt(&self) -> Option<ElationDebt> {
        Some(ElationDebt {
            ratio_bp: self.debt_ratio_bp,
            cap_bp: self.debt_cap_bp,
            minimum_bp: 2_000 * u32::from(self.eidolon == 6),
        })
    }

    fn on_ally_sustain(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.carnival && !battle.units[actor].aeon_aha_state.sustain_triggered {
            battle.units[actor].aeon_aha_state.sustain_triggered = true;
            grant_wishpower(battle, actor, 1);
        }
    }

    fn on_ally_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        let state = &mut battle.units[owner].aeon_aha_state;
        if state.emanator == Some(actor) {
            grant_wishpower(battle, owner, 1);
        } else if owner == actor && state.faces_turns > 0 {
            state.faces_turns -= 1;
            if state.faces_turns == 0 {
                battle.emit(
                    EventKind::StatusExpired(StatusKind::FacesOfElation),
                    Some(owner),
                    Some(owner),
                );
            }
        }
    }

    fn on_punchline_changed(&self, battle: &mut Battle<'_>, actor: usize) {
        if battle.units[actor].aeon_aha_state.threshold_triggered || battle.current_punchline() < 40
        {
            return;
        }
        battle.units[actor].aeon_aha_state.threshold_triggered = true;
        grant_wishpower(battle, actor, 1);
        for target in battle.party_units() {
            battle.apply_ally_buff(
                actor,
                target,
                AllyBuffKind::CritDamage,
                self.threshold_crit_bp,
                3,
            );
        }
    }

    fn on_aha_instant_ending(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        natural: bool,
        skill_damage_factor_bp: u16,
    ) {
        let truth_bp =
            self.truth_bp + percentage_of(self.truth_bp, 2_500) * u32::from(self.eidolon >= 4);
        let damage = battle
            .aha_instant_damage()
            .saturating_mul(u64::from(truth_bp))
            .saturating_mul(u64::from(skill_damage_factor_bp))
            / 100_000_000;
        deal_elation_truth(battle, actor, damage);

        if natural {
            battle.units[actor].aeon_aha_state.threshold_triggered = false;
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate {
            self.prepare_ultimate(battle, actor);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if matches!(action, Action::Basic | Action::Skill) {
            grant_wishpower(battle, actor, 1);
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        match battle.action {
            Some(Action::Basic) => {
                self.hit(battle, actor, target, self.basic_bp, 10, Action::Basic)
            }
            Some(Action::Skill) => self.skill(battle, actor, target),
            Some(Action::Ultimate) => {
                for enemy in battle.enemies() {
                    if battle.is_targetable(enemy) {
                        self.hit(battle, actor, enemy, self.ult_area_bp, 20, Action::Ultimate);
                    }
                }
                for _ in 0..5 {
                    let Some(enemy) = battle.random_living_enemy() else {
                        break;
                    };
                    self.hit(
                        battle,
                        actor,
                        enemy,
                        self.ult_bounce_bp,
                        5,
                        Action::Ultimate,
                    );
                }
            }
            _ => {}
        }
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy) {
                battle.elation_hit(
                    actor,
                    enemy,
                    ElationHit {
                        element: QUANTUM,
                        multiplier_bp: self.elation_bp,
                        punchline: instant.punchline,
                        toughness: 10,
                        origin: Action::ElationSkill,
                        factor_bp: instant.skill_damage_factor_bp,
                        assisted_by: None,
                    },
                );
            }
        }
        grant_wishpower(battle, actor, 1);
    }

    fn primary_attack_counts_as_follow_up(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        match (self.box_trace, battle.speed_micros(actor) / 1_000_000) {
            (true, 150..) => 15_000,
            (true, 125..) => 12_000,
            _ => 0,
        }
    }

    fn ally_elation_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        self.emanator_elation_bp
            * u32::from(battle.units[owner].aeon_aha_state.emanator == Some(actor))
    }

    fn ally_merrymake_bp(&self, _: &Battle<'_>, _: usize, _: usize) -> u32 {
        1_500 * u32::from(self.eidolon >= 1)
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        self.faces_crit_bp * u32::from(faces_active(battle, actor))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _: usize) -> u32 {
        self.faces_defense_ignore_bp * u32::from(faces_active(battle, actor))
    }

    fn elation_punchline_bonus(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if self.eidolon < 4 {
            return 0;
        }
        battle
            .allies()
            .filter(|&ally| ally != actor && battle.is_targetable(ally))
            .filter_map(|ally| battle.certified_banger(ally))
            .fold(0, u32::saturating_add)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill
                if !battle.units[actor]
                    .aeon_aha_state
                    .emanator
                    .is_some_and(|ally| battle.is_targetable(ally)) =>
            {
                Some(
                    if target.is_some_and(|ally| {
                        battle.plan.units[ally]
                            .combat_type
                            .is_some_and(|kind| kind.path == CombatPath::Elation)
                    }) {
                        1
                    } else {
                        2
                    },
                )
            }
            Action::Skill => Some(4),
            _ => None,
        }
    }
}
