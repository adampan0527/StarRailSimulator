import { invoke } from '@tauri-apps/api/core';
import { readRelicExportFile } from '../relic-export/client';
import { describeError, errorCode } from '../training/client';
import type {
  CharacterStats,
  CharacterSummary,
  TeamSlots,
} from '../training/client';

export const outerRelicSlots = [
  ['head', '头部'],
  ['hands', '手部'],
  ['body', '躯干'],
  ['feet', '脚部'],
] as const;

export const innerRelicSlots = [
  ['planar_sphere', '位面球'],
  ['link_rope', '连结绳'],
] as const;

export const relicSlots = [...outerRelicSlots, ...innerRelicSlots] as const;

export type RelicSlot = (typeof relicSlots)[number][0];
export type Relic = {
  data_id: string;
  level: number;
  main_stat: string;
  substats: Record<string, number>;
};
export type RelicSummary = {
  id: string;
  name: string;
  slot: RelicSlot;
  rarity: number;
  max_level: number;
  set_id: string;
  main_stats: string[];
};
export type LightCone = {
  data_id: string;
  level: number;
  ascension: number;
  superimposition: number;
};
export type LightConeSummary = {
  name: string;
  path: string;
  rarity: number;
  default: LightCone;
};
export type LightConeEquipment =
  | { source: 'custom'; value: LightCone }
  | { source: 'inventory'; value: number };
export type InventoryLightCone = {
  id: number;
  cone: LightCone;
  owner_id: string | null;
};
export type SavedTeam = { id: string; name: string; slots: TeamSlots };
export type Catalog = {
  game_version: string;
  teams: SavedTeam[];
  characters: CharacterSummary[];
  light_cones: LightConeSummary[];
  relics: RelicSummary[];
  sets: Record<string, { name: string }>;
  properties: Record<string, string>;
};
export type InventoryItem = {
  id: number;
  relic: Relic;
  owner_id: string | null;
};
export type RelicOptions = {
  main_stats: Record<string, number>;
  substats: Record<string, number[]>;
  min_lines: number;
  max_lines: number;
};
export type CharacterEdit = {
  eidolon: number;
  affinitive_eidolon: boolean;
  light_cone: LightConeEquipment | null;
  relics: Partial<Record<RelicSlot, number>>;
};
type Eidolon = { name: string; description: string };
export type CharacterProfile = {
  level: number;
  eidolons: Eidolon[];
  special_eidolon: Eidolon | null;
  edit: CharacterEdit;
  stats: CharacterStats;
  trace_bonuses: Record<string, number>;
};

export const characters = {
  index: () => invoke<Catalog>('character_index'),
  roster: () => invoke<CharacterSummary[]>('character_roster'),
  importCatalog: (json: string) =>
    invoke<Catalog>('character_import_catalog', { json }),
  profile: (characterId: string) =>
    invoke<CharacterProfile>('character_profile', { characterId }),
  save: (characterId: string, edit: CharacterEdit) =>
    invoke<[CharacterProfile, InventoryItem[], InventoryLightCone[]]>(
      'character_save',
      {
        characterId,
        edit,
      },
    ),
  saveTeam: (id: string | null, name: string, slots: TeamSlots) =>
    invoke<SavedTeam>('character_save_team', { id, name, slots }),
  inventory: () => invoke<InventoryItem[]>('relic_inventory'),
  importRelics: async (file: File) =>
    invoke<[number, InventoryItem[]]>('relic_inventory_import', {
      snapshot: await readRelicExportFile(file),
    }),
  lightConeInventory: () =>
    invoke<InventoryLightCone[]>('light_cone_inventory'),
  saveLightCone: (id: number | null, cone: LightCone) =>
    invoke<InventoryLightCone[]>('light_cone_save', { id, cone }),
  deleteLightCone: (id: number) =>
    invoke<InventoryLightCone[]>('light_cone_delete', { id }),
  saveRelic: (id: number | null, relic: Relic) =>
    invoke<InventoryItem[]>('relic_save', { id, relic }),
  deleteRelic: (id: number) => invoke<InventoryItem[]>('relic_delete', { id }),
  relicOptions: (relicId: string, level: number) =>
    invoke<RelicOptions>('character_relic_options', { relicId, level }),
};

export function catalogError(error: unknown) {
  switch (errorCode(error)) {
    case 'catalog_missing':
      return '尚未载入正式服数据。';
    case 'catalog_invalid':
      return '数据文件无效。请选择项目生成的 catalog.json。';
    case 'catalog_mismatch':
      return '当前存档需要原来的游戏数据版本。请载入对应的数据文件。';
    case 'invalid_eidolon':
      return '请检查星魂等级和奇缘星魂。';
    case 'invalid_relic':
      return '遗器无法保存。请检查部位、强化等级和词条组合。';
    case 'invalid_relic_snapshot':
      return '遗器 JSON 缺少库存字段、格式不完整或超过 8 MB。';
    case 'unsupported_relic_snapshot':
      return '请选择 Archiver 第 4 版导出文件，或模拟器保存的第 1 版遗器 JSON。';
    case 'invalid_light_cone':
      return '光锥无法保存。请检查光锥、等级、突破和叠影。';
    case 'light_cone_occupied':
      return '该光锥已由另一名角色装备。请选择光锥库中的其他实例。';
    case 'light_cone_equipped':
      return '请先在角色页卸下这件光锥。';
    case 'relic_occupied':
      return '该遗器已由另一名角色穿戴。请刷新列表后重新选择。';
    case 'relic_equipped':
      return '请先在角色页卸下该遗器。';
    case 'invalid_profile':
      return '找不到所选角色或遗器，请刷新后重试。';
    case 'invalid_team_name':
      return '配队名称需填写 1–48 个字符。';
    case 'invalid_team':
      return '配队已变更或站位有重复角色，请检查后重试。';
    default:
      return describeError(error);
  }
}

const flatStats = new Set(['hp_flat', 'attack_flat', 'defense_flat', 'speed']);
export const statName = (catalog: Catalog, id: string) =>
  catalog.properties[id] ?? id;
export function relicStatName(catalog: Catalog, id: string) {
  const name = statName(catalog, id);
  return ['hp_ratio', 'attack_ratio', 'defense_ratio', 'speed_ratio'].includes(
    id,
  )
    ? `${name}百分比`
    : name;
}
export const statValue = (id: string, value: number) =>
  flatStats.has(id)
    ? String(Number(value.toFixed(6)))
    : `${Number((value * 100).toFixed(4))}%`;
