import assert from 'node:assert/strict';
import test from 'node:test';
import { compareCharacterRelease } from '../src/characters/releaseOrder.ts';

test('sorts by debut batch from newest to oldest, then by English name', () => {
  const ids = [
    'character.1009', // Asta
    'character.1014', // Saber
    'character.1510', // Himeko • Nova
    'character.1508', // Rin Tohsaka
    'character.1204', // Jing Yuan, 1.0 phase II
    'character.1008', // Arlan
    'character.1408', // Phainon
    'character.1509', // Gilgamesh
    'character.1015', // Archer
    'character.1511', // Aeon ★ Aha
  ];
  const sorted = ids.map((id) => ({ id })).sort(compareCharacterRelease);
  assert.deepEqual(
    sorted.map(({ id }) => id),
    [
      'character.1511',
      'character.1509',
      'character.1508',
      'character.1510',
      'character.1015',
      'character.1014',
      'character.1408',
      'character.1204',
      'character.1008',
      'character.1009',
    ],
  );
});

test('uses the debut of each selectable path and keeps undated entries last', () => {
  const ids = [
    'import.newcomer',
    'march_7th.preservation',
    'import.another',
    'march_7th.hunt',
    'character.1221', // Yunli
  ];
  const sorted = ids.map((id) => ({ id })).sort(compareCharacterRelease);
  assert.deepEqual(
    sorted.map(({ id }) => id),
    [
      'march_7th.hunt',
      'character.1221',
      'march_7th.preservation',
      'import.newcomer',
      'import.another',
    ],
  );
});
