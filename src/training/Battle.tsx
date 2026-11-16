import { useId, useRef, useState } from 'react';
import type { BattleView, Command, Unit } from './client';
import { actionNames, av, outcomeNames, Timeline } from './Timeline';

type Props = {
  battle: BattleView;
  playing: boolean;
  busy?: boolean;
  issue?: string;
  onAct?: (command: Command) => void;
};

export function BattlePanel({ battle, playing, busy, issue, onAct }: Props) {
  const { id, snapshot, order, events, legal_actions, names } = battle;
  const titleId = useId();
  const decisionHeading = useRef<HTMLHeadingElement>(null);
  const unitName = (id: string) =>
    names.find((entry) => entry.id === id)?.name ?? id;
  const activeName =
    snapshot.ordinary_actor_id && unitName(snapshot.ordinary_actor_id);
  const actorLabel =
    activeName && snapshot.outcome === 'ongoing'
      ? `${activeName}的回合`
      : outcomeNames[snapshot.outcome];

  return (
    <div className="battle-layout">
      <section className="panel battle-status" aria-labelledby={titleId}>
        <div className="section-heading">
          <h2 id={titleId}>
            {playing ? '训练战斗' : '训练回放'}{' '}
            <span className="muted numeric">#{id}</span>
          </h2>
          <span className={`tag outcome-${snapshot.outcome}`}>
            {outcomeNames[snapshot.outcome]}
          </span>
        </div>
        <div className="battle-summary" role="status">
          <strong>{actorLabel}</strong>
          <span className="numeric">
            <span className="muted">战技点</span> {snapshot.skill_points} / 5
          </span>
          <span className="numeric">
            <span className="muted">行动值</span> {av(snapshot.av_micros)} AV
          </span>
          {snapshot.outcome !== 'ongoing' && (
            <span className="numeric">
              <span className="muted">有效伤害</span> {snapshot.enemy_hp_lost}
            </span>
          )}
        </div>
        {order.length > 0 && (
          <div className="turn-order">
            <h3>行动顺序</h3>
            <ol>
              {order.map(({ unit_id, remaining_av_micros }, index) => (
                <li
                  key={unit_id}
                  className={index === 0 ? 'current-turn' : undefined}
                >
                  <span>{unitName(unit_id)}</span>
                  <span className="numeric">
                    {index === 0 ? '当前' : `+${av(remaining_av_micros)} AV`}
                  </span>
                </li>
              ))}
            </ol>
          </div>
        )}
      </section>

      <div className="battle-grid">
        <section className="panel" aria-label="战场单位">
          <Roster
            title="敌方"
            units={snapshot.units.filter((unit) => !unit.is_ally)}
            unitName={unitName}
          />
          <Roster
            title="己方"
            units={snapshot.units.filter((unit) => unit.is_ally)}
            unitName={unitName}
          />
        </section>
        {playing && onAct && (
          <section className="panel">
            <h3 ref={decisionHeading} tabIndex={-1}>
              行动指令
            </h3>
            {legal_actions.length > 0 ? (
              <ActionForm
                key={`${id}-${snapshot.decision_index}`}
                actions={legal_actions}
                unitName={unitName}
                busy={busy ?? false}
                onAct={(command) => {
                  decisionHeading.current?.focus({ preventScroll: true });
                  onAct(command);
                }}
              />
            ) : (
              <p>本场战斗已结束。可以在历史回放中查看每一步。</p>
            )}
            {issue && <p role="alert">{issue}</p>}
          </section>
        )}
      </div>
      <Timeline key={id} events={events} unitName={unitName} />
    </div>
  );
}

function Roster({
  title,
  units,
  unitName,
}: {
  title: string;
  units: Unit[];
  unitName: (id: string) => string;
}) {
  return (
    <div className="roster">
      <h3>{title}</h3>
      <ul>
        {units.map((unit) => (
          <li
            key={unit.unit_id}
            className={unit.hp === 0 ? 'unit-defeated' : undefined}
          >
            <div className="unit-heading">
              <strong>{unitName(unit.unit_id)}</strong>
              {unit.hp === 0 ? (
                <span>已退场</span>
              ) : (
                <span className="numeric">速度 {av(unit.speed_micros)}</span>
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
        ))}
      </ul>
    </div>
  );
}

function Resource({
  name,
  value,
  max,
}: {
  name: string;
  value: number;
  max: number;
}) {
  return (
    <div className="resource">
      <span>{name}</span>
      <meter aria-label={name} value={value} min={0} max={max} />
      <span className="numeric">
        {value} / {max}
      </span>
    </div>
  );
}

function ActionForm({
  actions,
  unitName,
  busy,
  onAct,
}: {
  actions: Command[];
  unitName: (id: string) => string;
  busy: boolean;
  onAct: (command: Command) => void;
}) {
  const keyFor = (command: Command) => `${command.actor_id}:${command.action}`;
  const options = [
    ...new Map(actions.map((command) => [keyFor(command), command])).values(),
  ];
  const [choice, setChoice] = useState(keyFor(options[0]));
  const [target, setTarget] = useState('');
  const commands = actions.filter((item) => keyFor(item) === choice);
  const selected =
    commands.length === 1
      ? commands[0]
      : commands.find((command) => command.target_id === target);
  const targetOptions = commands.flatMap((command) =>
    command.target_id ? [command.target_id] : [],
  );

  return (
    <form
      className="action-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (selected && !busy) onAct(selected);
      }}
    >
      <fieldset disabled={busy}>
        <legend>选择技能</legend>
        <div className="action-options">
          {options.map((command) => (
            <button
              type="button"
              className="choice"
              key={keyFor(command)}
              aria-pressed={choice === keyFor(command)}
              onClick={() => {
                setChoice(keyFor(command));
                setTarget('');
              }}
            >
              <strong>{actionNames[command.action]}</strong>
              <span>{unitName(command.actor_id)}</span>
            </button>
          ))}
        </div>
      </fieldset>
      {targetOptions.length > 0 && (
        <label>
          选择目标
          <select
            disabled={busy}
            value={targetOptions.length === 1 ? targetOptions[0] : target}
            onChange={(event) => setTarget(event.target.value)}
          >
            {targetOptions.length > 1 && <option value="">请选择目标</option>}
            {targetOptions.map((id) => (
              <option key={id} value={id}>
                {unitName(id)}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className="form-actions">
        <button type="submit" disabled={!selected || busy}>
          {busy ? '正在结算…' : '执行指令'}
        </button>
      </div>
    </form>
  );
}
