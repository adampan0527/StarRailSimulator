import { useEffect, useId, useRef, useState } from 'react';
import { EnemyAvatar } from './EnemyArt';
import preview from './enemy-preview.json';

const pageSize = 24;
const collator = new Intl.Collator('zh-CN');
const searchText = (value: string) => value.normalize('NFKC').toLowerCase();
const roster = preview.enemies
  .map((enemy) => ({ ...enemy, searchName: searchText(enemy.name) }))
  .sort(
    (a, b) => collator.compare(a.name, b.name) || Number(a.id) - Number(b.id),
  );

export function EnemyPicker({
  position,
  selected,
  onSelect,
  onClose,
}: {
  position: number;
  selected?: string;
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const results = useRef<HTMLUListElement>(null);
  const title = useId();
  const [query, setQuery] = useState('');
  const [rank, setRank] = useState('');
  const [page, setPage] = useState(0);
  const term = searchText(query.trim());
  const matches = roster.filter(
    (enemy) =>
      (!rank || enemy.rank === rank) &&
      (enemy.searchName.includes(term) ||
        enemy.sourceIds.some((id) => id.includes(term))),
  );
  const pageCount = Math.ceil(matches.length / pageSize);
  const visible = matches.slice(page * pageSize, (page + 1) * pageSize);

  useEffect(() => {
    dialog.current?.showModal();
    search.current?.focus();
  }, []);

  function showPage(next: number) {
    setPage(next);
    results.current?.scrollTo(0, 0);
  }

  return (
    <dialog
      ref={dialog}
      className="picker-dialog enemy-picker"
      aria-labelledby={title}
      onClose={onClose}
    >
      <div className="section-heading">
        <h2 id={title}>选择敌方站位 {position + 1}</h2>
        <button
          type="button"
          className="secondary"
          onClick={() => dialog.current?.close()}
        >
          关闭
        </button>
      </div>
      <div className="picker-filters enemy-filters">
        <label>
          搜索敌人
          <input
            ref={search}
            type="search"
            placeholder="名称或怪物 ID"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              showPage(0);
            }}
          />
        </label>
        <label>
          敌方类别
          <select
            value={rank}
            onChange={(event) => {
              setRank(event.target.value);
              showPage(0);
            }}
          >
            <option value="">全部</option>
            <option value="普通">普通</option>
            <option value="精英">精英</option>
            <option value="首领">首领</option>
          </select>
        </label>
      </div>
      <p className="muted" role="status">
        {matches.length
          ? `共 ${matches.length.toLocaleString('zh-CN')} 种敌人`
          : '没有符合筛选的敌人。'}
      </p>
      <ul
        ref={results}
        className="picker-results enemy-results"
        aria-label="可选敌人"
      >
        {visible.map(({ id, name, rank, avatar }) => (
          <li key={id}>
            <button
              className="choice picker-card"
              type="button"
              aria-pressed={selected === id}
              onClick={() => {
                onSelect(id);
                dialog.current?.close();
              }}
            >
              <span className="enemy-card-body">
                <EnemyAvatar file={avatar} />
                <span className="enemy-card-label">
                  <strong>{name}</strong>
                  <span className="muted">{rank}</span>
                </span>
              </span>
            </button>
          </li>
        ))}
      </ul>
      {pageCount > 1 && (
        <div className="enemy-pages">
          <button
            type="button"
            className="secondary"
            disabled={page === 0}
            onClick={() => showPage(page - 1)}
          >
            上一页
          </button>
          <span className="numeric" aria-live="polite">
            {page + 1} / {pageCount}
          </span>
          <button
            type="button"
            className="secondary"
            disabled={page + 1 === pageCount}
            onClick={() => showPage(page + 1)}
          >
            下一页
          </button>
        </div>
      )}
    </dialog>
  );
}
