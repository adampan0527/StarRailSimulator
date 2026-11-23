use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1213";
const BASIC: &str = "skill.121301";
const SKILL: &str = "skill.121302";
const ULTIMATE: &str = "skill.121303";
const TALENT: &str = "skill.121304";
const TRANSCENDENCE: &str = "skill.121308";
const DIVINE_SPEAR: &str = "skill.121310";
const FULGURANT_LEAP: &str = "skill.121312";
const STAR_VEIL: &str = "trace.1213101";
const AQUA_REIGN: &str = "trace.1213102";
const JOLT_ANEW: &str = "trace.1213103";
const IMAGINARY: Element = Element::Imaginary;
const SQUAMA_CAP: u8 = 3;
const OUTROAR_CAP: u8 = 4;
const RIGHTEOUS_HEART_CAP: u8 = 6;
const E1_EXTRA_CAP: u8 = 4;
const E6_PENETRATION_BP: u32 = 2_000;
const E6_CAP: u8 = 3;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum BasicTier {
    #[default]
    Lotus,
    Transcendence,
    Spear,
    Leap,
}

impl BasicTier {
    const fn next(self) -> Self {
        match self {
            Self::Lotus => Self::Transcendence,
            Self::Transcendence => Self::Spear,
            Self::Spear => Self::Leap,
            Self::Leap => Self::Lotus,
        }
    }

    const fn cost(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Default)]
pub(in crate::battle) struct DanHengIlState {
    tier: BasicTier,
    squama: u8,
    righteous_heart: u8,
    outroar: u8,
    outroar_until_turn: u32,
    e6_stacks: u8,
    active_penetration_bp: u32,
}

#[derive(Clone, Copy)]
struct Hit {
    multiplier_bp: u16,
    toughness: u16,
}

#[derive(Clone, Copy)]
struct Strike {
    main: Hit,
    adjacent: Option<Hit>,
}

struct DanHengIl {
    enhanced_basics: [OfficialAbility; 3],
    basics: [Vec<Strike>; 4],
    ultimate: Vec<Strike>,
    righteous_heart_bp: u32,
    outroar_bp: u32,
    start_energy: bool,
    control_resistance: bool,
    weakness_crit: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140
        || input.combat_type.path != CombatPath::Destruction
        || input.combat_type.element != IMAGINARY
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, combat type or eidolon"));
    }

    let skill = input.parameters(SKILL, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let spear = input.parameters(DIVINE_SPEAR, 2)?;
    let leap = input.parameters(FULGURANT_LEAP, 2)?;
    if !(0.0..=1.0).contains(&skill[0])
        || !(0.0..=1.0).contains(&talent[0])
        || skill[1] != f64::from(OUTROAR_CAP)
        || talent[1] != f64::from(RIGHTEOUS_HEART_CAP)
        || ultimate[2] != 2.0
        || ultimate[3] != f64::from(SQUAMA_CAP)
    {
        return Err(invalid("stack parameters"));
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: attack(20_000, SkillPoints::Generate, false),
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 0,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        ultimate: attack(5_000, SkillPoints::Unchanged, true),
        mechanics: Arc::new(DanHengIl {
            enhanced_basics: [(30_000, false), (35_000, true), (40_000, true)]
                .map(|(energy, adjacent)| attack(energy, SkillPoints::Unchanged, adjacent)),
            basics: [
                strikes(BASIC, (input.parameters(BASIC, 1)?[0], 10, &[30, 70]), None)?,
                strikes(
                    TRANSCENDENCE,
                    (input.parameters(TRANSCENDENCE, 1)?[0], 20, &[33, 33, 34]),
                    None,
                )?,
                strikes(
                    DIVINE_SPEAR,
                    (spear[0], 30, &[20; 5]),
                    Some((spear[1], 10, &[50, 50])),
                )?,
                strikes(
                    FULGURANT_LEAP,
                    (leap[0], 40, &[142, 142, 142, 142, 142, 142, 148]),
                    Some((leap[1], 20, &[25; 4])),
                )?,
            ],
            ultimate: strikes(
                ULTIMATE,
                (ultimate[0], 20, &[30, 30, 40]),
                Some((ultimate[1], 20, &[30, 30, 40])),
            )?,
            righteous_heart_bp: basis_points(TALENT, talent[0])?,
            outroar_bp: basis_points(SKILL, skill[0])?,
            start_energy: trace(STAR_VEIL),
            control_resistance: trace(AQUA_REIGN),
            weakness_crit: trace(JOLT_ANEW),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u32, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u32::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u32)
}

fn attack(energy_milli: u32, skill_points: SkillPoints, adjacent: bool) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects: vec![Effect::DirectedAttack { adjacent }],
        skill_points,
    }
}

