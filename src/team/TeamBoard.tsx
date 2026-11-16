import { useId, useState } from 'react';
import {
  CharacterCardBody,
  CharacterStatGrid,
} from '../characters/CharacterCard';
import { describeError } from '../training/client';
import type { CharacterSummary, Name, TeamSlots } from '../training/client';
import { CharacterPicker } from './CharacterPicker';

type TrainingControls = {
  targets: Name[];
  selected: string[];
  toggle: (id: string) => void;
  start: () => void;
  onLoadout: () => void;
};

export function TeamBoard({
  roster,
  savedSlots,
  save,
  onSaved,
  busy = false,
  issue,
  training,
}: {
  roster: CharacterSummary[];
  savedSlots: TeamSlots;
  save: (slots: TeamSlots) => Promise<TeamSlots>;
  onSaved: (slots: TeamSlots) => void;
  busy?: boolean;
  issue?: string;
  training?: TrainingControls;
}) {
  const [slots, setSlots] = useState(savedSlots);
  const [picking, setPicking] = useState<number>();
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const boardId = useId();
  const dirty = slots.some((id, position) => id !== savedSlots[position]);
  const count = slots.filter((id) => id !== null).length;

  async function saveTeam() {
    setError('');
    setStatus('');
    setSaving(true);
    try {
      const saved = await save(slots);
      onSaved(saved);
      setStatus('队伍已保存。');
    } catch (error) {
      setError(describeError(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="panel team-board" aria-labelledby={boardId}>
      <div className="section-heading">
        <h2 id={boardId}>{training ? '训练队伍' : '当前队伍'}</h2>
        {training && (
          <button
            type="button"
            className="secondary"
            disabled={busy || saving}
            onClick={training.onLoadout}
          >
            训练配装
          </button>
        )}
      </div>
      <section
        className="field-zone enemy-zone"
        aria-labelledby={`${boardId}-enemy`}
      >
        <h3 id={`${boardId}-enemy`}>敌方</h3>
        {training ? (
          <div className="enemy-slots" role="group" aria-label="训练目标">
            {training.targets.map(({ id, name }) => (
              <button
                key={id}
                type="button"
                className="choice"
                aria-pressed={training.selected.includes(id)}
                disabled={busy}
                onClick={() => training.toggle(id)}
              >
                {name}
              </button>
            ))}
          </div>
        ) : (
          <p className="muted">正式关卡待接入。</p>
        )}
      </section>
      <section className="field-zone" aria-labelledby={`${boardId}-ally`}>
        <div className="section-heading">
          <h3 id={`${boardId}-ally`}>己方</h3>
          <div className="team-detail">
            <span className="muted">
              {training ? '训练配装属性' : '角色属性'}
            </span>
            <span className="tag numeric">{count} / 4</span>
          </div>
        </div>
        <ol className="ally-slots" aria-label="己方站位">
          {slots.map((id, position) => {
            const member = roster.find((item) => item.id === id);
            const statsId = `${boardId}-stats-${position}`;
            return (
              <li key={position}>
                <button
                  type="button"
                  className="choice ally-slot"
                  data-filled={member ? 'true' : undefined}
                  title={member?.name}
                  aria-label={`站位 ${position + 1}：${member?.name ?? '空位'}`}
                  aria-describedby={
                    member ? `${statsId}-traits ${statsId}` : undefined
                  }
                  aria-haspopup="dialog"
                  disabled={saving || busy}
                  onClick={() => setPicking(position)}
                >
                  <span className="slot-number numeric">{position + 1}</span>
                  <CharacterCardBody
                    character={member}
                    traitsId={`${statsId}-traits`}
                  >
                    {member && (
                      <CharacterStatGrid
                        id={statsId}
                        element={member.element}
                        values={member.stats}
                      />
                    )}
                  </CharacterCardBody>
                </button>
              </li>
            );
          })}
        </ol>
      </section>
      <div className="form-actions">
        <button
          type="button"
          className={training ? 'secondary' : undefined}
          disabled={!dirty || saving || busy}
          onClick={() => void saveTeam()}
        >
          {saving ? '正在保存…' : '保存队伍'}
        </button>
        {training && (
          <button
            type="button"
            disabled={
              dirty ||
              saving ||
              busy ||
              count === 0 ||
              training.selected.length === 0
            }
            onClick={training.start}
          >
            {busy ? '正在开战…' : '开始战斗'}
          </button>
        )}
        <span role="status">{status || (dirty ? '有未保存的站位。' : '')}</span>
      </div>
      {(error || issue) && <p role="alert">{error || issue}</p>}
      {picking !== undefined && (
        <CharacterPicker
          index={picking}
          roster={roster}
          slots={slots}
          onClose={() => setPicking(undefined)}
          onSelect={(id) => {
            setSlots((current) => {
              const next: TeamSlots = [...current];
              next[picking] = id;
              return next;
            });
            setError('');
            setStatus('');
          }}
        />
      )}
    </section>
  );
}
