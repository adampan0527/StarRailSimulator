import { useId, useState } from 'react';
import type { Action, BattleEvent, DamageKind, Outcome } from './client';

export const actionNames: Record<Action, string> = {
  basic: '普攻',
  skill: '战技',
  ultimate: '终结技',
};

export const damageNames: Record<DamageKind, string> = {
  direct: '伤害',
  weakness_break: '击破伤害',
  damage_over_time: '持续伤害',
  additional: '附加伤害',
  follow_up: '追加攻击',
};

export const outcomeNames: Record<Outcome, string> = {
  ongoing: '进行中',
  victory: '胜利',
  defeat: '失败',
  turn_limit: '达到回合上限',
};

const statusNames = {
  shield: '护盾',
  vulnerability: '易伤',
  speed_up: '加速',
  slow: '减速',
  defense_down: '减防',
  resistance_down: '减抗',
};
const number = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 6 });

export function av(micros: number) {
  return number.format(micros / 1_000_000);
}

function eventTitle({ kind }: BattleEvent): string {
  switch (kind.event) {
    case 'wave_started':
      return '战斗开始';
    case 'turn_started':
      return '回合开始';
    case 'action':
      return actionNames[kind.detail];
    case 'enemy_attack':
      return '敌方攻击';
    case 'follow_up':
      return '追加攻击';
    case 'skill_points':
      return '战技点变化';
    case 'energy':
      return '能量变化';
    case 'damage':
      return damageNames[kind.detail.kind];
    case 'heal':
      return '治疗';
    case 'hp_spent':
      return '消耗生命';
    case 'shock_applied':
      return '施加触电';
    case 'shock_extended':
      return '延长触电';
    case 'shock_expired':
      return '触电结束';
    case 'toughness':
      return '韧性变化';
    case 'weakness_broken':
      return '弱点击破';
    case 'gauge_shifted':
      return '行动条调整';
    case 'status_applied':
      return `施加${statusNames[kind.detail]}`;
    case 'status_expired':
      return `${statusNames[kind.detail]}结束`;
    case 'status_cleared':
      return '解除状态';
    case 'shield_depleted':
      return '护盾耗尽';
    case 'unit_defeated':
      return '单位退场';
    case 'battle_ended':
      return outcomeNames[kind.detail];
  }
}

function eventDetail({ kind }: BattleEvent) {
  switch (kind.event) {
    case 'skill_points':
    case 'toughness':
      return `${kind.detail.before} → ${kind.detail.after}`;
    case 'energy':
      return `${kind.detail.before_milli / 1000} → ${kind.detail.after_milli / 1000}`;
    case 'heal':
      return `恢复生命 ${kind.detail.amount}`;
    case 'hp_spent':
      return `消耗生命 ${kind.detail.amount}`;
    case 'damage': {
      const { amount, hp_loss, absorbed, critical } = kind.detail;
      const values = [`伤害 ${amount}`, `实际扣除生命 ${hp_loss}`];
      if (absorbed > 0) values.push(`护盾吸收 ${absorbed}`);
      if (critical) values.push('暴击');
      return values.join(' · ');
    }
    case 'gauge_shifted':
      return `${number.format(kind.detail.before_quanta / 1_000_000_000_000)} → ${number.format(kind.detail.after_quanta / 1_000_000_000_000)} 行动条点数`;
    default:
      return '';
  }
}

const keyEvents = new Set([
  'wave_started',
  'action',
  'weakness_broken',
  'unit_defeated',
  'battle_ended',
]);

export function Timeline({
  events,
  unitName,
}: {
  events: BattleEvent[];
  unitName: (id: string) => string;
}) {
  const titleId = useId();
  const [all, setAll] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const visible = events
    .map((event, index) => ({ event, index }))
    .filter(({ event }) => all || keyEvents.has(event.kind.event));
  const active =
    visible.find(({ index }) => index === selected) ?? visible.at(-1);

  function actors(event: BattleEvent) {
    return [event.source_id, event.target_id]
      .filter((id): id is string => id !== null)
      .map(unitName)
      .join(' → ');
  }

  return (
    <section className="panel timeline" aria-labelledby={titleId}>
      <div className="section-heading">
        <h3 id={titleId}>事件时间轴</h3>
        <span className="tag">{visible.length} 条</span>
      </div>
      {visible.length === 0 ? (
        <p>战斗尚无事件。</p>
      ) : (
        <>
          <div className="event-toolbar">
            <label>
              事件范围
              <select
                value={all ? 'all' : 'key'}
                onChange={(event) => setAll(event.target.value === 'all')}
              >
                <option value="key">关键事件</option>
                <option value="all">全部事件</option>
              </select>
            </label>
            <label>
              定位事件
              <select
                value={active?.index ?? ''}
                onChange={(event) => setSelected(Number(event.target.value))}
              >
                {visible.map(({ event, index }) => (
                  <option key={index} value={index}>
                    {av(event.av_micros)} AV · {eventTitle(event)}{' '}
                    {actors(event)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {active && (
            <div className="event-detail" role="status">
              <span className="numeric">{av(active.event.av_micros)} AV</span>
              <strong>{eventTitle(active.event)}</strong>
              <span>{actors(active.event)}</span>
              {eventDetail(active.event) && (
                <span>{eventDetail(active.event)}</span>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}
