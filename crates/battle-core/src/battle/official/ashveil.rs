use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1504";
const BASIC: &str = "skill.150401";
const SKILL: &str = "skill.150402";
const ULTIMATE: &str = "skill.150403";
const TALENT: &str = "skill.150404";
const DAMNATION_TRAIL: &str = "trace.1504101";
const PHANTOM_LIMB: &str = "trace.1504102";
const ALPHA_WOLF: &str = "trace.1504103";
const ENERGY_CAP: u32 = 150;
const STARTING_CHARGES: u8 = 2;
const CHARGE_CAP: u8 = 3;
const GLUTTONY_PER_HIT: u8 = 4;
const GLUTTONY_GAIN: u8 = 2;
const BASE_GLUTTONY_CAP: u8 = 12;
const E2_GLUTTONY_CAP: u8 = 18;
const E6_LIFETIME_CAP: u8 = 30;

#[derive(Clone, Default)]
pub(in crate::battle) struct AshveilState {
    bait: Option<usize>,
    allied_hit: Option<(usize, usize)>,
    charges: u8,
    gluttony: u8,
    lifetime_gluttony: u8,
    repeated_skill_target: Option<usize>,
}

struct Ashveil {
    skill_bp: u16,
    skill_extra_bp: u16,
    defense_down_bp: u32,
    enhanced_hit_bp: u16,
    follow_up: FollowUpAttack,
    eidolon: u8,
    damnation_trail: bool,
    phantom_limb: bool,
    alpha_wolf: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Hunt
        || input.combat_type.element != Element::Thunder
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 7)?;
    if skill[1] != 1.0 || skill[4] != 1.0 {
        return Err(invalid(SKILL));
    }
    if ultimate[1] != f64::from(CHARGE_CAP) || ultimate[2] != f64::from(GLUTTONY_PER_HIT) {
        return Err(invalid(ULTIMATE));
    }
    if talent[0] != f64::from(STARTING_CHARGES)
        || talent[1] != f64::from(CHARGE_CAP)
        || talent[2] != 1.0
        || talent[4] != f64::from(GLUTTONY_GAIN)
        || talent[5] != f64::from(BASE_GLUTTONY_CAP)
        || talent[6] != 8.0
    {
        return Err(invalid(TALENT));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let skill_bp = ratio(SKILL, skill[0])?;
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: hit(ratio(BASIC, basic[0])?, 10, 20_000, SkillPoints::Generate),
        skill: hit(skill_bp, 20, 30_000, SkillPoints::Spend),
        ultimate: hit(
            ratio(ULTIMATE, ultimate[0])?,
            30,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Ashveil {
            skill_bp,
            skill_extra_bp: ratio(SKILL, skill[2])?,
            defense_down_bp: u32::from(ratio(SKILL, skill[3])?),
            enhanced_hit_bp: ratio(ULTIMATE, ultimate[3])?,
            follow_up: FollowUpAttack {
                element: Element::Thunder,
                multiplier_bp: ratio(TALENT, talent[3])?,
                toughness: 5,
                energy_milli: 5_000,
                first_hit: None,
            },
            eidolon: input.eidolon,
            damnation_trail: has_trace(DAMNATION_TRAIL),
            phantom_limb: has_trace(PHANTOM_LIMB),
            alpha_wolf: has_trace(ALPHA_WOLF),
        }),
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
            element: Element::Thunder,
            multiplier_bp,
            toughness,
        }],
        skill_points: points,
    }
}

fn bait(battle: &Battle<'_>, owner: usize) -> Option<usize> {
    battle.units[owner]
        .ashveil_state
        .bait
        .filter(|&target| battle.is_targetable(target))
}

