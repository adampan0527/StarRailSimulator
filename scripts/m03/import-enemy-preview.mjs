import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';
import { format, resolveConfig } from 'prettier';
import { check, index, lookup, readLock, sha256 } from './sources.mjs';
import { selectableEnemies } from './selectable-enemies.mjs';

const revision = 'dad9155eb00fdeda31fb8456507c53590d49071f';
const blobs = {
  MonsterConfig: 'b689baa89165368a8af239f179959147ace62827',
  MonsterTemplateConfig: '894de1554a58f444cf3fe3b4c3f6e9c46828f5db',
  HardLevelGroup: 'a5efd001f26d23f5fb46617039d9d5cdbb9a5b91',
  EliteGroup: '3649b66b64eae47f7d49900377c42f09d99f93e7',
  StageConfig: '2e1c9d6ce95988e680d7900a111e703f658c82d1',
  StageConfigLD: '27a5e3e955aa2a17d0e6ba96662ceca59149806d',
  StageInfiniteMonsterGroup: '9afc7e55aece474d942a4ce32de4149bf5c6ab2c',
};
const levels = Array.from({ length: 100 }, (_, index) => index + 1);
const ranks = {
  Minion: '普通',
  MinionLv2: '普通',
  Elite: '精英',
  LittleBoss: '首领',
  BigBoss: '首领',
};
const elements = new Set([
  'physical',
  'fire',
  'ice',
  'thunder',
  'wind',
  'quantum',
  'imaginary',
]);

function previewValue(template, monster, elite, curve, stat, offset = 0) {
  const base = template[`${stat}Base`]?.Value;
  const variant = monster[`${stat}ModifyRatio`]?.Value;
  if (base === undefined || variant === undefined || offset === null)
    return null;
  const result =
    base *
      variant *
      elite[`${stat}Ratio`]?.Value *
      curve[`${stat}Ratio`]?.Value +
    offset;
  check(Number.isFinite(result), `invalid ${monster.MonsterID} ${stat}`);
  return result > 0 ? Number(result.toFixed(6)) : null;
}

function speedOffset(template, monster) {
  const templateOffset = template.SpeedModifyValue?.Value;
  const variantOffset = monster.SpeedModifyValue?.Value;
  // The precedence of the two adjustments on the same unit is unverified.
  if (templateOffset !== undefined && variantOffset !== undefined) return null;
  return variantOffset ?? templateOffset ?? 0;
}

function assetName(path, directory) {
  const prefix = `SpriteOutput/${directory}/`;
  if (!path?.startsWith(prefix)) return null;
  const file = path.slice(prefix.length);
  check(/^[A-Za-z0-9_]+\.png$/.test(file), `invalid enemy image ${path}`);
  return /^Monster_\d+\.png$/.test(file) ? file.slice(0, -4) : null;
}

export function enemyPreview(tables, textMap) {
  const monsters = index(tables.MonsterConfig, 'MonsterID', 'monsters');
  const templates = index(
    tables.MonsterTemplateConfig,
    'MonsterTemplateID',
    'templates',
  );
  const elite = index(tables.EliteGroup, 'EliteGroup', 'elite groups');
  const curves = new Map();
  for (const row of tables.HardLevelGroup) {
    const key = `${row.HardLevelGroup}.${row.Level}`;
    check(!curves.has(key), `duplicate level curve ${key}`);
    curves.set(key, row);
  }
  const statSeries = [];
  const seriesIds = new Map();
  function series(values) {
    if (values.every((value) => value === null)) return null;
    const key = JSON.stringify(values);
    if (!seriesIds.has(key)) {
      seriesIds.set(key, statSeries.length);
      statSeries.push(values);
    }
    return seriesIds.get(key);
  }
  const enemies = selectableEnemies(tables, monsters, templates, textMap).map(
    ({ monster, template, name, variant, sourceIds }) => {
      const id = monster.MonsterID;
      const group = lookup(elite, monster.EliteGroup, 'elite group');
      const levelCurves = levels.map((level) =>
        lookup(curves, `${monster.HardLevelGroup}.${level}`, 'level curve'),
      );
      const weaknesses = monster.StanceWeakList.map((type) =>
        type.toLowerCase(),
      );
      check(
        weaknesses.every((type) => elements.has(type)) &&
          new Set(weaknesses).size === weaknesses.length,
        `invalid weaknesses for ${id}`,
      );
      const offset = speedOffset(template, monster);
      return {
        id: String(id),
        name,
        sourceIds,
        rank: lookup(ranks, template.Rank, 'rank'),
        variant,
        avatar: assetName(template.IconPath, 'MosterIcon'),
        artwork: assetName(template.ImagePath, 'MonsterFigure'),
        weaknesses,
        hp: series(
          levelCurves.map((curve) =>
            previewValue(template, monster, group, curve, 'HP'),
          ),
        ),
        defense: series(
          levelCurves.map((curve) =>
            previewValue(template, monster, group, curve, 'Defence'),
          ),
        ),
        speed: series(
          levelCurves.map((curve) =>
            previewValue(template, monster, group, curve, 'Speed', offset),
          ),
        ),
      };
    },
  );
  return { revision, levels, statSeries, enemies };
}

async function main() {
  const dir = process.argv[2] ?? '.m03-data/turnbased';
  const lock = await readLock();
  const source = lock.sources.find(
    ({ directory }) => directory === 'turnbased',
  );
  check(
    source?.revision === revision,
    'enemy preview and character data versions differ',
  );
  const textMap = await readFile(join(dir, 'TextMapCHS.json'));
  check(
    sha256(textMap) === source.files['TextMap/TextMapCHS.json'],
    'invalid TextMapCHS.json',
  );
  const tables = Object.fromEntries(
    await Promise.all(
      Object.entries(blobs).map(async ([name, sha]) => {
        const file = `${name}.json`;
        const content = await readFile(join(dir, file));
        const hash = createHash('sha1')
          .update(`blob ${content.length}\0`)
          .update(content)
          .digest('hex');
        check(hash === sha, `wrong revision for ${file}`);
        const table = JSON.parse(
          content.toString('utf8'),
          (key, value, context) =>
            key === 'Hash' && typeof value === 'number'
              ? context.source
              : value,
        );
        return [name, table];
      }),
    ),
  );
  const filepath = fileURLToPath(
    new URL('../../src/team/enemy-preview.json', import.meta.url),
  );
  const preview = enemyPreview(tables, JSON.parse(textMap.toString('utf8')));
  const config = await resolveConfig(filepath);
  await writeFile(
    filepath,
    await format(JSON.stringify(preview, null, 2), { ...config, filepath }),
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await main();
}
