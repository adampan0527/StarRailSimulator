use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{Battle, support::percentage_of},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1314";
const BASIC: &str = "skill.131401";
const SKILL: &str = "skill.131402";
const ULTIMATE: &str = "skill.131403";
const TALENT: &str = "skill.131404";
const REVERSE_REPO: &str = "trace.1314101";
const COLLATERAL_TICKET: &str = "trace.1314102";
const ASSET_FORFEITURE: &str = "trace.1314103";
const ENERGY_CAP: u32 = 140;
const COLLECTOR_TURNS: u32 = 3;
const COLLECTOR_SPEED_MICROS: u64 = 30_000_000;
const COLLECTOR_HP_COST_BP: u32 = 200;
const CHARGE_THRESHOLD: u8 = 8;
const ASSET_CAP: u8 = 50;
const FOLLOW_UP_ASSETS: u8 = 5;
const ENHANCED_ATTACKS: u8 = 2;

#[derive(Clone, Copy)]
struct DebtCollector {
    target: usize,
    expires_at: u32,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct JadeState {
    collector: Option<DebtCollector>,
    charge: u8,
    assets: u8,
    enhanced_attacks: u8,
    defense_ignore_until: u32,
}

struct Jade {
    eidolon: u8,
    reverse_repo: bool,
    collateral_ticket: bool,
    asset_forfeiture: bool,
    additional_bp: u16,
    follow_up_bp: u16,
    enhanced_follow_up_bp: u16,
    asset_crit_damage_bp: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != Element::Quantum
    {
        return Err(invalid("combat type, energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 2)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 5)?;
    if skill[0] != (COLLECTOR_SPEED_MICROS / 1_000_000) as f64
        || skill[1] != f64::from(COLLECTOR_HP_COST_BP) / 10_000.0
        || skill[3] != f64::from(COLLECTOR_TURNS)
        || ultimate[1] != f64::from(ENHANCED_ATTACKS)
        || talent[1] != f64::from(ASSET_CAP)
        || talent[2] != f64::from(CHARGE_THRESHOLD)
        || talent[3] != f64::from(FOLLOW_UP_ASSETS)
    {
        return Err(invalid("collector duration, speed, cost or talent limits"));
    }
    let follow_up_bp = basis_points(TALENT, talent[4])?;
    let enhanced_follow_up_bp = follow_up_bp
        .checked_add(basis_points(ULTIMATE, ultimate[0])?)
        .ok_or_else(|| invalid("enhanced follow-up multiplier"))?;
    let trace = |id| input.enabled_trace_ids.iter().any(|entry| entry == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![
                Effect::Hit {
                    element: Element::Quantum,
                    multiplier_bp: basis_points(BASIC, basic[0])?,
                    toughness: 10,
                },
                Effect::BlastAdjacent {
                    element: Element::Quantum,
                    multiplier_bp: basis_points(BASIC, basic[1])?,
                    toughness: 5,
                },
            ],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![Effect::Hit {
                element: Element::Quantum,
                multiplier_bp: basis_points(ULTIMATE, ultimate[2])?,
                toughness: 20,
            }],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Jade {
            eidolon: input.eidolon,
            reverse_repo: trace(REVERSE_REPO),
            collateral_ticket: trace(COLLATERAL_TICKET),
            asset_forfeiture: trace(ASSET_FORFEITURE),
            additional_bp: basis_points(SKILL, skill[2])?,
            follow_up_bp,
            enhanced_follow_up_bp,
            asset_crit_damage_bp: u32::from(basis_points(TALENT, talent[0])?),
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

impl Jade {
    fn collector(battle: &Battle<'_>, owner: usize) -> Option<usize> {
        battle.units[owner]
            .jade_state
            .collector
            .map(|collector| collector.target)
            .filter(|&target| battle.units[target].hp > 0)
    }

    fn has_collector_status(&self, battle: &Battle<'_>, owner: usize, target: usize) -> bool {
        Self::collector(battle, owner)
            .is_some_and(|collector| target == collector || (self.eidolon >= 6 && target == owner))
    }

    fn clear_collector(battle: &mut Battle<'_>, owner: usize) {
        if let Some(collector) = battle.units[owner].jade_state.collector.take() {
            battle.emit(
                EventKind::DebtCollector { active: false },
                Some(owner),
                Some(collector.target),
            );
        }
    }

    fn gain_assets(battle: &mut Battle<'_>, owner: usize, amount: u8) {
        let assets = &mut battle.units[owner].jade_state.assets;
        let before = *assets;
        let after = before.saturating_add(amount).min(ASSET_CAP);
        if after != before {
            *assets = after;
            battle.emit(
                EventKind::PawnedAsset { before, after },
                Some(owner),
                Some(owner),
            );
        }
    }

    fn charge(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        amount: u8,
        origin: Action,
    ) {
        let before = battle.units[owner].jade_state.charge;
        let after = before.saturating_add(amount);
        battle.units[owner].jade_state.charge = after;
        battle.emit(
            EventKind::JadeCharge { before, after },
            Some(actor),
            Some(owner),
        );
        while battle.units[owner].jade_state.charge >= CHARGE_THRESHOLD
            && battle.enemies().any(|target| battle.is_targetable(target))
        {
            let before = battle.units[owner].jade_state.charge;
            let after = before - CHARGE_THRESHOLD;
            battle.units[owner].jade_state.charge = after;
            battle.emit(
                EventKind::JadeCharge { before, after },
                Some(owner),
                Some(owner),
            );
            battle.follow_up(
                owner,
                Element::Quantum,
                self.follow_up_bp,
                10,
                origin,
                10_000,
            );
        }
    }

    fn collect(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, origin: Action) {
        for target in battle.attack_targets().to_vec() {
            battle.secondary_hit(
                owner,
                target,
                Element::Quantum,
                self.additional_bp,
                DamageKind::Additional,
                origin,
            );
        }
        if actor != owner {
            let hp_cost = percentage_of(battle.effective_max_hp(actor), COLLECTOR_HP_COST_BP);
            battle.spend_ally_hp(owner, actor, hp_cost, 1);
        }
    }

    fn preferred_collector(battle: &Battle<'_>, owner: usize) -> usize {
        battle
            .allies()
            .filter(|&target| target != owner && battle.is_targetable(target))
            .max_by_key(|&target| {
                let area_path = battle.plan.units[target].combat_type.is_some_and(|kind| {
                    matches!(kind.path, CombatPath::Erudition | CombatPath::Destruction)
                });
                (area_path, battle.speed_micros(target), Reverse(target))
            })
            .unwrap_or(owner)
    }
}

impl Mechanics for Jade {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.collateral_ticket {
            battle.advance(actor, 5_000);
        }
    }

    fn on_wave_start(&self, battle: &mut Battle<'_>, owner: usize) {
        if self.reverse_repo {
            let arrivals = battle
                .enemies()
                .filter(|&target| battle.is_targetable(target))
                .count();
            Self::gain_assets(battle, owner, u8::try_from(arrivals).unwrap_or(ASSET_CAP));
        }
    }

    fn on_ally_turn_opening(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        extra: bool,
    ) {
        if owner == actor
            && !extra
            && battle.units[owner]
                .jade_state
                .collector
                .is_some_and(|collector| battle.units[owner].normal_turns >= collector.expires_at)
        {
            Self::clear_collector(battle, owner);
        }
        if self.reverse_repo && self.has_collector_status(battle, owner, actor) {
            Self::gain_assets(battle, owner, 3);
        }
    }

    fn on_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize) {
        if Self::collector(battle, owner).is_none() {
            Self::clear_collector(battle, owner);
        }
    }

    fn on_defeated(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::clear_collector(battle, actor);
    }

    fn can_use_skill(&self, battle: &Battle<'_>, actor: usize) -> bool {
        Self::collector(battle, actor).is_none()
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => {
                let unit = &mut battle.units[actor];
                unit.jade_state.collector = Some(DebtCollector {
                    target,
                    expires_at: unit.normal_turns + COLLECTOR_TURNS,
                });
                battle.emit(
                    EventKind::DebtCollector { active: true },
                    Some(actor),
                    Some(target),
                );
            }
            (Action::Ultimate, _) => {
                let unit = &mut battle.units[actor];
                unit.jade_state.enhanced_attacks = ENHANCED_ATTACKS;
                if self.eidolon >= 4 {
                    unit.jade_state.defense_ignore_until = unit.finished_turns + 3;
                }
            }
            _ => {}
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
        _target: Option<usize>,
    ) {
        let collector = self.has_collector_status(battle, owner, actor);
        if actor != owner && !collector {
            return;
        }
        let hits = u8::try_from(battle.attack_targets().len()).unwrap_or(u8::MAX);
        if hits == 0 {
            return;
        }
        if collector {
            self.collect(battle, owner, actor, origin);
        }
        if actor != owner || !battle.in_follow_up() {
            let charge = if self.eidolon >= 1 && collector {
                hits.max(3)
            } else {
                hits
            };
            self.charge(battle, owner, actor, charge, origin);
        }
    }

    fn before_follow_up(&self, battle: &mut Battle<'_>, actor: usize) {
        Self::gain_assets(battle, actor, FOLLOW_UP_ASSETS);
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        let state = &mut battle.units[actor].jade_state;
        state.enhanced_attacks = state.enhanced_attacks.saturating_sub(1);
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if kind == DamageKind::FollowUp && battle.units[actor].jade_state.enhanced_attacks > 0 {
            u32::from(self.enhanced_follow_up_bp)
        } else {
            base
        }
    }

    fn damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        3_200 * u32::from(self.eidolon >= 1 && kind == DamageKind::FollowUp)
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        1_800 * u32::from(self.eidolon >= 2 && battle.units[actor].jade_state.assets >= 15)
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        self.asset_crit_damage_bp * u32::from(battle.units[actor].jade_state.assets)
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u32 {
        if self.asset_forfeiture && owner == target {
            u32::from(battle.units[owner].jade_state.assets) * 50
        } else {
            0
        }
    }

    fn ally_flat_speed_micros(&self, battle: &Battle<'_>, owner: usize, target: usize) -> u64 {
        COLLECTOR_SPEED_MICROS
            * u64::from(target != owner && Self::collector(battle, owner) == Some(target))
    }

    fn defense_ignore_bp(&self, battle: &Battle<'_>, actor: usize, _target: usize) -> u32 {
        let unit = &battle.units[actor];
        1_200 * u32::from(unit.finished_turns < unit.jade_state.defense_ignore_until)
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        2_000
            * u32::from(
                self.eidolon >= 6
                    && element == Element::Quantum
                    && Self::collector(battle, actor).is_some(),
            )
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        if action == Action::Skill && target == Some(Self::preferred_collector(battle, actor)) {
            Some(2)
        } else {
            None
        }
    }
}
