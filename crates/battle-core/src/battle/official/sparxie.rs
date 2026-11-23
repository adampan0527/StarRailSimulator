use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        elation::AhaInstant,
        resolve::{DamageTargetAura, ElationHit},
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1501";
const BASIC: &str = "skill.150101";
const SKILL: &str = "skill.150102";
const ULTIMATE: &str = "skill.150103";
const TALENT: &str = "skill.150104";
const ENHANCED_BASIC: &str = "skill.150108";
const ENGAGEMENT_FARMING: &str = "skill.150109";
const ELATION_SKILL: &str = "skill.150120";
const SWEET: &str = "trace.1501101";
const DAZZLING: &str = "trace.1501102";
const SIZZLING: &str = "trace.1501103";
const FIRE: Element = Element::Fire;
const ENERGY: u32 = 160;
const E2_CRIT_STACKS: usize = 4;

#[derive(Clone, Default)]
pub(in crate::battle) struct SparxieState {
    live_interactions: [u8; 2],
    live_straight_fire: [u8; 2],
    interactions_since_straight_fire: Option<u8>,
    thrill: u32,
    thrill_crit_expires: [u32; E2_CRIT_STACKS],
    ult_elation_expires: u32,
}

struct Sparxie {
    eidolon: u8,
    sweet: bool,
    dazzling: bool,
    sizzling: bool,
    live_limit: u8,
    straight_fire_chance_bp: u32,
    straight_fire_limit: u8,
    straight_fire_guarantee: u8,
    straight_fire_start: u8,
    straight_fire_points: u8,
    straight_fire_punchline: u32,
    unreal_banger_punchline: u32,
    enhanced_basic: OfficialAbility,
    enhanced_bp: [u16; 2],
    interaction_bp: [u16; 2],
    ultimate_punchline: u32,
    ultimate_bp: (u16, u16),
    talent_basic_bp: [u16; 2],
    talent_bounce_bp: u16,
    talent_ultimate_bp: u16,
    encore_bp: [u16; 2],
    encore_bounces: u32,
    encore_thrill: u32,
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn basis_points(field: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(field));
    }
    Ok(value as u16)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != FIRE
        || input.energy_capacity != ENERGY
    {
        return Err(invalid("combat type or energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 4)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 2)?;
    let interaction = input.parameters(ENGAGEMENT_FARMING, 5)?;
    let encore = input.parameters(ELATION_SKILL, 4)?;
    if skill != [20.0, 28.0, 3.0, 7.0, 4.0] {
        return Err(invalid(SKILL));
    }
    if ultimate[0] != 2.0 || ultimate[2] != 0.6 {
        return Err(invalid(ULTIMATE));
    }
    if interaction[..3] != [2.0, 1.0, 2.0] {
        return Err(invalid(ENGAGEMENT_FARMING));
    }
    if encore[2..] != [20.0, 2.0] {
        return Err(invalid(ELATION_SKILL));
    }
    let enhanced_bp = [
        basis_points(ENHANCED_BASIC, enhanced[0])?,
        basis_points(ENHANCED_BASIC, enhanced[1])?,
    ];
    let interaction_bp = [
        basis_points(ENGAGEMENT_FARMING, interaction[3])?,
        basis_points(ENGAGEMENT_FARMING, interaction[4])?,
    ];
    let live_limit = skill[0] as u8;
    if enhanced_bp
        .iter()
        .zip(interaction_bp)
        .any(|(&base, per_point)| {
            u32::from(base) + u32::from(per_point) * u32::from(live_limit) > u32::from(u16::MAX)
        })
    {
        return Err(invalid(ENGAGEMENT_FARMING));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            effects: vec![Effect::Hit {
                element: FIRE,
                multiplier_bp: basis_points(BASIC, basic[0])?,
                toughness: 10,
            }],
            energy_milli: 20_000,
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            effects: Vec::new(),
            energy_milli: 0,
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            effects: vec![Effect::DirectedAreaAttack],
            energy_milli: 5_000,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Sparxie {
            eidolon: input.eidolon,
            sweet: has_trace(SWEET),
            dazzling: has_trace(DAZZLING),
            sizzling: has_trace(SIZZLING),
            live_limit,
            // Hidden fields follow the observed refund cap and persistent guarantee.
            straight_fire_chance_bp: (skill[1] * 100.0) as u32,
            straight_fire_limit: skill[2] as u8,
            straight_fire_guarantee: skill[3] as u8,
            straight_fire_start: skill[4] as u8,
            straight_fire_points: interaction[0] as u8,
            unreal_banger_punchline: interaction[1] as u32,
            straight_fire_punchline: interaction[2] as u32,
            enhanced_basic: OfficialAbility {
                target: Target::Enemy,
                effects: vec![Effect::DirectedAttack { adjacent: true }],
                energy_milli: 40_000,
                skill_points: SkillPoints::Generate,
            },
            enhanced_bp,
            interaction_bp,
            ultimate_punchline: ultimate[0] as u32,
            ultimate_bp: (
                basis_points(ULTIMATE, ultimate[1])?,
                basis_points(ULTIMATE, ultimate[2])?,
            ),
            talent_basic_bp: [
                basis_points(TALENT, talent[2])?,
                basis_points(TALENT, talent[3])?,
            ],
            talent_bounce_bp: basis_points(TALENT, talent[0])?,
            talent_ultimate_bp: basis_points(TALENT, talent[1])?,
            encore_bp: [
                basis_points(ELATION_SKILL, encore[1])?,
                basis_points(ELATION_SKILL, encore[0])?,
            ],
            encore_bounces: encore[2] as u32,
            encore_thrill: encore[3] as u32,
        }),
    })
}

