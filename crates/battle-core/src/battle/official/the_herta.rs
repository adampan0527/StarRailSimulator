use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1401";
const BASIC: &str = "skill.140101";
const SKILL: &str = "skill.140102";
const ULTIMATE: &str = "skill.140103";
const TALENT: &str = "skill.140104";
const ENHANCED_SKILL: &str = "skill.140109";
const HONESTY: &str = "trace.1401101";
const LETTER: &str = "trace.1401102";
const LANDSCAPE: &str = "trace.1401103";

const INTERPRETATION_CAP: u8 = 42;
const INSPIRATION_CAP: u8 = 4;
const ANSWER_CAP: u8 = 99;
const WAKE_UP_INTERPRETATION: u8 = 25;
const ENERGY_CAP_MILLI: u32 = 220_000;
const SKILL_RADIUS: u8 = 2;
const ICE: Element = Element::Ice;

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::battle) struct TheHertaState {
    inspiration: u8,
    answer: u8,
    carried_interpretation: u8,
    enhanced_layers_twice: Option<u8>,
    ultimate_multiplier_bonus_bp: u32,
}

struct TheHerta {
    enhanced_skill: OfficialAbility,
    skill_bp: u16,
    enhanced_skill_bp: u16,
    enhanced_finale_bp: u16,
    interpretation_main_bp: u16,
    interpretation_other_bp: u16,
    ultimate_attack_bp: u32,
    honesty: bool,
    letter: bool,
    landscape: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 220
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != ICE
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 4)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 6)?;
    let enhanced = input.parameters(ENHANCED_SKILL, 3)?;
    if skill[1] != 1.0 || enhanced[1] != 1.0 {
        return Err(invalid("skill Interpretation application"));
    }
    if ultimate[4] != 3.0 || ultimate[5] != f64::from(INSPIRATION_CAP) {
        return Err(invalid(ULTIMATE));
    }
    if talent[2..]
        != [
            f64::from(INTERPRETATION_CAP),
            1.0,
            f64::from(INTERPRETATION_CAP),
            f64::from(WAKE_UP_INTERPRETATION),
        ]
    {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP_MILLI,
        basic: OfficialAbility {
            target: Target::Enemy,
            skill_points: SkillPoints::Generate,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
        },
        skill: skill_attack(Effect::DirectedRadiusAttack {
            radius: SKILL_RADIUS,
        }),
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            skill_points: SkillPoints::Unchanged,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ULTIMATE, ultimate[0])?, 20)],
        },
        mechanics: Arc::new(TheHerta {
            enhanced_skill: skill_attack(Effect::DirectedAreaAttack),
            skill_bp: basis_points(SKILL, skill[0])?,
            enhanced_skill_bp: basis_points(ENHANCED_SKILL, enhanced[0])?,
            enhanced_finale_bp: basis_points(ENHANCED_SKILL, enhanced[2])?,
            interpretation_main_bp: basis_points(TALENT, talent[0])?,
            interpretation_other_bp: basis_points(TALENT, talent[1])?,
            ultimate_attack_bp: u32::from(basis_points(ULTIMATE, ultimate[3])?),
            honesty: trace(HONESTY),
            letter: trace(LETTER),
            landscape: trace(LANDSCAPE),
            eidolon: input.eidolon,
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
        element: ICE,
        multiplier_bp,
        toughness,
    }
}

fn skill_attack(effect: Effect) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        skill_points: SkillPoints::Spend,
        energy_milli: 30_000,
        effects: vec![effect],
    }
}

fn is_erudition(battle: &Battle<'_>, actor: usize) -> bool {
    battle.plan.units[actor]
        .combat_type
        .is_some_and(|kind| kind.path == CombatPath::Erudition)
}

fn dual_erudition(battle: &Battle<'_>) -> bool {
    battle
        .allies()
        .filter(|&ally| is_erudition(battle, ally))
        .nth(1)
        .is_some()
}

fn eligible_enemies<'a>(battle: &'a Battle<'_>) -> impl Iterator<Item = usize> + 'a {
    battle
        .enemies()
        .filter(|&enemy| battle.is_targetable(enemy))
}

fn set_interpretation(battle: &mut Battle<'_>, owner: usize, target: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[target].the_herta_interpretation, after);
    if before != after {
        battle.emit(
            EventKind::Interpretation { before, after },
            Some(owner),
            Some(target),
        );
    }
}

fn set_inspiration(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].the_herta_state.inspiration, after);
    if before != after {
        battle.emit(
            EventKind::Inspiration { before, after },
            Some(owner),
            Some(owner),
        );
    }
}

fn transfer_target(battle: &Battle<'_>) -> Option<usize> {
    eligible_enemies(battle).min_by_key(|&enemy| {
        (
            !battle.is_elite(enemy),
            Reverse(battle.units[enemy].the_herta_interpretation),
            enemy,
        )
    })
}

