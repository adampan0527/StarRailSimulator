import { invoke } from '@tauri-apps/api/core';
import { accountError, invokeForAccount } from './accounts';
import type { AccountSelection } from './accounts';

const endpoint = 'ws://127.0.0.1:23313/ws';
const timeoutMs = 120_000;
const maxMessageBytes = 8 * 1024 * 1024;

export type RelicExport = {
  file_url: string;
  relic_count: number;
  account: {
    uid: number | null;
    trailblazer: string | null;
    characters: number;
    light_cones: number;
    materials: number | null;
    stellar_jade: number | null;
    oneiric_shards: number | null;
    recon_relics: number | null;
  } | null;
};

export type AccountImport = {
  characters_applied: number;
  characters_preserved: number;
  characters_skipped: number;
  light_cones_added: number;
  light_cones_updated: number;
  light_cones_equipped: number;
  light_cones_skipped: number;
  relics: { added: number; equipped: number };
  relics_updated: number;
};

export type ImportedAccount = {
  uid: number | null;
  created: boolean;
  summary: AccountImport;
  selection: AccountSelection;
};

export type ExportOrigin = 'live' | 'file';

export async function openArchiver(reselect: boolean) {
  try {
    return await invoke<'opened' | 'already_open' | 'canceled'>(
      'archiver_open',
      { reselect },
    );
  } catch (cause) {
    if (cause === 'already_open') {
      throw new Error('请先关闭模拟器打开的 Archiver，再更换程序。', { cause });
    }
    if (cause === 'invalid_program') {
      throw new Error('请选择官方的 reliquary-archiver Windows 可执行文件。', {
        cause,
      });
    }
    if (cause === 'dialog_failed') {
      throw new Error('选取程序失败，请重试。', { cause });
    }
    throw new Error('无法打开 Archiver。可手动启动后再开始接收。', { cause });
  }
}

export function receiveAccount(
  signal: AbortSignal,
  onConnected: () => void,
): Promise<string> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const socket = new WebSocket(endpoint);
    let connected = false;

    function finish() {
      window.clearTimeout(timeout);
      signal.removeEventListener('abort', cancel);
      socket.onopen = null;
      socket.onmessage = null;
      socket.onclose = null;
      socket.close();
    }

    function fail(message: string) {
      finish();
      reject(new Error(message));
    }

    function cancel() {
      finish();
      reject(signal.reason);
    }

    const timeout = window.setTimeout(() => {
      fail(
        connected
          ? '已连接导出器，但两分钟内没有收到库存。请确认导出器已识别当前账号，或选择导出的 JSON 文件。'
          : '连接导出器超时。请检查实时导出是否已在 23313 端口开启。',
      );
    }, timeoutMs);

    signal.addEventListener('abort', cancel, { once: true });
    socket.onopen = () => {
      connected = true;
      onConnected();
    };
    socket.onclose = () => {
      fail(
        connected
          ? '导出器已断开。请重新开启实时导出后再试，或选择导出的 JSON 文件。'
          : '未能连接到导出器。请先打开 Reliquary Archiver，在其中开启实时导出并确认端口为 23313。',
      );
    };
    socket.onmessage = (message: MessageEvent<unknown>) => {
      const body = message.data;
      if (typeof body !== 'string' || body.length > maxMessageBytes) {
        fail('收到的导出消息格式不正确或超过 8 MB。');
        return;
      }

      let event: unknown;
      try {
        event = JSON.parse(body);
      } catch {
        fail('导出器返回了无效的 JSON。');
        return;
      }

      if (
        typeof event === 'object' &&
        event !== null &&
        'event' in event &&
        event.event === 'InitialScan'
      ) {
        finish();
        resolve(body);
      }
    };
  });
}

export async function saveRelicExport(
  snapshot: string,
  origin: ExportOrigin,
): Promise<RelicExport> {
  try {
    return await invoke<RelicExport>('relic_export_save', { snapshot, origin });
  } catch (cause) {
    if (cause === 'unsupported_format') {
      const message =
        origin === 'file'
          ? '请选择 Archiver 第 4 版导出文件，或模拟器保存的第 1 版遗器 JSON。'
          : '实时接收支持 Reliquary Archiver 的第 4 版导出格式。';
      throw new Error(message, { cause });
    }
    if (cause === 'invalid_snapshot') {
      throw new Error('收到的快照格式不正确或超过 8 MB。', { cause });
    }
    throw new Error('写入本地快照失败，请重试。', { cause });
  }
}

export async function readRelicExportFile(file: File): Promise<string> {
  if (file.size > maxMessageBytes) {
    throw new Error('导出文件超过 8 MB。');
  }
  return file.text();
}

export async function importAccount(
  snapshot: string,
  origin: ExportOrigin,
): Promise<ImportedAccount> {
  try {
    return await invokeForAccount<ImportedAccount>('archiver_import', {
      snapshot,
      origin,
    });
  } catch (cause) {
    const code =
      typeof cause === 'object' && cause !== null && 'code' in cause
        ? cause.code
        : undefined;
    switch (code) {
      case 'invalid_account_selection':
        throw new Error(accountError(cause), { cause });
      case 'archiver_uid_missing':
        throw new Error('账号快照缺少 UID，请重新导出完整账号快照。', {
          cause,
        });
      case 'archiver_account_mismatch':
        throw new Error('账号存档绑定的 UID 与快照不一致，请检查导出文件。', {
          cause,
        });
      case 'catalog_missing':
        throw new Error('请先在「角色」页载入正式服数据，再重试导入。', {
          cause,
        });
      case 'catalog_mismatch':
        throw new Error('当前存档需要原来的游戏数据版本。', { cause });
      case 'invalid_relic_snapshot':
      case 'unsupported_relic_snapshot':
        throw new Error('快照内容不完整或格式不受支持。请重新导出。', {
          cause,
        });
      case 'invalid_relic':
      case 'invalid_eidolon':
      case 'invalid_light_cone':
      case 'invalid_profile':
        throw new Error(
          '快照中的部分养成或装备数值与当前图鉴不匹配，导入已回滚。',
          { cause },
        );
      default:
        throw new Error('写入存档失败，请重试。', { cause });
    }
  }
}
