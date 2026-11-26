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

test('lists each endgame combat 4.7.0 enemy once with complete level previews', () => {
  assert.equal(preview.revision, 'dad9155eb00fdeda31fb8456507c53590d49071f');
  assert.equal(preview.enemies.length, 159);
  assert.equal(byId.size, preview.enemies.length);
  assert.equal(
    new Set(
      preview.enemies.map(({ name }) =>
        name.normalize('NFKC').replace(/\s+/g, ''),
      ),
    ).size,
    159,
  );
  assert.deepEqual(
    preview.levels,
    Array.from({ length: 100 }, (_, index) => index + 1),
  );
  const ids = preview.enemies.flatMap((enemy) => enemy.sourceIds);
  assert.equal(new Set(ids).size, ids.length);
  const standardAvatars = new Set();
  let apocalypticCount = 0;
  for (const enemy of preview.enemies) {
    assert.ok(enemy.name && /^\d+$/.test(enemy.id));
    assert.ok(enemy.sourceIds.includes(enemy.id));
    assert.ok(enemy.variant === null || enemy.variant === '末日幻影');
    if (enemy.variant === '末日幻影') {
      assert.equal(enemy.rank, '首领');
      apocalypticCount += 1;
    } else if (enemy.avatar) {
      assert.ok(
        !standardAvatars.has(enemy.avatar),
        `duplicate standard avatar ${enemy.avatar} on ${enemy.name}`,
      );
      standardAvatars.add(enemy.avatar);
    }
    for (const stat of [enemy.hp, enemy.defense, enemy.speed]) {
      assert.notEqual(stat, null, `missing stat on ${enemy.name}`);
      const series = preview.statSeries[stat];
      assert.equal(series.length, preview.levels.length);
      assert.ok(series.every((value) => Number.isFinite(value) && value > 0));
    }
  }
  assert.equal(apocalypticCount, 18);
});

test('uses canonical stats and excludes reskins, summons, controllers, and gimmick units', () => {
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
  assert.equal(byId.get('3025010').name, '「齐响诗班」神主日');
  assert.equal(byId.get('3025010').variant, null);
  assert.equal(byId.get('4014010').name, '蛮神，疯王，纷争的化身');
  assert.equal(byId.get('4034010').name, '至黑之剑，盗火行者');
  assert.equal(byId.get('4034010').variant, null);
  assert.equal(byId.get('4034013').name, '末日歧途的盗火者');
  assert.equal(byId.get('4034013').variant, '末日幻影');
  assert.equal(byId.get('4035020').name, '铁墓，反诺斯，灵知的葬仪');
  assert.equal(byId.get('4035020').variant, null);
  for (const name of [
    '「盾」',
    '「授勋」',
    '「枪尖」',
    '辅助机械臂单元',
    '反物质引擎',
    '1号输出端',
    '？？？',
    '「黑斗篷魔王」',
    '「哔•咔•啾」',
    '「灰背鲸」',
    '「机械格斗流」多蒙',
    '「机械格斗流」尊者',
    '「身陷樊笼的往昔」（错误）',
    '丰饶玄鹿（完整）',
    '正阳狮鹫（错误）',
    '末度',
    '飞剑',
    '肃风之杪',
    '繁花之杪',
    '「合金保姆」',
    '「贼灵」巴特鲁斯',
    '颁赐者，千军首，天谴之矛',
    '王下一桶',
    '浮烟',
    '昔在、今在、永在的剧目',
    '奇想林中的蕉研组',
    '蕉研组的助蕉',
    '次元扑满',
    '序列扑满',
    '狂兽的胚芽',
    '千星戍卫',
    '毁灭的容器，卡厄斯兰那',
    '黑塔#错误型号',
  ]) {
    assert.ok(
      preview.enemies.every((enemy) => enemy.name !== name),
      name,
    );
  }
  assert.equal(byId.get('8033020').sourceIds.length, 4);
  assert.ok(byId.get('1012020'));
});

