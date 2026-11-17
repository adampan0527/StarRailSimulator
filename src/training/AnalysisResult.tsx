import { actionNames, av } from './Timeline';
import type { TrainingAnalysis } from './client';

const dprFormat = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 });

export function AnalysisResult({ analysis }: { analysis: TrainingAnalysis }) {
  if (analysis.kind === 'dpr') {
    const { cycles, enemy_hp_lost, dpr } = analysis.result;
    return (
      <div role="status">
        <p>
          <strong>{dprFormat.format(dpr)} DPR</strong> · 前 {cycles} 个关卡轮次
          · 有效伤害 {enemy_hp_lost}
        </p>
        <p className="muted">
          固定打法优先使用伤害型终结技、伤害型战技，再用普攻。按首个合法目标结算。
        </p>
      </div>
    );
  }

  const { exact, evaluated, budget, victory } = analysis.result;
  const unitName = (id: string) =>
    analysis.names.find((unit) => unit.id === id)?.name ?? id;

  return (
    <div>
      <p role="status">
        {victory
          ? `${exact ? '最少行动值' : '本次预算内找到的最佳'} ${av(victory.av_micros)} AV`
          : exact
            ? '此战场没有可通关的打法。'
            : '本次预算内未找到通关打法。'}
        {' · 已评估 '}
        {evaluated} / {budget} 条指令
      </p>
      {victory && (
        <ol aria-label="通关指令">
          {victory.commands.map(
            ({ decision_index, actor_id, action, target_id }) => (
              <li key={decision_index}>
                {unitName(actor_id)} · {actionNames[action]}
                {target_id && ` → ${unitName(target_id)}`}
              </li>
            ),
          )}
        </ol>
      )}
    </div>
  );
}
