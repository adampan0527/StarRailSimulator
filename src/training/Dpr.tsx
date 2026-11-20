import { actionNames, av, damageNames } from './Timeline';
import type { DprResult } from './client';
import './dpr.css';

const format = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });

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
                    {hit.kind !== 'direct' && ` · ${damageNames[hit.kind]}`}
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
