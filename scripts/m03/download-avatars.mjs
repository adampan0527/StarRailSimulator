import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { check, gameVersion, readLock, sha256 } from './sources.mjs';

const dataDir = join('.m03-data', gameVersion);
const imageDir = join(dataDir, 'avatars');
const indexFile = 'index_new/cn/characters.json';
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

function isAvatar(bytes) {
  return (
    bytes.length >= 24 &&
    bytes.subarray(0, 8).equals(pngSignature) &&
    bytes.readUInt32BE(16) === 256 &&
    bytes.readUInt32BE(20) === 256
  );
}

async function cachedAvatar(path) {
  try {
    const bytes = await readFile(path);
    return isAvatar(bytes) ? bytes : null;
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
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
  const catalog = JSON.parse(
    await readFile(join(dataDir, 'catalog.json'), 'utf8'),
  );
  const roles = Object.entries(catalog.content.characters);
  const ids = [...new Set(roles.flatMap(([, role]) => role.source_ids))].sort();
  await mkdir(imageDir, { recursive: true });

  const assets = {};
  let next = 0;
  async function worker() {
    for (;;) {
      const id = ids[next++];
      if (id === undefined) return;
      check(/^\d+$/.test(id), `无效的角色编号 ${id}`);
      const path = `icon/character/${id}.png`;
      check(index[id]?.icon === path, `角色 ${id} 的头像路径与索引不符`);
      const file = `${id}.png`;
      const destination = join(imageDir, file);
      let bytes = await cachedAvatar(destination);
      if (!bytes) {
        const url = `https://raw.githubusercontent.com/${source.repository}/${source.revision}/${path}`;
        const response = await globalThis.fetch(url, {
          signal: globalThis.AbortSignal.timeout(30_000),
        });
        check(response.ok, `${path}: HTTP ${response.status}`);
        bytes = Buffer.from(await response.arrayBuffer());
        check(isAvatar(bytes), `${path}: 不是预期的 256×256 PNG`);
        await writeFile(destination, bytes);
      }
      assets[id] = { file, width: 256, height: 256, sha256: sha256(bytes) };
    }
  }
  await Promise.all(Array.from({ length: 8 }, worker));
  const manifest = {
    data_version: catalog.data_version,
    source: source.repository,
    revision: source.revision,
    roles: Object.fromEntries(
      roles.map(([id, { name, source_ids }]) => [id, { name, source_ids }]),
    ),
    assets,
  };
  await writeFile(
    join(imageDir, 'manifest.json'),
    JSON.stringify(manifest, null, 2) + '\n',
  );
  console.log(`已核对 ${roles.length} 个角色形态的 ${ids.length} 张头像`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
