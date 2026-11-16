import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import process from 'node:process';
import { check, readLock, sha256 } from './sources.mjs';

async function main() {
  const [directory = '.m03-data', ...extra] = process.argv.slice(2);
  check(
    extra.length === 0,
    'Usage: node scripts/m03/download.mjs [source-directory]',
  );
  const lock = await readLock();
  for (const source of lock.sources) {
    for (const [path, digest] of Object.entries(source.files)) {
      const destination = join(
        directory,
        source.directory,
        source.flat ? basename(path) : path,
      );
      const cached = await readFile(destination).catch((error) => {
        if (error.code === 'ENOENT') return null;
        throw error;
      });
      if (cached && sha256(cached) === digest) continue;
      const url = `https://raw.githubusercontent.com/${source.repository}/${source.revision}/${path}`;
      const response = await globalThis.fetch(url, {
        signal: globalThis.AbortSignal.timeout(60_000),
      });
      check(response.ok, `${path}: download HTTP ${response.status}`);
      const bytes = Buffer.from(await response.arrayBuffer());
      check(sha256(bytes) === digest, `${path}: SHA-256 mismatch`);
      await mkdir(dirname(destination), { recursive: true });
      await writeFile(destination, bytes);
      console.log(`${source.directory}/${path}`);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
