use std::sync::Arc;

use crate::{
    Action, BattleError, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        periodic::{DotEvent, DotScale, DotTiming, SpecialDotTrigger},
        resolve::PeriodicHit,
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1307";
const WIND: Element = Element::Wind;
const DOT_ELEMENTS: [Element; 4] = [WIND, Element::Physical, Element::Fire, Element::Thunder];
const TRACE_CHANCE_BP: u32 = 6_500;
const DETONATION_STACK_LIMIT: u8 = 3;
const E1_RESISTANCE_DOWN_BP: u32 = 2_500;
const E2_CHANCE_BP: u32 = 10_000;
const E2_BASE_STACKS: u32 = 6;
const E2_ENHANCED_STACKS: u32 = 30;
const E4_BASE_EFFECT_RESISTANCE_DOWN_BP: u16 = 1_000;
const E4_ENHANCED_VULNERABILITY_BP: u32 = 2_000;
const E4_ENERGY_MILLI: u32 = 8_000;
const E6_BASE_EXTRA_CHANCE_BP: u32 = 5_000;
const E6_ENHANCED_LIMIT_INCREASE: u32 = 30;
const ENHANCED_A2_STACKS: u32 = 5;

#[derive(Clone, Copy)]
struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    viscera: &'static str,
    goblet: &'static str,
    candle: &'static str,
}

const BASE: KitIds = KitIds {
    basic: "skill.130701",
    skill: "skill.130702",
    ultimate: "skill.130703",
    talent: "skill.130704",
    viscera: "trace.1307101",
    goblet: "trace.1307102",
    candle: "trace.1307103",
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1130701",
    skill: "skill.1130702",
    ultimate: "skill.1130703",
    talent: "skill.1130704",
    viscera: "trace.11307101",
    goblet: "trace.11307102",
    candle: "trace.11307103",
};

#[derive(Clone, Copy)]
pub(in crate::battle) struct Arcana {
    pub(in crate::battle) source: usize,
    pub(in crate::battle) stacks: u32,
    origin: Action,
    all_elements: bool,
}

impl Arcana {
    pub(in crate::battle) fn matches_element(
        self,
        battle: &Battle<'_>,
        target: usize,
        element: Element,
    ) -> bool {
        element == WIND
            || (DOT_ELEMENTS.contains(&element)
                && (self.all_elements || battle.has_epiphany_from(self.source, target)))
    }
}

#[derive(Clone, Default)]
pub(in crate::battle) struct BlackSwanTarget {
    pub(in crate::battle) arcana: Option<Arcana>,
    preserve_charges: u8,
    epiphany_energy_used: bool,
    detonations: DetonationStacks,
}

#[derive(Clone, Default)]
struct DetonationStacks {
    attack_index: u32,
    awarded: u8,
}

#[derive(Clone, Copy)]
enum EpiphanyRetention {
    Once(u8),
    Throughout { extra_stack_chance_bp: u32 },
}

