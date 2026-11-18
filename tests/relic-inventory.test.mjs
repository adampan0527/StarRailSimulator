import assert from 'node:assert/strict';
import test from 'node:test';
import {
  compareRelicSetRelease,
  compareRelicStatOrder,
  emptyRelicFilters,
  filterRelics,
} from '../src/characters/relicInventory.ts';

function relic(id, mainStat, substats, setId, slot, level = 0) {
  return {
    item: {
      id,
      relic: {
        level,
        main_stat: mainStat,
        substats: Object.fromEntries(substats.map((stat) => [stat, 1])),
      },
    },
    data: { set_id: setId, slot },
  };
}

const entries = [
  relic(1, 'crit_rate', ['speed', 'hp_ratio'], 'set.a', 'body'),
  relic(2, 'crit_damage', ['speed', 'hp_ratio'], 'set.b', 'body'),
  relic(3, 'crit_damage', ['speed', 'hp_flat'], 'set.a', 'body'),
  relic(4, 'crit_rate', ['speed', 'hp_ratio'], 'set.c', 'body'),
  relic(5, 'attack_ratio', ['speed', 'hp_ratio'], 'set.a', 'body'),
  relic(6, 'crit_rate', ['speed', 'hp_ratio'], 'set.a', 'feet'),
  relic(7, 'crit_damage', ['speed', 'hp_ratio'], 'set.b', 'planar_sphere'),
];
const exampleFilters = {
  ...emptyRelicFilters,
  mainStats: ['crit_rate', 'crit_damage'],
  substats: ['speed', 'hp_ratio'],
  sets: ['set.a', 'set.b'],
  slots: ['body', 'planar_sphere'],
  minimum: 3,
};
const ids = (filters) =>
  filterRelics(entries, filters).map(({ item }) => item.id);

test('intersects groups and caps the minimum at the number of selected substats', () => {
  assert.deepEqual(ids(exampleFilters), [1, 2, 7]);
  assert.deepEqual(ids({ ...exampleFilters, minimum: 1 }), [1, 3, 2, 7]);
  assert.deepEqual(
    ids({ ...exampleFilters, substats: ['speed', 'hp_ratio', 'crit_rate'] }),
    [],
  );
});

test('empty filter groups impose no restriction, including an empty substat selection', () => {
  assert.deepEqual(ids(emptyRelicFilters), [1, 3, 5, 6, 2, 7, 4]);
  assert.deepEqual(ids({ ...exampleFilters, substats: [] }), [1, 3, 2, 7]);
});

test('orders relic sets by release version, then set ID within a version', () => {
  const setIds = [
    'set.308',
    'set.326',
    'set.330',
    'set.133',
    'set.101',
    'set.129',
    'set.custom',
    'set.329',
  ];
  assert.deepEqual(setIds.sort(compareRelicSetRelease), [
    'set.329',
    'set.330',
    'set.133',
    'set.326',
    'set.129',
    'set.101',
    'set.308',
    'set.custom',
  ]);
});

test('orders matching cards by enhancement level, then set release order', () => {
  const inventory = [
    relic(1, 'crit_rate', [], 'set.329', 'planar_sphere', 9),
    relic(2, 'crit_rate', [], 'set.101', 'body', 15),
    relic(3, 'crit_rate', [], 'set.330', 'planar_sphere', 15),
    relic(4, 'crit_rate', [], 'set.329', 'planar_sphere', 15),
    relic(5, 'crit_damage', [], 'set.329', 'planar_sphere', 15),
    relic(6, 'crit_rate', [], 'set.329', 'planar_sphere', 15),
  ];
  const matches = filterRelics(inventory, {
    ...emptyRelicFilters,
    mainStats: ['crit_rate'],
    sets: ['set.101', 'set.330', 'set.329'],
  });
  assert.deepEqual(
    matches.map(({ item }) => item.id),
    [4, 6, 3, 2, 1],
  );
  assert.deepEqual(
    inventory.map(({ item }) => item.id),
    [1, 2, 3, 4, 5, 6],
  );
});

test('orders relic stats by priority and leaves unranked stats at the end', () => {
  const substats = [
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
  ];
  assert.deepEqual(substats.toReversed().sort(compareRelicStatOrder), substats);

  const mainStats = [
    ...substats,
    'spratiobase',
    'healratiobase',
    'physicaladdedratio',
    'fireaddedratio',
    'iceaddedratio',
    'thunderaddedratio',
    'windaddedratio',
    'quantumaddedratio',
    'imaginaryaddedratio',
    'extra_stat',
  ];
  assert.deepEqual(
    mainStats.toReversed().sort(compareRelicStatOrder),
    mainStats,
  );
});