fn live_slot(battle: &Battle<'_>, actor: usize) -> usize {
    usize::from(battle.in_extra_turn(actor))
}

fn set_live_interactions(battle: &mut Battle<'_>, actor: usize, slot: usize, after: u8) {
    let state = &mut battle.units[actor].sparxie_state;
    let before = std::mem::replace(&mut state.live_interactions[slot], after);
    if after == 0 {
        state.live_straight_fire[slot] = 0;
    }
    if before != after {
        battle.emit(
            EventKind::BasicEnhancement { before, after },
            Some(actor),
            None,
        );
    }
}

fn set_thrill(battle: &mut Battle<'_>, actor: usize, after: u32) {
    let before = std::mem::replace(&mut battle.units[actor].sparxie_state.thrill, after);
    if before != after {
        battle.emit(EventKind::Thrill { before, after }, Some(actor), None);
    }
}

fn add_thrill(battle: &mut Battle<'_>, actor: usize, amount: u32) {
    let before = battle.units[actor].sparxie_state.thrill;
    set_thrill(battle, actor, before.saturating_add(amount));
}

fn talent_hit(
    battle: &mut Battle<'_>,
    actor: usize,
    target: usize,
    punchline: u32,
    origin: Action,
    multiplier_bp: u16,
    toughness: u16,
) {
    battle.elation_hit(
        actor,
        target,
        ElationHit {
            element: FIRE,
            multiplier_bp: u32::from(multiplier_bp),
            punchline,
            toughness,
            origin,
            factor_bp: 10_000,
            assisted_by: None,
        },
    );
}

// Retain sub-unit toughness between distinct hits of the same Encore on the same target.
fn encore_toughness(remainders: &mut [(usize, u16)], target: usize, thirds: u16) -> u16 {
    if let Some((_, remainder)) = remainders.iter_mut().find(|(enemy, _)| *enemy == target) {
        let total = *remainder + thirds;
        *remainder = total % 3;
        total / 3
    } else {
        thirds / 3
    }
}

