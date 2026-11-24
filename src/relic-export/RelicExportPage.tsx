import { useEffect, useRef, useState } from 'react';
import { AccountPanel } from './AccountPanel';
import { accountError, gameAccounts } from './accounts';
import type { AccountSelection } from './accounts';
import {
  importAccount,
  openArchiver,
  readRelicExportFile,
  receiveAccount,
  saveRelicExport,
} from './client';
import type { ExportOrigin, ImportedAccount, RelicExport } from './client';

type ExportState =
  | { kind: 'idle' | 'connecting' | 'waiting' | 'saving' }
  | { kind: 'error'; message: string }
  | {
      kind: 'ready';
      result: RelicExport;
      snapshot: string;
      origin: ExportOrigin;
    };

const progress = {
  idle: '等待连接导出器。',
  connecting: '正在连接本机导出器…',
  waiting: '正在等待账号数据。请在游戏标题画面点击「开始游戏」。',
  saving: '正在保存本地快照…',
};

export function RelicExportPage({
  selection,
  accountIssue,
  onRetry,
  onSelection,
  onImported,
}: {
  selection?: AccountSelection;
  accountIssue: string;
  onRetry: () => void;
  onSelection: (selection: AccountSelection) => void;
  onImported: (result: ImportedAccount) => void;
}) {
  const [state, setState] = useState<ExportState>({ kind: 'idle' });
  const [copyStatus, setCopyStatus] = useState('');
  const [launching, setLaunching] = useState(false);
  const [launchFeedback, setLaunchFeedback] = useState<{
    message: string;
    error: boolean;
  }>();
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState('');
  const [imported, setImported] = useState<ImportedAccount>();
  const [managing, setManaging] = useState(false);
  const [managementIssue, setManagementIssue] = useState('');
  const activeRequest = useRef<AbortController>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const linkInput = useRef<HTMLInputElement>(null);
  const receiving = state.kind === 'connecting' || state.kind === 'waiting';
  const busy =
    receiving || state.kind === 'saving' || importing || launching || managing;

  useEffect(() => () => activeRequest.current?.abort(), []);

  async function manage(operation: () => Promise<AccountSelection>) {
    setManaging(true);
    setManagementIssue('');
    try {
      onSelection(await operation());
    } catch (error) {
      setManagementIssue(accountError(error));
    } finally {
      setManaging(false);
    }
  }

  async function launch(reselect: boolean) {
    setLaunching(true);
    setLaunchFeedback(undefined);
    try {
      const result = await openArchiver(reselect);
      if (result !== 'canceled') {
        setLaunchFeedback({
          message: 'Archiver 已打开。请开启实时导出，然后返回开始接收。',
          error: false,
        });
      }
    } catch (error) {
      setLaunchFeedback({
        message: error instanceof Error ? error.message : '无法打开 Archiver。',
        error: true,
      });
    } finally {
      setLaunching(false);
    }
  }

  async function receive(file?: File) {
    if (activeRequest.current) return;
    const request = new AbortController();
    activeRequest.current = request;
    setCopyStatus('');
    setLaunchFeedback(undefined);
    setImported(undefined);
    setImportError('');
    setState({ kind: file ? 'saving' : 'connecting' });
    try {
      const origin = file ? 'file' : 'live';
      const snapshot = file
        ? await readRelicExportFile(file)
        : await receiveAccount(request.signal, () =>
            setState({ kind: 'waiting' }),
          );
      request.signal.throwIfAborted();
      setState({ kind: 'saving' });
      const result = await saveRelicExport(snapshot, origin);
      if (!request.signal.aborted)
        setState({ kind: 'ready', result, snapshot, origin });
    } catch (error) {
      if (!request.signal.aborted) {
        setState({
          kind: 'error',
          message:
            error instanceof Error ? error.message : '读取账号快照失败。',
        });
      }
    } finally {
      if (activeRequest.current === request) activeRequest.current = null;
    }
  }

  async function applySnapshot() {
    if (state.kind !== 'ready' || activeRequest.current) return;
    const request = new AbortController();
    activeRequest.current = request;
    setImporting(true);
    setImportError('');
    setImported(undefined);
    try {
      const result = await importAccount(state.snapshot, state.origin);
      if (!request.signal.aborted) {
        onImported(result);
        setImported(result);
      }
    } catch (error) {
      if (!request.signal.aborted) {
        setImportError(
          error instanceof Error ? error.message : '导入存档失败。',
        );
      }
    } finally {
      if (activeRequest.current === request) activeRequest.current = null;
      if (!request.signal.aborted) setImporting(false);
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
    <div className="account-workspace">
      <AccountPanel
        selection={selection}
        issue={managementIssue || accountIssue}
        busy={busy}
        onRetry={onRetry}
        onSelect={(uid) => void manage(() => gameAccounts.select(uid))}
        onRename={(uid, remark) =>
          void manage(() => gameAccounts.rename(uid, remark))
        }
      />
      <section className="panel" aria-labelledby="relic-export-title">
        <h2 id="relic-export-title">账号导入</h2>
        <p className="muted">
          在 Reliquary Archiver 开启 23313
          端口的实时导出后接收账号快照，也可直接选择已导出的 JSON。
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
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => void launch(false)}
          >
            打开 Archiver
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => void launch(true)}
          >
            更换并打开
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
        {launching && <p role="status">正在打开 Archiver…</p>}
        {launchFeedback && (
          <p
            className={launchFeedback.error ? 'outcome-defeat' : undefined}
            role={launchFeedback.error ? 'alert' : 'status'}
          >
            {launchFeedback.message}
          </p>
        )}
        {state.kind === 'error' ? (
          <p className="outcome-defeat" role="alert">
            {state.message}
          </p>
        ) : (
          state.kind !== 'ready' && <p role="status">{progress[state.kind]}</p>
        )}
        {state.kind === 'ready' && (
          <>
            <SnapshotSummary result={state.result} />
            <div className="form-actions">
              <button
                type="button"
                disabled={
                  busy || !selection || state.result.account?.uid === null
                }
                onClick={() => void applySnapshot()}
              >
                {state.result.account ? '导入角色与装备' : '导入遗器'}
              </button>
            </div>
            {state.result.account?.uid === null && (
              <p className="outcome-defeat" role="alert">
                账号快照缺少 UID，请重新导出完整账号快照。
              </p>
            )}
            {importing && <p role="status">正在写入存档…</p>}
            {importError && (
              <p className="outcome-defeat" role="alert">
                {importError}
              </p>
            )}
            {imported && (
              <ImportSummary
                result={imported}
                account={Boolean(state.result.account)}
              />
            )}
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
    </div>
  );
}

function SnapshotSummary({ result }: { result: RelicExport }) {
  const { account } = result;
  if (!account)
    return <p role="status">已保存 {result.relic_count} 件遗器的快照。</p>;

  const extras = [
    account.trailblazer === 'Stelle' ? '开拓者 星' : null,
    account.trailblazer === 'Caelus' ? '开拓者 穹' : null,
    account.materials === null ? null : `${account.materials} 种背包道具`,
    account.stellar_jade === null ? null : `星琼 ${account.stellar_jade}`,
    account.oneiric_shards === null
      ? null
      : `古老梦华 ${account.oneiric_shards}`,
    account.recon_relics === null
      ? null
      : `奇库寻遗 ${account.recon_relics} 件`,
  ].filter((item) => item !== null);
  return (
    <div role="status">
      <p>
        {account.uid !== null && <>UID {account.uid} · </>}
        已保存 {account.characters} 名角色、{account.light_cones} 张光锥和{' '}
        {result.relic_count} 件遗器。
      </p>
      {extras.length > 0 && <p className="muted">{extras.join(' · ')}</p>}
    </div>
  );
}

function ImportSummary({
  result: { uid, created, summary: result },
  account,
}: {
  result: ImportedAccount;
  account: boolean;
}) {
  const skipped = result.characters_skipped + result.light_cones_skipped;
  return (
    <div role="status">
      {uid !== null && (
        <p>
          {created ? '已创建账号' : '已更新账号'} UID {uid}。
        </p>
      )}
      {account && (
        <p>
          写入 {result.characters_applied} 名角色的养成数据，保留{' '}
          {result.characters_preserved} 名角色的本地配置。 新增{' '}
          {result.light_cones_added} 张光锥，更新 {result.light_cones_updated}{' '}
          张，装备 {result.light_cones_equipped} 张。
        </p>
      )}
      <p>
        新增 {result.relics.added} 件遗器
        {account && <>，更新 {result.relics_updated} 件</>}，穿戴{' '}
        {result.relics.equipped} 件。
      </p>
      {skipped > 0 && (
        <p className="muted">
          {skipped} 条角色或光锥记录缺少图鉴匹配或完整的养成数据，已跳过。
        </p>
      )}
    </div>
  );
}
