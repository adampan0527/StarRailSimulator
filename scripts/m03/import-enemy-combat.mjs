import { createHash } from 'node:crypto';
import console from 'node:console';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';
import { format, resolveConfig } from 'prettier';
import { localFilePath, readEnemyAiLock } from './download-enemy-ai.mjs';
import { PLANETS, selectableEnemies } from './selectable-enemies.mjs';
import { check, index, lookup, readLock, sha256 } from './sources.mjs';

const REVISION = 'dad9155eb00fdeda31fb8456507c53590d49071f';
const BLOBS = {
  MonsterConfig: 'b689baa89165368a8af239f179959147ace62827',
  MonsterTemplateConfig: '894de1554a58f444cf3fe3b4c3f6e9c46828f5db',
  HardLevelGroup: 'a5efd001f26d23f5fb46617039d9d5cdbb9a5b91',
  EliteGroup: '3649b66b64eae47f7d49900377c42f09d99f93e7',
  StageConfig: '2e1c9d6ce95988e680d7900a111e703f658c82d1',
  StageConfigLD: '27a5e3e955aa2a17d0e6ba96662ceca59149806d',
  StageInfiniteMonsterGroup: '9afc7e55aece474d942a4ce32de4149bf5c6ab2c',
};
const RANKS = {
  Minion: '普通',
  MinionLv2: '普通',
  Elite: '精英',
  LittleBoss: '首领',
  BigBoss: '首领',
};
const ELEMENTS = [
  'physical',
  'fire',
  'ice',
  'thunder',
  'wind',
  'quantum',
  'imaginary',
];
const SPECIAL_SKILL_TAGS = new Set(['蓄力', '召唤', '锁定', '分摊', '其他']);

function speedOffset(template, monster) {
  const templateOffset = template.SpeedModifyValue?.Value;
  const variantOffset = monster.SpeedModifyValue?.Value;
  if (templateOffset !== undefined && variantOffset !== undefined) return null;
  return variantOffset ?? templateOffset ?? 0;
}

function parseJsonPreservingHashes(text) {
  return JSON.parse(text, (key, value, context) =>
    key === 'Hash' && typeof value === 'number' ? context.source : value,
  );
}

function cleanText(text) {
  if (!text) return '';
  return text.replaceAll(/<\/?unbreak>/g, '').trim();
}

function stripTypePrefix(typeName) {
  return typeof typeName === 'string'
    ? typeName.replace(/^RPG\.GameCore\./, '')
    : '';
}

function collectNodeFeatures(node, state, bucket = 'task') {
  if (!node || typeof node !== 'object') return;
  if (Array.isArray(node)) {
    for (const item of node) collectNodeFeatures(item, state, bucket);
    return;
  }
  const type = stripTypePrefix(node.$type);
  if (type) {
    if (bucket === 'predicate') state.predicateTypes.add(type);
    else if (bucket === 'selector') state.selectorTypes.add(type);
    else state.taskTypes.add(type);
  }
  if (type === 'UseSkill' && node.SkillName) {
    state.skillsUsed.add(node.SkillName);
  }
  if (type === 'UseSequencedSkill') {
    state.usesSequencedSkill = true;
  }
  if (type === 'SelectAISkillTarget' || type === 'Retarget') {
    state.usesTargetSelector = true;
  }
  if (
    type === 'DefineDynamicValue' ||
    type === 'SetDynamicValue' ||
    type === 'SetDynamicValueByAddValue' ||
    type === 'SetDynamicValueByCharacterCount'
  ) {
    const key = node.DynamicKey?.Value ?? node.DynamicKey;
    if (typeof key === 'string' && key) state.mutatedDynamicKeys.add(key);
  }
  if (type === 'ByCompareDynamicValue') {
    const key = node.DynamicKey?.Value ?? node.DynamicKey;
    if (typeof key === 'string' && key) state.checkedDynamicKeys.add(key);
  }
  if (type === 'AddModifier' || type === 'RemoveModifier') {
    state.mutatesModifiers = true;
  }
  for (const [key, value] of Object.entries(node)) {
    if (key === 'Predicate' || key === 'PredicateList') {
      collectNodeFeatures(value, state, 'predicate');
    } else if (key === 'Selector' || key === 'SelectorList') {
      collectNodeFeatures(value, state, 'selector');
    } else {
      collectNodeFeatures(value, state, bucket);
    }
  }
}

