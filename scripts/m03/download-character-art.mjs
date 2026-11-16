import { spawnSync } from 'node:child_process';
import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { check, gameVersion, readLock, sha256 } from './sources.mjs';

const dataDir = join('.m03-data', gameVersion);
const imageDir = join(dataDir, 'character-art');
const indexFile = 'index_new/cn/characters.json';

function isWebp(bytes) {
  return (
    bytes !== null &&
    bytes.length > 12 &&
    bytes.toString('ascii', 0, 4) === 'RIFF' &&
    bytes.toString('ascii', 8, 12) === 'WEBP' &&
    bytes.readUInt32LE(4) + 8 === bytes.length
  );
}

function resizeArtwork(png, path) {
  const result = spawnSync(
    'ffmpeg',
    [
      '-v',
      'error',
      '-xerror',
      '-i',
      'pipe:0',
      '-vf',
      'scale=960:960:force_original_aspect_ratio=decrease',
      '-frames:v',
      '1',
      '-c:v',
      'libwebp',
      '-pix_fmt',
      'yuva420p',
      '-quality',
      '86',
      '-f',
      'webp',
      'pipe:1',
    ],
    { input: png, windowsHide: true, maxBuffer: 8 * 1024 * 1024 },
  );
  check(!result.error, `${path}: 请确认 FFmpeg 已加入 PATH。 ${result.error}`);
  check(
    result.status === 0 && isWebp(result.stdout),
    `${path}: 无法转换立绘。 ${result.stderr}`,
  );
  return result.stdout;
}

async function prepareArtwork(source, path, destination) {
  const cached = await readFile(destination).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (isWebp(cached)) return cached;

  const artwork = resizeArtwork(await downloadArtwork(source, path), path);
  await writeFile(destination, artwork);
  return artwork;
}

async function downloadArtwork(source, path) {
  const url = `https://raw.githubusercontent.com/${source.repository}/${source.revision}/${path}`;
  for (let attempt = 1; ; attempt += 1) {
    try {
      const response = await globalThis.fetch(url, {
        signal: globalThis.AbortSignal.timeout(60_000),
      });
      check(response.ok, `HTTP ${response.status}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      if (attempt === 3) throw new Error(`${path}: 下载失败`, { cause: error });
    }
  }
}

async function main() {
  const lock = await readLock();
  const source = lock.sources.find(
    ({ directory }) => directory === 'starrailres',
  );
  check(source, '缺少 StarRailRes 数据源');
  const indexBytes = await readFile(
    join('.m03-data', source.directory, indexFile),
  );
  check(
    sha256(indexBytes) === source.files[indexFile],
    '角色索引与锁定的修订不符',
  );
  const index = JSON.parse(indexBytes);
  const catalog = JSON.parse(await readFile(join(dataDir, 'catalog.json')));
  const roles = Object.values(catalog.content.characters);
  const ids = [
    ...new Set(roles.flatMap(({ source_ids }) => source_ids)),
  ].sort();
  await mkdir(imageDir, { recursive: true });

  const assets = new Map();
  let next = 0;
  async function worker() {
    for (;;) {
      const id = ids[next++];
      if (id === undefined) return;
      check(/^\d+$/.test(id), `无效的角色编号 ${id}`);
      const path = `image/character_portrait/${id}.png`;
      check(index[id]?.portrait === path, `角色 ${id} 的立绘路径与索引不符`);
      const file = `${id}.webp`;
      const bytes = await prepareArtwork(source, path, join(imageDir, file));
      assets.set(id, { file, sha256: sha256(bytes) });
    }
  }
  await Promise.all(Array.from({ length: 6 }, worker));
  const manifest = {
    data_version: catalog.data_version,
    source: source.repository,
    revision: source.revision,
    assets: Object.fromEntries(ids.map((id) => [id, assets.get(id)])),
  };
  await writeFile(
    join(imageDir, 'manifest.json'),
    JSON.stringify(manifest, null, 2) + '\n',
  );
  console.log(`已核对 ${roles.length} 个角色形态的 ${ids.length} 张立绘`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
