import { params } from './characters.mjs';
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

const slots = {
  HEAD: 'head',
  HAND: 'hands',
  BODY: 'body',
  FOOT: 'feet',
  NECK: 'planar_sphere',
  OBJECT: 'link_rope',
};
const initialSubstats = { 2: [0, 0], 3: [1, 2], 4: [2, 3], 5: [3, 4] };
const coneStats = {
  hp: ['BaseHP', 'BaseHPAdd'],
  atk: ['BaseAttack', 'BaseAttackAdd'],
  def: ['BaseDefence', 'BaseDefenceAdd'],
};

export function buildEquipment(
  compact,
  native,
  toPath,
  propertyId,
  propertyBonuses,
) {
  const cones = {};
  const releasedCones = index(
    native.EquipmentConfig.filter((row) => row.Release),
    'EquipmentID',
    'released light cones',
  );
  sameIds(
    new Set(Object.keys(compact.light_cones)),
    new Set(releasedCones.keys()),
    'light cone roster',
  );
  const ranks = index(
    native.EquipmentSkillConfig.map((row) => ({
      ...row,
      id: `${row.SkillID}.${row.Level}`,
    })),
    'id',
    'EquipmentSkillConfig',
  );
  const promotions = promotionIndex(
    native.EquipmentPromotionConfig,
    'EquipmentID',
    'EquipmentPromotionConfig',
  );
  for (const [id, source] of releasedCones) {
    const cone = lookup(compact.light_cones, id, 'light_cones');
    const effect = lookup(compact.light_cone_ranks, id, 'light_cone_ranks');
    check(
      cone.path === source.AvatarBaseType &&
        source.MaxRank === 5 &&
        effect.params.length === 5 &&
        effect.properties.length === 5,
      `light cone ${id}: expected five superimpositions and matching path`,
    );
    const rankData = effect.params.map((_, i) => ({
      parameters: params(
        lookup(ranks, `${source.SkillID}.${i + 1}`, 'EquipmentSkillConfig')
          .ParamList,
        `light cone ${id} S${i + 1}`,
      ),
      properties: propertyBonuses(
        effect.properties[i],
        `light cone ${id} S${i + 1}`,
      ),
    }));
    checkParameters(
      effect.desc,
      rankData.map((rank) => rank.parameters),
      `light cone ${id}`,
    );
    cones[`cone.${id}`] = {
      name: textValue(cone.name, `light cone ${id}`),
      rarity: cone.rarity,
      path: toPath(cone.path),
      promotions: checkedPromotions(
        lookup(compact.light_cone_promotions, id, 'light_cone_promotions')
          .values,
        promotions,
        id,
        coneStats,
      ),
      effect_name: textValue(effect.skill, `light cone ${id} effect`),
      description: textValue(effect.desc, `light cone ${id} description`),
      ranks: rankData,
    };
  }

  const relicSets = {};
  const releasedSets = index(
    native.RelicSetConfig.filter((row) => row.Release),
    'SetID',
    'released relic sets',
  );
  sameIds(
    new Set(Object.keys(compact.relic_sets)),
    new Set(releasedSets.keys()),
    'relic set roster',
  );
  for (const [id, row] of releasedSets) {
    const set = lookup(compact.relic_sets, id, 'relic_sets');
    check(
      row.SetSkillList.length === set.desc.length &&
        set.desc.length === set.properties.length,
      `relic set ${id}: effects mismatch`,
    );
    relicSets[`set.${id}`] = {
      name: textValue(set.name, `relic set ${id}`),
      effects: Object.fromEntries(
        row.SetSkillList.map((count, i) => [
          count,
          textValue(set.desc[i], `relic set ${id} ${count}-piece`),
        ]),
      ),
      properties: Object.fromEntries(
        row.SetSkillList.map((count, i) => [
          count,
          propertyBonuses(set.properties[i], `relic set ${id}`),
        ]),
      ),
    };
  }

  const main = affixGroups(
    native.RelicMainAffixConfig,
    propertyId,
    (row) => row.LevelAdd.Value ?? 0,
  );
  const sub = affixGroups(
    native.RelicSubAffixConfig,
    propertyId,
    (row) => row.StepValue.Value ?? 0,
  );
  const relics = {};
  const releasedRelics = index(
    native.RelicConfig.filter((row) => releasedSets.has(String(row.SetID))),
    'ID',
    'released relics',
  );
  sameIds(
    new Set(Object.keys(compact.relics)),
    new Set(releasedRelics.keys()),
    'relic roster',
  );
  for (const [id, row] of releasedRelics) {
    const relic = lookup(compact.relics, id, 'relics');
    check(
      relic.set_id === String(row.SetID) &&
        relic.type === row.Type &&
        relic.max_level === row.MaxLevel &&
        relic.main_affix_id === String(row.MainAffixGroup) &&
        relic.sub_affix_id === String(row.SubAffixGroup),
      `relic ${id}: slot, level, set or affix mismatch`,
    );
    const mainGroup = `main.${relic.main_affix_id}`;
    const subGroup = `sub.${relic.sub_affix_id}`;
    lookup(main, mainGroup, `relic ${id} main group`);
    lookup(sub, subGroup, `relic ${id} sub group`);
    relics[`relic.${id}`] = {
      name: textValue(relic.name, `relic ${id}`),
      slot: lookup(slots, relic.type, `relic ${id} slot`),
      rarity: relic.rarity,
      set_id: `set.${relic.set_id}`,
      max_level: relic.max_level,
      main_group: mainGroup,
      sub_group: subGroup,
      initial_substats: lookup(
        initialSubstats,
        relic.rarity,
        `relic ${id} rarity`,
      ),
      upgrade_interval: 3,
    };
  }

  return {
    light_cones: cones,
    relics,
    relic_sets: relicSets,
    main_affixes: main,
    sub_affixes: sub,
  };
}

function affixGroups(rows, propertyId, step) {
  const result = {};
  for (const row of rows) {
    const label = `affix ${row.GroupID}.${row.AffixID}`;
    const id = propertyId(row.Property);
    const group = `${row.StepNum === undefined ? 'main' : 'sub'}.${row.GroupID}`;
    const affixes = (result[group] ??= {});
    check(!Object.hasOwn(affixes, id), `${label}: duplicate property ${id}`);
    affixes[id] = {
      base: numberValue(row.BaseValue.Value, label),
      step: numberValue(step(row), label),
      step_count: row.StepNum ?? 0,
    };
  }
  return result;
}
