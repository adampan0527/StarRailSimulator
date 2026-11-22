use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1201";
const BASIC: &str = "skill.120101";
const SKILL: &str = "skill.120102";
const ULTIMATE: &str = "skill.120103";
const TALENT: &str = "skill.120104";
const ENHANCED_BASIC: &str = "skill.120108";
const TILE_BATTLE: &str = "trace.1201101";
const BIDE_TIME: &str = "trace.1201102";
const WINNING_HAND: &str = "trace.1201103";
const QUANTUM: Element = Element::Quantum;
const SUITS: usize = 3;
const MAX_TILES: u8 = 4;
const SKILL_DRAWS: u8 = 2;
const MAX_DAMAGE_STACKS: u8 = 4;
const BASIC_TOUGHNESS: u16 = 10;
const ENHANCED_TOUGHNESS: u16 = 20;
const ADJACENT_TOUGHNESS: u16 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JadeHand {
    Tiles([u8; SUITS]),
    Hidden,
}

impl Default for JadeHand {
    fn default() -> Self {
        Self::Tiles([0; SUITS])
    }
}

impl JadeHand {
    fn is_complete(self) -> bool {
        match self {
            Self::Tiles(suits) => suits.contains(&MAX_TILES),
            Self::Hidden => true,
        }
    }

    fn discard(suits: &mut [u8; SUITS]) {
        if let Some(count) = suits.iter_mut().filter(|count| **count > 0).min() {
            *count -= 1;
        }
    }
}

#[derive(Clone, Default)]
pub(in crate::battle) struct QingqueState {
    hand: JadeHand,
    damage_stacks: u8,
    first_skill_refunded: bool,
    autarky: bool,
}

struct Qingque {
    basic_bp: u16,
    enhanced_bp: u16,
    adjacent_bp: u16,
    enhanced_basic: OfficialAbility,
    skill_damage_bp: u32,
    hidden_attack_bp: u32,
    tile_battle: bool,
    winning_hand: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != QUANTUM
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, combat type or eidolon"));
    }

    let basic_bp = basis_points(BASIC, input.parameters(BASIC, 1)?[0])?;
    let ultimate_bp = basis_points(ULTIMATE, input.parameters(ULTIMATE, 1)?[0])?;
    let hidden_attack_bp = u32::from(basis_points(TALENT, input.parameters(TALENT, 1)?[0])?);
    let skill = input.parameters(SKILL, 3)?;
    if skill[0] != f64::from(SKILL_DRAWS) || skill[2] != f64::from(MAX_DAMAGE_STACKS) {
        return Err(invalid(SKILL));
    }
    let enhanced = input.parameters(ENHANCED_BASIC, 2)?;
    let enhanced_bp = basis_points(ENHANCED_BASIC, enhanced[0])?;
    let adjacent_bp = basis_points(ENHANCED_BASIC, enhanced[1])?;
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: basic(vec![hit(basic_bp, BASIC_TOUGHNESS)], SkillPoints::Generate),
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 0,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(ultimate_bp, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Qingque {
            basic_bp,
            enhanced_bp,
            adjacent_bp,
            enhanced_basic: basic(
                vec![
                    hit(enhanced_bp, ENHANCED_TOUGHNESS),
                    Effect::BlastAdjacent {
                        element: QUANTUM,
                        multiplier_bp: adjacent_bp,
                        toughness: ADJACENT_TOUGHNESS,
                    },
                ],
                SkillPoints::Unchanged,
            ),
            skill_damage_bp: u32::from(basis_points(SKILL, skill[1])?)
                + u32::from(trace(BIDE_TIME)) * 1_000,
            hidden_attack_bp,
            tile_battle: trace(TILE_BATTLE),
            winning_hand: trace(WINNING_HAND),
            eidolon: input.eidolon,
        }),
    })
}

fn basic(effects: Vec<Effect>, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli: 20_000,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: QUANTUM,
        multiplier_bp,
        toughness,
    }
}

fn invalid(field: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {field}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(skill));
    }
    Ok(bp as u16)
}

impl Qingque {
    fn set_hand(battle: &mut Battle<'_>, owner: usize, source: usize, hand: JadeHand) {
        let before = std::mem::replace(&mut battle.units[owner].qingque_state.hand, hand);
        if before != hand {
            let (suits, hidden_hand) = match hand {
                JadeHand::Tiles(suits) => (suits, false),
                JadeHand::Hidden => ([0; SUITS], true),
            };
            battle.emit(
                EventKind::JadeTiles { suits, hidden_hand },
                Some(source),
                Some(owner),
            );
        }
    }

    fn draw_tiles(&self, battle: &mut Battle<'_>, owner: usize, source: usize, count: u8) {
        let hand = battle.units[owner].qingque_state.hand;
        let JadeHand::Tiles(mut suits) = hand else {
            return;
        };
        if hand.is_complete() {
            return;
        }
        for _ in 0..count {
            suits[battle.random_index(SUITS)] += 1;
            if suits.iter().sum::<u8>() > MAX_TILES {
                JadeHand::discard(&mut suits);
            }
            if suits.contains(&MAX_TILES) {
                break;
            }
        }
        Self::set_hand(battle, owner, source, JadeHand::Tiles(suits));
        if self.eidolon >= 2 {
            battle.gain_energy(owner, owner, 1_000);
        }
    }

