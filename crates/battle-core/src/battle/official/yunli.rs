use std::{ops::RangeInclusive, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        resolve::{FollowUpAttack, FollowUpBounces},
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1221";
const BASIC: &str = "skill.122101";
const SKILL: &str = "skill.122102";
const ULTIMATE: &str = "skill.122103";
const TALENT: &str = "skill.122104";
const A2: &str = "trace.1221101";
const A4: &str = "trace.1221102";
const A6: &str = "trace.1221103";
const E4: &str = "eidolon.122104";
const ENERGY_CAP: u32 = 240;
const ULTIMATE_COST: u32 = 120;
const CULL_BOUNCES: u8 = 6;

#[derive(Clone, Default)]
pub(in crate::battle) struct YunliState {
    parry: bool,
    parry_expiry_turn: Option<(usize, bool)>,
    next_slash_is_cull: bool,
}

struct Yunli {
    heal_attack_bp: u16,
    heal_flat_hundredths: u32,
    healing_bp: u32,
    ultimate_crit_damage_bp: u16,
    ultimate_blast_bp: (u16, u16),
    cull_bounce_bp: u16,
    talent_blast_bp: (u16, u16),
    talent_energy_milli: u32,
    eidolon: u8,
    a2: bool,
    a4: bool,
    a6: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != Element::Physical
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 8)?;
    let talent = input.parameters(TALENT, 3)?;
    let flat_heal = (skill[3] * 100.0).round();
    if !(0.0..=f64::from(u32::MAX)).contains(&flat_heal) {
        return Err(invalid(SKILL));
    }
    if ultimate[2] != 0.0
        || ultimate[3] != f64::from(CULL_BOUNCES)
        || ultimate[4] != 0.0
        || ultimate[7] != f64::from(ULTIMATE_COST)
    {
        return Err(invalid(ULTIMATE));
    }
    let talent_energy = (talent[2] * 1_000.0).round();
    if !(1.0..=f64::from(ENERGY_CAP * 1_000)).contains(&talent_energy) {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Physical,
                multiplier_bp: basis_points(BASIC, basic[0])?,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                Effect::Hit {
                    element: Element::Physical,
                    multiplier_bp: basis_points(SKILL, skill[0])?,
                    toughness: 20,
                },
                Effect::BlastAdjacent {
                    element: Element::Physical,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Yunli {
            heal_attack_bp: basis_points(SKILL, skill[2])?,
            heal_flat_hundredths: flat_heal as u32,
            healing_bp: input.stats.healing_bp,
            ultimate_crit_damage_bp: basis_points(ULTIMATE, ultimate[1])?,
            ultimate_blast_bp: (
                basis_points(ULTIMATE, ultimate[0])?,
                basis_points(ULTIMATE, ultimate[5])?,
            ),
            cull_bounce_bp: basis_points(ULTIMATE, ultimate[6])?,
            talent_blast_bp: (
                basis_points(TALENT, talent[0])?,
                basis_points(TALENT, talent[1])?,
            ),
            talent_energy_milli: talent_energy as u32,
            eidolon: input.eidolon,
            a2: trace(A2),
            a4: trace(A4),
            a6: trace(A6),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn parrying(battle: &Battle<'_>, owner: usize) -> bool {
    battle.units[owner].yunli_state.parry
}

fn apply_taunts(battle: &mut Battle<'_>, owner: usize) {
    for enemy in battle.enemies() {
        if battle.is_targetable(enemy) {
            battle.apply_permanent_debuff(owner, enemy, DebuffKind::Taunt, 0);
        }
    }
}

fn end_parry(battle: &mut Battle<'_>, owner: usize) {
    let state = &mut battle.units[owner].yunli_state;
    if std::mem::take(&mut state.parry) {
        state.parry_expiry_turn = None;
        battle.emit(
            EventKind::StatusExpired(StatusKind::Parry),
            Some(owner),
            Some(owner),
        );
        for enemy in battle.enemies() {
            battle.remove_debuff(owner, enemy, DebuffKind::Taunt);
        }
    }
}

fn enhanced_counter(origin: Option<Action>, kind: DamageKind) -> bool {
    kind == DamageKind::FollowUp && origin == Some(Action::Ultimate)
}

impl Yunli {
    fn observe_turn_opening(battle: &mut Battle<'_>, owner: usize, actor: usize, extra: bool) {
        let state = &mut battle.units[owner].yunli_state;
        if state.parry {
            state.parry_expiry_turn = Some((actor, extra));
        }
    }

    fn queue_counter(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        preferred: Option<usize>,
        ultimate: bool,
        cull: bool,
    ) {
        let (primary_bp, adjacent_bp) = if ultimate {
            self.ultimate_blast_bp
        } else {
            self.talent_blast_bp
        };
        battle.queue_counter_blast(
            owner,
            preferred,
            if ultimate {
                Action::Ultimate
            } else {
                Action::Basic
            },
            FollowUpAttack {
                element: Element::Physical,
                multiplier_bp: primary_bp,
                toughness: if ultimate { 20 } else { 10 },
                energy_milli: 10_000,
                first_hit: None,
            },
            (adjacent_bp, 10),
            cull.then_some(FollowUpBounces {
                hit_count: CULL_BOUNCES + 3 * u8::from(self.eidolon >= 1),
                multiplier_bp: self.cull_bounce_bp,
                toughness_half_units: 5,
            }),
        );
    }
}

impl Mechanics for Yunli {
    fn ultimate_energy_range(&self, _: Action) -> Option<RangeInclusive<u32>> {
        Some(ULTIMATE_COST * 1_000..=ENERGY_CAP * 1_000)
    }

    fn can_use_ultimate(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !parrying(battle, actor) && !battle.is_crowd_controlled(actor)
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

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Skill {
            let attack = u64::from(battle.effective_attack(actor));
            let raw_heal = attack * u64::from(self.heal_attack_bp)
                + u64::from(self.heal_flat_hundredths) * 100;
            let heal = u32::try_from(raw_heal / 10_000).unwrap_or(u32::MAX);
            let healing_factor = 10_000u32.saturating_add(self.healing_bp);
            battle.heal_unit(actor, actor, percentage_of(heal, healing_factor));
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if action == Action::Ultimate {
            battle.units[actor].yunli_state.parry = true;
            battle.emit(
                EventKind::StatusApplied(StatusKind::Parry),
                Some(actor),
                Some(actor),
            );
            apply_taunts(battle, actor);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if parrying(battle, owner) {
            apply_taunts(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        end_parry(battle, actor);
    }

    fn taunts_enemy(&self, battle: &Battle<'_>, owner: usize, enemy: usize) -> bool {
        parrying(battle, owner)
            && battle.units[enemy]
                .debuffs
                .iter()
                .any(|debuff| debuff.kind == DebuffKind::Taunt && debuff.source == owner)
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        attacker: usize,
        target: usize,
        _: bool,
    ) {
        let attacked = owner == target;
        if attacked {
            battle.gain_energy(attacker, owner, self.talent_energy_milli);
        }
        if battle.is_crowd_controlled(owner) {
            return;
        }
        if parrying(battle, owner) {
            if attacked || self.eidolon >= 6 {
                end_parry(battle, owner);
                self.queue_counter(battle, owner, Some(attacker), true, true);
            }
        } else if attacked {
            self.queue_counter(battle, owner, Some(attacker), false, false);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        Self::observe_turn_opening(battle, owner, actor, extra);
    }

    fn on_enemy_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        Self::observe_turn_opening(battle, owner, actor, false);
    }

    fn after_battle_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, extra: bool) {
        if battle.units[owner].yunli_state.parry_expiry_turn != Some((actor, extra)) {
            return;
        }
        end_parry(battle, owner);
        if !battle.is_crowd_controlled(owner) {
            let state = &mut battle.units[owner].yunli_state;
            let cull = state.next_slash_is_cull;
            state.next_slash_is_cull = self.a2 && !cull;
            self.queue_counter(battle, owner, None, true, cull);
            battle.resolve_follow_ups();
        }
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.a6 {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::AttackPercent,
                3_000,
                1,
                Some(A6),
            );
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, origin: Action) {
        if self.eidolon >= 4 && origin == Action::Ultimate {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::EffectResistance,
                5_000,
                1,
                Some(E4),
            );
        }
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, actor: usize) -> bool {
        self.a4 && parrying(battle, actor)
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: DamageKind,
    ) -> u32 {
        2_000 * u32::from(self.a4 && parrying(battle, actor))
    }

    fn crit_damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        u32::from(self.ultimate_crit_damage_bp) * u32::from(enhanced_counter(origin, kind))
    }

    fn damage_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        2_000 * u32::from(self.eidolon >= 1 && enhanced_counter(origin, kind))
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        1_500 * u32::from(self.eidolon >= 6 && enhanced_counter(origin, kind))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, _: usize, _: usize) -> u32 {
        2_000 * u32::from(self.eidolon >= 2 && battle.in_follow_up())
    }

    fn follow_up_counts_as_ultimate(&self, origin: Option<Action>) -> bool {
        origin == Some(Action::Ultimate)
    }

    fn ultimate_resistance_penetration_bp(&self) -> u32 {
        2_000 * u32::from(self.eidolon >= 6)
    }
}
