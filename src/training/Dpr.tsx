import { useState } from 'react';
import { actionNames, av } from './Timeline';
import type { DprResult } from './client';
import './dpr.css';

const defaultToughness = [
  ['200'],
  ['120', '120'],
  ['30', '120', '30'],
  ['30', '120', '30', '30'],
  ['30', '30', '120', '30', '30'],
];
const maxToughness = 65_535;
const format = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });

export function DprControls({
  disabled,
  teamReady,
  running,
  onChange,
  onCalculate,
}: {
  disabled: boolean;
  teamReady: boolean;
  running: boolean;
  onChange: () => void;
  onCalculate: (toughness: number[]) => void;
}) {
  const [count, setCount] = useState(3);
  const [drafts, setDrafts] = useState<Record<number, string[]>>({});
  const values = drafts[count] ?? defaultToughness[count - 1];
  const toughness = values.map(Number);
  const valid = toughness.every(
    (value) => Number.isInteger(value) && value >= 1 && value <= maxToughness,
  );

  return (
    <form
      className="field-zone dpr-config"
      aria-labelledby="dpr-title"
      onSubmit={(event) => {
        event.preventDefault();
        if (teamReady && !disabled && valid) onCalculate(toughness);
      }}
    >
      <div className="section-heading">
        <h3 id="dpr-title">DPR 靶机</h3>
        <label className="enemy-level">
          靶机数量
          <select
            value={count}
            disabled={disabled}
            onChange={(event) => {
              setCount(Number(event.target.value));
              onChange();
            }}
          >
            {defaultToughness.map((_, index) => (
              <option key={index} value={index + 1}>
                {index + 1}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className="muted">
        前三轮 · 永久靶机 · 全属性弱点 · 0% 属性抗性 · 靶机回合待机
      </p>
      <div className="dpr-targets">
        {values.map((value, index) => (
          <label key={index}>
            靶机 {index + 1} 最大韧性
            <input
              type="number"
              inputMode="numeric"
              min={1}
              max={maxToughness}
              step={1}
              required
              value={value}
              disabled={disabled}
              onChange={(event) => {
                const next = values.slice();
                next[index] = event.target.value;
                setDrafts((current) => ({ ...current, [count]: next }));
                onChange();
              }}
            />
          </label>
        ))}
      </div>
      <div className="form-actions">
        <button type="submit" disabled={disabled || !teamReady || !valid}>
          {running ? '正在计算…' : '计算 DPR'}
        </button>
        {!valid && <span role="status">请输入 1 至 65,535 的整数韧性。</span>}
      </div>
    </form>
  );
}

export function DprResults({
  result,
  unitName,
}: {
  result: DprResult;
  unitName: (id: string) => string;
}) {
  return (
    <section
      className="field-zone dpr-results"
      aria-labelledby="dpr-result-title"
    >
      <h3 id="dpr-result-title">DPR 结果</h3>
      <p role="status">
        全队 <strong>{format.format(result.dpr)} DPR</strong> · 前{' '}
        {result.cycles} 轮 · 总伤害 {format.format(result.total_damage)}
      </p>
      <table className="dpr-contributions" aria-label="角色伤害贡献">
        <thead>
          <tr>
            <th scope="col">角色</th>
            <th scope="col">自身贡献</th>
            <th scope="col">增益贡献</th>
            <th scope="col">贡献 DPR</th>
          </tr>
        </thead>
        <tbody>
          {result.characters.map(
            ({ unit_id, own_damage, assist_damage, dpr }) => (
              <tr key={unit_id}>
                <th scope="row">{unitName(unit_id)}</th>
                <td>{format.format(own_damage)}</td>
                <td>{format.format(assist_damage)}</td>
                <td>{format.format(dpr)}</td>
              </tr>
            ),
          )}
        </tbody>
      </table>
      <h4>逐靶机伤害</h4>
      {result.targets.map(({ max_toughness, damage, hits }, index) => (
        <details key={index} className="dpr-damage">
          <summary>
            靶机 {index + 1} · 韧性 {max_toughness} · 伤害{' '}
            {format.format(damage)}
          </summary>
          {hits.length === 0 ? (
            <p className="muted">本次没有受到伤害。</p>
          ) : (
            <ol className="dpr-hits" aria-label={`靶机 ${index + 1} 伤害明细`}>
              {hits.map((hit, sequence) => (
                <li key={sequence}>
                  <span>
                    <span className="muted">{av(hit.av_micros)} AV</span> ·{' '}
                    {unitName(hit.source_id)} · {actionNames[hit.action]}
                    {hit.kind === 'weakness_break' && ' · 击破'}
                    {hit.critical && ' · 暴击'} ·{' '}
                    <strong>{format.format(hit.amount)}</strong>
                  </span>
                  <span className="muted">
                    归属{' '}
                    {hit.credits
                      .map(
                        ({ unit_id, amount }) =>
                          `${unitName(unit_id)} ${format.format(amount)}`,
                      )
                      .join('、')}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </details>
      ))}
    </section>
  );
}
