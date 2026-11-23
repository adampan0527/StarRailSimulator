use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        resolve::{DamageTargetAura, FollowUpAttack},
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1321";
const BASIC: &str = "skill.132101";
const SKILL: &str = "skill.132102";
const ULTIMATE: &str = "skill.132103";
const TALENT: &str = "skill.132104";
const A2: &str = "trace.1321101";
const A4: &str = "trace.1321102";
const A6: &str = "trace.1321103";
const FIRE: Element = Element::Fire;

#[derive(Clone, Default)]
pub(in crate::battle) struct DahliaState {
    partner: Option<usize>,
    zone_expires_on: Option<u32>,
    last_follow_up_turn: Option<u32>,
    last_sustain_turn: Option<u32>,
    follow_ups_since_refund: u8,
    ultimate_targets: Vec<usize>,
    eidolon_toughness_used: bool,
}

struct Dahlia {
    eidolon: u8,
    sustain_share: bool,
    skill_point_refund: bool,
    implant_bonus: bool,
    zone_turns: u32,
    zone_efficiency_bp: u32,
    wilt_turns: u32,
    wilt_defense_down_bp: u32,
    start_energy_milli: u32,
    dance_super_break_bp: u16,
    follow_up_super_break_bp: u16,
    follow_up_hits: u8,
    follow_up: FollowUpAttack,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != FIRE
        || input.energy_capacity != 130
        || input.eidolon > 6
    {
        return Err(invalid("combat type, energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 5)?;
    if skill[1] != 3.0 || skill[2] != 0.5 {
        return Err(invalid(SKILL));
    }
    if ultimate[1] != 4.0 {
        return Err(invalid(ULTIMATE));
    }
    if talent[1] != 5.0 || talent[3] != 35.0 {
        return Err(invalid(TALENT));
    }
    let skill_bp = ratio_bp(SKILL, skill[0])?;
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(ratio_bp(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                hit(skill_bp, 10),
                Effect::BlastAdjacent {
                    element: FIRE,
                    multiplier_bp: skill_bp,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(ratio_bp(ULTIMATE, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Dahlia {
            eidolon: input.eidolon,
            sustain_share: trace(A2),
            skill_point_refund: trace(A4),
            implant_bonus: trace(A6),
            zone_turns: skill[1] as u32,
            zone_efficiency_bp: u32::from(ratio_bp(SKILL, skill[2])?),
            wilt_turns: ultimate[1] as u32,
            wilt_defense_down_bp: u32::from(ratio_bp(ULTIMATE, ultimate[2])?),
            start_energy_milli: talent[3] as u32 * 1_000,
            dance_super_break_bp: ratio_bp(TALENT, talent[4])?,
            follow_up_super_break_bp: ratio_bp(TALENT, talent[2])?,
            follow_up_hits: talent[1] as u8 + if input.eidolon >= 4 { 5 } else { 0 },
            follow_up: FollowUpAttack {
                element: FIRE,
                multiplier_bp: ratio_bp(TALENT, talent[0])?,
                toughness: 3,
                energy_milli: 2_000,
                first_hit: None,
            },
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(id));
    }
    Ok(value as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: FIRE,
        multiplier_bp,
        toughness,
    }
}

fn partner(battle: &Battle<'_>, owner: usize) -> Option<usize> {
    battle.units[owner]
        .dahlia_state
        .partner
        .filter(|&ally| battle.is_targetable(ally))
}

fn is_dancer(battle: &Battle<'_>, owner: usize, actor: usize) -> bool {
    actor == owner || partner(battle, owner) == Some(actor)
}

fn ensure_partner(battle: &mut Battle<'_>, owner: usize) {
    if partner(battle, owner).is_some() {
        return;
    }
    if let Some(old) = battle.units[owner].dahlia_state.partner.take() {
        battle.emit(
            EventKind::StatusExpired(StatusKind::DancePartner),
            Some(owner),
            Some(old),
        );
    }
    let selected = battle
        .allies()
        .filter(|&ally| ally != owner && battle.is_targetable(ally))
        .max_by_key(|&ally| (battle.effective_break_effect(ally), Reverse(ally)));
    battle.units[owner].dahlia_state.partner = selected;
    if let Some(ally) = selected {
        battle.emit(
            EventKind::StatusApplied(StatusKind::DancePartner),
            Some(owner),
            Some(ally),
        );
    }
}

fn weakness_mask(battle: &Battle<'_>, owner: usize) -> u8 {
    let fire = 1 << FIRE.index();
    partner(battle, owner)
        .and_then(|ally| battle.plan.units[ally].combat_type)
        .map_or(fire, |kind| fire | (1 << kind.element.index()))
}

impl Dahlia {
    fn share_break_effect(&self, battle: &mut Battle<'_>, owner: usize, turns: u32) {
        let bp = 5_000 + percentage_of(battle.effective_break_effect(owner), 2_400);
        for ally in battle.allies().filter(|&ally| ally != owner) {
            battle.apply_ally_buff_with_id(
                owner,
                ally,
                AllyBuffKind::BreakEffect,
                bp,
                turns,
                Some(A2),
            );
        }
    }

    fn wilt(&self, battle: &mut Battle<'_>, owner: usize, turns: u32) {
        let kind = DebuffKind::Wilt {
            weaknesses: weakness_mask(battle, owner),
        };
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy) {
                battle.apply_debuff(owner, enemy, kind, self.wilt_defense_down_bp, turns);
            }
        }
    }

    fn apply_implant_bonus(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
    ) {
        if !self.implant_bonus
            || !battle.plan.units[actor]
                .combat_type
                .is_some_and(|kind| kind.element == FIRE)
        {
            return;
        }
        let mut implanted = false;
        let implants = battle.attack_implants.len();
        for index in 0..implants {
            let (source, enemy) = battle.attack_implants[index];
            if source == actor {
                implanted = true;
                battle.fixed_attack_toughness((owner, actor), enemy, FIRE, origin, 2_000);
            }
        }
        let max = battle.plan.units[actor].kit.max_energy_milli();
        if implanted && max > 0 {
            let remaining_to_cap = (max / 2).saturating_sub(battle.units[actor].energy_milli);
            battle.restore_energy_flat(owner, actor, (max / 10).min(remaining_to_cap));
        }
    }

    fn apply_eidolon_toughness(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
    ) {
        if self.eidolon < 1 || !is_dancer(battle, owner, actor) {
            return;
        }
        let element = battle.plan.units[actor]
            .combat_type
            .map_or(FIRE, |kind| kind.element);
        let count = battle.attack_targets().len();
        for index in 0..count {
            let enemy = battle.attack_targets()[index];
            if !battle.is_targetable(enemy)
                || battle.units[enemy].dahlia_state.eidolon_toughness_used
            {
                continue;
            }
            battle.units[enemy].dahlia_state.eidolon_toughness_used = true;
            let max = u32::from(battle.plan.units[enemy].kit.max_toughness());
            let quarter_hundredths = (25 * max).clamp(1_000, 30_000) as u16;
            battle.fixed_attack_toughness(
                (owner, actor),
                enemy,
                element,
                origin,
                quarter_hundredths,
            );
        }
    }
}

impl Mechanics for Dahlia {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.gain_energy(owner, owner, self.start_energy_milli);
        battle.emit(
            EventKind::StatusApplied(StatusKind::DancePartner),
            Some(owner),
            Some(owner),
        );
        ensure_partner(battle, owner);
        if self.sustain_share {
            self.share_break_effect(battle, owner, 1);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        ensure_partner(battle, owner);
        if self.eidolon >= 2 {
            self.wilt(battle, owner, 3);
        }
    }

    fn on_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize) {
        ensure_partner(battle, owner);
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, _: usize, _: usize, target: usize) {
        battle.units[target].dahlia_state.eidolon_toughness_used = false;
    }

    fn on_ally_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, _: usize, _: bool) {
        ensure_partner(battle, owner);
        let unit = &mut battle.units[owner];
        if unit
            .dahlia_state
            .zone_expires_on
            .is_some_and(|end| unit.normal_turns >= end)
        {
            unit.dahlia_state.zone_expires_on = None;
            battle.emit(
                EventKind::StatusExpired(StatusKind::DahliaZone),
                Some(owner),
                Some(owner),
            );
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
                let unit = &mut battle.units[owner];
                unit.dahlia_state.zone_expires_on = Some(unit.normal_turns + self.zone_turns);
                battle.emit(
                    EventKind::StatusApplied(StatusKind::DahliaZone),
                    Some(owner),
                    Some(owner),
                );
            }
            Action::Ultimate => {
                battle.units[owner].dahlia_state.ultimate_targets = battle
                    .enemies()
                    .filter(|&enemy| battle.is_targetable(enemy))
                    .collect();
                self.wilt(battle, owner, self.wilt_turns);
            }
            _ => {}
        }
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if action != Some(Action::Ultimate) || kind != DamageKind::Direct {
            return base;
        }
        let targets = &battle.units[owner].dahlia_state.ultimate_targets;
        targets
            .iter()
            .position(|&enemy| enemy == target)
            .map_or(0, |rank| {
                let count = targets.len() as u32;
                base / count + u32::from((rank as u32) < base % count)
            })
    }

    fn damage_target_aura(&self, battle: &Battle<'_>, owner: usize, _: usize) -> DamageTargetAura {
        DamageTargetAura {
            resistance_down_bp: 2_000 * u32::from(self.eidolon >= 2 && battle.is_targetable(owner)),
            ..DamageTargetAura::default()
        }
    }

    fn provides_super_break(&self) -> bool {
        true
    }

    fn allows_unbroken_super_break(&self, battle: &Battle<'_>, owner: usize) -> bool {
        let unit = &battle.units[owner];
        unit.dahlia_state
            .zone_expires_on
            .is_some_and(|end| unit.normal_turns < end)
    }

    fn ally_toughness_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        self.zone_efficiency_bp * u32::from(self.allows_unbroken_super_break(battle, owner))
    }

    fn super_break_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
        _: usize,
    ) -> u16 {
        let dancer = is_dancer(battle, owner, actor);
        match (self.eidolon >= 1, dancer) {
            (true, true) => self.dance_super_break_bp + 4_000,
            (_, true) => self.dance_super_break_bp,
            (true, false) if battle.allies().contains(&actor) => self.dance_super_break_bp,
            _ => 0,
        }
    }

