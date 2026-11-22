use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, resolve::DamageTargetAura, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1403";
const BASIC: &str = "skill.140301";
const SKILL: &str = "skill.140302";
const ULTIMATE: &str = "skill.140303";
const TALENT: &str = "skill.140304";
const LAMB_OUTSIDE_THE_WALL: &str = "trace.1403101";
const GLASS_BALL_WITH_WINGS: &str = "trace.1403102";
const PEBBLE_AT_CROSSROADS: &str = "trace.1403103";
const ENERGY_CAP: u32 = 120;
const NUMINOSITY_TURNS: u32 = 3;
const ZONE_TURNS: u32 = 2;
const LAMB_DAMAGE_BP: u32 = 7_200;
const LAMB_STACKS: u8 = 3;
const LAMB_TURNS: u32 = 3;
const GLASS_BALL_HP_BP: u32 = 900;
const PEBBLE_STARTING_ENERGY_MILLI: u32 = 30_000;
const PEBBLE_ENERGY_PER_TARGET_MILLI: u32 = 1_500;
const E1_TRUE_DAMAGE_BP: u64 = 2_400;
const E2_ADDITIONAL_RATIO_BP: u32 = 12_000;
const E4_DEFENSE_IGNORE_BP: u32 = 1_800;
const E6_FOLLOW_UP_DAMAGE_BP: u32 = 72_900;

#[derive(Clone, Default)]
pub(in crate::battle) struct TribbieState {
    numinosity_until: u32,
    zone_until: u32,
    zone_target: Option<usize>,
    // Official teams contain at most four slots, each of which may trigger once per zone cast.
    talent_triggers: u8,
    follow_up_next_wave: Option<Action>,
    lamb_stacks: u8,
    lamb_until: u32,
}

