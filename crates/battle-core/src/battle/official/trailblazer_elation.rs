use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        elation::{AhaInstant, ElationAscension},
        resolve::ElationHit,
        support::AllyBuffKind,
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints, aeon_aha,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "trailblazer.elation";
const BASIC: &str = "skill.800901";
const SKILL: &str = "skill.800902";
const ULTIMATE: &str = "skill.800903";
const TALENT: &str = "skill.800904";
const ELATION_SKILL: &str = "skill.800920";
const CLOUD_NINE: &str = "trace.8009101";
const SCREW_IT: &str = "trace.8009102";
const SIC_EM: &str = "trace.8009103";
const THUNDER: Element = Element::Thunder;

#[derive(Clone, Default)]
pub(in crate::battle) struct ElationTrailblazerState {
    e1_stacks: u8,
    next_skill_bonus: bool,
    pending_elation: Option<DirectedElation>,
}

#[derive(Clone, Copy)]
struct DirectedElation {
    target: usize,
    boosted_factor_bp: Option<u16>,
}

struct ElationTrailblazer {
    eidolon: u8,
    cloud_nine: bool,
    screw_it: bool,
    sic_em: bool,
    skill_bp: u16,
    skill_banger: u32,
    ultimate_crit_bp: u32,
    ultimate_turns: u32,
    ultimate_advance_bp: u16,
    ultimate_banger: u32,
    ultimate_elation_punchline: u32,
    ultimate_punchline: u32,
    talent_energy_milli: u32,
    talent_punchline: u32,
    talent_hit_bp: u32,
    bounce_count: u8,
    bounce_bp: u32,
    finisher_bp: u32,
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field} combat value"))
}

fn ratio_bp(skill: &str, value: f64) -> Result<u16, BattleError> {
    let value = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(skill));
    }
    Ok(value as u16)
}