export function parseAiConfig(aiJson) {
  const axisTypes = new Set();
  const predicateTypes = new Set();
  const selectorTypes = new Set();
  const taskTypes = new Set();
  const checkedDynamicKeys = new Set();
  const mutatedDynamicKeys = new Set();
  const decisions = [];

  for (const decision of aiJson.DecisionList ?? []) {
    const local = {
      predicateTypes: new Set(),
      selectorTypes: new Set(),
      taskTypes: new Set(),
      skillsUsed: new Set(),
      checkedDynamicKeys: new Set(),
      mutatedDynamicKeys: new Set(),
      usesSequencedSkill: false,
      usesTargetSelector: false,
      mutatesModifiers: false,
    };
    const axes = [];
    for (const axis of decision.ConsiderAxisList ?? []) {
      const axisType = stripTypePrefix(axis.$type);
      if (axisType) axisTypes.add(axisType);
      const axisSummary = { type: axisType };
      if (axis.SkillName) axisSummary.skillName = axis.SkillName;
      if (axis.CD !== undefined) axisSummary.cd = axis.CD;
      if (axis.InitialCD !== undefined) axisSummary.initialCd = axis.InitialCD;
      if (axis.Predicate) {
        const predState = {
          predicateTypes: new Set(),
          selectorTypes: new Set(),
          taskTypes: new Set(),
          skillsUsed: new Set(),
          checkedDynamicKeys: new Set(),
          mutatedDynamicKeys: new Set(),
          usesSequencedSkill: false,
          usesTargetSelector: false,
          mutatesModifiers: false,
        };
        collectNodeFeatures(axis.Predicate, predState, 'predicate');
        for (const item of predState.predicateTypes) {
          local.predicateTypes.add(item);
        }
        for (const item of predState.checkedDynamicKeys) {
          local.checkedDynamicKeys.add(item);
        }
        axisSummary.predicates = [...predState.predicateTypes].sort();
      }
      axes.push(axisSummary);
    }
    collectNodeFeatures(decision.RootTask, local, 'task');
    for (const item of local.predicateTypes) predicateTypes.add(item);
    for (const item of local.selectorTypes) selectorTypes.add(item);
    for (const item of local.taskTypes) taskTypes.add(item);
    for (const item of local.checkedDynamicKeys) checkedDynamicKeys.add(item);
    for (const item of local.mutatedDynamicKeys) mutatedDynamicKeys.add(item);
    decisions.push({
      name: decision.DecisionName ?? '',
      weight: decision.Weight?.Value ?? 1,
      axes,
      skillsUsed: local.usesSequencedSkill
        ? ['<sequence>']
        : [...local.skillsUsed].sort(),
      usesTargetSelector: local.usesTargetSelector,
      mutatesModifiers: local.mutatesModifiers,
      mutatedDynamicKeys: [...local.mutatedDynamicKeys].sort(),
    });
  }

  return {
    aiName: aiJson.AIName ?? '',
    variables: (aiJson.VariableList ?? []).map((variable) => ({
      name: variable.Name,
      type: stripTypePrefix(variable.$type),
      value: variable.Value,
    })),
    decisionCount: decisions.length,
    axisTypes: [...axisTypes].sort(),
    predicateTypes: [...predicateTypes].sort(),
    selectorTypes: [...selectorTypes].sort(),
    taskTypes: [...taskTypes].sort(),
    checkedDynamicKeys: [...checkedDynamicKeys].sort(),
    mutatedDynamicKeys: [...mutatedDynamicKeys].sort(),
    decisions,
  };
}

