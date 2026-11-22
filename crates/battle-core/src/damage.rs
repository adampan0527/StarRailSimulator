use crate::fixture::{BREAK_BASE_AT_LEVEL_30, Element, Stats};

const BP: u128 = 10_000;
const LEVEL_80_BREAK_BASE_X_BP: u128 = 37_675_533;

#[derive(Clone, Copy)]
pub(crate) struct DamageTarget {
    pub stats: Stats,
    pub vulnerability_bp: u32,
    pub defense_down_bp: u32,
    pub resistance_down_bp: u32,
    pub damage_reduction_bp: u32,
    pub break_damage_bonus_bp: u32,
    pub damage_share_bp: u16,
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

fn break_base_x_bp(level: u16) -> u128 {
    if level == 80 {
        LEVEL_80_BREAK_BASE_X_BP
    } else {
        u128::from(BREAK_BASE_AT_LEVEL_30) * BP
    }
}

fn damage_after_reduction(numerator: u128, denominator: u128, target: DamageTarget) -> u32 {
    let reduction = u128::from(target.damage_reduction_bp).min(BP);
    let reduction_factor = BP - reduction;
    // Split the unmitigated hit before folding in the recipient's defensive stats.
    let (scale, factor) = if u128::from(target.damage_share_bp) == BP {
        (BP, reduction_factor)
    } else {
        (
            BP * BP,
            reduction_factor * u128::from(target.damage_share_bp),
        )
    };
    let amount =
        (numerator / denominator * factor + numerator % denominator * factor / denominator) / scale;
    u32::try_from(amount).unwrap_or(u32::MAX)
}

/// Keep every multiplier as a ratio and truncate once after multiplying.
pub(crate) fn direct(
    source: Stats,
    target: DamageTarget,
    element: Element,
    base_bp: u64,
    critical: bool,
) -> u32 {
    let defense_points = defense_points(source.level);
    let crit = if critical {
        BP + u128::from(source.crit_damage_bp)
    } else {
        BP
    };
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    let numerator = u128::from(base_bp)
        * (BP + u128::from(source.element_damage_bp[element.index()]))
        * crit
        * defense_points
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(5) * defense_factor(defense_points, target);
    damage_after_reduction(numerator, denominator, target)
}

pub(crate) fn weakness_break(
    source: Stats,
    target: DamageTarget,
    element: Element,
    ratio_bp: u32,
) -> u32 {
    let defense_points = defense_points(source.level);
    let base = break_base_x_bp(source.level);
    let numerator = base
        * u128::from(element.break_multiplier_bp())
        * (20 + u128::from(target.max_toughness))
        * defense_points
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * (BP + u128::from(source.break_effect_bp))
        * u128::from(ratio_bp);
    let denominator = BP.pow(5) * 40 * defense_factor(defense_points, target);
    break_damage_after_reduction(numerator, denominator, target)
}

/// Toughness uses the same scale as the rest of combat, with 10 for an ordinary Basic ATK.
pub(crate) fn super_break(
    source: Stats,
    target: DamageTarget,
    element: Element,
    toughness: u32,
    ratio_bp: u16,
) -> u32 {
    let defense = defense_points(source.level);
    let numerator = break_base_x_bp(source.level)
        * u128::from(toughness)
        * u128::from(ratio_bp)
        * (BP + u128::from(source.break_effect_bp))
        * defense
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp));
    let denominator = 10 * BP.pow(4) * defense_factor(defense, target);
    break_damage_after_reduction(numerator, denominator, target)
}

pub(crate) fn break_status(
    source: Stats,
    target: DamageTarget,
    element: Element,
    ratio_bp: u32,
) -> u32 {
    let defense = defense_points(source.level);
    let toughness = if target.toughness > 0 { 9_000 } else { BP };
    let large_dummy = target.max_toughness >= 120;
    let base = match element {
        Element::Imaginary => return 0, // Imprisonment deals no periodic damage.
        Element::Ice | Element::Fire => LEVEL_80_BREAK_BASE_X_BP,
        Element::Quantum => LEVEL_80_BREAK_BASE_X_BP * (20 + u128::from(target.max_toughness)) / 40,
        Element::Thunder => 2 * LEVEL_80_BREAK_BASE_X_BP,
        Element::Wind => {
            if large_dummy {
                3 * LEVEL_80_BREAK_BASE_X_BP
            } else {
                LEVEL_80_BREAK_BASE_X_BP
            }
        }
        Element::Physical => {
            let hp_bp = if large_dummy { 700 } else { 1_600 };
            let from_hp = u128::from(target.stats.hp) * hp_bp;
            let cap = LEVEL_80_BREAK_BASE_X_BP * 2 * (20 + u128::from(target.max_toughness)) / 40;
            from_hp.min(cap)
        }
    };
    let numerator = base * u128::from(ratio_bp) / BP
        * (BP + u128::from(source.break_effect_bp))
        * defense
        * resist_factor(target, element)
        * (BP + u128::from(target.vulnerability_bp))
        * toughness;
    let denominator = BP.pow(4) * defense_factor(defense, target);
    break_damage_after_reduction(numerator, denominator, target)
}

fn break_damage_after_reduction(numerator: u128, denominator: u128, target: DamageTarget) -> u32 {
    if target.break_damage_bonus_bp == 0 {
        return damage_after_reduction(numerator, denominator, target);
    }
    damage_after_reduction(
        numerator * (BP + u128::from(target.break_damage_bonus_bp)),
        denominator * BP,
        target,
    )
}
