use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

use crate::{
    BattleError,
    fixture::{Element, Stats},
};

const ENEMY_COMBAT_JSON: &str = include_str!("../../data/enemy-combat-4.7.0.json");
const BP_SCALE: f64 = 10_000.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialEnemy {
    pub unit_id: String,
    pub enemy_id: String,
    pub level: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfficialEnemyStats {
    pub level: u16,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub speed_micros: u64,
    pub max_toughness: u16,
    pub weaknesses: [bool; Element::COUNT],
    pub resist_bp: [u16; Element::COUNT],
    pub effect_hit_rate_bp: u32,
    pub effect_resistance_bp: u32,
    pub initial_delay_ratio_bp: u32,
}

#[derive(Clone, Default)]
pub(super) struct EnemyRuntimeState {
    pub sequence_cursor: usize,
    pub locked_target: Option<usize>,
    pub skill_cooldowns: Vec<u16>,
    pub barrier_active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnemyDebuffKind {
    DefenseDown,
    Slow,
    Weaken,
    EffectResistanceDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnemyDotScale {
    Attack(u16),
    TargetMaxHp(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnemyOnHit {
    Debuff {
        kind: EnemyDebuffKind,
        base_chance_bp: u32,
        magnitude_bp: u32,
        turns: u32,
    },
    ActionDelay {
        base_chance_bp: u32,
        delay_bp: u16,
    },
    Dot {
        element: Element,
        scale: EnemyDotScale,
        base_chance_bp: u32,
        turns: u32,
    },
    Reverberation {
        base_chance_bp: u32,
        turns: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnemySkillAction {
    SingleAttack {
        element: Element,
        multiplier_bp: u32,
        sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    },
    #[allow(dead_code)]
    BlastAttack {
        element: Element,
        main_multiplier_bp: u32,
        adjacent_multiplier_bp: u32,
        sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    },
    #[allow(dead_code)]
    AllAttack {
        element: Element,
        multiplier_bp: u32,
        sp_hit_milli: u32,
        on_hit: Option<EnemyOnHit>,
    },
    BounceAttack {
        element: Element,
        primary_multiplier_bp: u32,
        primary_sp_hit_milli: u32,
        bounce_count: u8,
        bounce_multiplier_bp: u32,
        bounce_sp_hit_milli: u32,
    },
    HealAllEnemies {
        max_hp_ratio_bp: u32,
    },
    SelfBarrierAndSingleAttack {
        element: Element,
        multiplier_bp: u32,
        sp_hit_milli: u32,
    },
}

#[derive(Clone, Debug)]
pub(super) struct EnemySkillPlan {
    #[allow(dead_code)]
    pub id: String,
    pub action: EnemySkillAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(super) struct CooldownDecision {
    pub skill_index: usize,
    pub weight_bp: u32,
    pub initial_cd: u16,
    pub cd: u16,
}

#[derive(Clone, Debug)]
pub(super) enum EnemyActionSelector {
    Sequence(Vec<usize>),
    #[allow(dead_code)]
    CooldownWeighted(Vec<CooldownDecision>),
}

#[derive(Clone, Debug)]
pub(super) struct OfficialEnemyKit {
    #[allow(dead_code)]
    pub enemy_id: String,
    pub max_toughness: u16,
    pub weakness: [bool; Element::COUNT],
    pub initial_delay_ratio_bp: u32,
    pub debuff_resist_bp: Vec<(String, u32)>,
    pub selector: EnemyActionSelector,
    pub skills: Vec<EnemySkillPlan>,
}

impl OfficialEnemyKit {
    pub fn init_runtime_state(&self) -> EnemyRuntimeState {
        let skill_cooldowns = match &self.selector {
            EnemyActionSelector::Sequence(_) => Vec::new(),
            EnemyActionSelector::CooldownWeighted(decisions) => decisions
                .iter()
                .map(|decision| decision.initial_cd)
                .collect(),
        };
        EnemyRuntimeState {
            sequence_cursor: 0,
            locked_target: None,
            skill_cooldowns,
            barrier_active: false,
        }
    }

    pub fn select_next_skill(&self, state: &mut EnemyRuntimeState) -> Option<usize> {
        match &self.selector {
            EnemyActionSelector::Sequence(sequence) => {
                if sequence.is_empty() {
                    return None;
                }
                let index = sequence[state.sequence_cursor % sequence.len()];
                state.sequence_cursor = state.sequence_cursor.wrapping_add(1);
                Some(index)
            }
            EnemyActionSelector::CooldownWeighted(decisions) => {
                if decisions.is_empty() {
                    return None;
                }
                if state.skill_cooldowns.len() != decisions.len() {
                    state.skill_cooldowns = decisions
                        .iter()
                        .map(|decision| decision.initial_cd)
                        .collect();
                }
                let chosen = decisions
                    .iter()
                    .enumerate()
                    .filter(|(slot, _)| state.skill_cooldowns[*slot] == 0)
                    .max_by(|(left_slot, left), (right_slot, right)| {
                        left.weight_bp
                            .cmp(&right.weight_bp)
                            .then_with(|| right_slot.cmp(left_slot))
                    })
                    .map(|(slot, _)| slot);
                for cd in &mut state.skill_cooldowns {
                    *cd = cd.saturating_sub(1);
                }
                let slot = chosen?;
                state.skill_cooldowns[slot] = decisions[slot].cd;
                Some(decisions[slot].skill_index)
            }
        }
    }

    pub fn debuff_resist_for(&self, key: &str) -> u32 {
        self.debuff_resist_bp
            .iter()
            .find_map(|(entry_key, bp)| (entry_key == key).then_some(*bp))
            .unwrap_or(0)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCatalog {
    level_curves: BTreeMap<String, Vec<RawLevelCurve>>,
    enemies: Vec<RawEnemy>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLevelCurve {
    level: u16,
    hp_ratio: f64,
    attack_ratio: f64,
    defense_ratio: f64,
    speed_ratio: f64,
    stance_ratio: f64,
    status_resistance: f64,
    status_probability: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDebuffResist {
    key: String,
    value: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCombatStats {
    hard_level_group: u32,
    hp_base: Option<f64>,
    hp_modify_ratio: f64,
    hp_elite_ratio: f64,
    attack_base: Option<f64>,
    attack_modify_ratio: f64,
    attack_elite_ratio: f64,
    defense_base: Option<f64>,
    defense_modify_ratio: f64,
    defense_elite_ratio: f64,
    speed_base: Option<f64>,
    speed_modify_ratio: f64,
    speed_elite_ratio: f64,
    speed_modify_value: Option<f64>,
    stance_base: f64,
    stance_modify_ratio: f64,
    initial_delay_ratio: f64,
    status_resistance_base: f64,
    damage_type_resistance: BTreeMap<String, f64>,
    debuff_resist: Vec<RawDebuffResist>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSkill {
    id: String,
    sp_hit_base: Option<f64>,
    params: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEnemy {
    id: String,
    weaknesses: Vec<String>,
    combat_stats: RawCombatStats,
    skill_sequence: Vec<String>,
    skills: Vec<RawSkill>,
}

struct EnemyCatalog {
    level_curves: BTreeMap<u32, Vec<RawLevelCurve>>,
    enemies: BTreeMap<String, RawEnemy>,
}

fn catalog() -> Result<&'static EnemyCatalog, BattleError> {
    static CATALOG: OnceLock<Result<EnemyCatalog, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let raw: RawCatalog =
                serde_json::from_str(ENEMY_COMBAT_JSON).map_err(|error| error.to_string())?;
            let mut level_curves = BTreeMap::new();
            for (group_key, rows) in raw.level_curves {
                let group_id = group_key
                    .parse::<u32>()
                    .map_err(|error| format!("invalid hardLevelGroup {group_key}: {error}"))?;
                level_curves.insert(group_id, rows);
            }
            let mut enemies = BTreeMap::new();
            for enemy in raw.enemies {
                enemies.insert(enemy.id.clone(), enemy);
            }
            Ok(EnemyCatalog {
                level_curves,
                enemies,
            })
        })
        .as_ref()
        .map_err(|error| BattleError::InvalidInput(format!("enemy catalog: {error}")))
}

/// Pure zero-passive `sequence_only` 4.7.0 enemies whose skill multipliers, hit splits,
/// and secondary effects have been verified directly against `ConfigAbility/Monster/*.json`.
const VERIFIED_PURE_SEQUENCE_ENEMIES: [&str; 16] = [
    "8001010", // 炎华造物
    "8001020", // 霜晶造物
    "1002040", // 银鬃近卫
    "1002050", // 银鬃射手
    "1012010", // 自动机兵•战犬
    "1012030", // 自动机兵•甲虫
    "1022010", // 永冬灾影
    "1022020", // 火焚灾影
    "2002030", // 幽府武弁
    "2011010", // 入魔机巧•率从狻猊
    "2011020", // 入魔机巧•浓云金蟾
    "2022060", // 「药王秘传」器元士
    "8001040", // 鸣雷造物
    "8001050", // 巽风造物
    "3001010", // 惊梦剧团的弹簧荷官
    "3011010", // 忆域迷因「视线无处不在」
];

pub fn supports_official_enemy(enemy_id: &str) -> bool {
    VERIFIED_PURE_SEQUENCE_ENEMIES.contains(&enemy_id)
}

pub fn official_enemy_stats(enemy_id: &str, level: u16) -> Result<OfficialEnemyStats, BattleError> {
    let catalog = catalog()?;
    let raw = catalog
        .enemies
        .get(enemy_id)
        .ok_or_else(|| BattleError::UnsupportedData(format!("enemy {enemy_id}")))?;
    resolve_enemy_stats(catalog, raw, level)
}

pub(super) fn build_official_enemy(
    enemy: &OfficialEnemy,
) -> Result<(OfficialEnemyKit, Stats), BattleError> {
    if !supports_official_enemy(&enemy.enemy_id) {
        return Err(BattleError::UnsupportedData(format!(
            "official enemy {}",
            enemy.enemy_id
        )));
    }
    let catalog = catalog()?;
    let raw = catalog
        .enemies
        .get(&enemy.enemy_id)
        .ok_or_else(|| BattleError::UnsupportedData(format!("enemy {}", enemy.enemy_id)))?;
    let resolved = resolve_enemy_stats(catalog, raw, enemy.level)?;
    let kit = build_verified_kit(raw, resolved)?;
    let stats = Stats {
        level: resolved.level,
        hp: resolved.hp,
        base_hp: resolved.hp,
        attack: resolved.attack,
        base_attack: resolved.attack,
        defense: resolved.defense,
        base_defense: resolved.defense,
        base_aggro: 100,
        base_speed_micros: resolved.speed_micros,
        flat_speed_micros: 0,
        resist_bp: resolved.resist_bp,
        crit_rate_bp: 0,
        crit_damage_bp: 0,
        element_damage_bp: [0; Element::COUNT],
        break_effect_bp: 0,
        effect_hit_rate_bp: resolved.effect_hit_rate_bp,
        effect_resistance_bp: resolved.effect_resistance_bp,
        energy_regeneration_bp: 0,
    };
    Ok((kit, stats))
}

fn resolve_enemy_stats(
    catalog: &EnemyCatalog,
    raw: &RawEnemy,
    level: u16,
) -> Result<OfficialEnemyStats, BattleError> {
    if !(1..=100).contains(&level) {
        return Err(BattleError::InvalidInput(format!(
            "enemy {} level {level} out of range 1..=100",
            raw.id
        )));
    }
    let curves = catalog
        .level_curves
        .get(&raw.combat_stats.hard_level_group)
        .ok_or_else(|| {
            BattleError::UnsupportedData(format!(
                "enemy {} hardLevelGroup {}",
                raw.id, raw.combat_stats.hard_level_group
            ))
        })?;
    let curve = curves
        .get(usize::from(level - 1))
        .filter(|row| row.level == level)
        .ok_or_else(|| {
            BattleError::InvalidInput(format!("enemy {} missing level curve {level}", raw.id))
        })?;
    let cs = &raw.combat_stats;
    let hp_base = cs
        .hp_base
        .ok_or_else(|| BattleError::UnsupportedData(format!("enemy {} missing hpBase", raw.id)))?;
    let attack_base = cs.attack_base.ok_or_else(|| {
        BattleError::UnsupportedData(format!("enemy {} missing attackBase", raw.id))
    })?;
    let defense_base = cs.defense_base.ok_or_else(|| {
        BattleError::UnsupportedData(format!("enemy {} missing defenseBase", raw.id))
    })?;
    let speed_base = cs.speed_base.ok_or_else(|| {
        BattleError::UnsupportedData(format!("enemy {} missing speedBase", raw.id))
    })?;
    let speed_offset = cs.speed_modify_value.ok_or_else(|| {
        BattleError::UnsupportedData(format!("enemy {} ambiguous speedModifyValue", raw.id))
    })?;

    let hp = scaled_u32(
        hp_base * cs.hp_modify_ratio * cs.hp_elite_ratio * curve.hp_ratio,
        "hp",
        &raw.id,
    )?;
    let attack = scaled_u32(
        attack_base * cs.attack_modify_ratio * cs.attack_elite_ratio * curve.attack_ratio,
        "attack",
        &raw.id,
    )?;
    let defense = scaled_u32(
        defense_base * cs.defense_modify_ratio * cs.defense_elite_ratio * curve.defense_ratio,
        "defense",
        &raw.id,
    )?;
    let speed_value = speed_base * cs.speed_modify_ratio * cs.speed_elite_ratio * curve.speed_ratio
        + speed_offset;
    let speed_micros = scaled_u64(speed_value * 1_000_000.0, "speed", &raw.id)?;
    // TurnBasedGameData StanceBase uses 3x internal units (30 = 10 display toughness = 1 Basic).
    let toughness_raw = cs.stance_base * cs.stance_modify_ratio * curve.stance_ratio / 3.0;
    let max_toughness = u16::try_from(scaled_u32(toughness_raw, "toughness", &raw.id)?)
        .map_err(|_| BattleError::InvalidInput(format!("enemy {} toughness overflow", raw.id)))?;

    let mut weaknesses = [false; Element::COUNT];
    for name in &raw.weaknesses {
        let element = Element::from_id(name).ok_or_else(|| {
            BattleError::InvalidInput(format!("enemy {} unknown weakness {name}", raw.id))
        })?;
        weaknesses[element.index()] = true;
    }
    let mut resist_bp = [0_u16; Element::COUNT];
    for (name, ratio) in &cs.damage_type_resistance {
        let element = Element::from_id(name).ok_or_else(|| {
            BattleError::InvalidInput(format!("enemy {} unknown resistance {name}", raw.id))
        })?;
        let bp = ratio_to_bp(*ratio, "resistance", &raw.id)?;
        resist_bp[element.index()] = u16::try_from(bp).map_err(|_| {
            BattleError::InvalidInput(format!("enemy {} resistance overflow", raw.id))
        })?;
    }
    let effect_hit_rate_bp = ratio_to_bp(curve.status_probability, "effectHitRate", &raw.id)?;
    let effect_resistance_bp = ratio_to_bp(
        cs.status_resistance_base + curve.status_resistance,
        "effectResistance",
        &raw.id,
    )?;
    let initial_delay_ratio_bp = ratio_to_bp(cs.initial_delay_ratio, "initialDelayRatio", &raw.id)?;

    Ok(OfficialEnemyStats {
        level,
        hp,
        attack,
        defense,
        speed_micros,
        max_toughness,
        weaknesses,
        resist_bp,
        effect_hit_rate_bp,
        effect_resistance_bp,
        initial_delay_ratio_bp,
    })
}

fn build_verified_kit(
    raw: &RawEnemy,
    stats: OfficialEnemyStats,
) -> Result<OfficialEnemyKit, BattleError> {
    let mut skills = Vec::with_capacity(raw.skills.len());
    for skill in &raw.skills {
        let action = resolve_verified_skill(&raw.id, skill)?;
        skills.push(EnemySkillPlan {
            id: skill.id.clone(),
            action,
        });
    }
    let mut sequence = Vec::with_capacity(raw.skill_sequence.len());
    for skill_id in &raw.skill_sequence {
        let position = skills
            .iter()
            .position(|skill| &skill.id == skill_id)
            .ok_or_else(|| {
                BattleError::InvalidInput(format!(
                    "enemy {} sequence references unknown skill {skill_id}",
                    raw.id
                ))
            })?;
        sequence.push(position);
    }
    if sequence.is_empty() {
        return Err(BattleError::UnsupportedData(format!(
            "enemy {} has empty skill sequence",
            raw.id
        )));
    }
    let mut debuff_resist_bp = Vec::with_capacity(raw.combat_stats.debuff_resist.len());
    for entry in &raw.combat_stats.debuff_resist {
        debuff_resist_bp.push((
            entry.key.clone(),
            ratio_to_bp(entry.value, "debuffResist", &raw.id)?,
        ));
    }
    Ok(OfficialEnemyKit {
        enemy_id: raw.id.clone(),
        max_toughness: stats.max_toughness,
        weakness: stats.weaknesses,
        initial_delay_ratio_bp: stats.initial_delay_ratio_bp,
        debuff_resist_bp,
        selector: EnemyActionSelector::Sequence(sequence),
        skills,
    })
}

fn resolve_verified_skill(
    enemy_id: &str,
    skill: &RawSkill,
) -> Result<EnemySkillAction, BattleError> {
    let param = |idx: usize| -> Result<f64, BattleError> {
        skill.params.get(idx).copied().ok_or_else(|| {
            BattleError::InvalidInput(format!(
                "enemy {enemy_id} skill {} missing param[{idx}]",
                skill.id
            ))
        })
    };
    let sp_milli = || -> Result<u32, BattleError> {
        let base = skill.sp_hit_base.ok_or_else(|| {
            BattleError::InvalidInput(format!(
                "enemy {enemy_id} skill {} missing spHitBase",
                skill.id
            ))
        })?;
        scaled_u32(base * 1_000.0, "spHitBase", enemy_id)
    };
    let param_bp = |idx: usize| -> Result<u32, BattleError> {
        ratio_to_bp(param(idx)?, "skillParam", enemy_id)
    };
    let param_u16_bp = |idx: usize| -> Result<u16, BattleError> {
        u16::try_from(param_bp(idx)?).map_err(|_| {
            BattleError::InvalidInput(format!(
                "enemy {enemy_id} skill {} param[{idx}] overflow",
                skill.id
            ))
        })
    };
    let param_turns =
        |idx: usize| -> Result<u32, BattleError> { scaled_u32(param(idx)?, "turns", enemy_id) };

    match (enemy_id, skill.id.as_str()) {
        // 8001010 炎华造物: Fire Single 250%, SP 10
        ("8001010", "800101001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Fire,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: None,
        }),
        // 8001020 霜晶造物: Ice Single 250%, SP 10
        ("8001020", "800102001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Ice,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: None,
        }),
        // 1002040 银鬃近卫: Physical Single 300%, SP 10, 100% DefenseDown 50% 2 turns
        ("1002040", "100204001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Debuff {
                kind: EnemyDebuffKind::DefenseDown,
                base_chance_bp: 10_000,
                magnitude_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 1002050 银鬃射手: Physical Single 300%, SP 10, 100% Bleed (15% target Max HP, 2 turns)
        ("1002050", "100205001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Dot {
                element: Element::Physical,
                scale: EnemyDotScale::TargetMaxHp(param_u16_bp(2)?),
                base_chance_bp: param_bp(1)?,
                turns: param_turns(3)?,
            }),
        }),
        // 1012010 自动机兵•战犬: Turn 1 Single 250% SP 15, Turn 2 Heal all enemies 25% Max HP
        ("1012010", "101201001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: None,
        }),
        ("1012010", "101201002") => Ok(EnemySkillAction::HealAllEnemies {
            max_hp_ratio_bp: param_bp(0)?,
        }),
        // 1012030 自动机兵•甲虫: Self Barrier + Physical Single 300%, SP 15
        ("1012030", "101203001") => Ok(EnemySkillAction::SelfBarrierAndSingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
        }),
        // 1022010 永冬灾影: Ice Single 300%, SP 15, Action Delay 50% (100% base chance)
        ("1022010", "102201001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Ice,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::ActionDelay {
                base_chance_bp: 10_000,
                delay_bp: param_u16_bp(1)?,
            }),
        }),
        // 1022020 火焚灾影: Fire Single 300%, SP 15, 100% Burn (50% ATK, 3 turns)
        ("1022020", "102202001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Fire,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Dot {
                element: Element::Fire,
                scale: EnemyDotScale::Attack(param_u16_bp(3)?),
                base_chance_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 2002030 幽府武弁: Physical Single 250%, SP 15, 35% Reverberation (2 turns)
        ("2002030", "200203001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Reverberation {
                base_chance_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 2011010 入魔机巧•率从狻猊: Fire Single 200%, SP 10, 100% Weaken 30% (2 turns)
        ("2011010", "201101001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Fire,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Debuff {
                kind: EnemyDebuffKind::Weaken,
                base_chance_bp: param_bp(1)?,
                magnitude_bp: param_bp(2)?,
                turns: param_turns(3)?,
            }),
        }),
        // 2011020 入魔机巧•浓云金蟾: Imaginary Single 200%, SP 10, 100% Slow 25% (2 turns)
        ("2011020", "201102001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Imaginary,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Debuff {
                kind: EnemyDebuffKind::Slow,
                base_chance_bp: param_bp(1)?,
                magnitude_bp: param_bp(2)?,
                turns: param_turns(3)?,
            }),
        }),
        // 2022060 「药王秘传」器元士: Thunder Bounce 1 primary (SP 15) + 5 random bounces (SP 3 each)
        ("2022060", "202206001") => {
            let total_hits = scaled_u32(param(1)?, "bounceHits", enemy_id)?;
            let bounce_count = u8::try_from(total_hits.saturating_sub(1)).map_err(|_| {
                BattleError::InvalidInput(format!("enemy {enemy_id} invalid bounce count"))
            })?;
            let primary_sp = sp_milli()?;
            let multiplier_bp = param_bp(0)?;
            Ok(EnemySkillAction::BounceAttack {
                element: Element::Thunder,
                primary_multiplier_bp: multiplier_bp,
                primary_sp_hit_milli: primary_sp,
                bounce_count,
                bounce_multiplier_bp: multiplier_bp,
                bounce_sp_hit_milli: primary_sp / 5,
            })
        }
        // 8001040 鸣雷造物: Thunder Single 250%, SP 10, 100% Shock (80% ATK, 2 turns)
        ("8001040", "800104001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Thunder,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Dot {
                element: Element::Thunder,
                scale: EnemyDotScale::Attack(param_u16_bp(3)?),
                base_chance_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 8001050 巽风造物: Wind Single 250%, SP 10, 100% WindShear (80% ATK, 2 turns)
        ("8001050", "800105001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Wind,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Dot {
                element: Element::Wind,
                scale: EnemyDotScale::Attack(param_u16_bp(3)?),
                base_chance_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 3001010 惊梦剧团的弹簧荷官: Physical Single 250%, SP 10, 100% EffectResDown 20% (1 turn)
        ("3001010", "300101001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Physical,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::Debuff {
                kind: EnemyDebuffKind::EffectResistanceDown,
                base_chance_bp: 10_000,
                magnitude_bp: param_bp(1)?,
                turns: param_turns(2)?,
            }),
        }),
        // 3011010 忆域迷因「视线无处不在」: Quantum Single 250%, SP 10, Action Delay 20%
        ("3011010", "301101001") => Ok(EnemySkillAction::SingleAttack {
            element: Element::Quantum,
            multiplier_bp: param_bp(0)?,
            sp_hit_milli: sp_milli()?,
            on_hit: Some(EnemyOnHit::ActionDelay {
                base_chance_bp: 10_000,
                delay_bp: param_u16_bp(1)?,
            }),
        }),
        _ => Err(BattleError::UnsupportedData(format!(
            "enemy {enemy_id} skill {}",
            skill.id
        ))),
    }
}

