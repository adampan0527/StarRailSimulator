import { useId } from 'react';
import type { BattleView, Unit } from './client';
import { av, outcomeNames, Timeline } from './Timeline';

export function BattlePanel({ battle }: { battle: BattleView }) {
  const { id, snapshot, order, events, names } = battle;
  const titleId = useId();
  const unitName = (id: string) =>
    id === 'summon:aha'
      ? '阿哈'
      : (names.find((entry) => entry.id === id)?.name ?? id);
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
            战斗回放 <span className="muted numeric">#{id}</span>
          </h2>
          <span className={`tag outcome-${snapshot.outcome}`}>
            {outcomeNames[snapshot.outcome]}
          </span>
        </div>
        <div className="battle-summary" role="status">
          <strong>{actorLabel}</strong>
          <span className="numeric">
            <span className="muted">战技点</span> {snapshot.skill_points} /{' '}
            {snapshot.skill_point_cap ?? 5}
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
                  key={`${unit_id}:${index === 0 ? 'current' : 'scheduled'}`}
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

export function Resource({
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
