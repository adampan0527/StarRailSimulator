import { useEffect, useId, useRef, useState } from 'react';
import type { CharacterSummary } from '../training/client';
import { CombatTraitFilter } from './CombatTrait';
import { LightConeCard } from './LightConeCard';
import type {
  Catalog,
  InventoryLightCone,
  LightCone,
  LightConeEquipment,
  LightConeSummary,
} from './client';
import { compareLightConeRelease } from './releaseOrder';

const tabs = [
  ['custom', '自选光锥'],
  ['inventory', '光锥库'],
] as const;

type Source = LightConeEquipment['source'];
type LightConeProps = {
  catalog: Catalog;
  active: CharacterSummary;
  inventory: InventoryLightCone[];
  saved: LightConeEquipment | null;
  value: LightConeEquipment | null;
  onChange: (cone: LightConeEquipment | null) => void;
};

export function CharacterLightCone(props: LightConeProps) {
  const { catalog, active, inventory, value } = props;
  const [open, setOpen] = useState(false);
  const selected =
    value?.source === 'custom'
      ? value.value
      : inventory.find(({ id }) => id === value?.value)?.cone;
  const data = catalog.light_cones.find(
    (cone) => cone.default.data_id === selected?.data_id,
  );

  return (
    <div className="field-stack">
      <h3>光锥</h3>
      <button
        type="button"
        className="choice light-cone-preview"
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
      >
        <strong>选择光锥</strong>
        {selected ? (
          <LightConeCard data={data} dataId={selected.data_id}>
            Lv. {selected.level} · S{selected.superimposition} ·{' '}
            {value?.source === 'custom' ? '自选' : '光锥库'}
          </LightConeCard>
        ) : (
          <span className="muted">
            {value?.source === 'inventory'
              ? `光锥 #${value.value} 已不在库中`
              : '未装备'}
          </span>
        )}
      </button>
      {data && data.path !== active.path && (
        <p className="muted">
          光锥提供基础属性，命途相同时可获得固定光锥加成。
        </p>
      )}
      {open && <LightConePicker {...props} onClose={() => setOpen(false)} />}
    </div>
  );
}

type ConeChoice = {
  data: LightConeSummary;
  cone: LightCone;
  owned?: InventoryLightCone;
};

