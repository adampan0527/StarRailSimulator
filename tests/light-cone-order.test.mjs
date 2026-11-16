import assert from 'node:assert/strict';
import test from 'node:test';
import { compareLightConeRelease } from '../src/characters/releaseOrder.ts';

test('orders light cones by version, rarity, debut date, then English name', () => {
  const entries = [
    ['cone.23001', 5], // In the Night, 1.0 phase I
    ['cone.23062', 5], // I Am As You Behold, 4.4 collaboration
    ['cone.23010', 5], // Before Dawn, 1.0 phase II
    ['cone.23065', 5], // Upon the First Echo of "Aha", 4.7
    ['cone.23060', 5], // A Star That Lights the Night, 4.4 phase I
    ['cone.20003', 3], // Amber, 1.0 phase I
    ['cone.23061', 5], // Flickering Stars, 4.4 collaboration
    ['cone.22009', 4], // See You at the World's End!, 4.7
    ['cone.23055', 5], // Colors for Tomorrow, 4.6
    ['cone.22003', 4], // Ninja Record: Sound Hunt, Oct 25 in 2.6
    ['cone.21048', 4], // Dream's Montage, Oct 23 in 2.6
    ['cone.21047', 4], // Shadowed by Night, 2.5 phase II
    ['cone.23031', 5], // I Venture Forth to Hunt, 2.5 phase I
    ['cone.21012', 4], // A Secret Vow, 1.0 phase I
  ];
  const sorted = entries
    .map(([data_id, rarity]) => ({ default: { data_id }, rarity }))
    .sort(compareLightConeRelease);
  assert.deepEqual(
    sorted.map((entry) => entry.default.data_id),
    [
      'cone.23065',
      'cone.22009',
      'cone.23055',
      'cone.23061',
      'cone.23062',
      'cone.23060',
      'cone.22003',
      'cone.21048',
      'cone.23031',
      'cone.21047',
      'cone.23010',
      'cone.23001',
      'cone.21012',
      'cone.20003',
    ],
  );
});

test('keeps duplicate inventory instances and imports without a date stable', () => {
  const items = [
    { id: 1 },
    { id: 3, data: { default: { data_id: 'cone.23065' }, rarity: 5 } },
    { id: 8, data: { default: { data_id: 'cone.22009' }, rarity: 4 } },
    { id: 4, data: { default: { data_id: 'cone.23065' }, rarity: 5 } },
    { id: 7, data: { default: { data_id: 'import.other' }, rarity: 5 } },
  ];
  items.sort((first, second) =>
    compareLightConeRelease(first.data, second.data),
  );
  assert.deepEqual(
    items.map(({ id }) => id),
    [3, 4, 8, 1, 7],
  );
});