export function classifyEnemyAi({
  parsedAi,
  skillSequence,
  skills,
  summonIds,
}) {
  const axes = new Set(parsedAi.axisTypes);
  const predicates = new Set(parsedAi.predicateTypes);
  const tasks = new Set(parsedAi.taskTypes);
  const reasons = [];

  const isSingleSequenceDecision =
    parsedAi.decisionCount === 1 &&
    tasks.has('UseSequencedSkill') &&
    axes.size === 1 &&
    axes.has('ChoseSequencedSkillAxis');

  const activeSkills = skills.filter(
    (skill) => skill.skillTag !== '天赋' && skill.triggerKey !== '',
  );
  const knownTriggerKeys = new Set(
    skills.map((skill) => skill.triggerKey).filter(Boolean),
  );
  const hasThreatOrSpecialTag = activeSkills.some(
    (skill) => skill.isThreat || SPECIAL_SKILL_TAGS.has(skill.skillTag),
  );
  const hasSummons = summonIds.length > 0;
  const hasPhase = predicates.has('ByCompareMonsterPhase');
  const hasModifierLogic =
    predicates.has('ByIsContainModifier') ||
    predicates.has('ByCompareModifierValue') ||
    tasks.has('AddModifier') ||
    tasks.has('RemoveModifier');
  const hasTargetSelector =
    tasks.has('SelectAISkillTarget') || tasks.has('Retarget');
  const hasAdvancedPredicates = [
    'ByCheckCustomValueBool',
    'ByCompareBP',
    'ByContainBehaviorFlag',
    'ByCompareSkillUsageLimit',
    'ByCompareCharacterNumber',
    'ByCompareAliveEnemyNumber',
    'ByCompareTargetCount',
    'ByTargetAliveState',
    'ByRandomChance',
    'TargetSortByFormation',
    'TargetIndex',
  ].some((name) => predicates.has(name));
  const hasDynamicState =
    predicates.has('ByCompareDynamicValue') ||
    tasks.has('DefineDynamicValue') ||
    tasks.has('SetDynamicValue') ||
    tasks.has('SetDynamicValueByAddValue') ||
    tasks.has('SetDynamicValueByCharacterCount');
  const mutatedKeys = new Set(parsedAi.mutatedDynamicKeys);
  const hasExternalDynamicState = parsedAi.checkedDynamicKeys.some(
    (key) => !mutatedKeys.has(key),
  );
  const hasEmptyDecision =
    parsedAi.decisionCount === 0 ||
    parsedAi.decisions.some((decision) => decision.skillsUsed.length === 0);
  const hasUnmappedAiSkill = parsedAi.decisions.some((decision) =>
    decision.skillsUsed.some(
      (skillName) =>
        skillName !== '<sequence>' && !knownTriggerKeys.has(skillName),
    ),
  );

  if (isSingleSequenceDecision) {
    if (skillSequence.length === 0) {
      reasons.push('empty_sequence_ability_driven');
      return { tier: 'custom_mechanics', reasons };
    }
    const skillIds = new Set(skills.map((skill) => skill.id));
    check(
      skillSequence.every((id) => skillIds.has(id)),
      `skillSequence references unknown skill in ${parsedAi.aiName}`,
    );
    reasons.push('fixed_skill_sequence');
    if (hasThreatOrSpecialTag) reasons.push('contains_special_skill_tag');
    if (hasSummons) reasons.push('has_summon_ids');
    return {
      tier:
        hasThreatOrSpecialTag || hasSummons
          ? 'custom_mechanics'
          : 'sequence_only',
      reasons,
    };
  }

  if (hasEmptyDecision) reasons.push('empty_ai_decision_ability_driven');
  if (hasUnmappedAiSkill) reasons.push('unmapped_ai_skill');
  if (hasExternalDynamicState) reasons.push('external_dynamic_state');
  if (hasPhase) reasons.push('multi_phase');
  if (hasSummons) reasons.push('has_summon_ids');
  if (hasThreatOrSpecialTag) reasons.push('contains_special_skill_tag');
  if (hasModifierLogic) reasons.push('modifier_condition_or_mutation');
  if (hasTargetSelector) reasons.push('custom_target_selector');
  if (hasAdvancedPredicates) reasons.push('battle_state_predicates');
  if (hasDynamicState) reasons.push('dynamic_state_machine');
  if (axes.has('CheckSkillUsabilityAxis')) reasons.push('skill_cd_usability');

  if (
    !hasEmptyDecision &&
    !hasUnmappedAiSkill &&
    !hasExternalDynamicState &&
    !hasPhase &&
    !hasSummons &&
    !hasThreatOrSpecialTag &&
    !hasModifierLogic &&
    !hasTargetSelector &&
    !hasAdvancedPredicates
  ) {
    return { tier: 'generic_rules', reasons };
  }

  return { tier: 'custom_mechanics', reasons };
}

function extractSkillSequence(monster, template) {
  const raw =
    monster.OverrideAISkillSequence?.length > 0
      ? monster.OverrideAISkillSequence
      : (template.AISkillSequence ?? []);
  return raw.map((entry) => {
    const values = Object.values(entry);
    check(
      values.length === 1 && Number.isInteger(values[0]),
      `unexpected AISkillSequence entry on monster ${monster.MonsterID}`,
    );
    return String(values[0]);
  });
}

