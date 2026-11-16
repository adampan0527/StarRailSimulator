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
    let crit = if critical { 15_000 } else { BP };
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    let numerator = u128::from(source.attack)
        * u128::from(multiplier_bp)
        * crit
        * defense_points
        * resist_factor(target.stats, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(5) * (defense_points + u128::from(target.stats.defense));
    (numerator / denominator) as u32
}

pub(crate) fn weakness_break(source: Stats, target: DamageTarget, element: Element) -> u32 {
    let defense_points = defense_points(source.level);
    let numerator = u128::from(BREAK_BASE_AT_LEVEL_30)
        * u128::from(element.break_multiplier_bp())
        * (20 + u128::from(target.max_toughness))
        * defense_points
        * resist_factor(target.stats, element)
        * (BP + u128::from(target.vulnerability_bp));
    let denominator = BP.pow(3) * 40 * (defense_points + u128::from(target.stats.defense));
    (numerator / denominator) as u32
}