fn transfer_interpretation(battle: &mut Battle<'_>, owner: usize, layers: u8) {
    if let Some(enemy) = transfer_target(battle) {
        let after = (battle.units[enemy].the_herta_interpretation + layers).min(INTERPRETATION_CAP);
        set_interpretation(battle, owner, enemy, after);
    }
}

impl TheHerta {
    fn apply_interpretation(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        enemy: usize,
        layers: u8,
    ) {
        let before = battle.units[enemy].the_herta_interpretation;
        let after = (before + layers).min(INTERPRETATION_CAP);
        set_interpretation(battle, owner, enemy, after);
        if self.landscape {
            let answer = &mut battle.units[owner].the_herta_state.answer;
            let before_answer = *answer;
            *answer = (*answer + (after - before)).min(ANSWER_CAP);
            let after = *answer;
            if before_answer != after {
                battle.emit(
                    EventKind::HertaAnswer {
                        before: before_answer,
                        after,
                    },
                    Some(owner),
                    Some(owner),
                );
            }
        }
    }

    fn enhanced_layers_twice(&self, battle: &Battle<'_>, target: usize) -> u8 {
        let base = 2 * battle.units[target].the_herta_interpretation;
        let extra = if self.eidolon >= 1 {
            std::iter::once(target)
                .chain(battle.adjacent_enemies(target))
                .map(|enemy| battle.units[enemy].the_herta_interpretation)
                .max()
                .unwrap_or_default()
        } else {
            0
        };
        (base + extra).min(2 * INTERPRETATION_CAP)
    }

    fn start_ultimate(&self, battle: &mut Battle<'_>, owner: usize) {
        let mut enemies: Vec<_> = eligible_enemies(battle).collect();
        let mut layers: Vec<_> = enemies
            .iter()
            .map(|&enemy| battle.units[enemy].the_herta_interpretation)
            .collect();
        enemies.sort_unstable_by_key(|&enemy| (!battle.is_elite(enemy), enemy));
        layers.sort_unstable_by_key(|&count| Reverse(count));
        let eidolon_bonus = if self.eidolon >= 6 {
            match enemies.len() {
                1 => 40_000,
                2 => 25_000,
                _ => 14_000,
            }
        } else {
            0
        };
        let state = &mut battle.units[owner].the_herta_state;
        state.ultimate_multiplier_bonus_bp = eidolon_bonus + 100 * u32::from(state.answer);
        for (enemy, count) in enemies.into_iter().zip(layers) {
            set_interpretation(battle, owner, enemy, count);
        }
        battle.apply_ally_buff_with_id(
            owner,
            owner,
            AllyBuffKind::AttackPercent,
            self.ultimate_attack_bp,
            3,
            Some(ULTIMATE),
        );
    }

    fn skill_hits(&self, battle: &mut Battle<'_>, owner: usize, primary: usize) {
        let enhanced_layers = battle.units[owner].the_herta_state.enhanced_layers_twice;
        let enhanced = enhanced_layers.is_some();
        let phase_count = if enhanced { 4 } else { 3 };
        let opening_bp = if enhanced {
            self.enhanced_skill_bp
        } else {
            self.skill_bp
        };
        let team_factor = if dual_erudition(battle) { 2 } else { 1 };
        for phase in 0..phase_count {
            let finale = phase == 3;
            for enemy in battle.enemies() {
                let distance = primary.abs_diff(enemy);
                if !battle.is_targetable(enemy) || (!finale && distance > phase) {
                    continue;
                }
                let main = enemy == primary;
                let mut multiplier = if finale {
                    self.enhanced_finale_bp
                } else {
                    opening_bp
                };
                if let Some(layers_twice) = enhanced_layers {
                    let per_layer = if main {
                        self.interpretation_main_bp
                    } else {
                        self.interpretation_other_bp
                    };
                    let total_bonus =
                        u32::from(per_layer) * u32::from(layers_twice) * team_factor / 2;
                    let hits = (phase_count - distance.min(3)) as u32;
                    let bonus = total_bonus / hits + if finale { total_bonus % hits } else { 0 };
                    multiplier =
                        multiplier.saturating_add(u16::try_from(bonus).unwrap_or(u16::MAX));
                }
                let toughness = if main || (!finale && distance <= usize::from(SKILL_RADIUS)) {
                    5
                } else {
                    0
                };
                battle.attack_hit(owner, enemy, ICE, multiplier, toughness);
            }
        }
    }
}