fn strikes(
    skill: &str,
    main: (f64, u16, &[u16]),
    adjacent: Option<(f64, u16, &[u16])>,
) -> Result<Vec<Strike>, BattleError> {
    let (ratio, toughness, weights) = main;
    let mut strikes = split_hits(skill, ratio, toughness, weights)?
        .map(|main| Strike {
            main,
            adjacent: None,
        })
        .collect::<Vec<_>>();
    if let Some((ratio, toughness, weights)) = adjacent {
        let start = strikes.len() - weights.len();
        strikes[start..]
            .iter_mut()
            .zip(split_hits(skill, ratio, toughness, weights)?)
            .for_each(|(strike, hit)| strike.adjacent = Some(hit));
    }
    Ok(strikes)
}

fn split_hits<'a>(
    skill: &str,
    ratio: f64,
    toughness: u16,
    weights: &'a [u16],
) -> Result<impl Iterator<Item = Hit> + 'a, BattleError> {
    let multiplier = u64::from(basis_points(skill, ratio)?);
    let total = weights.iter().map(|&weight| u64::from(weight)).sum::<u64>();
    let max_weight = u64::from(weights.iter().copied().max().unwrap_or_default());
    if (multiplier * max_weight).div_ceil(total) > u64::from(u16::MAX) {
        return Err(invalid(skill));
    }
    let mut cumulative = 0;
    Ok(weights.iter().map(move |&weight| {
        let previous = cumulative;
        cumulative += u64::from(weight);
        let split = |value| (value * cumulative / total - value * previous / total) as u16;
        Hit {
            multiplier_bp: split(multiplier),
            toughness: split(u64::from(toughness)),
        }
    }))
}

impl DanHengIl {
    fn affordable_tier(battle: &Battle<'_>, actor: usize) -> u8 {
        (battle.skill_points + battle.units[actor].dan_heng_il_state.squama)
            .min(BasicTier::Leap.cost())
    }

