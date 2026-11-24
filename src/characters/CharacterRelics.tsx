import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { relicSlots } from './client';
import type {
  Catalog,
  CharacterEdit,
  InventoryItem,
  RelicSlot,
} from './client';
import { RelicCard } from './RelicCard';
import { RelicInventoryFilters } from './RelicInventoryFilters';
import { emptyRelicFilters, filterRelics } from './relicInventory';
import type { RelicFilters, RelicInventoryEntry } from './relicInventory';

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
  const slots = useRef<HTMLUListElement>(null);
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
      <ul ref={slots} className="equipped-relic-grid" role="list">
        {relicSlots.map(([slot, label]) => {
          const equipped = entries.find(
            ({ item }) => item.id === equipment[slot],
          );
          return (
            <li key={slot} data-slot={slot}>
              {equipped ? (
                <RelicCard
                  catalog={catalog}
                  item={equipped.item}
                  data={equipped.data}
                  action="change"
                  onSelect={() => setOpenSlot(slot)}
                />
              ) : (
                <button
                  type="button"
                  className="choice relic-slot"
                  aria-haspopup="dialog"
                  aria-label={`${label} · 空位，选择遗器`}
                  onClick={() => setOpenSlot(slot)}
                >
                  <span className="muted">{label}</span>
                  空位
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {openSlot && (
        <RelicPicker
          initialSlot={openSlot}
          catalog={catalog}
          entries={entries}
          characterId={characterId}
          equipment={equipment}
          onEquip={onEquip}
          onClose={() => {
            setOpenSlot(undefined);
            slots.current
              ?.querySelector<HTMLButtonElement>(
                `[data-slot="${openSlot}"] > button`,
              )
              ?.focus();
          }}
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
  entries: RelicInventoryEntry[];
  characterId: string;
  equipment: Equipment;
  onEquip: (slot: RelicSlot, id: number | null) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const title = useId();
  const [filters, setFilters] = useState<RelicFilters>({
    ...emptyRelicFilters,
    slots: [initialSlot],
  });
  const visible = filterRelics(entries, filters);
  const removable = relicSlots.filter(
    ([slot]) => filters.slots.includes(slot) && equipment[slot] !== undefined,
  );

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
      className="picker-dialog relic-picker-dialog"
      aria-labelledby={title}
      onClose={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
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
      {entries.length > 0 && (
        <RelicInventoryFilters
          catalog={catalog}
          entries={entries}
          filters={filters}
          onChange={setFilters}
        />
      )}
      <div className="section-heading relic-results-heading">
        <p className="muted" role="status">
          {entries.length === 0
            ? '库存为空，请先到「遗器」页录入。'
            : visible.length
              ? `找到 ${visible.length} 件遗器`
              : '没有符合筛选的遗器。'}
        </p>
        {removable.length > 0 && (
          <div className="relic-picker-actions">
            {removable.map(([slot, label]) => (
              <button
                key={slot}
                type="button"
                className="secondary"
                onClick={() => choose(slot, null)}
              >
                卸下{label}遗器
              </button>
            ))}
          </div>
        )}
      </div>
      {visible.length > 0 && (
        <ul className="relic-inventory-grid" role="list">
          {visible.map(({ item, data }) => {
            const otherOwner =
              item.owner_id !== characterId ? item.owner_id : null;
            return (
              <li key={item.id}>
                <RelicCard
                  catalog={catalog}
                  item={item}
                  data={data}
                  selectedSubstats={filters.substats}
                  disabled={otherOwner !== null}
                  action="equip"
                  equipped={equipment[data.slot] === item.id}
                  onSelect={() => choose(data.slot, item.id)}
                />
              </li>
            );
          })}
        </ul>
      )}
    </dialog>
  );
}
