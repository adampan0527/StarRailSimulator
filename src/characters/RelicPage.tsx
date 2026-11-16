import { useEffect, useState } from 'react';
import {
  catalogError,
  characters,
  relicSlots,
  statName,
  statValue,
} from './client';
import type {
  Catalog,
  InventoryItem,
  Relic,
  RelicOptions,
  RelicSlot,
  RelicSummary,
} from './client';

export function RelicPage({ catalog }: { catalog: Catalog }) {
  const [items, setItems] = useState<InventoryItem[]>();
  const [editing, setEditing] = useState<InventoryItem | 'new'>();
  const [issue, setIssue] = useState('');
  const [message, setMessage] = useState('');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let canceled = false;
    void characters
      .inventory()
      .then((items) => {
        if (!canceled) setItems(items);
      })
      .catch((error: unknown) => {
        if (!canceled) setIssue(catalogError(error));
      });
    return () => {
      canceled = true;
    };
  }, [attempt]);

  return (
    <section className="panel" aria-labelledby="inventory-title">
      <div className="section-heading">
        <h2 id="inventory-title">遗器库存</h2>
        {items && !editing && (
          <button
            type="button"
            onClick={() => {
              setMessage('');
              setEditing('new');
            }}
          >
            新增遗器
          </button>
        )}
      </div>
      {issue && (
        <div>
          <p role="alert">{issue}</p>
          <button
            type="button"
            className="secondary"
            onClick={() => {
              setIssue('');
              setAttempt(attempt + 1);
            }}
          >
            重试
          </button>
        </div>
      )}
      {!items && !issue && <p role="status">正在读取遗器库存…</p>}
      {editing && (
        <RelicEditor
          catalog={catalog}
          item={editing === 'new' ? undefined : editing}
          onCancel={() => setEditing(undefined)}
          onSaved={(updated, status) => {
            setItems(updated);
            setEditing(undefined);
            setMessage(status);
          }}
        />
      )}
      <p role="status" className="muted">
        {message}
      </p>
      {items && items.length === 0 && !editing && (
        <p className="muted">库存还没有遗器。</p>
      )}
      {items && items.length > 0 && (
        <ul className="inventory-list">
          {items.map((item) => {
            const data = catalog.relics.find(
              (entry) => entry.id === item.relic.data_id,
            );
            const owner = catalog.characters.find(
              (entry) => entry.id === item.owner_id,
            );
            return (
              <li key={item.id}>
                <div>
                  <h3>
                    {data?.name ?? item.relic.data_id}{' '}
                    <span className="muted">#{item.id}</span>
                  </h3>
                  <p className="muted">
                    {data?.rarity} 星 +{item.relic.level} ·{' '}
                    {statName(catalog, item.relic.main_stat)} ·{' '}
                    {owner?.name ?? item.owner_id ?? '未穿戴'}
                  </p>
                </div>
                <button
                  type="button"
                  className="secondary"
                  disabled={editing !== undefined}
                  aria-label={`编辑遗器 #${item.id}`}
                  onClick={() => {
                    setMessage('');
                    setEditing(item);
                  }}
                >
                  编辑
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function RelicEditor({
  catalog,
  item,
  onCancel,
  onSaved,
}: {
  catalog: Catalog;
  item?: InventoryItem;
  onCancel: () => void;
  onSaved: (items: InventoryItem[], status: string) => void;
}) {
  const original = catalog.relics.find(
    (entry) => entry.id === item?.relic.data_id,
  );
  const [slot, setSlot] = useState<RelicSlot>(original?.slot ?? 'head');
  const [rarity, setRarity] = useState(
    original?.rarity ??
      Math.max(...catalog.relics.map((entry) => entry.rarity)),
  );
  const [draft, setDraft] = useState(item?.relic);
  const [busy, setBusy] = useState(false);
  const [issue, setIssue] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const data = catalog.relics.find((entry) => entry.id === draft?.data_id);
  const options = catalog.relics.filter(
    (entry) => entry.slot === slot && entry.rarity === rarity,
  );
  const rarities = [
    ...new Set(catalog.relics.map((entry) => entry.rarity)),
  ].sort((a, b) => b - a);

  function pick(id: string) {
    const chosen = options.find((entry) => entry.id === id);
    const main = chosen?.main_stats[0];
    setDraft(
      chosen && main
        ? { data_id: chosen.id, level: 0, main_stat: main, substats: {} }
        : undefined,
    );
    setIssue('');
  }

  async function submit(
    operation: () => Promise<InventoryItem[]>,
    status: string,
  ) {
    setBusy(true);
    setIssue('');
    try {
      onSaved(await operation(), status);
    } catch (error) {
      setIssue(catalogError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      className="inventory-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (draft && !confirmDelete)
          void submit(
            () => characters.saveRelic(item?.id ?? null, draft),
            '遗器已保存。',
          );
      }}
    >
      <h3>{item ? `编辑遗器 #${item.id}` : '新增遗器'}</h3>
      <fieldset disabled={busy || confirmDelete} className="field-stack">
        <div className="equipment-grid">
          <label>
            部位
            <select
              value={slot}
              disabled={Boolean(item?.owner_id)}
              onChange={(event) => {
                const next = relicSlots.find(
                  ([key]) => key === event.target.value,
                );
                if (next) {
                  setSlot(next[0]);
                  setDraft(undefined);
                }
              }}
            >
              {relicSlots.map(([key, label]) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            稀有度
            <select
              value={rarity}
              onChange={(event) => {
                setRarity(Number(event.target.value));
                setDraft(undefined);
              }}
            >
              {rarities.map((value) => (
                <option key={value} value={value}>
                  {value} 星
                </option>
              ))}
            </select>
          </label>
        </div>
        <label>
          遗器
          <select
            value={draft?.data_id ?? ''}
            onChange={(event) => pick(event.target.value)}
            required
          >
            <option value="">选择遗器</option>
            {options.map((entry) => (
              <option key={entry.id} value={entry.id}>
                {catalog.sets[entry.set_id]?.name} · {entry.name}
              </option>
            ))}
          </select>
        </label>
        {data && draft && (
          <RelicStats
            key={data.id}
            catalog={catalog}
            data={data}
            draft={draft}
            onChange={setDraft}
          />
        )}
      </fieldset>
      <div className="form-actions">
        {confirmDelete && item ? (
          <>
            <span>删除遗器 #{item.id}？</span>
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                void submit(
                  () => characters.deleteRelic(item.id),
                  '遗器已删除。',
                )
              }
            >
              确认删除
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy}
              onClick={() => {
                setConfirmDelete(false);
                setIssue('');
              }}
            >
              返回编辑
            </button>
          </>
        ) : (
          <>
            <button type="submit" disabled={!draft || busy}>
              {busy ? '正在保存…' : '保存遗器'}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy}
              onClick={onCancel}
            >
              取消
            </button>
            {item && !item.owner_id && (
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => setConfirmDelete(true)}
              >
                删除
              </button>
            )}
            {item?.owner_id && (
              <span className="muted">卸下后可以删除或更换部位。</span>
            )}
          </>
        )}
      </div>
      {issue && <p role="alert">{issue}</p>}
    </form>
  );
}

function RelicStats({
  catalog,
  data,
  draft,
  onChange,
}: {
  catalog: Catalog;
  data: RelicSummary;
  draft: Relic;
  onChange: (relic: Relic) => void;
}) {
  const [loaded, setLoaded] = useState<{
    level: number;
    options: RelicOptions;
  }>();
  const [issue, setIssue] = useState('');
  const [attempt, setAttempt] = useState(0);
  const options = loaded?.level === draft.level ? loaded.options : undefined;
  useEffect(() => {
    let canceled = false;
    void characters
      .relicOptions(data.id, draft.level)
      .then((options) => {
        if (!canceled) {
          setLoaded({ level: draft.level, options });
          setIssue('');
        }
      })
      .catch((error: unknown) => {
        if (!canceled) setIssue(catalogError(error));
      });
    return () => {
      canceled = true;
    };
  }, [data.id, draft.level, attempt]);
  const substats = Object.entries(draft.substats);
  const available = Object.entries(options?.substats ?? {}).filter(
    ([id, values]) =>
      id !== draft.main_stat && !(id in draft.substats) && values.length > 0,
  );

  function addSubstat(id: string) {
    const value = options?.substats[id]?.[0];
    if (value !== undefined)
      onChange({ ...draft, substats: { ...draft.substats, [id]: value } });
  }

  return (
    <div className="field-stack">
      <div className="equipment-grid">
        <label>
          强化等级
          <select
            value={draft.level}
            onChange={(event) =>
              onChange({ ...draft, level: Number(event.target.value) })
            }
          >
            {Array.from({ length: data.max_level + 1 }, (_, level) => (
              <option key={level} value={level}>
                +{level}
              </option>
            ))}
          </select>
        </label>
        <label>
          主词条
          <select
            value={draft.main_stat}
            onChange={(event) => {
              const substats = { ...draft.substats };
              delete substats[event.target.value];
              onChange({ ...draft, main_stat: event.target.value, substats });
            }}
          >
            {data.main_stats.map((id) => {
              const value = options?.main_stats[id];
              return (
                <option key={id} value={id}>
                  {statName(catalog, id)}
                  {value === undefined ? '' : ` +${statValue(id, value)}`}
                </option>
              );
            })}
          </select>
        </label>
      </div>
      {issue && (
        <div>
          <p role="alert">{issue}</p>
          <button
            type="button"
            className="secondary"
            onClick={() => {
              setIssue('');
              setAttempt(attempt + 1);
            }}
          >
            重试
          </button>
        </div>
      )}
      {!options && !issue && <p role="status">正在读取可选词条…</p>}
      {options && (
        <div className="field-stack">
          <p className="muted">
            当前等级需要{' '}
            {options.min_lines === options.max_lines
              ? options.min_lines
              : `${options.min_lines}～${options.max_lines}`}{' '}
            条副词条。数值按游戏实际档位选择。
          </p>
          {substats.map(([id, value]) => {
            const values = options.substats[id] ?? [];
            return (
              <div key={id} className="substat-row">
                <label>
                  {statName(catalog, id)}
                  <select
                    value={value}
                    onChange={(event) =>
                      onChange({
                        ...draft,
                        substats: {
                          ...draft.substats,
                          [id]: Number(event.target.value),
                        },
                      })
                    }
                  >
                    {!values.includes(value) && (
                      <option value={value}>原数值不适用，请重选</option>
                    )}
                    {values.map((choice) => (
                      <option key={choice} value={choice}>
                        +{statValue(id, choice / 1_000_000)}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`移除${statName(catalog, id)}副词条`}
                  onClick={() => {
                    const substats = { ...draft.substats };
                    delete substats[id];
                    onChange({ ...draft, substats });
                  }}
                >
                  移除
                </button>
              </div>
            );
          })}
          {substats.length < options.max_lines && available.length > 0 && (
            <label>
              添加副词条
              <select
                value=""
                onChange={(event) => addSubstat(event.target.value)}
              >
                <option value="">选择属性</option>
                {available.map(([id]) => (
                  <option key={id} value={id}>
                    {statName(catalog, id)}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}
    </div>
  );
}
