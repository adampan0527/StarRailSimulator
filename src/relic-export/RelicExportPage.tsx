import { useEffect, useRef, useState } from 'react';
import { receiveRelics, saveRelicExport, saveRelicExportFile } from './client';
import type { RelicExport } from './client';

type ExportState =
  | { kind: 'idle' | 'connecting' | 'waiting' | 'saving' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; result: RelicExport };

const progress = {
  idle: '等待连接导出器。',
  connecting: '正在连接本机导出器…',
  waiting: '正在等待库存。请在游戏标题画面点击「开始游戏」。',
  saving: '正在读取并保存遗器 JSON…',
};

export function RelicExportPage() {
  const [state, setState] = useState<ExportState>({ kind: 'idle' });
  const [copyStatus, setCopyStatus] = useState('');
  const activeRequest = useRef<AbortController>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const linkInput = useRef<HTMLInputElement>(null);
  const receiving = state.kind === 'connecting' || state.kind === 'waiting';
  const busy = receiving || state.kind === 'saving';

  useEffect(() => () => activeRequest.current?.abort(), []);

  async function receive(file?: File) {
    if (activeRequest.current) return;
    const request = new AbortController();
    activeRequest.current = request;
    setCopyStatus('');
    setState({ kind: file ? 'saving' : 'connecting' });
    try {
      let result: RelicExport;
      if (file) {
        result = await saveRelicExportFile(file);
      } else {
        const snapshot = await receiveRelics(request.signal, () =>
          setState({ kind: 'waiting' }),
        );
        request.signal.throwIfAborted();
        setState({ kind: 'saving' });
        result = await saveRelicExport(snapshot, 'live');
      }
      if (!request.signal.aborted) setState({ kind: 'ready', result });
    } catch (error) {
      if (!request.signal.aborted) {
        setState({
          kind: 'error',
          message:
            error instanceof Error ? error.message : '接收遗器库存失败。',
        });
      }
    } finally {
      if (activeRequest.current === request) activeRequest.current = null;
    }
  }

  function cancel() {
    activeRequest.current?.abort();
    activeRequest.current = null;
    setState({ kind: 'idle' });
  }

  async function copyLink(link: string) {
    try {
      await navigator.clipboard.writeText(link);
      setCopyStatus('链接已复制');
    } catch {
      linkInput.current?.focus();
      linkInput.current?.select();
      setCopyStatus('无法访问剪贴板。链接已选中，可按 Ctrl+C 复制。');
    }
  }

  return (
    <section className="panel" aria-labelledby="relic-export-title">
      <h2 id="relic-export-title">遗器导出</h2>
      <p className="muted">
        实时接收需要先在本机的 Reliquary Archiver 中开启实时导出，端口为
        23313。也可直接选择 Archiver 导出的 JSON 或模拟器已保存的遗器 JSON。
      </p>
      <div className="form-actions">
        <button type="button" disabled={busy} onClick={() => void receive()}>
          {state.kind === 'ready' ? '重新接收' : '开始接收'}
        </button>
        <button
          type="button"
          className="secondary"
          disabled={busy}
          onClick={() => fileInput.current?.click()}
        >
          选择导出 JSON
        </button>
        <input
          ref={fileInput}
          type="file"
          accept=".json,application/json"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = '';
            if (file) void receive(file);
          }}
        />
        {receiving && (
          <button type="button" className="secondary" onClick={cancel}>
            取消
          </button>
        )}
      </div>
      {state.kind === 'error' ? (
        <p className="outcome-defeat" role="alert">
          {state.message}
        </p>
      ) : (
        <p role="status">
          {state.kind === 'ready'
            ? `已保存 ${state.result.relic_count} 件遗器到文件。可在「遗器」页选择「导入 JSON」。`
            : progress[state.kind]}
        </p>
      )}
      {state.kind === 'ready' && (
        <>
          <label className="relic-export-link">
            本地文件链接
            <input ref={linkInput} readOnly value={state.result.file_url} />
          </label>
          <div className="form-actions">
            <button
              type="button"
              className="secondary"
              onClick={() => void copyLink(state.result.file_url)}
            >
              复制链接
            </button>
            <span role="status">{copyStatus}</span>
          </div>
        </>
      )}
    </section>
  );
}
