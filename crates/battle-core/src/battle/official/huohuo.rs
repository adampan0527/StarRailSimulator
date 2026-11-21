use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::{Healing, MICROS},
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1217";
const MAX_ENERGY: u32 = 140;
const E1_SPEED_BP: u32 = 1_200;
const E1_HEALING_BP: u32 = 2_000;
const E2_RESCUE_LIMIT: u8 = 2;
const E4_HEALING_MICROS: u64 = 800_000;
const E6_DAMAGE_BP: u32 = 5_000;
const E6_DAMAGE_TURNS: u32 = 2;
const A4_ENERGY_THRESHOLD_MILLI: u32 = 160_000;
const A4_ATTACK_BP: u32 = 2_400;

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    battle_start: &'static str,
    control_resistance: &'static str,
    talent_energy: &'static str,
}

impl KitIds {
    fn present(self, input: &OfficialCharacter) -> bool {
        [self.basic, self.skill, self.ultimate, self.talent]
            .into_iter()
            .any(|id| input.skill_parameters.contains_key(id))
    }
}

const BASE: KitIds = KitIds {
    basic: "skill.121701",
    skill: "skill.121702",
    ultimate: "skill.121703",
    talent: "skill.121704",
    battle_start: "trace.1217101",
    control_resistance: "trace.1217102",
    talent_energy: "trace.1217103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1121701",
    skill: "skill.1121702",
    ultimate: "skill.1121703",
    talent: "skill.1121704",
    battle_start: "trace.11217101",
    control_resistance: "trace.11217102",
    talent_energy: "trace.11217103",
};

#[derive(Clone, Default)]
pub(in crate::battle) struct HuohuoState {
    provision_turns: u8,
    cleanses_left: u8,
    rescues_used: u8,
}

struct Huohuo {
    enhanced: bool,
    eidolon: u8,
    battle_start: bool,
    control_resistance: bool,
    talent_energy: bool,
    skill_heal: Healing,
    adjacent_heal: Healing,
    talent_heal: Healing,
    talent_turns: u8,
    cleanse_limit: u8,
    low_hp_bp: u32,
    ultimate_energy_bp: u32,
    attack_bp: u32,
    attack_turns: u32,
    healing_micros: u64,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != MAX_ENERGY
        || input.combat_type.path != CombatPath::Abundance
        || input.combat_type.element != Element::Wind
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let (enhanced, ids) = match (BASE.present(input), NOVAFLARE.present(input)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic = input.parameters(ids.basic, 1)?[0];
    let skill = input.parameters(ids.skill, 5)?;
    let ultimate = input.parameters(ids.ultimate, 3)?;
    let talent = input.parameters(ids.talent, 7)?;
    let talent_turns = if enhanced { 3 } else { 2 };
    if skill[4] != 1.0 || ultimate[2] != 2.0 {
        return Err(invalid("skill cleanse count or ultimate duration"));
    }
    if talent[0] != f64::from(talent_turns)
        || talent[1] != 1.0
        || talent[3] != 0.0
        || talent[5] != 0.5
        || talent[6] != 6.0
    {
        return Err(invalid(ids.talent));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: MAX_ENERGY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Wind,
                multiplier_bp: ratio_bp(ids.basic, basic)?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Huohuo {
            enhanced,
            eidolon: input.eidolon,
            battle_start: trace(ids.battle_start),
            control_resistance: trace(ids.control_resistance),
            talent_energy: trace(ids.talent_energy),
            skill_heal: Healing::from_data(FORM, ids.skill, skill[0], skill[1])?,
            adjacent_heal: Healing::from_data(FORM, ids.skill, skill[2], skill[3])?,
            talent_heal: Healing::from_data(FORM, ids.talent, talent[2], talent[4])?,
            talent_turns,
            cleanse_limit: talent[6] as u8,
            low_hp_bp: u32::from(ratio_bp(ids.talent, talent[5])?),
            ultimate_energy_bp: u32::from(ratio_bp(ids.ultimate, ultimate[0])?),
            attack_bp: u32::from(ratio_bp(ids.ultimate, ultimate[1])?),
            attack_turns: ultimate[2] as u32,
            healing_micros: u64::from(input.stats.healing_bp) * 100,
        }),
    })
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn ratio_bp(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=10_000.0).contains(&rounded) {
        return Err(invalid(skill));
    }
    Ok(rounded as u16)
}

fn lowest_hp_ally(battle: &Battle<'_>) -> Option<usize> {
    battle
        .allies()
        .filter(|&ally| battle.is_targetable(ally))
        .min_by(|&left, &right| {
            let (left_hp, left_max) = battle.hp(left);
            let (right_hp, right_max) = battle.hp(right);
            (u64::from(left_hp) * u64::from(right_max))
                .cmp(&(u64::from(right_hp) * u64::from(left_max)))
        })
}

impl Huohuo {
    fn is_low_hp(&self, battle: &Battle<'_>, target: usize) -> bool {
        let (hp, max_hp) = battle.hp(target);
        u64::from(hp) * 10_000 <= u64::from(max_hp) * u64::from(self.low_hp_bp)
    }

    fn provision_active(battle: &Battle<'_>, owner: usize) -> bool {
        battle.units[owner].huohuo_state.provision_turns > 0
    }

    fn refresh_provision(&self, battle: &mut Battle<'_>, owner: usize, turns: u8) {
        let state = &mut battle.units[owner].huohuo_state;
        state.provision_turns = turns;
        state.cleanses_left = self.cleanse_limit;
    }

    fn grant_talent_provision(&self, battle: &mut Battle<'_>, owner: usize) {
        self.refresh_provision(
            battle,
            owner,
            self.talent_turns + u8::from(self.eidolon >= 1),
        );
    }

