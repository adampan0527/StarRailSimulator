import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { URL } from 'node:url';
import { selectableEnemies } from '../scripts/m03/selectable-enemies.mjs';

const preview = JSON.parse(
  readFileSync(
    new URL('../src/team/enemy-preview.json', import.meta.url),
    'utf8',
  ),
);
const byId = new Map(preview.enemies.map((enemy) => [enemy.id, enemy]));

test('lists each independently selectable 4.7.0 enemy once with every level preview', () => {
  assert.equal(preview.revision, 'dad9155eb00fdeda31fb8456507c53590d49071f');
  assert.equal(preview.enemies.length, 383);
  assert.equal(byId.size, preview.enemies.length);
  assert.equal(
    new Set(
      preview.enemies.map(({ name }) =>
        name.normalize('NFKC').replace(/\s+/g, ''),
      ),
    ).size,
    383,
  );
  assert.deepEqual(
    preview.levels,
    Array.from({ length: 100 }, (_, index) => index + 1),
  );
  const ids = preview.enemies.flatMap((enemy) => enemy.sourceIds);
  assert.equal(new Set(ids).size, ids.length);
  for (const enemy of preview.enemies) {
    assert.ok(enemy.name && /^\d+$/.test(enemy.id));
    assert.ok(enemy.sourceIds.includes(enemy.id));
    for (const stat of [enemy.hp, enemy.defense, enemy.speed]) {
      if (stat === null) continue;
      const series = preview.statSeries[stat];
      assert.equal(series.length, preview.levels.length);
      assert.ok(
        series.every(
          (value) => value === null || (Number.isFinite(value) && value > 0),
        ),
      );
    }
  }
});

test('uses canonical stats and collapses selectable difficulty variants', () => {
  const lieutenant = byId.get('1003010');
  const at100 = (series) =>
    preview.statSeries[series][preview.levels.indexOf(100)];
  assert.ok(lieutenant.sourceIds.includes('100301004'));
  assert.ok(!byId.has('100301004'));
  assert.equal(lieutenant.avatar, 'Monster_1003010');
  assert.equal(lieutenant.artwork, 'Monster_1003010');
  assert.equal(at100(lieutenant.hp), 338371.3674);
  assert.equal(at100(lieutenant.speed), 158.4);
  assert.equal(byId.get('3024010').name, '银枝');
  for (const name of [
    '「盾」',
    '「授勋」',
    '「枪尖」',
    '辅助机械臂单元',
    '反物质引擎',
    '1号输出端',
  ]) {
    assert.ok(
      preview.enemies.every((enemy) => enemy.name !== name),
      name,
    );
  }
  assert.equal(byId.get('8033020').sourceIds.length, 4);
  assert.ok(byId.get('1012020'));
  assert.equal(byId.get('5032010').name, '狂兽的胚芽');
});

test('enemies with a separate encounter remain selectable even if another unit can summon them', () => {
  const input = [
    { id: 100, name: '首领', summons: [200, 300] },
    { id: 101, name: '首领', summons: [] },
    { id: 200, name: '随从', summons: [] },
    { id: 300, name: '小怪', summons: [] },
    { id: 400, name: '图鉴单位', summons: [] },
  ];
  const tables = {
    StageConfig: [
      {
        MonsterList: [
          { Monster0: 101, Monster1: 200, Monster2: 300 },
          { Monster0: 300 },
        ],
      },
    ],
    StageConfigLD: [],
    StageInfiniteMonsterGroup: [],
  };
  const templates = new Map(
    input.map(({ id }) => [
      id,
      { MonsterTemplateID: id, ...(id === 400 ? { AtlasSortID: 1 } : {}) },
    ]),
  );
  const monsters = new Map(
    input.map(({ id, summons }) => [
      id,
      {
        MonsterID: id,
        MonsterTemplateID: id,
        MonsterName: { Hash: id },
        SummonIDList: summons,
        CustomValueTags: [],
      },
    ]),
  );
  const names = Object.fromEntries(input.map(({ id, name }) => [id, name]));
  const lineup = selectableEnemies(tables, monsters, templates, names);
  assert.deepEqual(
    lineup.map(({ name }) => name),
    ['首领', '小怪', '图鉴单位'],
  );
  assert.deepEqual(lineup[0].sourceIds, ['100', '101']);
});
