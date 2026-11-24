use std::{collections::VecDeque, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        elation::AhaInstant,
        resolve::{DamageTargetAura, ElationHit},
        support::percentage_of,
    },
    CombatPath, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1506";
const BASIC: &str = "skill.150601";
const SKILL: &str = "skill.150602";
const ULTIMATE: &str = "skill.150603";
const TALENT: &str = "skill.150604";
const ENHANCED: &str = "skill.150608";
const ELATION: &str = "skill.150620";
const ENHANCED_ELATION: &str = "skill.150621";
const IMAGINARY: Element = Element::Imaginary;
const FULL_CHANCE_BP: u32 = 10_000;
const RATING_THRESHOLD: u32 = 60;
const RATING_CAP: u32 = 300;
const E2_EXTRA_TURN_THRESHOLD: u32 = 120;
const BOUNCES: u8 = 100;
const LOOT_BOXES: u8 = 3;
const BETWEEN_BOXES: u8 = BOUNCES / LOOT_BOXES;

#[derive(Clone, Copy)]
enum LootBox {
    Sword,
    Egg,
    Bean,
}

#[derive(Clone, Copy, Default)]
struct BounceProgress {
    shots: u8,
    boxes: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct SilverWolf999State {
    rating: u32,
    godmode: Option<Godmode>,
    enhanced_in_progress: bool,
    enhanced_completed: bool,
    pending_boxes: VecDeque<LootBox>,
}

#[derive(Clone)]
struct Godmode {
    basics_left: u8,
    rating_gained: u32,
    box_chance_bp: u32,
    continuation: Option<BounceProgress>,
}

struct SilverWolf999 {
    eidolon: u8,
    speed_trace: bool,
    elation_trace: bool,
    godmode_trace: bool,
    enhanced: OfficialAbility,
    skill_punchline: u32,
    talent_damage_bp: u16,
    crit_rate_per_rating_bp: u32,
    crit_damage_per_rating_bp: u32,
    bounce_total_bp: u16,
    finisher_bp: u16,
    box_damage_bp: u16,
    elation_rating: u32,
    enhanced_elation_damage_bp: u16,
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn ratio(field: &str, value: f64) -> Result<u16, BattleError> {
    let value = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(field));
    }
    Ok(value as u16)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Elation
        || input.combat_type.element != IMAGINARY
        || input.energy_capacity != RATING_THRESHOLD
    {
        return Err(invalid("combat type or capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 7)?;
    let talent = input.parameters(TALENT, 6)?;
    let enhanced = input.parameters(ENHANCED, 8)?;
    let elation = input.parameters(ELATION, 1)?;
    let enhanced_elation = input.parameters(ENHANCED_ELATION, 2)?;
    let equal_ratio = |actual: f64, expected: f64| (actual - expected).abs() < 0.000_001;
    if skill[1] != 5.0
        || ultimate[0] != f64::from(RATING_THRESHOLD)
        || !equal_ratio(ultimate[3], 0.2)
        || !equal_ratio(ultimate[4], 0.2)
        || ultimate[5..] != [3.0, 2.0]
        || talent[0] != f64::from(RATING_THRESHOLD)
        || talent[1] != f64::from(RATING_CAP - RATING_THRESHOLD)
        || talent[4] != 3.0
        || enhanced[2] != f64::from(BOUNCES)
        || enhanced[4..7] != [f64::from(LOOT_BOXES), 60.0, 2.0]
        || !equal_ratio(enhanced[7], 0.15)
        || elation[0] != 15.0
        || enhanced_elation[1] != 6.0
    {
        return Err(invalid("4.7 skill parameters"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: OfficialAbility {
            target: Target::Enemy,
            effects: vec![Effect::Hit {
                element: IMAGINARY,
                multiplier_bp: ratio(BASIC, basic[0])?,
                toughness: 10,
            }],
            energy_milli: 0,
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllEnemies,
            effects: vec![Effect::Hit {
                element: IMAGINARY,
                multiplier_bp: ratio(SKILL, skill[0])?,
                toughness: 10,
            }],
            energy_milli: 0,
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            effects: Vec::new(),
            energy_milli: 0,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(SilverWolf999 {
            eidolon: input.eidolon,
            speed_trace: has_trace("trace.1506101"),
            elation_trace: has_trace("trace.1506102"),
            godmode_trace: has_trace("trace.1506103"),
            enhanced: OfficialAbility {
                target: Target::AllEnemies,
                effects: vec![Effect::DirectedAreaAttack],
                energy_milli: 0,
                skill_points: SkillPoints::Unchanged,
            },
            skill_punchline: skill[1] as u32,
            talent_damage_bp: ratio(TALENT, talent[2])?,
            crit_rate_per_rating_bp: u32::from(ratio(TALENT, talent[3])?),
            crit_damage_per_rating_bp: u32::from(ratio(TALENT, talent[5])?),
            bounce_total_bp: ratio(ENHANCED, enhanced[0])?,
            finisher_bp: ratio(ENHANCED, enhanced[3])?,
            box_damage_bp: ratio(ULTIMATE, ultimate[2])?,
            elation_rating: elation[0] as u32,
            enhanced_elation_damage_bp: ratio(ENHANCED_ELATION, enhanced_elation[0])?,
        }),
    })
}

fn state<'a>(battle: &'a Battle<'_>, actor: usize) -> &'a SilverWolf999State {
    &battle.units[actor].silver_wolf_999_state
}

fn state_mut<'a>(battle: &'a mut Battle<'_>, actor: usize) -> &'a mut SilverWolf999State {
    &mut battle.units[actor].silver_wolf_999_state
}

fn set_rating(battle: &mut Battle<'_>, actor: usize, after: u32) {
    let before = std::mem::replace(&mut state_mut(battle, actor).rating, after);
    if before != after {
        battle.emit(EventKind::HiddenMmr { before, after }, Some(actor), None);
    }
}

fn extend_buffs(battle: &mut Battle<'_>, actor: usize) {
    battle.extend_active_buffs_by_one_turn(actor);
    battle.extend_certified_banger_by_one_turn(actor);
}

fn score_cap(battle: &Battle<'_>) -> u32 {
    match battle.elation_ascension() {
        Some(ascension) if ascension.innate_level >= 2 => 999,
        Some(_) => 360,
        None => RATING_CAP,
    }
}

fn select_loot_box(battle: &mut Battle<'_>) -> LootBox {
    [LootBox::Sword, LootBox::Egg, LootBox::Bean][battle.random_index(3)]
}

/// Distribute fixed-point rounding remainders in hit order.
fn split(value: u16, count: u16, index: u16) -> u16 {
    value / count + u16::from(index < value % count)
}

impl SilverWolf999 {
    fn gain_rating(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        let before = state(battle, actor).rating;
        let after = before
            .saturating_add(amount)
            .min(score_cap(battle).max(before));
        set_rating(battle, actor, after);
        self.add_godmode_rating(battle, actor, after - before);
    }

    fn add_godmode_rating(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        if self.eidolon < 2 || amount == 0 {
            return;
        }
        if let Some(godmode) = &mut state_mut(battle, actor).godmode {
            let before = godmode.rating_gained / E2_EXTRA_TURN_THRESHOLD;
            godmode.rating_gained += amount;
            let after = godmode.rating_gained / E2_EXTRA_TURN_THRESHOLD;
            godmode.basics_left += (after - before) as u8;
            for _ in before..after {
                battle.queue_extra_turn(actor);
            }
        }
    }

    fn enter_godmode(&self, battle: &mut Battle<'_>, actor: usize) {
        if state(battle, actor).godmode.is_some() {
            return;
        }
        let initial_rating = state(battle, actor).rating;
        state_mut(battle, actor).godmode = Some(Godmode {
            basics_left: 3,
            rating_gained: 0,
            box_chance_bp: FULL_CHANCE_BP,
            continuation: None,
        });
        battle.emit(
            EventKind::StatusApplied(StatusKind::GodmodePlayer),
            Some(actor),
            Some(actor),
        );
        battle.advance_immediate(actor);
        if self.eidolon >= 2 {
            extend_buffs(battle, actor);
            self.add_godmode_rating(battle, actor, initial_rating);
        }
        if self.godmode_trace {
            self.gain_rating(battle, actor, 20);
        }
    }

    fn finish_enhanced(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = state_mut(battle, actor);
        if !std::mem::take(&mut state.enhanced_completed) {
            return;
        }
        let Some(godmode) = &mut state.godmode else {
            return;
        };
        godmode.basics_left -= 1;
        if godmode.basics_left == 0 {
            state.godmode = None;
            let retained = if self.eidolon >= 1 {
                state.rating / 5
            } else {
                0
            };
            set_rating(battle, actor, retained);
            battle.emit(
                EventKind::StatusExpired(StatusKind::GodmodePlayer),
                Some(actor),
                Some(actor),
            );
        }
    }

    fn hit_elation(
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        multiplier_bp: u16,
        toughness: u16,
        origin: Action,
        instant: AhaInstant,
    ) {
        battle.elation_hit(
            actor,
            target,
            ElationHit {
                element: IMAGINARY,
                multiplier_bp: u32::from(multiplier_bp),
                punchline: instant.punchline,
                toughness,
                origin,
                factor_bp: instant.skill_damage_factor_bp,
                assisted_by: None,
            },
        );
    }

    fn enhanced_hit(
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        multiplier_bp: u16,
        toughness: u16,
    ) {
        if let Some(punchline) = battle.certified_banger(actor) {
            Self::hit_elation(
                battle,
                actor,
                target,
                multiplier_bp,
                toughness,
                Action::Basic,
                AhaInstant {
                    punchline,
                    skill_damage_factor_bp: FULL_CHANCE_BP as u16,
                },
            );
        } else {
            battle.attack_hit(actor, target, IMAGINARY, multiplier_bp, toughness);
        }
    }

    fn loot_box(&self, battle: &mut Battle<'_>, actor: usize, kind: LootBox, origin: Action) {
        let targets: Vec<_> = battle
            .enemies()
            .filter(|&target| battle.is_targetable(target))
            .collect();
        if targets.is_empty() {
            return;
        }
        let instant = AhaInstant {
            punchline: battle.certified_banger(actor).unwrap_or(0),
            skill_damage_factor_bp: FULL_CHANCE_BP as u16,
        };
        let previous_damage = battle.attack_damage.replace(0);
        for (index, &target) in targets.iter().enumerate() {
            let multiplier = split(self.box_damage_bp, targets.len() as u16, index as u16);
            Self::hit_elation(battle, actor, target, multiplier, 10, origin, instant);
        }
        let damage = battle.attack_damage.take().unwrap_or_default();
        battle.attack_damage = previous_damage.map(|previous| previous.saturating_add(damage));
        match kind {
            LootBox::Sword => {
                if let Some(target) = targets
                    .into_iter()
                    .filter(|&target| battle.is_targetable(target))
                    .max_by_key(|&target| (battle.units[target].hp, std::cmp::Reverse(target)))
                {
                    let amount = (damage / 5).min(u64::from(u32::MAX)) as u32;
                    if amount > 0 {
                        battle.true_damage(actor, target, origin, amount, Vec::new());
                    }
                }
            }
            LootBox::Egg => {
                battle.restore_skill_points(actor, 2);
            }
            LootBox::Bean => battle.add_punchline(actor, 3),
        }
    }

    fn enhanced_basic(&self, battle: &mut Battle<'_>, actor: usize) {
        let Some(godmode) = &mut state_mut(battle, actor).godmode else {
            return;
        };
        let mut progress = godmode.continuation.take().unwrap_or_default();
        state_mut(battle, actor).enhanced_in_progress = true;
        while battle.enemies().any(|target| battle.is_targetable(target)) {
            if progress.boxes < LOOT_BOXES && progress.shots == (progress.boxes + 1) * BETWEEN_BOXES
            {
                let kind = select_loot_box(battle);
                self.loot_box(battle, actor, kind, Action::Basic);
                progress.boxes += 1;
                continue;
            }
            if progress.shots == BOUNCES {
                break;
            }
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            let multiplier = split(
                self.bounce_total_bp,
                u16::from(BOUNCES),
                u16::from(progress.shots),
            );
            // The first bounce and the finisher carry their respective stance values.
            Self::enhanced_hit(
                battle,
                actor,
                target,
                multiplier,
                10 * u16::from(progress.shots == 0),
            );
            progress.shots += 1;
        }
        let targets: Vec<_> = battle
            .enemies()
            .filter(|&target| battle.is_targetable(target))
            .collect();
        let complete = progress.shots == BOUNCES && !targets.is_empty();
        if complete {
            for (index, &target) in targets.iter().enumerate() {
                let multiplier = split(self.finisher_bp, targets.len() as u16, index as u16);
                Self::enhanced_hit(battle, actor, target, multiplier, 10);
            }
        } else {
            if let Some(godmode) = &mut state_mut(battle, actor).godmode {
                godmode.continuation = Some(progress);
            }
            extend_buffs(battle, actor);
            battle.queue_extra_turn(actor);
        }
        let state = state_mut(battle, actor);
        state.enhanced_in_progress = false;
        state.enhanced_completed = complete;
    }

    fn resolve_pending_boxes(&self, battle: &mut Battle<'_>, actor: usize) {
        while battle.enemies().any(|target| battle.is_targetable(target)) {
            let Some(kind) = state_mut(battle, actor).pending_boxes.pop_front() else {
                break;
            };
            let parent = battle.begin_debuff_action();
            battle.begin_attack();
            self.loot_box(battle, actor, kind, Action::Ultimate);
            battle.notify_attack(actor, Action::Ultimate, None);
            battle.finish_debuff_action(actor, parent);
            battle.resolve_follow_ups();
        }
    }
}

impl Mechanics for SilverWolf999 {
    fn elation_participant(&self) -> Option<u16> {
        Some(136)
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        Some(state(battle, actor).rating >= RATING_THRESHOLD)
    }

    fn can_use_ultimate(&self, battle: &Battle<'_>, actor: usize) -> bool {
        state(battle, actor).godmode.is_none()
    }

    fn activate_special_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        let needed = RATING_THRESHOLD.saturating_sub(state(battle, actor).rating);
        self.gain_rating(battle, actor, needed);
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        state(battle, actor).godmode.is_none()
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && state(battle, actor).godmode.is_some())
            .then_some(&self.enhanced)
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, actor: usize) -> bool {
        state(battle, actor).godmode.is_some()
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            battle.add_punchline(actor, self.skill_punchline);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Ultimate => self.enter_godmode(battle, actor),
            Action::Basic if state(battle, actor).godmode.is_some() => {
                self.enhanced_basic(battle, actor);
            }
            Action::Basic | Action::Skill => {
                let Some(punchline) = battle.certified_banger(actor) else {
                    return;
                };
                for enemy in battle.enemies() {
                    if target.is_none_or(|selected| selected == enemy)
                        && battle.is_targetable(enemy)
                    {
                        Self::hit_elation(
                            battle,
                            actor,
                            enemy,
                            self.talent_damage_bp,
                            0,
                            action,
                            AhaInstant {
                                punchline,
                                skill_damage_factor_bp: FULL_CHANCE_BP as u16,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn on_punchline_gained(&self, battle: &mut Battle<'_>, actor: usize, points: u32) {
        self.gain_rating(battle, actor, points);
    }

    fn on_certified_banger_gained(&self, battle: &mut Battle<'_>, actor: usize, points: u32) {
        if let Some(ascension) = battle.elation_ascension() {
            let ratio = if ascension.innate_level >= 2 {
                4_000
            } else {
                2_000
            };
            self.gain_rating(battle, actor, percentage_of(points, ratio));
        }
    }

    fn on_ally_shared_skill_points_spent(&self, battle: &mut Battle<'_>, owner: usize, points: u8) {
        if battle.certified_banger(owner).is_none() {
            return;
        }
        for _ in 0..points {
            let Some(chance) = state(battle, owner)
                .godmode
                .as_ref()
                .map(|godmode| godmode.box_chance_bp)
            else {
                return;
            };
            if battle.roll_fixed(chance) {
                if let Some(godmode) = &mut state_mut(battle, owner).godmode {
                    godmode.box_chance_bp /= 5;
                }
                let kind = select_loot_box(battle);
                state_mut(battle, owner).pending_boxes.push_back(kind);
            }
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        self.resolve_pending_boxes(battle, owner);
        if actor == owner && action == Action::Basic {
            self.finish_enhanced(battle, owner);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon >= 6 {
            for target in battle.enemies() {
                battle.apply_permanent_debuff(owner, target, DebuffKind::RestrictedWeakness, 2_000);
            }
        }
        self.resolve_pending_boxes(battle, owner);
    }

    fn on_elation_skill(&self, battle: &mut Battle<'_>, actor: usize, mut instant: AhaInstant) {
        if self.elation_trace {
            self.gain_rating(
                battle,
                actor,
                20 * (u32::from(instant.punchline >= 20) + u32::from(instant.punchline >= 40)),
            );
        }
        if let Some(godmode) = &mut state_mut(battle, actor).godmode {
            godmode.box_chance_bp = FULL_CHANCE_BP;
            if self.eidolon >= 4 {
                instant.punchline = instant.punchline.saturating_mul(6);
            }
            for index in 0..6 {
                let Some(target) = battle.random_living_enemy() else {
                    break;
                };
                Self::hit_elation(
                    battle,
                    actor,
                    target,
                    self.enhanced_elation_damage_bp,
                    10 * u16::from(index == 0),
                    Action::ElationSkill,
                    instant,
                );
            }
        } else {
            self.gain_rating(battle, actor, self.elation_rating);
        }
    }

    fn personal_elation_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let speed = (battle.speed_micros(actor) / 1_000_000) as u32;
        if self.speed_trace && speed >= 160 {
            5_000 + (speed - 160).min(100) * 200
        } else {
            0
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        state(battle, actor).rating * self.crit_rate_per_rating_bp
    }

    fn crit_rate_overflow_damage_bp(&self, battle: &Battle<'_>, actor: usize, total: u32) -> u32 {
        let rating = state(battle, actor).rating;
        let overflow = total.saturating_sub(FULL_CHANCE_BP);
        let excess_points = (overflow / self.crit_rate_per_rating_bp).min(rating);
        excess_points * self.crit_damage_per_rating_bp
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        if battle.elation_ascension().is_some() {
            4_000
        } else {
            0
        }
    }

    fn damage_target_aura(&self, battle: &Battle<'_>, owner: usize, _: usize) -> DamageTargetAura {
        DamageTargetAura {
            vulnerability_bp: 2_000
                * u32::from(self.eidolon >= 1 && state(battle, owner).godmode.is_some()),
            ..DamageTargetAura::default()
        }
    }

    fn action_damage_factor_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u16 {
        if action == Some(Action::Basic)
            && state(battle, actor).enhanced_in_progress
            && matches!(kind, DamageKind::Direct | DamageKind::Elation)
        {
            10_000 + 1_500 * (state(battle, actor).rating / 60).min(2) as u16
        } else {
            10_000
        }
    }

    fn ally_merrymake_bp(&self, battle: &Battle<'_>, source: usize, actor: usize) -> u32 {
        5_000
            * u32::from(
                self.eidolon >= 6 && source == actor && state(battle, source).enhanced_in_progress,
            )
    }

    fn dpr_priority(
        &self,
        _: &Battle<'_>,
        _: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Ultimate).then_some(0)
    }
}