impl Sparxie {
    fn give_interaction_gift(&self, battle: &mut Battle<'_>, actor: usize, slot: usize) {
        let state = &battle.units[actor].sparxie_state;
        let attempts = state
            .interactions_since_straight_fire
            .unwrap_or(self.straight_fire_start)
            .saturating_add(1)
            .min(self.straight_fire_guarantee);
        let straight_fire = state.live_straight_fire[slot] < self.straight_fire_limit
            && (attempts >= self.straight_fire_guarantee
                || battle.roll_fixed(self.straight_fire_chance_bp));
        let state = &mut battle.units[actor].sparxie_state;
        state.interactions_since_straight_fire = Some(if straight_fire { 0 } else { attempts });
        let punchline = if straight_fire {
            state.live_straight_fire[slot] += 1;
            battle.restore_skill_points(actor, self.straight_fire_points);
            self.straight_fire_punchline
        } else {
            self.unreal_banger_punchline
        };
        battle.add_punchline(actor, punchline);
    }

    fn perform_interaction(&self, battle: &mut Battle<'_>, actor: usize) {
        let slot = live_slot(battle, actor);
        let state = &battle.units[actor].sparxie_state;
        let uses = state.live_interactions[slot];
        let thrill = state.thrill;
        if thrill > 0 {
            set_thrill(battle, actor, thrill - 1);
            if self.eidolon >= 2 {
                let unit = &mut battle.units[actor];
                let expiry = unit.normal_turns + 2;
                if let Some(stack) = unit.sparxie_state.thrill_crit_expires.iter_mut().min() {
                    *stack = expiry;
                }
            }
            battle.spend_skill_points_with_substitute(actor, 0, 1);
        } else {
            battle.spend_skill_points(actor, 1);
        }
        set_live_interactions(battle, actor, slot, uses + 1);
        self.give_interaction_gift(battle, actor, slot);
    }

    fn enhanced_basic(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let slot = live_slot(battle, actor);
        let uses = battle.units[actor].sparxie_state.live_interactions[slot];
        let mut targets: Vec<_> = std::iter::once(target)
            .chain(battle.adjacent_enemies(target))
            .collect();
        // The setup presses consumed points before this attack's own resolution window.
        battle.attack_spent_skill_points = true;
        for (index, &enemy) in targets.iter().enumerate() {
            let adjacent = usize::from(index != 0);
            let multiplier =
                self.enhanced_bp[adjacent] + self.interaction_bp[adjacent] * u16::from(uses);
            let toughness = [10, 5][adjacent];
            battle.attack_hit(actor, enemy, FIRE, multiplier, toughness);
        }
        if let Some(punchline) = battle.certified_banger(actor) {
            for (index, &enemy) in targets.iter().enumerate() {
                let multiplier = self.talent_basic_bp[usize::from(index != 0)];
                talent_hit(
                    battle,
                    actor,
                    enemy,
                    punchline,
                    Action::Basic,
                    multiplier,
                    0,
                );
            }
            targets.retain(|&enemy| battle.is_targetable(enemy));
            for _ in 0..uses {
                let selected = battle.random_index(targets.len());
                let Some(&enemy) = targets.get(selected) else {
                    break;
                };
                talent_hit(
                    battle,
                    actor,
                    enemy,
                    punchline,
                    Action::Basic,
                    self.talent_bounce_bp,
                    5,
                );
                if !battle.is_targetable(enemy) {
                    targets.remove(selected);
                }
            }
        }
        set_live_interactions(battle, actor, slot, 0);
    }
}

