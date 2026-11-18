const statIds = {
  HPDelta: 'hp_flat',
  AttackDelta: 'attack_flat',
  DefenceDelta: 'defense_flat',
  Speed: 'speed_total',
  SpeedDelta: 'speed',
  HPAddedRatio: 'hp_ratio',
  AttackAddedRatio: 'attack_ratio',
  DefenceAddedRatio: 'defense_ratio',
  SpeedAddedRatio: 'speed_ratio',
  CriticalChanceBase: 'crit_rate',
  CriticalDamageBase: 'crit_damage',
};

export const propertyId = (key) => statIds[key] ?? key.toLowerCase();