    fn set_tier(battle: &mut Battle<'_>, actor: usize, tier: BasicTier) {
        let before = std::mem::replace(&mut battle.units[actor].dan_heng_il_state.tier, tier);
        if before != tier {
            battle.emit(
                EventKind::BasicEnhancement {
                    before: before.cost(),
                    after: tier.cost(),
                },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn set_squama(battle: &mut Battle<'_>, actor: usize, after: u8) {
        let before = std::mem::replace(&mut battle.units[actor].dan_heng_il_state.squama, after);
        if before != after {
            battle.emit(
                EventKind::SquamaSacrosancta { before, after },
                Some(actor),
                Some(actor),
            );
        }
    }

    fn finish_turn(battle: &mut Battle<'_>, actor: usize) {
        let unit = &mut battle.units[actor];
        let state = &mut unit.dan_heng_il_state;
        state.righteous_heart = 0;
        if unit.normal_turns >= state.outroar_until_turn {
            state.outroar = 0;
        }
        Self::set_tier(battle, actor, BasicTier::Lotus);
    }

    fn strike(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        main: usize,
        target: Strike,
        add_outroar: bool,
    ) {
        let adjacent_hit =
            target.adjacent.is_some() && battle.adjacent_enemies(main).next().is_some();
        if !battle.is_targetable(main) && !adjacent_hit {
            return;
        }
        if add_outroar {
            let unit = &mut battle.units[actor];
            let state = &mut unit.dan_heng_il_state;
            state.outroar = (state.outroar + 1).min(OUTROAR_CAP);
            state.outroar_until_turn = unit.normal_turns + u32::from(self.eidolon >= 4);
        }
        battle.attack_hit(
            actor,
            main,
            IMAGINARY,
            target.main.multiplier_bp,
            target.main.toughness,
        );
        if let Some(hit) = target.adjacent {
            for neighbor in battle.adjacent_enemies(main) {
                battle.attack_hit(actor, neighbor, IMAGINARY, hit.multiplier_bp, hit.toughness);
            }
        }
        let state = &mut battle.units[actor].dan_heng_il_state;
        let extra = u8::from(self.eidolon >= 1);
        let cap = RIGHTEOUS_HEART_CAP + extra * E1_EXTRA_CAP;
        state.righteous_heart = (state.righteous_heart + 1 + extra).min(cap);
    }
}

impl Mechanics for DanHengIl {
    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn prepare_action(&self, battle: &mut Battle<'_>, actor: usize, action: Action) -> bool {
        if action != Action::Skill {
            return false;
        }
        let next = battle.units[actor].dan_heng_il_state.tier.next();
        let tier = if next.cost() <= Self::affordable_tier(battle, actor) {
            next
        } else {
            BasicTier::Lotus
        };
        Self::set_tier(battle, actor, tier);
        true
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        Self::affordable_tier(battle, actor) > 0
            || battle.units[actor].dan_heng_il_state.tier != BasicTier::Lotus
    }

    fn shared_skill_point_cost(&self, battle: &Battle<'_>, actor: usize, action: Action) -> u8 {
        if action == Action::Basic {
            let state = &battle.units[actor].dan_heng_il_state;
            state.tier.cost().saturating_sub(state.squama)
        } else {
            0
        }
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        if action != Action::Basic {
            return None;
        }
        let tier = battle.units[actor].dan_heng_il_state.tier.cost();
        self.enhanced_basics.get(usize::from(tier.checked_sub(1)?))
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        let state = &battle.units[actor].dan_heng_il_state;
        let cost = state.tier.cost();
        if action == Action::Basic && cost > 0 {
            let squama = state.squama.min(cost);
            Self::set_squama(battle, actor, state.squama - squama);
            battle.spend_skill_points_with_substitute(actor, cost - squama, squama);
        }
    }

    fn resolve_special_attack(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let tier = battle.units[actor].dan_heng_il_state.tier;
        let is_ultimate = battle.action.is_some_and(Action::is_ultimate);
        let strikes = if is_ultimate {
            &self.ultimate
        } else {
            &self.basics[usize::from(tier.cost())]
        };
        if !is_ultimate && tier == BasicTier::Leap {
            let state = &mut battle.units[actor].dan_heng_il_state;
            state.active_penetration_bp =
                u32::from(std::mem::take(&mut state.e6_stacks)) * E6_PENETRATION_BP;
        }
        for &strike in strikes {
            self.strike(
                battle,
                actor,
                target,
                strike,
                !is_ultimate && strike.adjacent.is_some(),
            );
        }
        battle.units[actor].dan_heng_il_state.active_penetration_bp = 0;
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        match action {
            Action::AssistSkill | Action::ElationSkill => {}
            Action::Basic => {
                Self::set_tier(battle, actor, BasicTier::Lotus);
                if battle.in_extra_turn(actor) {
                    Self::finish_turn(battle, actor);
                }
            }
            Action::Ultimate => {
                let after =
                    battle.units[actor].dan_heng_il_state.squama + 2 + u8::from(self.eidolon >= 2);
                Self::set_squama(battle, actor, after.min(SQUAMA_CAP));
                if self.eidolon >= 2 {
                    battle.advance(actor, 10_000);
                }
            }
            Action::Skill | Action::EnhancedSkill | Action::EnhancedUltimate => {}
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if self.eidolon == 6 && owner != actor && action.is_ultimate() {
            let state = &mut battle.units[owner].dan_heng_il_state;
            state.e6_stacks = (state.e6_stacks + 1).min(E6_CAP);
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner == actor {
            Self::finish_turn(battle, actor);
        }
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.start_energy {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

    fn control_resistance_bp(&self) -> u32 {
        3_500 * u32::from(self.control_resistance)
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        self.righteous_heart_bp * u32::from(battle.units[actor].dan_heng_il_state.righteous_heart)
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        let outroar = u32::from(battle.units[actor].dan_heng_il_state.outroar);
        self.outroar_bp * outroar
            + 2_400 * u32::from(self.weakness_crit && battle.has_weakness(target, IMAGINARY))
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        if element == IMAGINARY {
            battle.units[actor].dan_heng_il_state.active_penetration_bp
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        let affordable = Self::affordable_tier(battle, actor);
        let selected = battle.units[actor].dan_heng_il_state.tier.cost();
        match action {
            Action::AssistSkill | Action::ElationSkill => None,
            Action::Skill | Action::EnhancedSkill if selected < affordable => Some(1),
            Action::Skill | Action::EnhancedSkill => Some(5),
            Action::Basic => Some(2),
            Action::Ultimate
                if self.eidolon >= 2 && affordable == 3 && battle.ordinary_actor == Some(actor) =>
            {
                Some(4)
            }
            Action::Ultimate => Some(0),
            Action::EnhancedUltimate => None,
        }
    }
}
