use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, combat_status::DebuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1317";
const BASIC: &str = "skill.131701";
const SKILL: &str = "skill.131702";
const ULTIMATE: &str = "skill.131703";
const TALENT: &str = "skill.131704";
const ENHANCED_BASIC: &str = "skill.131718";
const BLAST_OPENING: &str = "skill.131708";
const BLAST_SECOND: &str = "skill.131710";
const AREA_FINISH: &str = "skill.131712";
const A2: &str = "trace.1317101";
const A4: &str = "trace.1317102";
const A6: &str = "trace.1317103";
const IMAGINARY: Element = Element::Imaginary;
const INK_CAP: u8 = 3;
const CHARGE_CAP: u8 = 10;
const E6_CHARGE: u8 = 5;

#[derive(Clone, Copy, Default)]
pub(in crate::battle) struct RappaState {
    ink: u8,
    charge: u8,
}

struct Rappa {
    eidolon: u8,
    sky_high: bool,
    sea_echo: bool,
    withered_leaf: bool,
    efficiency_bp: u32,
    seal_break_effect_bp: u32,
    off_element_bp: u16,
    talent_base_bp: u32,
    talent_per_charge_bp: u32,
    blast_bp: u16,
    adjacent_bp: u16,
    area_bp: u16,
    enhanced_basic: OfficialAbility,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != IMAGINARY
    {
        return Err(invalid("energy capacity, eidolon or combat type"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 1)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 6)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 4)?;
    let blast_opening = input.parameters(BLAST_OPENING, 4)?;
    let blast_second = input.parameters(BLAST_SECOND, 2)?;
    let area_finish = input.parameters(AREA_FINISH, 1)?;
    if ultimate[0] != 0.5 || ultimate[2] != f64::from(INK_CAP) {
        return Err(invalid(ULTIMATE));
    }
    if talent[0] != f64::from(CHARGE_CAP)
        || talent[1] != 0.0
        || talent[3] != 2.0
        || talent[5] != 1.0
    {
        return Err(invalid(TALENT));
    }
    if enhanced[3] != 0.5
        || blast_opening != enhanced
        || blast_second != &enhanced[..2]
        || area_finish[0] != enhanced[2]
    {
        return Err(invalid(ENHANCED_BASIC));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 30_000,
            effects: vec![hit(basis_points(SKILL, skill[0])?, 10)],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Rappa {
            eidolon: input.eidolon,
            sky_high: has_trace(A2),
            sea_echo: has_trace(A4),
            withered_leaf: has_trace(A6),
            efficiency_bp: u32::from(basis_points(ULTIMATE, ultimate[0])?),
            seal_break_effect_bp: u32::from(basis_points(ULTIMATE, ultimate[1])?),
            off_element_bp: basis_points(ENHANCED_BASIC, enhanced[3])?,
            talent_base_bp: u32::from(basis_points(TALENT, talent[2])?),
            talent_per_charge_bp: u32::from(basis_points(TALENT, talent[4])?),
            blast_bp: basis_points(BLAST_OPENING, blast_opening[0])?,
            adjacent_bp: basis_points(BLAST_SECOND, blast_second[1])?,
            area_bp: basis_points(AREA_FINISH, area_finish[0])?,
            enhanced_basic: OfficialAbility {
                target: Target::Enemy,
                energy_milli: 20_000,
                effects: vec![Effect::DirectedAreaAttack],
                skill_points: SkillPoints::Unchanged,
            },
        }),
    })
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {id}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: IMAGINARY,
        multiplier_bp,
        toughness,
    }
}

fn sealed(battle: &Battle<'_>, actor: usize) -> bool {
    battle.units[actor].rappa_state.ink > 0
}

fn set_ink(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let previous = &mut battle.units[actor].rappa_state.ink;
    let before = std::mem::replace(previous, after);
    if before != after {
        battle.emit(
            EventKind::ChromaInk { before, after },
            Some(actor),
            Some(actor),
        );
    }
}

fn set_charge(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let previous = &mut battle.units[actor].rappa_state.charge;
    let before = std::mem::replace(previous, after);
    if before != after {
        battle.emit(
            EventKind::RappaCharge { before, after },
            Some(actor),
            Some(actor),
        );
    }
}

