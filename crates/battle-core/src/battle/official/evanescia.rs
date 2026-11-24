use std::{ops::RangeInclusive, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Ability, Effect, Element, Target},
};

use super::{
    super::{Battle, elation::AhaInstant, resolve::ElationHit, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1505";
const BASIC: &str = "skill.150501";
const SKILL: &str = "skill.150502";
const ULTIMATE: &str = "skill.150503";
const TALENT: &str = "skill.150504";
const ELATION: &str = "skill.150520";
const PHYSICAL: Element = Element::Physical;

const ENERGY_CAP: u32 = 480;
const ULTIMATE_COST_MILLI: u32 = 240_000;
const PARTICIPANT: u16 = 146;
const BANGER_PER_ENERGY_GAIN_CAP: u32 = 100;
const PARTY_TRICK_THRESHOLD: u32 = 480;

#[derive(Clone, Default)]
pub(in crate::battle) struct EvanesciaState {
    conversion_in_progress: bool,
    fox_energy_milli: u32,
    pending_fox_attacks: u32,
    fox_queued: bool,
    attack_banger: Option<u32>,
    ultimate_bounces: u8,
    overflow_energy_milli: u32,
    party_trick_earned: u32,
    next_ultimate_amplified: bool,
    active_ultimate_factor_bp: u32,
    ultimates_until_refund: u8,
}

struct Evanescia {
    eidolon: u8,
    watch_all_revels: bool,
    weigh_all_truths: bool,
    best_all_blooms: bool,
    skill_effects: [Effect; 2],
    skill_banger_bp: u32,
    skill_punchline: u32,
    ultimate_bounce_count: u8,
    ultimate_bounce_bp: u16,
    ultimate_banger_area_bp: u32,
    ultimate_banger_bounce_bp: u32,
    fox_attack_bp: u16,
    fox_banger_bp: u32,
    fox_threshold_milli: u32,
    fox_energy_milli: u32,
    elation_from_crit_bp: u32,
    elation_banger: u32,
    elation_skill_bp: u32,
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id} combat parameters"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(id));
    }
    Ok(value as u16)
}

