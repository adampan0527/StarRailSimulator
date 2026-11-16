import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { URL } from 'node:url';

export const gameVersion = '4.7.0';

export const sha256 = (bytes) =>
  createHash('sha256').update(bytes).digest('hex');

export function check(condition, message) {
  if (!condition) throw new Error(message);
}

export function index(rows, key, table) {
  const result = new Map();
  for (const row of rows) {
    const id = String(row[key]);
    check(
      id !== 'undefined' && !result.has(id),
      `${table}: missing or duplicate ${key} ${id}`,
    );
    result.set(id, row);
  }
  return result;
}

export function lookup(table, id, name) {
  const row =
    table instanceof Map ? (table.get(id) ?? table.get(String(id))) : table[id];
  check(row !== undefined, `${name}: missing ${id}`);
  return row;
}

export function sameIds(actual, expected, name) {
  const unknown = [...actual].filter((id) => !expected.has(id));
  const missing = [...expected].filter((id) => !actual.has(id));
  check(
    !missing.length && !unknown.length,
    `${name}: missing [${missing}], unexpected [${unknown}]`,
  );
}

export function textValue(text, label) {
  check(
    typeof text === 'string' && text.trim() !== '',
    `${label}: missing text`,
  );
  return text;
}

export function numberValue(value, label, minimum = 0) {
  check(
    typeof value === 'number' && Number.isFinite(value) && value >= minimum,
    `${label}: invalid number ${value}`,
  );
  return value;
}

export function checkParameters(description, levels, label) {
  const slots = [...description.matchAll(/#(\d+)\[/g)].map((match) =>
    Number(match[1]),
  );
  const needed = Math.max(0, ...slots);
  check(
    !needed ||
      (levels.length > 0 && levels.every((params) => params.length >= needed)),
    `${label}: description references a missing parameter`,
  );
}

export function promotionIndex(rows, owner, name) {
  return index(
    rows.map((row) => ({ ...row, key: `${row[owner]}.${row.Promotion ?? 0}` })),
    'key',
    name,
  );
}

export function checkedPromotions(compact, native, id, statFields) {
  const caps = [20, 30, 40, 50, 60, 70, 80];
  check(
    compact.length === caps.length,
    `promotion ${id}: expected seven tiers`,
  );
  return caps.map((cap, i) => {
    const label = `promotion ${id} tier ${i}`;
    const row = lookup(native, `${id}.${i}`, label);
    check(row.MaxLevel === cap, `${label}: invalid level cap`);
    return Object.fromEntries(
      Object.entries(statFields).map(([stat, [baseField, stepField]]) => {
        const base = numberValue(
          row[baseField]?.Value ?? 0,
          `${label} ${stat} base`,
        );
        const step = numberValue(
          row[stepField]?.Value ?? 0,
          `${label} ${stat} growth`,
        );
        const expected = lookup(compact[i], stat, label);
        check(
          Math.abs(base - expected.base) <= 0.0001 &&
            Math.abs(step - expected.step) <= 0.0001,
          `${label} ${stat}: source values differ (${base}, ${step} vs ${expected.base}, ${expected.step})`,
        );
        return [stat, { base, step }];
      }),
    );
  });
}

export async function readLock() {
  return JSON.parse(
    await readFile(new URL('./source-lock.json', import.meta.url), 'utf8'),
  );
}

export async function readSources(lock, directories) {
  return Promise.all(
    lock.sources.map(async (source, position) => {
      const directory = directories[position];
      const tables = {};
      for (const [path, digest] of Object.entries(source.files)) {
        const file = source.flat ? basename(path) : path;
        const bytes = await readFile(join(directory, file));
        check(
          sha256(bytes) === digest,
          `${source.repository}/${path}: SHA-256 does not match ${source.revision}`,
        );
        if (path.endsWith('.json')) {
          // TextMap references are unsigned 64-bit integers. Retain their source digits.
          tables[basename(path, '.json')] = JSON.parse(
            bytes.toString('utf8'),
            (key, value, context) =>
              key === 'Hash' && typeof value === 'number'
                ? context.source
                : value,
          );
        }
      }
      return tables;
    }),
  );
}
