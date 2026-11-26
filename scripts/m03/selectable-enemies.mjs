import { check, lookup, textValue } from './sources.mjs';

const BOSS_RANKS = new Set(['LittleBoss', 'BigBoss']);
const MINION_RANKS = new Set(['Minion', 'MinionLv2']);
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

function isCombatantEntity(monster, template) {
  // Exclude background encounter controllers / gimmick mascots with no toughness bar,
  // as well as gimmick minions without fixed weaknesses (e.g. Royal Trashcan, Spirithief).
  if (!((template.StanceBase?.Value ?? 0) > 0)) return false;
  if (MINION_RANKS.has(template.Rank) && monster.StanceWeakList.length === 0) {
    return false;
  }
  return true;
}

function isApocalypticShadowStage(stage) {
  return (
    stage.StageType === 'Challenge' &&
    Number.isInteger(stage.StageID) &&
    Math.floor(stage.StageID / 1000) === 420
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

function collectChallengeOccurrences(tables, entries, summoners) {
  // Only enemies that appear directly in endgame challenge modes
  // (Memory of Chaos, Pure Fiction, Apocalyptic Shadow, Anomaly Arbitration)
  // belong in the combat simulator roster.
  const direct = new Set();
  const modes = new Map();
  for (const stage of [...tables.StageConfig, ...tables.StageConfigLD]) {
    if (stage.Release === false || stage.StageType !== 'Challenge') continue;
    const mode = isApocalypticShadowStage(stage) ? 'apocalyptic' : 'standard';
    for (const wave of stage.MonsterList) {
      const units = Object.values(wave)
        .map((id) => entries.get(id))
        .filter(Boolean);
      const names = new Set(units.map(({ key }) => key));
      for (const { monster, key } of units) {
        const owners = summoners.get(key);
        if (!owners || ![...owners].some((owner) => names.has(owner))) {
          direct.add(monster.MonsterID);
          if (!modes.has(monster.MonsterID))
            modes.set(monster.MonsterID, new Set());
          modes.get(monster.MonsterID).add(mode);
        }
      }
    }
  }
  return { direct, modes };
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

function familyModes(candidates, modes) {
  const result = new Set();
  for (const { monster } of candidates) {
    const entryModes = modes.get(monster.MonsterID);
    if (!entryModes) continue;
    for (const mode of entryModes) result.add(mode);
  }
  return result;
}

function classifyTemplate(template, modes, atlasPrimaryGroups, atlasIcons) {
  if (isCanonicalAtlasTemplate(template, atlasPrimaryGroups)) {
    return { keep: true, variant: null };
  }
  if (
    BOSS_RANKS.has(template.Rank) &&
    modes.size === 1 &&
    modes.has('apocalyptic')
  ) {
    return { keep: true, variant: '末日幻影' };
  }
  // Keep standalone endgame bosses that have their own template group and art
  // even when not listed in the atlas (e.g. Anomaly Arbitration bosses).
  if (
    BOSS_RANKS.has(template.Rank) &&
    template.TemplateGroupID === template.MonsterTemplateID &&
    modes.has('standard') &&
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
  const { direct, modes } = collectChallengeOccurrences(
    tables,
    entries,
    summoners,
  );
  const lineup = [];
  for (const [, family] of families) {
    // Argenti's totems are auxiliary targets even in pools that preload them.
    // MonsterConfig entries that rename a template are story/event reskins.
    const candidates = family.filter(
      ({ monster, template, matchesTemplateName }) =>
        matchesTemplateName &&
        !monster.CustomValueTags.includes('Argenti_Totem') &&
        isCombatantEntity(monster, template),
    );
    if (!candidates.length) continue;

    if (!candidates.some(({ monster }) => direct.has(monster.MonsterID))) {
      continue;
    }

    const challengeModes = familyModes(candidates, modes);
    const supported = new Map();
    for (const { template } of candidates) {
      const result = classifyTemplate(
        template,
        challengeModes,
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
