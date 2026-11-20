use crate::fixture::{BREAK_BASE_AT_LEVEL_30, Element, Stats};

const BP: u128 = 10_000;

#[derive(Clone, Copy)]
pub(crate) struct DamageTarget {
    pub stats: Stats,
    pub vulnerability_bp: u32,
    pub defense_down_bp: u32,
    pub resistance_down_bp: u32,
    pub toughness: u16,
    pub max_toughness: u16,
}

fn defense_points(level: u16) -> u128 {
    200 + 10 * u128::from(level)
}

fn resist_factor(target: DamageTarget, element: Element) -> u128 {
    let resistance = u128::from(target.stats.resist_bp[element.index()]);
    (BP + u128::from(target.resistance_down_bp)).saturating_sub(resistance)
}

fn defense_factor(source_defense: u128, target: DamageTarget) -> u128 {
    source_defense * BP
        + u128::from(target.stats.defense) * BP.saturating_sub(u128::from(target.defense_down_bp))
}

/// Keep every multiplier as a ratio and truncate once after multiplying.
pub(crate) fn direct(
    source: Stats,
    target: DamageTarget,
    element: Element,
    multiplier_bp: u16,
    critical: bool,
) -> u32 {
    let defense_points = defense_points(source.level);
    let crit = if critical {
        BP + u128::from(source.crit_damage_bp)
    } else {
        BP
    };
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    let numerator = u128::from(source.attack)
        * u128::from(multiplier_bp)
        * (BP + u128::from(source.element_damage_bp))
        * crit
        * defense_points
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(5) * defense_factor(defense_points, target);
    u32::try_from(numerator / denominator).unwrap_or(u32::MAX)
}

pub(crate) fn weakness_break(source: Stats, target: DamageTarget, element: Element) -> u32 {
    let defense_points = defense_points(source.level);
    let base = if source.level == 80 {
        37_675_533
    } else {
        u128::from(BREAK_BASE_AT_LEVEL_30) * BP
    };
    let numerator = base
        * u128::from(element.break_multiplier_bp())
        * (20 + u128::from(target.max_toughness))
        * defense_points
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * (BP + u128::from(source.break_effect_bp));
    let denominator = BP.pow(4) * 40 * defense_factor(defense_points, target);
    u32::try_from(numerator / denominator).unwrap_or(u32::MAX)
}

pub(crate) fn break_status(source: Stats, target: DamageTarget, element: Element) -> u32 {
    let defense = defense_points(source.level);
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    const LEVEL_80_BASE_X_BP: u128 = 37_675_533;
    let large_dummy = target.max_toughness >= 120;
    let base = match element {
        Element::Ice => LEVEL_80_BASE_X_BP,
        Element::Thunder => 2 * LEVEL_80_BASE_X_BP,
        Element::Wind => {
            if large_dummy {
                3 * LEVEL_80_BASE_X_BP
            } else {
                LEVEL_80_BASE_X_BP
            }
        }
        Element::Physical => {
            let hp_bp = if large_dummy { 700 } else { 1_600 };
            let from_hp = u128::from(target.stats.hp) * hp_bp;
            let cap = LEVEL_80_BASE_X_BP * 2 * (20 + u128::from(target.max_toughness)) / 40;
            from_hp.min(cap)
        }
    };
    let numerator = base
        * (BP + u128::from(source.break_effect_bp))
        * defense
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(4) * defense_factor(defense, target);
    u32::try_from(numerator / denominator).unwrap_or(u32::MAX)
}
