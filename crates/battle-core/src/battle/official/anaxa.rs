use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
pub(super) mod tests;

pub(super) const FORM: &str = "character.1405";
const BASIC: &str = "skill.140501";
const SKILL: &str = "skill.140502";
const ULTIMATE: &str = "skill.140503";
const TALENT: &str = "skill.140504";
const ROAMING_SIGNIFIER: &str = "trace.1405101";
const IMPERATIVE_HIATUS: &str = "trace.1405102";
const QUALITATIVE_SHIFT: &str = "trace.1405103";
const E4_ATTACK: &str = "eidolon.140504";

const WIND: Element = Element::Wind;
const ENERGY_CAPACITY: u32 = 140;
const SKILL_BOUNCES: u8 = 4;
const SKILL_HIT_ENERGY_MILLI: u32 = 6_000;
const DISCLOSURE_WEAKNESSES: u32 = 5;
const IMPLANT_TURNS: u32 = 3;

#[derive(Clone, Copy, Default)]
pub(in crate::battle) struct AnaxaState {
    first_skill_used: bool,
    cyrene_reason: Option<CyreneReason>,
    skill_damage_bonus_bp: u32,
}

#[derive(Clone, Copy)]
struct CyreneReason {
    source: usize,
    extra_hits: u8,
    skill_damage_bp: u32,
    attack_bp: u32,
    expires: u32,
    knowledge: bool,
}

pub(super) fn apply_cyrene_reason(
    battle: &mut Battle<'_>,
    source: usize,
    actor: usize,
    values: [u32; 4],
) {
    battle.units[actor].anaxa_state.cyrene_reason = Some(CyreneReason {
        source,
        extra_hits: values[0] as u8,
        skill_damage_bp: values[1],
        attack_bp: values[2],
        expires: battle.units[actor].normal_turns + 2,
        knowledge: false,
    });
}

pub(super) fn cyrene_knowledge(battle: &Battle<'_>, source: usize) -> Option<(u32, u32)> {
    battle.allies().find_map(|actor| {
        battle.units[actor]
            .anaxa_state
            .cyrene_reason
            .filter(|reason| {
                reason.source == source
                    && reason.knowledge
                    && battle.units[actor].normal_turns < reason.expires
            })
            .map(|reason| (reason.attack_bp, reason.skill_damage_bp))
    })
}

struct Anaxa {
    skill: OfficialAbility,
    skill_hit_bp: u16,
    skill_bonus_per_enemy_bp: u32,
    disclosure_damage_bp: u32,
    roaming_signifier: bool,
    imperative_hiatus: bool,
    qualitative_shift: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != WIND
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 1)?;
    let talent = input.parameters(TALENT, 3)?;
    if skill[1] != f64::from(SKILL_BOUNCES) {
        return Err(invalid(SKILL));
    }
    if talent[1..] != [f64::from(IMPLANT_TURNS), f64::from(DISCLOSURE_WEAKNESSES)] {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            skill_points: SkillPoints::Generate,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
        },
        skill: skill_ability(SkillPoints::Spend),
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            skill_points: SkillPoints::Unchanged,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ULTIMATE, ultimate[0])?, 20)],
        },
        mechanics: Arc::new(Anaxa {
            skill: skill_ability(SkillPoints::Unchanged),
            skill_hit_bp: basis_points(SKILL, skill[0])?,
            skill_bonus_per_enemy_bp: u32::from(basis_points(SKILL, skill[2])?),
            disclosure_damage_bp: u32::from(basis_points(TALENT, talent[0])?),
            roaming_signifier: trace(ROAMING_SIGNIFIER),
            imperative_hiatus: trace(IMPERATIVE_HIATUS),
            qualitative_shift: trace(QUALITATIVE_SHIFT),
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
        element: WIND,
        multiplier_bp,
        toughness,
    }
}

fn skill_ability(skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        skill_points,
        // The frozen SPBase belongs to each bounce segment.
        energy_milli: 0,
        effects: vec![Effect::DirectedAreaAttack],
    }
}

fn targetable_enemies<'a>(battle: &'a Battle<'_>) -> impl Iterator<Item = usize> + 'a {
    battle
        .enemies()
        .filter(|&enemy| battle.is_targetable(enemy))
}

fn weakness_count(battle: &Battle<'_>, target: usize) -> u32 {
    Element::ALL
        .into_iter()
        .filter(|&element| battle.has_weakness(target, element))
        .count() as u32
}

fn disclosed(battle: &Battle<'_>, target: usize) -> bool {
    weakness_count(battle, target) >= DISCLOSURE_WEAKNESSES
}

fn implant_weakness(battle: &mut Battle<'_>, source: usize, target: usize) {
    if !battle.is_targetable(target) {
        return;
    }
    let mut missing = [WIND; Element::COUNT];
    let mut count = 0;
    for element in Element::ALL {
        if !battle.has_weakness(target, element) {
            missing[count] = element;
            count += 1;
        }
    }
    let candidates = if count == 0 {
        &Element::ALL[..]
    } else {
        &missing[..count]
    };
    let element = candidates[battle.random_index(candidates.len())];
    battle.apply_debuff(
        source,
        target,
        DebuffKind::AccumulatedWeakness(element),
        0,
        IMPLANT_TURNS,
    );
}

fn has_two_erudition(battle: &Battle<'_>) -> bool {
    battle
        .allies()
        .filter(|&actor| {
            battle.plan.units[actor]
                .combat_type
                .is_some_and(|kind| kind.path == CombatPath::Erudition)
        })
        .nth(1)
        .is_some()
}

