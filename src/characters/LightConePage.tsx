import { useEffect, useState } from 'react';
import { LightConeFields, lightConeName } from './CharacterLightCone';
import { catalogError, characters } from './client';
import type { Catalog, InventoryLightCone, LightCone } from './client';

export function LightConePage({ catalog }: { catalog: Catalog }) {
  const [items, setItems] = useState<InventoryLightCone[]>();
  const [editing, setEditing] = useState<InventoryLightCone | 'new'>();
  const [issue, setIssue] = useState('');
  const [status, setStatus] = useState('');
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let canceled = false;
    void characters
      .lightConeInventory()
      .then((inventory) => {
        if (!canceled) setItems(inventory);
      })
      .catch((error: unknown) => {
        if (!canceled) setIssue(catalogError(error));
      });
    return () => {
      canceled = true;
    };
  }, [attempt]);

  function edit(item: InventoryLightCone | 'new') {
    setEditing(item);
    setStatus('');
  }

  return (
    <section className="panel" aria-labelledby="light-cone-title">
      <div className="section-heading">
        <h2 id="light-cone-title">我的光锥库</h2>
        {items && !editing && (
          <button type="button" onClick={() => edit('new')}>
            录入光锥
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
      {!items && !issue && <p role="status">正在读取光锥库…</p>}
      {editing && (
        <LightConeEditor
          catalog={catalog}
          item={editing === 'new' ? undefined : editing}
          onCancel={() => setEditing(undefined)}
          onSaved={(inventory, message) => {
            setItems(inventory);
            setStatus(message);
            setEditing(undefined);
          }}
        />
      )}
      <p className="muted" role="status">
        {status}
      </p>
      {items?.length === 0 && !editing && (
        <p className="muted">录入你实际持有的光锥，即可在「角色」页装备。</p>
      )}
      {items && items.length > 0 && (
        <ul className="inventory-list">
          {items.map((item) => {
            const owner = catalog.characters.find(
              ({ id }) => id === item.owner_id,
            )?.name;
            return (
              <li key={item.id}>
                <div>
                  <h3>
                    {lightConeName(catalog, item.cone)}{' '}
                    <span className="muted">#{item.id}</span>
                  </h3>
                  <p className="muted">
                    Lv. {item.cone.level} · S{item.cone.superimposition} ·{' '}
                    {owner ?? item.owner_id ?? '未装备'}
                  </p>
                </div>
                <button
                  type="button"
                  className="secondary"
                  disabled={editing !== undefined}
                  aria-label={`编辑光锥 #${item.id}`}
                  onClick={() => edit(item)}
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

function LightConeEditor({
  catalog,
  item,
  onCancel,
  onSaved,
}: {
  catalog: Catalog;
  item?: InventoryLightCone;
  onCancel: () => void;
  onSaved: (items: InventoryLightCone[], message: string) => void;
}) {
  const [cone, setCone] = useState<LightCone | null>(item?.cone ?? null);
  const [busy, setBusy] = useState(false);
  const [issue, setIssue] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function submit(
    operation: () => Promise<InventoryLightCone[]>,
    message: string,
  ) {
    setBusy(true);
    setIssue('');
    try {
      onSaved(await operation(), message);
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
        if (cone && !confirmDelete && !busy)
          void submit(
            () => characters.saveLightCone(item?.id ?? null, cone),
            '光锥已保存。',
          );
      }}
    >
      <h3>{item ? `编辑光锥 #${item.id}` : '录入持有的光锥'}</h3>
      <fieldset className="field-stack" disabled={busy || confirmDelete}>
        <LightConeFields
          cones={catalog.light_cones}
          label="光锥"
          saved={item?.cone ?? null}
          value={cone}
          onChange={setCone}
        />
        {cone && (
          <div className="equipment-grid">
            <label>
              等级
              <input
                type="number"
                required
                min={1}
                max={80}
                value={Number.isNaN(cone.level) ? '' : cone.level}
                onChange={(event) =>
                  setCone({ ...cone, level: event.target.valueAsNumber })
                }
              />
            </label>
            <label>
              突破阶段
              <select
                value={cone.ascension}
                onChange={(event) =>
                  setCone({ ...cone, ascension: Number(event.target.value) })
                }
              >
                {Array.from({ length: 7 }, (_, stage) => (
                  <option key={stage} value={stage}>
                    {stage} 阶
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
      </fieldset>
      <div className="form-actions">
        {confirmDelete && item ? (
          <>
            <span>删除光锥 #{item.id}？</span>
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                void submit(
                  () => characters.deleteLightCone(item.id),
                  '光锥已删除。',
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
            <button type="submit" disabled={!cone || busy}>
              {busy ? '正在保存…' : '保存光锥'}
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
              <span className="muted">卸下后可以删除这件光锥。</span>
            )}
          </>
        )}
      </div>
      {issue && <p role="alert">{issue}</p>}
    </form>
  );
}