fn set_bait(battle: &mut Battle<'_>, owner: usize, target: Option<usize>) {
    let previous = std::mem::replace(&mut battle.units[owner].ashveil_state.bait, target);
    if previous == target {
        return;
    }
    if let Some(previous) = previous {
        battle.emit(
            EventKind::StatusExpired(StatusKind::Bait),
            Some(owner),
            Some(previous),
        );
    }
    if let Some(target) = target {
        battle.emit(
            EventKind::StatusApplied(StatusKind::Bait),
            Some(owner),
            Some(target),
        );
        battle.record_enemy_debuff(target);
    }
}

fn find_bait(battle: &mut Battle<'_>, owner: usize) {
    if bait(battle, owner).is_none() {
        let lowest_hp = battle
            .enemies()
            .filter(|&enemy| battle.is_targetable(enemy))
            .min_by_key(|&enemy| (battle.units[enemy].hp, enemy));
        set_bait(battle, owner, lowest_hp);
    }
}

fn set_charges(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].ashveil_state.charges, after);
    if before != after {
        battle.emit(
            EventKind::AshveilCharge { before, after },
            Some(owner),
            None,
        );
    }
}

impl Ashveil {
    fn gain_gluttony(&self, battle: &mut Battle<'_>, owner: usize, gain: u8) {
        let state = &mut battle.units[owner].ashveil_state;
        let cap = if self.eidolon >= 2 {
            E2_GLUTTONY_CAP
        } else {
            BASE_GLUTTONY_CAP
        };
        let after = state.gluttony.saturating_add(gain).min(cap);
        state.lifetime_gluttony = state
            .lifetime_gluttony
            .saturating_add(after - state.gluttony)
            .min(E6_LIFETIME_CAP);
        set_gluttony(battle, owner, after);
    }
}

fn set_gluttony(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].ashveil_state.gluttony, after);
    if before != after {
        battle.emit(EventKind::Gluttony { before, after }, Some(owner), None);
    }
}

