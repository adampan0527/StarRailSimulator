use crate::BattleError;

pub(super) const MICROS: u64 = 1_000_000;

#[derive(Clone, Copy)]
pub(super) struct Healing {
    pub(super) hp_ratio_micros: u64,
    pub(super) flat_micros: u64,
}

impl Healing {
    pub(super) fn from_data(
        character: &str,
        skill: &str,
        hp_ratio: f64,
        flat: f64,
    ) -> Result<Self, BattleError> {
        Ok(Self {
            hp_ratio_micros: healing_micros(character, skill, hp_ratio, 10.0)?,
            flat_micros: healing_micros(character, skill, flat, 1_000_000.0)?,
        })
    }

    pub(super) fn amount(self, caster_max_hp: u32, boost_micros: u64) -> u32 {
        self.scaled_amount(caster_max_hp, boost_micros, MICROS)
    }

    pub(super) fn scaled_amount(
        self,
        caster_max_hp: u32,
        boost_micros: u64,
        scale_micros: u64,
    ) -> u32 {
        let raw = u128::from(caster_max_hp) * u128::from(self.hp_ratio_micros)
            + u128::from(self.flat_micros);
        let scaled =
            raw * (u128::from(MICROS) + u128::from(boost_micros)) * u128::from(scale_micros);
        u32::try_from(scaled / u128::from(MICROS).pow(3)).unwrap_or(u32::MAX)
    }
}

pub(super) fn healing_micros(
    character: &str,
    skill: &str,
    value: f64,
    limit: f64,
) -> Result<u64, BattleError> {
    if !(0.0..=limit).contains(&value) {
        return Err(BattleError::InvalidInput(format!(
            "{character} {skill} heal coefficient"
        )));
    }
    Ok((value * MICROS as f64).round() as u64)
}
