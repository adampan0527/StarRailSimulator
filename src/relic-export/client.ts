import { invoke } from '@tauri-apps/api/core';

const endpoint = 'ws://127.0.0.1:23313/ws';
const timeoutMs = 120_000;
const maxMessageBytes = 8 * 1024 * 1024;

export type RelicExport = {
  file_url: string;
  relic_count: number;
};

export function receiveRelics(
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
  origin: 'live' | 'file',
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
      throw new Error('收到的遗器库存格式不正确或超过 8 MB。', { cause });
    }
    throw new Error('写入本地遗器文件失败，请重试。', { cause });
  }
}

export async function readRelicExportFile(file: File): Promise<string> {
  if (file.size > maxMessageBytes) {
    throw new Error('导出文件超过 8 MB。');
  }
  return file.text();
}

export async function saveRelicExportFile(file: File): Promise<RelicExport> {
  return saveRelicExport(await readRelicExportFile(file), 'file');
}
