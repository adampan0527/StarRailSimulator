import { useId, useState } from 'react';
import { relicSlots, relicStatName, statName, statValue } from './client';
import type { Catalog, InventoryItem, RelicSummary } from './client';
import { combatTraitName } from './CombatTrait';
import { relicIcon } from './relicIcons';

function RelicCardStat({
  catalog,
  id,
  value,
  upgrades,
}: {
  catalog: Catalog;
  id: string;
  value: number;
  upgrades?: number;
}) {
  const icon = relicIcon(`stat.${id}`);
  const upgradeLabel =
    upgrades === undefined ? undefined : `强化 ${upgrades} 次`;

  return (
    <span className="relic-card-stat">
      <span className="relic-card-stat-name">
        {icon && (
          <img src={icon} alt="" width={16} height={16} loading="lazy" />
        )}
        <span>{statName(catalog, id)}</span>
      </span>{' '}
      <span className="relic-card-stat-result">
        <span className="relic-card-stat-value">
          +{statValue(id, value / 1_000_000, 3)}
        </span>{' '}
        {upgrades !== undefined && (
          <span
            className="relic-card-upgrades"
            data-count={upgrades}
            role="img"
            aria-label={upgradeLabel}
            title={upgradeLabel}
          >
            {upgrades}
          </span>
        )}
      </span>
    </span>
  );
}

function relicOwnerName(
  roster: Catalog['characters'],
  character: Catalog['characters'][number],
) {
  return roster.some(
    ({ id, name }) => id !== character.id && name === character.name,
  )
    ? `${character.name}（${combatTraitName('path', character.path)}）`
    : character.name;
}

export function RelicCard({
  catalog,
  item,
  data,
  disabled = false,
  action = 'edit',
  equipped,
  onSelect,
}: {
  catalog: Catalog;
  item: InventoryItem;
  data: RelicSummary;
  disabled?: boolean;
  action?: 'edit' | 'equip' | 'change';
  equipped?: boolean;
  onSelect: (button: HTMLButtonElement) => void;
}) {
  const [failedIcon, setFailedIcon] = useState<string>();
  const substatList = useId();
  // The bundled item artwork is indexed by the five-star variants.
  const icon =
    relicIcon(data.id) ??
    relicIcon(
      catalog.relics.find(
        (variant) =>
          variant.rarity === 5 &&
          variant.set_id === data.set_id &&
          variant.slot === data.slot,
      )?.id ?? '',
    );
  const slot = relicSlots.find(([id]) => id === data.slot)?.[1];
  const main = relicStatName(catalog, item.relic.main_stat);
  const level = `+${item.relic.level}`;
  const inLoadout = action === 'change';
  const substats = Object.entries(item.relic.substats);
  const preview = Object.entries(item.source?.preview_substats ?? {});
  const location = item.source?.location;
  const gameOwner = location
    ? catalog.characters.find(({ source_ids }) => source_ids.includes(location))
    : undefined;
  const owner = catalog.characters.find(({ id }) => id === item.owner_id);
  const details =
    action === 'equip'
      ? `${slot} · ${catalog.sets[data.set_id]?.name ?? data.set_id} · ${data.rarity} 星`
      : undefined;
  const ownerLabel =
    !inLoadout && item.owner_id && !equipped
      ? `${owner ? relicOwnerName(catalog.characters, owner) : item.owner_id}已穿戴`
      : undefined;
  const gameOwnerLabel =
    location && (!gameOwner || gameOwner.id !== item.owner_id)
      ? `游戏穿戴 · ${gameOwner ? relicOwnerName(catalog.characters, gameOwner) : `角色 ${location}`}`
      : location === '' && item.owner_id
        ? '游戏未穿戴'
        : undefined;
  const label = [
    inLoadout
      ? `${slot} · ${data.name}，选择遗器`
      : `${action === 'edit' ? '编辑' : '穿戴'}${data.name}`,
    details,
    level,
    main,
    ownerLabel,
    !inLoadout && gameOwnerLabel,
  ]
    .filter(Boolean)
    .join('，');

  return (
    <button
      type="button"
      className="relic-card"
      data-action={action}
      data-rarity={data.rarity}
      disabled={disabled}
      aria-label={label}
      aria-haspopup={inLoadout ? 'dialog' : undefined}
      aria-describedby={
        inLoadout || substats.length || preview.length ? substatList : undefined
      }
      aria-pressed={equipped}
      onClick={(event) => onSelect(event.currentTarget)}
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
      {inLoadout && <span className="relic-card-slot">{slot}</span>}
      <span className="relic-card-name">{data.name}</span>
      {details && <span className="relic-card-detail">{details}</span>}
      <span className="relic-card-main">{main}</span>
      {(inLoadout || substats.length > 0 || preview.length > 0) && (
        <span id={substatList} className="relic-card-substats">
          {inLoadout && substats.length === 0 && <span>无副词条</span>}
          {substats.map(([id, value]) => {
            const rolls = item.source?.rolls[id];
            return (
              <RelicCardStat
                key={id}
                catalog={catalog}
                id={id}
                value={value}
                upgrades={rolls ? rolls.count - 1 : undefined}
              />
            );
          })}
          {preview.length > 0 && (
            <span className="relic-card-preview">
              <span>
                {data.rarity === 5 &&
                substats.length === 3 &&
                preview.length === 1
                  ? '第 4 条预览'
                  : '未解锁预览'}
              </span>
              {preview.map(([id, value]) => (
                <RelicCardStat
                  key={id}
                  catalog={catalog}
                  id={id}
                  value={value}
                />
              ))}
            </span>
          )}
        </span>
      )}
      {ownerLabel && <span className="relic-card-detail">{ownerLabel}</span>}
      {!inLoadout && gameOwnerLabel && (
        <span className="relic-card-detail">{gameOwnerLabel}</span>
      )}
      {equipped && <span className="relic-card-equipped">当前穿戴</span>}
    </button>
  );
}