struct BlackSwan {
    enhanced: bool,
    eidolon: u8,
    viscera: bool,
    goblet: bool,
    candle: bool,
    basic_arcana_chances: Option<(u32, u32)>,
    skill_arcana_chance_bp: Option<u32>,
    defense_down_chance_bp: u32,
    defense_down_bp: u32,
    defense_down_turns: u32,
    epiphany_turns: u32,
    epiphany_vulnerability_bp: u32,
    epiphany: EpiphanyRetention,
    talent_chance_bp: u32,
    arcana_base_bp: u32,
    arcana_per_stack_bp: u32,
    arcana_overflow_stack_bp: u32,
    arcana_limit: u32,
    splash_threshold: u32,
    splash_bp: u32,
    ignore_threshold: u32,
    defense_ignore_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let has_variant = |ids: KitIds| {
        [ids.basic, ids.skill, ids.ultimate, ids.talent]
            .into_iter()
            .any(|id| input.skill_parameters.contains_key(id))
    };
    let (enhanced, ids) = match (has_variant(BASE), has_variant(NOVAFLARE)) {
        (true, false) => (false, BASE),
        (false, true) => (true, NOVAFLARE),
        _ => return Err(invalid("skill variant")),
    };
    let basic = input.parameters(ids.basic, if enhanced { 1 } else { 3 })?;
    let skill = input.parameters(ids.skill, if enhanced { 4 } else { 5 })?;
    let ultimate = input.parameters(ids.ultimate, 4)?;
    let talent = input.parameters(ids.talent, 8)?;
    let ratio = |id, value| basis_points(id, value).map(u32::from);
    let traced = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let count = |id, value, expected| fixed_count(id, value, expected).map(u32::from);
    let (skill_arcana_chance_bp, defense_turns) = if enhanced {
        (None, skill[1])
    } else {
        (Some(ratio(ids.skill, skill[1])?), skill[4])
    };
    let (basic_arcana_chances, epiphany, splash_threshold, ignore_threshold, overflow_bp) =
        if enhanced {
            (
                None,
                EpiphanyRetention::Throughout {
                    extra_stack_chance_bp: ratio(ids.ultimate, ultimate[3])?,
                },
                1,
                count(ids.talent, talent[5], 0)?,
                ratio(ids.talent, talent[3])?,
            )
        } else {
            (
                Some((ratio(ids.basic, basic[1])?, ratio(ids.basic, basic[2])?)),
                EpiphanyRetention::Once(fixed_count(ids.ultimate, ultimate[3], 1)?),
                count(ids.talent, talent[3], 3)?,
                count(ids.talent, talent[5], 7)?,
                0,
            )
        };
    let skill_multiplier = basis_points(ids.skill, skill[0])?;
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(ids.basic, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                hit(skill_multiplier, 20),
                Effect::BlastAdjacent {
                    element: WIND,
                    multiplier_bp: skill_multiplier,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ids.ultimate, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(BlackSwan {
            enhanced,
            eidolon: input.eidolon,
            viscera: traced(ids.viscera),
            goblet: traced(ids.goblet),
            candle: traced(ids.candle),
            basic_arcana_chances,
            skill_arcana_chance_bp,
            defense_down_chance_bp: ratio(ids.skill, skill[2])?,
            defense_down_bp: ratio(ids.skill, skill[3])?,
            defense_down_turns: count(ids.skill, defense_turns, 3)?,
            epiphany_turns: count(ids.ultimate, ultimate[1], 2)?,
            epiphany_vulnerability_bp: ratio(ids.ultimate, ultimate[2])?,
            epiphany,
            talent_chance_bp: ratio(ids.talent, talent[1])?,
            arcana_base_bp: ratio(ids.talent, talent[0])?,
            arcana_per_stack_bp: ratio(ids.talent, talent[2])?,
            arcana_overflow_stack_bp: overflow_bp,
            arcana_limit: count(ids.talent, talent[7], 50)?
                + if enhanced && input.eidolon >= 6 {
                    E6_ENHANCED_LIMIT_INCREASE
                } else {
                    0
                },
            splash_threshold,
            splash_bp: ratio(ids.talent, talent[4])?,
            ignore_threshold,
            defense_ignore_bp: basis_points(ids.talent, talent[6])?,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(skill));
    }
    Ok(rounded as u16)
}

fn fixed_count(skill: &str, value: f64, expected: u8) -> Result<u8, BattleError> {
    if value != f64::from(expected) {
        return Err(invalid(skill));
    }
    Ok(expected)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: WIND,
        multiplier_bp,
        toughness,
    }
}

impl BlackSwan {
    fn defense_down(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if battle.hp(target).0 > 0 && battle.roll_effect(owner, target, self.defense_down_chance_bp)
        {
            battle.apply_defense_down(owner, target, self.defense_down_bp, self.defense_down_turns);
        }
    }

    fn apply_epiphany(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let extra_vulnerability = if self.enhanced && self.eidolon >= 4 {
            E4_ENHANCED_VULNERABILITY_BP
        } else {
            0
        };
        let effect_resistance_down_bp = if !self.enhanced && self.eidolon >= 4 {
            E4_BASE_EFFECT_RESISTANCE_DOWN_BP
        } else {
            0
        };
        if battle.apply_debuff(
            owner,
            target,
            DebuffKind::Epiphany {
                only_on_turn: !self.enhanced,
                effect_resistance_down_bp,
            },
            self.epiphany_vulnerability_bp + extra_vulnerability,
            self.epiphany_turns,
        ) {
            let state = &mut battle.units[target].black_swan;
            state.epiphany_energy_used = false;
            state.preserve_charges = match self.epiphany {
                EpiphanyRetention::Once(charges) => charges,
                EpiphanyRetention::Throughout { .. } => 0,
            };
        }
    }

    fn epiphany_energy(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        if self.eidolon < 4 || !battle.has_epiphany_from(owner, target) {
            return;
        }
        let state = &mut battle.units[target].black_swan;
        if self.enhanced || !state.epiphany_energy_used {
            state.epiphany_energy_used = true;
            battle.gain_energy(owner, owner, E4_ENERGY_MILLI);
        }
    }

