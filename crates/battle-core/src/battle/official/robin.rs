use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, SummonTurn, resolve::DamageTargetAura},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1309";
const BASIC: &str = "skill.130901";
const SKILL: &str = "skill.130902";
const ULTIMATE: &str = "skill.130903";
const TALENT: &str = "skill.130904";
const A2: &str = "trace.1309101";
const A4: &str = "trace.1309102";
const A6: &str = "trace.1309103";

const ENERGY_CAPACITY: u32 = 160;
const CONCERTO_SPEED_MICROS: u64 = 90_000_000;
const A2_ADVANCE_BP: u16 = 2_500;
const A4_FOLLOW_UP_CRIT_DAMAGE_BP: u32 = 2_500;
const A6_ENERGY_MILLI: u32 = 5_000;
const E1_PENETRATION_BP: u32 = 2_400;
const E2_SPEED_BP: u32 = 1_600;
const E2_ENERGY_MILLI: u32 = 1_000;
const E4_EFFECT_RESISTANCE_BP: u32 = 5_000;
const E6_HIT_COUNT: u8 = 8;
const E6_CRIT_DAMAGE_BP: u32 = 45_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct RobinState {
    skill_turns: u32,
    concerto: Option<Concerto>,
}

#[derive(Clone, Copy)]
struct Concerto {
    attack_bonus: u32,
    encore_hits: u8,
}

