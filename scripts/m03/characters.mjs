import xxhash from 'xxhash-wasm';
import { displaySkills } from './patches.mjs';
import {
  check,
  checkParameters,
  checkedPromotions,
  index,
  lookup,
  numberValue,
  promotionIndex,
  sameIds,
  textValue,
} from './sources.mjs';

const stableIds = {
  1001: 'march_7th.preservation',
  1002: 'dan_heng.hunt',
  1105: 'natasha.abundance',
  1224: 'march_7th.hunt',
  8001: 'trailblazer.destruction',
  8003: 'trailblazer.preservation',
  8005: 'trailblazer.harmony',
  8007: 'trailblazer.remembrance',
  8009: 'trailblazer.elation',
};

const specialResources = {
  1220: 'flying_aureus',
  1308: 'slashed_dream',
  1407: 'newbud',
  1408: 'coreflame',
  1415: 'recollection',
  1511: 'wish_power',
};

const skillId = (id) => `skill.${id}`;
const traceId = (id) => `trace.${id}`;
const jsonEqual = (left, right) =>
  JSON.stringify(left) === JSON.stringify(right);
const characterStats = {
  hp: ['HPBase', 'HPAdd'],
  atk: ['AttackBase', 'AttackAdd'],
  def: ['DefenceBase', 'DefenceAdd'],
  spd: ['SpeedBase'],
  taunt: ['BaseAggro'],
  crit_rate: ['CriticalChance'],
  crit_dmg: ['CriticalDamage'],
};

export function params(rows, label) {
  return rows.map((row) => {
    // The exporter emits an empty value object for explicit zeroes.
    const value =
      typeof row === 'number'
        ? row
        : Object.keys(row).length === 0
          ? 0
          : row.Value;
    return numberValue(value, label, -Infinity);
  });
}

function skillRows(tables) {
  const result = new Map();
  for (const row of tables.flat()) {
    const levels = result.get(String(row.SkillID)) ?? new Map();
    check(
      !levels.has(row.Level),
      `skill ${row.SkillID}: duplicate level ${row.Level}`,
    );
    levels.set(row.Level, row);
    result.set(String(row.SkillID), levels);
  }
  return result;
}

function requireRanks(ids, nativeRanks, source, skills) {
  check(
    ids.length === 6,
    `rank group ${ids[0]}: expected six ordinary eidolons`,
  );
  return ids.map((id, i) => {
    const rank = lookup(source, id, 'character_ranks');
    const raw = lookup(nativeRanks, id, 'AvatarRankConfig');
    check(
      rank.rank === i + 1 && raw.Rank === rank.rank,
      `rank ${id}: invalid position`,
    );
    const bonuses = Object.fromEntries(
      Object.entries(raw.SkillAddLevelList).map(([skill, bonus]) => {
        check(
          Object.hasOwn(skills, skillId(skill)),
          `rank ${id}: unregistered bonus skill ${skill}`,
        );
        check(
          Number.isInteger(bonus) && bonus > 0,
          `rank ${id}: invalid skill bonus for ${skill}`,
        );
        return [skillId(skill), bonus];
      }),
    );
    return {
      id: `eidolon.${id}`,
      name: textValue(rank.name, `rank ${id}`),
      description: textValue(rank.desc, `rank ${id}`),
      skill_bonuses: bonuses,
      parameters: params(raw.Param, `rank ${id}`),
    };
  });
}

