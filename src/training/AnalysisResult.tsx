import { av } from './Timeline';
import { DprResults } from './Dpr';
import { PlannedBattle } from './PlannedBattle';
import type { Profile, TrainingAnalysis } from './client';

export function AnalysisResult({
  analysis,
  profiles,
}: {
  analysis: TrainingAnalysis;
  profiles: Profile[];
}) {
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
      {victory && analysis.replay && (
        <PlannedBattle
          commands={victory.commands}
          replay={analysis.replay}
          names={analysis.names}
          profiles={profiles}
        />
      )}
    </div>
  );
}
