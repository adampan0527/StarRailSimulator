import { check, lookup, textValue } from './sources.mjs';

const BOSS_RANKS = new Set(['LittleBoss', 'BigBoss']);
const STANDARD_ICON = /^SpriteOutput\/MosterIcon\/Monster_\d+\.png$/;
const STANDARD_IMAGE = /^SpriteOutput\/MonsterFigure\/Monster_\d+\.png$/;

function cleanName(name) {
  return name.replaceAll(/<\/?unbreak>/g, '');
}

function normalizeName(name) {
  return name.normalize('NFKC').replace(/\s+/g, '');
}

function displayName(monster, template, textMap) {
  const name =
    textMap[monster.MonsterName.Hash]?.trim() ||
    textMap[template.MonsterName?.Hash]?.trim();
  return cleanName(textValue(name, `monster ${monster.MonsterID}`));
}

function templateDisplayName(template, textMap) {
  const name = textMap[template.MonsterName?.Hash]?.trim();
  return name ? cleanName(name) : null;
}

function findAtlasPrimaryGroups(templates) {
  const groups = new Set();
  for (const template of templates.values()) {
    if (
      template.AtlasSortID !== undefined &&
      template.TemplateGroupID === template.MonsterTemplateID
    ) {
      groups.add(template.TemplateGroupID);
    }
  }
  return groups;
}

function isCanonicalAtlasTemplate(template, atlasPrimaryGroups) {
  if (template.AtlasSortID === undefined) return false;
  // Duplicate templates copied from an atlas primary (e.g. error variants)
  // inherit AtlasSortID while pointing TemplateGroupID at the primary entry.
  return (
    template.TemplateGroupID === undefined ||
    template.TemplateGroupID === template.MonsterTemplateID ||
    !atlasPrimaryGroups.has(template.TemplateGroupID)
  );
}

function hasStandardArt(template) {
  return (
    STANDARD_ICON.test(template.IconPath ?? '') &&
    STANDARD_IMAGE.test(template.ImagePath ?? '')
  );
}

function findSummoners(entries) {
  const summoners = new Map();
  for (const { monster, key } of entries.values()) {
    for (const id of monster.SummonIDList) {
      const summoned = entries.get(id)?.key;
      if (!summoned || summoned === key) continue;
      if (!summoners.has(summoned)) summoners.set(summoned, new Set());
      summoners.get(summoned).add(key);
    }
  }
  return summoners;
}

function collectStageOccurrences(tables, entries, summoners) {
  const direct = new Set();
  const stageTypes = new Map();
  function visit(ids, stageType) {
    // Only entries in MonsterConfig participate in the catalog.
    const units = ids.map((id) => entries.get(id)).filter(Boolean);
    const names = new Set(units.map(({ key }) => key));
    for (const { monster, key } of units) {
      const owners = summoners.get(key);
      if (!owners || ![...owners].some((owner) => names.has(owner))) {
        direct.add(monster.MonsterID);
        if (stageType !== undefined) {
          if (!stageTypes.has(monster.MonsterID))
            stageTypes.set(monster.MonsterID, new Set());
          stageTypes.get(monster.MonsterID).add(stageType);
        }
      }
    }
  }
  for (const stage of [...tables.StageConfig, ...tables.StageConfigLD]) {
    if (stage.Release === false) continue;
    for (const wave of stage.MonsterList)
      visit(Object.values(wave), stage.StageType);
  }
  for (const group of tables.StageInfiniteMonsterGroup)
    visit(group.MonsterList, undefined);
  return { direct, stageTypes };
}

function canonicalOrder(a, b) {
  const isAtlas = ({ template }) => template.AtlasSortID !== undefined;
  const isBase = ({ monster }) =>
    monster.MonsterID === monster.MonsterTemplateID;
  const isDefault = ({ monster }) =>
    monster.EliteGroup === 1 &&
    monster.HPModifyRatio?.Value === 1 &&
    monster.DefenceModifyRatio?.Value === 1 &&
    monster.SpeedModifyRatio?.Value === 1;
  return (
    Number(isAtlas(b)) - Number(isAtlas(a)) ||
    Number(isBase(b)) - Number(isBase(a)) ||
    Number(isDefault(b)) - Number(isDefault(a)) ||
    a.monster.MonsterID - b.monster.MonsterID
  );
}

