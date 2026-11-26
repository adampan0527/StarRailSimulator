import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { URL } from 'node:url';
import {
  classifyEnemyAi,
  parseAiConfig,
} from '../scripts/m03/import-enemy-combat.mjs';

const preview = JSON.parse(
  readFileSync(
    new URL('../src/team/enemy-preview.json', import.meta.url),
    'utf8',
  ),
);
const combat = JSON.parse(
  readFileSync(
    new URL(
      '../crates/battle-core/data/enemy-combat-4.7.0.json',
      import.meta.url,
    ),
    'utf8',
  ),
);
const lock = JSON.parse(
  readFileSync(
    new URL('../scripts/m03/enemy-ai-lock.json', import.meta.url),
    'utf8',
  ),
);

const combatById = new Map(combat.enemies.map((enemy) => [enemy.id, enemy]));

test('locks all 275 upstream enemy AI, skill, status, and character config files at 4.7.0 revision', () => {
  assert.equal(lock.repository, 'DimbreathBot/TurnBasedGameData');
  assert.equal(lock.revision, preview.revision);
  const paths = Object.keys(lock.files);
  assert.equal(paths.length, 275);
  assert.ok(paths.includes('ExcelOutput/MonsterSkillConfig.json'));
  assert.ok(paths.includes('ExcelOutput/MonsterStatusConfig.json'));
  for (const [repoPath, digest] of Object.entries(lock.files)) {
    assert.match(digest, /^[a-f0-9]{64}$/, `invalid SHA-256 on ${repoPath}`);
  }
  for (const enemy of combat.enemies) {
    assert.ok(lock.files[enemy.aiPath], `unlocked AIPath ${enemy.aiPath}`);
    assert.ok(
      lock.files[enemy.jsonConfig],
      `unlocked JsonConfig ${enemy.jsonConfig}`,
    );
  }
});

test('aligns enemy combat catalog 1:1 with enemy-preview and matches level 1 and 100 stats', () => {
  assert.equal(combat.schemaVersion, 1);
  assert.equal(combat.gameVersion, '4.7.0');
  assert.equal(combat.revision, preview.revision);
  assert.equal(combat.enemies.length, preview.enemies.length);
  assert.equal(combat.summary.enemyCount, 159);
  assert.equal(combat.summary.totalSkills, 970);
  assert.equal(
    combat.summary.byTier.sequence_only +
      combat.summary.byTier.generic_rules +
      combat.summary.byTier.custom_mechanics,
    159,
  );
  assert.equal(
    combat.summary.sequenceOnlyPure + combat.summary.sequenceOnlyWithPassive,
    combat.summary.byTier.sequence_only,
  );

  for (const previewEnemy of preview.enemies) {
    const entry = combatById.get(previewEnemy.id);
    assert.ok(entry, `missing combat enemy ${previewEnemy.id}`);
    assert.equal(entry.name, previewEnemy.name);
    assert.equal(entry.planet, previewEnemy.planet);
    assert.equal(entry.rank, previewEnemy.rank);
    assert.equal(entry.variant, previewEnemy.variant);
    assert.deepEqual(entry.weaknesses, previewEnemy.weaknesses);
    assert.ok(entry.skills.length > 0, `no skills on ${entry.id}`);
    assert.ok(entry.combatStats.stanceBase > 0, `no stance on ${entry.id}`);
    assert.equal(Object.keys(entry.combatStats.damageTypeResistance).length, 7);
    assert.ok(entry.unresolved.length > 0);

    const curves = combat.levelCurves[String(entry.combatStats.hardLevelGroup)];
    assert.equal(curves.length, 100);
    for (const level of [1, 100]) {
      const curve = curves[level - 1];
      const idx = preview.levels.indexOf(level);
      const hp = Number(
        (
          entry.combatStats.hpBase *
          entry.combatStats.hpModifyRatio *
          entry.combatStats.hpEliteRatio *
          curve.hpRatio
        ).toFixed(6),
      );
      const defense = Number(
        (
          entry.combatStats.defenseBase *
          entry.combatStats.defenseModifyRatio *
          entry.combatStats.defenseEliteRatio *
          curve.defenseRatio
        ).toFixed(6),
      );
      const speed = Number(
        (
          entry.combatStats.speedBase *
            entry.combatStats.speedModifyRatio *
            entry.combatStats.speedEliteRatio *
            curve.speedRatio +
          entry.combatStats.speedModifyValue
        ).toFixed(6),
      );
      assert.equal(hp, preview.statSeries[previewEnemy.hp][idx]);
      assert.equal(defense, preview.statSeries[previewEnemy.defense][idx]);
      assert.equal(speed, preview.statSeries[previewEnemy.speed][idx]);
    }
  }
});

