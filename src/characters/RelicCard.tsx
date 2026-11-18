import { useId, useState } from 'react';
import { relicSlots, relicStatName, statName, statValue } from './client';
import type { Catalog, InventoryItem, RelicSummary } from './client';
import { relicIcon } from './relicIcons';

export function RelicCard({
  catalog,
  item,
  data,
  owner,
  disabled,
  onEdit,
}: {
  catalog: Catalog;
  item: InventoryItem;
  data: RelicSummary;
  owner: string | undefined;
  disabled: boolean;
  onEdit: (button: HTMLButtonElement) => void;
}) {
  const [failedIcon, setFailedIcon] = useState<string>();
  const substatList = useId();
  const icon = relicIcon(data.id);
  const slot = relicSlots.find(([id]) => id === data.slot)?.[1];
  const main = relicStatName(catalog, item.relic.main_stat);
  const level = `+${item.relic.level}`;
  const substats = Object.entries(item.relic.substats);

  return (
    <button
      type="button"
      className="relic-card"
      disabled={disabled}
      aria-label={`编辑${data.name}，${level}，${main}${owner ? `，${owner}已穿戴` : ''}`}
      aria-describedby={substats.length ? substatList : undefined}
      onClick={(event) => onEdit(event.currentTarget)}
    >
      <span className="relic-card-artwork">
        {icon && icon !== failedIcon ? (
          <img
            src={icon}
            alt=""
            width={64}
            height={64}
            loading="lazy"
            decoding="async"
            onError={() => setFailedIcon(icon)}
          />
        ) : (
          <span aria-hidden="true">{slot}</span>
        )}
        <span className="relic-card-level">{level}</span>
      </span>
      <span className="relic-card-name">{data.name}</span>
      <span className="relic-card-main">{main}</span>
      {substats.length > 0 && (
        <span id={substatList} className="relic-card-substats">
          {substats.map(([id, value]) => (
            <span key={id} className="relic-card-stat">
              <span>{statName(catalog, id)}</span>{' '}
              <span>+{statValue(id, value / 1_000_000)}</span>
            </span>
          ))}
        </span>
      )}
      {owner && <span className="relic-card-detail">{owner}已穿戴</span>}
    </button>
  );
}
