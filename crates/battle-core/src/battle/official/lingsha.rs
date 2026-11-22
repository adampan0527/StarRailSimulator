use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle, SummonTurn,
        combat_status::DebuffKind,
        resolve::FollowUpAttack,
        support::{AllyBuffKind, percentage_of},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::Healing,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1222";
const BASIC: &str = "skill.122201";
const SKILL: &str = "skill.122202";
const ULTIMATE: &str = "skill.122203";
const TALENT: &str = "skill.122204";
const VERMILION_WAFT: &str = "trace.1222101";
const SYLVAN_SMOKE: &str = "trace.1222102";
const EMBER_ECHO: &str = "trace.1222103";
const FUYUAN_SPEED_MICROS: u64 = 90_000_000;
const FUYUAN_MAX_ACTIONS: u8 = 5;
const FUYUAN_ADDED_ACTIONS: u8 = 3;
const FUYUAN_SINGLE_TOUGHNESS: u16 = 10;
const EMERGENCY_THRESHOLD_BP: u32 = 6_000;
const EMERGENCY_COOLDOWN: u32 = 2;
const E1_DEFENSE_DOWN_BP: u32 = 2_000;
const E6_RESISTANCE_DOWN_BP: u32 = 2_000;
const E6_HITS: u8 = 4;

#[derive(Clone, Default)]
pub(in crate::battle) struct LingshaState {
    fuyuan_actions: u8,
    scheduled_attack: bool,
    emergency_pending: bool,
    emergency_ready_after_turn: u32,
}

struct Lingsha {
    eidolon: u8,
    healing_bp: u32,
    vermilion_waft: bool,
    ember_echo: bool,
    skill_heal: Healing,
    ultimate_heal: Healing,
    follow_up_heal: Healing,
    extra_heal: Healing,
    skill_advance_bp: u16,
    ultimate_advance_bp: u16,
    befog_bp: u32,
    befog_turns: u32,
    fuyuan_attack: FollowUpAttack,
    fuyuan_single_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Abundance
        || input.combat_type.element != Element::Fire
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 6)?;
    let talent = input.parameters(TALENT, 8)?;
    if skill[3] != 0.2 || ultimate[4] != 2.0 || ultimate[5] != 1.0 {
        return Err(invalid("advance or Befog duration"));
    }
    if talent[0] != 90.0
        || talent[4] != f64::from(FUYUAN_MAX_ACTIONS)
        || talent[5] != 1.0
        || talent[6] != f64::from(FUYUAN_ADDED_ACTIONS)
    {
        return Err(invalid(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: 110_000,
        basic: fire_attack(
            Target::Enemy,
            basis_points(BASIC, basic[0])?,
            10,
            20_000 + 10_000 * u32::from(trace(SYLVAN_SMOKE)),
            SkillPoints::Generate,
        ),
        skill: fire_attack(
            Target::AllEnemies,
            basis_points(SKILL, skill[0])?,
            10,
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: fire_attack(
            Target::AllEnemies,
            basis_points(ULTIMATE, ultimate[0])?,
            20,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Lingsha {
            eidolon: input.eidolon,
            healing_bp: input.stats.healing_bp,
            vermilion_waft: trace(VERMILION_WAFT),
            ember_echo: trace(EMBER_ECHO),
            skill_heal: Healing::from_data(FORM, SKILL, skill[1], skill[2])?,
            ultimate_heal: Healing::from_data(FORM, ULTIMATE, ultimate[1], ultimate[2])?,
            follow_up_heal: Healing::from_data(FORM, TALENT, talent[2], talent[3])?,
            extra_heal: Healing::from_data(FORM, "eidolon.122204", 0.4, 0.0)?,
            skill_advance_bp: basis_points(SKILL, skill[3])?,
            ultimate_advance_bp: basis_points(ULTIMATE, ultimate[5])?,
            befog_bp: u32::from(basis_points(ULTIMATE, ultimate[3])?),
            befog_turns: ultimate[4] as u32,
            fuyuan_attack: FollowUpAttack {
                element: Element::Fire,
                multiplier_bp: basis_points(TALENT, talent[1])?,
                toughness: 10,
                energy_milli: 0,
                first_hit: None,
            },
            fuyuan_single_bp: basis_points(TALENT, talent[7])?,
        }),
    })
}

fn invalid(parameter: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {parameter}"))
}

fn basis_points(parameter: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(parameter));
    }
    Ok(bp as u16)
}

