use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1205";
const WIND: Element = Element::Wind;
const MAX_ENERGY: u32 = 130;
const HALF_HP_BP: u32 = 5_000;
const BASE_CHARGE_LIMIT: u8 = 5;
const E6_CHARGE_LIMIT: u8 = 4;
const E1_TALLY_BP: u16 = 15_000;
const E2_CRIT_BP: u32 = 1_500;
const E4_MAX_HP_BP: u32 = 2_000;
const E4_LIMIT: u8 = 2;
const E6_MAX_HP_BP: u16 = 5_000;
const INCOMING_HEAL_BP: u32 = 2_000;
const A4_MAX_HP_HEAL_BP: u32 = 500;
const A4_FLAT_HEAL: u32 = 100;
const A4_HEAL_TALLY_BP: u32 = 2_500;
const A6_FOLLOW_UP_BONUS_BP: u32 = 2_000;
const A6_FOLLOW_UP_ENERGY_MILLI: u32 = 15_000;

struct KitIds {
    basic: &'static str,
    skill: &'static str,
    ultimate: &'static str,
    talent: &'static str,
    enhanced_basic: &'static str,
    traces: [&'static str; 3],
}

const BASE: KitIds = KitIds {
    basic: "skill.120501",
    skill: "skill.120502",
    ultimate: "skill.120503",
    talent: "skill.120504",
    enhanced_basic: "skill.120508",
    traces: ["trace.1205101", "trace.1205102", "trace.1205103"],
};
const NOVAFLARE: KitIds = KitIds {
    basic: "skill.1120501",
    skill: "skill.1120502",
    ultimate: "skill.1120503",
    talent: "skill.1120504",
    enhanced_basic: "skill.1120508",
    traces: ["trace.11205101", "trace.11205102", "trace.11205103"],
};

#[derive(Clone, Default)]
pub(in crate::battle) struct BladeState {
    hellscape_until: u32,
    hp_tally: u32,
    charges: u8,
    charged_from_enemy_attack: bool,
    e4_stacks: u8,
    hit_broken_target: bool,
}

#[derive(Clone, Copy)]
struct DamageRatio {
    attack_bp: u16,
    max_hp_bp: u16,
    tally_bp: u16,
}

struct Blast {
    primary: DamageRatio,
    adjacent: DamageRatio,
    adjacent_toughness: u16,
}

struct Blade {
    enhanced: bool,
    eidolon: u8,
    a2: bool,
    a4: bool,
    a6: bool,
    skill_hp_cost_bp: u32,
    skill_turns: u32,
    skill_damage_bp: u32,
    hellscape_aggro_bp: u32,
    basic_hp_cost_bp: u32,
    tally_cap_bp: u32,
    enhanced_basic: OfficialAbility,
    basic_blast: Blast,
    ultimate_blast: Blast,
    follow_up_attack_bp: u16,
    follow_up_max_hp_bp: u16,
    follow_up_heal_bp: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != MAX_ENERGY || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM} energy or eidolon"
        )));
    }
    let (ids, enhanced) = match (
        input.skill_parameters.contains_key(BASE.basic),
        input.skill_parameters.contains_key(NOVAFLARE.basic),
    ) {
        (true, false) => (&BASE, false),
        (false, true) => (&NOVAFLARE, true),
        _ => return Err(BattleError::InvalidInput(format!("{FORM} Blade variant"))),
    };
    let basic_bp = ratio_bp(ids.basic, input.parameters(ids.basic, 1)?[0])?;
    let skill = input.parameters(ids.skill, if enhanced { 5 } else { 4 })?;
    let ultimate = input.parameters(ids.ultimate, 7)?;
    let talent = input.parameters(ids.talent, 4)?;
    let enhanced_basic = input.parameters(ids.enhanced_basic, 5)?;
    if skill[0] != 0.3 || skill[1] != 3.0 || enhanced_basic[0] != 0.1 || ultimate[6] != 0.9 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM} Blade fixed parameters"
        )));
    }
    let convert = |id, attack, max_hp, tally| {
        Ok::<DamageRatio, BattleError>(DamageRatio {
            attack_bp: ratio_bp(id, attack)?,
            max_hp_bp: ratio_bp(id, max_hp)?,
            tally_bp: ratio_bp(id, tally)?,
        })
    };
    let basic_ratios = if enhanced {
        [(0.0, enhanced_basic[1]), (0.0, enhanced_basic[2])]
    } else {
        [
            (enhanced_basic[1], enhanced_basic[3]),
            (enhanced_basic[2], enhanced_basic[4]),
        ]
    };
    let ultimate_ratios = if enhanced {
        [(0.0, ultimate[0]), (0.0, ultimate[2])]
    } else {
        [(ultimate[0], ultimate[1]), (ultimate[2], ultimate[3])]
    };
    let [a2, a4, a6] = ids
        .traces
        .map(|trace| input.enabled_trace_ids.iter().any(|id| id == trace));
    let basic_effect = if enhanced {
        Effect::MaxHpHit {
            element: WIND,
            multiplier_bp: basic_bp,
            toughness: 10,
        }
    } else {
        Effect::Hit {
            element: WIND,
            multiplier_bp: basic_bp,
            toughness: 10,
        }
    };
    Ok(OfficialKit {
        max_energy_milli: MAX_ENERGY * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![basic_effect],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 0,
            effects: vec![],
            skill_points: SkillPoints::Spend,
        },
        ultimate: blast_ability(5_000),
        mechanics: Arc::new(Blade {
            enhanced,
            eidolon: input.eidolon,
            a2,
            a4,
            a6,
            skill_hp_cost_bp: u32::from(ratio_bp(ids.skill, skill[0])?),
            skill_turns: skill[1] as u32,
            skill_damage_bp: u32::from(ratio_bp(ids.skill, skill[3])?),
            hellscape_aggro_bp: if enhanced {
                10_000 + checked_aggro_bp(ids.skill, skill[4])?
            } else {
                10_000
            },
            basic_hp_cost_bp: u32::from(ratio_bp(ids.enhanced_basic, enhanced_basic[0])?),
            tally_cap_bp: u32::from(ratio_bp(ids.ultimate, ultimate[6])?),
            enhanced_basic: blast_ability(30_000),
            basic_blast: Blast {
                primary: convert(
                    ids.enhanced_basic,
                    basic_ratios[0].0,
                    basic_ratios[0].1,
                    0.0,
                )?,
                adjacent: convert(
                    ids.enhanced_basic,
                    basic_ratios[1].0,
                    basic_ratios[1].1,
                    0.0,
                )?,
                adjacent_toughness: 10,
            },
            ultimate_blast: Blast {
                primary: convert(
                    ids.ultimate,
                    ultimate_ratios[0].0,
                    ultimate_ratios[0].1,
                    ultimate[4],
                )?,
                adjacent: convert(
                    ids.ultimate,
                    ultimate_ratios[1].0,
                    ultimate_ratios[1].1,
                    ultimate[5],
                )?,
                adjacent_toughness: 20,
            },
            follow_up_attack_bp: if enhanced {
                0
            } else {
                ratio_bp(ids.talent, talent[1])?
            },
            follow_up_max_hp_bp: ratio_bp(
                ids.talent,
                if enhanced { talent[1] } else { talent[3] },
            )?
            .checked_add(if input.eidolon >= 6 { E6_MAX_HP_BP } else { 0 })
            .ok_or_else(|| BattleError::InvalidInput(format!("{FORM} Blade follow-up ratio")))?,
            follow_up_heal_bp: u32::from(ratio_bp(ids.talent, talent[2])?),
        }),
    })
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(0.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM} {id} damage ratio"
        )));
    }
    Ok(bp as u16)
}

