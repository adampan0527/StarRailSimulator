import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { propertyId } from './property-ids.mjs';
import { check, gameVersion, lookup, readLock, sha256 } from './sources.mjs';

const dataDir = join('.m03-data', gameVersion);
const imageDir = join(dataDir, 'relic-icons');
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

async function prepareIcon(url, destination) {
  const cached = await readFile(destination).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (cached?.subarray(0, 8).equals(pngSignature)) return;

  const response = await globalThis.fetch(url, {
    signal: globalThis.AbortSignal.timeout(30_000),
  });
  check(response.ok, `${url}: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  check(bytes.subarray(0, 8).equals(pngSignature), `${url}: 应为 PNG 图片`);
  await writeFile(destination, bytes);
}

async function readIndex(source, name) {
  const path = `index_new/cn/${name}.json`;
  const bytes = await readFile(join('.m03-data', source.directory, path));
  check(sha256(bytes) === source.files[path], `${path} 与锁定的修订不符`);
  return JSON.parse(bytes);
}

async function main() {
  const lock = await readLock();
  const source = lock.sources.find(
    ({ directory }) => directory === 'starrailres',
  );
  check(source, '缺少 StarRailRes 数据源');
  const [relics, sets, properties, catalogBytes] = await Promise.all([
    readIndex(source, 'relics'),
    readIndex(source, 'relic_sets'),
    readIndex(source, 'properties'),
    readFile(join(dataDir, 'catalog.json')),
  ]);
  const { content } = JSON.parse(catalogBytes);
  const statIcons = Object.fromEntries(
    Object.entries(properties).map(([id, row]) => [propertyId(id), row.icon]),
  );
  const icons = new Map();
  for (const [id, relic] of Object.entries(content.relics)) {
    if (relic.rarity !== 5) continue;
    const row = lookup(relics, id.slice('relic.'.length), '五星遗器');
    const set = lookup(sets, relic.set_id.slice('set.'.length), '遗器套装');
    check(row.rarity === 5, `${id}: 应为五星遗器`);
    icons.set(id, row.icon);
    icons.set(relic.set_id, set.icon);
    for (const stat of [
      ...Object.keys(content.main_affixes[relic.main_group]),
      ...Object.keys(content.sub_affixes[relic.sub_group]),
    ]) {
      icons.set(`stat.${stat}`, lookup(statIcons, stat, '遗器词条'));
    }
  }
  const baseUrl = `https://raw.githubusercontent.com/${source.repository}/${source.revision}`;
  await mkdir(imageDir, { recursive: true });

  const pending = icons.entries();
  async function worker() {
    for (const [id, path] of pending) {
      check(
        /^icon\/(?:relic\/\d+(?:_\d+)?|property\/[A-Za-z0-9_]+)\.png$/.test(
          path,
        ),
        `${id} 缺少图标路径`,
      );
      await prepareIcon(`${baseUrl}/${path}`, join(imageDir, `${id}.png`));
    }
  }

  await Promise.all(Array.from({ length: 8 }, worker));
  console.log(`已准备 ${icons.size} 个五星遗器、套装与词条图标文件`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
