import { actionNames, av } from './Timeline';
import { DprResults } from './Dpr';
import type { TrainingAnalysis } from './client';

export function AnalysisResult({ analysis }: { analysis: TrainingAnalysis }) {
  const unitName = (id: string) =>
    analysis.names.find((unit) => unit.id === id)?.name ?? id;
  if (analysis.kind === 'dpr') {
    return <DprResults result={analysis.result} unitName={unitName} />;
  }

  const { exact, evaluated, budget, victory } = analysis.result;

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
