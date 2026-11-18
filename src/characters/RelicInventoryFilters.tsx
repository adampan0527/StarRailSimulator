import { useEffect, useId, useRef, useState } from 'react';
import { innerRelicSlots, outerRelicSlots, relicStatName } from './client';
import type { Catalog } from './client';
import { relicIcon } from './relicIcons';
import {
  compareRelicSetRelease,
  compareRelicStatOrder,
  emptyRelicFilters,
} from './relicInventory';
import type { RelicFilters, RelicInventoryEntry } from './relicInventory';

const groups = [
  { key: 'mainStats', label: '主词条' },
  { key: 'substats', label: '副词条' },
  { key: 'sets', label: '遗器套装' },
  { key: 'slots', label: '遗器部位' },
] as const;

type FilterGroup = (typeof groups)[number];
type FilterOptions = {
  label?: string;
  options: { id: string; name: string; icon?: string }[];
};

function filterOptions(
  key: FilterGroup['key'],
  catalog: Catalog,
  entries: RelicInventoryEntry[],
  selected: string[],
): FilterOptions[] {
  if (key === 'slots') {
    return [
      {
        label: '外圈遗器',
        options: outerRelicSlots.map(([id, name]) => ({ id, name })),
      },
      {
        label: '内圈饰品',
        options: innerRelicSlots.map(([id, name]) => ({ id, name })),
      },
    ];
  }

  const itemIcons =
    key === 'sets'
      ? new Map(entries.map(({ data }) => [data.set_id, relicIcon(data.id)]))
      : undefined;
  const ids = itemIcons
    ? [...itemIcons.keys()]
    : key === 'mainStats'
      ? entries.map(({ item }) => item.relic.main_stat)
      : entries.flatMap(({ item }) => Object.keys(item.relic.substats));
  const options = [...new Set([...ids, ...selected])].map((id) => ({
    id,
    name: itemIcons
      ? (catalog.sets[id]?.name ?? id)
      : relicStatName(catalog, id),
    icon: itemIcons
      ? (relicIcon(id) ?? itemIcons.get(id))
      : relicIcon(`stat.${id}`),
  }));
  options.sort((a, b) =>
    itemIcons
      ? compareRelicSetRelease(a.id, b.id)
      : compareRelicStatOrder(a.id, b.id) ||
        a.name.localeCompare(b.name, 'zh-CN'),
  );
  return [{ options }];
}

export function RelicInventoryFilters({
  catalog,
  entries,
  filters,
  disabled,
  onChange,
}: {
  catalog: Catalog;
  entries: RelicInventoryEntry[];
  filters: RelicFilters;
  disabled: boolean;
  onChange: (filters: RelicFilters) => void;
}) {
  const [active, setActive] = useState<FilterGroup>();
  const trigger = useRef<HTMLButtonElement>(null);
  const canReset =
    groups.some(({ key }) => filters[key].length > 0) || filters.minimum !== 1;

  return (
    <>
      <fieldset className="relic-inventory-filters" disabled={disabled}>
        <legend>筛选遗器</legend>
        <div className="relic-filter-groups">
          {groups.map((group) => {
            const count = filters[group.key].length;
            return (
              <button
                key={group.key}
                type="button"
                className="secondary relic-filter-trigger"
                data-filtered={count > 0}
                aria-haspopup="dialog"
                onClick={(event) => {
                  trigger.current = event.currentTarget;
                  setActive(group);
                }}
              >
                {group.label}
                <span>{count ? `已选 ${count}` : '不限'}</span>
              </button>
            );
          })}
        </div>
        <button
          type="button"
          className="secondary"
          disabled={!canReset}
          onClick={() => onChange(emptyRelicFilters)}
        >
          清除筛选
        </button>
      </fieldset>
      {active && (
        <RelicFilterDialog
          group={active}
          sections={filterOptions(
            active.key,
            catalog,
            entries,
            filters[active.key],
          )}
          filters={filters}
          onApply={onChange}
          onClose={() => {
            setActive(undefined);
            trigger.current?.focus();
          }}
        />
      )}
    </>
  );
}

