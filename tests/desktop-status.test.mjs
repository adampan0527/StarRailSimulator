import assert from 'node:assert/strict';
import test from 'node:test';
import { readDesktopStatus } from '../src/desktop/status.ts';

test('asks the Rust backend for the package version', async () => {
  const result = await readDesktopStatus(async (command) => {
    assert.equal(command, 'desktop_status');
    return { version: '0.1.0' };
  });

  assert.deepEqual(result, { version: '0.1.0' });
});

test('reports a bad response separately from a failed call', async () => {
  await assert.rejects(
    readDesktopStatus(async () => ({ version: 1 })),
    {
      message: 'INVALID_DESKTOP_STATUS',
    },
  );
  await assert.rejects(
    readDesktopStatus(async () => {
      throw new Error('offline');
    }),
    { message: 'DESKTOP_CALL_FAILED' },
  );
});
