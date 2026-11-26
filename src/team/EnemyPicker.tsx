import { useEffect, useId, useRef, useState } from 'react';
import { EnemyAvatar } from './EnemyArt';
import preview from './enemy-preview.json';

const planetOrder = new Map(
  preview.planets.map((planet, index) => [planet, index]),
);
const rankOrder: Record<string, number> = { 首领: 0, 精英: 1, 普通: 2 };
const searchText = (value: string) => value.normalize('NFKC').toLowerCase();
const roster = preview.enemies
  .map((enemy) => ({
    ...enemy,
    searchName: searchText(
      [enemy.name, enemy.planet, enemy.variant ?? ''].join(' '),
    ),
  }))
  .sort(
    (a, b) =>
      (planetOrder.get(a.planet) ?? 99) - (planetOrder.get(b.planet) ?? 99) ||
      (rankOrder[a.rank] ?? 99) - (rankOrder[b.rank] ?? 99) ||
      Number(a.variant !== null) - Number(b.variant !== null) ||
      Number(a.id) - Number(b.id),
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
  const results = useRef<HTMLDivElement>(null);
  const title = useId();
  const [query, setQuery] = useState('');
  const [planet, setPlanet] = useState('');
  const [rank, setRank] = useState('');
  const term = searchText(query.trim());
  const matches = roster.filter(
    (enemy) =>
      (!planet || enemy.planet === planet) &&
      (!rank ||
        (rank === '末日幻影'
          ? enemy.variant === '末日幻影'
          : enemy.rank === rank)) &&
      (enemy.searchName.includes(term) ||
        enemy.sourceIds.some((id) => id.includes(term))),
  );
  const groups = preview.planets
    .map((name) => ({
      planet: name,
      items: matches.filter((enemy) => enemy.planet === name),
    }))
    .filter(({ items }) => items.length > 0);

  useEffect(() => {
    dialog.current?.showModal();
    search.current?.focus();
  }, []);

  function resetScroll() {
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
        <label className="enemy-filter-search">
          搜索敌人
          <input
            ref={search}
            type="search"
            placeholder="名称、星球或怪物 ID"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              resetScroll();
            }}
          />
        </label>
        <label>
          首次出场星球
          <select
            value={planet}
            onChange={(event) => {
              setPlanet(event.target.value);
              resetScroll();
            }}
          >
            <option value="">全部星球</option>
            {preview.planets.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <label>
          敌方类别
          <select
            value={rank}
            onChange={(event) => {
              setRank(event.target.value);
              resetScroll();
            }}
          >
            <option value="">全部</option>
            <option value="首领">首领</option>
            <option value="精英">精英</option>
            <option value="普通">普通</option>
            <option value="末日幻影">末日幻影</option>
          </select>
        </label>
      </div>
      <p className="muted" role="status">
        {matches.length
          ? `共 ${matches.length.toLocaleString('zh-CN')} 种敌人`
          : '没有符合筛选的敌人。'}
      </p>
      <div
        ref={results}
        className="enemy-groups"
        aria-label="按星球分类的可选敌人"
      >
        {groups.map(({ planet: groupName, items }) => (
          <section
            key={groupName}
            className="enemy-planet-group"
            aria-label={groupName}
          >
            <div className="enemy-planet-heading">
              <h3>{groupName}</h3>
              <span className="muted numeric">{items.length} 种</span>
            </div>
            <ul
              className="picker-results enemy-results"
              aria-label={`${groupName}可选敌人`}
            >
              {items.map(({ id, name, rank: enemyRank, variant, avatar }) => (
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
                        <span className="muted">
                          {variant ? `${enemyRank} · ${variant}` : enemyRank}
                        </span>
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </dialog>
  );
}