fn counter(skill: &str, value: f64) -> Result<u8, BattleError> {
    if !(1.0..=f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(skill));
    }
    Ok(value as u8)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != THUNDER
        || input.energy_capacity != 160
    {
        return Err(invalid("combat type or energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 3)?;
    let elation = input.parameters(ELATION_SKILL, 3)?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let skill_bp = ratio_bp(SKILL, skill[0])?;

    Ok(OfficialKit {
        max_energy_milli: 160_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            effects: vec![Effect::Hit {
                element: THUNDER,
                multiplier_bp: ratio_bp(BASIC, basic[0])?,
                toughness: 10,
            }],
            energy_milli: 20_000,
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllEnemies,
            effects: vec![Effect::Hit {
                element: THUNDER,
                multiplier_bp: skill_bp,
                toughness: 20,
            }],
            energy_milli: 30_000,
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::Ally,
            effects: Vec::new(),
            energy_milli: 5_000,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(ElationTrailblazer {
            eidolon: input.eidolon,
            cloud_nine: has_trace(CLOUD_NINE),
            screw_it: has_trace(SCREW_IT),
            sic_em: has_trace(SIC_EM),
            skill_bp,
            skill_banger: u32::from(counter(SKILL, skill[1])?),
            ultimate_crit_bp: u32::from(ratio_bp(ULTIMATE, ultimate[0])?),
            ultimate_turns: u32::from(counter(ULTIMATE, ultimate[1])?),
            ultimate_advance_bp: ratio_bp(ULTIMATE, ultimate[2])?,
            ultimate_banger: u32::from(counter(ULTIMATE, ultimate[3])?),
            ultimate_elation_punchline: u32::from(counter(ULTIMATE, ultimate[4])?),
            ultimate_punchline: u32::from(counter(ULTIMATE, ultimate[5])?),
            talent_energy_milli: u32::from(counter(TALENT, talent[0])?) * 1_000,
            talent_punchline: u32::from(counter(TALENT, talent[1])?),
            talent_hit_bp: u32::from(ratio_bp(TALENT, talent[2])?),
            bounce_count: counter(ELATION_SKILL, elation[0])?,
            bounce_bp: u32::from(ratio_bp(ELATION_SKILL, elation[1])?),
            finisher_bp: u32::from(ratio_bp(ELATION_SKILL, elation[2])?),
        }),
    })
}

fn best_banger(battle: &Battle<'_>) -> u32 {
    battle
        .allies()
        .filter(|&target| battle.is_targetable(target))
        .filter_map(|target| battle.certified_banger(target))
        .max()
        .unwrap_or(0)
}

fn is_elation_participant(battle: &Battle<'_>, target: usize) -> bool {
    battle
        .mechanics(target)
        .is_some_and(|rules| rules.elation_participant().is_some())
}

fn ascended_target(battle: &Battle<'_>, target: usize) -> Option<ElationAscension> {
    battle.mechanics(target)?.elation_ascension(battle, target)
}

fn ascension_bonus(ascension: ElationAscension) -> (u32, u16) {
    if ascension.innate_level >= 2 {
        (3_000, 40_000)
    } else {
        (2_000, 30_000)
    }
}

impl ElationTrailblazer {
    fn attack_talent(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.restore_energy_flat(actor, actor, self.talent_energy_milli);
        battle.add_punchline(actor, self.talent_punchline);
    }

    fn skill_extras(&self, battle: &mut Battle<'_>, actor: usize) {
        let has_banger = battle.certified_banger(actor).is_some();
        let state = &mut battle.units[actor].elation_trailblazer_state;
        let bonus = 2 * u32::from(std::mem::take(&mut state.next_skill_bonus));
        if self.eidolon >= 1 {
            state.e1_stacks = (state.e1_stacks + 1).min(3);
        }
        battle.grant_certified_banger(actor, self.skill_banger + bonus);
        if has_banger {
            let punchline = best_banger(battle);
            for enemy in battle.enemies() {
                if battle.is_targetable(enemy) {
                    Self::elation_hit(
                        battle,
                        actor,
                        enemy,
                        self.talent_hit_bp,
                        5,
                        Action::Skill,
                        AhaInstant {
                            punchline,
                            skill_damage_factor_bp: 10_000,
                        },
                    );
                }
            }
        }
        self.attack_talent(battle, actor);
    }

    fn ultimate(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.add_punchline(actor, self.ultimate_punchline);
        let bonus_banger = 2 * u32::from(std::mem::take(
            &mut battle.units[actor].elation_trailblazer_state.e1_stacks,
        ));
        if let Some(ascension) = battle.elation_ascension() {
            let (bonus_crit, _) = ascension_bonus(ascension);
            for ally in battle.party_units() {
                battle.apply_ally_buff(
                    actor,
                    ally,
                    AllyBuffKind::CritDamage,
                    self.ultimate_crit_bp + bonus_crit,
                    self.ultimate_turns,
                );
            }
        } else {
            battle.apply_ally_buff(
                actor,
                target,
                AllyBuffKind::CritDamage,
                self.ultimate_crit_bp,
                self.ultimate_turns,
            );
        }
        battle.cleanse_crowd_control(actor, target);
        if self.eidolon >= 2 {
            battle.apply_ally_buff(actor, target, AllyBuffKind::Elation, 1_200, 2);
        }
        if self.screw_it {
            battle.restore_skill_points(actor, 1);
        }
        if is_elation_participant(battle, target) {
            battle.grant_certified_banger(target, self.ultimate_banger + bonus_banger);
            battle.units[actor]
                .elation_trailblazer_state
                .pending_elation = Some(DirectedElation {
                target,
                boosted_factor_bp: ascended_target(battle, target)
                    .map(|ascension| ascension_bonus(ascension).1),
            });
        } else {
            battle.advance_ally(actor, target, self.ultimate_advance_bp);
        }
    }

    fn elation_hit(
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        multiplier_bp: u32,
        toughness: u16,
        origin: Action,
        instant: AhaInstant,
    ) {
        battle.elation_hit(
            actor,
            target,
            ElationHit {
                element: THUNDER,
                multiplier_bp,
                punchline: instant.punchline,
                toughness,
                origin,
                factor_bp: instant.skill_damage_factor_bp,
                assisted_by: None,
            },
        );
    }

    fn ready_wave(battle: &mut Battle<'_>) -> bool {
        battle.resolve_wave_or_end();
        battle.enemies().any(|enemy| battle.is_targetable(enemy))
    }
}

impl Mechanics for ElationTrailblazer {
    fn elation_participant(&self) -> Option<u16> {
        Some(120)
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Basic => self.attack_talent(battle, actor),
            Action::Skill => self.skill_extras(battle, actor),
            Action::Ultimate => {
                if let Some(target) = target {
                    self.ultimate(battle, actor, target);
                }
            }
            _ => {}
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _: Option<usize>,
    ) -> bool {
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                battle.attack_hit(actor, target, THUNDER, self.skill_bp, 20);
            }
        }
        self.skill_extras(battle, actor);
        battle.notify_attack(actor, Action::Skill, None);
        true
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        action: Action,
    ) {
        if self.sic_em && action == Action::ElationSkill {
            battle.units[owner]
                .elation_trailblazer_state
                .next_skill_bonus = true;
        }
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner != actor {
            return;
        }
        let Some(directed) = battle.units[actor]
            .elation_trailblazer_state
            .pending_elation
            .take()
        else {
            return;
        };
        if !Self::ready_wave(battle) || !battle.is_targetable(directed.target) {
            return;
        }
        let mut boosted_damage = 0;
        if let Some(factor_bp) = directed.boosted_factor_bp {
            boosted_damage = battle.resolve_elation_skill_tracking_damage(
                actor,
                AhaInstant {
                    punchline: best_banger(battle),
                    skill_damage_factor_bp: factor_bp,
                },
            );
            if !Self::ready_wave(battle) {
                return;
            }
        }
        battle.resolve_elation_skill(
            directed.target,
            AhaInstant {
                punchline: self.ultimate_elation_punchline,
                skill_damage_factor_bp: 10_000,
            },
        );
        if boosted_damage > 0 {
            aeon_aha::deal_elation_truth(battle, directed.target, boosted_damage / 5);
        }
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        if self.eidolon >= 4 {
            for target in battle.enemies() {
                if battle.is_targetable(target) {
                    battle.apply_vulnerability(actor, target, 1_000, 2);
                }
            }
        }
        if self.eidolon >= 6 {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::CritDamage,
                10_000,
                3,
                Some("trailblazer_elation.e6"),
            );
        }
        for _ in 0..self.bounce_count {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            Self::elation_hit(
                battle,
                actor,
                target,
                self.bounce_bp,
                0,
                Action::ElationSkill,
                instant,
            );
        }
        let enemies: Vec<_> = battle
            .enemies()
            .filter(|&target| battle.is_targetable(target))
            .collect();
        let count = enemies.len() as u32;
        for (index, target) in enemies.into_iter().enumerate() {
            let multiplier =
                self.finisher_bp / count + u32::from((index as u32) < self.finisher_bp % count);
            Self::elation_hit(
                battle,
                actor,
                target,
                multiplier,
                20,
                Action::ElationSkill,
                instant,
            );
        }
        battle.gain_energy(actor, actor, 5_000);
        self.attack_talent(battle, actor);
    }

    fn elation_from_attack_bp(&self, attack: u32) -> u32 {
        if self.cloud_nine {
            (attack.saturating_sub(1_000) / 200).min(6) * 1_000
        } else {
            0
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        1_500 * u32::from(self.screw_it)
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if action != Action::Ultimate {
            return None;
        }
        let preferred = battle
            .allies()
            .filter(|&ally| battle.is_targetable(ally))
            .max_by_key(|&ally| {
                (
                    ascended_target(battle, ally).is_some(),
                    is_elation_participant(battle, ally),
                    ally != actor,
                    battle.plan.units[ally].stats.attack,
                    Reverse(ally),
                )
            });
        Some(u8::from(target != preferred))
    }
}
