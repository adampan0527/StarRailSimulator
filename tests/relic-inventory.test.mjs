import assert from 'node:assert/strict';
import test from 'node:test';
import {
  compareRelicStatOrder,
  emptyRelicFilters,
  filterRelics,
} from '../src/characters/relicInventory.ts';

function relic(id, mainStat, substats, setId, slot) {
  return {
    item: {
      id,
      relic: {
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
  assert.deepEqual(ids({ ...exampleFilters, minimum: 1 }), [1, 2, 3, 7]);
  assert.deepEqual(
    ids({ ...exampleFilters, substats: ['speed', 'hp_ratio', 'crit_rate'] }),
    [],
  );
});

test('empty filter groups impose no restriction, including an empty substat selection', () => {
  assert.deepEqual(ids(emptyRelicFilters), [1, 2, 3, 4, 5, 6, 7]);
  assert.deepEqual(ids({ ...exampleFilters, substats: [] }), [1, 2, 3, 7]);
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