test('keeps endgame combatants and apocalyptic bosses while dropping reskins, summons, controllers, and gimmicks', () => {
  const specs = [
    {
      id: 100,
      templateId: 100,
      name: '首领',
      rank: 'LittleBoss',
      atlas: 1,
      group: 100,
      icon: 100,
      stance: 480,
      weak: ['Fire'],
      summons: [200, 300],
    },
    {
      id: 101,
      templateId: 100,
      name: '首领',
      rank: 'LittleBoss',
      atlas: 1,
      group: 100,
      icon: 100,
      stance: 480,
      weak: ['Fire'],
      summons: [],
    },
    {
      id: 102,
      templateId: 100,
      name: '剧情改名首领',
      templateName: '首领',
      rank: 'LittleBoss',
      atlas: 1,
      group: 100,
      icon: 100,
      stance: 480,
      weak: ['Fire'],
      summons: [],
    },
    {
      id: 103,
      templateId: 103,
      name: '？？？',
      rank: 'LittleBoss',
      icon: 100,
      stance: 480,
      weak: ['Fire'],
      summons: [],
    },
    {
      id: 104,
      templateId: 104,
      name: '末日首领',
      rank: 'LittleBoss',
      group: 100,
      icon: 100,
      stance: 960,
      weak: ['Fire', 'Ice'],
      summons: [],
    },
    {
      id: 105,
      templateId: 105,
      name: '首领（错误）',
      rank: 'LittleBoss',
      atlas: 1,
      group: 100,
      icon: 100,
      stance: 480,
      weak: ['Fire'],
      summons: [],
    },
    {
      id: 106,
      templateId: 106,
      name: '后台载体',
      rank: 'LittleBoss',
      atlas: 4,
      group: 106,
      icon: 106,
      stance: 0,
      weak: [],
      summons: [],
    },
    {
      id: 107,
      templateId: 107,
      name: '王棋独立首领',
      rank: 'BigBoss',
      group: 107,
      icon: 107,
      stance: 690,
      weak: ['Ice'],
      summons: [],
    },
    {
      id: 200,
      templateId: 200,
      name: '随从',
      rank: 'MinionLv2',
      atlas: 5,
      group: 200,
      icon: 200,
      stance: 60,
      weak: ['Wind'],
      summons: [],
    },
    {
      id: 300,
      templateId: 300,
      name: '常规小怪',
      rank: 'MinionLv2',
      atlas: 2,
      group: 300,
      icon: 300,
      stance: 60,
      weak: ['Physical'],
      summons: [],
    },
    {
      id: 301,
      templateId: 301,
      name: '无弱点机制桶',
      rank: 'MinionLv2',
      atlas: 6,
      group: 301,
      icon: 301,
      stance: 360,
      weak: [],
      summons: [],
    },
    {
      id: 400,
      templateId: 400,
      name: '仅主线图鉴怪',
      rank: 'Elite',
      atlas: 3,
      group: 400,
      icon: 400,
      stance: 300,
      weak: ['Quantum'],
      summons: [],
    },
  ];
  const tables = {
    StageConfig: [
      {
        StageID: 30101011,
        StageType: 'Challenge',
        MonsterList: [
          { Monster0: 101, Monster1: 200, Monster2: 300 },
          {
            Monster0: 300,
            Monster1: 102,
            Monster2: 103,
            Monster3: 106,
            Monster4: 301,
          },
        ],
      },
      {
        StageID: 420101,
        StageType: 'Challenge',
        MonsterList: [{ Monster0: 104 }],
      },
      {
        StageID: 305090211,
        StageType: 'Challenge',
        MonsterList: [{ Monster0: 107 }],
      },
      {
        StageID: 103201,
        StageType: 'Mainline',
        MonsterList: [{ Monster0: 400, Monster1: 200 }],
      },
      {
        StageID: 307111,
        StageType: 'VerseSimulation',
        MonsterList: [{ Monster0: 105 }],
      },
    ],
    StageConfigLD: [],
    StageInfiniteMonsterGroup: [],
  };
  const names = {};
  const templates = new Map();
  const monsters = new Map();
  for (const spec of specs) {
    const monHash = `m_${spec.id}`;
    const tmplHash = `t_${spec.templateId}`;
    names[monHash] = spec.name;
    names[tmplHash] = spec.templateName ?? spec.name;
    if (!templates.has(spec.templateId)) {
      templates.set(spec.templateId, {
        MonsterTemplateID: spec.templateId,
        MonsterName: { Hash: tmplHash },
        Rank: spec.rank,
        IconPath: `SpriteOutput/MosterIcon/Monster_${spec.icon}.png`,
        ImagePath: `SpriteOutput/MonsterFigure/Monster_${spec.icon}.png`,
        ...(spec.stance > 0 ? { StanceBase: { Value: spec.stance } } : {}),
        ...(spec.atlas !== undefined ? { AtlasSortID: spec.atlas } : {}),
        ...(spec.group !== undefined ? { TemplateGroupID: spec.group } : {}),
      });
    }
    monsters.set(spec.id, {
      MonsterID: spec.id,
      MonsterTemplateID: spec.templateId,
      MonsterName: { Hash: monHash },
      StanceWeakList: spec.weak,
      SummonIDList: spec.summons,
      CustomValueTags: [],
    });
  }

  const lineup = selectableEnemies(tables, monsters, templates, names);
  assert.deepEqual(
    lineup.map(({ name, variant }) => ({ name, variant })),
    [
      { name: '首领', variant: null },
      { name: '末日首领', variant: '末日幻影' },
      { name: '王棋独立首领', variant: null },
      { name: '常规小怪', variant: null },
    ],
  );
  assert.deepEqual(lineup[0].sourceIds, ['100', '101']);
});
