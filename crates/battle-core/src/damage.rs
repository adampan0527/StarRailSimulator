use crate::fixture::{BREAK_BASE_AT_LEVEL_30, Element, Stats};

const BP: u128 = 10_000;

#[derive(Clone, Copy)]
pub(crate) struct DamageTarget {
    pub stats: Stats,
    pub vulnerability_bp: u32,
    pub toughness: u16,
    pub max_toughness: u16,
}

fn defense_points(level: u16) -> u128 {
    200 + 10 * u128::from(level)
}

fn resist_factor(target: Stats, element: Element) -> u128 {
    BP - u128::from(target.resist_bp[element.index()])
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
        * resist_factor(target.stats, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(6) * (defense_points + u128::from(target.stats.defense));
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
        * resist_factor(target.stats, element)
        * (BP + u128::from(target.vulnerability_bp))
        * (BP + u128::from(source.break_effect_bp));
    let denominator = BP.pow(5) * 40 * (defense_points + u128::from(target.stats.defense));
    u32::try_from(numerator / denominator).unwrap_or(u32::MAX)
}

pub(crate) fn break_status(source: Stats, target: DamageTarget, element: Element) -> u32 {
    let defense = defense_points(source.level);
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    let multiplier = if matches!(element, Element::Thunder) {
        2
    } else {
        1
    };
    let numerator = 37_675_533
        * multiplier
        * (BP + u128::from(source.break_effect_bp))
        * defense
        * resist_factor(target.stats, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(5) * (defense + u128::from(target.stats.defense));
    u32::try_from(numerator / denominator).unwrap_or(u32::MAX)
}
