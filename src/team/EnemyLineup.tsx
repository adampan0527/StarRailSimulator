import { useRef, useState } from 'react';
import { CombatTraitIcon } from '../characters/CombatTrait';
import { EnemyArtwork, EnemyAvatar } from './EnemyArt';
import { EnemyPicker } from './EnemyPicker';
import preview from './enemy-preview.json';

const MAX_ENEMIES = 5;
const DEFAULT_LEVEL = 80;
const formatStat = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });
const enemies = new Map(preview.enemies.map((enemy) => [enemy.id, enemy]));

function statText(series: number | null, levelIndex: number) {
  const value = series === null ? null : preview.statSeries[series][levelIndex];
  return value === null ? '—' : formatStat.format(value);
}

export default function EnemyLineup() {
  const [lineup, setLineup] = useState<
    { slot: number; id: string; level: number }[]
  >([]);
  const [picking, setPicking] = useState<number>();
  const nextSlot = useRef(0);
  const focusSlot = useRef<number>(null);
  const addButton = useRef<HTMLButtonElement>(null);

  function selectEnemy(position: number, id: string) {
    if (position === lineup.length) {
      const slot = nextSlot.current++;
      focusSlot.current = slot;
      setLineup((current) => [...current, { slot, id, level: DEFAULT_LEVEL }]);
    } else {
      setLineup((current) =>
        current.map((entry, index) =>
          index === position ? { ...entry, id } : entry,
        ),
      );
    }
  }

  function setEnemyLevel(slot: number, level: number) {
    setLineup((current) =>
      current.map((entry) =>
        entry.slot === slot ? { ...entry, level } : entry,
      ),
    );
  }

  function removeEnemy(slot: number, position: number) {
    const remaining = lineup.filter((entry) => entry.slot !== slot);
    const next = remaining[position] ?? remaining.at(-1);
    if (next) focusSlot.current = next.slot;
    else addButton.current?.focus();
    setLineup(remaining);
  }

  return (
    <section
      className="field-zone enemy-zone"
      aria-labelledby="team-enemy-title"
    >
      <h3 id="team-enemy-title">敌方</h3>
      {lineup.length > 0 && (
        <ol className="enemy-lineup" aria-label="敌方站位">
          {lineup.map(({ slot, id, level }, position) => {
            const enemy = enemies.get(id);
            if (!enemy) return null;
            const levelIndex = preview.levels.indexOf(level);

            return (
              <li className="enemy-entry" key={slot}>
                <div className="enemy-entry-controls">
                  <button
                    type="button"
                    className="choice enemy-slot"
                    ref={(button) => {
                      if (button && focusSlot.current === slot) {
                        button.focus();
                        focusSlot.current = null;
                      }
                    }}
                    aria-label={`敌方站位 ${position + 1}：${enemy.name}`}
                    aria-haspopup="dialog"
                    onClick={() => setPicking(position)}
                  >
                    <span className="enemy-card-body">
                      <EnemyAvatar file={enemy.avatar} />
                      <span className="enemy-card-label">
                        <strong>{enemy.name}</strong>
                        <span className="muted">{enemy.rank}</span>
                      </span>
                    </span>
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    aria-label={`移除敌方站位 ${position + 1}`}
                    onClick={() => removeEnemy(slot, position)}
                  >
                    移除
                  </button>
                </div>
                <label className="enemy-level enemy-slot-level">
                  等级
                  <select
                    aria-label={`敌方站位 ${position + 1} 等级`}
                    value={level}
                    onChange={(event) =>
                      setEnemyLevel(slot, Number(event.target.value))
                    }
                  >
                    {preview.levels.map((value) => (
                      <option key={value} value={value}>
                        {value} 级
                      </option>
                    ))}
                  </select>
                </label>
                <dl className="enemy-stats">
                  <div>
                    <dt>生命</dt>
                    <dd>{statText(enemy.hp, levelIndex)}</dd>
                  </div>
                  <div>
                    <dt>防御</dt>
                    <dd>{statText(enemy.defense, levelIndex)}</dd>
                  </div>
                  <div>
                    <dt>速度</dt>
                    <dd>{statText(enemy.speed, levelIndex)}</dd>
                  </div>
                  <div>
                    <dt>弱点</dt>
                    <dd className="enemy-weaknesses">
                      {enemy.weaknesses.length
                        ? enemy.weaknesses.map((id) => (
                            <CombatTraitIcon key={id} kind="element" id={id} />
                          ))
                        : '无'}
                    </dd>
                  </div>
                </dl>
                <EnemyArtwork file={enemy.artwork} name={enemy.name} />
              </li>
            );
          })}
        </ol>
      )}
      <div className="form-actions">
        <button
          ref={addButton}
          type="button"
          className="secondary"
          aria-haspopup="dialog"
          disabled={lineup.length >= MAX_ENEMIES}
          onClick={() => setPicking(lineup.length)}
        >
          添加敌人
        </button>
        {lineup.length >= MAX_ENEMIES && (
          <span role="status">已添加 5 个敌方站位。</span>
        )}
      </div>
      {picking !== undefined && (
        <EnemyPicker
          position={picking}
          selected={lineup[picking]?.id}
          onSelect={(id) => selectEnemy(picking, id)}
          onClose={() => setPicking(undefined)}
        />
      )}
    </section>
  );
}
