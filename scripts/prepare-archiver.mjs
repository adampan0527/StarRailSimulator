import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import process from 'node:process';
import { URL } from 'node:url';

const filename = 'reliquary-archiver-pktmon-x64.exe';
const release = 'v0.20.0';
const sha256 =
  'ce4a5bb00ba0c3d681772d9151ac2332934dd1b497926c3d7994f40c2a94cd9d';
const directory = new URL('../.archiver/', import.meta.url);
const destination = new URL(filename, directory);
const temporary = new URL(`${filename}.${process.pid}.tmp`, directory);
const download = `https://github.com/IceDynamix/reliquary-archiver/releases/download/${release}/${filename}`;

function matchesRelease(bytes) {
  return createHash('sha256').update(bytes).digest('hex') === sha256;
}

const cached = await readFile(destination).catch((error) => {
  if (error.code === 'ENOENT') return null;
  throw error;
});
if (!cached || !matchesRelease(cached)) {
  const response = await globalThis.fetch(download, {
    signal: globalThis.AbortSignal.timeout(120_000),
  });
  if (!response.ok) {
    throw new Error(
      `Archiver ${release} download failed: HTTP ${response.status}`,
    );
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  if (!matchesRelease(bytes)) {
    throw new Error(`Archiver ${release} SHA-256 does not match the release.`);
  }
  await mkdir(directory, { recursive: true });
  try {
    await writeFile(temporary, bytes);
    await rename(temporary, destination);
  } finally {
    await rm(temporary, { force: true });
  }
}
