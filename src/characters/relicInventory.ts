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

// ReleaseVersion from RelicSetConfig.json in the 4.7.0 source lock.
// Newest version first, with set IDs ascending within each version.
const relicSetReleaseOrder = [
  [329, 330], // 4.7
  [133, 134], // 4.6
  [327, 328], // 4.4
  [131, 132], // 4.3
  [325, 326], // 4.1
  [129, 130], // 4.0
  [323, 324], // 3.7
  [127, 128], // 3.6
  [321, 322], // 3.4
  [125, 126], // 3.3
  [319, 320], // 3.1
  [123, 124], // 3.0
  [121, 122], // 2.6
  [317, 318], // 2.5
  [119, 120, 315, 316], // 2.3
  [313, 314], // 2.1
  [117, 118], // 2.0
  [115, 116, 311, 312], // 1.5
  [113, 114, 309, 310], // 1.2
  [
    101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 301, 302, 303,
    304, 305, 306, 307, 308,
  ], // 1.0
].flat();
const relicSetReleaseRanks = new Map(
  relicSetReleaseOrder.map((id, rank) => [`set.${id}`, rank]),
);

export function compareRelicSetRelease(first: string, second: string) {
  return (
    (relicSetReleaseRanks.get(first) ?? relicSetReleaseOrder.length) -
      (relicSetReleaseRanks.get(second) ?? relicSetReleaseOrder.length) ||
    first.localeCompare(second, 'en')
  );
}

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

  return entries
    .filter(
      ({ item, data }) =>
        includes(filters.mainStats, item.relic.main_stat) &&
        includes(filters.sets, data.set_id) &&
        includes(filters.slots, data.slot) &&
        (minimum === 0 ||
          filters.substats.filter((id) =>
            Object.hasOwn(item.relic.substats, id),
          ).length >= minimum),
    )
    .sort(
      (first, second) =>
        second.item.relic.level - first.item.relic.level ||
        compareRelicSetRelease(first.data.set_id, second.data.set_id),
    );
}
