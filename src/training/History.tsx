import { useEffect, useRef, useState } from 'react';
import { BattlePanel } from './Battle';
import { describeError, training } from './client';
import type { BattleView, ReplaySummary } from './client';

const date = new Intl.DateTimeFormat('zh-CN', {
  dateStyle: 'short',
  timeStyle: 'short',
});

export function History({
  onResume,
}: {
  onResume: (battle: BattleView) => void;
}) {
  const [replays, setReplays] = useState<ReplaySummary[] | null>(null);
  const [opened, setOpened] = useState<BattleView>();
  const [opening, setOpening] = useState<number>();
  const [issue, setIssue] = useState('');
  const detail = useRef<HTMLDivElement>(null);

  async function load() {
    setIssue('');
    try {
      setReplays(await training.replays());
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
      setOpened(await training.replay(id));
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
        {replays?.length === 0 && (
          <p>还没有战斗记录。开始训练后，记录会自动保存在这里。</p>
        )}
        {replays && replays.length > 0 && (
          <ul className="replay-list">
            {replays.map(({ id, started_at, finished }) => (
              <li key={id}>
                <div>
                  <strong>训练 #{id}</strong>
                  <span className="muted">
                    {date.format(started_at * 1000)} ·{' '}
                    {finished ? '已结束' : '未结束'}
                  </span>
                </div>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`打开训练回放 #${id}`}
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
          {opened.snapshot.outcome === 'ongoing' && (
            <div className="resume-bar">
              <button type="button" onClick={() => onResume(opened)}>
                继续这场战斗
              </button>
            </div>
          )}
          <BattlePanel battle={opened} playing={false} />
        </div>
      )}
    </div>
  );
}