    fn add_arcana(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        origin: Action,
        chance_bp: u32,
        stacks: u32,
    ) -> bool {
        if battle.hp(target).0 == 0
            || !battle.roll_effect(owner, target, chance_bp)
            || battle.resists_debuff(target)
        {
            return false;
        }
        let e6_extra = match (self.eidolon >= 6, self.enhanced) {
            (false, _) => 0,
            (true, false) => u32::from(battle.roll_fixed(E6_BASE_EXTRA_CHANCE_BP)),
            (true, true) => stacks,
        };
        let epiphany_extra = match self.epiphany {
            EpiphanyRetention::Throughout {
                extra_stack_chance_bp,
            } if battle.has_epiphany_from(owner, target) => (0..stacks)
                .filter(|_| battle.roll_fixed(extra_stack_chance_bp))
                .count() as u32,
            _ => 0,
        };
        let state = &mut battle.units[target].black_swan.arcana;
        let previous = state.map_or(0, |arcana| arcana.stacks);
        let updated = previous.saturating_add(stacks + e6_extra + epiphany_extra);
        let updated = if self.enhanced {
            updated
        } else {
            updated.min(self.arcana_limit)
        };
        if updated != previous {
            *state = Some(Arcana {
                source: owner,
                stacks: updated,
                origin: state.map_or(origin, |arcana| arcana.origin),
                all_elements: self.enhanced,
            });
            battle.record_enemy_debuff(target);
            battle.emit(
                EventKind::StatusApplied(StatusKind::Arcana),
                Some(owner),
                Some(target),
            );
        }
        updated != previous
    }

    fn matching_dot_count(&self, battle: &Battle<'_>, target: usize) -> usize {
        DOT_ELEMENTS
            .into_iter()
            .filter(|&element| battle.has_elemental_dot(target, element))
            .count()
    }

    fn arcana_multiplier_bp(&self, stacks: u32) -> u32 {
        let within_limit = stacks.min(self.arcana_limit);
        self.arcana_base_bp
            .saturating_add(self.arcana_per_stack_bp * (within_limit - 1))
            .saturating_add(
                self.arcana_overflow_stack_bp
                    .saturating_mul(stacks.saturating_sub(within_limit)),
            )
    }

    fn arcana_hit(
        &self,
        battle: &mut Battle<'_>,
        arcana: Arcana,
        target: usize,
        trigger: SpecialDotTrigger,
    ) {
        let (natural, ratio_bp, origin) = match trigger {
            SpecialDotTrigger::TurnStart => (true, 10_000, arcana.origin),
            SpecialDotTrigger::Detonation {
                matching_element,
                ratio_bp,
                origin,
            } => {
                if !matching_element
                    .is_none_or(|element| arcana.matches_element(battle, target, element))
                {
                    return;
                }
                (false, ratio_bp, origin)
            }
        };
        let ignore_defense = (self.enhanced || natural) && arcana.stacks >= self.ignore_threshold;
        let defense_ignore_bp = if ignore_defense {
            self.defense_ignore_bp
        } else {
            0
        };
        let mut hit = PeriodicHit {
            element: WIND,
            damage: DotScale::StackedAttack {
                multiplier_bp: self.arcana_multiplier_bp(arcana.stacks),
                defense_ignore_bp,
            },
            stacks: 1,
            ratio_bp,
            origin,
        };
        battle.periodic_hit(arcana.source, target, hit);

        if let Some(active) = &mut battle.units[target].black_swan.arcana {
            // A tick listener can add stacks during this hit. Thresholds and damage
            // use the pre-hit stacks, while the post-hit reset uses the current count.
            active.stacks = active.stacks.min(self.arcana_limit);
        }
        if natural {
            self.finish_natural_tick(battle, arcana.source, target);
            if arcana.stacks >= self.splash_threshold {
                hit.damage = DotScale::StackedAttack {
                    multiplier_bp: self.splash_bp,
                    defense_ignore_bp,
                };
                for neighbor in battle.adjacent_enemies(target) {
                    battle.periodic_hit(arcana.source, neighbor, hit);
                    if !self.enhanced && battle.hp(arcana.source).0 > 0 {
                        self.add_arcana(
                            battle,
                            arcana.source,
                            neighbor,
                            origin,
                            self.talent_chance_bp,
                            1,
                        );
                    }
                }
            }
        }
    }

    fn finish_natural_tick(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        let epiphany = battle.has_epiphany_from(owner, target);
        let state = &mut battle.units[target].black_swan;
        let Some(arcana) = &mut state.arcana else {
            return;
        };
        if self.enhanced {
            if !epiphany {
                arcana.stacks = (arcana.stacks / 2).max(1);
            }
        } else if epiphany && state.preserve_charges > 0 {
            state.preserve_charges -= 1;
        } else {
            arcana.stacks = 1;
        }
    }