impl Rappa {
    fn add_charge(&self, battle: &mut Battle<'_>, actor: usize, amount: u8) {
        let cap = CHARGE_CAP + if self.eidolon >= 6 { E6_CHARGE } else { 0 };
        let next = (battle.units[actor].rappa_state.charge + amount).min(cap);
        set_charge(battle, actor, next);
    }

    fn apply_talent_hit(&self, battle: &mut Battle<'_>, actor: usize) {
        // Finish the direct part of the third strike before sampling Charge. Breaks
        // caused by the additional damage or its Toughness belong to the next attack.
        let charge = battle.units[actor].rappa_state.charge;
        set_charge(battle, actor, 0);
        let ratio_bp = self.talent_base_bp + self.talent_per_charge_bp * u32::from(charge);
        let toughness = 2 + u16::from(charge);
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                battle.attack_break_hit(
                    actor,
                    target,
                    IMAGINARY,
                    Action::Basic,
                    ratio_bp,
                    toughness,
                );
            }
        }
        if self.eidolon >= 6 {
            self.add_charge(battle, actor, E6_CHARGE);
        }
    }
}

impl Mechanics for Rappa {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 6 {
            self.add_charge(battle, actor, E6_CHARGE);
        }
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !sealed(battle, actor)
    }

    fn can_use_ultimate(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !sealed(battle, actor)
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && sealed(battle, actor)).then_some(&self.enhanced_basic)
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Ultimate {
            set_ink(battle, actor, INK_CAP);
            battle.queue_extra_turn(actor);
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let adjacent: Vec<_> = battle.adjacent_enemies(target).collect();
        let primary_toughness = if self.eidolon >= 2 { 15 } else { 10 };
        for _ in 0..2 {
            battle.attack_hit(actor, target, IMAGINARY, self.blast_bp, primary_toughness);
            for &neighbor in &adjacent {
                battle.attack_hit(actor, neighbor, IMAGINARY, self.adjacent_bp, 5);
            }
        }
        for target in battle.enemies() {
            if battle.is_targetable(target) {
                battle.attack_hit(actor, target, IMAGINARY, self.area_bp, 5);
            }
        }
        self.apply_talent_hit(battle, actor);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if actor != owner || action != Action::Basic || !sealed(battle, owner) {
            return;
        }
        // Keep Sealform live for Super Break until the last enhanced Basic is settled.
        let after = battle.units[owner].rappa_state.ink - 1;
        set_ink(battle, owner, after);
        if after == 0 && self.eidolon >= 1 {
            battle.gain_energy(owner, owner, 20_000);
        }
    }

    fn off_element_toughness_bp(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> u16 {
        self.off_element_bp
            * u16::from(
                owner == actor && battle.action == Some(Action::Basic) && sealed(battle, owner),
            )
    }

    fn toughness_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        self.efficiency_bp * u32::from(action == Some(Action::Basic) && sealed(battle, actor))
    }

    fn break_effect_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        self.seal_break_effect_bp * u32::from(sealed(battle, actor))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _target: usize) -> u32 {
        1_500 * u32::from(self.eidolon >= 1 && sealed(battle, actor))
    }

    fn ally_speed_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        1_200 * u32::from(self.eidolon >= 4 && sealed(battle, owner))
    }

    fn provides_super_break(&self) -> bool {
        self.sea_echo
    }

    fn super_break_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
        _target: usize,
    ) -> u16 {
        6_000
            * u16::from(
                self.sea_echo
                    && owner == actor
                    && battle.action == Some(Action::Basic)
                    && sealed(battle, owner),
            )
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
        _origin: Action,
    ) {
        if !battle.allies().contains(&source) || !battle.is_targetable(owner) {
            return;
        }
        let elite_bonus = self.sky_high && battle.is_elite(target);
        self.add_charge(battle, owner, 1 + u8::from(elite_bonus));
        if elite_bonus {
            battle.gain_energy(owner, owner, 10_000);
        }
        if self.withered_leaf && battle.is_targetable(target) {
            let attack = battle.effective_attack(owner);
            let additional_bp = (attack.saturating_sub(2_400) / 100).min(8) * 100;
            battle.apply_debuff(
                owner,
                target,
                DebuffKind::WitheredLeaf,
                200 + additional_bp,
                2,
            );
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
