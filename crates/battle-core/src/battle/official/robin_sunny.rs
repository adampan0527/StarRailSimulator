use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Stats, Target},
};

use super::{
    super::{
        Battle, SummonTurn,
        resolve::DamageTargetAura,
        support::{AllyBuffKind, percentage_of},
    },
    CombatPath, Mechanics, MemospriteProfile, OfficialAbility, OfficialCharacter, OfficialKit,
    SkillPoints,
};

#[cfg(test)]
mod tests;

pub(super) const FORM: &str = "character.1512";
const BASIC: &str = "skill.151201";
const SKILL: &str = "skill.151202";
const ULTIMATE: &str = "skill.151203";
const TALENT: &str = "skill.151204";
const BIRD_SKILL: &str = "skill.1151201";
const BIRD_TALENT: &str = "skill.1151203";
const ARRIVAL: &str = "skill.1151205";
const DEPARTURE: &str = "skill.1151206";
const DEVIATED_CHORDS: &str = "trace.1512101";
const IMPROVISED_BLUES: &str = "trace.1512102";
const REBUILT_HARMONY: &str = "trace.1512103";
const SONGBIRDS: &str = "summer_songbirds";
const FEVER_TIMER: &str = "summer_fever";
const ENERGY_MILLI: u32 = 140_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct RobinSunnyState {
    vibes: u32,
    rhythm: u32,
    birds: u8,
    first_fever_seen: bool,
    last_sustain_turn: Option<u64>,
    last_rhythm_turn: Option<u64>,
    last_e2_turn: Option<u64>,
    guest_turns: Vec<(usize, u8)>,
    damage_record: u64,
}

struct RobinSunny {
    eidolon: u8,
    chords: bool,
    blues: bool,
    harmony: bool,
    skill_heal_bp: u32,
    ultimate_energy_bp: u32,
    fever_defense_base_bp: u32,
    bird_damage_bp: u32,
    fever_damage_base_bp: u32,
    fever_damage_per_vibe_bp: u32,
    vulnerability_bp: [u32; 3],
}

fn invalid(name: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {name}"))
}

