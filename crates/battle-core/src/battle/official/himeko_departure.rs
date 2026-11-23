use std::sync::Arc;

use crate::{
    Action, BattleError, CombatPath, DamageKind, EventKind,
    fixture::{Effect, Element, Target},
};

use super::{Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1510";
const BASIC: &str = "skill.151001";
const SKILL: &str = "skill.151002";
const ULTIMATE: &str = "skill.151003";
const TALENT: &str = "skill.151004";
const BEAM: &str = "skill.151008";
const PULSE: &str = "skill.151009";
const FINISHER: &str = "skill.151014";
const ASSIST: &str = "skill.151022";
const VERDICT: &str = "skill.151025";
const DECIMATION: &str = "skill.151026";
const FREE_ASSIST: &str = "trace.1510101";
const EXTRA_TURN: &str = "trace.1510102";
const INITIAL_SOURCE: &str = "trace.1510103";
const FIRE: Element = Element::Fire;
const ENERGY_CAP: u32 = 150_000;
const BEAMS: u8 = 6;
const SOURCE_CAP: u8 = 3;
const SOURCE_CAP_E6: u8 = 6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Protocol {
    #[default]
    None,
    Verdict,
    Decimation,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct HimekoDepartureState {
    assist_uses: u8,
    source_energy: u8,
    cresset_expires: u32,
    protocol: Protocol,
    protocol_progress: u8,
    protocol_uses: u8,
    pending_assist: bool,
    assist_active: bool,
    automatic_assist: bool,
}

struct HimekoDeparture {
    own_assist: OfficialAbility,
    ally_assist: OfficialAbility,
    cresset_bp: u32,
    cresset_turns: u32,
    assist_crit_bp: u32,
    assist_penetration_bp: u32,
    ally_energy_milli: u32,
    verdict_bp: u32,
    verdict_ultimate_bp: u32,
    verdict_threshold: u8,
    decimation_bp: u32,
    decimation_skill_bp: u32,
    decimation_threshold: u8,
    max_protocol_uses: u8,
    beam_bp: u16,
    pulse_bp: u16,
    pulse_bounce_bp: u16,
    finisher_bp: u16,
    finisher_hits: u8,
    free_assist: bool,
    extra_turn: bool,
    initial_source: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != ENERGY_CAP / 1_000
        || input.eidolon > 6
        || input.combat_type.path != CombatPath::Erudition
        || input.combat_type.element != FIRE
    {
        return Err(invalid("energy, combat type or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 10)?;
    let talent = input.parameters(TALENT, 3)?;
    let assist = input.parameters(ASSIST, 8)?;
    let verdict = input.parameters(VERDICT, 10)?;
    let decimation = input.parameters(DECIMATION, 10)?;
    let beam = input.parameters(BEAM, 3)?;
    let pulse = input.parameters(PULSE, 3)?;
    let finisher = input.parameters(FINISHER, 2)?;
    if beam[1..] != [1.0, f64::from(SOURCE_CAP)]
        || pulse[1] != 1.0
        || ultimate[5] != finisher[0]
        || ultimate[6] != finisher[1]
    {
        return Err(invalid(ULTIMATE));
    }
    let first = input.eidolon >= 1;
    let trace = |id| input.enabled_trace_ids.iter().any(|enabled| enabled == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_CAP,
        basic: ability(
            Target::Enemy,
            20_000,
            SkillPoints::Generate,
            vec![hit(ratio_bp(BASIC, basic[0])?, 10)],
        ),
        skill: ability(Target::SelfOnly, 30_000, SkillPoints::Spend, Vec::new()),
        ultimate: ability(
            Target::AllEnemies,
            5_000,
            SkillPoints::Unchanged,
            Vec::new(),
        ),
        mechanics: Arc::new(HimekoDeparture {
            own_assist: assist_ability(assist[3], assist[4], assist[5], first)?,
            ally_assist: assist_ability(assist[0], assist[1], assist[2], false)?,
            cresset_bp: ratio_bp(SKILL, skill[0])?.into(),
            cresset_turns: count(SKILL, skill[1])?.into(),
            assist_crit_bp: ratio_bp(TALENT, talent[0])?.into(),
            assist_penetration_bp: ratio_bp(TALENT, talent[1])?.into(),
            ally_energy_milli: u32::from(count(TALENT, talent[2])?) * 1_000,
            verdict_bp: ratio_bp(VERDICT, verdict[7])?.into(),
            verdict_ultimate_bp: ratio_bp(VERDICT, verdict[9])?.into(),
            verdict_threshold: count(VERDICT, verdict[8])?
                .saturating_sub(u8::from(first))
                .max(1),
            decimation_bp: ratio_bp(DECIMATION, decimation[7])?.into(),
            decimation_skill_bp: ratio_bp(DECIMATION, decimation[9])?.into(),
            decimation_threshold: count(DECIMATION, decimation[8])?
                .saturating_sub(3 * u8::from(first))
                .max(1),
            max_protocol_uses: count(ASSIST, assist[7])?.saturating_add(u8::from(first)),
            beam_bp: ratio_bp(BEAM, beam[0])?,
            pulse_bp: ratio_bp(PULSE, pulse[0])?,
            pulse_bounce_bp: ratio_bp(PULSE, pulse[2])?,
            finisher_bp: ratio_bp(FINISHER, finisher[1])?,
            finisher_hits: count(FINISHER, finisher[0])?,
            free_assist: trace(FREE_ASSIST),
            extra_turn: trace(EXTRA_TURN),
            initial_source: trace(INITIAL_SOURCE),
            eidolon: input.eidolon,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn ratio_bp(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(0.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(id));
    }
    Ok(bp as u16)
}

fn count(id: &str, value: f64) -> Result<u8, BattleError> {
    if !value.is_finite() || !(0.0..=f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u8)
}

fn ability(
    target: Target,
    energy_milli: u32,
    skill_points: SkillPoints,
    effects: Vec<Effect>,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        skill_points,
        effects,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: FIRE,
        multiplier_bp,
        toughness,
    }
}

fn assist_ability(
    aoe: f64,
    hits: f64,
    bounce: f64,
    eidolon_bonus: bool,
) -> Result<OfficialAbility, BattleError> {
    Ok(ability(
        Target::AllEnemies,
        18_000,
        SkillPoints::Unchanged,
        vec![
            hit(ratio_bp(ASSIST, aoe)?, 10),
            Effect::RandomHits {
                element: FIRE,
                multiplier_bp: ratio_bp(ASSIST, bounce)?,
                toughness: 5,
                count: count(ASSIST, hits)?.saturating_add(u8::from(eidolon_bonus)),
            },
        ],
    ))
}

fn companion(battle: &Battle<'_>, actor: usize) -> Protocol {
    match battle.plan.units[actor].official_form.as_deref() {
        Some("dan_heng.hunt" | "character.1213" | "character.1414" | "character.1313") => {
            Protocol::Verdict
        }
        Some(form) if form.starts_with("trailblazer.") => Protocol::Verdict,
        Some(
            "march_7th.preservation"
            | "march_7th.hunt"
            | "character.1413"
            | "character.1003"
            | "character.1004",
        ) => Protocol::Decimation,
        _ => Protocol::None,
    }
}

fn cresset_active(battle: &Battle<'_>, actor: usize) -> bool {
    let unit = &battle.units[actor];
    unit.himeko_departure.cresset_expires > unit.normal_turns
}

fn set_assist_uses(battle: &mut Battle<'_>, owner: usize, after: u8) {
    let before = std::mem::replace(&mut battle.units[owner].himeko_departure.assist_uses, after);
    if before != after {
        battle.emit(
            EventKind::HimekoAssistUses { before, after },
            Some(owner),
            None,
        );
    }
}

fn set_source(battle: &mut Battle<'_>, actor: usize, after: u8) {
    let before = std::mem::replace(
        &mut battle.units[actor].himeko_departure.source_energy,
        after,
    );
    if before != after {
        battle.emit(
            EventKind::HimekoSourceEnergy { before, after },
            Some(actor),
            None,
        );
    }
}

impl HimekoDeparture {
    fn assist_cap(&self) -> u8 {
        if self.eidolon >= 2 { 2 } else { 1 }
    }

    fn source_cap(&self) -> u8 {
        if self.eidolon >= 6 {
            SOURCE_CAP_E6
        } else {
            SOURCE_CAP
        }
    }

    fn add_source(&self, battle: &mut Battle<'_>, actor: usize, add: u8) {
        let source = battle.units[actor].himeko_departure.source_energy;
        set_source(
            battle,
            actor,
            source.saturating_add(add).min(self.source_cap()),
        );
    }

    fn resolves_companion_turn(&self, battle: &Battle<'_>, actor: usize) -> bool {
        self.extra_turn
            && !battle.in_extra_turn(actor)
            && (self.eidolon >= 2 || companion(battle, actor) != Protocol::None)
    }

    fn advance_protocol(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        protocol: Protocol,
        progress: u8,
    ) {
        let state = &mut battle.units[owner].himeko_departure;
        if state.protocol != protocol
            || state.protocol_uses >= self.max_protocol_uses
            || state.pending_assist
        {
            return;
        }
        state.protocol_progress = state.protocol_progress.saturating_add(progress);
        let threshold = match protocol {
            Protocol::Verdict => self.verdict_threshold,
            Protocol::Decimation => self.decimation_threshold,
            Protocol::None => return,
        };
        if state.protocol_progress >= threshold {
            state.protocol_progress = 0;
            state.pending_assist = true;
        }
    }

    fn resolve_assist(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, automatic: bool) {
        let attack = if owner == actor {
            &self.own_assist
        } else {
            &self.ally_assist
        };
        let (ability, _) = attack.resolve();
        let previous_action = battle.action.replace(Action::Skill);
        let parent_debuffs = battle.begin_debuff_action();
        battle.units[owner].himeko_departure.assist_active = true;
        battle.units[owner].himeko_departure.automatic_assist = automatic;
        if automatic {
            battle.emit(EventKind::TriggeredSkill, Some(owner), None);
        }
        if self.eidolon >= 6 {
            self.add_source(battle, owner, 1);
        }
        battle.begin_attack();
        battle.notify_active_ability(owner);
        battle.activate_attack_bonuses(owner, Action::Skill);
        battle.prepare_skill_copy(owner, None, ability);
        battle.notify_ally_action_started(owner, None, ability);
        battle.resolve_ability(owner, None, ability);
        battle.gain_energy(owner, owner, attack.energy_milli);
        if owner != actor {
            battle.restore_energy_flat(owner, actor, self.ally_energy_milli);
        }
        battle.notify_attack(owner, Action::Skill, None);
        battle.finish_debuff_action(owner, parent_debuffs);
        battle.resolve_follow_ups();
        battle.notify_ally_action_finished(owner, Action::Skill);
        battle.units[owner].himeko_departure.assist_active = false;
        battle.units[owner].himeko_departure.automatic_assist = false;
        battle.units[owner].active_attack_bonus_bp = 0;
        battle.units[owner].active_penetration_bp = 0;
        battle.action = previous_action;
    }

    fn aoe(battle: &mut Battle<'_>, actor: usize, multiplier_bp: u16, toughness: u16) {
        for target in battle.enemies() {
            battle.attack_hit(actor, target, FIRE, multiplier_bp, toughness);
        }
    }

    fn bounce(
        battle: &mut Battle<'_>,
        actor: usize,
        multiplier_bp: u16,
        toughness: u16,
        count: u8,
    ) {
        for _ in 0..count {
            let Some(target) = battle.random_living_enemy() else {
                break;
            };
            battle.attack_hit(actor, target, FIRE, multiplier_bp, toughness);
        }
    }

    fn pulse(&self, battle: &mut Battle<'_>, actor: usize) {
        let source = battle.units[actor].himeko_departure.source_energy;
        if source == 0 || !battle.enemies().any(|target| battle.is_targetable(target)) {
            return;
        }
        set_source(battle, actor, 0);
        Self::aoe(battle, actor, self.pulse_bp, 2);
        if source > 1 {
            let bonus = u16::from(self.initial_source && source >= SOURCE_CAP) * 3_000;
            Self::bounce(
                battle,
                actor,
                self.pulse_bounce_bp.saturating_add(bonus),
                2,
                source,
            );
        }
        if self.eidolon >= 6 && source == SOURCE_CAP_E6 {
            Self::aoe(battle, actor, 16_000, 0);
        }
    }

    fn launch_ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        {
            let state = &mut battle.units[actor].himeko_departure;
            state.protocol_progress = 0;
            state.protocol_uses = 0;
            state.pending_assist = false;
        }
        if self.initial_source {
            self.add_source(battle, actor, SOURCE_CAP);
        }
        for _ in 0..BEAMS {
            if battle.units[actor].himeko_departure.source_energy >= self.source_cap() {
                self.pulse(battle, actor);
            }
            if !battle.enemies().any(|target| battle.is_targetable(target)) {
                break;
            }
            Self::aoe(battle, actor, self.beam_bp, 2);
            self.add_source(battle, actor, if self.eidolon >= 6 { 2 } else { 1 });
        }
        self.pulse(battle, actor);
        Self::bounce(battle, actor, self.finisher_bp, 4, self.finisher_hits);
    }
}

impl Mechanics for HimekoDeparture {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        set_assist_uses(battle, actor, 1);
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
        let uses = battle.units[owner].himeko_departure.assist_uses;
        if self.free_assist && uses == self.assist_cap() {
            battle.restore_energy_flat(owner, owner, 5_000);
        }
        if cresset_active(battle, owner) {
            set_assist_uses(battle, owner, self.assist_cap());
        }
    }

    fn resolves_attack_without_effects(&self, action: Action) -> bool {
        action == Action::Ultimate
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        match action {
            Action::Skill => {
                let unit = &mut battle.units[actor];
                unit.himeko_departure.cresset_expires = unit.normal_turns + self.cresset_turns;
                set_assist_uses(battle, actor, self.assist_cap());
            }
            Action::Ultimate => self.launch_ultimate(battle, actor),
            _ => {}
        }
    }

    fn assist_skill_priority(&self, battle: &Battle<'_>, owner: usize, actor: usize) -> Option<u8> {
        let state = &battle.units[owner].himeko_departure;
        if state.assist_uses == 0 && (owner != actor || !self.free_assist) {
            return None;
        }
        let protocol = companion(battle, actor);
        Some(if owner == actor {
            2
        } else if !battle.in_extra_turn(actor)
            && (self.resolves_companion_turn(battle, actor)
                || (protocol != Protocol::None && state.protocol == Protocol::None))
        {
            if state.protocol == Protocol::None {
                1
            } else {
                2
            }
        } else {
            5
        })
    }

    fn on_assist_skill(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner != actor || !self.free_assist {
            let uses = battle.units[owner].himeko_departure.assist_uses;
            set_assist_uses(battle, owner, uses.saturating_sub(1));
        }
        let protocol = companion(battle, actor);
        let state = &mut battle.units[owner].himeko_departure;
        if protocol != Protocol::None && state.protocol != protocol {
            state.protocol = protocol;
            state.protocol_progress = 0;
        }
        if owner != actor && self.resolves_companion_turn(battle, actor) {
            battle.queue_extra_turn(actor);
        }
        self.resolve_assist(battle, owner, actor, false);
    }

    fn on_ally_ultimate_started(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: u32,
    ) {
        if owner != actor && battle.in_territory(owner) {
            self.advance_protocol(battle, owner, Protocol::Verdict, 1);
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _: Action,
        _: Option<usize>,
    ) {
        if (owner != actor || !battle.units[owner].himeko_departure.automatic_assist)
            && battle.in_territory(owner)
        {
            let targets = u8::try_from(battle.attack_targets().len()).unwrap_or(u8::MAX);
            self.advance_protocol(battle, owner, Protocol::Decimation, targets);
        }
    }

    fn on_unit_action_finished(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        if battle.in_territory(owner)
            && battle.enemies().any(|target| battle.is_targetable(target))
            && std::mem::take(&mut battle.units[owner].himeko_departure.pending_assist)
        {
            battle.units[owner].himeko_departure.protocol_uses += 1;
            self.resolve_assist(battle, owner, owner, true);
        }
    }

    fn ignores_weakness(&self, action: Option<Action>, kind: DamageKind) -> bool {
        kind == DamageKind::Direct && action == Some(Action::Skill)
    }

    fn action_damage_factor_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u16 {
        if self.eidolon >= 2
            && kind == DamageKind::Direct
            && (battle.units[actor].himeko_departure.assist_active
                || action.is_some_and(Action::is_ultimate))
        {
            13_000
        } else {
            10_000
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        action: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor].himeko_departure;
        let verdict = u32::from(state.protocol == Protocol::Verdict);
        verdict
            * (self.verdict_bp
                + u32::from(action.is_some_and(Action::is_ultimate)) * self.verdict_ultimate_bp)
            + u32::from(self.eidolon >= 6 && state.assist_active) * 7_500
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        u32::from(battle.units[actor].himeko_departure.assist_active) * self.assist_crit_bp
    }

    fn ally_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize, _: Element) -> u32 {
        u32::from(cresset_active(battle, owner)) * self.cresset_bp
    }

    fn ally_crit_damage_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        u32::from(battle.units[owner].himeko_departure.protocol == Protocol::Decimation)
            * self.decimation_bp
    }

    fn ally_skill_crit_damage_bp(&self, battle: &Battle<'_>, owner: usize, _: usize) -> u32 {
        u32::from(battle.units[owner].himeko_departure.protocol == Protocol::Decimation)
            * self.decimation_skill_bp
    }

    fn resistance_penetration_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        element: Element,
    ) -> u32 {
        let assist_bonus = if self.eidolon >= 4 {
            1_000
        } else {
            self.assist_penetration_bp
        };
        u32::from(battle.units[actor].himeko_departure.assist_active) * assist_bonus
            + u32::from(self.eidolon >= 6 && element == FIRE) * 2_000
    }

    fn ally_resistance_penetration_bp(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        u32::from(self.eidolon >= 4 && battle.units[owner].himeko_departure.assist_active)
            * self.assist_penetration_bp
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill if !cresset_active(battle, actor) => Some(1),
            Action::Skill => Some(4),
            _ => None,
        }
    }
}
