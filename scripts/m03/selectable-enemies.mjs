import { check, lookup, textValue } from './sources.mjs';

function displayName(monster, template, textMap) {
  const name =
    textMap[monster.MonsterName.Hash]?.trim() ||
    textMap[template.MonsterName.Hash]?.trim();
  return textValue(name, `monster ${monster.MonsterID}`).replaceAll(
    /<\/?unbreak>/g,
    '',
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

function stageEntries(tables, entries, summoners) {
  const direct = new Set();
  function visit(ids) {
    // Only entries in MonsterConfig participate in the catalog.
    const units = ids.map((id) => entries.get(id)).filter(Boolean);
    const names = new Set(units.map(({ key }) => key));
    for (const { monster, key } of units) {
      const owners = summoners.get(key);
      if (!owners || ![...owners].some((owner) => names.has(owner)))
        direct.add(monster.MonsterID);
    }
  }
  for (const stage of [...tables.StageConfig, ...tables.StageConfigLD]) {
    if (stage.Release === false) continue;
    for (const wave of stage.MonsterList) visit(Object.values(wave));
  }
  for (const group of tables.StageInfiniteMonsterGroup)
    visit(group.MonsterList);
  return direct;
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

export function selectableEnemies(tables, monsters, templates, textMap) {
  const entries = new Map();
  const families = new Map();
  for (const monster of monsters.values()) {
    const id = monster.MonsterID;
    check(Number.isSafeInteger(id) && id > 0, `invalid monster ID ${id}`);
    const template = lookup(templates, monster.MonsterTemplateID, 'template');
    const name = displayName(monster, template, textMap);
    const key = name.normalize('NFKC').replace(/\s+/g, '');
    const entry = { key, name, monster, template };
    entries.set(id, entry);
    if (!families.has(key)) families.set(key, []);
    families.get(key).push(entry);
  }

  const summoners = findSummoners(entries);
  const direct = stageEntries(tables, entries, summoners);
  const lineup = [];
  for (const [key, family] of families) {
    // Argenti's totems are auxiliary targets even in pools that preload them.
    const candidates = family.filter(
      ({ monster }) => !monster.CustomValueTags.includes('Argenti_Totem'),
    );
    const supportedTemplates = new Set(
      candidates
        .filter(
          ({ monster, template }) =>
            direct.has(monster.MonsterID) ||
            (!summoners.has(key) && template.AtlasSortID !== undefined),
        )
        .map(({ template }) => template.MonsterTemplateID),
    );
    const canonical = candidates
      .filter(({ template }) =>
        supportedTemplates.has(template.MonsterTemplateID),
      )
      .sort(canonicalOrder)[0];
    if (!canonical) continue;
    const sourceIds = candidates
      .map(({ monster }) => monster.MonsterID)
      .sort((a, b) => a - b)
      .map(String);
    lineup.push({ ...canonical, sourceIds });
  }
  return lineup.sort((a, b) => a.monster.MonsterID - b.monster.MonsterID);
}
