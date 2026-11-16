import console from 'node:console';
import { mkdir, rename, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { buildCharacters } from './characters.mjs';
import { buildEquipment } from './equipment.mjs';
import { propertyNames } from './patches.mjs';
import {
  check,
  gameVersion,
  lookup,
  numberValue,
  readLock,
  readSources,
  sha256,
  textValue,
} from './sources.mjs';

const statIds = {
  HPDelta: 'hp_flat',
  AttackDelta: 'attack_flat',
  DefenceDelta: 'defense_flat',
  Speed: 'speed_total',
  SpeedDelta: 'speed',
  HPAddedRatio: 'hp_ratio',
  AttackAddedRatio: 'attack_ratio',
  DefenceAddedRatio: 'defense_ratio',
  SpeedAddedRatio: 'speed_ratio',
  CriticalChanceBase: 'crit_rate',
  CriticalDamageBase: 'crit_damage',
};

const identity = (text) =>
  text.toLowerCase().replaceAll(' ', '_').replace(/^the_/, '');
const [
  compactDirectory = '.m03-data/starrailres',
  nativeDirectory = '.m03-data/turnbased',
  output = '.m03-data/4.7.0',
  ...extra
] = process.argv.slice(2);

async function main() {
  check(
    extra.length === 0,
    'Usage: node scripts/m03/import.mjs [StarRailRes] [TurnBasedGameData] [output]',
  );
  const lock = await readLock();
  const [compact, native] = await readSources(lock, [
    compactDirectory,
    nativeDirectory,
  ]);
  check(
    compact.info.version === gameVersion,
    `expected game version ${gameVersion}`,
  );
  const paths = Object.fromEntries(
    Object.values(compact.paths).map((row) => [row.id, identity(row.text)]),
  );
  const toPath = (id) => lookup(paths, id, 'path');
  const toElement = (id) =>
    identity(lookup(compact.elements, id, 'element').id);
  const propertyId = (key) => {
    lookup(compact.properties, key, 'property');
    return statIds[key] ?? key.toLowerCase();
  };
  const propertyBonuses = (rows, label) =>
    rows.map((row) => ({
      property: propertyId(row.type),
      value: numberValue(row.value, label, -Infinity),
    }));
  const properties = Object.fromEntries(
    Object.entries(compact.properties).map(([id, row]) => [
      propertyId(id),
      textValue(propertyNames[id] ?? row.name, `property ${id}`),
    ]),
  );
  check(
    Object.keys(properties).length === Object.keys(compact.properties).length,
    'duplicate normalized property IDs',
  );
  const { characters, roster } = await buildCharacters(
    compact,
    native,
    toPath,
    toElement,
    propertyBonuses,
  );
  const equipment = buildEquipment(
    compact,
    native,
    toPath,
    propertyId,
    propertyBonuses,
  );
  const content = {
    schema_version: 1,
    game_version: gameVersion,
    characters,
    ...equipment,
    properties,
  };
  const dataVersion = `sr.4.7.0.m03.${sha256(JSON.stringify(content))}`;
  const catalog = { data_version: dataVersion, content };
  const audit = {
    game_version: gameVersion,
    data_version: dataVersion,
    sources: lock.sources.map(({ repository, revision }) => ({
      repository,
      revision,
    })),
    character_source_count: roster.reduce(
      (sum, item) => sum + item.source_ids.length,
      0,
    ),
    character_form_count: roster.length,
    variants: roster.reduce(
      (sum, item) => sum + Object.keys(item.variants).length,
      0,
    ),
    characters: roster,
    light_cones: Object.keys(equipment.light_cones).map((id) => ({
      id,
      superimpositions: [1, 2, 3, 4, 5],
    })),
    relic_ids: Object.keys(equipment.relics),
    relic_set_ids: Object.keys(equipment.relic_sets),
  };
  await mkdir(output, { recursive: true });
  await writeOutput('catalog.json', catalog);
  await writeOutput('audit.json', audit);
  console.log(
    `${roster.length} forms / ${audit.character_source_count} character records / ${audit.variants} kits`,
  );
  console.log(
    `${audit.light_cones.length} light cones / ${audit.relic_ids.length} relics / ${audit.relic_set_ids.length} relic sets`,
  );
  console.log(dataVersion);
}

async function writeOutput(file, value) {
  const target = join(output, file);
  const pending = `${target}.${process.pid}.tmp`;
  await writeFile(pending, `${JSON.stringify(value)}\n`, { flag: 'wx' });
  await rename(pending, target);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