fn fire_attack(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Fire,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn lowest_hp_ally(battle: &Battle<'_>) -> Option<usize> {
    battle
        .allies()
        .filter(|&target| battle.is_targetable(target))
        .min_by(|&left, &right| {
            let (left_hp, left_max) = battle.hp(left);
            let (right_hp, right_max) = battle.hp(right);
            (u64::from(left_hp) * u64::from(right_max))
                .cmp(&(u64::from(right_hp) * u64::from(left_max)))
        })
}

fn emergency_threshold_met(battle: &Battle<'_>) -> bool {
    battle.allies().any(|target| {
        let (hp, max_hp) = battle.hp(target);
        battle.is_targetable(target) && hp <= percentage_of(max_hp, EMERGENCY_THRESHOLD_BP)
    })
}

impl Lingsha {
    fn healing_amount(&self, battle: &Battle<'_>, owner: usize, heal: Healing) -> u32 {
        let trace_bonus = if self.vermilion_waft {
            percentage_of(battle.effective_break_effect(owner), 1_000).min(2_000)
        } else {
            0
        };
        heal.amount(
            battle.effective_attack(owner),
            u64::from(self.healing_bp.saturating_add(trace_bonus)) * 100,
        )
    }

    fn heal_team(&self, battle: &mut Battle<'_>, owner: usize, heal: Healing, cleanse: bool) {
        let amount = self.healing_amount(battle, owner, heal);
        for target in battle.allies() {
            if battle.is_targetable(target) {
                if cleanse {
                    battle.cleanse_one(owner, target);
                }
                battle.heal_unit(owner, target, amount);
            }
        }
    }

    fn summon_fuyuan(&self, battle: &mut Battle<'_>, owner: usize) {
        let unit = &mut battle.units[owner];
        let summon = unit
            .summon_turn
            .get_or_insert_with(|| SummonTurn::new("fuyuan", FUYUAN_SPEED_MICROS));
        unit.lingsha_state.fuyuan_actions =
            (unit.lingsha_state.fuyuan_actions + FUYUAN_ADDED_ACTIONS).min(FUYUAN_MAX_ACTIONS);
        summon.advance(self.skill_advance_bp);
    }

    fn queue_fuyuan(&self, battle: &mut Battle<'_>, owner: usize) {
        let attack = &self.fuyuan_attack;
        battle.follow_up(
            owner,
            attack.element,
            attack.multiplier_bp,
            attack.toughness,
            Action::Skill,
            attack.energy_milli,
        );
    }

    fn queue_emergency(&self, battle: &mut Battle<'_>, owner: usize) -> bool {
        let unit = &mut battle.units[owner];
        if !std::mem::take(&mut unit.lingsha_state.emergency_pending)
            || unit.summon_turn.is_none()
            || !battle.enemies().any(|enemy| battle.is_targetable(enemy))
        {
            return false;
        }
        let unit = &mut battle.units[owner];
        unit.lingsha_state.emergency_ready_after_turn =
            unit.finished_turns.saturating_add(EMERGENCY_COOLDOWN);
        self.queue_fuyuan(battle, owner);
        true
    }

    fn follow_up_target(battle: &mut Battle<'_>) -> Option<usize> {
        let preferred: Vec<_> = battle
            .enemies()
            .filter(|&enemy| {
                battle.is_targetable(enemy)
                    && battle.units[enemy].toughness > 0
                    && battle.has_weakness(enemy, Element::Fire)
            })
            .collect();
        if preferred.is_empty() {
            battle.random_living_enemy()
        } else {
            Some(preferred[battle.random_index(preferred.len())])
        }
    }
}

impl Mechanics for Lingsha {
    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action != Action::Ultimate {
            return;
        }
        if self.eidolon >= 2 {
            for ally in battle.allies() {
                if battle.is_targetable(ally) {
                    battle.apply_ally_buff(owner, ally, AllyBuffKind::BreakEffect, 4_000, 3);
                }
            }
        }
        for enemy in battle.enemies() {
            battle.apply_debuff(
                owner,
                enemy,
                DebuffKind::Befog,
                self.befog_bp,
                self.befog_turns,
            );
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                self.heal_team(battle, owner, self.skill_heal, false);
                self.summon_fuyuan(battle, owner);
            }
            Action::Ultimate => {
                self.heal_team(battle, owner, self.ultimate_heal, false);
                if let Some(summon) = &mut battle.units[owner].summon_turn {
                    summon.advance(self.ultimate_advance_bp);
                }
            }
            Action::Basic | Action::EnhancedUltimate => {}
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].lingsha_state.scheduled_attack = true;
        self.queue_fuyuan(battle, owner);
        battle.resolve_follow_ups();
    }

    fn on_ally_health_event(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _target: usize,
        _attack_index: Option<u32>,
    ) {
        let unit = &battle.units[owner];
        if self.ember_echo
            && unit.summon_turn.is_some()
            && unit.finished_turns >= unit.lingsha_state.emergency_ready_after_turn
            && emergency_threshold_met(battle)
        {
            battle.units[owner].lingsha_state.emergency_pending = true;
        }
    }

    fn after_enemy_action(&self, battle: &mut Battle<'_>, owner: usize) {
        self.queue_emergency(battle, owner);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        _action: Action,
    ) {
        if self.queue_emergency(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, _actor: usize) {
        if self.queue_emergency(battle, owner) {
            battle.resolve_follow_ups();
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, owner: usize, origin: Action) {
        let extra_hits = std::iter::once((self.fuyuan_single_bp, FUYUAN_SINGLE_TOUGHNESS)).chain(
            std::iter::repeat_n(
                (5_000, 5),
                usize::from(E6_HITS * u8::from(self.eidolon >= 6)),
            ),
        );
        for (multiplier_bp, toughness) in extra_hits {
            let Some(target) = Self::follow_up_target(battle) else {
                break;
            };
            battle.follow_up_hit(
                owner,
                target,
                origin,
                Element::Fire,
                multiplier_bp,
                toughness,
            );
        }
        let extra_target = (self.eidolon >= 4)
            .then(|| lowest_hp_ally(battle))
            .flatten();
        self.heal_team(battle, owner, self.follow_up_heal, true);
        if let Some(target) = extra_target {
            let amount = self.healing_amount(battle, owner, self.extra_heal);
            battle.heal_unit(owner, target, amount);
        }

        let unit = &mut battle.units[owner];
        if std::mem::take(&mut unit.lingsha_state.scheduled_attack) {
            unit.lingsha_state.fuyuan_actions = unit.lingsha_state.fuyuan_actions.saturating_sub(1);
            if unit.lingsha_state.fuyuan_actions == 0 {
                unit.summon_turn = None;
            }
        }
    }

    fn damage_comes_from_summon(
        &self,
        battle: &Battle<'_>,
        _owner: usize,
        origin: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        origin == Some(Action::Skill)
            && battle.in_follow_up()
            && matches!(kind, DamageKind::FollowUp | DamageKind::WeaknessBreak)
    }

    fn attack_bonus_from_break_effect_bp(&self, break_effect_bp: u32) -> u32 {
        percentage_of(break_effect_bp, 2_500).min(5_000) * u32::from(self.vermilion_waft)
    }

    fn toughness_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        5_000
            * u32::from(
                self.eidolon >= 1 && matches!(kind, DamageKind::Direct | DamageKind::FollowUp),
            )
    }

    fn target_defense_down_bp(&self, battle: &Battle<'_>, _owner: usize, target: usize) -> u32 {
        E1_DEFENSE_DOWN_BP * u32::from(self.eidolon >= 1 && battle.units[target].toughness == 0)
    }

    fn all_resistance_down_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        E6_RESISTANCE_DOWN_BP
            * u32::from(self.eidolon >= 6 && battle.units[owner].summon_turn.is_some())
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, owner: usize) {
        battle.units[owner].lingsha_state = LingshaState::default();
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Skill).then_some(
            if battle.units[owner].lingsha_state.fuyuan_actions <= 1 {
                2
            } else {
                4
            },
        )
    }
}
