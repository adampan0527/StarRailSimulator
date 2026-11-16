import assert from 'node:assert/strict';
import test from 'node:test';
import { compareLightConeRelease } from '../src/characters/releaseOrder.ts';

test('orders light cones by first availability and English name within the same batch', () => {
  const ids = [
    'cone.23001', // In the Night, 1.0 phase I
    'cone.23062', // I Am As You Behold, 4.4 collaboration
    'cone.23010', // Before Dawn, 1.0 phase II
    'cone.23065', // Upon the First Echo of "Aha", 4.7
    'cone.23060', // A Star That Lights the Night, 4.4 phase I
    'cone.20003', // Amber, 1.0 phase I
    'cone.23061', // Flickering Stars, 4.4 collaboration
    'cone.22009', // See You at the World's End!, 4.7
    'cone.23055', // Colors for Tomorrow, 4.6
    'cone.22003', // Ninja Record: Sound Hunt, event starts during 2.6 phase I
    'cone.21048', // Dream's Montage, 2.6 phase I
  ];
  const sorted = ids
    .map((data_id) => ({ data_id }))
    .sort(compareLightConeRelease);
  assert.deepEqual(
    sorted.map(({ data_id }) => data_id),
    [
      'cone.22009',
      'cone.23065',
      'cone.23055',
      'cone.23061',
      'cone.23062',
      'cone.23060',
      'cone.22003',
      'cone.21048',
      'cone.23010',
      'cone.20003',
      'cone.23001',
    ],
  );
});

test('keeps duplicate inventory instances and imports without a date stable', () => {
  const items = [
    { id: 1, data_id: 'import.unknown' },
    { id: 3, data_id: 'cone.23065' },
    { id: 8, data_id: 'cone.22009' },
    { id: 4, data_id: 'cone.23065' },
    { id: 7, data_id: 'import.other' },
  ];
  items.sort(compareLightConeRelease);
  assert.deepEqual(
    items.map(({ id }) => id),
    [8, 3, 4, 1, 7],
  );
});
