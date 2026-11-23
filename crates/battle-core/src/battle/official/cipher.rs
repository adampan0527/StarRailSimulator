use std::{cmp::Reverse, iter, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        combat_status::DebuffKind,
        resolve::{DamageTargetAura, FollowUpAttack},
        support::AllyBuffKind,
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1406";
const BASIC: &str = "skill.140601";
const SKILL: &str = "skill.140602";
const ULTIMATE: &str = "skill.140603";
const TALENT: &str = "skill.140604";
const EMPYREAN_STRIDES: &str = "trace.1406101";
const THREE_HUNDRED_ROGUES: &str = "trace.1406102";
const SLEIGHT_OF_SKY: &str = "trace.1406103";
const E1_ATTACK: &str = "eidolon.140601";
const QUANTUM: Element = Element::Quantum;
const ENERGY_CAPACITY: u32 = 130;
const BP_UNIT: u64 = 10_000;
const NON_PATRON_TALLY_BP: u16 = 800;

#[derive(Clone, Copy, Default)]
pub(in crate::battle) struct CipherState {
    patron: Option<usize>,
    follow_up_used: bool,
    tally_in_basis_points: u64,
}

struct Cipher {
    eidolon: u8,
    empyrean_strides: bool,
    three_hundred_rogues: bool,
    sleight_of_sky: bool,
    skill_attack_bp: u32,
    weaken_bp: u32,
    weaken_chance_bp: u32,
    skill_turns: u32,
    first_ultimate_hit_bp: u16,
    blast_ultimate_hit_bp: u16,
    single_true_damage_bp: u16,
    patron_tally_bp: u16,
    follow_up: FollowUpAttack,
}

pub(super) fn is_patron(battle: &Battle<'_>, owner: usize, target: usize) -> bool {
    battle.units[owner].cipher_state.patron == Some(target)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAPACITY
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Nihility
        || input.combat_type.element != QUANTUM
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 6)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 4)?;
    if skill[3] != 2.0 || ultimate[1..=2] != [0.25, 0.75] || talent[2..] != [1.0, 5.0] {
        return Err(invalid("duration, Ultimate split or Talent triggers"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAPACITY * 1_000,
        basic: ability(
            20_000,
            SkillPoints::Generate,
            vec![hit(basis_points(BASIC, basic[0])?, 10)],
        ),
        skill: ability(
            30_000,
            SkillPoints::Spend,
            vec![
                hit(basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: QUANTUM,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
        ),
        ultimate: ability(
            5_000,
            SkillPoints::Unchanged,
            vec![Effect::DirectedAreaAttack],
        ),
        mechanics: Arc::new(Cipher {
            eidolon: input.eidolon,
            empyrean_strides: trace(EMPYREAN_STRIDES),
            three_hundred_rogues: trace(THREE_HUNDRED_ROGUES),
            sleight_of_sky: trace(SLEIGHT_OF_SKY),
            skill_attack_bp: u32::from(basis_points(SKILL, skill[4])?),
            weaken_bp: u32::from(basis_points(SKILL, skill[2])?),
            weaken_chance_bp: u32::from(basis_points(SKILL, skill[5])?),
            skill_turns: 2,
            first_ultimate_hit_bp: basis_points(ULTIMATE, ultimate[0])?,
            blast_ultimate_hit_bp: basis_points(ULTIMATE, ultimate[3])?,
            single_true_damage_bp: basis_points(ULTIMATE, ultimate[1])?,
            patron_tally_bp: basis_points(TALENT, talent[1])?,
            follow_up: FollowUpAttack {
                element: QUANTUM,
                multiplier_bp: basis_points(TALENT, talent[0])?,
                toughness: 20,
                energy_milli: 5_000,
                first_hit: None,
            },
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(id: &str, value: f64) -> Result<u16, BattleError> {
    let rounded = (value * BP_UNIT as f64).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(id));
    }
    Ok(rounded as u16)
}

fn ability(energy_milli: u32, skill_points: SkillPoints, effects: Vec<Effect>) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        skill_points,
        effects,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: QUANTUM,
        multiplier_bp,
        toughness,
    }
}

fn mark_patron(battle: &mut Battle<'_>, owner: usize, target: Option<usize>) {
    let previous = std::mem::replace(&mut battle.units[owner].cipher_state.patron, target);
    if let Some(previous) = previous.filter(|&previous| Some(previous) != target) {
        battle.remove_debuff(owner, previous, DebuffKind::Patron);
    }
    if let Some(target) = target {
        battle.apply_permanent_debuff(owner, target, DebuffKind::Patron, 0);
    }
}

fn pick_patron(battle: &mut Battle<'_>, owner: usize) {
    let target = battle
        .enemies()
        .filter(|&target| battle.is_targetable(target))
        .max_by_key(|&target| (battle.hp(target).1, Reverse(target)));
    mark_patron(battle, owner, target);
}

impl Cipher {
    fn stride_level(&self, battle: &Battle<'_>, actor: usize) -> u64 {
        match battle.speed_micros(actor) {
            170_000_000.. if self.empyrean_strides => 2,
            140_000_000.. if self.empyrean_strides => 1,
            _ => 0,
        }
    }

    fn e2_vulnerability(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        if self.eidolon >= 2
            && battle.is_targetable(target)
            && battle.roll_effect(actor, target, 12_000)
        {
            battle.apply_debuff(actor, target, DebuffKind::Vulnerability, 3_000, 2);
        }
    }
}

impl Mechanics for Cipher {
    fn tracks_ally_damage(&self) -> bool {
        true
    }

    fn on_ally_damage(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
        kind: DamageKind,
        amount: u32,
    ) {
        let base_bp = if battle.units[owner].cipher_state.patron == Some(target) {
            self.patron_tally_bp
        } else if self.three_hundred_rogues {
            NON_PATRON_TALLY_BP
        } else {
            0
        };
        let stride_factor = 2 + self.stride_level(battle, owner);
        let eidolon_factor = if self.eidolon >= 1 { 3 } else { 2 };
        let mut record_bp = u64::from(base_bp) * stride_factor * eidolon_factor / 4;
        if self.eidolon >= 6 && source == owner && kind == DamageKind::FollowUp {
            record_bp += 1_600;
        }
        let state = &mut battle.units[owner].cipher_state;
        state.tally_in_basis_points = state
            .tally_in_basis_points
            .saturating_add(u64::from(amount) * record_bp);
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        pick_patron(battle, owner);
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if battle.units[owner].cipher_state.patron == Some(target) {
            pick_patron(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        mark_patron(battle, actor, None);
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            battle.units[owner].cipher_state.follow_up_used = false;
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let Some(target) = target else { return };
        if matches!(action, Action::Skill | Action::Ultimate) {
            mark_patron(battle, actor, Some(target));
        }
        if action == Action::Skill {
            for enemy in iter::once(target).chain(battle.adjacent_enemies(target)) {
                if battle.roll_effect(actor, enemy, self.weaken_chance_bp) {
                    battle.apply_debuff(
                        actor,
                        enemy,
                        DebuffKind::Weaken,
                        self.weaken_bp,
                        self.skill_turns,
                    );
                }
            }
            battle.apply_ally_buff(
                actor,
                actor,
                AllyBuffKind::AttackPercent,
                self.skill_attack_bp,
                self.skill_turns,
            );
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.attack_hit(actor, target, QUANTUM, self.first_ultimate_hit_bp, 10);
        let released = battle.units[actor].cipher_state.tally_in_basis_points / BP_UNIT;
        let single = released * u64::from(self.single_true_damage_bp) / BP_UNIT;
        deal_tallied_damage(battle, actor, target, single);

        let mut targets: Vec<_> = iter::once(target)
            .chain(battle.adjacent_enemies(target))
            .filter(|&unit| battle.is_targetable(unit))
            .collect();
        for &unit in &targets {
            battle.attack_hit(actor, unit, QUANTUM, self.blast_ultimate_hit_bp, 20);
        }
        targets.retain(|&unit| battle.is_targetable(unit));
        if !targets.is_empty() {
            let blast = released - single;
            let count = targets.len() as u64;
            for (index, unit) in targets.into_iter().enumerate() {
                let share = blast / count + u64::from((index as u64) < blast % count);
                deal_tallied_damage(battle, actor, unit, share);
            }
        }

        let tally = &mut battle.units[actor].cipher_state.tally_in_basis_points;
        *tally = if self.eidolon >= 6 { *tally / 5 } else { 0 };
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
        _primary_target: Option<usize>,
    ) {
        let Some(target) = battle.units[owner].cipher_state.patron.filter(|target| {
            battle.attack_targets().contains(target) && battle.is_targetable(*target)
        }) else {
            return;
        };
        if self.eidolon >= 4 {
            battle.secondary_hit(
                owner,
                target,
                QUANTUM,
                5_000,
                DamageKind::Additional,
                origin,
            );
        }
        if owner != source
            && !battle.units[owner].cipher_state.follow_up_used
            && battle.is_targetable(target)
            && !battle.is_crowd_controlled(owner)
        {
            battle.units[owner].cipher_state.follow_up_used = true;
            battle.follow_up_single(owner, target, origin, self.follow_up.clone());
        }
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            battle.apply_ally_buff_with_id(
                actor,
                actor,
                AllyBuffKind::AttackPercent,
                8_000,
                2,
                Some(E1_ATTACK),
            );
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
        self.e2_vulnerability(battle, actor, target);
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _origin: Action,
    ) {
        self.e2_vulnerability(battle, actor, target);
    }

    fn damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        35_000 * u32::from(self.eidolon >= 6 && kind == DamageKind::FollowUp)
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        2_500 * self.stride_level(battle, actor) as u32
    }

    fn crit_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        10_000 * u32::from(self.sleight_of_sky && kind == DamageKind::FollowUp)
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _attacker: usize,
    ) -> DamageTargetAura {
        DamageTargetAura {
            vulnerability_bp: 4_000 * u32::from(self.sleight_of_sky && battle.is_targetable(owner)),
            ..DamageTargetAura::default()
        }
    }
}

fn deal_tallied_damage(battle: &mut Battle<'_>, actor: usize, target: usize, amount: u64) {
    if amount > 0 && battle.is_targetable(target) {
        let amount = u32::try_from(amount).unwrap_or(u32::MAX);
        battle.true_damage(actor, target, Action::Ultimate, amount, Vec::new());
    }
}
