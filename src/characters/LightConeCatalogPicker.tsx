import { useEffect, useId, useRef, useState } from 'react';
import { CombatTraitFilter } from './CombatTrait';
import { LightConeCard } from './LightConeCard';
import type { LightConeSummary } from './client';
import { compareLightConeRelease } from './releaseOrder';

export function LightConeCatalogPicker({
  cones,
  selectedId,
  onSelect,
  onClose,
}: {
  cones: LightConeSummary[];
  selectedId?: string;
  onSelect: (cone: LightConeSummary) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const title = useId();
  const [query, setQuery] = useState('');
  const [path, setPath] = useState('');
  const visible = cones
    .filter(
      (cone) =>
        (!path || cone.path === path) &&
        cone.name.toLowerCase().includes(query.trim().toLowerCase()),
    )
    .sort(compareLightConeRelease);

  useEffect(() => {
    dialog.current?.showModal();
    search.current?.focus();
  }, []);

  return (
    <dialog
      ref={dialog}
      className="picker-dialog light-cone-dialog"
      aria-labelledby={title}
      onClose={onClose}
    >
      <div className="section-heading">
        <h2 id={title}>选择要录入的光锥</h2>
        <button
          type="button"
          className="secondary"
          onClick={() => dialog.current?.close()}
        >
          关闭
        </button>
      </div>
      <div className="light-cone-filters">
        <label>
          搜索光锥
          <input
            ref={search}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <CombatTraitFilter
          kind="path"
          options={cones.map(({ path }) => path)}
          value={path}
          onChange={setPath}
        />
      </div>
      <p className="muted" role="status">
        {visible.length
          ? `找到 ${visible.length} 件光锥`
          : '没有符合筛选的光锥。'}
      </p>
      {visible.length > 0 && (
        <ul className="light-cone-results">
          {visible.map((data) => (
            <li key={data.default.data_id}>
              <button
                type="button"
                className="light-cone-card-button"
                aria-pressed={data.default.data_id === selectedId}
                onClick={() => {
                  onSelect(data);
                  dialog.current?.close();
                }}
              >
                <LightConeCard data={data} dataId={data.default.data_id} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </dialog>
  );
}