fn points(id: &str, value: f64, limit: u32) -> Result<u32, BattleError> {
    if !(1.0..=f64::from(limit)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u32)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: PHYSICAL,
        multiplier_bp,
        toughness,
    }
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != PHYSICAL
    {
        return Err(invalid("energy capacity or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 8)?;
    let elation = input.parameters(ELATION, 2)?;
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    let skill_effects = [
        hit(ratio_bp(SKILL, skill[1])?, 20),
        Effect::BlastAdjacent {
            element: PHYSICAL,
            multiplier_bp: ratio_bp(SKILL, skill[2])?,
            toughness: 10,
        },
    ];
    let mechanics = Evanescia {
        eidolon: input.eidolon,
        watch_all_revels: trace("trace.1505101"),
        weigh_all_truths: trace("trace.1505102"),
        best_all_blooms: trace("trace.1505103"),
        skill_effects,
        skill_banger_bp: u32::from(ratio_bp(TALENT, talent[6])?),
        skill_punchline: points(SKILL, skill[3], 1_000)?,
        ultimate_bounce_count: points(ULTIMATE, ultimate[1], 20)? as u8,
        ultimate_bounce_bp: ratio_bp(ULTIMATE, ultimate[2])?,
        ultimate_banger_area_bp: u32::from(ratio_bp(TALENT, talent[5])?),
        ultimate_banger_bounce_bp: u32::from(ratio_bp(TALENT, talent[7])?),
        fox_attack_bp: ratio_bp(TALENT, talent[0])?,
        fox_banger_bp: u32::from(ratio_bp(TALENT, talent[1])?),
        fox_threshold_milli: points(TALENT, talent[2], ENERGY_CAP)? * 1_000,
        fox_energy_milli: points(TALENT, talent[3], 100)? * 1_000,
        elation_from_crit_bp: u32::from(ratio_bp(TALENT, talent[4])?),
        elation_banger: points(ELATION, elation[0], 100)?,
        elation_skill_bp: u32::from(ratio_bp(ELATION, elation[1])?),
    };

    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            effects: vec![hit(ratio_bp(BASIC, basic[0])?, 10)],
            energy_milli: 20_000,
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            effects: skill_effects.to_vec(),
            energy_milli: 30_000,
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            effects: vec![hit(ratio_bp(ULTIMATE, ultimate[0])?, 20)],
            energy_milli: 5_000,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

impl Evanescia {
    fn queue_fox(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &battle.units[actor].evanescia_state;
        if state.pending_fox_attacks == 0
            || state.fox_queued
            || !battle.is_targetable(actor)
            || !battle.in_territory(actor)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return;
        }
        battle.units[actor].evanescia_state.fox_queued = true;
        let origin = battle.action.unwrap_or(Action::Skill);
        battle.follow_up(
            actor,
            PHYSICAL,
            self.fox_attack_bp,
            10,
            origin,
            self.fox_energy_milli,
        );
    }

    fn add_fox_energy(&self, battle: &mut Battle<'_>, actor: usize, milli: u32) {
        let state = &mut battle.units[actor].evanescia_state;
        let energy = state.fox_energy_milli + milli.min(self.fox_threshold_milli);
        state.pending_fox_attacks = state
            .pending_fox_attacks
            .saturating_add(energy / self.fox_threshold_milli);
        state.fox_energy_milli = energy % self.fox_threshold_milli;
        self.queue_fox(battle, actor);
    }

    fn elation_hit(
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
        multiplier_bp: u32,
        punchline: u32,
    ) {
        battle.elation_hit(
            actor,
            target,
            ElationHit {
                element: PHYSICAL,
                multiplier_bp,
                punchline,
                toughness: 0,
                origin,
                factor_bp: 10_000,
                assisted_by: None,
            },
        );
    }

    fn skill_banger_hits(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: Option<usize>,
        banger: Option<u32>,
    ) {
        if let (Some(target), Some(banger)) = (target, banger) {
            for enemy in std::iter::once(target).chain(battle.adjacent_enemies(target)) {
                Self::elation_hit(
                    battle,
                    actor,
                    enemy,
                    Action::Skill,
                    self.skill_banger_bp,
                    banger,
                );
            }
        }
    }

    fn finish_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &battle.units[actor].evanescia_state;
        let banger = state.attack_banger;
        let bounces = state.ultimate_bounces;
        let floor = battle.energy_capacity_milli(actor) / 1_000;
        let punchline = banger.map(|points| points.max(floor));
        if let Some(punchline) = punchline {
            for target in battle.enemies() {
                Self::elation_hit(
                    battle,
                    actor,
                    target,
                    Action::Ultimate,
                    self.ultimate_banger_area_bp,
                    punchline,
                );
            }
        }
        for _ in 0..bounces {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.attack_hit(actor, target, PHYSICAL, self.ultimate_bounce_bp, 5);
            if let Some(punchline) = punchline {
                Self::elation_hit(
                    battle,
                    actor,
                    target,
                    Action::Ultimate,
                    self.ultimate_banger_bounce_bp,
                    punchline,
                );
            }
        }

        let state = &mut battle.units[actor].evanescia_state;
        state.active_ultimate_factor_bp = 0;
        let overflow = std::mem::take(&mut state.overflow_energy_milli);
        let e6_refund = self.eidolon == 6 && state.ultimates_until_refund == 0;
        if self.eidolon == 6 {
            state.ultimates_until_refund = if e6_refund {
                3
            } else {
                state.ultimates_until_refund - 1
            };
        }
        if overflow > 0 {
            battle.units[actor].evanescia_state.conversion_in_progress = true;
            battle.restore_energy_flat(actor, actor, overflow);
            battle.units[actor].evanescia_state.conversion_in_progress = false;
        }
        if e6_refund {
            battle.restore_energy_flat(actor, actor, 120_000);
        }
    }
}

impl Mechanics for Evanescia {
    fn elation_participant(&self) -> Option<u16> {
        Some(PARTICIPANT)
    }

    fn ultimate_energy_range(&self, action: Action) -> Option<RangeInclusive<u32>> {
        (action == Action::Ultimate).then_some(ULTIMATE_COST_MILLI..=ENERGY_CAP * 1_000)
    }

    fn elation_from_crit_damage_bp(&self, crit_damage_bp: u32) -> u32 {
        percentage_of(crit_damage_bp, self.elation_from_crit_bp)
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        3_000 * u32::from(self.watch_all_revels)
    }

    fn crit_damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        3_600 * u32::from(self.eidolon >= 2)
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, _: Element) -> u32 {
        2_000 * u32::from(self.eidolon >= 1)
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, _: usize, _: usize) -> u32 {
        let ascension = battle
            .elation_ascension()
            .map_or(0, |form| if form.innate_level >= 2 { 2_400 } else { 1_000 });
        ascension + 1_500 * u32::from(self.eidolon >= 4)
    }

    fn certified_banger_duration(&self) -> u32 {
        2 + u32::from(self.eidolon == 6)
    }

    fn ally_merrymake_bp(&self, battle: &Battle<'_>, source: usize, actor: usize) -> u32 {
        if source == actor && self.eidolon == 6 {
            1_500 + 200 * (battle.certified_banger(actor).unwrap_or(0).min(1_000) / 100)
        } else {
            0
        }
    }

    fn on_energy_gained(&self, battle: &mut Battle<'_>, actor: usize, milli: u32) {
        self.add_fox_energy(battle, actor, milli);
        if !battle.units[actor].evanescia_state.conversion_in_progress {
            let banger = (milli / 1_000).min(BANGER_PER_ENERGY_GAIN_CAP);
            if banger > 0 {
                battle.units[actor].evanescia_state.conversion_in_progress = true;
                battle.grant_certified_banger(actor, banger);
                battle.units[actor].evanescia_state.conversion_in_progress = false;
            }
        }
    }

    fn on_energy_overflow(&self, battle: &mut Battle<'_>, actor: usize, amount_milli: u32) {
        if let Some(form) = battle.elation_ascension() {
            let cap = if form.innate_level >= 2 {
                240_000
            } else {
                60_000
            };
            let stored = &mut battle.units[actor].evanescia_state.overflow_energy_milli;
            *stored = stored.saturating_add(amount_milli).min(cap);
        }
    }

    fn on_banger_gained(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        recipient: usize,
        amount: u32,
    ) {
        if owner != recipient {
            if self.watch_all_revels
                && battle
                    .mechanics(recipient)
                    .and_then(|rules| rules.elation_participant())
                    .is_some_and(|participant| participant < PARTICIPANT)
            {
                let ratio = if self.eidolon >= 2 { 7_500 } else { 5_000 };
                battle.grant_certified_banger(owner, percentage_of(amount, ratio));
            }
            return;
        }
        if battle.elation_ascension().is_some() {
            let state = &mut battle.units[owner].evanescia_state;
            let earned = state.party_trick_earned.saturating_add(amount);
            state.next_ultimate_amplified |= earned >= PARTY_TRICK_THRESHOLD;
            state.party_trick_earned = earned % PARTY_TRICK_THRESHOLD;
        }
        if !battle.units[owner].evanescia_state.conversion_in_progress {
            battle.units[owner].evanescia_state.conversion_in_progress = true;
            battle.gain_energy(owner, owner, amount.min(BANGER_PER_ENERGY_GAIN_CAP) * 1_000);
            battle.units[owner].evanescia_state.conversion_in_progress = false;
        }
    }

    fn on_banger_expired(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        recipient: usize,
        amount: u32,
    ) {
        if self.best_all_blooms && owner != recipient {
            let ratio = if self.eidolon >= 2 { 10_000 } else { 5_000 };
            battle.grant_certified_banger(owner, percentage_of(amount, ratio));
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, actor: usize) {
        // A queued attack without a surviving target was dropped by the old wave.
        battle.units[actor].evanescia_state.fox_queued = false;
        self.queue_fox(battle, actor);
    }

    fn can_execute_follow_up(&self, battle: &Battle<'_>, actor: usize) -> bool {
        battle.units[actor].evanescia_state.pending_fox_attacks > 0
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].evanescia_state;
        state.fox_queued = false;
        state.pending_fox_attacks -= 1;
        if self.weigh_all_truths {
            for target in battle.enemies() {
                if battle.is_targetable(target) {
                    battle.apply_vulnerability(actor, target, 1_200, 3);
                }
            }
        }
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
    ) {
        if let Some(banger) = battle.certified_banger(actor) {
            Self::elation_hit(battle, actor, target, origin, self.fox_banger_bp, banger);
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _: Action) {
        self.queue_fox(battle, actor);
        if self.eidolon >= 1 {
            battle.queue_triggered_elation_skill(actor);
        }
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        for target in battle.enemies() {
            battle.elation_hit(
                actor,
                target,
                ElationHit {
                    element: PHYSICAL,
                    multiplier_bp: self.elation_skill_bp,
                    punchline: instant.punchline,
                    toughness: 20,
                    origin: Action::ElationSkill,
                    factor_bp: instant.skill_damage_factor_bp,
                    assisted_by: None,
                },
            );
        }
        battle.gain_energy(actor, actor, 5_000);
        battle.grant_certified_banger(
            actor,
            self.elation_banger + 10 * u32::from(self.eidolon >= 1),
        );
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        let banger = battle.certified_banger(actor);
        battle.units[actor].evanescia_state.attack_banger = banger;
        if action != Action::Ultimate {
            return;
        }
        let extra_bounces = if self.watch_all_revels {
            match battle
                .enemies()
                .filter(|&enemy| battle.is_targetable(enemy))
                .count()
            {
                0 => 0,
                1 => 4,
                2 => 2,
                _ => 1,
            }
        } else {
            0
        };
        let ascension = battle.elation_ascension();
        let state = &mut battle.units[actor].evanescia_state;
        state.ultimate_bounces = self.ultimate_bounce_count + extra_bounces;
        if let Some(form) = ascension
            && std::mem::take(&mut state.next_ultimate_amplified)
        {
            state.active_ultimate_factor_bp = if form.innate_level >= 2 {
                25_000
            } else {
                17_500
            };
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
            Action::Skill => {
                let banger = battle.units[actor].evanescia_state.attack_banger;
                self.skill_banger_hits(battle, actor, target, banger);
                battle.add_punchline(actor, self.skill_punchline);
            }
            Action::Ultimate => self.finish_ultimate(battle, actor),
            Action::Basic
            | Action::EnhancedSkill
            | Action::AssistSkill
            | Action::EnhancedUltimate
            | Action::ElationSkill => {}
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: Option<usize>,
    ) -> bool {
        let banger = battle.certified_banger(actor);
        battle.resolve_ability(
            actor,
            target,
            Ability {
                target: Target::Enemy,
                effects: &self.skill_effects,
                energy_milli: 30_000,
            },
        );
        self.skill_banger_hits(battle, actor, target, banger);
        battle.add_punchline(actor, self.skill_punchline);
        battle.notify_attack(actor, Action::Skill, target);
        true
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        let factor = battle.units[actor]
            .evanescia_state
            .active_ultimate_factor_bp;
        if factor > 0
            && action == Some(Action::Ultimate)
            && matches!(kind, DamageKind::Direct | DamageKind::Elation)
        {
            percentage_of(base, factor)
        } else {
            base
        }
    }
}