impl Mechanics for Anaxa {
    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.eidolon < 2 {
            return;
        }
        for target in battle.enemies() {
            if !battle.is_targetable(target) {
                continue;
            }
            implant_weakness(battle, owner, target);
            battle.apply_permanent_debuff(owner, target, DebuffKind::AllResistanceDown, 2_000);
        }
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if self.roaming_signifier
            && owner == actor
            && !targetable_enemies(battle).any(|target| disclosed(battle, target))
        {
            battle.gain_energy(owner, owner, 30_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let turn = battle.units[actor].normal_turns;
        if matches!(action, Action::Basic | Action::Skill)
            && let Some(reason) = &mut battle.units[actor].anaxa_state.cyrene_reason
            && turn < reason.expires
            && !reason.knowledge
        {
            reason.knowledge = true;
            reason.expires = turn + 1;
        }
        match action {
            Action::Basic | Action::Skill => {
                if let Some(target) = target.filter(|&target| disclosed(battle, target)) {
                    battle.queue_triggered_skill(actor, target);
                }
            }
            Action::Ultimate => {
                for target in battle.enemies() {
                    if !battle.is_targetable(target) {
                        continue;
                    }
                    battle.apply_debuff(
                        actor,
                        target,
                        DebuffKind::Sublimation {
                            blocks_action: !battle.plan.units[target].has_control_resistance,
                        },
                        0,
                        1,
                    );
                }
            }
            _ => {}
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Basic && self.roaming_signifier {
            battle.gain_energy(actor, actor, 10_000);
        }
    }

    fn before_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
    ) {
        if self.eidolon >= 1 && action == Action::Skill {
            battle.apply_debuff(actor, target, DebuffKind::DefenseDown, 1_600, 2);
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _action: Action,
        _hp_before: u32,
    ) {
        implant_weakness(battle, actor, target);
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if self.eidolon >= 4 {
            let kind = AllyBuffKind::AttackPercent;
            let value = (battle.ally_buff_value(actor, actor, kind) + 3_000).min(6_000);
            battle.apply_ally_buff_with_id(actor, actor, kind, value, 2, Some(E4_ATTACK));
        }
        let enemy_count = targetable_enemies(battle).count() as u32;
        battle.units[actor].anaxa_state.skill_damage_bonus_bp =
            self.skill_bonus_per_enemy_bp.saturating_mul(enemy_count);

        let mut hit_targets = Vec::with_capacity(usize::from(SKILL_BOUNCES) + 1);
        let mut target = target;
        let extra = battle.units[actor]
            .anaxa_state
            .cyrene_reason
            .filter(|reason| battle.units[actor].normal_turns < reason.expires)
            .map_or(0, |reason| reason.extra_hits);
        let bounces = SKILL_BOUNCES + extra;
        for segment in 0..=bounces {
            let toughness = if segment == 0 { 10 } else { 5 };
            battle.attack_hit(actor, target, WIND, self.skill_hit_bp, toughness);
            battle.gain_energy(actor, actor, SKILL_HIT_ENERGY_MILLI);
            hit_targets.push(target);
            if segment == bounces {
                break;
            }

            let remaining: Vec<_> = targetable_enemies(battle).collect();
            if remaining.is_empty() {
                break;
            }
            let unhit: Vec<_> = remaining
                .iter()
                .copied()
                .filter(|target| !hit_targets.contains(target))
                .collect();
            let candidates = if unhit.is_empty() { &remaining } else { &unhit };
            target = candidates[battle.random_index(candidates.len())];
        }
        let state = &mut battle.units[actor].anaxa_state;
        state.skill_damage_bonus_bp = 0;
        if self.eidolon >= 1
            && battle
                .nested_skill
                .is_none_or(|skill| skill.copied_by.is_none())
            && !std::mem::replace(&mut state.first_skill_used, true)
        {
            battle.restore_skill_points(actor, 1);
        }
    }

    fn resolve_copied_skill(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: Option<usize>,
    ) -> bool {
        let Some(target) = target else { return false };
        self.before_action(battle, actor, Action::Skill, Some(target));
        self.resolve_special_attack(battle, actor, target);
        battle.notify_attack(actor, Action::Skill, Some(target));
        true
    }

    fn on_triggered_skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let target = if battle.is_targetable(target) {
            target
        } else {
            let remaining: Vec<_> = targetable_enemies(battle).collect();
            let index = battle.random_index(remaining.len());
            let Some(&target) = remaining.get(index) else {
                return;
            };
            target
        };
        battle.perform_triggered_skill(actor, target, self.skill.resolve().0);
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let skill_bonus = if origin == Some(Action::Skill) && kind == DamageKind::Direct {
            battle.units[actor].anaxa_state.skill_damage_bonus_bp
        } else {
            0
        };
        skill_bonus + self.disclosure_damage_bp * u32::from(disclosed(battle, target))
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _origin: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        14_000
            * u32::from(self.imperative_hiatus && (self.eidolon == 6 || !has_two_erudition(battle)))
    }

    fn ally_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
        _element: Element,
    ) -> u32 {
        5_000
            * u32::from(self.imperative_hiatus && (self.eidolon == 6 || has_two_erudition(battle)))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, _actor: usize, target: usize) -> u32 {
        if self.qualitative_shift {
            weakness_count(battle, target) * 400
        } else {
            0
        }
    }

    fn outgoing_damage_factor_bp(&self) -> u32 {
        if self.eidolon == 6 { 13_000 } else { 10_000 }
    }
}
