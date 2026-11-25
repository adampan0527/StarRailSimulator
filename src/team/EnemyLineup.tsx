import { useId, useRef, useState } from 'react';
import { CombatTraitIcon } from '../characters/CombatTrait';
import { EnemyArtwork, EnemyAvatar } from './EnemyArt';
import { EnemyPicker } from './EnemyPicker';
import { enemies } from './enemyCatalog';
import type { EnemySlot, EnemySlots } from './enemyFormationClient';
import preview from './enemy-preview.json';

const DEFAULT_LEVEL = 80;
const formatStat = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });

function statText(series: number | null, levelIndex: number) {
  const value =
    series === null ? undefined : preview.statSeries[series][levelIndex];
  return value === null || value === undefined ? '—' : formatStat.format(value);
}

function EnemySlotCard({
  number,
  slot,
  onPick,
  onLevelChange,
  onRemove,
  disabled,
}: {
  number: number;
  slot: EnemySlot | null;
  onPick: () => void;
  onLevelChange: (level: number) => void;
  onRemove: () => void;
  disabled: boolean;
}) {
  const pickButton = useRef<HTMLButtonElement>(null);
  const statsId = useId();
  const enemy = slot && enemies.get(slot.enemy_id);
  const name = enemy?.name ?? (slot ? `敌人 ${slot.enemy_id}` : '空位');
  const levelIndex = slot ? preview.levels.indexOf(slot.level) : -1;

  return (
    <li className="enemy-entry" data-filled={slot ? true : undefined}>
      <button
        ref={pickButton}
        type="button"
        className="choice enemy-slot"
        aria-label={`敌方站位 ${number}：${name}`}
        aria-describedby={enemy ? statsId : undefined}
        aria-haspopup="dialog"
        disabled={disabled}
        onClick={onPick}
      >
        <span className="slot-number numeric">{number}</span>
        <span className="character-card-body">
          {enemy ? (
            <EnemyAvatar file={enemy.avatar} />
          ) : (
            <span className="character-portrait" aria-hidden="true">
              <span className="portrait-fallback">+</span>
            </span>
          )}
          <strong className="enemy-name">{name}</strong>
          {enemy && (
            <span className="enemy-attributes" id={statsId}>
              <span className="enemy-weaknesses combat-trait-inline">
                <span>弱点</span>
                {enemy.weaknesses.length
                  ? enemy.weaknesses.map((id) => (
                      <CombatTraitIcon key={id} kind="element" id={id} />
                    ))
                  : '无'}
              </span>
              <span className="enemy-stats character-stats">
                {[
                  { label: '生命值', series: enemy.hp },
                  { label: '防御力', series: enemy.defense },
                  { label: '速度', series: enemy.speed },
                ].map(({ label, series }) => (
                  <span className="character-stat" key={label}>
                    <span>{label}</span>
                    <strong className="numeric">
                      {statText(series, levelIndex)}
                    </strong>
                  </span>
                ))}
              </span>
            </span>
          )}
        </span>
      </button>
      {slot && (
        <>
          <div className="enemy-slot-actions">
            <select
              aria-label={`敌方站位 ${number} 等级`}
              value={slot.level}
              disabled={disabled}
              onChange={(event) => onLevelChange(Number(event.target.value))}
            >
              {preview.levels.map((value) => (
                <option key={value} value={value}>
                  {value} 级
                </option>
              ))}
            </select>
            <button
              type="button"
              className="secondary"
              disabled={disabled}
              aria-label={`移除敌方站位 ${number}`}
              onClick={() => {
                pickButton.current?.focus();
                onRemove();
              }}
            >
              移除
            </button>
          </div>
          <div className="enemy-slot-art">
            <EnemyArtwork file={enemy?.artwork ?? null} name={name} />
          </div>
        </>
      )}
    </li>
  );
}

export default function EnemyLineup({
  lineup,
  onChange,
  disabled,
}: {
  lineup: EnemySlots;
  onChange: (lineup: EnemySlots) => void;
  disabled: boolean;
}) {
  const [picking, setPicking] = useState<number>();
  const count = lineup.filter((slot) => slot !== null).length;

  function updateSlot(
    position: number,
    update: (slot: EnemySlot | null) => EnemySlot | null,
  ) {
    const next: EnemySlots = [...lineup];
    next[position] = update(next[position]);
    onChange(next);
  }

  return (
    <section
      className="field-zone enemy-zone"
      aria-labelledby="team-enemy-title"
    >
      <div className="section-heading">
        <h3 id="team-enemy-title">敌方</h3>
        <span className="tag numeric">
          {count} / {lineup.length}
        </span>
      </div>
      <ol className="enemy-lineup" aria-label="敌方站位">
        {lineup.map((slot, position) => (
          <EnemySlotCard
            key={position}
            number={position + 1}
            slot={slot}
            disabled={disabled}
            onPick={() => setPicking(position)}
            onLevelChange={(level) =>
              updateSlot(
                position,
                (current) => current && { ...current, level },
              )
            }
            onRemove={() => updateSlot(position, () => null)}
          />
        ))}
      </ol>
      {picking !== undefined && (
        <EnemyPicker
          position={picking}
          selected={lineup[picking]?.enemy_id}
          onSelect={(id) => {
            const enemy = enemies.get(id);
            if (enemy) {
              updateSlot(picking, (current) => ({
                enemy_id: enemy.id,
                level: current?.level ?? DEFAULT_LEVEL,
              }));
            }
          }}
          onClose={() => setPicking(undefined)}
        />
      )}
    </section>
  );
}
