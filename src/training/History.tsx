import { useEffect, useRef, useState } from 'react';
import { BattlePanel } from './Battle';
import { describeError, battleHistory } from './client';
import type { BattleView, ReplaySummary } from './client';

const date = new Intl.DateTimeFormat('zh-CN', {
  dateStyle: 'short',
  timeStyle: 'short',
});

export function History() {
  const [replays, setReplays] = useState<ReplaySummary[] | null>(null);
  const [opened, setOpened] = useState<BattleView>();
  const [opening, setOpening] = useState<number>();
  const [issue, setIssue] = useState('');
  const detail = useRef<HTMLDivElement>(null);

  async function load() {
    setIssue('');
    try {
      setReplays(await battleHistory.list());
    } catch (error) {
      setIssue(describeError(error));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  useEffect(() => {
    if (opened) detail.current?.focus();
  }, [opened]);

  async function open(id: number) {
    setOpening(id);
    setIssue('');
    try {
      setOpened(await battleHistory.replay(id));
    } catch (error) {
      setIssue(describeError(error));
    } finally {
      setOpening(undefined);
    }
  }

  return (
    <div className="history-layout">
      <section className="panel" aria-labelledby="history-title">
        <div className="section-heading">
          <h2 id="history-title">本地回放</h2>
        </div>
        {issue && <p role="alert">{issue}</p>}
        {!replays &&
          (issue ? (
            <button type="button" className="secondary" onClick={load}>
              重试
            </button>
          ) : (
            <p role="status">正在读取本地回放…</p>
          ))}
        {replays?.length === 0 && <p>暂无战斗记录。</p>}
        {replays && replays.length > 0 && (
          <ul className="replay-list">
            {replays.map(({ id, started_at, finished }) => (
              <li key={id}>
                <div>
                  <strong>战斗 #{id}</strong>
                  <span className="muted">
                    {date.format(started_at * 1000)} ·{' '}
                    {finished ? '已结束' : '保存时未结束'}
                  </span>
                </div>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`打开战斗回放 #${id}`}
                  disabled={opening !== undefined}
                  onClick={() => void open(id)}
                >
                  {opening === id ? '读取中…' : '打开回放'}
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
      {opened && (
        <div
          className="history-detail"
          ref={detail}
          tabIndex={-1}
          role="region"
          aria-label="回放详情"
        >
          <BattlePanel battle={opened} />
        </div>
      )}
    </div>
  );
}