    fn apply_healing_buff(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if self.eidolon >= 6 {
            battle.apply_ally_buff(
                owner,
                target,
                AllyBuffKind::Damage,
                E6_DAMAGE_BP,
                E6_DAMAGE_TURNS,
            );
        }
    }

    fn heal(&self, battle: &mut Battle<'_>, owner: usize, target: usize, healing: Healing) {
        let e1_bonus = self.enhanced && self.eidolon >= 1 && Self::provision_active(battle, owner);
        let boost = self.healing_micros + u64::from(e1_bonus) * u64::from(E1_HEALING_BP) * 100;
        let (hp, max_hp) = battle.hp(target);
        let missing_hp = u64::from(max_hp.saturating_sub(hp));
        let scale = MICROS
            + u64::from(self.eidolon >= 4) * E4_HEALING_MICROS * missing_hp / u64::from(max_hp);
        let amount = healing.scaled_amount(battle.hp(owner).1, boost, scale);
        battle.heal_unit(owner, target, amount);
        self.apply_healing_buff(battle, owner, target);
    }

    fn heal_with_talent(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if battle.units[owner].huohuo_state.cleanses_left > 0 && battle.cleanse_one(owner, target) {
            battle.units[owner].huohuo_state.cleanses_left -= 1;
        }
        self.heal(battle, owner, target, self.talent_heal);
        if self.talent_energy {
            battle.gain_energy(owner, owner, 1_000);
        }
    }

    fn trigger_talent(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if !Self::provision_active(battle, owner) || !battle.is_targetable(actor) {
            return;
        }
        let lowest = self.enhanced.then(|| lowest_hp_ally(battle)).flatten();
        let mut actor_is_low = self.is_low_hp(battle, actor);
        self.heal_with_talent(battle, owner, actor);
        if self.enhanced {
            if let Some(target) = lowest.filter(|&target| target != actor) {
                self.heal_with_talent(battle, owner, target);
            }
            actor_is_low = self.is_low_hp(battle, actor);
        }
        for target in battle.allies() {
            if battle.is_targetable(target)
                && (if target == actor {
                    actor_is_low
                } else {
                    self.is_low_hp(battle, target)
                })
            {
                self.heal_with_talent(battle, owner, target);
            }
        }
    }

    fn skill(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        battle.cleanse_one(owner, target);
        self.heal(battle, owner, target, self.skill_heal);
        for adjacent in battle.allies() {
            if adjacent.abs_diff(target) == 1 && battle.is_targetable(adjacent) {
                self.heal(battle, owner, adjacent, self.adjacent_heal);
            }
        }
        self.grant_talent_provision(battle, owner);
    }

    fn ultimate(&self, battle: &mut Battle<'_>, owner: usize) {
        for target in battle.allies() {
            if target == owner || !battle.is_targetable(target) {
                continue;
            }
            let max_energy = battle.plan.units[target].kit.max_energy_milli();
            if max_energy > 0 {
                battle.restore_energy_flat(
                    owner,
                    target,
                    percentage_of(max_energy, self.ultimate_energy_bp),
                );
            }
            let bonus_attack = if self.enhanced
                && self.control_resistance
                && max_energy >= A4_ENERGY_THRESHOLD_MILLI
            {
                A4_ATTACK_BP
            } else {
                0
            };
            battle.apply_ally_buff(
                owner,
                target,
                AllyBuffKind::AttackPercent,
                self.attack_bp + bonus_attack,
                self.attack_turns,
            );
        }
        if self.enhanced {
            self.grant_talent_provision(battle, owner);
        }
    }

    fn rescue(&self, battle: &mut Battle<'_>, owner: usize, target: usize) -> bool {
        let state = &mut battle.units[owner].huohuo_state;
        if self.eidolon < 2 || state.provision_turns == 0 || state.rescues_used >= E2_RESCUE_LIMIT {
            return false;
        }
        let amount = (battle.hp(target).1 / 2).max(1);
        battle.heal_unit(owner, target, amount);
        if !battle.is_targetable(target) {
            return false;
        }
        let state = &mut battle.units[owner].huohuo_state;
        state.rescues_used += 1;
        state.provision_turns -= 1;
        self.apply_healing_buff(battle, owner, target);
        true
    }
}

impl Mechanics for Huohuo {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.battle_start {
            if self.enhanced {
                battle.gain_energy(owner, owner, 30_000);
            }
            self.refresh_provision(battle, owner, if self.enhanced { 2 } else { 1 });
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if actor == owner && !extra {
            let turns = &mut battle.units[owner].huohuo_state.provision_turns;
            *turns = turns.saturating_sub(1);
        }
        self.trigger_talent(battle, owner, actor);
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.skill(battle, owner, target),
            (Action::Ultimate, _) => self.ultimate(battle, owner),
            _ => {}
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if action == Action::Ultimate {
            self.trigger_talent(battle, owner, actor);
        }
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        E1_SPEED_BP * u32::from(self.eidolon >= 1 && Self::provision_active(battle, owner))
    }

    fn control_resistance_bp(&self) -> u32 {
        3_500 * u32::from(self.control_resistance)
    }

    fn prevent_defeat(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        battle.enemy_action.is_some() && self.rescue(battle, actor, actor)
    }

    fn prevent_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize, target: usize) -> bool {
        self.rescue(battle, owner, target)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill => {
                let target = target?;
                let refresh = !Self::provision_active(battle, owner)
                    && (self.talent_energy || self.eidolon >= 1);
                ((refresh || self.is_low_hp(battle, target))
                    && lowest_hp_ally(battle) == Some(target))
                .then_some(2)
            }
            Action::Basic => None,
        }
    }
}
