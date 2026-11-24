import { useState } from 'react';
import type { AccountSelection } from './accounts';

export function AccountPanel({
  selection,
  issue,
  busy,
  onRetry,
  onSelect,
  onRename,
}: {
  selection?: AccountSelection;
  issue: string;
  busy: boolean;
  onRetry: () => void;
  onSelect: (uid: number) => void;
  onRename: (uid: number, remark: string) => void;
}) {
  const account = selection?.accounts.find(
    ({ uid }) => uid === selection.active_uid,
  );

  return (
    <section className="panel" aria-labelledby="account-panel-title">
      <h2 id="account-panel-title">账号管理</h2>
      {issue && <p role="alert">{issue}</p>}
      {selection ? (
        account ? (
          <div className="account-fields">
            <label>
              当前账号
              <select
                value={account.uid}
                disabled={busy}
                onChange={(event) => onSelect(Number(event.target.value))}
              >
                {selection.accounts.map(({ uid, remark }) => (
                  <option key={uid} value={uid}>
                    {remark === String(uid) ? remark : `${remark} · UID ${uid}`}
                  </option>
                ))}
              </select>
            </label>
            <AccountRemark
              key={`${account.uid}:${account.remark}`}
              initial={account.remark}
              fallback={String(account.uid)}
              busy={busy}
              onSave={(remark) => onRename(account.uid, remark)}
            />
          </div>
        ) : (
          <p className="muted">
            当前使用本地存档。导入完整快照后可按 UID 管理账号。
          </p>
        )
      ) : issue ? (
        <div className="form-actions">
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={onRetry}
          >
            重试
          </button>
        </div>
      ) : (
        <p role="status">正在读取账号…</p>
      )}
    </section>
  );
}

function AccountRemark({
  initial,
  fallback,
  busy,
  onSave,
}: {
  initial: string;
  fallback: string;
  busy: boolean;
  onSave: (remark: string) => void;
}) {
  const [remark, setRemark] = useState(initial);

  return (
    <form
      className="account-remark"
      onSubmit={(event) => {
        event.preventDefault();
        const next = remark.trim();
        if ((next || fallback) === initial) setRemark(initial);
        else onSave(next);
      }}
    >
      <label>
        账号备注
        <input
          value={remark}
          disabled={busy}
          maxLength={48}
          placeholder="留空恢复 UID"
          onChange={(event) => setRemark(event.target.value)}
        />
      </label>
      <button
        type="submit"
        className="secondary"
        disabled={busy || remark.trim() === initial}
      >
        保存备注
      </button>
    </form>
  );
}