    fn enter_hidden_hand(battle: &mut Battle<'_>, actor: usize) {
        if battle.units[actor].qingque_state.hand.is_complete() {
            Self::set_hand(battle, actor, actor, JadeHand::Hidden);
        }
    }

    fn draw_skill(&self, battle: &mut Battle<'_>, actor: usize) {
        let state = &mut battle.units[actor].qingque_state;
        state.damage_stacks = (state.damage_stacks + 1).min(MAX_DAMAGE_STACKS);
        if self.tile_battle && !state.first_skill_refunded {
            state.first_skill_refunded = true;
            battle.restore_skill_points(actor, 1);
        }
        self.draw_tiles(battle, actor, actor, SKILL_DRAWS);
        if self.eidolon >= 4 && battle.roll_fixed(2_400) {
            battle.units[actor].qingque_state.autarky = true;
        }
        Self::enter_hidden_hand(battle, actor);
    }

    fn finish_basic(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let state = &mut battle.units[actor].qingque_state;
        let hidden = state.hand == JadeHand::Hidden;
        let autarky = std::mem::take(&mut state.autarky);
        if hidden {
            // The chained attack inherits skill stacks, but Hidden Hand ends with the basic.
            Self::set_hand(battle, actor, actor, JadeHand::default());
            if self.eidolon >= 6 {
                battle.restore_skill_points(actor, 1);
            }
            if self.winning_hand {
                battle.apply_ally_buff_with_id(
                    actor,
                    actor,
                    AllyBuffKind::SpeedPercent,
                    1_000,
                    1,
                    Some(WINNING_HAND),
                );
            }
        }
        if autarky {
            let attack = FollowUpAttack {
                element: QUANTUM,
                multiplier_bp: if hidden {
                    self.enhanced_bp
                } else {
                    self.basic_bp
                },
                toughness: if hidden {
                    ENHANCED_TOUGHNESS
                } else {
                    BASIC_TOUGHNESS
                },
                energy_milli: 0,
                first_hit: None,
            };
            if hidden {
                battle.follow_up_blast(
                    actor,
                    target,
                    Action::Basic,
                    attack,
                    (self.adjacent_bp, ADJACENT_TOUGHNESS),
                );
            } else {
                battle.follow_up_single(actor, target, Action::Basic, attack);
            }
        }
    }
}

impl Mechanics for Qingque {
    fn keeps_turn(&self, action: Action) -> bool {
        action == Action::Skill
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        !battle.units[actor].qingque_state.hand.is_complete()
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && battle.units[actor].qingque_state.hand == JadeHand::Hidden)
            .then_some(&self.enhanced_basic)
    }

    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if !battle.is_targetable(owner) {
            return;
        }
        self.draw_tiles(battle, owner, actor, 1);
        if owner == actor {
            Self::enter_hidden_hand(battle, owner);
        }
    }

    fn on_extra_turn_start(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::enter_hidden_hand(battle, actor);
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if let (Action::Basic, JadeHand::Tiles(mut suits)) =
            (action, battle.units[actor].qingque_state.hand)
        {
            JadeHand::discard(&mut suits);
            Self::set_hand(battle, actor, actor, JadeHand::Tiles(suits));
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::ElationSkill, _) => {}
            (Action::Skill, _) => self.draw_skill(battle, actor),
            (Action::Basic, Some(target)) => self.finish_basic(battle, actor, target),
            (Action::Ultimate, _)
                if battle.ordinary_actor == Some(actor)
                    || battle.interrupted_turn_actor() == Some(actor) =>
            {
                Self::set_hand(battle, actor, actor, JadeHand::Hidden);
            }
            (Action::Ultimate, _) => {
                Self::set_hand(battle, actor, actor, JadeHand::Tiles([MAX_TILES, 0, 0]))
            }
            (Action::Basic, None) | (Action::EnhancedUltimate, _) => {}
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if actor == owner && action == Action::Basic {
            battle.units[actor].qingque_state.damage_stacks = 0;
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner == target && battle.units[owner].qingque_state.hand == JadeHand::Hidden {
            self.hidden_attack_bp
        } else {
            0
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let skill =
            self.skill_damage_bp * u32::from(battle.units[actor].qingque_state.damage_stacks);
        let ultimate = u32::from(
            self.eidolon >= 1 && action == Some(Action::Ultimate) && kind == DamageKind::Direct,
        ) * 1_000;
        skill + ultimate
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::ElationSkill => None,
            Action::Skill => Some(2),
            Action::Ultimate if battle.ordinary_actor != Some(actor) => Some(5),
            Action::Ultimate => {
                let state = &battle.units[actor].qingque_state;
                let keep_drawing = !state.hand.is_complete()
                    && battle.skill_points > 0
                    && state.damage_stacks < MAX_DAMAGE_STACKS;
                Some(if keep_drawing { 4 } else { 0 })
            }
            Action::Basic | Action::EnhancedUltimate => None,
        }
    }
}
