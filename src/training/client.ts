import { invoke } from '@tauri-apps/api/core';

export type Equipment = {
  light_cone_equipped: boolean;
  boots_speed_micros: number | null;
};

export type Preset = { name: string; equipment: Equipment };
export type Name = { id: string; name: string };
export type Profile = Name & {
  level: number;
  can_equip_light_cone: boolean;
  current_preset: string;
  presets: Preset[];
};
export type Team = Name & { character_form_ids: string[] };
export type Setup = {
  profiles: Profile[];
  team: Team;
  enemies: Name[];
  boots_speed_tiers_micros: number[];
};

export type Action = 'basic' | 'skill' | 'ultimate';
export type Outcome = 'ongoing' | 'victory' | 'defeat' | 'turn_limit';
export type Command = {
  decision_index: number;
  actor_id: string;
  action: Action;
  target_id: string | null;
};

export type Unit = {
  unit_id: string;
  is_ally: boolean;
  hp: number;
  max_hp: number;
  energy_milli: number;
  max_energy_milli: number;
  toughness: number;
  max_toughness: number;
  speed_micros: number;
};

type Status = 'shield' | 'vulnerability' | 'speed_up';
type EventKind =
  | {
      event:
        | 'wave_started'
        | 'turn_started'
        | 'enemy_attack'
        | 'weakness_broken'
        | 'shield_depleted'
        | 'unit_defeated';
    }
  | { event: 'action'; detail: Action }
  | { event: 'battle_ended'; detail: Outcome }
  | {
      event: 'skill_points' | 'toughness';
      detail: { before: number; after: number };
    }
  | { event: 'energy'; detail: { before_milli: number; after_milli: number } }
  | {
      event: 'damage';
      detail: {
        kind: 'direct' | 'weakness_break';
        amount: number;
        hp_loss: number;
        absorbed: number;
        critical: boolean;
      };
    }
  | { event: 'heal'; detail: { amount: number } }
  | {
      event: 'gauge_shifted';
      detail: { before_quanta: number; after_quanta: number };
    }
  | { event: 'status_applied' | 'status_expired'; detail: Status };

export type BattleEvent = {
  av_micros: number;
  wave_index: number;
  source_id: string | null;
  target_id: string | null;
  kind: EventKind;
};

export type BattleView = {
  id: number;
  snapshot: {
    outcome: Outcome;
    av_micros: number;
    decision_index: number;
    ordinary_actor_id: string | null;
    skill_points: number;
    enemy_hp_lost: number;
    units: Unit[];
  };
  order: { unit_id: string; remaining_av_micros: number }[];
  legal_actions: Command[];
  events: BattleEvent[];
  names: Name[];
};

export type ReplaySummary = {
  id: number;
  started_at: number;
  finished: boolean;
};

export const training = {
  setup: () => invoke<Setup>('training_setup'),
  saveLoadout: (
    characterId: string,
    presetName: string,
    equipment: Equipment,
  ) =>
    invoke<Profile>('training_save_loadout', {
      characterId,
      presetName,
      equipment,
    }),
  selectLoadout: (characterId: string, presetName: string) =>
    invoke<Profile>('training_select_loadout', { characterId, presetName }),
  saveTeam: (characterIds: string[]) =>
    invoke<Team>('training_save_team', { characterIds }),
  start: (enemyIds: string[]) =>
    invoke<BattleView>('training_start', { enemyIds }),
  act: (sessionId: number, command: Command) =>
    invoke<BattleView>('training_act', { sessionId, command }),
  replays: () => invoke<ReplaySummary[]>('training_replays'),
  replay: (sessionId: number) =>
    invoke<BattleView>('training_replay', { sessionId }),
};

export function errorCode(error: unknown) {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    return error.code;
  }
}

export function describeError(error: unknown) {
  switch (errorCode(error)) {
    case 'invalid_loadout':
      return '配装未保存。请检查方案名称和训练装备。';
    case 'invalid_team':
      return '队伍需要 1 到 4 名不同角色。';
    case 'invalid_enemies':
      return '请选择至少一台训练靶机。';
    case 'replay_not_found':
      return '找不到这场战斗的记录。';
    case 'incompatible_version':
      return '当前版本无法读取这份存档或重算回放。';
    case 'stale_action':
      return '战斗已推进，请按当前状态重新选择指令。';
    case 'invalid_action':
      return '这条指令当前不可用，请重新选择。';
    case 'storage_failed':
      return '无法读取或写入本地存档，请重试。';
    default:
      return '无法调用后端，请在桌面应用中重试。';
  }
}
