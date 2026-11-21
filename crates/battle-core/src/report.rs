use serde::Serialize;

use crate::Action;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogMode {
    Off,
    Detailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ongoing,
    Victory,
    Defeat,
    TurnLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    Shield,
    Vulnerability,
    SpeedUp,
    Slow,
    DefenseDown,
    ResistanceDown,
    Frozen,
    DotVulnerability,
    EffectResistanceDown,
    Besotted,
    AttackDown,
    Entangled,
    SurvivalResponse,
    WisemansFolly,
    Imprisoned,
    Weightless,
    Arcana,
    Epiphany,
    WeaknessImplanted,
    UltimateVulnerability,
}

impl StatusKind {
    pub(crate) const COUNT: usize = Self::UltimateVulnerability.index() + 1;
    pub(crate) const ALL: [Self; 3] = [Self::Shield, Self::Vulnerability, Self::SpeedUp];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Shield => 0,
            Self::Vulnerability => 1,
            Self::SpeedUp => 2,
            Self::Slow => 3,
            Self::DefenseDown => 4,
            Self::ResistanceDown => 5,
            Self::Frozen => 6,
            Self::DotVulnerability => 7,
            Self::EffectResistanceDown => 8,
            Self::Besotted => 9,
            Self::AttackDown => 10,
            Self::Entangled => 11,
            Self::SurvivalResponse => 12,
            Self::WisemansFolly => 13,
            Self::Imprisoned => 14,
            Self::Weightless => 15,
            Self::Arcana => 16,
            Self::Epiphany => 17,
            Self::WeaknessImplanted => 18,
            Self::UltimateVulnerability => 19,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageKind {
    Direct,
    WeaknessBreak,
    DamageOverTime,
    Additional,
    FollowUp,
    TrueDamage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "event", content = "detail", rename_all = "snake_case")]
pub enum EventKind {
    WaveStarted,
    TurnStarted,
    ExtraTurnStarted,
    TriggeredSkill,
    Action(Action),
    FollowUp,
    EnemyAttack,
    SkillPoints {
        before: u8,
        after: u8,
    },
    Energy {
        before_milli: u32,
        after_milli: u32,
    },
    /// Flying Aureus uses half-points so progress from individual attacks is exact.
    FlyingAureus {
        before_halves: u8,
        after_halves: u8,
    },
    SlashedDream {
        before: u8,
        after: u8,
        overflow_before: u8,
        overflow_after: u8,
    },
    CrimsonKnot {
        before: u8,
        after: u8,
    },
    Damage {
        kind: DamageKind,
        amount: u32,
        hp_loss: u32,
        absorbed: u32,
        critical: bool,
    },
    Heal {
        amount: u32,
    },
    HpSpent {
        amount: u32,
    },
    ShockApplied {
        origin: Action,
        from_break: bool,
        turns: u32,
    },
    ShockExtended {
        extra_turns: u32,
    },
    ShockExpired {
        origin: Action,
        from_break: bool,
    },
    Toughness {
        before: u16,
        after: u16,
    },
    WeaknessBroken,
    GaugeShifted {
        before_quanta: u64,
        after_quanta: u64,
    },
    StatusApplied(StatusKind),
    StatusExpired(StatusKind),
    StatusCleared,
    ShieldDepleted,
    UnitDefeated,
    BattleEnded(Outcome),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Event {
    pub av_micros: u64,
    pub wave_index: usize,
    pub source_id: Option<String>,
    pub target_id: Option<String>,
    pub kind: EventKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnitSnapshot {
    pub unit_id: String,
    pub is_ally: bool,
    pub hp: u32,
    pub max_hp: u32,
    pub energy_milli: u32,
    pub max_energy_milli: u32,
    pub toughness: u16,
    pub max_toughness: u16,
    pub shield: u32,
    pub vulnerability_bp: u32,
    pub speed_micros: u64,
    /// Remaining gauge in trillionths of one gauge point.
    pub gauge_quanta: u64,
    pub normal_turns: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActionOrderEntry {
    pub unit_id: String,
    pub remaining_av_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub outcome: Outcome,
    /// Elapsed action value in millionths; this is not wall clock time.
    pub av_micros: u64,
    pub wave_index: usize,
    pub decision_index: u32,
    pub ordinary_actor_id: Option<String>,
    pub skill_points: u8,
    pub skill_point_cap: u8,
    pub random_draws: u32,
    /// Actual enemy HP removed; excludes overkill and damage to shields.
    pub enemy_hp_lost: u32,
    /// Allies, then enemies in order of appearance up through the current wave.
    pub units: Vec<UnitSnapshot>,
}
