import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { check, gameVersion, readLock, sha256 } from './sources.mjs';

const dataDir = join('.m03-data', gameVersion);
const imageDir = join(dataDir, 'light-cones');
const indexFile = 'index_new/cn/light_cones.json';
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const size = 128;

function isLightConeIcon(bytes) {
  return (
    bytes !== null &&
    bytes.length >= 24 &&
    bytes.subarray(0, 8).equals(pngSignature) &&
    bytes.readUInt32BE(16) === size &&
    bytes.readUInt32BE(20) === size
  );
}

async function prepareIcon(source, path, destination) {
  const cached = await readFile(destination).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (isLightConeIcon(cached)) return cached;

  const url = `https://raw.githubusercontent.com/${source.repository}/${source.revision}/${path}`;
  const response = await globalThis.fetch(url, {
    signal: globalThis.AbortSignal.timeout(30_000),
  });
  check(response.ok, `${path}: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  check(isLightConeIcon(bytes), `${path}: 不是预期的 ${size}×${size} PNG`);
  await writeFile(destination, bytes);
  return bytes;
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
    '光锥索引与锁定的修订不符',
  );
  const index = JSON.parse(indexBytes);
  const catalog = JSON.parse(await readFile(join(dataDir, 'catalog.json')));
  const ids = Object.keys(catalog.content.light_cones).sort();
  await mkdir(imageDir, { recursive: true });

  const assets = new Map();
  let next = 0;
  async function worker() {
    for (;;) {
      const key = ids[next++];
      if (key === undefined) return;
      const id = /^cone\.(\d+)$/.exec(key)?.[1];
      const path = `icon/light_cone/${id}.png`;
      check(id && index[id]?.icon === path, `光锥 ${key} 的图片路径与索引不符`);
      const file = `${key}.png`;
      const bytes = await prepareIcon(source, path, join(imageDir, file));
      assets.set(key, { file, sha256: sha256(bytes) });
    }
  }
  await Promise.all(Array.from({ length: 8 }, worker));
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
  console.log(`已核对 ${ids.length} 张光锥图片`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