function buildVariant(avatar, refs, tables, propertyBonuses, label) {
  const { compact, skillLevels, rankRows, innate, rawTrees } = tables;
  const knownSkills = new Set(refs.skills);
  const upgrades = new Map();
  const traces = {};
  const tracePoints = refs.traces.map((id) =>
    lookup(compact.character_skill_trees, id, 'character_skill_trees'),
  );
  const pointsById = index(tracePoints, 'id', label);
  const progressPoints = new Set();

  for (const point of tracePoints) {
    check(
      point.levels.length === point.max_level,
      `trace ${point.id}: level count mismatch`,
    );
    for (const link of point.level_up_skills) {
      knownSkills.add(link.id);
    }
    if (point.max_level > 1) {
      check(
        point.level_up_skills.length > 0,
        `trace ${point.id}: missing skill progression links`,
      );
      const [primary, ...linked] = point.level_up_skills;
      const requirements = point.levels.map((row) => ({
        level: row.level,
        ascension: row.promotion,
      }));
      for (const target of [primary, ...linked]) {
        check(
          target.num === 1 && !upgrades.has(target.id),
          `trace ${point.id}: invalid progression for ${target.id}`,
        );
        upgrades.set(
          target.id,
          target === primary
            ? { requirements }
            : { source: skillId(primary.id) },
        );
      }
      progressPoints.add(point.id);
    }
  }

  for (const point of tracePoints) {
    if (progressPoints.has(point.id)) continue;
    const row = point.levels[0];
    const grant = point.level_up_skills[0]?.id;
    const raw = lookup(rawTrees, point.id, 'AvatarSkillTreeConfig');
    if (!point.name && raw.DefaultUnlock && grant) continue;
    const automatic = innate.has(grant) || Boolean(raw.DefaultUnlock);
    const prerequisites = point.pre_points.filter((parent) => {
      const previous = lookup(
        pointsById,
        parent,
        `trace ${point.id} prerequisite`,
      );
      return (
        !progressPoints.has(parent) &&
        !(
          !previous.name &&
          lookup(rawTrees, parent, 'AvatarSkillTreeConfig').DefaultUnlock
        )
      );
    });
    const parameters = point.params.map((rank) =>
      params(rank, `trace ${point.id}`),
    );
    checkParameters(point.desc, parameters, `trace ${point.id}`);
    traces[traceId(point.id)] = {
      name: textValue(point.name, `trace ${point.id}`),
      description: point.desc,
      requirement: { level: row.level, ascension: row.promotion },
      prerequisites: prerequisites.map(traceId),
      properties: propertyBonuses(row.properties, `trace ${point.id}`),
      parameters,
      automatic,
    };
  }

  const skills = {};
  for (const id of [...knownSkills].sort()) {
    const skill = lookup(compact.character_skills, id, 'character_skills');
    const raw = lookup(skillLevels, id, 'AvatarSkillConfig');
    check(
      Number.isInteger(skill.max_level) && skill.max_level > 0,
      `skill ${id}: invalid maximum level`,
    );
    const levels = Array.from({ length: skill.max_level }, (_, i) =>
      lookup(raw, i + 1, `skill ${id} level`),
    );
    const paired = displaySkills[id];
    const pairProgression = paired ? upgrades.get(paired) : undefined;
    const progression =
      upgrades.get(id) ??
      (pairProgression?.requirements
        ? { source: skillId(paired) }
        : pairProgression);
    const rawTalent = levels[0];
    const effects = innate.get(id) ?? [];
    const parameters = levels.map((level) =>
      params(level.ParamList, `skill ${id} level ${level.Level}`),
    );
    const description = paired ? '' : skill.desc;
    checkParameters(description, parameters, `skill ${id}`);
    skills[skillId(id)] = {
      name: skill.name,
      kind: textValue(skill.type, `skill ${id} kind`),
      description,
      parameters,
      upgrade_requirements: progression?.requirements ?? [],
      level_source: progression?.source ?? null,
      conditional_levels: effects.map((effect) => {
        const [source, minimum] = String(effect.UnlockCondition.Param).split(
          '.',
        );
        return {
          level: effect.Level,
          kind: effect.UnlockCondition.Type,
          character_id: stableIds[source] ?? `character.${source}`,
          minimum: Number(minimum),
        };
      }),
      hidden:
        Boolean(paired) ||
        !skill.name ||
        (!description && rawTalent.SkillTriggerKey === ''),
      display_skill_id: paired ? skillId(paired) : null,
    };
  }
  return {
    skills,
    traces,
    eidolons: requireRanks(
      avatar.RankIDList,
      rankRows,
      compact.character_ranks,
      skills,
    ),
    servant: null,
  };
}