fn ratio(name: &str, value: f64) -> Result<u32, BattleError> {
    let bp = (value * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(name));
    }
    Ok(bp as u32)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.combat_type.path != CombatPath::Remembrance
        || input.combat_type.element != Element::Wind
        || input.energy_capacity != ENERGY_MILLI / 1000
        || input.eidolon > 6
    {
        return Err(invalid("energy capacity, combat type or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 9)?;
    let bird_skill = input.parameters(BIRD_SKILL, 2)?;
    let bird_talent = input.parameters(BIRD_TALENT, 10)?;
    if skill[1] != 6.0
        || ultimate[..2] != [1.0, 2.0]
        || talent[..2] != [0.7, 1.8]
        || talent[4..7] != [50.0, 6.0, 12.0]
        || talent[8] != 0.005
        || bird_talent[5] != 12.0
        || bird_talent[8..] != [140.0, 0.5]
        || input.parameters(ARRIVAL, 1)? != [20.0]
        || input.parameters(DEPARTURE, 1)? != [0.5]
    {
        return Err(invalid("unexpected frozen skill conditions"));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|entry| entry == id);
    Ok(OfficialKit {
        max_energy_milli: ENERGY_MILLI,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Wind,
                multiplier_bp: ratio(BASIC, basic[0])? as u16,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::SelfOnly,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::OtherAlly,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(RobinSunny {
            eidolon: input.eidolon,
            chords: trace(DEVIATED_CHORDS),
            blues: trace(IMPROVISED_BLUES),
            harmony: trace(REBUILT_HARMONY),
            skill_heal_bp: ratio(SKILL, skill[0])?,
            ultimate_energy_bp: ratio(ULTIMATE, ultimate[2])?,
            fever_defense_base_bp: ratio(TALENT, talent[7])?,
            bird_damage_bp: ratio(BIRD_SKILL, bird_skill[1])?,
            fever_damage_base_bp: ratio(BIRD_TALENT, bird_talent[0])?,
            fever_damage_per_vibe_bp: ratio(BIRD_TALENT, bird_talent[1])?,
            vulnerability_bp: [
                ratio(BIRD_TALENT, bird_talent[2])?,
                ratio(BIRD_TALENT, bird_talent[3])?,
                ratio(BIRD_TALENT, bird_talent[4])?,
            ],
        }),
    })
}

fn pos_state<'b>(battle: &'b Battle<'_>, actor: usize) -> &'b RobinSunnyState {
    &battle.units[actor].robin_sunny_state
}

impl RobinSunny {
    fn set_vibes(battle: &mut Battle<'_>, owner: usize, after: u32) {
        let previous = std::mem::replace(&mut battle.units[owner].robin_sunny_state.vibes, after);
        if previous != after {
            battle.emit(
                EventKind::RobinVibes {
                    before: previous,
                    after,
                },
                Some(owner),
                None,
            );
        }
    }

    fn set_rhythm(battle: &mut Battle<'_>, owner: usize, after: u32) {
        let previous = std::mem::replace(&mut battle.units[owner].robin_sunny_state.rhythm, after);
        if previous != after {
            battle.emit(
                EventKind::RobinRhythm {
                    before: previous,
                    after,
                },
                Some(owner),
                None,
            );
        }
    }

    fn guest(&self, battle: &Battle<'_>, owner: usize, source: usize) -> bool {
        let character = battle.memosprite_owner(source).unwrap_or(source);
        pos_state(battle, owner)
            .guest_turns
            .iter()
            .any(|&(guest, _)| guest == character)
    }

    fn trace_chords(&self, battle: &mut Battle<'_>, owner: usize, source: usize) {
        let vibes = pos_state(battle, owner).vibes;
        let (kind, amount) = if battle.effective_attack_excluding(source, Some(owner))
            > battle.effective_attack_excluding(owner, Some(owner))
        {
            (
                AllyBuffKind::AttackFlat,
                percentage_of(battle.effective_max_hp(owner), 1_600 + 40 * vibes),
            )
        } else {
            (AllyBuffKind::CritDamage, 4_000 + 150 * vibes)
        };
        battle.replace_ally_buff(owner, source, kind, amount, 2, DEVIATED_CHORDS);
    }

    fn add_vibes(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        gain: u32,
        skill: bool,
    ) {
        let turn = battle.turn_sequence;
        let state = &mut battle.units[owner].robin_sunny_state;
        let bonus = self.eidolon >= 2 && skill && state.last_e2_turn != Some(turn);
        let cap = if self.eidolon >= 2 { 70 } else { 50 };
        let after = (state.vibes + gain + 2 * u32::from(bonus)).min(cap);
        if after == state.vibes {
            return;
        }
        if bonus {
            state.last_e2_turn = Some(turn);
        }
        let rhythm = self.blues && state.last_rhythm_turn != Some(turn) && state.rhythm > 0;
        state.last_rhythm_turn = Some(turn);
        let rhythm_after = state.rhythm.saturating_sub(1);
        Self::set_vibes(battle, owner, after);
        if rhythm {
            Self::set_rhythm(battle, owner, rhythm_after);
            battle.restore_energy_flat(owner, owner, 3_000);
        }
        self.recruit(battle, owner);
        if self.chords {
            self.trace_chords(battle, owner, source);
        }
    }

    fn recruit(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        let state = pos_state(battle, owner);
        let birds = 1 + u8::from(state.vibes >= 6) + u8::from(state.vibes >= 12);
        // Falling Vibes during Fever do not remove individual musicians.
        for member in state.birds + 1..=birds {
            battle.units[owner].robin_sunny_state.birds = member;
            battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
            if member < 3 {
                battle.gain_energy(sprite, owner, 20_000);
            } else {
                self.start_fever(battle, owner, sprite);
                battle.gain_energy(sprite, owner, 20_000);
            }
        }
        self.update_bird_speed(battle, owner, sprite);
    }

    fn start_fever(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        if battle.current_action().is_none_or(Action::is_ultimate) {
            battle.close_turn_before_suspension(owner);
        }
        let speed = battle.plan.units[sprite].stats.base_speed_micros;
        battle.units[owner].summon_turn = Some(SummonTurn::new(SONGBIRDS, speed));
        battle.units[owner].countdown = Some(SummonTurn::with_action_advance(
            FEVER_TIMER,
            140_000_000,
            false,
        ));
        battle.cleanse_crowd_control(owner, owner);
        battle.cleanse_crowd_control(owner, sprite);
        battle.emit(
            EventKind::StatusApplied(StatusKind::Fever),
            Some(owner),
            Some(owner),
        );
        if self.eidolon >= 4 {
            Self::set_vibes(battle, owner, (pos_state(battle, owner).vibes + 12).min(70));
        }
        let first = !std::mem::replace(
            &mut battle.units[owner].robin_sunny_state.first_fever_seen,
            true,
        );
        if self.eidolon >= 6 && first {
            battle.restore_energy_flat(owner, owner, ENERGY_MILLI);
        }
    }

    fn update_bird_speed(&self, battle: &mut Battle<'_>, owner: usize, sprite: usize) {
        let ratio = 10_000
            + u64::from(self.eidolon >= 4)
                * (2_000 + 50 * u64::from(pos_state(battle, owner).vibes));
        let base = battle.plan.units[sprite].stats.base_speed_micros;
        if let Some(turn) = &mut battle.units[owner].summon_turn {
            turn.speed_micros = base.saturating_mul(ratio) / 10_000;
        }
    }

    fn cast_skill(&self, battle: &mut Battle<'_>, owner: usize) {
        if let Some(sprite) = battle.active_memosprite(owner) {
            battle.heal_unit(
                owner,
                sprite,
                percentage_of(battle.effective_max_hp(sprite), self.skill_heal_bp),
            );
            self.add_vibes(battle, owner, owner, 6, true);
        } else if let Some(sprite) = battle.memosprite(owner) {
            let hp = percentage_of(battle.effective_max_hp(owner), 7_000).max(1);
            battle.units[sprite].memosprite_max_hp = Some(hp);
            battle.units[sprite].memosprite_generation += 1;
            battle.units[sprite].hp = battle.effective_max_hp(sprite);
            battle.units[owner].robin_sunny_state.birds = 1;
            battle.emit(EventKind::TriggeredSkill, Some(owner), Some(sprite));
            battle.gain_energy(sprite, owner, 20_000);
            self.recruit(battle, owner);
        }
    }

    fn cast_ultimate(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        battle.advance_ally_immediate(owner, target);
        let cap = battle.plan.units[target].kit.max_energy_milli();
        battle.restore_energy_flat(owner, target, percentage_of(cap, self.ultimate_energy_bp));
        let guests = &mut battle.units[owner].robin_sunny_state.guest_turns;
        if let Some(guest) = guests.iter_mut().find(|guest| guest.0 == target) {
            guest.1 = 2;
        } else {
            guests.push((target, 2));
        }
        battle.emit(
            EventKind::StatusApplied(StatusKind::SpecialGuest),
            Some(owner),
            Some(target),
        );
    }

    fn singing(&self, battle: &Battle<'_>, owner: usize) -> bool {
        pos_state(battle, owner).birds == 3
    }

    fn fever_damage(&self, battle: &Battle<'_>, owner: usize) -> u32 {
        if self.singing(battle, owner) {
            self.fever_damage_base_bp
                + self.fever_damage_per_vibe_bp * pos_state(battle, owner).vibes
        } else {
            0
        }
    }
}

impl Mechanics for RobinSunny {
    fn memosprite_profile(&self, master: Stats) -> Option<MemospriteProfile> {
        Some(MemospriteProfile {
            name: SONGBIRDS,
            stats: Stats {
                hp: percentage_of(master.hp, 7_000).max(1),
                base_hp: percentage_of(master.base_hp, 7_000).max(1),
                base_speed_micros: (master.base_speed_micros + master.flat_speed_micros) * 18 / 10,
                flat_speed_micros: 0,
                base_aggro: 100,
                crit_rate_bp: master
                    .crit_rate_bp
                    .saturating_add(5_000 * u32::from(self.harmony)),
                ..master
            },
        })
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, _) => self.cast_skill(battle, actor),
            (Action::Ultimate, Some(target)) => self.cast_ultimate(battle, actor, target),
            _ => {}
        }
    }

    fn on_ally_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        _: Action,
        _: Option<usize>,
    ) {
        if battle.is_targetable(owner) && battle.is_party_unit(source) {
            let amount = 1 + 2 * u32::from(self.guest(battle, owner, source));
            self.add_vibes(battle, owner, source, amount, true);
        }
    }

    fn observes_sustain_sources(&self) -> bool {
        true
    }

    fn on_ally_sustain_applied(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
    ) {
        if self.blues
            && source != target
            && source != owner
            && (target == owner || battle.memosprite_owner(target) == Some(owner))
        {
            Self::set_rhythm(battle, owner, 12);
        }
        if pos_state(battle, owner).last_sustain_turn != Some(battle.turn_sequence) {
            battle.units[owner].robin_sunny_state.last_sustain_turn = Some(battle.turn_sequence);
            self.add_vibes(battle, owner, source, 1, true);
        }
    }

    fn blocks_ally_advance(&self, battle: &Battle<'_>, owner: usize, source: usize) -> bool {
        self.guest(battle, owner, source)
    }

    fn on_ally_turn_opening(&self, battle: &mut Battle<'_>, owner: usize, actor: usize, _: bool) {
        let guests = &mut battle.units[owner].robin_sunny_state.guest_turns;
        if let Some(guest) = guests.iter_mut().find(|guest| guest.0 == actor) {
            guest.1 -= 1;
            if guest.1 == 0 {
                guests.retain(|guest| guest.0 != actor);
                battle.emit(
                    EventKind::StatusExpired(StatusKind::SpecialGuest),
                    Some(owner),
                    Some(actor),
                );
            }
        }
    }

    fn suspends_turn(&self, battle: &Battle<'_>, actor: usize) -> bool {
        self.singing(battle, actor)
    }

    fn can_ult_while_suspended(&self) -> bool {
        true
    }

    fn ultimate_energy_range(&self, _: Action) -> Option<std::ops::RangeInclusive<u32>> {
        let cap = ENERGY_MILLI * if self.eidolon >= 6 { 2 } else { 1 };
        Some(ENERGY_MILLI..=cap)
    }

    fn energy_storage_multiplier(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        1 + u32::from(self.eidolon >= 6 && self.singing(battle, actor))
    }

    fn immune_to_enemy_control(&self, battle: &Battle<'_>, actor: usize) -> bool {
        self.singing(battle, actor)
    }

    fn memosprite_immune_to_enemy_control(&self, battle: &Battle<'_>, owner: usize) -> bool {
        self.singing(battle, owner)
    }

    fn on_countdown_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let vibes = pos_state(battle, owner).vibes;
        let remainder = vibes.saturating_sub((vibes / 2).max(12));
        Self::set_vibes(battle, owner, remainder);
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        if remainder == 0 {
            battle.despawn_memosprite(owner, sprite);
        } else {
            self.update_bird_speed(battle, owner, sprite);
            battle.units[owner].countdown = Some(SummonTurn::with_action_advance(
                FEVER_TIMER,
                140_000_000,
                false,
            ));
        }
        if self.eidolon >= 6 {
            battle.restore_energy_flat(owner, owner, ENERGY_MILLI);
        }
    }

    fn on_memosprite_departed(&self, battle: &mut Battle<'_>, owner: usize, _: usize) {
        let was_singing = self.singing(battle, owner);
        battle.units[owner].robin_sunny_state.birds = 0;
        battle.units[owner].countdown = None;
        if was_singing {
            battle.emit(
                EventKind::StatusExpired(StatusKind::Fever),
                Some(owner),
                Some(owner),
            );
        }
        if battle.units[owner].energy_milli > ENERGY_MILLI {
            battle.set_energy(owner, owner, ENERGY_MILLI);
        }
        if battle.is_targetable(owner) {
            battle.advance(owner, 5_000);
        }
    }

    fn tracks_ally_damage(&self) -> bool {
        self.eidolon >= 1
    }

    fn on_ally_damage(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _: usize,
        _: usize,
        _: DamageKind,
        amount: u32,
    ) {
        if self.eidolon >= 1 {
            let ledger = &mut battle.units[owner].robin_sunny_state.damage_record;
            *ledger = ledger.saturating_add(u64::from(amount));
        }
    }

    fn on_summon_turn(&self, battle: &mut Battle<'_>, owner: usize) {
        let Some(sprite) = battle.active_memosprite(owner) else {
            return;
        };
        battle.begin_attack();
        battle.emit(EventKind::TriggeredSkill, Some(sprite), None);
        let ratio = self.bird_damage_bp * if self.eidolon >= 6 { 2 } else { 1 };
        for target in battle
            .enemies()
            .filter(|&target| battle.is_targetable(target))
            .collect::<Vec<_>>()
        {
            battle.memosprite_hp_hit(sprite, target, Element::Wind, ratio, 10, Action::Skill);
        }
        if self.eidolon >= 1
            && let Some(target) = battle
                .enemies()
                .filter(|&target| battle.is_targetable(target))
                .max_by_key(|&target| (battle.units[target].hp, Reverse(target)))
        {
            let state = &mut battle.units[owner].robin_sunny_state;
            let amount = state
                .damage_record
                .saturating_mul(1_100 + 10 * u64::from(state.vibes))
                / 10_000;
            state.damage_record -= state.damage_record / 2;
            if amount > 0 {
                battle.true_damage(
                    sprite,
                    target,
                    Action::Skill,
                    u32::try_from(amount).unwrap_or(u32::MAX),
                    Vec::new(),
                );
            }
        }
        battle.gain_energy(sprite, owner, 20_000);
        battle.notify_attack(sprite, Action::Skill, None);
        battle.resolve_follow_ups();
    }

    fn crit_rate_bonus_bp(
        &self,
        _: &Battle<'_>,
        _: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        5_000 * u32::from(self.harmony)
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        self.fever_damage(battle, owner)
    }

    fn memosprite_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        sprite: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        battle
            .memosprite_owner(sprite)
            .map_or(0, |owner| self.fever_damage(battle, owner))
    }

    fn damage_target_aura(&self, battle: &Battle<'_>, owner: usize, _: usize) -> DamageTargetAura {
        let state = pos_state(battle, owner);
        DamageTargetAura {
            vulnerability_bp: state
                .birds
                .checked_sub(1)
                .map_or(0, |index| self.vulnerability_bp[usize::from(index)]),
            defense_down_bp: if self.singing(battle, owner) {
                self.fever_defense_base_bp + 50 * state.vibes
            } else {
                0
            },
            resistance_down_bp: 1_800 * u32::from(self.eidolon >= 2),
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Skill => Some(2),
            Action::Ultimate => {
                let favored = battle
                    .allies()
                    .filter(|&unit| {
                        unit != owner
                            && battle.is_targetable(unit)
                            && !battle.turn_is_suspended(unit)
                    })
                    .max_by_key(|&unit| {
                        (
                            battle.effective_attack(unit),
                            battle.gauge_quanta(unit),
                            Reverse(unit),
                        )
                    });
                Some(if target == favored { 0 } else { 5 })
            }
            _ => None,
        }
    }
}