function RelicFilterDialog({
  group: { key, label },
  sections,
  filters,
  onApply,
  onClose,
}: {
  group: FilterGroup;
  sections: FilterOptions[];
  filters: RelicFilters;
  onApply: (filters: RelicFilters) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const title = useId();
  const [selected, setSelected] = useState(filters[key]);
  const [minimum, setMinimum] = useState(filters.minimum);
  const matchingMinimum = Math.min(minimum, selected.length);

  useEffect(() => {
    const modal = dialog.current;
    modal?.showModal();
    modal?.querySelector<HTMLInputElement>('input[type="checkbox"]')?.focus();
  }, []);

  function toggleOptions(ids: string[], checked: boolean) {
    setSelected((current) =>
      checked
        ? [...new Set([...current, ...ids])]
        : current.filter((id) => !ids.includes(id)),
    );
  }

  return (
    <dialog
      ref={dialog}
      className="picker-dialog relic-filter-dialog"
      aria-labelledby={title}
      onClose={onClose}
    >
      <form
        className="relic-filter-form"
        onSubmit={(event) => {
          event.preventDefault();
          onApply({ ...filters, [key]: selected, minimum });
          dialog.current?.close();
        }}
      >
        <h2 id={title}>筛选{label}</h2>
        <div className="relic-filter-body">
          {sections.map((section) => (
            <fieldset
              key={section.label ?? label}
              aria-label={section.label ? undefined : label}
            >
              {section.label && (
                <RelicSlotGroupLegend
                  label={section.label}
                  ids={section.options.map(({ id }) => id)}
                  selected={selected}
                  onToggle={toggleOptions}
                />
              )}
              <div className="relic-filter-options">
                {section.options.map(({ id, name, icon }) => (
                  <label key={id} className="check-label relic-filter-option">
                    <input
                      type="checkbox"
                      checked={selected.includes(id)}
                      onChange={(event) =>
                        toggleOptions([id], event.target.checked)
                      }
                    />
                    {icon && (
                      <img
                        className={
                          key === 'sets'
                            ? 'relic-filter-set-icon'
                            : 'relic-filter-stat-icon'
                        }
                        src={icon}
                        alt=""
                        width={32}
                        height={32}
                      />
                    )}
                    <span>{name}</span>
                  </label>
                ))}
              </div>
              {section.options.length === 0 && (
                <p className="muted">库存中没有可选{label}。</p>
              )}
            </fieldset>
          ))}
          {key === 'substats' && (
            <div className="relic-filter-minimum">
              <label>
                满足条数
                <select
                  value={minimum}
                  onChange={(event) => setMinimum(Number(event.target.value))}
                >
                  {Array.from(
                    { length: Math.max(4, selected.length, minimum) },
                    (_, index) => (
                      <option key={index + 1} value={index + 1}>
                        {index + 1} 条
                      </option>
                    ),
                  )}
                </select>
              </label>
              <p className="muted" role="status">
                {matchingMinimum
                  ? `至少匹配 ${matchingMinimum} 条已选副词条`
                  : '勾选副词条后生效'}
              </p>
            </div>
          )}
        </div>
        <div className="relic-filter-actions">
          <button
            type="button"
            className="secondary"
            onClick={() => dialog.current?.close()}
          >
            取消
          </button>
          <button type="submit">应用筛选</button>
        </div>
      </form>
    </dialog>
  );
}

function RelicSlotGroupLegend({
  label,
  ids,
  selected,
  onToggle,
}: {
  label: string;
  ids: string[];
  selected: string[];
  onToggle: (ids: string[], checked: boolean) => void;
}) {
  const count = ids.filter((id) => selected.includes(id)).length;
  return (
    <legend>
      <label className="check-label relic-filter-slot-group">
        <input
          type="checkbox"
          checked={count === ids.length}
          ref={(input) => {
            if (input) input.indeterminate = count > 0 && count < ids.length;
          }}
          onChange={(event) => onToggle(ids, event.target.checked)}
        />
        {label}
      </label>
    </legend>
  );
}
