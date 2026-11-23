use std::sync::Arc;

use crate::{
    Action, BattleError, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        periodic::{DotEvent, DotScale, DotTiming, SkillDot},
        resolve::{DamageTargetAura, PeriodicHit},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1410";
const BASIC: &str = "skill.141001";
const SKILL: &str = "skill.141002";
const ULTIMATE: &str = "skill.141003";
const TALENT: &str = "skill.141004";
const GLADIUS: &str = "trace.1410101";
const BANQUET: &str = "trace.1410102";
const FIDDLE: &str = "trace.1410103";
const PHYSICAL: Element = Element::Physical;
const TALENT_ELEMENTS: [Element; 4] = [Element::Wind, PHYSICAL, Element::Fire, Element::Thunder];
const ENERGY_CAPACITY: u32 = 110;

#[derive(Clone, Default)]
pub(in crate::battle) struct HysilensState {
    zone_until: Option<u32>,
    echoes: Vec<ZoneEcho>,
}

#[derive(Clone, Copy)]
struct ZoneEcho {
    target: usize,
    count: u8,
    origin: Action,
}

struct Hysilens {
    eidolon: u8,
    gladius: bool,
    banquet: bool,
    fiddle: bool,
    vulnerability_bp: u32,
    skill_chance_bp: u32,
    zone_turns: u32,
    zone_attack_down_bp: u32,
    zone_defense_down_bp: u32,
    echo_limit: u8,
    echo_damage_bp: u16,
    talent_chance_bp: u32,
    talent_dot_bp: u16,
    bleed_hp_bp: u16,
    bleed_attack_cap_bp: u16,
    dot_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != PHYSICAL
    {
        return Err(invalid("path, element, energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 3)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 5)?;
    if skill[3] != 3.0 || ultimate[1] != 3.0 || ultimate[4] != 8.0 || talent[4] != 2.0 {
        return Err(invalid("effect duration or trigger count"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let eidolon = input.eidolon;
    let echo_bonus = if eidolon >= 6 { 2_000 } else { 0 };
    let rules = Arc::new(Hysilens {
        eidolon,
        gladius: trace(GLADIUS),
        banquet: trace(BANQUET),
        fiddle: trace(FIDDLE),
        vulnerability_bp: u32::from(points(SKILL, skill[2])?),
        skill_chance_bp: u32::from(points(SKILL, skill[1])?),
        zone_turns: ultimate[1] as u32,
        zone_attack_down_bp: u32::from(points(ULTIMATE, ultimate[5])?),
        zone_defense_down_bp: u32::from(points(ULTIMATE, ultimate[2])?),
        echo_limit: if eidolon >= 6 { 12 } else { ultimate[4] as u8 },
        echo_damage_bp: points(ULTIMATE, ultimate[3])?.saturating_add(echo_bonus),
        talent_chance_bp: u32::from(points(TALENT, talent[0])?),
        talent_dot_bp: points(TALENT, talent[1])?,
        bleed_hp_bp: points(TALENT, talent[2])?,
        bleed_attack_cap_bp: points(TALENT, talent[3])?,
        dot_turns: talent[4] as u32,
    });
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: hit_ability(
            Target::Enemy,
            20_000,
            points(BASIC, basic[0])?,
            10,
            SkillPoints::Generate,
        ),
        skill: hit_ability(
            Target::AllEnemies,
            30_000,
            points(SKILL, skill[0])?,
            10,
            SkillPoints::Spend,
        ),
        ultimate: hit_ability(
            Target::AllEnemies,
            5_000,
            points(ULTIMATE, ultimate[0])?,
            20,
            SkillPoints::Unchanged,
        ),
        mechanics: rules,
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {detail}"))
}

fn points(id: &str, value: f64) -> Result<u16, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit_ability(
    target: Target,
    energy_milli: u32,
    multiplier_bp: u16,
    toughness: u16,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: PHYSICAL,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn zone_active(battle: &Battle<'_>, owner: usize) -> bool {
    let state = &battle.units[owner];
    battle.is_targetable(owner)
        && state
            .hysilens_state
            .zone_until
            .is_some_and(|until| state.normal_turns < until)
}

impl Hysilens {
    fn open_zone(&self, battle: &mut Battle<'_>, owner: usize) {
        let unit = &mut battle.units[owner];
        unit.hysilens_state.zone_until = Some(unit.normal_turns + self.zone_turns);
        battle.emit(
            EventKind::StatusApplied(StatusKind::SirenZone),
            Some(owner),
            Some(owner),
        );
        if self.gladius {
            battle.restore_skill_points(owner, 1);
        }
    }

    fn close_zone(battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner].hysilens_state;
        state.echoes.clear();
        if state.zone_until.take().is_some() {
            battle.emit(
                EventKind::StatusExpired(StatusKind::SirenZone),
                Some(owner),
                Some(owner),
            );
        }
    }

    fn apply_talent_dot(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        origin: Action,
        separate: bool,
    ) -> bool {
        let mut missing = [PHYSICAL; TALENT_ELEMENTS.len()];
        let mut count = 0;
        for element in TALENT_ELEMENTS {
            if !battle.has_owned_skill_dot(owner, target, element, separate) {
                missing[count] = element;
                count += 1;
            }
        }
        let choices = if count > 0 {
            &missing[..count]
        } else {
            &TALENT_ELEMENTS
        };
        let element = choices[battle.random_index(choices.len())];
        let damage = if element == PHYSICAL {
            DotScale::TargetHp {
                hp_bp: self.bleed_hp_bp,
                attack_cap_bp: self.bleed_attack_cap_bp,
            }
        } else {
            DotScale::Attack(self.talent_dot_bp)
        };
        let dot = SkillDot {
            element,
            damage,
            origin,
            turns: self.dot_turns,
            max_stacks: 1,
        };
        if separate {
            battle.apply_separate_skill_dot(owner, target, 10_000, dot)
        } else {
            battle.apply_skill_dot(owner, target, self.talent_chance_bp, dot)
        }
    }
}

impl Mechanics for Hysilens {
    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.gladius {
            self.open_zone(battle, owner);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner == actor && !extra && !zone_active(battle, owner) {
            Self::close_zone(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        Self::close_zone(battle, owner);
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
                for target in battle.enemies() {
                    if battle.is_targetable(target)
                        && battle.roll_effect(owner, target, self.skill_chance_bp)
                    {
                        battle.apply_debuff(
                            owner,
                            target,
                            DebuffKind::Vulnerability,
                            self.vulnerability_bp,
                            3,
                        );
                    }
                }
            }
            Action::Ultimate => self.open_zone(battle, owner),
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate && self.banquet {
            for target in battle.enemies() {
                if battle.is_targetable(target) {
                    battle.detonate_all_dots(owner, target, 15_000, Action::Ultimate);
                }
            }
        }
    }

    fn on_ally_first_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: usize,
    ) {
        if !battle.is_targetable(owner) || !battle.is_party_unit(actor) {
            return;
        }
        // Natural damage is owned by this Talent, regardless of who applied the attack.
        let applied = self.apply_talent_dot(battle, owner, target, Action::Skill, false);
        if applied && self.eidolon >= 1 {
            self.apply_talent_dot(battle, owner, target, Action::Skill, true);
        }
    }

    fn on_enemy_dot_damage(&self, battle: &mut Battle<'_>, owner: usize, event: DotEvent) {
        if event.is_echo
            || event.timing == DotTiming::Other
            || !zone_active(battle, owner)
            || !battle.is_targetable(event.target)
        {
            return;
        }
        let echoes = &mut battle.units[owner].hysilens_state.echoes;
        if let Some(echo) = echoes.iter_mut().find(|echo| echo.target == event.target) {
            echo.count = (echo.count + 1).min(self.echo_limit);
        } else {
            echoes.push(ZoneEcho {
                target: event.target,
                count: 1,
                origin: match event.timing {
                    DotTiming::TurnStart(_) => Action::Ultimate,
                    _ => event.origin,
                },
            });
        }
        battle.pending_dot_batch = true;
    }

    fn on_dot_batch_finished(&self, battle: &mut Battle<'_>, owner: usize) {
        let echoes = std::mem::take(&mut battle.units[owner].hysilens_state.echoes);
        if !zone_active(battle, owner) {
            return;
        }
        for echo in echoes {
            for _ in 0..echo.count {
                if !battle.is_targetable(echo.target) {
                    break;
                }
                battle.periodic_hit(
                    owner,
                    echo.target,
                    PeriodicHit {
                        element: PHYSICAL,
                        damage: DotScale::Attack(self.echo_damage_bp),
                        stacks: 1,
                        ratio_bp: 10_000,
                        origin: echo.origin,
                    },
                );
            }
        }
    }

    fn damage_target_aura(&self, battle: &Battle<'_>, owner: usize, _: usize) -> DamageTargetAura {
        if !zone_active(battle, owner) {
            return DamageTargetAura::default();
        }
        DamageTargetAura {
            defense_down_bp: self.zone_defense_down_bp,
            resistance_down_bp: if self.eidolon >= 4 { 2_000 } else { 0 },
            ..DamageTargetAura::default()
        }
    }

    fn attack_reduction_bp(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> u32 {
        if zone_active(battle, owner) && battle.enemies().contains(&enemy) {
            self.zone_attack_down_bp
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
        _: Element,
    ) -> u32 {
        if !self.fiddle
            || !battle.is_targetable(owner)
            || !battle.is_party_unit(target)
            || (owner != target && (self.eidolon < 2 || !zone_active(battle, owner)))
        {
            return 0;
        }
        let excess = battle
            .effective_effect_hit_rate_bp(owner)
            .saturating_sub(6_000);
        (excess / 1_000 * 1_500).min(9_000)
    }

    fn ally_dot_damage_factor_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.eidolon >= 1 && battle.is_targetable(owner) {
            11_600
        } else {
            10_000
        }
    }
}
