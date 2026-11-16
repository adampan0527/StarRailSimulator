import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { relicSlots, statName, statValue } from './client';
import type {
  Catalog,
  CharacterEdit,
  InventoryItem,
  RelicSlot,
  RelicSummary,
} from './client';

type Entry = { item: InventoryItem; data: RelicSummary };
type Equipment = CharacterEdit['relics'];

export function CharacterRelics({
  catalog,
  items,
  characterId,
  equipment,
  onEquip,
}: {
  catalog: Catalog;
  items: InventoryItem[];
  characterId: string;
  equipment: Equipment;
  onEquip: (slot: RelicSlot, id: number | null) => void;
}) {
  const [openSlot, setOpenSlot] = useState<RelicSlot>();
  const entries = useMemo(() => {
    const definitions = new Map(catalog.relics.map((data) => [data.id, data]));
    return items.flatMap((item) => {
      const data = definitions.get(item.relic.data_id);
      return data ? [{ item, data }] : [];
    });
  }, [catalog.relics, items]);

  return (
    <div className="field-stack">
      <h3>穿戴遗器</h3>
      {entries.length === 0 && (
        <p className="muted">库存为空，请先到「遗器」页录入。</p>
      )}
      <div className="equipment-grid">
        {relicSlots.map(([slot, label]) => {
          const equipped = entries.find(
            ({ item }) => item.id === equipment[slot],
          );
          return (
            <button
              type="button"
              key={slot}
              className="choice relic-slot"
              aria-haspopup="dialog"
              aria-label={`${label} · ${equipped?.data.name ?? '空位'}，选择遗器`}
              onClick={() => setOpenSlot(slot)}
            >
              <span className="muted">{label}</span>
              {equipped
                ? `${equipped.data.name} +${equipped.item.relic.level}`
                : '空位'}
            </button>
          );
        })}
      </div>
      {openSlot && (
        <RelicPicker
          initialSlot={openSlot}
          catalog={catalog}
          entries={entries}
          characterId={characterId}
          equipment={equipment}
          onEquip={onEquip}
          onClose={() => setOpenSlot(undefined)}
        />
      )}
    </div>
  );
}

function RelicPicker({
  initialSlot,
  catalog,
  entries,
  characterId,
  equipment,
  onEquip,
  onClose,
}: {
  initialSlot: RelicSlot;
  catalog: Catalog;
  entries: Entry[];
  characterId: string;
  equipment: Equipment;
  onEquip: (slot: RelicSlot, id: number | null) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const title = useId();
  const [slot, setSlot] = useState<RelicSlot | ''>(initialSlot);
  const [set, setSet] = useState('');
  const [main, setMain] = useState('');
  const [sub, setSub] = useState('');
  const slotLabel = relicSlots.find(([id]) => id === slot)?.[1];

  const visible = entries.filter(
    ({ item, data }) =>
      (!slot || data.slot === slot) &&
      (!set || data.set_id === set) &&
      (!main || item.relic.main_stat === main) &&
      (!sub || Object.hasOwn(item.relic.substats, sub)),
  );
  const setName = (id: string) => catalog.sets[id]?.name ?? id;
  const statLabel = (id: string) => {
    const name = statName(catalog, id);
    return ['hp_ratio', 'attack_ratio', 'defense_ratio'].includes(id)
      ? `${name}%`
      : name;
  };
  const sorted = (ids: string[], label: (id: string) => string) =>
    [...new Set(ids)].sort((a, b) => label(a).localeCompare(label(b), 'zh-CN'));

  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  function choose(target: RelicSlot, id: number | null) {
    onEquip(target, id);
    dialog.current?.close();
  }

  return (
    <dialog
      ref={dialog}
      className="picker-dialog"
      aria-labelledby={title}
      onClose={onClose}
    >
      <div className="section-heading">
        <h2 id={title}>选择遗器</h2>
        <button
          type="button"
          className="secondary"
          onClick={() => dialog.current?.close()}
        >
          关闭
        </button>
      </div>
      <div className="picker-filters">
        <label>
          部位
          <select
            value={slot}
            onChange={(event) => {
              setSlot(
                relicSlots.find(([id]) => id === event.target.value)?.[0] ?? '',
              );
            }}
          >
            <option value="">全部部位</option>
            {relicSlots.map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <RelicFilter
          label="套装"
          value={set}
          onChange={setSet}
          options={sorted(
            entries.map(({ data }) => data.set_id),
            setName,
          )}
          name={setName}
        />
        <RelicFilter
          label="主词条"
          value={main}
          onChange={setMain}
          options={sorted(
            entries.map(({ item }) => item.relic.main_stat),
            statLabel,
          )}
          name={statLabel}
        />
        <RelicFilter
          label="副词条"
          value={sub}
          onChange={setSub}
          options={sorted(
            entries.flatMap(({ item }) => Object.keys(item.relic.substats)),
            statLabel,
          )}
          name={statLabel}
        />
      </div>
      <div className="section-heading">
        <p className="muted" role="status">
          {entries.length === 0
            ? '库存为空，请先到「遗器」页录入。'
            : visible.length
              ? `找到 ${visible.length} 件遗器`
              : '没有符合筛选的遗器。'}
        </p>
        {slot && equipment[slot] !== undefined && (
          <button
            type="button"
            className="secondary"
            onClick={() => choose(slot, null)}
          >
            卸下{slotLabel}遗器
          </button>
        )}
      </div>
      {visible.length > 0 && (
        <ul className="relic-picker-results">
          {visible.map(({ item, data }) => {
            const occupied =
              item.owner_id !== null && item.owner_id !== characterId;
            const owner = catalog.characters.find(
              ({ id }) => id === item.owner_id,
            )?.name;
            const label = relicSlots.find(([id]) => id === data.slot)?.[1];
            return (
              <li key={item.id}>
                <button
                  type="button"
                  className="choice relic-choice"
                  disabled={occupied}
                  aria-pressed={equipment[data.slot] === item.id}
                  onClick={() => choose(data.slot, item.id)}
                >
                  <span>
                    <strong>{data.name}</strong>{' '}
                    <span className="muted">
                      #{item.id} · {data.rarity} 星 +{item.relic.level}
                    </span>
                  </span>
                  <span className="muted">
                    {label} · {setName(data.set_id)}
                    {occupied && ` · ${owner ?? item.owner_id}已穿戴`}
                  </span>
                  <span>主词条 · {statLabel(item.relic.main_stat)}</span>
                  {Object.keys(item.relic.substats).length > 0 && (
                    <span className="muted">
                      {Object.entries(item.relic.substats)
                        .map(
                          ([id, value]) =>
                            `${statName(catalog, id)} +${statValue(id, value / 1_000_000)}`,
                        )
                        .join(' · ')}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </dialog>
  );
}

function RelicFilter({
  label,
  value,
  onChange,
  options,
  name,
}: {
  label: string;
  value: string;
  onChange: (id: string) => void;
  options: string[];
  name: (id: string) => string;
}) {
  return (
    <label>
      {label}
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">全部{label}</option>
        {options.map((id) => (
          <option key={id} value={id}>
            {name(id)}
          </option>
        ))}
      </select>
    </label>
  );
}