function compareGender(male, female, compact, skills, config) {
  const prefix = (id) => (ref) =>
    ref.replace(new RegExp(`^(1?)${id}`), '$1avatar');
  const normalMale = prefix(male.id);
  const normalFemale = prefix(female.id);
  const skillSignature = (id) => {
    const levels = lookup(skills, id, 'Trailblazer skill levels');
    const first = levels.get(1);
    return [
      first.AttackType,
      [...levels.values()].map((level) => level.ParamList),
    ];
  };
  const bases = (avatar) => avatar.SkillList.map(String);
  const aliases =
    config.get(male.id).BaseAvatarID === config.get(female.id).BaseAvatarID;
  check(
    aliases &&
      male.element === female.element &&
      male.path === female.path &&
      male.max_sp === female.max_sp &&
      jsonEqual(
        compact.character_promotions[male.id].values,
        compact.character_promotions[female.id].values,
      ) &&
      jsonEqual(
        bases(male.raw).map(skillSignature),
        bases(female.raw).map(skillSignature),
      ) &&
      jsonEqual(
        male.skill_trees.map(normalMale),
        female.skill_trees.map(normalFemale),
      ),
    `Trailblazer aliases ${male.id} / ${female.id} differ`,
  );
}

export async function buildCharacters(
  compact,
  native,
  toPath,
  toElement,
  propertyBonuses,
) {
  const released = index(
    [...native.AvatarConfig, ...native.AvatarConfigLD].filter(
      (row) => row.Release,
    ),
    'AvatarID',
    'released characters',
  );
  sameIds(
    new Set(Object.keys(compact.characters)),
    new Set(released.keys()),
    'character release roster',
  );
  const enhanced = index(
    native.AvatarConfigEnhanced,
    'AvatarID',
    'AvatarConfigEnhanced',
  );
  const shapes = index(
    native.MultiplePathAvatarConfig,
    'AvatarID',
    'MultiplePathAvatarConfig',
  );
  const servants = index(
    native.AvatarServantConfig,
    'ServantID',
    'AvatarServantConfig',
  );
  const promotions = promotionIndex(
    [...native.AvatarPromotionConfig, ...native.AvatarPromotionConfigLD],
    'AvatarID',
    'AvatarPromotionConfig',
  );
  const skillLevels = skillRows([
    native.AvatarSkillConfig,
    native.AvatarSkillConfigLD,
    native.AvatarServantSkillConfig,
  ]);
  const rankRows = index(
    [...native.AvatarRankConfig, ...native.AvatarRankConfigLD],
    'RankID',
    'character rank rows',
  );
  const rawTrees = index(
    [...native.AvatarSkillTreeConfig, ...native.AvatarSkillTreeConfigLD].filter(
      (row) => row.Level === 1,
    ),
    'PointID',
    'character trace rows',
  );
  const innate = new Map();
  for (const row of native.AvatarSeriesSkill) {
    const key = String(row.SkillID);
    const alternatives = innate.get(key) ?? [];
    check(
      !alternatives.some((item) => item.Level === row.Level),
      `innate skill ${key}: duplicate level`,
    );
    alternatives.push(row);
    innate.set(key, alternatives);
  }
  // The 4.7.0 source uses obfuscated column names for these three entries.
  const special = index(
    native.AvatarExRankConfig,
    'ACCJKGEKHKP',
    'AvatarExRankConfig',
  );
  const { h64 } = await xxhash();
  const text = (key) => textValue(native.TextMapCHS[h64(key).toString()], key);
  const textHash = (key) =>
    textValue(native.TextMapCHS[key.Hash], String(key.Hash));
  const data = {};
  const roster = [];
  for (const [id, raw] of released) {
    const avatar = lookup(compact.characters, id, 'characters');
    const capacity = raw.SPNeed?.Value ?? null;
    check(
      avatar.id === id &&
        avatar.path === raw.AvatarBaseType &&
        avatar.element === raw.DamageType &&
        avatar.max_sp === capacity,
      `character ${id}: source identity, path, element or resource mismatch`,
    );
    if (id.startsWith('8') && Number(id) % 2 === 0) continue;
    const sourceIds = [id];
    if (id.startsWith('8')) {
      const alias = String(Number(id) + 1);
      compareGender(
        { ...avatar, raw },
        {
          ...lookup(compact.characters, alias, 'Trailblazer alias'),
          raw: released.get(alias),
        },
        compact,
        skillLevels,
        shapes,
      );
      sourceIds.push(alias);
    }
    const upgrade = enhanced.get(id);
    const upgradeSkills = new Set(upgrade?.SkillList.map(String));
    const servant = servants.get(`1${id}`);
    const initialSkills = [
      ...new Set([
        ...raw.SkillList.map(String),
        ...avatar.skills.filter((skill) => !upgradeSkills.has(skill)),
        ...(servant?.SkillIDList ?? []).map(String),
      ]),
    ];
    const initialTraces = [];
    const enhancedTraces = [];
    for (const point of avatar.skill_trees) {
      const nativePoint = lookup(rawTrees, point, 'AvatarSkillTreeConfig');
      check(nativePoint.AvatarID === Number(id), `trace ${point}: wrong owner`);
      (nativePoint.EnhancedID ? enhancedTraces : initialTraces).push(point);
    }
    const tables = { compact, skillLevels, rankRows, innate, rawTrees };
    const variants = {
      base: buildVariant(
        raw,
        { skills: initialSkills, traces: initialTraces },
        tables,
        propertyBonuses,
        id,
      ),
    };
    const servantStat = (base, inheritance, skill) => ({
      base,
      inheritance,
      skill_id: skill ? skillId(skill) : null,
    });
    if (servant) {
      variants.base.servant = {
        name: textHash(servant.ServantName),
        skill_ids: servant.SkillIDList.map(skillId),
        hp: servantStat(servant.HPBase, servant.HPInherit, servant.HPSkill),
        speed: servantStat(
          servant.SpeedBase,
          servant.SpeedInherit,
          servant.SpeedSkill,
        ),
        aggro: numberValue(
          servant.Aggro.Value,
          `servant ${servant.ServantID} aggro`,
        ),
      };
    }
    if (upgrade) {
      check(
        enhancedTraces.length > 0,
        `character ${id}: missing enhanced traces`,
      );
      variants.novaflare = buildVariant(
        upgrade,
        { skills: [...upgradeSkills], traces: enhancedTraces },
        tables,
        propertyBonuses,
        id,
      );
    }
    const extra = special.get(id);
    const specialRank = extra
      ? {
          id: `affinitive.${extra.PHFMCACHFIJ}`,
          name: text(extra.OENAMINOLLF),
          description: text(extra.NMAHGFAPENI),
          skill_bonuses: {},
          parameters: params(extra.NNACKOBKFGE, `affinitive eidolon ${id}`),
        }
      : null;
    if (specialRank)
      checkParameters(
        specialRank.description,
        [specialRank.parameters],
        `affinitive eidolon ${id}`,
      );
    const formId = stableIds[id] ?? `character.${id}`;
    check(!Object.hasOwn(data, formId), `duplicate form ID ${formId}`);
    data[formId] = {
      name: id.startsWith('8')
        ? '开拓者'
        : textValue(avatar.name, `character ${id}`),
      source_ids: sourceIds,
      rarity: avatar.rarity,
      path: toPath(avatar.path),
      element: toElement(avatar.element),
      resource: {
        kind: specialResources[id] ?? 'energy',
        capacity,
      },
      promotions: checkedPromotions(
        lookup(compact.character_promotions, id, 'character_promotions').values,
        promotions,
        id,
        characterStats,
      ),
      default_variant: upgrade ? 'novaflare' : 'base',
      variants,
      affinitive_eidolon: specialRank,
    };
    roster.push({
      id: formId,
      source_ids: sourceIds,
      variants: Object.fromEntries(
        Object.entries(variants).map(([variant, value]) => [
          variant,
          {
            skills: Object.keys(value.skills),
            traces: Object.keys(value.traces),
            eidolons: value.eidolons.map((rank) => rank.id),
          },
        ]),
      ),
      affinitive_eidolon: specialRank?.id ?? null,
    });
  }
  sameIds(
    new Set(roster.flatMap((row) => row.source_ids)),
    new Set(released.keys()),
    'character form mapping',
  );
  return { characters: data, roster };
}
