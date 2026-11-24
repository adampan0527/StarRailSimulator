use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        elation::{AhaInstant, ElationSkillSource},
        resolve::{ElationHit, PendingAttack},
        support::AllyBuffKind,
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1513";
const BASIC: &str = "skill.151301";
const SKILL: &str = "skill.151302";
const ULTIMATE: &str = "skill.151303";
const TALENT: &str = "skill.151304";
const ELATION: &str = "skill.151320";
const ENHANCED_ELATION: &str = "skill.151321";
const QUANTUM: Element = Element::Quantum;

#[derive(Clone, Default)]
pub(in crate::battle) struct AventurineSurfState {
    heat: u8,
    queued_thresholds: u8,
    enhanced_next: bool,
    elation_casts: u8,
    trace_activations: u8,
    aha_speed_micros: u64,
}

struct HeatGain {
    punchline: u8,
    heat: u8,
}

struct ElationAttack {
    area_bp: u32,
    area_toughness: u16,
    bounce_bp: u32,
    bounces: u8,
}

struct AventurineSurf {
    eidolon: u8,
    speed_trace: bool,
    waves_trace: bool,
    crit_trace: bool,
    skill_gain: HeatGain,
    ultimate_gain: HeatGain,
    ultimate_speed_bp: u32,
    ultimate_speed_turns: u32,
    heat_threshold: u8,
    heat_cap: u8,
    ability_talent_bp: [u32; 2],
    triggered_punchline: u32,
    ally_attack_gain: HeatGain,
    elation: ElationAttack,
    enhanced_elation: ElationAttack,
    heat_bounce_bp: u32,
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id} combat value"))
}

