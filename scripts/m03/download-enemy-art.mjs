import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { Buffer } from 'node:buffer';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { URL } from 'node:url';
import { check, gameVersion, sha256 } from './sources.mjs';

const repo = 'umaichanuwu/StarRailTextures';
const revision = '2a4b9a7eb7ac9db7f48d627fa5cdfd3822c902ce';
const dataDir = join('.m03-data', gameVersion);
const sources = {
  avatar: {
    sprite: 'mostericon',
    local: 'enemy-avatars',
    extension: 'png',
    tree: '42f6aaa4a01b1e5e8bdbda7cc28b022aa50dc093',
  },
  artwork: {
    sprite: 'monsterfigure',
    local: 'enemy-art',
    extension: 'webp',
    tree: '6ab87aa3c092aa75af3cd15e083bba4451a455bd',
  },
};

async function fetchFile(url) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      const res = await globalThis.fetch(url, {
        signal: globalThis.AbortSignal.timeout(45_000),
      });
      check(res.ok, `HTTP ${res.status} ${url}`);
      return Buffer.from(await res.arrayBuffer());
    } catch (error) {
      if (attempt === 3) throw error;
    }
  }
}

function artworkWebp(png, file) {
  const result = spawnSync(
    'ffmpeg',
    [
      '-v',
      'error',
      '-xerror',
      '-i',
      'pipe:0',
      '-vf',
      'scale=768:768:force_original_aspect_ratio=decrease',
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
    { input: png, windowsHide: true, maxBuffer: 4 * 1024 * 1024 },
  );
  if (result.error) throw result.error;
  check(
    result.status === 0 && result.stdout.toString('ascii', 8, 12) === 'WEBP',
    `无法转换 ${file}：${result.stderr}`,
  );
  return result.stdout;
}

async function prepare(kind, ids, prior) {
  const { sprite, local, extension, tree } = sources[kind];
  const dir = join(dataDir, local);
  await mkdir(dir, { recursive: true });
  const listing = JSON.parse(
    await fetchFile(`https://api.github.com/repos/${repo}/git/trees/${tree}`),
  );
  check(listing.sha === tree && !listing.truncated, `资源目录不完整 ${sprite}`);
  const files = new Map(
    listing.tree
      .filter((entry) => entry.type === 'blob')
      .map((entry) => [entry.path, entry.sha]),
  );
  const available = [...ids].sort().filter((id) => files.has(`${id}.png`));
  const assets = new Map();
  let next = 0;

  async function work() {
    while (next < available.length) {
      const id = available[next++];
      const file = `${id}.png`;
      const blob = files.get(file);
      const destination = join(dir, `${id}.${extension}`);
      const cached = await readFile(destination).catch((error) => {
        if (error.code === 'ENOENT') return null;
        throw error;
      });
      if (
        cached &&
        prior?.[id]?.blob === blob &&
        prior[id].sha256 === sha256(cached)
      ) {
        assets.set(id, prior[id]);
        continue;
      }
      const png = await fetchFile(
        `https://raw.githubusercontent.com/${repo}/${revision}/assets/asbres/spriteoutput/${sprite}/${file}`,
      );
      const digest = createHash('sha1')
        .update(`blob ${png.length}\0`)
        .update(png)
        .digest('hex');
      check(
        digest === blob && png.readUInt32BE(0) === 0x89504e47,
        `素材校验失败 ${file}`,
      );
      const bytes = kind === 'avatar' ? png : artworkWebp(png, file);
      await writeFile(destination, bytes);
      assets.set(id, { blob, sha256: sha256(bytes) });
    }
  }

  await Promise.all(Array.from({ length: 8 }, work));
  return Object.fromEntries(available.map((id) => [id, assets.get(id)]));
}

async function main() {
  const catalog = JSON.parse(
    await readFile(
      new URL('../../src/team/enemy-preview.json', import.meta.url),
    ),
  );
  const manifest = join(dataDir, 'enemy-art-manifest.json');
  const previous = await readFile(manifest, 'utf8')
    .then(JSON.parse)
    .catch((error) => {
      if (error.code === 'ENOENT') return null;
      throw error;
    });
  const entries = await Promise.all(
    Object.keys(sources).map(async (kind) => {
      const ids = new Set(
        catalog.enemies.map((enemy) => enemy[kind]).filter(Boolean),
      );
      const assets = await prepare(
        kind,
        ids,
        previous?.revision === revision ? previous[kind] : null,
      );
      const covered = catalog.enemies.filter((enemy) =>
        Object.hasOwn(assets, enemy[kind]),
      ).length;
      console.log(
        `${kind} ${Object.keys(assets).length} 张，覆盖 ${covered}/${catalog.enemies.length} 个配置`,
      );
      return [kind, assets];
    }),
  );
  await writeFile(
    manifest,
    `${JSON.stringify({ repo, revision, ...Object.fromEntries(entries) }, null, 2)}\n`,
  );
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