function buildResistances(monster) {
  const map = Object.fromEntries(ELEMENTS.map((element) => [element, 0]));
  for (const entry of monster.DamageTypeResistance ?? []) {
    const element = entry.DamageType?.toLowerCase();
    check(
      ELEMENTS.includes(element),
      `unknown damage resistance element ${entry.DamageType} on ${monster.MonsterID}`,
    );
    map[element] = Number((entry.Value?.Value ?? 0).toFixed(6));
  }
  return map;
}

function buildDebuffResist(monster) {
  return (monster.DebuffResist ?? [])
    .map((entry) => ({
      key: entry.Key,
      value: Number((entry.Value?.Value ?? 0).toFixed(6)),
    }))
    .sort((a, b) => a.key.localeCompare(b.key));
}

function resolveOverrideParams(monster) {
  const overrides = new Map();
  for (const entry of monster.OverrideSkillParams ?? []) {
    if (entry.SkillID !== undefined && Array.isArray(entry.ParamList)) {
      overrides.set(
        String(entry.SkillID),
        entry.ParamList.map((item) => item.Value),
      );
    }
  }
  return overrides;
}

export function buildEnemyCombatCatalog({
  tables,
  textMap,
  monsterSkills,
  monsterStatuses,
  aiConfigs,
  charConfigs,
}) {
  const monsters = index(tables.MonsterConfig, 'MonsterID', 'MonsterConfig');
  const templates = index(
    tables.MonsterTemplateConfig,
    'MonsterTemplateID',
    'MonsterTemplateConfig',
  );
  const eliteGroups = index(tables.EliteGroup, 'EliteGroup', 'EliteGroup');
  const skillById = index(monsterSkills, 'SkillID', 'MonsterSkillConfig');
  const selected = selectableEnemies(tables, monsters, templates, textMap);

  const enemies = selected.map(
    ({ monster, template, name, planet, variant, sourceIds }) => {
      const id = String(monster.MonsterID);
      const templateId = String(monster.MonsterTemplateID);
      const elite = lookup(eliteGroups, monster.EliteGroup, 'EliteGroup');
      const aiPath = monster.OverrideAIPath || template.AIPath;
      const charPath = template.JsonConfig;
      const aiJson = lookup(aiConfigs, aiPath, 'ConfigAI');
      const charJson = lookup(charConfigs, charPath, 'ConfigCharacter');
      const charSkillsByName = new Map(
        (charJson.SkillList ?? []).map((skill) => [skill.Name, skill]),
      );
      const overrideParams = resolveOverrideParams(monster);

      const skills = (monster.SkillList ?? []).map((rawSkillId) => {
        const skillId = String(rawSkillId);
        const row = lookup(skillById, skillId, 'MonsterSkillConfig');
        const triggerKey = row.SkillTriggerKey ?? '';
        const charSkill = triggerKey
          ? (charSkillsByName.get(triggerKey) ?? null)
          : null;
        const params =
          overrideParams.get(skillId) ??
          (row.ParamList ?? []).map((item) => item.Value);
        return {
          id: skillId,
          name: cleanText(textMap[row.SkillName?.Hash]),
          triggerKey,
          skillTag: cleanText(textMap[row.SkillTag?.Hash]),
          skillTypeDesc: cleanText(textMap[row.SkillTypeDesc?.Hash]),
          damageType: row.DamageType ? row.DamageType.toLowerCase() : null,
          attackType: row.AttackType ?? null,
          targetType: charSkill?.TargetInfo?.TargetType ?? null,
          useType: charSkill?.UseType ?? null,
          entryAbility: charSkill?.EntryAbility ?? null,
          spHitBase: row.SPHitBase?.Value ?? null,
          delayRatio: row.DelayRatio?.Value ?? 1,
          aiCd: row.AI_CD ?? null,
          aiIcd: row.AI_ICD ?? null,
          isThreat: Boolean(row.IsThreat),
          phaseList: row.PhaseList ?? [],
          params,
          desc: cleanText(textMap[row.SkillDesc?.Hash]),
        };
      });

      const skillSequence = extractSkillSequence(monster, template);
      const summonIds = (monster.SummonIDList ?? []).map(String);
      const parsedAi = parseAiConfig(aiJson);
      const classification = classifyEnemyAi({
        parsedAi,
        skillSequence,
        skills,
        summonIds,
      });

      const attackBase = template.AttackBase?.Value ?? null;
      const attackModifyRatio = monster.AttackModifyRatio?.Value ?? 1;
      const attackEliteRatio = elite.AttackRatio?.Value ?? 1;
      const hpBase = template.HPBase?.Value ?? null;
      const hpModifyRatio = monster.HPModifyRatio?.Value ?? 1;
      const hpEliteRatio = elite.HPRatio?.Value ?? 1;
      const defenseBase = template.DefenceBase?.Value ?? null;
      const defenseModifyRatio = monster.DefenceModifyRatio?.Value ?? 1;
      const defenseEliteRatio = elite.DefenceRatio?.Value ?? 1;
      const speedBase = template.SpeedBase?.Value ?? null;
      const speedModifyRatio = monster.SpeedModifyRatio?.Value ?? 1;
      const speedEliteRatio = elite.SpeedRatio?.Value ?? 1;
      const speedModifyValue = speedOffset(template, monster);
      const passiveSkillIds = skills
        .filter((skill) => skill.skillTag === '天赋')
        .map((skill) => skill.id);

      return {
        id,
        templateId,
        name,
        planet,
        rank: lookup(RANKS, template.Rank, 'rank'),
        variant,
        sourceIds,
        weaknesses: monster.StanceWeakList.map((type) => type.toLowerCase()),
        combatStats: {
          hardLevelGroup: monster.HardLevelGroup,
          eliteGroup: monster.EliteGroup,
          hpBase,
          hpModifyRatio,
          hpEliteRatio,
          attackBase,
          attackModifyRatio,
          attackEliteRatio,
          defenseBase,
          defenseModifyRatio,
          defenseEliteRatio,
          speedBase,
          speedModifyRatio,
          speedEliteRatio,
          speedModifyValue,
          stanceBase: template.StanceBase?.Value ?? 0,
          stanceModifyRatio: monster.StanceModifyRatio?.Value ?? 1,
          stanceCount: template.StanceCount ?? 1,
          initialDelayRatio: template.InitialDelayRatio?.Value ?? 1,
          statusResistanceBase: template.StatusResistanceBase?.Value ?? 0,
          minimumFatigueRatio: template.MinimumFatigueRatio?.Value ?? 0,
          damageTypeResistance: buildResistances(monster),
          debuffResist: buildDebuffResist(monster),
        },
        summonIds,
        abilityNames: monster.AbilityNameList ?? [],
        customValueTags: monster.CustomValueTags ?? [],
        aiPath,
        jsonConfig: charPath,
        skillSequence,
        hasPassiveTalent: passiveSkillIds.length > 0,
        passiveSkillIds,
        tier: classification.tier,
        classificationReasons: classification.reasons,
        ai: parsedAi,
        skills,
        unresolved: [
          'ability_damage_multipliers_and_hit_splits_from_config_ability',
          'status_base_chance_and_duration_from_config_ability',
        ],
      };
    },
  );

  const referencedLevelGroups = new Set(
    enemies.map((enemy) => enemy.combatStats.hardLevelGroup),
  );
  const levelCurves = {};
  for (const groupId of [...referencedLevelGroups].sort((a, b) => a - b)) {
    const rows = tables.HardLevelGroup.filter(
      (row) =>
        row.HardLevelGroup === groupId && row.Level >= 1 && row.Level <= 100,
    ).sort((a, b) => a.Level - b.Level);
    check(
      rows.length === 100,
      `hardLevelGroup ${groupId}: expected 100 levels`,
    );
    levelCurves[String(groupId)] = rows.map((row) => ({
      level: row.Level,
      hpRatio: row.HPRatio?.Value ?? 1,
      attackRatio: row.AttackRatio?.Value ?? 1,
      defenseRatio: row.DefenceRatio?.Value ?? 1,
      speedRatio: row.SpeedRatio?.Value ?? 1,
      stanceRatio: row.StanceRatio?.Value ?? 1,
      statusResistance: row.StatusResistance?.Value ?? 0,
      statusProbability: row.StatusProbability?.Value ?? 0,
    }));
  }

  const summary = summarizeCatalog(enemies, monsterStatuses);
  return {
    schemaVersion: 1,
    gameVersion: '4.7.0',
    revision: REVISION,
    planets: PLANETS,
    levelCurves,
    summary,
    enemies,
  };
}