struct Robin {
    skill_damage_bp: u32,
    skill_turns: u32,
    concerto_attack_ratio_bp: u16,
    concerto_attack_flat_centi: u32,
    concerto_hit_bp: u16,
    concerto_crit_damage_bp: u32,
    talent_crit_damage_bp: u32,
    talent_energy_milli: u32,
    eidolon: u8,
    first_advance: bool,
    follow_up_crit_damage: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != Element::Physical
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 2)?;
    let concerto_attack_flat_centi = (ultimate[2] * 100.0).round();
    if skill[1] != 3.0
        || ultimate[1] != 90.0
        || ultimate[4] != 1.0
        || ultimate[5] != 1.5
        || talent[1] != 2.0
        || !(1.0..=f64::from(u32::MAX)).contains(&concerto_attack_flat_centi)
    {
        return Err(invalid("skill duration or fixed Concerto parameters"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Physical,
                multiplier_bp: basis_points(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 30_000 + A6_ENERGY_MILLI * u32::from(trace(A6)),
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Robin {
            skill_damage_bp: u32::from(basis_points(SKILL, skill[0])?),
            skill_turns: skill[1] as u32,
            concerto_attack_ratio_bp: basis_points(ULTIMATE, ultimate[0])?,
            concerto_attack_flat_centi: concerto_attack_flat_centi as u32,
            concerto_hit_bp: basis_points(ULTIMATE, ultimate[3])?,
            concerto_crit_damage_bp: u32::from(basis_points(ULTIMATE, ultimate[5])?),
            talent_crit_damage_bp: u32::from(basis_points(TALENT, talent[0])?),
            talent_energy_milli: talent[1] as u32 * 1_000,
            eidolon: input.eidolon,
            first_advance: trace(A2),
            follow_up_crit_damage: trace(A4),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(field: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(field));
    }
    Ok(bp as u16)
}

impl Robin {
    fn concerto_active(battle: &Battle<'_>, owner: usize) -> bool {
        battle.units[owner].robin_state.concerto.is_some()
    }

    fn start_concerto(&self, battle: &mut Battle<'_>, owner: usize) {
        let attack = u64::from(battle.effective_attack(owner));
        let bonus = (attack * u64::from(self.concerto_attack_ratio_bp)
            + 100 * u64::from(self.concerto_attack_flat_centi))
            / 10_000;
        battle.units[owner].robin_state.concerto = Some(Concerto {
            attack_bonus: u32::try_from(bonus).unwrap_or(u32::MAX),
            encore_hits: E6_HIT_COUNT * u8::from(self.eidolon >= 6),
        });
        battle.units[owner].summon_turn = Some(SummonTurn::with_action_advance(
            "concerto",
            CONCERTO_SPEED_MICROS,
            false,
        ));
        battle.close_turn_before_suspension(owner);
        for target in battle.allies() {
            if !battle.is_targetable(target) {
                continue;
            }
            if self.eidolon >= 4 {
                battle.cleanse_control(owner, target);
            }
            if target != owner {
                battle.shift_gauge(owner, target, 0, true);
            }
        }
    }
}

impl Mechanics for Robin {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.first_advance {
            battle.advance(owner, A2_ADVANCE_BP);
        }
    }

    fn suspends_turn(&self, battle: &Battle<'_>, owner: usize) -> bool {
        Self::concerto_active(battle, owner)
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, owner: usize) -> bool {
        Self::concerto_active(battle, owner)
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].robin_state.concerto = None;
        battle.units[owner].summon_turn = None;
        battle.shift_gauge(owner, owner, 0, true);
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if actor == owner && !extra {
            let turns = &mut battle.units[owner].robin_state.skill_turns;
            *turns = turns.saturating_sub(1);
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
            Action::ElationSkill => {}
            Action::Basic | Action::EnhancedUltimate => {}
            Action::Skill => battle.units[owner].robin_state.skill_turns = self.skill_turns,
            Action::Ultimate => self.start_concerto(battle, owner),
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        _origin: Action,
        primary_target: Option<usize>,
    ) {
        let energy = self.talent_energy_milli + E2_ENERGY_MILLI * u32::from(self.eidolon >= 2);
        battle.gain_energy(owner, owner, energy);
        if owner == attacker || !Self::concerto_active(battle, owner) {
            return;
        }
        let target = primary_target
            .filter(|&target| battle.is_targetable(target))
            .or_else(|| {
                battle
                    .attack_targets()
                    .iter()
                    .copied()
                    .find(|&target| battle.is_targetable(target))
            });
        let Some(target) = target else {
            return;
        };
        let Some(concerto) = &mut battle.units[owner].robin_state.concerto else {
            return;
        };
        let crit_damage =
            self.concerto_crit_damage_bp + E6_CRIT_DAMAGE_BP * u32::from(concerto.encore_hits > 0);
        concerto.encore_hits = concerto.encore_hits.saturating_sub(1);
        battle.fixed_critical_additional_hit(
            owner,
            target,
            Element::Physical,
            self.concerto_hit_bp,
            crit_damage,
            Action::Ultimate,
        );
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _target: usize,
        _element: Element,
    ) -> u32 {
        self.skill_damage_bp * u32::from(battle.units[owner].robin_state.skill_turns > 0)
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        battle.units[owner]
            .robin_state
            .concerto
            .map_or(0, |concerto| concerto.attack_bonus)
    }

    fn ally_crit_damage_bonus_bp(&self, _: &Battle<'_>, _: usize, _: usize) -> u32 {
        self.talent_crit_damage_bp
    }

    fn follow_up_crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _enemy: usize,
    ) -> u32 {
        A4_FOLLOW_UP_CRIT_DAMAGE_BP
            * u32::from(self.follow_up_crit_damage && Self::concerto_active(battle, owner))
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _attacker: usize,
    ) -> DamageTargetAura {
        DamageTargetAura {
            resistance_down_bp: E1_PENETRATION_BP
                * u32::from(self.eidolon >= 1 && Self::concerto_active(battle, owner)),
            ..DamageTargetAura::default()
        }
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        E2_SPEED_BP * u32::from(self.eidolon >= 2 && Self::concerto_active(battle, owner))
    }

    fn effect_resistance_aura_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        E4_EFFECT_RESISTANCE_BP
            * u32::from(self.eidolon >= 4 && Self::concerto_active(battle, owner))
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::ElationSkill => None,
            Action::Ultimate if battle.ordinary_actor == Some(owner) => Some(4),
            Action::Ultimate => Some(0),
            Action::Skill if battle.units[owner].robin_state.skill_turns == 0 => Some(2),
            Action::Basic | Action::Skill | Action::EnhancedUltimate => None,
        }
    }
}