impl Mechanics for Ashveil {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        set_charges(battle, owner, STARTING_CHARGES);
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        find_bait(battle, owner);
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        set_bait(battle, owner, None);
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                battle.units[owner].ashveil_state.repeated_skill_target =
                    target.filter(|&target| bait(battle, owner) == Some(target));
                set_bait(battle, owner, target);
                if self.damnation_trail {
                    self.gain_gluttony(battle, owner, 1);
                }
            }
            Action::Ultimate => {
                set_bait(battle, owner, target);
                if self.damnation_trail {
                    self.gain_gluttony(battle, owner, 2);
                }
                if self.eidolon >= 4 {
                    battle.apply_ally_buff(owner, owner, AllyBuffKind::AttackPercent, 4_000, 3);
                }
            }
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
            Action::Skill => {
                let repeated_target = battle.units[owner]
                    .ashveil_state
                    .repeated_skill_target
                    .take();
                if let Some(target) = target {
                    if repeated_target == Some(target) {
                        battle.attack_hit(owner, target, Element::Thunder, self.skill_extra_bp, 0);
                        battle.restore_skill_points(owner, 1);
                    }
                    if battle.is_targetable(target) {
                        set_bait(battle, owner, Some(target));
                    }
                }
            }
            Action::Ultimate => {
                set_charges(battle, owner, CHARGE_CAP);
                battle.queue_special_follow_up(owner, action, self.follow_up.clone());
            }
            _ => {}
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: Option<usize>,
    ) -> bool {
        let Some(target) = target.filter(|&target| {
            battle.units[owner].ashveil_state.repeated_skill_target == Some(target)
        }) else {
            return false;
        };
        battle.attack_hit(owner, target, Element::Thunder, self.skill_bp, 20);
        battle.attack_hit(owner, target, Element::Thunder, self.skill_extra_bp, 0);
        battle.notify_attack(owner, Action::Skill, Some(target));
        true
    }

    fn on_ally_first_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
    ) {
        if source != owner
            && battle.is_targetable(owner)
            && battle.is_party_unit(source)
            && bait(battle, owner) == Some(target)
        {
            battle.units[owner].ashveil_state.allied_hit = Some((source, target));
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        let Some((attacker, target)) = battle.units[owner].ashveil_state.allied_hit else {
            return;
        };
        if source != attacker {
            return;
        }
        battle.units[owner].ashveil_state.allied_hit = None;
        battle.restore_energy_flat(owner, owner, 8_000);
        if battle.is_targetable(target) && self.can_execute_follow_up(battle, owner) {
            battle.follow_up_single(owner, target, origin, self.follow_up.clone());
        }
    }

    fn can_execute_follow_up(&self, battle: &Battle<'_>, owner: usize) -> bool {
        battle.is_targetable(owner)
            && !battle.is_crowd_controlled(owner)
            && battle.units[owner].ashveil_state.charges > 0
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, owner: usize) {
        let remaining = battle.units[owner].ashveil_state.charges.saturating_sub(1);
        set_charges(battle, owner, remaining);
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, owner: usize, _: Action) {
        self.gain_gluttony(battle, owner, GLUTTONY_GAIN);
    }

    fn special_follow_up_target(&self, battle: &Battle<'_>, owner: usize) -> Option<usize> {
        if battle.is_crowd_controlled(owner) {
            return None;
        }
        bait(battle, owner)
    }

    fn resolve_special_follow_up(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        origin: Action,
    ) {
        battle.follow_up_hit(
            owner,
            target,
            origin,
            Element::Thunder,
            self.follow_up.multiplier_bp,
            self.follow_up.toughness,
        );
        self.gain_gluttony(battle, owner, GLUTTONY_GAIN);

        let mut spent = 0_u16;
        while let Some(target) = bait(battle, owner) {
            let stacks = battle.units[owner].ashveil_state.gluttony;
            if stacks < GLUTTONY_PER_HIT {
                break;
            }
            set_gluttony(battle, owner, stacks - GLUTTONY_PER_HIT);
            spent += u16::from(GLUTTONY_PER_HIT);
            battle.follow_up_hit(
                owner,
                target,
                origin,
                Element::Thunder,
                self.enhanced_hit_bp,
                self.follow_up.toughness,
            );
        }
        if self.eidolon >= 2 {
            self.gain_gluttony(battle, owner, (spent * 35 / 100) as u8);
        }
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, source: usize, target: usize) {
        if self.damnation_trail && source == owner && battle.in_follow_up() {
            self.gain_gluttony(battle, owner, 1);
        }
        if battle.units[owner].ashveil_state.bait == Some(target) {
            find_bait(battle, owner);
        }
    }

    fn additional_debuff_count(&self, battle: &Battle<'_>, owner: usize, target: usize) -> usize {
        usize::from(battle.is_targetable(owner) && bait(battle, owner) == Some(target))
    }

    fn target_defense_down_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if battle.is_targetable(owner) && bait(battle, owner).is_some() {
            self.defense_down_bp
        } else {
            0
        }
    }

    fn target_vulnerability_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.eidolon < 1 || !battle.is_targetable(owner) {
            return 0;
        }
        let (hp, max_hp) = battle.hp(target);
        if hp <= max_hp / 2 { 3_600 } else { 2_400 }
    }

    fn all_resistance_down_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if self.eidolon >= 6 && battle.is_targetable(owner) && bait(battle, owner).is_some() {
            2_000
        } else {
            0
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[owner].ashveil_state;
        let follow_up = if self.phantom_limb && kind == DamageKind::FollowUp {
            8_000 + 1_000 * u32::from(state.gluttony)
        } else {
            0
        };
        follow_up + 400 * u32::from(state.lifetime_gluttony) * u32::from(self.eidolon >= 6)
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        4_000 * u32::from(self.alpha_wolf && battle.is_targetable(owner))
    }

    fn follow_up_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        8_000 * u32::from(self.alpha_wolf && battle.is_targetable(owner))
    }
}