function summarizeCatalog(enemies, monsterStatuses) {
  const byTier = { sequence_only: 0, generic_rules: 0, custom_mechanics: 0 };
  const byTierAndRank = {
    sequence_only: {},
    generic_rules: {},
    custom_mechanics: {},
  };
  const reasonCounts = new Map();
  const targetTypeCounts = new Map();
  let totalSkills = 0;
  let skillsWithSpHitBase = 0;
  let skillsWithoutCharTrigger = 0;
  let sequenceOnlyPure = 0;
  let sequenceOnlyWithPassive = 0;

  for (const enemy of enemies) {
    byTier[enemy.tier] += 1;
    byTierAndRank[enemy.tier][enemy.rank] =
      (byTierAndRank[enemy.tier][enemy.rank] ?? 0) + 1;
    if (enemy.tier === 'sequence_only') {
      if (enemy.hasPassiveTalent) sequenceOnlyWithPassive += 1;
      else sequenceOnlyPure += 1;
    }
    for (const reason of enemy.classificationReasons) {
      reasonCounts.set(reason, (reasonCounts.get(reason) ?? 0) + 1);
    }
    for (const skill of enemy.skills) {
      totalSkills += 1;
      if (skill.spHitBase !== null) skillsWithSpHitBase += 1;
      if (skill.targetType === null) skillsWithoutCharTrigger += 1;
      else {
        targetTypeCounts.set(
          skill.targetType,
          (targetTypeCounts.get(skill.targetType) ?? 0) + 1,
        );
      }
    }
  }

  return {
    enemyCount: enemies.length,
    distinctAiPaths: new Set(enemies.map((enemy) => enemy.aiPath)).size,
    distinctCharConfigs: new Set(enemies.map((enemy) => enemy.jsonConfig)).size,
    totalSkills,
    skillsWithSpHitBase,
    skillsWithoutCharTrigger,
    statusConfigCount: monsterStatuses.length,
    byTier,
    byTierAndRank,
    sequenceOnlyPure,
    sequenceOnlyWithPassive,
    reasonCounts: Object.fromEntries(
      [...reasonCounts.entries()].sort((a, b) => b[1] - a[1]),
    ),
    targetTypeCounts: Object.fromEntries(
      [...targetTypeCounts.entries()].sort((a, b) => b[1] - a[1]),
    ),
  };
}

