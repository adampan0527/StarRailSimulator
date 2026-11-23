use std::sync::Arc;

use crate::{
    Action, BattleError, Element, EventKind, StatusKind,
    fixture::{Effect, Stats, Target},
};

use super::{
    super::{Battle, resolve::DamageTargetAura, support::percentage_of},
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
pub(super) mod tests;

pub(super) const FORM: &str = "character.1415";
const DEMIURGE: &str = "demiurge";
const RECOLLECTION_CAP: u8 = 27;
const FIRST_ULTIMATE: u8 = 24;
const ULTIMATE_COST: u8 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Heir {
    Genesis,
    Romance,
    Passage,
    Strife,
    Death,
    Reason,
    Sky,
    Trickery,
    Worldbearing,
    Ocean,
    Law,
    Time,
    Earth,
}

impl Heir {
    fn for_form(form: &str) -> Option<Self> {
        Some(match form {
            "trailblazer.remembrance" => Self::Genesis,
            "character.1402" => Self::Romance,
            "character.1403" => Self::Passage,
            "character.1404" => Self::Strife,
            "character.1407" => Self::Death,
            "character.1405" => Self::Reason,
            "character.1409" => Self::Sky,
            "character.1406" => Self::Trickery,
            "character.1408" => Self::Worldbearing,
            "character.1410" => Self::Ocean,
            "character.1412" => Self::Law,
            "character.1413" => Self::Time,
            "character.1414" => Self::Earth,
            _ => return None,
        })
    }
}

#[derive(Clone, Default)]
pub(in crate::battle) struct CyreneState {
    recollection: u8,
    transformed: bool,
    zone_until: u32,
    future: Vec<usize>,
    contributors: Vec<usize>,
    blessed: Vec<usize>,
    odes: [Option<AppliedOde>; 4],
    story: u8,
    free_minuets: u8,
    minuet_count: u8,
    free_casts: u8,
    genesis_pending: bool,
}

#[derive(Clone, Copy)]
struct AppliedOde {
    heir: Option<Heir>,
    expires: u32,
}

#[derive(Clone, Copy)]
pub(super) struct TimeOde {
    pub memoria: u32,
    pub crit_ratio_bp: u32,
}

struct Cyrene {
    eidolon: u8,
    lasting_future: bool,
    opening_recollection: bool,
    causality: bool,
    true_bp: u32,
    damage_bp: u32,
    transformation_crit_bp: u32,
    pair_hp_bp: u32,
    enhanced_main_bp: u16,
    enhanced_area_bp: u16,
    minuet_bp: u32,
    bounce_bp: u32,
    generic_ode_bp: u32,
    genesis: [u32; 2],
    romance: [u32; 3],
    passage: [u32; 2],
    strife: [u32; 2],
    death: [u32; 3],
    reason: [u32; 4],
    sky: [u32; 2],
    trickery: [u32; 3],
    worldbearing: [u32; 8],
    ocean: [u32; 4],
    law: [u32; 2],
    time: [u32; 3],
    earth: [u32; 4],
    enhanced_basic: OfficialAbility,
    directed_ultimate: OfficialAbility,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 0
        || input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != Element::Ice
        || input.eidolon > 6
    {
        return Err(invalid("resource, combat type or eidolon"));
    }
    let basic = input.parameters("skill.141501", 2)?;
    let skill = input.parameters("skill.141502", 3)?;
    let ultimate = input.parameters("skill.141503", 4)?;
    let talent = input.parameters("skill.141504", 5)?;
    let enhanced = input.parameters("skill.141508", 3)?;
    let minuet = input.parameters("skill.1141501", 2)?;
    let ode = input.parameters("skill.1141502", 3)?;
    let sprite = input.parameters("skill.1141503", 2)?;
    let story = input.parameters("skill.1141526", 2)?;
    if basic[1] != 1.0
        || skill[1..] != [2.0, 3.0]
        || ultimate[0..2] != [1.0, 24.0]
        || ultimate[3] != 12.0
        || talent[0] != 1.0
        || talent[2..] != [27.0, 24.0, 12.0]
        || enhanced[1] != 3.0
        || minuet[1] != 12.0
        || ode[0] != 12.0
        || ode[2] != 2.0
        || sprite[1] != 0.0
        || story[1] != 3.0
        || input.parameters("skill.141514", 1)? != [12.0]
    {
        return Err(invalid("unexpected frozen resource conditions"));
    }
    let genesis = input.parameters("skill.1141513", 3)?;
    let romance = input.parameters("skill.1141514", 3)?;
    let passage = input.parameters("skill.1141515", 3)?;
    let strife = input.parameters("skill.1141516", 2)?;
    let death = input.parameters("skill.1141517", 6)?;
    let reason = input.parameters("skill.1141518", 4)?;
    let sky = input.parameters("skill.1141519", 2)?;
    let trickery = input.parameters("skill.1141520", 3)?;
    let worldbearing = input.parameters("skill.1141521", 9)?;
    let ocean = input.parameters("skill.1141522", 4)?;
    let law = input.parameters("skill.1141523", 2)?;
    let time = input.parameters("skill.1141524", 3)?;
    let earth = input.parameters("skill.1141525", 5)?;
    if romance[0] != 70.0
        || genesis[2] != 1.0
        || passage[0] != 1.0
        || death[2] != 2.0
        || death[5] != 2.0
        || reason[0] != 3.0
        || reason[3] != 1.0
        || worldbearing[2] != 5.0
        || worldbearing[4] != 12.0
        || worldbearing[7..] != [6.0, 4.0]
        || law[1] != 1.0
        || time[1] != 1.0
        || earth[1..3] != [2.0, 3.0]
        || earth[4] != 1.5
    {
        return Err(invalid("unexpected frozen ode conditions"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|entry| entry == id);
    let support = |target| OfficialAbility {
        target,
        energy_milli: 0,
        effects: Vec::new(),
        skill_points: SkillPoints::Unchanged,
    };
    Ok(OfficialKit {
        max_energy_milli: 0,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![Effect::MaxHpHit {
                element: Element::Ice,
                multiplier_bp: bp(basic[0])? as u16,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            skill_points: SkillPoints::Spend,
            ..support(Target::SelfOnly)
        },
        ultimate: support(Target::SelfOnly),
        mechanics: Arc::new(Cyrene {
            eidolon: input.eidolon,
            lasting_future: trace("trace.1415101"),
            opening_recollection: trace("trace.1415102"),
            causality: trace("trace.1415103"),
            true_bp: bp(skill[0])?,
            damage_bp: bp(talent[1])?,
            transformation_crit_bp: bp(ultimate[2])?,
            pair_hp_bp: bp(sprite[0])?,
            enhanced_main_bp: bp(enhanced[2])? as u16,
            enhanced_area_bp: bp(enhanced[0])? as u16,
            minuet_bp: bp(minuet[0])?,
            bounce_bp: bp(story[0])?,
            generic_ode_bp: bp(ode[1])?,
            genesis: [bp(genesis[0])?, bp(genesis[1])?],
            romance: [70_000, bp(romance[1])?, bp(romance[2])?],
            passage: [1, bp(passage[1])?],
            strife: [bp(strife[0])?, bp(strife[1])?],
            death: [bp(death[1])?, bp(death[4])?, 2],
            reason: [3, bp(reason[1])?, bp(reason[2])?, 1],
            sky: [bp(sky[0])?, (sky[1] * 1_000.0).round() as u32],
            trickery: [bp(trickery[0])?, bp(trickery[1])?, bp(trickery[2])?],
            worldbearing: [
                bp(worldbearing[0])?,
                bp(worldbearing[1])?,
                5,
                bp(worldbearing[3])?,
                bp(worldbearing[5])?,
                bp(worldbearing[6])?,
                6,
                4,
            ],
            ocean: [bp(ocean[0])?, bp(ocean[1])?, bp(ocean[2])?, 60_000],
            law: [bp(law[0])?, 1],
            time: [bp(time[0])?, 1, bp(time[2])?],
            earth: [bp(earth[0])?, 3, bp(earth[3])?, 15_000],
            enhanced_basic: OfficialAbility {
                effects: vec![Effect::DirectedAreaAttack],
                ..support(Target::Enemy)
            },
            directed_ultimate: support(Target::OtherAlly),
        }),
    })
}

fn invalid(reason: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {reason}"))
}

fn bp(value: f64) -> Result<u32, BattleError> {
    let scaled = (value * 10_000.0).round();
    if (1.0..=u16::MAX as f64).contains(&scaled) {
        Ok(scaled as u32)
    } else {
        Err(invalid("skill ratio"))
    }
}

fn heir(battle: &Battle<'_>, target: usize) -> Option<Heir> {
    Heir::for_form(battle.plan.units[target].official_form.as_deref()?)
}

fn zone_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    unit.hp > 0
        && (unit.cyrene_state.transformed || unit.normal_turns < unit.cyrene_state.zone_until)
}

pub(super) fn set_recollection(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let after = after.min(RECOLLECTION_CAP);
    let before = std::mem::replace(&mut battle.units[owner].cyrene_state.recollection, after);
    if before != after {
        battle.emit(EventKind::Recollection { before, after }, Some(owner), None);
    }
    if before < FIRST_ULTIMATE
        && after >= FIRST_ULTIMATE
        && !battle.units[owner].cyrene_state.transformed
    {
        battle.cleanse_crowd_control(owner, owner);
        while battle.cleanse_one(owner, owner) {}
    }
}

fn gain_recollection(battle: &mut Battle<'_>, owner: usize, amount: u8) {
    set_recollection(
        battle,
        owner,
        battle.units[owner]
            .cyrene_state
            .recollection
            .saturating_add(amount),
    );
}

fn ode_active(battle: &Battle<'_>, owner: usize, target: usize, heir: Option<Heir>) -> bool {
    battle.units[owner]
        .cyrene_state
        .odes
        .get(target)
        .copied()
        .flatten()
        .is_some_and(|ode| ode.heir == heir && battle.units[target].normal_turns < ode.expires)
}

impl Cyrene {
    fn refresh_future(battle: &mut Battle<'_>, owner: usize) {
        let future: Vec<_> = battle
            .party_units()
            .filter(|&unit| unit != owner && battle.memosprite_owner(unit) != Some(owner))
            .collect();
        battle.units[owner].cyrene_state.future = future;
    }

    fn preferred_ode_target(&self, battle: &Battle<'_>, owner: usize) -> Option<usize> {
        battle
            .allies()
            .filter(|&target| {
                target != owner
                    && battle.is_targetable(target)
                    && !battle.units[owner].cyrene_state.blessed.contains(&target)
            })
            .max_by_key(|&target| {
                let priority = match heir(battle, target) {
                    Some(Heir::Time | Heir::Death | Heir::Worldbearing) => 3,
                    Some(_) => 2,
                    None => 1,
                };
                (
                    priority,
                    battle.plan.units[target].stats.attack,
                    std::cmp::Reverse(target),
                )
            })
    }

    fn initial_ultimate(&self, battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        let sprite = battle.memosprite(owner)?;
        let before_hp = battle.effective_max_hp(owner);
        let before_current = battle.units[owner].hp;
        battle.units[owner].cyrene_state.transformed = true;
        {
            let unit = &mut battle.units[sprite];
            unit.memosprite_max_hp = Some(before_hp);
            unit.memosprite_generation += 1;
            unit.hp = before_current;
        }
        self.sync_sprite_health(battle, owner, sprite);
        let max_hp = battle.effective_max_hp(owner);
        battle.units[owner].hp = battle.units[owner]
            .hp
            .saturating_add(max_hp.saturating_sub(before_hp))
            .min(max_hp);
        self.sync_sprite_health(battle, owner, sprite);
        battle.emit(
            EventKind::StatusApplied(StatusKind::Ripples),
            Some(owner),
            Some(owner),
        );
        for target in battle.party_units().filter(|&target| target != sprite) {
            if !battle.is_targetable(target) {
                continue;
            }
            battle.cleanse_crowd_control(sprite, target);
            if target != owner && battle.allies().contains(&target) {
                let cap = battle.plan.units[target].kit.max_energy_milli();
                if cap > 0 {
                    battle.set_energy(owner, target, cap);
                } else if let Some(rules) = battle.mechanics(target) {
                    rules.activate_special_ultimate(battle, target);
                }
            }
            if self.eidolon >= 6 {
                battle.advance_ally(owner, target, 10_000);
            }
        }
        self.add_story(battle, owner);
        Some(sprite)
    }

    fn sync_sprite_health(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        let hp = battle.units[owner].hp;
        let maximum = battle.effective_max_hp(owner);
        let new_hp =
            u64::from(battle.effective_max_hp(sprite)) * u64::from(hp) / u64::from(maximum.max(1));
        battle.units[sprite].hp = (new_hp as u32).max(1);
    }

    fn apply_ode(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize, target: usize) {
        let target_heir = heir(battle, target);
        let turn = battle.units[target].normal_turns;
        let state = &mut battle.units[owner].cyrene_state;
        if !state.blessed.contains(&target) {
            state.blessed.push(target);
        }
        state.odes[target] = Some(AppliedOde {
            heir: target_heir,
            expires: if target_heir.is_some() {
                u32::MAX
            } else {
                turn + 2
            },
        });
        battle.emit(EventKind::TriggeredSkill, Some(sprite), Some(target));
        if let Some(heir) = target_heir {
            self.apply_heir_ode(battle, owner, target, heir);
        }
    }

    fn add_story(&self, battle: &mut Battle<'_>, owner: usize) {
        let state = &mut battle.units[owner].cyrene_state;
        state.story += 1;
        if state.story == 3 {
            state.story = 0;
            state.free_minuets += 1;
        }
    }

    fn minuet(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize, free_story: bool) {
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        for enemy in battle.enemies() {
            battle.memosprite_hp_hit(
                sprite,
                enemy,
                Element::Ice,
                self.minuet_bp,
                10,
                Action::Skill,
            );
        }
        let state = &battle.units[owner].cyrene_state;
        let count = state.contributors.len()
            + if free_story && self.eidolon >= 1 {
                12
            } else {
                0
            };
        let bounce_bp = self.bounce_bp + 600 * u32::from(state.minuet_count);
        for _ in 0..count {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.memosprite_hp_hit(sprite, target, Element::Ice, bounce_bp, 0, Action::Skill);
        }
        battle.notify_attack(sprite, Action::Skill, None);
        if self.eidolon >= 4 {
            let count = &mut battle.units[owner].cyrene_state.minuet_count;
            *count = (*count + 1).min(24);
        }
        if free_story {
            if self.eidolon >= 1 {
                gain_recollection(battle, owner, 6);
            }
            let count = &mut battle.units[owner].cyrene_state.free_casts;
            *count = count.saturating_add(1);
            if self.eidolon >= 6 && *count >= 2 {
                for ally in battle
                    .party_units()
                    .filter(|&ally| battle.is_targetable(ally))
                    .collect::<Vec<_>>()
                {
                    battle.advance_ally(owner, ally, 2_400);
                }
            }
        }
    }

    fn sprite_turn(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        sprite: usize,
        target: Option<usize>,
        story: bool,
    ) {
        if !battle.is_targetable(sprite) {
            return;
        }
        battle.units[sprite].normal_turns += 1;
        battle.emit(EventKind::TurnStarted, Some(sprite), None);
        battle.notify_active_ability(sprite);
        self.on_any_ode(battle, owner);
        if let Some(target) = target.filter(|&target| battle.is_targetable(target)) {
            self.apply_ode(battle, owner, sprite, target);
        } else {
            self.minuet(battle, owner, sprite, story);
        }
        battle.notify_memosprite_turn_finished(sprite);
        battle.finish_turn(sprite);
    }

    fn use_ultimate(&self, battle: &mut Battle<'_>, owner: usize, target: Option<usize>) {
        let points = battle.units[owner].cyrene_state.recollection;
        set_recollection(battle, owner, points - ULTIMATE_COST);
        let sprite = if battle.units[owner].cyrene_state.transformed {
            battle.active_memosprite(owner)
        } else {
            self.initial_ultimate(battle, owner)
        };
        let Some(sprite) = sprite else {
            return;
        };
        self.add_story(battle, owner);
        self.sprite_turn(battle, owner, sprite, target, false);
        let free = std::mem::take(&mut battle.units[owner].cyrene_state.free_minuets);
        for _ in 0..free {
            self.sprite_turn(battle, owner, sprite, None, true);
        }
    }

    fn penetration(&self, battle: &Battle<'_>, owner: usize, element: Element) -> u32 {
        if self.causality && element == Element::Ice {
            200 * (battle.speed_micros(owner) / 1_000_000)
                .saturating_sub(180)
                .min(60) as u32
        } else {
            0
        }
    }
}

impl Mechanics for Cyrene {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        Some(MemospriteProfile {
            name: DEMIURGE,
            stats: Stats {
                base_speed_micros: 0,
                flat_speed_micros: 0,
                base_aggro: 100,
                ..master
            },
        })
    }
    fn memosprite_has_fixed_zero_speed(&self) -> bool {
        true
    }
    fn memosprite_is_backup(&self) -> bool {
        true
    }
    fn tracks_active_abilities(&self) -> bool {
        true
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        Self::refresh_future(battle, owner);
        let army = battle
            .allies()
            .filter(|&ally| {
                ally != owner
                    && (heir(battle, ally).is_some()
                        || battle.plan.units[ally]
                            .combat_type
                            .is_some_and(|kind| kind.path == CombatPath::Remembrance))
            })
            .count();
        let trace = [0, 2, 3, 6][army.min(3)] * u8::from(self.opening_recollection);
        gain_recollection(battle, owner, trace + 12 * u8::from(self.eidolon >= 2));
    }

    fn on_active_ability(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if actor == owner
            || !battle.is_party_unit(actor)
            || battle.memosprite_owner(actor) == Some(owner)
        {
            return;
        }
        let sprite = battle.memosprite_owner(actor).is_some();
        let state = &mut battle.units[owner].cyrene_state;
        let indexed = state.future.iter().position(|&unit| unit == actor);
        if !(sprite && self.lasting_future) {
            let Some(index) = indexed else {
                return;
            };
            state.future.swap_remove(index);
        }
        if !state.contributors.contains(&actor) {
            state.contributors.push(actor);
        }
        gain_recollection(battle, owner, 1);
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !battle.units[actor].cyrene_state.transformed
    }

    fn alternate_ultimate_resource_ready(&self, battle: &Battle<'_>, actor: usize) -> Option<bool> {
        let state = &battle.units[actor].cyrene_state;
        Some(if state.transformed {
            state.recollection >= ULTIMATE_COST && battle.active_memosprite(actor).is_some()
        } else {
            state.recollection >= FIRST_ULTIMATE
        })
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        match action {
            Action::Basic if battle.units[actor].cyrene_state.transformed => {
                Some(&self.enhanced_basic)
            }
            Action::EnhancedUltimate => Some(&self.directed_ultimate),
            _ => None,
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        let gain = match action {
            Action::Basic if !battle.units[owner].cyrene_state.transformed => 1,
            Action::Basic | Action::Skill => 3,
            _ => 0,
        };
        if gain > 0 {
            gain_recollection(battle, owner, gain);
        }
        if action == Action::Skill {
            let unit = &mut battle.units[owner];
            unit.cyrene_state.zone_until = unit.normal_turns + 2;
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action.is_ultimate() {
            self.use_ultimate(battle, owner, target);
        }
        Self::refresh_future(battle, owner);
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        battle.attack_hit_with_max_hp(
            owner,
            target,
            Element::Ice,
            (0, self.enhanced_main_bp),
            0,
            10,
        );
        for enemy in battle.enemies() {
            battle.attack_hit_with_max_hp(
                owner,
                enemy,
                Element::Ice,
                (0, self.enhanced_area_bp),
                0,
                5,
            );
        }
    }

    fn memosprite_true_damage_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        if !zone_active(battle, owner) {
            return 0;
        }
        self.true_bp
            + if self.eidolon >= 2 {
                600 * battle.units[owner].cyrene_state.blessed.len().min(4) as u32
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
        if !battle.is_targetable(owner) {
            return 0;
        }
        let speed_bonus = if self.causality && battle.speed_micros(owner) >= 180_000_000 {
            2_000
        } else {
            0
        };
        let earth = u32::from(super::dan_heng_permansor::cyrene_bondmate(
            battle, owner, target,
        )) * self.earth[0];
        self.damage_bp + speed_bonus + earth + self.ode_damage(battle, owner, target)
    }

    fn ally_skill_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if !battle.plan.units[target]
            .combat_type
            .is_some_and(|kind| kind.path == CombatPath::Erudition)
        {
            return 0;
        }
        super::anaxa::cyrene_knowledge(battle, owner).map_or(0, |(_, skill_bp)| skill_bp)
    }

    fn ally_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if target == owner
            && battle.units[owner].cyrene_state.transformed
            && battle.active_memosprite(owner).is_some()
        {
            percentage_of(battle.plan.units[owner].stats.base_hp, self.pair_hp_bp)
        } else {
            0
        }
    }

    fn memosprite_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if battle.memosprite_owner(target) == Some(owner)
            && battle.units[owner].cyrene_state.transformed
        {
            percentage_of(battle.plan.units[target].stats.base_hp, self.pair_hp_bp)
        } else {
            0
        }
    }

    fn ally_crit_rate_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if target == owner || battle.memosprite_owner(target) == Some(owner) {
            return self.transformation_crit_bp
                * u32::from(battle.units[owner].cyrene_state.transformed);
        }
        self.ode_crit_rate(battle, owner, target)
    }

    fn ally_attack_bonus_flat(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let knowledge = if battle.plan.units[target]
            .combat_type
            .is_some_and(|kind| kind.path == CombatPath::Erudition)
        {
            super::anaxa::cyrene_knowledge(battle, owner).map_or(0, |(attack_bp, _)| {
                percentage_of(battle.plan.units[target].stats.base_attack, attack_bp)
            })
        } else {
            0
        };
        knowledge + self.ode_attack(battle, owner, target)
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        self.ode_crit_damage(battle, owner, target)
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        let global = self.eidolon >= 6
            && battle.units[owner].cyrene_state.free_casts > 0
            && battle.active_memosprite(owner).is_some();
        DamageTargetAura {
            defense_down_bp: 2_000 * u32::from(global) + self.ode_defense(battle, owner, actor),
            ..DamageTargetAura::default()
        }
    }

    fn target_defense_down_bp(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> u32 {
        battle
            .allies()
            .find(|&target| ode_active(battle, owner, target, Some(Heir::Trickery)))
            .map_or(0, |target| {
                self.trickery[if super::cipher::is_patron(battle, target, enemy) {
                    1
                } else {
                    2
                }]
            })
    }

    fn cyrene_ode_parameters(&self, form: &str) -> Option<[u32; 8]> {
        let source: &[u32] = match Heir::for_form(form)? {
            Heir::Genesis => &self.genesis,
            Heir::Romance => &self.romance,
            Heir::Passage => &self.passage,
            Heir::Strife => &self.strife,
            Heir::Death => &self.death,
            Heir::Reason => &self.reason,
            Heir::Sky => &self.sky,
            Heir::Trickery => &self.trickery,
            Heir::Worldbearing => &self.worldbearing,
            Heir::Ocean => &self.ocean,
            Heir::Law => &self.law,
            Heir::Time => &self.time,
            Heir::Earth => &self.earth,
        };
        let mut values = [0; 8];
        values[..source.len()].copy_from_slice(source);
        Some(values)
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        self.penetration(battle, owner, element)
    }
    fn memosprite_resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        element: Element,
    ) -> u32 {
        self.penetration(battle, owner, element)
    }

    fn tracks_action_end_reactions(&self) -> bool {
        true
    }

    fn react_to_action_end(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if let Some(sprite) = battle.active_memosprite(owner) {
            self.sync_sprite_health(battle, owner, sprite);
            if actor != sprite
                && std::mem::take(&mut battle.units[owner].cyrene_state.genesis_pending)
            {
                self.sprite_turn(battle, owner, sprite, None, false);
                battle.resolve_follow_ups();
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
        if action == Action::Basic
            && ode_active(battle, owner, actor, Some(Heir::Genesis))
            && super::trailblazer_remembrance::take_cyrene_joint_basic(battle, actor)
        {
            battle.units[owner].cyrene_state.genesis_pending = true;
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::EnhancedUltimate if target == self.preferred_ode_target(battle, owner) => {
                Some(0)
            }
            Action::Ultimate if self.preferred_ode_target(battle, owner).is_none() => Some(0),
            Action::Ultimate | Action::EnhancedUltimate => Some(9),
            Action::Skill if !zone_active(battle, owner) => Some(2),
            Action::Skill => Some(4),
            _ => None,
        }
    }
}

impl Cyrene {
    fn ode_damage(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let ally = battle.memosprite_owner(target).unwrap_or(target);
        let kind = heir(battle, ally);
        if !ode_active(battle, owner, ally, kind) {
            return 0;
        }
        match kind {
            None => self.generic_ode_bp,
            Some(Heir::Romance) if super::aglaea::cyrene_romance_active(battle, ally) => {
                self.romance[1]
            }
            Some(Heir::Trickery) if target == ally => self.trickery[0],
            Some(Heir::Ocean) if target == ally => self.ocean[0],
            Some(Heir::Time)
                if target != ally && battle.current_action() != Some(Action::Ultimate) =>
            {
                self.time[0]
            }
            _ => 0,
        }
    }

    fn ode_attack(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let ally = battle.memosprite_owner(target).unwrap_or(target);
        if ode_active(battle, owner, ally, Some(Heir::Genesis)) {
            return battle.active_memosprite(owner).map_or(0, |sprite| {
                percentage_of(battle.effective_max_hp(sprite), self.genesis[0])
            });
        }
        0
    }

    fn ode_crit_rate(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        let ally = battle.memosprite_owner(target).unwrap_or(target);
        if ode_active(battle, owner, ally, Some(Heir::Genesis)) {
            return battle.active_memosprite(owner).map_or(0, |sprite| {
                let rate =
                    battle.plan.units[sprite].stats.crit_rate_bp + self.transformation_crit_bp;
                percentage_of(rate.min(10_000), self.genesis[1])
            });
        }
        0
    }

    fn ode_crit_damage(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if battle.allies().any(|ally| {
            ode_active(battle, owner, ally, Some(Heir::Law))
                && super::cerydra::holder(battle, ally) == Some(target)
        }) {
            return self.law[0];
        }
        0
    }

    fn ode_defense(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u32 {
        let ally = battle.memosprite_owner(actor).unwrap_or(actor);
        if ode_active(battle, owner, ally, Some(Heir::Passage)) {
            self.passage[1]
        } else if ode_active(battle, owner, ally, Some(Heir::Romance))
            && super::aglaea::cyrene_romance_active(battle, ally)
        {
            self.romance[2]
        } else {
            0
        }
    }

    fn apply_heir_ode(&self, battle: &mut Battle<'_>, owner: usize, target: usize, heir: Heir) {
        match heir {
            Heir::Romance => super::aglaea::apply_cyrene_romance(battle, target, self.romance[0]),
            Heir::Strife => {
                battle.cleanse_crowd_control(owner, target);
                if let Some(rules) = battle.mechanics(target) {
                    rules.cyrene_ode_applied(battle, owner, target, [self.strife[0], 0, 0, 0]);
                }
            }
            Heir::Reason => {
                battle.restore_skill_points(owner, self.reason[3] as u8);
                battle.advance_ally_immediate(owner, target);
                super::anaxa::apply_cyrene_reason(battle, owner, target, self.reason);
            }
            Heir::Sky => battle.gain_energy(owner, target, self.sky[1]),
            Heir::Worldbearing => {
                if let Some(rules) = battle.mechanics(target) {
                    rules.cyrene_ode_applied(
                        battle,
                        owner,
                        target,
                        [self.worldbearing[6], 0, 0, 0],
                    );
                }
            }
            Heir::Ocean => super::hysilens::apply_cyrene_ocean(battle, target, self.ocean[3]),
            Heir::Earth => super::dan_heng_permansor::activate_cyrene_dragon(
                battle, owner, target, self.earth, true,
            ),
            _ => {}
        }
    }

    fn on_any_ode(&self, battle: &mut Battle<'_>, owner: usize) {
        if let Some(target) = battle
            .allies()
            .find(|&ally| heir(battle, ally) == Some(Heir::Sky))
        {
            super::hyacine::apply_cyrene_sky(battle, target, self.sky[0]);
        }
        if let Some(target) = battle
            .allies()
            .find(|&ally| heir(battle, ally) == Some(Heir::Earth))
        {
            super::dan_heng_permansor::activate_cyrene_dragon(
                battle, owner, target, self.earth, false,
            );
        }
    }
}

pub(super) fn active_ode_parameters(
    battle: &Battle<'_>,
    target: usize,
    form: &str,
) -> Option<(usize, [u32; 8])> {
    let family = Heir::for_form(form)?;
    battle.allies().find_map(|owner| {
        if !ode_active(battle, owner, target, Some(family)) {
            return None;
        }
        let rules = battle.mechanics(owner)?;
        Some((owner, rules.cyrene_ode_parameters(form)?))
    })
}

pub(super) fn time_ode(battle: &Battle<'_>, target: usize) -> Option<TimeOde> {
    active_ode_parameters(battle, target, super::evernight::FORM).map(|(_, values)| TimeOde {
        memoria: values[1],
        crit_ratio_bp: values[2],
    })
}
