use std::sync::Arc;

use crate::{
    Action, BattleError, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        elation::AhaInstant,
        resolve::ElationHit,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1502";
const BASIC: &str = "skill.150201";
const SKILL: &str = "skill.150202";
const ULTIMATE: &str = "skill.150203";
const TALENT: &str = "skill.150204";
const ELATION_SKILL: &str = "skill.150220";
const AMAZE_IN_GRACE: &str = "trace.1502101";
const POISED_AND_SATED: &str = "trace.1502102";
const FELICITY_ENSEMBLE: &str = "trace.1502103";
const PHYSICAL: Element = Element::Physical;
const SPEED_MICROS_PER_POINT: u64 = 1_000_000;

struct YaoGuang {
    eidolon: u8,
    amaze_in_grace: bool,
    poised_and_sated: bool,
    felicity_ensemble: bool,
    zone_turns: u32,
    zone_elation_ratio_bp: u32,
    zone_punchline: u32,
    ult_punchline: u32,
    penetration_bp: u32,
    penetration_turns: u32,
    extra_punchline: u32,
    boon_multiplier_bp: u32,
    skill_multiplier_bp: u32,
    whisper_vulnerability_bp: u32,
    whisper_turns: u32,
    single_hits: u32,
    single_multiplier_bp: u32,
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id} combat value"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != PHYSICAL
        || input.energy_capacity != 180
    {
        return Err(invalid("combat type or energy capacity"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 1)?;
    let elation = input.parameters(ELATION_SKILL, 6)?;
    if skill[0] != 3.0 || skill[2] != 3.0 {
        return Err(invalid(SKILL));
    }
    if ultimate[0] != 5.0 || ultimate[2..] != [3.0, 20.0, 1.0] {
        return Err(invalid(ULTIMATE));
    }
    if elation[0] != 1.0 || elation[3] != 3.0 || elation[4] != 5.0 {
        return Err(invalid(ELATION_SKILL));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 180_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            effects: vec![
                Effect::Hit {
                    element: PHYSICAL,
                    multiplier_bp: ratio_bp(BASIC, basic[0])?,
                    toughness: 10,
                },
                Effect::BlastAdjacent {
                    element: PHYSICAL,
                    multiplier_bp: ratio_bp(BASIC, basic[1])?,
                    toughness: 5,
                },
            ],
            energy_milli: 30_000,
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            effects: Vec::new(),
            energy_milli: 30_000,
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            effects: Vec::new(),
            energy_milli: 5_000,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(YaoGuang {
            eidolon: input.eidolon,
            amaze_in_grace: has_trace(AMAZE_IN_GRACE),
            poised_and_sated: has_trace(POISED_AND_SATED),
            felicity_ensemble: has_trace(FELICITY_ENSEMBLE),
            zone_turns: skill[0] as u32,
            zone_elation_ratio_bp: u32::from(ratio_bp(SKILL, skill[1])?),
            zone_punchline: skill[2] as u32,
            ult_punchline: ultimate[0] as u32,
            penetration_bp: u32::from(ratio_bp(ULTIMATE, ultimate[1])?),
            penetration_turns: ultimate[2] as u32,
            extra_punchline: ultimate[3] as u32,
            boon_multiplier_bp: u32::from(ratio_bp(TALENT, talent[0])?),
            skill_multiplier_bp: u32::from(ratio_bp(ELATION_SKILL, elation[1])?),
            whisper_vulnerability_bp: u32::from(ratio_bp(ELATION_SKILL, elation[2])?),
            whisper_turns: elation[3] as u32,
            single_hits: elation[4] as u32,
            single_multiplier_bp: u32::from(ratio_bp(ELATION_SKILL, elation[5])?),
        }),
    })
}

fn zone_active(battle: &Battle<'_>, owner: usize) -> bool {
    let state = &battle.units[owner];
    battle.is_targetable(owner)
        && state.statuses[StatusKind::WealLots.index()]
            .is_some_and(|status| state.normal_turns < status.expires_after_turn)
}

fn choose_hit_target(battle: &mut Battle<'_>) -> Option<usize> {
    let targets: Vec<_> = battle
        .attack_targets()
        .iter()
        .copied()
        .filter(|&target| battle.is_targetable(target))
        .collect();
    let index = battle.random_index(targets.len());
    targets.get(index).copied()
}

impl YaoGuang {
    fn skill_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        multiplier_bp: u32,
        toughness: u16,
        instant: AhaInstant,
    ) {
        battle.elation_hit(
            actor,
            target,
            ElationHit {
                element: PHYSICAL,
                multiplier_bp: multiplier_bp * (1 + u32::from(self.eidolon == 6)),
                punchline: instant.punchline,
                toughness,
                origin: Action::ElationSkill,
                factor_bp: instant.skill_damage_factor_bp,
                assisted_by: None,
            },
        );
    }
}

