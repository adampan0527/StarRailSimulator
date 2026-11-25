import { useEffect, useRef, useState } from 'react';
import { CharacterPortrait } from '../characters/CharacterCard';
import { Resource } from './Battle';
import {
  actionName,
  av,
  eventDetail,
  eventTitle,
  outcomeNames,
} from './Timeline';
import type {
  BattleEvent,
  CharacterSummary,
  Command,
  Name,
  PlannedReplay,
  Unit,
} from './client';
import './planned.css';

const summaryEvents = new Set([
  'damage',
  'weakness_broken',
  'unit_defeated',
  'battle_ended',
]);

export function PlannedBattle({
  commands,
  replay,
  names,
  profiles,
}: {
  commands: Command[];
  replay: PlannedReplay;
  names: Name[];
  profiles: CharacterSummary[];
}) {
  const [step, setStep] = useState(0);
  const [continuous, setContinuous] = useState(false);
  const order = useRef<HTMLOListElement>(null);
  const pending = useRef<HTMLLIElement>(null);
  const { frames, events } = replay;
  const frame = frames[step];
  const next = commands[step];
  const playing = continuous && step < commands.length;

  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => {
      setStep((current) => Math.min(current + 1, commands.length));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [playing, commands.length]);

  useEffect(() => {
    const list = order.current;
    const row = pending.current;
    if (!list || !row) return;
    const viewport = list.getBoundingClientRect();
    const position = row.getBoundingClientRect();
    if (position.top < viewport.top)
      list.scrollTop -= viewport.top - position.top;
    if (position.bottom > viewport.bottom)
      list.scrollTop += position.bottom - viewport.bottom;
  }, [step]);

  const byId = new Map(names.map((entry) => [entry.id, entry]));
  const unitName = (id: string) => byId.get(id)?.name ?? id;
  const allSettled = events.slice(frames[0].event_count, frame.event_count);
  const lastSettled =
    step === 0
      ? []
      : events.slice(frames[step - 1].event_count, frame.event_count);
  const highlights = lastSettled.filter((event) =>
    summaryEvents.has(event.kind.event),
  );
  const summary = highlights.length ? highlights : lastSettled.slice(0, 1);
  const { snapshot } = frame;

  function go(offset: number) {
    setContinuous(false);
    setStep((current) =>
      Math.max(0, Math.min(current + offset, commands.length)),
    );
  }

  const portrait = (unit: Unit) => {
    const character = profiles.find(
      (profile) => profile.id === byId.get(unit.unit_id)?.character_id,
    );
    return (
      <li
        key={unit.unit_id}
        data-active={next?.actor_id === unit.unit_id || undefined}
        data-defeated={unit.hp === 0 || undefined}
      >
        <div
          role="img"
          aria-label={unitName(unit.unit_id)}
          title={unitName(unit.unit_id)}
        >
          {unit.is_ally ? (
            <CharacterPortrait character={character} />
          ) : (
            <span className="planned-target" aria-hidden="true">
              {unitName(unit.unit_id).slice(0, 1)}
            </span>
          )}
        </div>
        <Resource name="生命" value={unit.hp} max={unit.max_hp} />
        {unit.max_energy_milli > 0 && (
          <Resource
            name="能量"
            value={unit.energy_milli / 1000}
            max={unit.max_energy_milli / 1000}
          />
        )}
        {unit.max_toughness > 0 && (
          <Resource
            name="韧性"
            value={unit.toughness}
            max={unit.max_toughness}
          />
        )}
      </li>
    );
  };

  return (
    <section className="planned-replay" aria-label="方案回放">
      <div className="section-heading">
        <h3>方案回放</h3>
        <p className="numeric">
          第 {snapshot.wave_index + 1} 波 · {av(snapshot.av_micros)} AV · 战技点{' '}
          {snapshot.skill_points} / {snapshot.skill_point_cap ?? 5}
          {snapshot.outcome !== 'ongoing' &&
            ` · ${outcomeNames[snapshot.outcome]} · 有效伤害 ${snapshot.enemy_hp_lost}`}
        </p>
      </div>
      <div className="planned-layout">
        <div className="planned-field" aria-label="战场状态">
          <section className="planned-side planned-enemies" aria-label="敌方">
            <h3>敌方</h3>
            <ul>
              {snapshot.units.filter((unit) => !unit.is_ally).map(portrait)}
            </ul>
          </section>
          <div className="planned-result" role="status">
            <strong>{step === 0 ? '开局状态' : '上一步结果'}</strong>
            {summary.map((event, index) => (
              <Settlement key={index} event={event} unitName={unitName} />
            ))}
          </div>
          <section className="planned-side planned-allies" aria-label="己方">
            <h3>己方</h3>
            <ul>
              {snapshot.units.filter((unit) => unit.is_ally).map(portrait)}
            </ul>
          </section>
        </div>
        <aside className="planned-order" aria-label="行动顺序">
          <h3>行动顺序</h3>
          <ol ref={order}>
            {commands.map((command, index) => (
              <li
                key={command.decision_index}
                ref={index === step ? pending : undefined}
                aria-current={index === step ? 'step' : undefined}
                data-complete={index < step || undefined}
              >
                <div className="planned-action-heading">
                  <strong>{unitName(command.actor_id)}</strong>
                  <span className="muted numeric">
                    {av(frames[index].snapshot.av_micros)} AV
                  </span>
                </div>
                <div>
                  {actionName(command.action, command.actor_id)}
                  {command.target_id && ` → ${unitName(command.target_id)}`}
                </div>
                {index <= step && (
                  <div className="planned-action-state">
                    {index === step ? '待执行' : '已执行'}
                  </div>
                )}
              </li>
            ))}
          </ol>
        </aside>
      </div>
      <div className="form-actions">
        <button
          type="button"
          className="secondary"
          disabled={step === 0}
          onClick={() => go(-1)}
        >
          上一步
        </button>
        <button
          type="button"
          onClick={() => {
            if (step === commands.length) setStep(0);
            setContinuous(!playing);
          }}
        >
          {playing ? '暂停回放' : '播放回放'}
        </button>
        <button
          type="button"
          className="secondary"
          disabled={step === commands.length}
          onClick={() => go(1)}
        >
          下一步
        </button>
        <span className="muted numeric">
          已执行 {step} / {commands.length} 步
        </span>
      </div>
      <details className="planned-log">
        <summary>结算记录</summary>
        <ol>
          {allSettled.map((event, index) => (
            <li key={index}>
              <span className="numeric">{av(event.av_micros)} AV · </span>
              <Settlement event={event} unitName={unitName} />
            </li>
          ))}
        </ol>
      </details>
    </section>
  );
}

function Settlement({
  event,
  unitName,
}: {
  event: BattleEvent;
  unitName: (id: string) => string;
}) {
  const actors = [event.source_id, event.target_id]
    .filter((id): id is string => id !== null)
    .map(unitName)
    .join(' → ');
  const detail =
    event.kind.event === 'battle_ended'
      ? outcomeNames[event.kind.detail]
      : eventDetail(event);
  return (
    <span>
      {[eventTitle(event), actors, detail].filter(Boolean).join(' · ')}
    </span>
  );
}