function stripDescriptionsForCompactCatalog(catalog) {
  return {
    ...catalog,
    enemies: catalog.enemies.map(({ skills, ...rest }) => ({
      ...rest,
      skills: skills.map(({ desc, ...skillRest }) => {
        void desc;
        return skillRest;
      }),
    })),
  };
}

function formatAuditMarkdown(catalog) {
  const { summary, enemies } = catalog;
  const seqEnemies = enemies.filter((enemy) => enemy.tier === 'sequence_only');
  const genericEnemies = enemies.filter(
    (enemy) => enemy.tier === 'generic_rules',
  );
  const customEnemies = enemies.filter(
    (enemy) => enemy.tier === 'custom_mechanics',
  );

  const lines = [
    '# 4.7.0 敌方静态技能与 AI 决策归类审计',
    '',
    `数据基线：\`DimbreathBot/TurnBasedGameData\` 修订 \`${catalog.revision}\`（国服 4.7.0），与 [enemy-preview.json](../src/team/enemy-preview.json) 严格 1:1 对齐，共覆盖 **${summary.enemyCount}** 个高难实体战斗单位。所有 275 份上游源文件均通过 [enemy-ai-lock.json](../scripts/m03/enemy-ai-lock.json) 的 SHA-256 校验，可完全离线重建。`,
    '',
    '## 总体统计',
    '',
    '| 指标 | 数值 |',
    '| --- | --- |',
    `| 高难可选敌方单位 | ${summary.enemyCount} |`,
    `| 独立 ConfigAI 决策文件 | ${summary.distinctAiPaths} |`,
    `| 独立 ConfigCharacter 角色配置文件 | ${summary.distinctCharConfigs} |`,
    `| 已解析技能条目（MonsterSkillConfig） | ${summary.totalSkills}（含受击回能 \`SPHitBase\`：${summary.skillsWithSpHitBase} 条；纯被动/未绑定触发键：${summary.skillsWithoutCharTrigger} 条） |`,
    `| 上游状态定义（MonsterStatusConfig） | ${summary.statusConfigCount} |`,
    '',
    '## 实现层级归类（供第二步与第三步排期）',
    '',
    '| 层级 | 数量 | 构成 | 落地策略 |',
    '| --- | --- | --- | --- |',
    `| \`sequence_only\`（纯固定序列） | ${summary.byTier.sequence_only} | 普通 ${summary.byTierAndRank.sequence_only['普通'] ?? 0}（无被动白板 ${summary.sequenceOnlyPure}、含被动天赋 ${summary.sequenceOnlyWithPassive}） | 第二步通用执行器直接数据驱动：按 \`skillSequence\` 循环施放；含被动天赋单位另行标记被动钩子 |`,
    `| \`generic_rules\`（通用周期/CD 规则） | ${summary.byTier.generic_rules} | 普通 ${summary.byTierAndRank.generic_rules['普通'] ?? 0}、精英 ${summary.byTierAndRank.generic_rules['精英'] ?? 0}、首领 ${summary.byTierAndRank.generic_rules['首领'] ?? 0} | 第二步通用执行器直接数据驱动：支持 \`AI_CD\`/\`AI_ICD\` 与自洽局部周期变量 |`,
    `| \`custom_mechanics\`（专属机制） | ${summary.byTier.custom_mechanics} | 首领 ${summary.byTierAndRank.custom_mechanics['首领'] ?? 0}、精英 ${summary.byTierAndRank.custom_mechanics['精英'] ?? 0}、普通 ${summary.byTierAndRank.custom_mechanics['普通'] ?? 0} | 第三步按高难出场优先级逐个建立独立 Rust 机制模块并补逐行动回归 |`,
    '',
    '### 特征命中统计',
    '',
    '| 分类特征 | 命中单位数 |',
    '| --- | --- |',
    ...Object.entries(summary.reasonCounts).map(
      ([reason, count]) => `| \`${reason}\` | ${count} |`,
    ),
    '',
    '### 技能目标类型分布（来自 `ConfigCharacter`）',
    '',
    '| `TargetType` | 技能数 |',
    '| --- | --- |',
    ...Object.entries(summary.targetTypeCounts).map(
      ([targetType, count]) => `| \`${targetType}\` | ${count} |`,
    ),
    '',
    '## 第二步可直接数据驱动的单位清单',
    '',
    `### 1. \`sequence_only\`（${seqEnemies.length} 个）`,
    '',
    '| ID | 名称 | 星球 | 序列长度 | 含被动 | 技能列表（触发键 / 目标 / 受击回能） |',
    '| --- | --- | --- | --- | --- | --- |',
    ...seqEnemies.map((enemy) => {
      const skillText = enemy.skills
        .map(
          (skill) =>
            `${skill.name || skill.id}(${skill.triggerKey || '-'}/${skill.targetType ?? '-'}/回能${skill.spHitBase ?? 0})`,
        )
        .join('、');
      return `| \`${enemy.id}\` | ${enemy.name} | ${enemy.planet} | ${enemy.skillSequence.length} | ${enemy.hasPassiveTalent ? '是' : '否'} | ${skillText} |`;
    }),
    '',
    `### 2. \`generic_rules\`（${genericEnemies.length} 个）`,
    '',
    '| ID | 名称 | 分级 | 星球 | 决策数 | 特征 |',
    '| --- | --- | --- | --- | --- | --- |',
    ...genericEnemies.map(
      (enemy) =>
        `| \`${enemy.id}\` | ${enemy.name} | ${enemy.rank} | ${enemy.planet} | ${enemy.ai.decisionCount} | ${enemy.classificationReasons.map((reason) => `\`${reason}\``).join('、')} |`,
    ),
    '',
    `## 第三步需专属机制模块的单位概览（${customEnemies.length} 个）`,
    '',
    '| 分级 | 数量 | 代表单位（前 6 个） |',
    '| --- | --- | --- |',
    ...['首领', '精英', '普通'].map((rank) => {
      const group = customEnemies.filter((enemy) => enemy.rank === rank);
      const sample = group
        .slice(0, 6)
        .map((enemy) => `${enemy.name}(\`${enemy.id}\`)`)
        .join('、');
      return `| ${rank} | ${group.length} | ${sample} |`;
    }),
    '',
    '## 显式标记的待补全项',
    '',
    '遵循 [模拟与游戏数据规范](simulation-standards.md)，本阶段不以零值或估算值静默补齐以下两类数据，均在导出结构的 `unresolved` 字段显式声明，留待第二步/第三步按 `Config/ConfigAbility/Monster/*.json` 与实测对照补齐：',
    '',
    '- **多段攻击分拆与精确倍率公式**：`MonsterSkillConfig.ParamList` 已完整导出，但部分技能的单段倍率分配与 `SPHitRatio` 需从对应 `ConfigAbility/Monster/*.json` 的 `DamageByAttackProperty` 核对。',
    '- **敌方异常状态的基础概率、持续回合与驱散规则**：状态字典已锁定 `MonsterStatusConfig.json`，具体挂钩时机与数值需在对应敌人规则接入时逐项核对。',
    '',
  ];
  return lines.join('\n');
}

