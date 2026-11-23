use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Ability, Effect, Element, Target},
};

use super::{
    super::{
        Battle, Kit,
        resolve::DamageTargetAura,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
pub(super) mod tests;

pub(super) const FORM: &str = "character.1412";
const BASIC: &str = "skill.141201";
const SKILL: &str = "skill.141202";
const ULTIMATE: &str = "skill.141203";
const TALENT: &str = "skill.141204";
const VENI: &str = "trace.1412101";
const VIDI: &str = "trace.1412102";
const VICI: &str = "trace.1412103";
const WIND: Element = Element::Wind;
const ENERGY_CAP: u32 = 130;
const CHARGE_CAP: u8 = 8;
const PEERAGE_CHARGE: u8 = 6;
const ADDITIONAL_LIMIT: u8 = 20;

#[derive(Clone, Default)]
pub(in crate::battle) struct CerydraState {
    holder: Option<usize>,
    charge: u8,
    coup: bool,
    vidi_used: bool,
    additional_used: u8,
}

struct Cerydra {
    eidolon: u8,
    veni: bool,
    vidi: bool,
    vici: bool,
    merit_atk_bp: u32,
    peerage_crit_bp: u32,
    peerage_penetration_bp: u32,
    additional_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != WIND
    {
        return Err(invalid("energy, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 4)?;
    if skill[1..4] != [1.0, f64::from(CHARGE_CAP), f64::from(PEERAGE_CHARGE)]
        || ultimate[1] != 2.0
        || talent[0] != 1.0
        || talent[3] != f64::from(ADDITIONAL_LIMIT)
    {
        return Err(invalid("frozen charge and limits"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    let ultimate_bp =
        basis_points(ULTIMATE, ultimate[0])?.saturating_add(24_000 * u16::from(input.eidolon >= 4));
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            skill_points: SkillPoints::Generate,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
        },
        skill: OfficialAbility {
            target: Target::Ally,
            skill_points: SkillPoints::Spend,
            energy_milli: 30_000,
            effects: vec![],
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            skill_points: SkillPoints::Unchanged,
            energy_milli: 5_000,
            effects: vec![hit(ultimate_bp, 20)],
        },
        mechanics: Arc::new(Cerydra {
            eidolon: input.eidolon,
            veni: trace(VENI),
            vidi: trace(VIDI),
            vici: trace(VICI),
            merit_atk_bp: u32::from(basis_points(TALENT, talent[1])?),
            peerage_crit_bp: u32::from(basis_points(SKILL, skill[0])?),
            peerage_penetration_bp: u32::from(basis_points(SKILL, skill[4])?),
            additional_bp: basis_points(TALENT, talent[2])?
                .saturating_add(30_000 * u16::from(input.eidolon >= 6)),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(id: &str, value: f64) -> Result<u16, BattleError> {
    let rounded = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(id));
    }
    Ok(rounded as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: WIND,
        multiplier_bp,
        toughness,
    }
}

fn holder(battle: &Battle<'_>, owner: usize) -> Option<usize> {
    battle.units[owner]
        .cerydra_state
        .holder
        .filter(|&target| battle.is_targetable(target))
}

fn peerage(battle: &Battle<'_>, owner: usize, actor: usize) -> bool {
    holder(battle, owner) == Some(actor)
        && battle.units[owner].cerydra_state.charge >= PEERAGE_CHARGE
}

fn set_charge(battle: &mut Battle<'_>, owner: usize, charge: u8) {
    let state = &mut battle.units[owner].cerydra_state;
    let before = std::mem::replace(&mut state.charge, charge);
    let target = state.holder;
    if before == charge {
        return;
    }
    battle.emit(
        EventKind::CerydraCharge {
            before,
            after: charge,
        },
        Some(owner),
        target,
    );
    if let Some(target) = target {
        if before < PEERAGE_CHARGE && charge >= PEERAGE_CHARGE {
            battle.emit(
                EventKind::StatusApplied(StatusKind::Peerage),
                Some(owner),
                Some(target),
            );
            battle.cleanse_crowd_control(owner, target);
        } else if before >= PEERAGE_CHARGE && charge < PEERAGE_CHARGE {
            battle.emit(
                EventKind::StatusExpired(StatusKind::Peerage),
                Some(owner),
                Some(target),
            );
        }
    }
}

fn gain_charge(battle: &mut Battle<'_>, owner: usize, gain: u8) {
    let state = &battle.units[owner].cerydra_state;
    if !state.coup {
        set_charge(
            battle,
            owner,
            state.charge.saturating_add(gain).min(CHARGE_CAP),
        );
    }
}

fn set_holder(battle: &mut Battle<'_>, owner: usize, target: usize) {
    let previous = battle.units[owner].cerydra_state.holder;
    if previous == Some(target) {
        return;
    }
    if let Some(previous) = previous {
        set_charge(battle, owner, 0);
        battle.emit(
            EventKind::StatusExpired(StatusKind::MilitaryMerit),
            Some(owner),
            Some(previous),
        );
    }
    battle.units[owner].cerydra_state.holder = Some(target);
    battle.emit(
        EventKind::StatusApplied(StatusKind::MilitaryMerit),
        Some(owner),
        Some(target),
    );
    if peerage(battle, owner, target) {
        battle.emit(
            EventKind::StatusApplied(StatusKind::Peerage),
            Some(owner),
            Some(target),
        );
        battle.cleanse_crowd_control(owner, target);
    }
}

fn preferred_holder(battle: &Battle<'_>, owner: usize) -> usize {
    battle
        .allies()
        .filter(|&ally| ally != owner && battle.is_targetable(ally))
        .max_by_key(|&ally| {
            let copyable = match &battle.plan.units[ally].kit {
                Kit::Official(kit) => {
                    kit.mechanics
                        .can_copy_skill(battle, ally, kit.skill.resolve().0)
                }
                _ => false,
            };
            (
                copyable,
                battle.plan.units[ally].stats.attack,
                Reverse(ally),
            )
        })
        .unwrap_or(owner)
}

impl Mechanics for Cerydra {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let Some(target) = target else { return };
                set_holder(battle, owner, target);
                gain_charge(battle, owner, 1);
                if self.vici {
                    battle.apply_ally_buff(owner, owner, AllyBuffKind::SpeedFlat, 20_000_000, 3);
                    if target != owner {
                        battle.apply_ally_buff(
                            owner,
                            target,
                            AllyBuffKind::SpeedFlat,
                            20_000_000,
                            3,
                        );
                    }
                }
                if self.eidolon >= 1 {
                    battle.restore_energy_flat(owner, target, 2_000);
                }
            }
            Action::Ultimate => {
                if holder(battle, owner).is_none()
                    && let Some(first) = battle.allies().find(|&unit| battle.is_targetable(unit))
                {
                    set_holder(battle, owner, first);
                }
                gain_charge(battle, owner, 2);
                battle.units[owner].cerydra_state.additional_used = 0;
            }
            _ => {}
        }
    }

    fn tracks_skill_copies(&self) -> bool {
        true
    }

    fn before_ally_skill_copy(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        let Some(action @ (Action::Skill | Action::EnhancedSkill)) = battle.action else {
            return;
        };
        if battle.nested_skill.is_some()
            || battle.units[owner].cerydra_state.coup
            || !peerage(battle, owner, actor)
            || !battle
                .mechanics(actor)
                .is_some_and(|rules| rules.can_copy_skill(battle, actor, ability))
        {
            return;
        }
        battle.units[owner].cerydra_state.coup = true;
        battle.perform_skill_copy(owner, actor, target, ability, action);
        if target.is_some_and(|target| !battle.is_targetable(target)) {
            battle.action_target = battle.random_living_enemy();
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        let state = &battle.units[owner].cerydra_state;
        if state.holder != Some(actor) {
            return;
        }
        if state.coup {
            if battle.nested_skill.is_none() {
                let charge = state.charge.saturating_sub(PEERAGE_CHARGE);
                battle.units[owner].cerydra_state.coup = false;
                set_charge(battle, owner, charge);
                if self.vici && battle.is_targetable(owner) {
                    battle.gain_energy(actor, owner, 5_000);
                }
            }
            return;
        }
        if matches!(
            action,
            Action::Basic | Action::Skill | Action::EnhancedSkill
        ) {
            gain_charge(battle, owner, 1);
            if self.vici && battle.is_targetable(owner) {
                battle.gain_energy(actor, owner, 5_000);
            }
        } else if self.vidi && action.is_ultimate() && !state.vidi_used && state.charge < CHARGE_CAP
        {
            battle.units[owner].cerydra_state.vidi_used = true;
            gain_charge(battle, owner, 1);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        primary: Option<usize>,
    ) {
        if holder(battle, owner) != Some(actor)
            || !battle.is_targetable(owner)
            || battle.units[owner].cerydra_state.additional_used >= ADDITIONAL_LIMIT
        {
            return;
        }
        let target = primary
            .filter(|&target| battle.is_targetable(target))
            .or_else(|| {
                battle
                    .attack_targets()
                    .iter()
                    .copied()
                    .find(|&target| battle.is_targetable(target))
            });
        if let Some(target) = target {
            battle.units[owner].cerydra_state.additional_used += 1;
            battle.secondary_hit(
                owner,
                target,
                WIND,
                self.additional_bp,
                DamageKind::Additional,
                origin,
            );
        }
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if holder(battle, owner) == Some(target) {
            percentage_of(
                battle.effective_attack_excluding(owner, Some(owner)),
                self.merit_atk_bp,
            )
        } else {
            0
        }
    }

    fn ally_skill_crit_damage_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        self.peerage_crit_bp * u32::from(peerage(battle, owner, actor))
    }

    fn ally_skill_defense_ignore_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 1 && peerage(battle, owner, actor))
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        let holder = holder(battle, owner);
        DamageTargetAura {
            resistance_down_bp: self.peerage_penetration_bp
                * u32::from(peerage(battle, owner, actor))
                + 2_000
                    * u32::from(
                        self.eidolon >= 6
                            && (holder == Some(actor) || (actor == owner && holder.is_some())),
                    ),
            defense_down_bp: 1_600 * u32::from(self.eidolon >= 1 && holder == Some(actor)),
            ..DamageTargetAura::default()
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
        _: Element,
    ) -> u32 {
        if self.eidolon < 2 {
            return 0;
        }
        let holder = holder(battle, owner);
        4_000 * u32::from(holder == Some(actor))
            + 16_000 * u32::from(actor == owner && holder.is_some_and(|target| target != owner))
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        10_000 * u32::from(self.vidi)
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if self.veni {
            1_800 * (battle.effective_attack(owner).saturating_sub(2_000) / 100).min(20)
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        let recipient = preferred_holder(battle, owner);
        match action {
            Action::Skill
                if target == Some(recipient)
                    && (holder(battle, owner) != Some(recipient)
                        || (self.vici
                            && battle.ally_buff_value(
                                owner,
                                recipient,
                                AllyBuffKind::SpeedFlat,
                            ) == 0)) =>
            {
                Some(1)
            }
            _ => None,
        }
    }
}