function familyStageTypes(candidates, stageTypes) {
  const types = new Set();
  for (const { monster } of candidates) {
    const entryTypes = stageTypes.get(monster.MonsterID);
    if (!entryTypes) continue;
    for (const type of entryTypes) types.add(type);
  }
  return types;
}

function classifyTemplate(template, types, atlasPrimaryGroups, atlasIcons) {
  if (isCanonicalAtlasTemplate(template, atlasPrimaryGroups)) {
    return { keep: true, variant: null };
  }
  if (
    BOSS_RANKS.has(template.Rank) &&
    types.size > 0 &&
    [...types].every((type) => type === 'Challenge')
  ) {
    return { keep: true, variant: '末日幻影' };
  }
  const hasIndependentGroup =
    template.TemplateGroupID === undefined ||
    template.TemplateGroupID === template.MonsterTemplateID;
  const appearsOutsideChallenge = [...types].some(
    (type) => type !== 'Challenge',
  );
  if (
    hasIndependentGroup &&
    appearsOutsideChallenge &&
    hasStandardArt(template) &&
    !atlasIcons.has(template.IconPath)
  ) {
    return { keep: true, variant: null };
  }
  return { keep: false, variant: null };
}

export function selectableEnemies(tables, monsters, templates, textMap) {
  const atlasPrimaryGroups = findAtlasPrimaryGroups(templates);
  const atlasIcons = new Set();
  for (const template of templates.values()) {
    if (
      isCanonicalAtlasTemplate(template, atlasPrimaryGroups) &&
      template.IconPath
    ) {
      atlasIcons.add(template.IconPath);
    }
  }

  const entries = new Map();
  const families = new Map();
  for (const monster of monsters.values()) {
    const id = monster.MonsterID;
    check(Number.isSafeInteger(id) && id > 0, `invalid monster ID ${id}`);
    const template = lookup(templates, monster.MonsterTemplateID, 'template');
    const name = displayName(monster, template, textMap);
    const key = normalizeName(name);
    const templateName = templateDisplayName(template, textMap);
    const matchesTemplateName =
      templateName === null || normalizeName(templateName) === key;
    const entry = {
      key,
      name,
      monster,
      template,
      matchesTemplateName,
    };
    entries.set(id, entry);
    if (!families.has(key)) families.set(key, []);
    families.get(key).push(entry);
  }

  const summoners = findSummoners(entries);
  const { direct, stageTypes } = collectStageOccurrences(
    tables,
    entries,
    summoners,
  );
  const lineup = [];
  for (const [key, family] of families) {
    // Argenti's totems are auxiliary targets even in pools that preload them.
    // MonsterConfig entries that rename a template are story/event reskins.
    const candidates = family.filter(
      ({ monster, matchesTemplateName }) =>
        matchesTemplateName &&
        !monster.CustomValueTags.includes('Argenti_Totem'),
    );
    if (!candidates.length) continue;

    const types = familyStageTypes(candidates, stageTypes);
    const supported = new Map();
    for (const { monster, template } of candidates) {
      const isDirect = direct.has(monster.MonsterID);
      const isCanonicalAtlas = isCanonicalAtlasTemplate(
        template,
        atlasPrimaryGroups,
      );
      if (!isDirect && (summoners.has(key) || !isCanonicalAtlas)) continue;
      const result = classifyTemplate(
        template,
        types,
        atlasPrimaryGroups,
        atlasIcons,
      );
      if (result.keep) supported.set(template.MonsterTemplateID, result);
    }

    const canonical = candidates
      .filter(({ template }) => supported.has(template.MonsterTemplateID))
      .sort(canonicalOrder)[0];
    if (!canonical) continue;

    const { variant } = supported.get(canonical.template.MonsterTemplateID);
    const sourceIds = candidates
      .map(({ monster }) => monster.MonsterID)
      .sort((a, b) => a - b)
      .map(String);
    lineup.push({ ...canonical, variant, sourceIds });
  }
  return lineup.sort((a, b) => a.monster.MonsterID - b.monster.MonsterID);
}