    fn follow_up_hit_super_break_bp(&self) -> u16 {
        self.follow_up_super_break_bp
    }

    fn ally_break_effect_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        15_000 * u32::from(self.eidolon >= 6 && is_dancer(battle, owner, target))
    }

    fn on_ally_sustain(&self, battle: &mut Battle<'_>, owner: usize) {
        let unit = &mut battle.units[owner];
        if self.sustain_share && unit.dahlia_state.last_sustain_turn != Some(unit.normal_turns) {
            unit.dahlia_state.last_sustain_turn = Some(unit.normal_turns);
            self.share_break_effect(battle, owner, 3);
        }
    }

    fn observes_weakness_implants(&self) -> bool {
        self.implant_bonus
    }

    fn on_weakness_implanted(&self, battle: &mut Battle<'_>, owner: usize, source: usize) {
        battle.apply_ally_buff_with_id(
            owner,
            source,
            AllyBuffKind::SpeedPercent,
            3_000,
            2,
            Some(A6),
        );
    }

    fn provides_fixed_attack_toughness(&self) -> bool {
        self.eidolon >= 1 || self.implant_bonus
    }

    fn before_super_breaks(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
    ) {
        self.apply_implant_bonus(battle, owner, actor, origin);
        self.apply_eidolon_toughness(battle, owner, actor, origin);
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        primary_target: Option<usize>,
    ) {
        if !battle.is_targetable(owner) || battle.units[owner].dahlia_state.partner != Some(actor) {
            return;
        }
        let unit = &mut battle.units[owner];
        if unit.dahlia_state.last_follow_up_turn == Some(unit.normal_turns) {
            return;
        }
        unit.dahlia_state.last_follow_up_turn = Some(unit.normal_turns);
        battle.follow_up_bouncing_from(
            owner,
            origin,
            primary_target,
            self.follow_up_hits,
            self.follow_up.clone(),
        );
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 4 {
            for enemy in battle.enemies() {
                battle.apply_debuff(owner, enemy, DebuffKind::Vulnerability, 1_200, 2);
            }
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, owner: usize, _: Action) {
        if self.skill_point_refund {
            let counter = &mut battle.units[owner].dahlia_state.follow_ups_since_refund;
            *counter += 1;
            if *counter == 2 {
                *counter = 0;
                battle.restore_skill_points(owner, 1);
            }
        }
        if self.eidolon >= 6 {
            battle.advance(owner, 2_000);
            if let Some(ally) = partner(battle, owner) {
                battle.advance_ally(owner, ally, 2_000);
            }
        }
    }
}
