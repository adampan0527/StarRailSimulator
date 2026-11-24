import assert from 'node:assert/strict';
import test from 'node:test';
import {
  compareInventoryLightCones,
  compareLightConeRelease,
} from '../src/characters/releaseOrder.ts';

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

test('orders owned light cones by rarity, saved level, then debut date', () => {
  const items = [
    { id: 1, data: undefined, cone: { level: 80 } },
    owned(2, 'cone.23065', 5, 1),
    owned(8, 'cone.22009', 4, 80),
    owned(3, 'cone.23062', 5, 60),
    owned(4, 'cone.23065', 5, 60),
    owned(5, 'cone.23055', 5, 80),
    owned(6, 'cone.23065', 5, 60),
    owned(7, 'import.other', 5, 60),
    owned(9, 'cone.23061', 5, 60),
  ];
  items.sort(compareInventoryLightCones);
  assert.deepEqual(
    items.map(({ id }) => id),
    [5, 4, 6, 9, 3, 7, 2, 8, 1],
  );
});

function owned(id, data_id, rarity, level) {
  return { id, data: { default: { data_id }, rarity }, cone: { level } };
}