fn ratio_bp(id: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn counter(id: &str, value: f64) -> Result<u8, BattleError> {
    if !(1.0..=f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u8)
}

fn ability(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        effects: vec![Effect::Hit {
            element: QUANTUM,
            multiplier_bp,
            toughness,
        }],
        energy_milli,
        skill_points,
    }
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != QUANTUM
        || input.energy_capacity != 130
    {
        return Err(invalid("combat type or energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 7)?;
    let elation = input.parameters(ELATION, 3)?;
    let enhanced = input.parameters(ENHANCED_ELATION, 4)?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: ability(
            Target::Enemy,
            ratio_bp(BASIC, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: ability(
            Target::AllEnemies,
            ratio_bp(SKILL, skill[0])?,
            10,
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: ability(
            Target::AllEnemies,
            ratio_bp(ULTIMATE, ultimate[0])?,
            20,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(AventurineSurf {
            eidolon: input.eidolon,
            speed_trace: has_trace("trace.1513101"),
            waves_trace: has_trace("trace.1513102"),
            crit_trace: has_trace("trace.1513103"),
            skill_gain: HeatGain {
                punchline: counter(SKILL, skill[1])?,
                heat: counter(SKILL, skill[2])?,
            },
            ultimate_gain: HeatGain {
                punchline: counter(ULTIMATE, ultimate[2])?,
                heat: counter(ULTIMATE, ultimate[1])?,
            },
            ultimate_speed_bp: u32::from(ratio_bp(ULTIMATE, ultimate[3])?),
            ultimate_speed_turns: u32::from(counter(ULTIMATE, ultimate[4])?),
            heat_threshold: counter(TALENT, talent[0])?,
            heat_cap: if input.eidolon >= 2 {
                50
            } else {
                counter(TALENT, talent[3])?
            },
            ability_talent_bp: [
                u32::from(ratio_bp(TALENT, talent[1])?),
                u32::from(ratio_bp(TALENT, talent[2])?),
            ],
            triggered_punchline: u32::from(counter(TALENT, talent[4])?),
            ally_attack_gain: HeatGain {
                punchline: counter(TALENT, talent[5])?,
                heat: counter(TALENT, talent[6])?,
            },
            elation: ElationAttack {
                area_bp: u32::from(ratio_bp(ELATION, elation[0])?),
                area_toughness: 10,
                bounce_bp: u32::from(ratio_bp(ELATION, elation[2])?),
                bounces: counter(ELATION, elation[1])?,
            },
            enhanced_elation: ElationAttack {
                area_bp: u32::from(ratio_bp(ENHANCED_ELATION, enhanced[0])?),
                area_toughness: 20,
                bounce_bp: u32::from(ratio_bp(ENHANCED_ELATION, enhanced[3])?),
                bounces: counter(ENHANCED_ELATION, enhanced[2])?,
            },
            heat_bounce_bp: u32::from(ratio_bp(ENHANCED_ELATION, enhanced[1])?),
        }),
    })
}

fn set_heat(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let state = &mut battle.units[actor].aventurine_surf_state;
    let before = std::mem::replace(&mut state.heat, after);
    if before != after {
        battle.emit(
            EventKind::AventurineSurfHeat { before, after },
            Some(actor),
            None,
        );
    }
}

impl AventurineSurf {
    fn ascended_pair_level(&self, battle: &Battle<'_>) -> Option<u8> {
        if self.waves_trace && battle.elation_team_size() == 2 {
            battle
                .elation_ascension()
                .map(|ascension| ascension.innate_level)
        } else {
            None
        }
    }

    fn solo_effects(&self, battle: &Battle<'_>) -> bool {
        self.waves_trace
            && (battle.elation_team_size() == 1 || self.ascended_pair_level(battle).is_some())
    }

    fn gain_heat(&self, battle: &mut Battle<'_>, actor: usize, heat: u8) {
        let before = battle.units[actor].aventurine_surf_state.heat;
        set_heat(
            battle,
            actor,
            before.saturating_add(heat).min(self.heat_cap),
        );
        self.queue_heat_triggers(battle, actor);
    }

    fn queue_heat_triggers(&self, battle: &mut Battle<'_>, actor: usize) {
        if !battle.is_targetable(actor)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return;
        }
        let state = &mut battle.units[actor].aventurine_surf_state;
        let reached = state.heat / self.heat_threshold;
        let limit = if self.eidolon >= 1 {
            reached
        } else {
            reached.min(1)
        };
        for _ in state.queued_thresholds..limit {
            battle
                .pending_follow_ups
                .push_back(PendingAttack::TriggeredElationSkill {
                    actor,
                    punchline: self.triggered_punchline,
                });
        }
        state.queued_thresholds = state.queued_thresholds.max(limit);
    }

    fn elation_hit(
        battle: &mut Battle<'_>,
        actor: usize,
        enemy: usize,
        multiplier_bp: u32,
        toughness: u16,
        origin: Action,
        instant: AhaInstant,
    ) {
        battle.elation_hit(
            actor,
            enemy,
            ElationHit {
                element: QUANTUM,
                multiplier_bp,
                punchline: instant.punchline,
                toughness,
                origin,
                factor_bp: instant.skill_damage_factor_bp,
                assisted_by: None,
            },
        );
    }

    fn triggered_trace_heat(&self, battle: &mut Battle<'_>, actor: usize, origin: Action) -> u8 {
        if !self.crit_trace
            || (!battle.in_follow_up() && origin == Action::ElationSkill)
            || battle.units[actor].aventurine_surf_state.trace_activations >= 6
        {
            return 0;
        }
        battle.units[actor].aventurine_surf_state.trace_activations += 1;
        for target in battle.party_units() {
            battle.apply_ally_buff(actor, target, AllyBuffKind::CritDamage, 4_800, 3);
        }
        2
    }
}

impl Mechanics for AventurineSurf {
    fn elation_participant(&self) -> Option<u16> {
        Some(156)
    }

    fn certified_banger_duration(&self) -> u32 {
        3
    }

    fn aha_flat_speed_micros(&self, battle: &Battle<'_>, actor: usize) -> u64 {
        battle.units[actor].aventurine_surf_state.aha_speed_micros
    }

    fn on_aha_instant_finished(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].aventurine_surf_state.aha_speed_micros = 0;
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, actor: usize) {
        self.queue_heat_triggers(battle, actor);
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            battle.units[actor].aventurine_surf_state.trace_activations = 0;
            if self.eidolon >= 4 {
                for target in battle.party_units() {
                    let summon = battle.memosprite_owner(target).is_some();
                    battle.apply_ally_buff(
                        actor,
                        target,
                        AllyBuffKind::DefenseIgnore { summon },
                        1_800,
                        3,
                    );
                }
            }
        } else if action == Action::Ultimate {
            battle.apply_speed_up(
                actor,
                actor,
                self.ultimate_speed_bp,
                self.ultimate_speed_turns,
            );
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        let (gain, multiplier) = match action {
            Action::Skill => (&self.skill_gain, self.ability_talent_bp[0]),
            Action::Ultimate => (&self.ultimate_gain, self.ability_talent_bp[1]),
            _ => return,
        };
        if let Some(punchline) = battle.certified_banger(actor) {
            for enemy in battle.enemies() {
                if battle.is_targetable(enemy) {
                    Self::elation_hit(
                        battle,
                        actor,
                        enemy,
                        multiplier,
                        0,
                        action,
                        AhaInstant {
                            punchline,
                            skill_damage_factor_bp: 10_000,
                        },
                    );
                }
            }
        }
        battle.add_punchline(actor, u32::from(gain.punchline));
        self.gain_heat(battle, actor, gain.heat);
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        source: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        if source == actor || !battle.is_targetable(actor) || !battle.is_party_unit(source) {
            return;
        }
        let solo = self.solo_effects(battle);
        let boost = self.ascended_pair_level(battle).unwrap_or(0);
        if solo {
            let state = &mut battle.units[actor].aventurine_surf_state;
            state.aha_speed_micros = state.aha_speed_micros.saturating_add(25_000_000);
            battle.grant_certified_banger(actor, 2);
        }
        battle.add_punchline(
            actor,
            u32::from(self.ally_attack_gain.punchline) + u32::from(solo) + u32::from(boost),
        );
        let trace_heat = self.triggered_trace_heat(battle, actor, origin);
        self.gain_heat(
            battle,
            actor,
            self.ally_attack_gain
                .heat
                .saturating_add(trace_heat)
                .saturating_add(boost),
        );
    }

    fn on_elation_skill_from(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        instant: AhaInstant,
        source: ElationSkillSource,
    ) {
        let state = &battle.units[actor].aventurine_surf_state;
        let in_instant = source == ElationSkillSource::AhaInstant;
        let permanent = self.eidolon >= 6 && state.elation_casts >= 2;
        let enhanced = permanent || (in_instant && state.enhanced_next);
        let bonus_bounces = if enhanced { state.heat } else { 0 };
        let spent_heat = if enhanced && in_instant {
            state.heat
        } else {
            0
        };
        if enhanced && in_instant {
            let state = &mut battle.units[actor].aventurine_surf_state;
            state.enhanced_next = false;
            state.queued_thresholds = 0;
            set_heat(battle, actor, 0);
        }
        battle.emit(
            EventKind::AventurineSurfElation {
                enhanced,
                heat: spent_heat,
                punchline: instant.punchline,
            },
            Some(actor),
            None,
        );
        let attack = if enhanced {
            &self.enhanced_elation
        } else {
            &self.elation
        };
        for enemy in battle.enemies() {
            if battle.is_targetable(enemy) {
                Self::elation_hit(
                    battle,
                    actor,
                    enemy,
                    attack.area_bp,
                    attack.area_toughness,
                    Action::ElationSkill,
                    instant,
                );
            }
        }
        for index in 0..attack.bounces.saturating_add(bonus_bounces) {
            let Some(enemy) = battle.random_living_enemy() else {
                break;
            };
            let is_bonus = index >= attack.bounces;
            let multiplier = if is_bonus {
                self.heat_bounce_bp
            } else {
                attack.bounce_bp
            };
            let toughness = 5 * u16::from(is_bonus && index == attack.bounces);
            Self::elation_hit(
                battle,
                actor,
                enemy,
                multiplier,
                toughness,
                Action::ElationSkill,
                instant,
            );
        }
        battle.gain_energy(actor, actor, 5_000);
        let state = &mut battle.units[actor].aventurine_surf_state;
        state.elation_casts = state.elation_casts.saturating_add(1).min(2);
        if source == ElationSkillSource::Triggered {
            state.enhanced_next = true;
        }
        if self.eidolon >= 2 {
            self.gain_heat(battle, actor, 4);
        }
    }

    fn primary_attack_counts_as_follow_up(
        &self,
        battle: &Battle<'_>,
        _: usize,
        action: Action,
    ) -> bool {
        action == Action::ElationSkill && self.solo_effects(battle)
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let speed = battle.speed_micros(actor) / 1_000_000;
        if self.speed_trace && speed >= 140 {
            3_000 + (speed - 140).min(200) as u32 * 100
        } else {
            0
        }
    }

    fn ally_elation_bp(&self, battle: &Battle<'_>, actor: usize, target: usize) -> u32 {
        if self.waves_trace && battle.elation_team_size() > 1 {
            2_000 + 8_000 * u32::from(actor == target)
        } else {
            0
        }
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, _: usize, target: usize) -> u32 {
        if !battle.is_party_unit(target) {
            return 0;
        }
        match self.ascended_pair_level(battle) {
            Some(1) => 1_500,
            Some(_) => 2_500,
            None => 0,
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        4_800 * u32::from(self.crit_trace)
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, _: Element) -> u32 {
        2_400 * u32::from(self.eidolon >= 1)
    }

    fn ally_merrymake_bp(&self, _: &Battle<'_>, actor: usize, target: usize) -> u32 {
        2_500 * u32::from(self.eidolon >= 6 && actor == target)
    }
}