async function verifyAndLoadTables(turnbasedDir) {
  const sourceLock = await readLock();
  const source = sourceLock.sources.find(
    ({ directory }) => directory === 'turnbased',
  );
  check(
    source?.revision === REVISION,
    'enemy combat and character source revisions differ',
  );
  const textMapBytes = await readFile(join(turnbasedDir, 'TextMapCHS.json'));
  check(
    sha256(textMapBytes) === source.files['TextMap/TextMapCHS.json'],
    'invalid TextMapCHS.json',
  );
  const tables = Object.fromEntries(
    await Promise.all(
      Object.entries(BLOBS).map(async ([name, expectedSha1]) => {
        const bytes = await readFile(join(turnbasedDir, `${name}.json`));
        const gitBlobHash = createHash('sha1')
          .update(`blob ${bytes.length}\0`)
          .update(bytes)
          .digest('hex');
        check(gitBlobHash === expectedSha1, `wrong revision for ${name}.json`);
        return [name, parseJsonPreservingHashes(bytes.toString('utf8'))];
      }),
    ),
  );

  const aiLock = await readEnemyAiLock();
  check(aiLock.revision === REVISION, `wrong enemy-ai-lock revision`);
  let monsterSkills = null;
  let monsterStatuses = null;
  const aiConfigs = new Map();
  const charConfigs = new Map();

  for (const [repoPath, expectedSha256] of Object.entries(aiLock.files)) {
    const bytes = await readFile(localFilePath(turnbasedDir, repoPath));
    check(sha256(bytes) === expectedSha256, `SHA-256 mismatch for ${repoPath}`);
    const parsed = parseJsonPreservingHashes(bytes.toString('utf8'));
    if (repoPath === 'ExcelOutput/MonsterSkillConfig.json') {
      monsterSkills = parsed;
    } else if (repoPath === 'ExcelOutput/MonsterStatusConfig.json') {
      monsterStatuses = parsed;
    } else if (repoPath.startsWith('Config/ConfigAI/')) {
      aiConfigs.set(repoPath, parsed);
    } else if (repoPath.startsWith('Config/ConfigCharacter/')) {
      charConfigs.set(repoPath, parsed);
    }
  }
  check(monsterSkills && monsterStatuses, 'missing skill or status tables');

  return {
    tables,
    textMap: JSON.parse(textMapBytes.toString('utf8')),
    monsterSkills,
    monsterStatuses,
    aiConfigs,
    charConfigs,
  };
}