function LightConePicker({
  catalog,
  active,
  inventory,
  saved,
  value,
  onChange,
  onClose,
}: LightConeProps & { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const tabList = useRef<HTMLDivElement>(null);
  const id = useId();
  const [source, setSource] = useState<Source>(value?.source ?? 'custom');
  const [path, setPath] = useState('');
  const [rank, setRank] = useState(
    value?.source === 'custom' ? value.value.superimposition : 1,
  );

  const definitions = new Map(
    catalog.light_cones.map((data) => [data.default.data_id, data]),
  );
  const owned: ConeChoice[] = inventory.flatMap((item) => {
    const data = definitions.get(item.cone.data_id);
    return data ? [{ data, cone: item.cone, owned: item }] : [];
  });
  const choices: ConeChoice[] =
    source === 'custom'
      ? catalog.light_cones.map((data) => ({
          data,
          cone: data.default,
        }))
      : owned;
  const visible = choices
    .filter(({ data }) => !path || data.path === path)
    .sort((first, second) => compareLightConeRelease(first.cone, second.cone));

  useEffect(() => {
    dialog.current?.showModal();
    tabList.current
      ?.querySelector<HTMLButtonElement>('[aria-selected="true"]')
      ?.focus();
  }, []);

  function choose(equipment: LightConeEquipment | null) {
    onChange(equipment);
    dialog.current?.close();
  }

  function chooseCone({ cone, owned }: ConeChoice) {
    if (owned) {
      choose({ source: 'inventory', value: owned.id });
      return;
    }
    const retained = [value, saved].find(
      (equipment) =>
        equipment?.source === 'custom' &&
        equipment.value.data_id === cone.data_id,
    );
    const config = retained?.source === 'custom' ? retained.value : cone;
    choose({ source: 'custom', value: { ...config, superimposition: rank } });
  }

  return (
    <dialog
      ref={dialog}
      className="picker-dialog light-cone-dialog"
      aria-labelledby={`${id}-title`}
      onClose={onClose}
    >
      <div className="section-heading">
        <h2 id={`${id}-title`}>选择光锥</h2>
        <button
          type="button"
          className="secondary"
          onClick={() => dialog.current?.close()}
        >
          关闭
        </button>
      </div>
      <div
        className="light-cone-tabs"
        ref={tabList}
        role="tablist"
        aria-label="光锥来源"
        onKeyDown={(event) => {
          let next: Source;
          switch (event.key) {
            case 'ArrowLeft':
            case 'ArrowRight':
              next = source === 'custom' ? 'inventory' : 'custom';
              break;
            case 'Home':
              next = 'custom';
              break;
            case 'End':
              next = 'inventory';
              break;
            default:
              return;
          }
          event.preventDefault();
          setSource(next);
          tabList.current
            ?.querySelector<HTMLButtonElement>(`[data-source="${next}"]`)
            ?.focus();
        }}
      >
        {tabs.map(([key, name]) => (
          <button
            type="button"
            key={key}
            role="tab"
            id={`${id}-${key}-tab`}
            data-source={key}
            aria-controls={`${id}-${key}-panel`}
            aria-selected={source === key}
            tabIndex={source === key ? 0 : -1}
            onClick={() => setSource(key)}
          >
            {name}
          </button>
        ))}
      </div>
      {tabs.map(([key]) => (
        <div
          key={key}
          role="tabpanel"
          id={`${id}-${key}-panel`}
          aria-labelledby={`${id}-${key}-tab`}
          hidden={source !== key}
          className="light-cone-panel"
        >
          {source === key && (
            <>
              <div className="light-cone-filters">
                <CombatTraitFilter
                  kind="path"
                  options={catalog.light_cones.map((cone) => cone.path)}
                  value={path}
                  onChange={setPath}
                />
                {source === 'custom' && (
                  <LightConeRank value={rank} onChange={setRank} />
                )}
              </div>
              <div className="section-heading">
                <p className="muted" role="status">
                  {source === 'inventory' && inventory.length === 0
                    ? '光锥库为空，请先在顶部「光锥库」录入。'
                    : visible.length
                      ? `找到 ${visible.length} 件光锥`
                      : '没有符合筛选的光锥。'}
                </p>
                {value && (
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => choose(null)}
                  >
                    卸下光锥
                  </button>
                )}
              </div>
              {visible.length > 0 && (
                <ul className="light-cone-results">
                  {visible.map((choice) => {
                    const { data, cone, owned } = choice;
                    const occupied =
                      owned?.owner_id != null && owned.owner_id !== active.id;
                    const owner = occupied
                      ? (catalog.characters.find(
                          ({ id }) => id === owned.owner_id,
                        )?.name ?? owned.owner_id)
                      : null;
                    const selected = owned
                      ? value?.source === 'inventory' &&
                        value.value === owned.id
                      : value?.source === 'custom' &&
                        value.value.data_id === cone.data_id;
                    return (
                      <li key={owned?.id ?? cone.data_id}>
                        <button
                          type="button"
                          className="light-cone-card-button"
                          disabled={occupied}
                          aria-pressed={selected}
                          onClick={() => chooseCone(choice)}
                        >
                          <LightConeCard data={data} dataId={cone.data_id}>
                            {owned && (
                              <>
                                Lv. {cone.level} · S{cone.superimposition} · #
                                {owned.id}
                                {owner && ` · ${owner}已装备`}
                              </>
                            )}
                          </LightConeCard>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </>
          )}
        </div>
      ))}
    </dialog>
  );
}

export function LightConeRank({
  value,
  onChange,
}: {
  value: number;
  onChange: (rank: number) => void;
}) {
  return (
    <label>
      叠影
      <select
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      >
        {[1, 2, 3, 4, 5].map((rank) => (
          <option key={rank} value={rank}>
            S{rank}
          </option>
        ))}
      </select>
    </label>
  );
}

export function sameLightCone(
  a: LightConeEquipment | null,
  b: LightConeEquipment | null,
) {
  if (a?.source === 'custom' && b?.source === 'custom') {
    return (
      a.value.data_id === b.value.data_id &&
      a.value.level === b.value.level &&
      a.value.ascension === b.value.ascension &&
      a.value.superimposition === b.value.superimposition
    );
  }
  return a?.source === b?.source && a?.value === b?.value;
}