fn scaled_u32(value: f64, field: &str, enemy_id: &str) -> Result<u32, BattleError> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return Err(BattleError::InvalidInput(format!(
            "enemy {enemy_id} invalid {field}: {value}"
        )));
    }
    Ok(value.round() as u32)
}

fn scaled_u64(value: f64, field: &str, enemy_id: &str) -> Result<u64, BattleError> {
    if !value.is_finite() || value < 0.0 || value > u64::MAX as f64 {
        return Err(BattleError::InvalidInput(format!(
            "enemy {enemy_id} invalid {field}: {value}"
        )));
    }
    Ok(value.round() as u64)
}

fn ratio_to_bp(value: f64, field: &str, enemy_id: &str) -> Result<u32, BattleError> {
    scaled_u32(value * BP_SCALE, field, enemy_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_weighted_selector_respects_initial_cd_cd_and_weight_priority() {
        let kit = OfficialEnemyKit {
            enemy_id: "test.cd".into(),
            max_toughness: 20,
            weakness: [false; Element::COUNT],
            initial_delay_ratio_bp: 10_000,
            debuff_resist_bp: Vec::new(),
            selector: EnemyActionSelector::CooldownWeighted(vec![
                CooldownDecision {
                    skill_index: 0,
                    weight_bp: 8_000,
                    initial_cd: 0,
                    cd: 0,
                },
                CooldownDecision {
                    skill_index: 1,
                    weight_bp: 10_000,
                    initial_cd: 1,
                    cd: 2,
                },
            ]),
            skills: vec![
                EnemySkillPlan {
                    id: "s1".into(),
                    action: EnemySkillAction::SingleAttack {
                        element: Element::Physical,
                        multiplier_bp: 10_000,
                        sp_hit_milli: 10_000,
                        on_hit: None,
                    },
                },
                EnemySkillPlan {
                    id: "s2".into(),
                    action: EnemySkillAction::HealAllEnemies {
                        max_hp_ratio_bp: 2_500,
                    },
                },
            ],
        };
        let mut state = kit.init_runtime_state();
        assert_eq!(state.skill_cooldowns, vec![0, 1]);
        // Turn 1: skill 1 on initial_cd=1 -> selects skill 0, decrements skill 1 cd to 0.
        assert_eq!(kit.select_next_skill(&mut state), Some(0));
        // Turn 2: both ready, skill 1 has higher weight -> selects skill 1, sets its cd to 2.
        assert_eq!(kit.select_next_skill(&mut state), Some(1));
        // Turn 3: skill 1 cd=2 -> selects skill 0, decrements skill 1 cd to 1.
        assert_eq!(kit.select_next_skill(&mut state), Some(0));
        // Turn 4: skill 1 cd=1 -> selects skill 0, decrements skill 1 cd to 0.
        assert_eq!(kit.select_next_skill(&mut state), Some(0));
        // Turn 5: skill 1 ready again -> selects skill 1.
        assert_eq!(kit.select_next_skill(&mut state), Some(1));
    }

    #[test]
    fn sequence_selector_cycles_deterministically() {
        let kit = OfficialEnemyKit {
            enemy_id: "test.seq".into(),
            max_toughness: 10,
            weakness: [false; Element::COUNT],
            initial_delay_ratio_bp: 10_000,
            debuff_resist_bp: vec![("STAT_DOT_Burn".into(), 10_000)],
            selector: EnemyActionSelector::Sequence(vec![0, 1]),
            skills: Vec::new(),
        };
        let mut state = kit.init_runtime_state();
        assert_eq!(kit.select_next_skill(&mut state), Some(0));
        assert_eq!(kit.select_next_skill(&mut state), Some(1));
        assert_eq!(kit.select_next_skill(&mut state), Some(0));
        assert_eq!(kit.debuff_resist_for("STAT_DOT_Burn"), 10_000);
        assert_eq!(kit.debuff_resist_for("STAT_DOT_Bleed"), 0);
    }
}