impl Mechanics for Sparxie {
    fn elation_participant(&self) -> Option<u16> {
        Some(144)
    }

    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        let state = &battle.units[actor].sparxie_state;
        state.live_interactions[live_slot(battle, actor)] < self.live_limit
    }

    fn shared_skill_point_cost(&self, battle: &Battle<'_>, actor: usize, action: Action) -> u8 {
        u8::from(action == Action::Skill && battle.units[actor].sparxie_state.thrill == 0)
    }

    fn prepare_action(&self, battle: &mut Battle<'_>, actor: usize, action: Action) -> bool {
        if action != Action::Skill {
            return false;
        }
        self.perform_interaction(battle, actor);
        true
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic
            && battle.units[actor].sparxie_state.live_interactions[live_slot(battle, actor)] > 0)
            .then_some(&self.enhanced_basic)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action != Action::Ultimate {
            return;
        }
        battle.add_punchline(actor, self.ultimate_punchline);
        if self.dazzling {
            let tier = battle.elation_team_size().clamp(1, 3) - 1;
            battle.add_punchline(actor, [2, 4, 8][tier]);
            add_thrill(battle, actor, [1, 1, 4][tier]);
        }
        if self.eidolon >= 4 {
            battle.add_punchline(actor, 5);
            let unit = &mut battle.units[actor];
            unit.sparxie_state.ult_elation_expires = unit.normal_turns + 3;
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if battle.action == Some(Action::Ultimate) {
            battle.attack_hit_with_elation(actor, target, FIRE, self.ultimate_bp, 20);
        } else {
            self.enhanced_basic(battle, actor, target);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate
            && let Some(punchline) = battle.certified_banger(actor)
        {
            for target in battle.enemies() {
                talent_hit(
                    battle,
                    actor,
                    target,
                    punchline,
                    action,
                    self.talent_ultimate_bp,
                    0,
                );
            }
        }
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, instant: AhaInstant) {
        let mut toughness_remainders: Vec<_> = battle.enemies().map(|enemy| (enemy, 0)).collect();
        let mut hit = |battle: &mut Battle<'_>, target, multiplier_bp, toughness_thirds| {
            battle.elation_hit(
                actor,
                target,
                ElationHit {
                    element: FIRE,
                    multiplier_bp: u32::from(multiplier_bp),
                    punchline: instant.punchline,
                    toughness: encore_toughness(
                        &mut toughness_remainders,
                        target,
                        toughness_thirds,
                    ),
                    origin: Action::ElationSkill,
                    factor_bp: instant.skill_damage_factor_bp,
                    assisted_by: None,
                },
            );
        };
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                hit(battle, target, self.encore_bp[0], 20);
            }
        }
        let bounces = if self.eidolon == 6 {
            self.encore_bounces
                .saturating_add(instant.punchline)
                .min(40)
        } else {
            self.encore_bounces
        };
        for _ in 0..bounces {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            hit(battle, target, self.encore_bp[1], 5);
        }
        add_thrill(battle, actor, self.encore_thrill);
        battle.gain_energy(actor, actor, 5_000);
    }

    fn on_aha_instant_finished(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            battle.add_punchline(actor, 5);
        }
        if self.eidolon >= 2 {
            add_thrill(battle, actor, 2);
            battle.queue_extra_turn(actor);
        }
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, extra: bool) {
        if owner == actor && !extra {
            let unit = &mut battle.units[owner];
            let turn = unit.normal_turns;
            for expiry in &mut unit.sparxie_state.thrill_crit_expires {
                if *expiry <= turn {
                    *expiry = 0;
                }
            }
            if unit.sparxie_state.ult_elation_expires <= turn {
                unit.sparxie_state.ult_elation_expires = 0;
            }
        }
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        3_600 * u32::from(battle.units[actor].sparxie_state.ult_elation_expires > 0)
    }

    fn elation_from_attack_bp(&self, attack: u32) -> u32 {
        if self.sweet {
            (attack.saturating_sub(2_000) / 100).min(16) * 500
        } else {
            0
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        battle.units[actor]
            .sparxie_state
            .thrill_crit_expires
            .iter()
            .filter(|&&expiry| expiry > 0)
            .count() as u32
            * 1_000
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if self.sizzling && battle.is_targetable(owner) {
            battle.current_punchline().min(10) * 800
        } else {
            0
        }
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        let resistance_down_bp =
            if self.eidolon >= 1 && battle.is_targetable(owner) && battle.is_party_unit(actor) {
                battle.current_punchline().min(10) * 150
            } else {
                0
            };
        DamageTargetAura {
            resistance_down_bp,
            ..DamageTargetAura::default()
        }
    }

    fn resistance_penetration_bp(&self, _: &Battle<'_>, _: usize, _: Element) -> u32 {
        2_000 * u32::from(self.eidolon == 6)
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Skill => Some(1),
            Action::Basic => Some(2),
            _ => None,
        }
    }
}