    fn candle_bonus_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        (battle.effective_effect_hit_rate_bp(owner) * 60 / 100).min(7_200)
    }
}

impl Mechanics for BlackSwan {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate {
            for target in battle.enemies() {
                self.apply_epiphany(battle, actor, target);
            }
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let targets = battle.attack_targets().to_vec();
        if action == Action::Skill || (self.enhanced && self.goblet) {
            for &enemy in &targets {
                self.defense_down(battle, actor, enemy);
            }
        }
        let Some(target) = target else {
            return;
        };
        let extra = match action {
            Action::Basic => self.basic_arcana_chances,
            Action::Skill if self.viscera && !self.enhanced => Some((0, TRACE_CHANCE_BP)),
            _ => None,
        };
        // Arcana gained by this attack cannot qualify it for an extra application.
        let matching = extra.map_or(0, |_| self.matching_dot_count(battle, target));
        if let Some((base_chance, extra_chance)) = extra {
            if base_chance > 0 {
                self.add_arcana(battle, actor, target, action, base_chance, 1);
            }
            for _ in 0..matching {
                self.add_arcana(battle, actor, target, action, extra_chance, 1);
            }
        }
        if action == Action::Skill
            && let Some(chance) = self.skill_arcana_chance_bp
        {
            for enemy in targets {
                self.add_arcana(battle, actor, enemy, action, chance, 1);
            }
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        let stacks = if owner == source && self.enhanced && self.viscera {
            ENHANCED_A2_STACKS
        } else if owner != source && self.eidolon >= 6 {
            1
        } else {
            return;
        };
        for target in battle.attack_targets().to_vec() {
            self.add_arcana(battle, owner, target, origin, TRACE_CHANCE_BP, stacks);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        for target in battle.enemies() {
            if self.goblet {
                self.add_arcana(battle, owner, target, Action::Basic, TRACE_CHANCE_BP, 1);
                if self.enhanced {
                    self.defense_down(battle, owner, target);
                }
            }
            if self.enhanced && self.eidolon >= 2 {
                self.add_arcana(
                    battle,
                    owner,
                    target,
                    Action::Basic,
                    E2_CHANCE_BP,
                    E2_ENHANCED_STACKS,
                );
            }
        }
    }

    fn on_enemy_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        self.epiphany_energy(battle, owner, target);
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, _: usize, target: usize) {
        self.epiphany_energy(battle, owner, target);
        if !self.enhanced
            && self.eidolon >= 2
            && let Some(arcana) = battle.units[target].black_swan.arcana
            && arcana.source == owner
        {
            for neighbor in battle.adjacent_enemies(target) {
                self.add_arcana(
                    battle,
                    owner,
                    neighbor,
                    arcana.origin,
                    E2_CHANCE_BP,
                    E2_BASE_STACKS,
                );
            }
        }
    }

    fn on_enemy_dot_damage(&self, battle: &mut Battle<'_>, owner: usize, event: DotEvent) {
        if self.enhanced || event.timing == DotTiming::TurnStart(event.target) {
            self.add_arcana(
                battle,
                owner,
                event.target,
                event.origin,
                self.talent_chance_bp,
                1,
            );
        } else if self.goblet && event.timing == DotTiming::Detonation {
            let stacks = &mut battle.units[event.target].black_swan.detonations;
            if stacks.attack_index != battle.attack_index {
                stacks.attack_index = battle.attack_index;
                stacks.awarded = 0;
            }
            if stacks.awarded < DETONATION_STACK_LIMIT
                && self.add_arcana(
                    battle,
                    owner,
                    event.target,
                    event.origin,
                    TRACE_CHANCE_BP,
                    1,
                )
            {
                battle.units[event.target].black_swan.detonations.awarded += 1;
            }
        }
    }

    fn trigger_special_dot(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        trigger: SpecialDotTrigger,
    ) {
        if let Some(arcana) = battle.units[target].black_swan.arcana
            && arcana.source == owner
        {
            self.arcana_hit(battle, arcana, target, trigger);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: crate::DamageKind,
    ) -> u32 {
        if !self.enhanced && self.candle {
            self.candle_bonus_bp(battle, actor)
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize, _: Element) -> u32 {
        if self.enhanced && self.candle {
            self.candle_bonus_bp(battle, owner)
        } else {
            0
        }
    }

    fn elemental_resistance_down_bp(
        &self,
        battle: &Battle<'_>,
        _owner: usize,
        target: usize,
        element: Element,
    ) -> u32 {
        if self.eidolon >= 1
            && DOT_ELEMENTS.contains(&element)
            && battle.has_elemental_dot(target, element)
        {
            E1_RESISTANCE_DOWN_BP
        } else {
            0
        }
    }
}