async function writeFormatted(filepath, content) {
  await mkdir(dirname(filepath), { recursive: true });
  const config = await resolveConfig(filepath);
  await writeFile(filepath, await format(content, { ...config, filepath }));
}

async function main() {
  const [turnbasedDir = '.m03-data/turnbased', ...extra] =
    process.argv.slice(2);
  check(
    extra.length === 0,
    'Usage: node scripts/m03/import-enemy-combat.mjs [turnbased-dir]',
  );
  const loaded = await verifyAndLoadTables(turnbasedDir);
  const fullCatalog = buildEnemyCombatCatalog(loaded);
  const compactCatalog = stripDescriptionsForCompactCatalog(fullCatalog);

  const fullPath = fileURLToPath(
    new URL('../../.m03-data/4.7.0/enemy-combat-full.json', import.meta.url),
  );
  const compactPath = fileURLToPath(
    new URL(
      '../../crates/battle-core/data/enemy-combat-4.7.0.json',
      import.meta.url,
    ),
  );
  const auditDocPath = fileURLToPath(
    new URL('../../docs/enemy-ai-audit.md', import.meta.url),
  );

  await mkdir(dirname(fullPath), { recursive: true });
  await writeFile(fullPath, `${JSON.stringify(fullCatalog, null, 2)}\n`);
  await writeFormatted(compactPath, JSON.stringify(compactCatalog, null, 2));
  await writeFormatted(auditDocPath, formatAuditMarkdown(fullCatalog));

  console.log(
    JSON.stringify(
      {
        enemyCount: fullCatalog.summary.enemyCount,
        byTier: fullCatalog.summary.byTier,
        byTierAndRank: fullCatalog.summary.byTierAndRank,
        totalSkills: fullCatalog.summary.totalSkills,
      },
      null,
      2,
    ),
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
