import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';
import { format, resolveConfig } from 'prettier';
import { check, index, lookup, sha256 } from './sources.mjs';

const REPOSITORY = 'DimbreathBot/TurnBasedGameData';
const REVISION = 'dad9155eb00fdeda31fb8456507c53590d49071f';
const LOCK_PATH = fileURLToPath(
  new URL('./enemy-ai-lock.json', import.meta.url),
);
const PREVIEW_PATH = fileURLToPath(
  new URL('../../src/team/enemy-preview.json', import.meta.url),
);
const CONCURRENCY = 12;

export function localFilePath(baseDir, repoPath) {
  if (repoPath.startsWith('ExcelOutput/')) {
    return join(baseDir, basename(repoPath));
  }
  return join(baseDir, ...repoPath.split('/'));
}

export async function readEnemyAiLock() {
  return JSON.parse(await readFile(LOCK_PATH, 'utf8'));
}

async function collectRequiredPaths(turnbasedDir) {
  const preview = JSON.parse(await readFile(PREVIEW_PATH, 'utf8'));
  check(
    preview.revision === REVISION,
    `enemy-preview revision mismatch: ${preview.revision}`,
  );
  const monsters = index(
    JSON.parse(
      await readFile(join(turnbasedDir, 'MonsterConfig.json'), 'utf8'),
    ),
    'MonsterID',
    'MonsterConfig',
  );
  const templates = index(
    JSON.parse(
      await readFile(join(turnbasedDir, 'MonsterTemplateConfig.json'), 'utf8'),
    ),
    'MonsterTemplateID',
    'MonsterTemplateConfig',
  );
  const paths = new Set([
    'ExcelOutput/MonsterSkillConfig.json',
    'ExcelOutput/MonsterStatusConfig.json',
  ]);
  for (const enemy of preview.enemies) {
    const monster = lookup(monsters, enemy.id, 'MonsterConfig');
    const template = lookup(
      templates,
      monster.MonsterTemplateID,
      'MonsterTemplateConfig',
    );
    const aiPath = monster.OverrideAIPath || template.AIPath;
    check(aiPath, `missing AIPath for enemy ${enemy.id}`);
    check(template.JsonConfig, `missing JsonConfig for enemy ${enemy.id}`);
    paths.add(aiPath);
    paths.add(template.JsonConfig);
  }
  return [...paths].sort();
}

async function fetchRepoFile(repoPath) {
  const url = `https://raw.githubusercontent.com/${REPOSITORY}/${REVISION}/${repoPath}`;
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      const response = await globalThis.fetch(url, {
        signal: globalThis.AbortSignal.timeout(60_000),
      });
      check(response.ok, `${repoPath}: HTTP ${response.status}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      lastError = error;
      if (attempt === 3) break;
    }
  }
  throw lastError;
}

async function runPool(items, limit, worker) {
  let cursor = 0;
  async function next() {
    while (cursor < items.length) {
      const current = items[cursor];
      cursor += 1;
      await worker(current);
    }
  }
  await Promise.all(
    Array.from({ length: Math.min(limit, items.length) }, () => next()),
  );
}

async function downloadAndLock(turnbasedDir, generateLock) {
  if (generateLock) {
    const paths = await collectRequiredPaths(turnbasedDir);
    const digests = {};
    let downloaded = 0;
    await runPool(paths, CONCURRENCY, async (repoPath) => {
      const destination = localFilePath(turnbasedDir, repoPath);
      const cached = await readFile(destination).catch((error) => {
        if (error.code === 'ENOENT') return null;
        throw error;
      });
      const bytes = cached ?? (await fetchRepoFile(repoPath));
      if (!cached) {
        await mkdir(dirname(destination), { recursive: true });
        await writeFile(destination, bytes);
        downloaded += 1;
      }
      digests[repoPath] = sha256(bytes);
    });
    const sortedFiles = Object.fromEntries(
      Object.entries(digests).sort(([a], [b]) => a.localeCompare(b)),
    );
    const lock = {
      repository: REPOSITORY,
      revision: REVISION,
      files: sortedFiles,
    };
    const config = await resolveConfig(LOCK_PATH);
    await writeFile(
      LOCK_PATH,
      await format(JSON.stringify(lock, null, 2), {
        ...config,
        filepath: LOCK_PATH,
      }),
    );
    console.log(
      `Locked ${paths.length} enemy AI/skill files (downloaded ${downloaded}).`,
    );
    return;
  }

  const lock = await readEnemyAiLock();
  check(lock.repository === REPOSITORY, `unexpected repo ${lock.repository}`);
  check(lock.revision === REVISION, `unexpected revision ${lock.revision}`);
  const entries = Object.entries(lock.files);
  let downloaded = 0;
  await runPool(entries, CONCURRENCY, async ([repoPath, expectedDigest]) => {
    const destination = localFilePath(turnbasedDir, repoPath);
    const cached = await readFile(destination).catch((error) => {
      if (error.code === 'ENOENT') return null;
      throw error;
    });
    if (cached && sha256(cached) === expectedDigest) return;
    const bytes = await fetchRepoFile(repoPath);
    check(
      sha256(bytes) === expectedDigest,
      `${repoPath}: SHA-256 mismatch against enemy-ai-lock.json`,
    );
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, bytes);
    downloaded += 1;
  });
  console.log(
    `Verified ${entries.length} enemy AI/skill files (downloaded ${downloaded}).`,
  );
}

async function main() {
  const args = process.argv.slice(2);
  const generateLock = args.includes('--generate-lock');
  const positional = args.filter((arg) => arg !== '--generate-lock');
  const [turnbasedDir = '.m03-data/turnbased', ...extra] = positional;
  check(
    extra.length === 0,
    'Usage: node scripts/m03/download-enemy-ai.mjs [turnbased-dir] [--generate-lock]',
  );
  await downloadAndLock(turnbasedDir, generateLock);
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
