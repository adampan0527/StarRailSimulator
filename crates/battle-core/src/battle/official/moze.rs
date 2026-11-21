use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, SKILL_POINT_CAP, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1223";
const BASIC: &str = "skill.122301";
const SKILL: &str = "skill.122302";
const ULTIMATE: &str = "skill.122303";
const TALENT: &str = "skill.122304";
const NIGHTFEATHER: &str = "trace.1223101";
const DAGGERHOLD: &str = "trace.1223102";
const VENGEWISE: &str = "trace.1223103";
const FULL_CHARGE: u8 = 9;
const CHARGES_PER_FOLLOW_UP: u8 = 3;

#[derive(Clone, Copy)]
struct Prey {
    target: usize,
    charges: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct MozeState {
    prey: Option<Prey>,
    last_refund_turn: Option<u32>,
}

struct Moze {
    eidolon: u8,
    nightfeather: bool,
    daggerhold: bool,
    vengewise: bool,
    additional_bp: u16,
    follow_up: FollowUpAttack,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 {
        return Err(invalid("energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 1)?;
    let talent = input.parameters(TALENT, 3)?;
    if skill[1] != f64::from(FULL_CHARGE) || talent[1] != f64::from(CHARGES_PER_FOLLOW_UP) {
        return Err(invalid("Prey charge or follow-up threshold"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let follow_up_ratio = talent[2] + if input.eidolon >= 6 { 0.25 } else { 0.0 };
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: attack(BASIC, basic[0], 10, 20_000, SkillPoints::Generate)?,
        skill: attack(SKILL, skill[0], 20, 30_000, SkillPoints::Spend)?,
        ultimate: attack(ULTIMATE, ultimate[0], 30, 5_000, SkillPoints::Unchanged)?,
        mechanics: Arc::new(Moze {
            eidolon: input.eidolon,
            nightfeather: trace(NIGHTFEATHER),
            daggerhold: trace(DAGGERHOLD),
            vengewise: trace(VENGEWISE),
            additional_bp: to_basis_points(TALENT, talent[0])?,
            follow_up: FollowUpAttack {
                element: Element::Thunder,
                multiplier_bp: to_basis_points(TALENT, follow_up_ratio)?,
                toughness: 10,
                energy_milli: 10_000,
                first_hit: None,
            },
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn to_basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(&format!("{skill} damage multiplier")));
    }
    Ok(rounded as u16)
}

fn attack(
    id: &str,
    ratio: f64,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> Result<OfficialAbility, BattleError> {
    Ok(OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Thunder,
            multiplier_bp: to_basis_points(id, ratio)?,
            toughness,
        }],
        skill_points,
    })
}

impl Moze {
    fn prey(battle: &Battle<'_>, owner: usize) -> Option<Prey> {
        battle.units[owner].moze_state.prey
    }

    fn is_prey(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        Self::prey(battle, owner).is_some_and(|prey| prey.target == target)
    }

    fn return_to_field(&self, battle: &mut Battle<'_>, owner: usize) {
        let unit = &mut battle.units[owner];
        if unit.moze_state.prey.take().is_some() {
            unit.departed = false;
            if self.daggerhold {
                battle.advance(owner, 2_000);
            }
        }
    }

    fn mark_prey(battle: &mut Battle<'_>, owner: usize, target: usize) {
        let unit = &mut battle.units[owner];
        unit.moze_state.prey = Some(Prey {
            target,
            charges: FULL_CHARGE,
        });
        unit.departed = true;
    }

    fn on_prey_attacked(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        let Some(prey) = Self::prey(battle, owner) else {
            return;
        };
        let own_follow_up = source == owner && battle.in_follow_up();
        if !battle.attack_targets().contains(&prey.target) || (prey.charges == 0 && !own_follow_up)
        {
            return;
        }

        battle.secondary_hit(
            owner,
            prey.target,
            Element::Thunder,
            self.additional_bp,
            DamageKind::Additional,
            origin,
        );
        if self.eidolon >= 1 {
            battle.gain_energy(owner, owner, 2_000);
        }
        if own_follow_up && prey.charges == 0 {
            // The third charge-triggered follow-up still hits the marked enemy.
            self.return_to_field(battle, owner);
        } else if (source != owner || (!own_follow_up && origin != Action::Skill))
            && let Some(prey) = &mut battle.units[owner].moze_state.prey
        {
            // Lethal additional damage can remove Prey before the charge is spent.
            prey.charges -= 1;
            if prey.charges.is_multiple_of(CHARGES_PER_FOLLOW_UP) {
                let target = prey.target;
                battle.follow_up_single(owner, target, origin, self.follow_up.clone());
            }
        }
    }
}

impl Mechanics for Moze {
    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        battle
            .allies()
            .any(|ally| ally != actor && battle.is_targetable(ally))
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            battle.gain_energy(actor, actor, 20_000);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.daggerhold {
            battle.advance(owner, 3_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => Self::mark_prey(battle, actor, target),
            (Action::Ultimate, _) if self.eidolon >= 4 => {
                battle.apply_ally_buff(actor, actor, AllyBuffKind::Damage, 3_000, 2);
            }
            _ => {}
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        self.on_prey_attacked(battle, owner, source, origin);
        if source == owner
            && origin == Action::Ultimate
            && !battle.in_follow_up()
            && let Some(target) = battle.current_target()
        {
            battle.follow_up_single_or_random(owner, Some(target), origin, self.follow_up.clone());
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        let unit = &mut battle.units[actor];
        if self.nightfeather && unit.moze_state.last_refund_turn != Some(unit.normal_turns) {
            unit.moze_state.last_refund_turn = Some(unit.normal_turns);
            battle.set_skill_points(actor, (battle.skill_points + 1).min(SKILL_POINT_CAP));
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if Self::is_prey(battle, owner, target) {
            self.return_to_field(battle, owner);
        }
    }

    fn on_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize) {
        if !self.can_use_skill(battle, owner) {
            self.return_to_field(battle, owner);
        }
    }

    fn ultimate_counts_as_follow_up(&self) -> bool {
        self.vengewise
    }

    fn additional_debuff_count(&self, battle: &Battle<'_>, owner: usize, target: usize) -> usize {
        if Self::is_prey(battle, owner, target) {
            1 + usize::from(self.vengewise)
        } else {
            0
        }
    }

    fn follow_up_vulnerability_bp(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> u32 {
        if self.vengewise && Self::is_prey(battle, owner, enemy) {
            2_500
        } else {
            0
        }
    }

    fn target_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> u32 {
        if self.eidolon >= 2 && Self::is_prey(battle, owner, enemy) {
            4_000
        } else {
            0
        }
    }
}
