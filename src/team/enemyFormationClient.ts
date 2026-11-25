import { invoke } from '@tauri-apps/api/core';

export type EnemySlot = { enemy_id: string; level: number };
export type EnemySlots = [
  EnemySlot | null,
  EnemySlot | null,
  EnemySlot | null,
  EnemySlot | null,
  EnemySlot | null,
];

export type EnemyFormationDraft = { name: string; slots: EnemySlots };
export type EnemyFormation = EnemyFormationDraft & { id: number };

export const enemyFormations = {
  list: () => invoke<EnemyFormation[]>('enemy_formations'),
  save: (id: number | null, draft: EnemyFormationDraft) =>
    invoke<EnemyFormation>('enemy_formation_save', { id, draft }),
};

export function enemyFormationError(error: unknown) {
  const code =
    typeof error === 'object' && error !== null && 'code' in error
      ? error.code
      : undefined;
  switch (code) {
    case 'invalid_enemy_formation':
      return '请检查编队名称、敌人和等级。';
    case 'enemy_formation_missing':
      return '这套编队找不到了，请重新读取。';
    case 'incompatible_version':
      return '当前版本无法读取这份敌方编队存档。';
    default:
      return '无法读取或保存本地敌方编队，请重试。';
  }
}