impl Mechanics for YaoGuang {
    fn elation_participant(&self) -> Option<u16> {
        Some(116)
    }

    fn certified_banger_duration(&self) -> u32 {
        2 + u32::from(self.felicity_ensemble)
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Basic if zone_active(battle, actor) => {
                battle.add_punchline(actor, self.zone_punchline);
            }
            Action::Skill => {
                battle.apply_status(actor, actor, StatusKind::WealLots, 1, self.zone_turns);
                battle.add_punchline(actor, self.zone_punchline);
            }
            Action::Ultimate => {
                battle.add_punchline(actor, self.ult_punchline);
                for target in battle.party_units() {
                    battle.apply_ally_buff(
                        actor,
                        target,
                        AllyBuffKind::ResistancePenetration,
                        self.penetration_bp,
                        self.penetration_turns,
                    );
                }
                let fixed = if self.eidolon >= 1 {
                    40
                } else {
                    self.extra_punchline
                };
                battle.queue_aha_extra_turn(actor, fixed);
            }
            _ => {}
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner == actor
            && !extra
            && !zone_active(battle, owner)
            && battle.units[owner].statuses[StatusKind::WealLots.index()]
                .take()
                .is_some()
        {
            battle.emit(
                EventKind::StatusExpired(StatusKind::WealLots),
                Some(owner),
                Some(owner),
            );
        }
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy) {
                battle.apply_debuff(
                    actor,
                    enemy,
                    DebuffKind::WoesWhisper,
                    self.whisper_vulnerability_bp,
                    self.whisper_turns,
                );
            }
        }
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy) {
                self.skill_hit(battle, actor, enemy, self.skill_multiplier_bp, 20, instant);
            }
        }
        if let Some(target) = battle.random_living_enemy() {
            for _ in 0..self.single_hits {
                if !battle.is_targetable(target) {
                    break;
                }
                self.skill_hit(battle, actor, target, self.single_multiplier_bp, 1, instant);
            }
        }
        battle.gain_energy(actor, actor, 5_000);
        if self.poised_and_sated {
            battle.restore_skill_points(actor, 1);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        let Some(punchline) = battle.certified_banger(owner) else {
            return;
        };
        let Some(combat_type) = battle.plan.units[actor].combat_type else {
            return;
        };
        if !battle.is_targetable(owner) || !battle.is_party_unit(actor) {
            return;
        }
        let instances = 1 + usize::from(battle.attack_spent_skill_points);
        for _ in 0..instances {
            let Some(target) = choose_hit_target(battle) else {
                break;
            };
            battle.elation_hit(
                actor,
                target,
                ElationHit {
                    element: combat_type.element,
                    multiplier_bp: self.boon_multiplier_bp,
                    punchline,
                    toughness: 0,
                    origin,
                    factor_bp: 10_000,
                    assisted_by: Some(owner),
                },
            );
        }
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if !self.amaze_in_grace {
            return 0;
        }
        let speed = battle.speed_micros(actor) / SPEED_MICROS_PER_POINT;
        if speed < 120 {
            return 0;
        }
        3_000 + ((speed - 120).min(200) as u32) * 100
    }

    fn ally_elation_bp(&self, battle: &Battle<'_>, owner: usize, _actor: usize) -> u32 {
        if !zone_active(battle, owner) {
            return 0;
        }
        percentage_of(
            battle.intrinsic_elation_bp(owner),
            self.zone_elation_ratio_bp,
        ) + 1_600 * u32::from(self.eidolon >= 2)
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _actor: usize) -> u32 {
        1_200 * u32::from(self.eidolon >= 2 && zone_active(battle, owner))
    }

    fn ally_crit_damage_bonus_bp(&self, _: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        6_000 * u32::from(owner == actor && self.poised_and_sated)
    }

    fn ally_elation_defense_ignore_bp(&self, _: &Battle<'_>, _: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 1)
    }

    fn aha_extra_skill_damage_factor_bp(&self) -> u16 {
        if self.eidolon >= 4 { 15_000 } else { 10_000 }
    }

    fn ally_merrymake_bp(&self, _: &Battle<'_>, _: usize, _: usize) -> u32 {
        2_500 * u32::from(self.eidolon == 6)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill if !zone_active(battle, owner) => Some(2),
            _ => None,
        }
    }
}
