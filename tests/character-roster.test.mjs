import assert from 'node:assert/strict';
import test from 'node:test';
import { compareCharacterRelease } from '../src/characters/releaseOrder.ts';

test('orders characters by version, rarity, debut date, then English name', () => {
  const roster = [
    { id: 'character.1009', rarity: 4 }, // Asta
    { id: 'character.1014', rarity: 5 }, // Saber
    { id: 'character.1510', rarity: 5 }, // Himeko • Nova
    { id: 'character.1508', rarity: 5 }, // Rin Tohsaka
    { id: 'character.1204', rarity: 5 }, // Jing Yuan, 1.0 phase II
    { id: 'character.1008', rarity: 4 }, // Arlan
    { id: 'character.1408', rarity: 5 }, // Phainon
    { id: 'character.1509', rarity: 5 }, // Gilgamesh
    { id: 'character.1015', rarity: 5 }, // Archer
    { id: 'character.1511', rarity: 5 }, // Aeon ★ Aha
    { id: 'character.1003', rarity: 5 }, // Himeko
    { id: 'character.1104', rarity: 5 }, // Gepard
  ];
  const sorted = roster.sort(compareCharacterRelease);
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
      'character.1104',
      'character.1003',
      'character.1008',
      'character.1009',
    ],
  );
});

test('uses the debut of each selectable path and keeps undated entries last', () => {
  const roster = [
    { id: 'import.newcomer', rarity: 5 },
    { id: 'march_7th.preservation', rarity: 4 },
    { id: 'import.another', rarity: 4 },
    { id: 'march_7th.hunt', rarity: 4 },
    { id: 'character.1221', rarity: 5 }, // Yunli
    { id: 'character.1223', rarity: 4 }, // Moze, 2.5
  ];
  const sorted = roster.sort(compareCharacterRelease);
  assert.deepEqual(
    sorted.map(({ id }) => id),
    [
      'character.1223',
      'character.1221',
      'march_7th.hunt',
      'march_7th.preservation',
      'import.newcomer',
      'import.another',
    ],
  );
});