test('classifies representative pure sequence, generic CD, and custom boss/gimmick enemies accurately', () => {
  const flamespawn = combatById.get('8001010');
  assert.equal(flamespawn.tier, 'sequence_only');
  assert.equal(flamespawn.hasPassiveTalent, false);
  assert.deepEqual(flamespawn.skillSequence, ['800101001']);
  assert.equal(flamespawn.skills[0].spHitBase, 10);
  assert.equal(flamespawn.skills[0].targetType, 'EnemySelect');

  const dragonfish = combatById.get('2012010');
  assert.equal(dragonfish.tier, 'sequence_only');
  assert.equal(dragonfish.hasPassiveTalent, true);

  const titankin = combatById.get('4012010');
  assert.equal(titankin.tier, 'generic_rules');
  assert.ok(titankin.classificationReasons.includes('skill_cd_usability'));

  const domescreen = combatById.get('3002010');
  assert.equal(domescreen.tier, 'custom_mechanics');
  assert.ok(
    domescreen.classificationReasons.includes('external_dynamic_state'),
  );

  const mirror = combatById.get('3012010');
  assert.equal(mirror.tier, 'custom_mechanics');
  assert.ok(
    mirror.classificationReasons.includes('contains_special_skill_tag'),
  );

  const devourer = combatById.get('5023020');
  assert.equal(devourer.tier, 'custom_mechanics');
  assert.ok(
    devourer.classificationReasons.includes('empty_ai_decision_ability_driven'),
  );

  const doomsday = combatById.get('8015012');
  assert.equal(doomsday.tier, 'custom_mechanics');
  assert.ok(
    doomsday.classificationReasons.includes('empty_sequence_ability_driven'),
  );

  const cocolia = combatById.get('1004010');
  assert.equal(cocolia.tier, 'custom_mechanics');
  assert.ok(cocolia.classificationReasons.includes('dynamic_state_machine'));
  assert.ok(cocolia.skills.some((skill) => skill.spHitBase === 20));

  const trueSting = combatById.get('8024011');
  assert.equal(trueSting.tier, 'custom_mechanics');
  assert.ok(trueSting.classificationReasons.includes('multi_phase'));
});

test('parseAiConfig and classifyEnemyAi distinguish self-contained cycles from external state and empty decisions', () => {
  const sequenceAi = parseAiConfig({
    AIName: 'SequenceTest',
    DecisionList: [
      {
        DecisionName: 'UseSeq',
        ConsiderAxisList: [{ $type: 'RPG.GameCore.ChoseSequencedSkillAxis' }],
        RootTask: {
          $type: 'RPG.GameCore.SequenceConfig',
          TaskList: [{ $type: 'RPG.GameCore.UseSequencedSkill' }],
        },
      },
    ],
  });
  const basicSkill = {
    id: '1001',
    triggerKey: 'Skill01',
    skillTag: '单攻',
    isThreat: false,
  };
  assert.equal(
    classifyEnemyAi({
      parsedAi: sequenceAi,
      skillSequence: ['1001'],
      skills: [basicSkill],
      summonIds: [],
    }).tier,
    'sequence_only',
  );
  assert.deepEqual(
    classifyEnemyAi({
      parsedAi: sequenceAi,
      skillSequence: [],
      skills: [basicSkill],
      summonIds: [],
    }),
    { tier: 'custom_mechanics', reasons: ['empty_sequence_ability_driven'] },
  );

  const externalStateAi = parseAiConfig({
    AIName: 'ExternalStateTest',
    DecisionList: [
      {
        DecisionName: 'Use01',
        ConsiderAxisList: [
          {
            $type: 'RPG.GameCore.CheckPredicateAxis',
            Predicate: {
              $type: 'RPG.GameCore.ByCompareDynamicValue',
              DynamicKey: { Value: 'ExternalHitFlag' },
            },
          },
        ],
        RootTask: {
          $type: 'RPG.GameCore.SequenceConfig',
          TaskList: [{ $type: 'RPG.GameCore.UseSkill', SkillName: 'Skill01' }],
        },
      },
    ],
  });
  const externalResult = classifyEnemyAi({
    parsedAi: externalStateAi,
    skillSequence: [],
    skills: [basicSkill],
    summonIds: [],
  });
  assert.equal(externalResult.tier, 'custom_mechanics');
  assert.ok(externalResult.reasons.includes('external_dynamic_state'));
});
