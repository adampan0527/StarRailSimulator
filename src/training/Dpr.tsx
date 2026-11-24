import { actionNames, av, damageNames } from './Timeline';
import type { DprResult } from './client';
import './dpr.css';

const format = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });

const memospriteNames = new Map([
  ['trailblazer.remembrance:mem', '迷迷'],
  ['character.1402:garmentmaker', '衣匠'],
  ['character.1407:netherwing', '死龙'],
  ['character.1413:evey', '长夜'],
  ['character.1415:demiurge', '德谬歌'],
  ['character.1409:little-ica', '小伊卡'],
]);

export function DprResults({
  result,
  unitName,
}: {
  result: DprResult;
  unitName: (id: string) => string;
}) {
  const characters = [...result.characters].sort((a, b) => b.dpr - a.dpr);
  const maxContribution = Math.max(
    1,
    ...characters.map(
      ({ own_damage, assist_damage }) => own_damage + assist_damage,
    ),
  );

  return (
    <section
      className="field-zone dpr-results"
      aria-labelledby="dpr-result-title"
    >
      <div className="dpr-overview">
        <h3 id="dpr-result-title">DPR 结果</h3>
        <div className="dpr-overview-stats" role="status">
          <p className="dpr-team-rate">
            <strong>{format.format(result.dpr)}</strong>
            <span className="muted">全队每轮伤害</span>
          </p>
          <p className="muted">
            前 {result.cycles} 轮总伤害{' '}
            <strong>{format.format(result.total_damage)}</strong>
          </p>
        </div>
      </div>

      <div className="dpr-chart-heading">
        <h4>角色每轮贡献</h4>
        <div className="dpr-legend muted">
          <span>
            <span className="dpr-own" aria-hidden="true" />
            自身
          </span>
          <span>
            <span className="dpr-assist" aria-hidden="true" />
            助攻
          </span>
        </div>
      </div>
      <ul className="dpr-contributions" aria-label="角色每轮贡献">
        {characters.map(({ unit_id, own_damage, assist_damage, dpr }) => (
          <li key={unit_id}>
            <details>
              <summary>
                <span className="dpr-character">{unitName(unit_id)}</span>
                <span className="dpr-bar" aria-hidden="true">
                  <span
                    className="dpr-own"
                    style={{
                      width: `${(own_damage / maxContribution) * 100}%`,
                    }}
                  />
                  <span
                    className="dpr-assist"
                    style={{
                      width: `${(assist_damage / maxContribution) * 100}%`,
                    }}
                  />
                </span>
                <strong className="dpr-character-rate">
                  {format.format(dpr)}
                  <span className="muted"> / 轮</span>
                </strong>
              </summary>
              <div className="dpr-breakdown">
                <span className="muted">前 {result.cycles} 轮累计</span>
                <dl>
                  <div>
                    <dt>自身伤害</dt>
                    <dd>{format.format(own_damage)}</dd>
                  </div>
                  <div>
                    <dt>助攻伤害</dt>
                    <dd>{format.format(assist_damage)}</dd>
                  </div>
                </dl>
              </div>
            </details>
          </li>
        ))}
      </ul>

      <h4 className="dpr-target-heading">逐靶机伤害</h4>
      {result.targets.map(({ max_toughness, damage, hits }, index) => (
        <details key={index} className="dpr-damage">
          <summary>
            <span>
              靶机 {index + 1}{' '}
              <span className="muted">· 韧性 {max_toughness}</span>
            </span>
            <strong>{format.format(damage)}</strong>
          </summary>
          {hits.length === 0 ? (
            <p className="muted">本次没有受到伤害。</p>
          ) : (
            <ol className="dpr-hits" aria-label={`靶机 ${index + 1} 伤害明细`}>
              {hits.map((hit, sequence) => (
                <li key={sequence}>
                  <span>
                    <span className="muted">{av(hit.av_micros)} AV</span> ·{' '}
                    {memospriteNames.get(hit.source_id) ??
                      unitName(hit.source_id)}{' '}
                    · {actionNames[hit.action]}
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
