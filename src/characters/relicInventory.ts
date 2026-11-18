import type { InventoryItem, RelicSummary } from './client';

export type RelicInventoryEntry = { item: InventoryItem; data: RelicSummary };

export type RelicFilters = {
  mainStats: string[];
  substats: string[];
  sets: string[];
  slots: string[];
  minimum: number;
};

export const emptyRelicFilters: RelicFilters = {
  mainStats: [],
  substats: [],
  sets: [],
  slots: [],
  minimum: 1,
};

const relicStatOrder = [
  'crit_rate',
  'crit_damage',
  'speed',
  'attack_ratio',
  'attack_flat',
  'hp_ratio',
  'hp_flat',
  'defense_ratio',
  'defense_flat',
  'breakdamageaddedratiobase',
  'statusresistancebase',
  'statusprobabilitybase',
  'spratiobase',
  'healratiobase',
  'physicaladdedratio',
  'fireaddedratio',
  'iceaddedratio',
  'thunderaddedratio',
  'windaddedratio',
  'quantumaddedratio',
  'imaginaryaddedratio',
];
const relicStatRanks = new Map(relicStatOrder.map((id, rank) => [id, rank]));

export function compareRelicStatOrder(first: string, second: string) {
  return (
    (relicStatRanks.get(first) ?? relicStatOrder.length) -
    (relicStatRanks.get(second) ?? relicStatOrder.length)
  );
}

export function filterRelics(
  entries: RelicInventoryEntry[],
  filters: RelicFilters,
) {
  const minimum = Math.min(filters.minimum, filters.substats.length);
  const includes = (selected: string[], id: string) =>
    selected.length === 0 || selected.includes(id);

  return entries.filter(
    ({ item, data }) =>
      includes(filters.mainStats, item.relic.main_stat) &&
      includes(filters.sets, data.set_id) &&
      includes(filters.slots, data.slot) &&
      (minimum === 0 ||
        filters.substats.filter((id) => Object.hasOwn(item.relic.substats, id))
          .length >= minimum),
  );
}