struct Tribbie {
    eidolon: u8,
    lamb_outside_the_wall: bool,
    glass_ball_with_wings: bool,
    pebble_at_crossroads: bool,
    penetration_bp: u32,
    vulnerability_bp: u32,
    additional_bp: u16,
    follow_up_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Harmony
        || input.combat_type.element != Element::Quantum
    {
        return Err(invalid("combat type, energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 1)?;
    if skill[1] != f64::from(NUMINOSITY_TURNS) || ultimate[3] != f64::from(ZONE_TURNS) {
        return Err(invalid("skill or ultimate duration"));
    }
    let mut additional_bp = basis_points(ULTIMATE, ultimate[2])?;
    if input.eidolon >= 2 {
        additional_bp = u16::try_from(percentage_of(
            u32::from(additional_bp),
            E2_ADDITIONAL_RATIO_BP,
        ))
        .map_err(|_| invalid("E2 additional multiplier"))?;
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|entry| entry == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![
                Effect::MaxHpHit {
                    element: Element::Quantum,
                    multiplier_bp: basis_points(BASIC, basic[0])?,
                    toughness: 10,
                },
                Effect::MaxHpBlastAdjacent {
                    element: Element::Quantum,
                    multiplier_bp: basis_points(BASIC, basic[1])?,
                    toughness: 5,
                },
            ],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Quantum,
                multiplier_bp: basis_points(ULTIMATE, ultimate[0])?,
                toughness: 20,
            }],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Tribbie {
            eidolon: input.eidolon,
            lamb_outside_the_wall: trace(LAMB_OUTSIDE_THE_WALL),
            glass_ball_with_wings: trace(GLASS_BALL_WITH_WINGS),
            pebble_at_crossroads: trace(PEBBLE_AT_CROSSROADS),
            penetration_bp: u32::from(basis_points(SKILL, skill[0])?),
            vulnerability_bp: u32::from(basis_points(ULTIMATE, ultimate[1])?),
            additional_bp,
            follow_up_bp: basis_points(TALENT, talent[0])?,
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

fn numinosity_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    battle.is_targetable(owner) && unit.normal_turns < unit.tribbie_state.numinosity_until
}

fn zone_active(battle: &Battle<'_>, owner: usize) -> bool {
    let unit = &battle.units[owner];
    battle.is_targetable(owner) && unit.normal_turns < unit.tribbie_state.zone_until
}

fn queue_talent(battle: &mut Battle<'_>, owner: usize, origin: Action) {
    battle.follow_up(owner, Element::Quantum, 0, 5, origin, 5_000);
}

impl Tribbie {
    fn open_zone(&self, battle: &mut Battle<'_>, owner: usize) {
        let was_active = zone_active(battle, owner);
        let hp_before = battle.effective_max_hp(owner);
        let unit = &mut battle.units[owner];
        unit.tribbie_state.zone_until = unit.normal_turns + ZONE_TURNS;
        unit.tribbie_state.talent_triggers = 0;
        let hp_after = battle.effective_max_hp(owner);
        let hp = &mut battle.units[owner].hp;
        *hp = hp
            .saturating_add(hp_after.saturating_sub(hp_before))
            .min(hp_after);

        if !was_active && battle.observes_enemy_debuffs {
            for target in battle.enemies() {
                if battle.is_targetable(target) {
                    battle.record_enemy_debuff(target);
                }
            }
        }
    }
}

impl Mechanics for Tribbie {
    fn tracks_attack_damage(&self) -> bool {
        self.eidolon >= 1
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.pebble_at_crossroads {
            battle.gain_energy(owner, owner, PEBBLE_STARTING_ENERGY_MILLI);
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner != actor || extra {
            return;
        }
        let unit = &mut battle.units[owner];
        if unit.normal_turns >= unit.tribbie_state.lamb_until {
            unit.tribbie_state.lamb_stacks = 0;
        }
        if unit.tribbie_state.zone_until > 0 && unit.normal_turns >= unit.tribbie_state.zone_until {
            unit.tribbie_state.zone_until = 0;
            let max_hp = battle.effective_max_hp(owner);
            battle.units[owner].hp = battle.units[owner].hp.min(max_hp);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let unit = &mut battle.units[owner];
                unit.tribbie_state.numinosity_until = unit.normal_turns + NUMINOSITY_TURNS;
            }
            action if action.is_ultimate() => self.open_zone(battle, owner),
            _ => {}
        }
    }

    fn damage_target_aura(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        actor: usize,
    ) -> DamageTargetAura {
        if !battle.allies().contains(&actor) {
            return DamageTargetAura::default();
        }
        let numinosity = numinosity_active(battle, owner);
        DamageTargetAura {
            vulnerability_bp: self.vulnerability_bp * u32::from(zone_active(battle, owner)),
            resistance_down_bp: self.penetration_bp * u32::from(numinosity),
            defense_down_bp: E4_DEFENSE_IGNORE_BP * u32::from(numinosity && self.eidolon >= 4),
        }
    }

    fn ally_max_hp_bonus(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if owner != target || !self.glass_ball_with_wings || !zone_active(battle, owner) {
            return 0;
        }
        // Exclude this trace from the team sum so its own Max HP increase does not feed back.
        let team_hp = battle
            .allies()
            .map(|ally| battle.effective_max_hp_excluding(ally, Some(owner)))
            .fold(0, u32::saturating_add);
        percentage_of(team_hp, GLASS_BALL_HP_BP)
    }

    fn follow_up_max_hp_multiplier_bp(&self) -> u16 {
        self.follow_up_bp
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, owner: usize, _: Action) {
        if self.lamb_outside_the_wall {
            let unit = &mut battle.units[owner];
            let state = &mut unit.tribbie_state;
            state.lamb_stacks = (state.lamb_stacks + 1).min(LAMB_STACKS);
            state.lamb_until = unit.normal_turns + LAMB_TURNS;
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let unit = &battle.units[owner];
        let lamb = LAMB_DAMAGE_BP * u32::from(unit.tribbie_state.lamb_stacks);
        let talent =
            E6_FOLLOW_UP_DAMAGE_BP * u32::from(self.eidolon >= 6 && kind == DamageKind::FollowUp);
        lamb + talent
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _: Option<usize>,
    ) {
        let hits = battle.attack_targets().len();
        if self.pebble_at_crossroads && actor != owner {
            battle.gain_energy(actor, owner, hits as u32 * PEBBLE_ENERGY_PER_TARGET_MILLI);
        }
        if !zone_active(battle, owner) {
            return;
        }
        let target = battle
            .attack_targets()
            .iter()
            .copied()
            .filter(|&target| battle.is_targetable(target))
            .max_by_key(|&target| (battle.units[target].hp, Reverse(target)));
        if self.eidolon >= 1 {
            battle.units[owner].tribbie_state.zone_target = target;
        }
        if let Some(target) = target {
            for _ in 0..hits + usize::from(self.eidolon >= 2) {
                battle.secondary_hit_from_hp(
                    owner,
                    target,
                    Element::Quantum,
                    self.additional_bp,
                    origin,
                );
            }
        }
    }

    fn after_ally_attack_damage(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        origin: Action,
        damage: u64,
    ) {
        let target = battle.units[owner].tribbie_state.zone_target.take();
        let amount =
            u32::try_from(damage.saturating_mul(E1_TRUE_DAMAGE_BP) / 10_000).unwrap_or(u32::MAX);
        if let Some(target) = target.filter(|&target| battle.is_targetable(target))
            && amount > 0
        {
            battle.true_damage(owner, target, origin, amount, Vec::new());
        }
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if !action.is_ultimate() {
            return;
        }
        if actor == owner {
            if self.eidolon < 6 {
                return;
            }
        } else {
            let triggers = &mut battle.units[owner].tribbie_state.talent_triggers;
            let slot = 1 << actor;
            if *triggers & slot != 0 {
                return;
            }
            *triggers |= slot;
        }
        if battle.enemies().any(|target| battle.is_targetable(target)) {
            queue_talent(battle, owner, action);
            battle.resolve_follow_ups();
        } else {
            battle.units[owner].tribbie_state.follow_up_next_wave = Some(action);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if let Some(origin) = battle.units[owner].tribbie_state.follow_up_next_wave.take() {
            queue_talent(battle, owner, origin);
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill if !numinosity_active(battle, owner) => Some(1),
            _ => None,
        }
    }
}