fn checked_aggro_bp(id: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio != 10.0 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM} {id} aggro ratio"
        )));
    }
    Ok(100_000)
}

fn blast_ability(energy_milli: u32) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::DirectedAreaAttack],
        skill_points: SkillPoints::Unchanged,
    }
}

impl Blade {
    fn hellscape(battle: &Battle<'_>, actor: usize) -> bool {
        let unit = &battle.units[actor];
        unit.finished_turns < unit.blade_state.hellscape_until
    }

    fn charge_limit(&self) -> u8 {
        if self.eidolon >= 6 {
            E6_CHARGE_LIMIT
        } else {
            BASE_CHARGE_LIMIT
        }
    }

    fn set_charges(battle: &mut Battle<'_>, actor: usize, after: u8) {
        let before = std::mem::replace(&mut battle.units[actor].blade_state.charges, after);
        if after != before {
            battle.emit(
                EventKind::BladeCharge { before, after },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn gain_charge(&self, battle: &mut Battle<'_>, actor: usize) {
        let next = (battle.units[actor].blade_state.charges + 1).min(self.charge_limit());
        Self::set_charges(battle, actor, next);
    }

    fn tally_cap(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        percentage_of(battle.effective_max_hp(actor), self.tally_cap_bp)
    }

    fn add_tally(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        let cap = self.tally_cap(battle, actor);
        let tally = &mut battle.units[actor].blade_state.hp_tally;
        *tally = tally.saturating_add(amount).min(cap);
    }

    fn hp_lost(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        let (hp, max_hp) = battle.hp(actor);
        let unit = &mut battle.units[actor];
        if self.eidolon >= 4
            && unit.blade_state.e4_stacks < E4_LIMIT
            && hp.saturating_add(amount) > max_hp / 2
            && hp <= max_hp / 2
        {
            unit.blade_state.e4_stacks += 1;
            if hp > 0 {
                let after = battle.effective_max_hp(actor);
                battle.units[actor].hp = hp.saturating_add(after.saturating_sub(max_hp));
            }
        }
        self.add_tally(battle, actor, amount);
    }

    fn queue_follow_up(&self, battle: &mut Battle<'_>, actor: usize) -> bool {
        if battle.units[actor].blade_state.charges < self.charge_limit()
            || !battle.is_targetable(actor)
            || battle.is_crowd_controlled(actor)
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return false;
        }
        Self::set_charges(battle, actor, 0);
        let bonus = A6_FOLLOW_UP_ENERGY_MILLI * u32::from(self.enhanced && self.a6);
        battle.follow_up(
            actor,
            WIND,
            self.follow_up_attack_bp,
            10,
            battle.action.unwrap_or(Action::Basic),
            10_000 + bonus,
        );
        true
    }

    fn set_half_hp(battle: &mut Battle<'_>, actor: usize) {
        let (hp, max_hp) = battle.hp(actor);
        let half = percentage_of(max_hp, HALF_HP_BP).max(1);
        if hp > half {
            battle.spend_hp(actor, hp - half, 1);
        } else if hp < half {
            battle.restore_hp_unmodified(actor, actor, half - hp);
        }
    }

    fn blast(&self, battle: &mut Battle<'_>, actor: usize, target: usize, ultimate: bool) {
        let blast = if ultimate {
            &self.ultimate_blast
        } else {
            &self.basic_blast
        };
        let cap = self.tally_cap(battle, actor);
        let tally = u64::from(battle.units[actor].blade_state.hp_tally.min(cap));
        let e1 = E1_TALLY_BP * u16::from(self.eidolon >= 1 && (ultimate || self.enhanced));
        for (enemy, ratio, toughness, extra_bp) in std::iter::once((target, blast.primary, 20, e1))
            .chain(
                battle
                    .adjacent_enemies(target)
                    .map(|enemy| (enemy, blast.adjacent, blast.adjacent_toughness, 0)),
            )
        {
            battle.attack_hit_with_max_hp(
                actor,
                enemy,
                WIND,
                (ratio.attack_bp, ratio.max_hp_bp),
                tally * (u64::from(ratio.tally_bp) + u64::from(extra_bp)),
                toughness,
            );
        }
    }
}

impl Mechanics for Blade {
    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !Self::hellscape(battle, actor)
    }

    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && Self::hellscape(battle, actor)).then_some(&self.enhanced_basic)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let unit = &mut battle.units[actor];
                unit.blade_state.hellscape_until =
                    unit.finished_turns.saturating_add(self.skill_turns);
                battle.spend_hp(
                    actor,
                    percentage_of(battle.effective_max_hp(actor), self.skill_hp_cost_bp),
                    1,
                );
            }
            Action::Basic if Self::hellscape(battle, actor) => {
                battle.units[actor].blade_state.hit_broken_target = false;
                battle.spend_hp(
                    actor,
                    percentage_of(battle.effective_max_hp(actor), self.basic_hp_cost_bp),
                    1,
                );
            }
            Action::Ultimate => Self::set_half_hp(battle, actor),
            _ => {}
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        self.blast(
            battle,
            actor,
            target,
            battle.action == Some(Action::Ultimate),
        );
    }

    fn before_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
    ) {
        if !self.enhanced
            && self.a4
            && action == Action::Basic
            && Self::hellscape(battle, actor)
            && battle.units[target].toughness == 0
        {
            battle.units[actor].blade_state.hit_broken_target = true;
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Basic
                if std::mem::take(&mut battle.units[actor].blade_state.hit_broken_target) =>
            {
                let amount = percentage_of(battle.effective_max_hp(actor), A4_MAX_HP_HEAL_BP)
                    .saturating_add(A4_FLAT_HEAL);
                battle.heal_unit(actor, actor, amount);
            }
            Action::Ultimate => {
                let cap = self.tally_cap(battle, actor);
                let tally = &mut battle.units[actor].blade_state.hp_tally;
                *tally = if self.enhanced && self.a2 {
                    (*tally).min(cap) / 2
                } else {
                    0
                };
            }
            _ => {}
        }
    }

    fn after_hp_loss(&self, battle: &mut Battle<'_>, actor: usize, _source: usize, amount: u32) {
        self.hp_lost(battle, actor, amount);
        if battle.enemy_action.is_some() {
            let charged = &mut battle.units[actor].blade_state.charged_from_enemy_attack;
            if std::mem::replace(charged, true) {
                return;
            }
        }
        self.gain_charge(battle, actor);
    }

    fn after_hp_spent(&self, battle: &mut Battle<'_>, actor: usize, amount: u32) {
        self.hp_lost(battle, actor, amount);
        self.gain_charge(battle, actor);
    }

    fn after_enemy_action(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].blade_state.charged_from_enemy_attack = false;
        self.queue_follow_up(battle, owner);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        _action: Action,
    ) {
        if self.queue_follow_up(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, _actor: usize) {
        if self.queue_follow_up(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        let amount = percentage_of(battle.effective_max_hp(actor), self.follow_up_heal_bp);
        battle.heal_unit(actor, actor, amount);
    }

    fn follow_up_max_hp_multiplier_bp(&self) -> u16 {
        self.follow_up_max_hp_bp
    }

    fn incoming_healing_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let (hp, max_hp) = battle.hp(actor);
        INCOMING_HEAL_BP
            * u32::from(if self.enhanced {
                self.a4
            } else {
                self.a2 && hp <= max_hp / 2
            })
    }

    fn after_healing(&self, battle: &mut Battle<'_>, target: usize, restored: u32) {
        if self.enhanced && self.a4 {
            self.add_tally(battle, target, percentage_of(restored, A4_HEAL_TALLY_BP));
        }
    }

    fn ally_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner != target {
            return 0;
        }
        let stacks = u32::from(battle.units[owner].blade_state.e4_stacks);
        percentage_of(
            battle.plan.units[owner].stats.base_hp,
            stacks * E4_MAX_HP_BP,
        )
    }

    fn aggro_multiplier_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if Self::hellscape(battle, actor) {
            self.hellscape_aggro_bp
        } else {
            10_000
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        u32::from(Self::hellscape(battle, actor)) * self.skill_damage_bp
            + u32::from(self.a6 && kind == DamageKind::FollowUp) * A6_FOLLOW_UP_BONUS_BP
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        E2_CRIT_BP * u32::from(self.eidolon >= 2 && Self::hellscape(battle, actor))
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Skill).then_some(0)
    }
}