impl Mechanics for TheHerta {
    fn ability_override(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Skill && battle.units[owner].the_herta_state.inspiration > 0)
            .then_some(&self.enhanced_skill)
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 2 {
            set_inspiration(battle, owner, 1);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        let enemies: Vec<_> = eligible_enemies(battle).collect();
        for &enemy in &enemies {
            self.apply_interpretation(battle, owner, enemy, 1);
        }
        let mut elite: Vec<_> = enemies
            .iter()
            .copied()
            .filter(|&enemy| battle.is_elite(enemy))
            .collect();
        if elite.is_empty() {
            elite = enemies;
        }
        if !elite.is_empty() {
            let choice = battle.random_index(elite.len());
            self.apply_interpretation(battle, owner, elite[choice], WAKE_UP_INTERPRETATION);
        }
        let carried =
            std::mem::take(&mut battle.units[owner].the_herta_state.carried_interpretation);
        if carried > 0 {
            transfer_interpretation(battle, owner, carried);
        }
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, _: usize, enemy: usize) {
        let layers = battle.units[enemy].the_herta_interpretation;
        if layers == 0 {
            return;
        }
        set_interpretation(battle, owner, enemy, 0);
        if eligible_enemies(battle).next().is_some() {
            transfer_interpretation(battle, owner, layers);
        } else if battle.wave_index + 1 < battle.plan.wave_ends.len() {
            battle.units[owner].the_herta_state.carried_interpretation = layers;
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(enemy)) => {
                self.apply_interpretation(battle, owner, enemy, 1);
                let inspiration = battle.units[owner].the_herta_state.inspiration;
                if inspiration > 0 {
                    let snapshot = self.enhanced_layers_twice(battle, enemy);
                    battle.units[owner].the_herta_state.enhanced_layers_twice = Some(snapshot);
                    set_inspiration(battle, owner, inspiration - 1);
                    // Consume the original stacks before a lethal hit can transfer them.
                    let reset = if self.eidolon >= 1 { 15 } else { 1 };
                    set_interpretation(battle, owner, enemy, reset);
                }
            }
            (Action::Ultimate, _) => self.start_ultimate(battle, owner),
            _ => {}
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        self.skill_hits(battle, owner, target);
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Ultimate => {
                let state = &mut battle.units[owner].the_herta_state;
                state.ultimate_multiplier_bonus_bp = 0;
                let inspiration =
                    (state.inspiration + 1 + u8::from(self.eidolon >= 2)).min(INSPIRATION_CAP);
                set_inspiration(battle, owner, inspiration);
                if battle.ordinary_actor != Some(owner) {
                    battle.advance_immediate(owner);
                }
            }
            Action::Skill => {
                let enhanced = battle.units[owner]
                    .the_herta_state
                    .enhanced_layers_twice
                    .take()
                    .is_some();
                if enhanced && self.eidolon >= 2 {
                    battle.advance(owner, 3_500);
                }
            }
            _ => {}
        }
    }

    fn on_ally_first_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        enemy: usize,
    ) {
        if self.honesty {
            self.apply_interpretation(battle, owner, enemy, 1);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        _: Action,
        _: Option<usize>,
    ) {
        let letter_active = self.letter && dual_erudition(battle);
        if self.honesty {
            let targets = battle
                .attack_targets()
                .len()
                .clamp(if letter_active { 3 } else { 1 }, 5);
            battle.restore_energy_flat(owner, owner, 3_000 * targets as u32);
        }
        if letter_active {
            let target = battle
                .attack_targets()
                .iter()
                .copied()
                .filter(|&enemy| battle.is_targetable(enemy))
                .min_by_key(|&enemy| {
                    (Reverse(battle.units[enemy].the_herta_interpretation), enemy)
                });
            if let Some(enemy) = target {
                self.apply_interpretation(
                    battle,
                    owner,
                    enemy,
                    if is_erudition(battle, source) { 3 } else { 1 },
                );
            }
        }
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if action == Some(Action::Ultimate) && kind == DamageKind::Direct {
            base + battle.units[owner]
                .the_herta_state
                .ultimate_multiplier_bonus_bp
        } else {
            base
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.honesty
            && action == Some(Action::Skill)
            && kind == DamageKind::Direct
            && battle.units[owner].the_herta_state.enhanced_layers_twice
                == Some(2 * INTERPRETATION_CAP)
        {
            5_000
        } else {
            0
        }
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, _: usize, _: usize) -> u32 {
        if self.letter && dual_erudition(battle) {
            8_000
        } else {
            0
        }
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, _: usize, target: usize) -> u32 {
        if self.eidolon >= 4 && is_erudition(battle, target) {
            1_200
        } else {
            0
        }
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, element: Element) -> u32 {
        if self.eidolon >= 6 && element == ICE {
            2_000
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
        if action != Action::Skill || battle.units[owner].the_herta_state.inspiration == 0 {
            return None;
        }
        let target = target?;
        let strongest = eligible_enemies(battle)
            .map(|enemy| self.enhanced_layers_twice(battle, enemy))
            .max()?;
        Some(if self.enhanced_layers_twice(battle, target) == strongest {
            1
        } else {
            2
        })
    }
}
