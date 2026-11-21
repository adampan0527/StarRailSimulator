use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Ability, Effect, Element, Target},
};

use super::{
    super::{Battle, SummonTurn, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1112";
const BASIC: &str = "skill.111201";
const SKILL: &str = "skill.111202";
const ULTIMATE: &str = "skill.111203";
const TALENT: &str = "skill.111204";
const OVERDRAFT: &str = "trace.1112101";
const FINANCIAL_TURMOIL: &str = "trace.1112102";
const STONKS_MARKET: &str = "trace.1112103";
const NUMBY_SPEED_MICROS: u64 = 80_000_000;
const ENHANCED_ATTACKS: u8 = 2;

#[derive(Clone, Copy)]
struct ProofOfDebt {
    target: usize,
    debtor_stacks: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct TopazState {
    proof: Option<ProofOfDebt>,
    enhanced_attacks: u8,
    numby_turn: bool,
}

struct Topaz {
    eidolon: u8,
    overdraft: bool,
    financial_turmoil: bool,
    stonks_market: bool,
    follow_up_vulnerability_bp: u16,
    enhanced_multiplier_bp: u16,
    enhanced_crit_damage_bp: u16,
    enhanced_advance_bp: u16,
    follow_up_advance_bp: u16,
    follow_up: FollowUpAttack,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130 {
        return Err(invalid("energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 3)?;
    if talent[0] != 80.0 || ultimate[3] != f64::from(ENHANCED_ATTACKS) {
        return Err(invalid("Numby speed or enhanced attack count"));
    }
    let enhanced_multiplier_bp = basis_points(ULTIMATE, ultimate[0])?;
    let skill_multiplier_bp = basis_points(SKILL, skill[0])?;
    let talent_multiplier_bp = basis_points(TALENT, talent[1])?;
    if skill_multiplier_bp
        .max(talent_multiplier_bp)
        .checked_add(enhanced_multiplier_bp)
        .is_none()
    {
        return Err(invalid("enhanced Numby multiplier"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: attack(
            basis_points(BASIC, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(skill_multiplier_bp, 20, 30_000, SkillPoints::Spend),
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Topaz {
            eidolon: input.eidolon,
            overdraft: trace(OVERDRAFT),
            financial_turmoil: trace(FINANCIAL_TURMOIL),
            stonks_market: trace(STONKS_MARKET),
            follow_up_vulnerability_bp: basis_points(SKILL, skill[1])?,
            enhanced_multiplier_bp,
            enhanced_crit_damage_bp: basis_points(ULTIMATE, ultimate[1])?,
            enhanced_advance_bp: advance_basis_points(ULTIMATE, ultimate[2])?,
            follow_up_advance_bp: advance_basis_points(TALENT, talent[2])?,
            follow_up: FollowUpAttack {
                element: Element::Fire,
                multiplier_bp: talent_multiplier_bp,
                toughness: 20,
                energy_milli: 0,
                first_hit: None,
            },
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(&format!("{skill} multiplier")));
    }
    Ok(rounded as u16)
}

fn advance_basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = basis_points(skill, ratio)?;
    if bp > 10_000 {
        return Err(invalid(&format!("{skill} action advance")));
    }
    Ok(bp)
}

fn attack(
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Fire,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

impl Topaz {
    fn mark(battle: &mut Battle<'_>, owner: usize, target: usize) {
        let proof = &mut battle.units[owner].topaz_state.proof;
        if !proof.is_some_and(|proof| proof.target == target) {
            *proof = Some(ProofOfDebt {
                target,
                debtor_stacks: 0,
            });
            battle.record_enemy_debuff(target);
        }
    }

    fn ensure_proof(battle: &mut Battle<'_>, owner: usize) -> Option<usize> {
        if battle.units[owner].hp == 0 {
            return None;
        }
        if let Some(proof) = battle.units[owner].topaz_state.proof
            && battle.enemies().contains(&proof.target)
            && battle.is_targetable(proof.target)
        {
            return Some(proof.target);
        }
        let target = battle.random_living_enemy()?;
        Self::mark(battle, owner, target);
        Some(target)
    }

    fn read_proof(battle: &Battle<'_>, owner: usize, target: usize) -> Option<ProofOfDebt> {
        battle.units[owner]
            .topaz_state
            .proof
            .filter(|proof| proof.target == target)
    }

    fn is_enhanced_numby_attack(
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> bool {
        battle.units[actor].topaz_state.enhanced_attacks > 0
            && ((kind == DamageKind::Direct && action == Some(Action::Skill))
                || (kind == DamageKind::FollowUp && battle.units[actor].topaz_state.numby_turn))
    }

    fn finish_numby_attack(&self, battle: &mut Battle<'_>, actor: usize, independent: bool) {
        let state = &mut battle.units[actor].topaz_state;
        let enhanced = state.enhanced_attacks > 0;
        state.enhanced_attacks = state.enhanced_attacks.saturating_sub(1);
        let refund = u32::from(enhanced && self.stonks_market) * 10_000
            + u32::from(independent && self.eidolon >= 2) * 5_000;
        if refund > 0 {
            battle.gain_energy(actor, actor, refund);
        }
    }
}

impl Mechanics for Topaz {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].summon_turn = Some(SummonTurn::new("numby", NUMBY_SPEED_MICROS));
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, _actor: usize) {
        Self::ensure_proof(battle, owner);
    }

    fn before_ally_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _actor: usize,
        _target: Option<usize>,
        _ability: Ability<'_>,
    ) {
        Self::ensure_proof(battle, owner);
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => Self::mark(battle, actor, target),
            (Action::Ultimate, _) => {
                battle.units[actor].topaz_state.enhanced_attacks =
                    ENHANCED_ATTACKS + u8::from(self.eidolon >= 6);
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
        if action == Action::Skill {
            self.finish_numby_attack(battle, actor, false);
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(target) = Self::ensure_proof(battle, owner) else {
            return;
        };
        if self.eidolon >= 4 {
            battle.advance(owner, 2_000);
        }
        battle.units[owner].topaz_state.numby_turn = true;
        // The ledger's action enum has no Talent; the autonomous hit is distinguished by FollowUp.
        battle.follow_up_single(owner, target, Action::Skill, self.follow_up.clone());
        battle.resolve_follow_ups();
        battle.units[owner].topaz_state.numby_turn = false;
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        if battle.units[actor].topaz_state.numby_turn {
            self.finish_numby_attack(battle, actor, true);
        }
    }

    fn on_first_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        target: usize,
        primary: bool,
        follow_up: bool,
    ) {
        // A queued follow-up can start after an attack that defeated the previous debtor.
        if battle.attack_targets().len() == 1 {
            Self::ensure_proof(battle, owner);
        }
        let unit = &mut battle.units[owner];
        let state = &mut unit.topaz_state;
        let Some(proof) = &mut state.proof else {
            return;
        };
        if proof.target != target {
            return;
        }
        let applied_debtor = self.eidolon >= 1 && follow_up && proof.debtor_stacks < 2;
        if applied_debtor {
            proof.debtor_stacks += 1;
        }
        if !state.numby_turn
            && let Some(summon) = &mut unit.summon_turn
        {
            let talent = u16::from(follow_up) * self.follow_up_advance_bp;
            let ultimate =
                u16::from(primary && state.enhanced_attacks > 0) * self.enhanced_advance_bp;
            if talent + ultimate > 0 {
                summon.advance(talent + ultimate);
            }
        }
        if applied_debtor {
            battle.record_enemy_debuff(target);
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if Self::read_proof(battle, owner, target).is_some() {
            battle.units[owner].topaz_state.proof = None;
        }
    }

    fn primary_attack_counts_as_follow_up(&self, action: Action) -> bool {
        action == Action::Skill || (action == Action::Basic && self.overdraft)
    }

    fn follow_up_vulnerability_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        u32::from(Self::read_proof(battle, owner, target).is_some())
            * u32::from(self.follow_up_vulnerability_bp)
    }

    fn additional_debuff_count(&self, battle: &Battle<'_>, owner: usize, target: usize) -> usize {
        Self::read_proof(battle, owner, target)
            .map_or(0, |proof| 1 + usize::from(proof.debtor_stacks > 0))
    }

    fn follow_up_crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        target: usize,
    ) -> u32 {
        Self::read_proof(battle, owner, target)
            .map_or(0, |proof| u32::from(proof.debtor_stacks) * 2_500)
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        if self.financial_turmoil && battle.has_weakness(target, Element::Fire) {
            1_500
        } else {
            0
        }
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if Self::is_enhanced_numby_attack(battle, actor, action, kind) {
            base + u32::from(self.enhanced_multiplier_bp)
        } else {
            base
        }
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if Self::is_enhanced_numby_attack(battle, actor, action, kind) {
            u32::from(self.enhanced_crit_damage_bp)
        } else {
            0
        }
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        let state = &battle.units[actor].topaz_state;
        if self.eidolon >= 6
            && element == Element::Fire
            && state.enhanced_attacks > 0
            && (state.numby_turn || battle.current_action() == Some(Action::Skill))
        {
            1_000
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Ultimate).then_some(0)
    }
}
