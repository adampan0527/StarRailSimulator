import { lazy, Suspense, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { catalogError, characters } from '../characters/client';
import type { Catalog, SavedTeam } from '../characters/client';
import { TeamBoard } from './TeamBoard';

type Draft = Pick<SavedTeam, 'name' | 'slots'>;

const emptyTeam: Draft = { name: '', slots: [null, null, null, null] };
const EnemyLineup = lazy(() => import('./EnemyLineup'));

export function TeamWorkspace({
  catalog,
  catalogStatus,
  onSaved,
  rosterIssue,
  trainingContent,
  training,
  onModeChange,
  busy,
}: {
  catalog?: Catalog;
  catalogStatus: ReactNode;
  onSaved: (team: SavedTeam) => void;
  rosterIssue: string;
  trainingContent: ReactNode;
  training: boolean;
  onModeChange: (training: boolean) => void;
  busy: boolean;
}) {
  return (
    <section className="panel team-board" aria-labelledby="team-title">
      <div className="section-heading">
        <h2 id="team-title">配队</h2>
        <label className="enemy-level">
          战场
          <select
            className="team-select"
            value={training ? 'training' : 'custom'}
            disabled={busy}
            onChange={(event) =>
              onModeChange(event.target.value === 'training')
            }
          >
            <option value="custom">自选敌方（4.7.0）</option>
            <option value="training">合成训练</option>
          </select>
        </label>
      </div>
      <div className="team-field" hidden={training}>
        <Suspense fallback={<p role="status">正在读取敌方目录…</p>}>
          <EnemyLineup />
        </Suspense>
        {catalog ? (
          <NamedTeam
            catalog={catalog}
            onSaved={onSaved}
            rosterIssue={rosterIssue}
          />
        ) : (
          catalogStatus
        )}
        <div className="form-actions" aria-describedby="team-compute-status">
          <button type="button" disabled>
            开始战斗
          </button>
          <button type="button" className="secondary" disabled>
            计算 DPR
          </button>
          <button type="button" className="secondary" disabled>
            计算最优解
          </button>
        </div>
        <p id="team-compute-status" className="muted">
          当前可预览正式服敌方阵容。开战和计算将在正式服战斗规则接入后开放。
        </p>
      </div>
      <div className="team-field" hidden={!training}>
        {trainingContent}
      </div>
    </section>
  );
}

function NamedTeam({
  catalog,
  onSaved,
  rosterIssue,
}: {
  catalog: Catalog;
  onSaved: (team: SavedTeam) => void;
  rosterIssue: string;
}) {
  const [selected, setSelected] = useState<string>();
  const [drafts, setDrafts] = useState(() => new Map<string, Draft>());
  const [saving, setSaving] = useState(false);
  const [issue, setIssue] = useState('');
  const [status, setStatus] = useState('');
  const nameInput = useRef<HTMLInputElement>(null);
  const teamId = selected ?? catalog.teams[0]?.id ?? '';
  const saved = catalog.teams.find(({ id }) => id === teamId);
  const { name, slots } = drafts.get(teamId) ?? saved ?? emptyTeam;
  const dirty =
    name !== (saved?.name ?? '') ||
    slots.some((id, position) => id !== (saved?.slots[position] ?? null));

  function selectTeam(id: string) {
    setSelected(id);
    setIssue('');
    setStatus('');
  }

  function updateDraft(draft: Draft) {
    setDrafts((current) => new Map(current).set(teamId, draft));
    setIssue('');
    setStatus('');
  }

  async function saveTeam() {
    setSaving(true);
    setIssue('');
    setStatus('');
    try {
      const team = await characters.saveTeam(saved?.id ?? null, name, slots);
      onSaved(team);
      setDrafts((current) => {
        const remaining = new Map(current);
        remaining.delete(teamId);
        return remaining;
      });
      setSelected(team.id);
      setStatus('配队已保存。');
    } catch (error) {
      setIssue(catalogError(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void saveTeam();
      }}
    >
      <div className="team-controls" role="group" aria-label="配队管理">
        <select
          className="team-select"
          aria-label="已保存配队"
          value={teamId}
          title={saved?.name}
          disabled={saving || catalog.teams.length === 0}
          onChange={(event) => selectTeam(event.target.value)}
        >
          {(!teamId || drafts.has('')) && (
            <option value="">{drafts.has('') ? '未保存配队' : '新配队'}</option>
          )}
          {catalog.teams.map(({ id, name }) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="secondary"
          disabled={saving || !teamId}
          onClick={() => {
            selectTeam('');
            nameInput.current?.focus();
          }}
        >
          新建配队
        </button>
      </div>
      <label className="team-name">
        配队名称
        <input
          ref={nameInput}
          value={name}
          placeholder="输入配队名称"
          maxLength={48}
          required
          disabled={saving}
          onChange={(event) => updateDraft({ name: event.target.value, slots })}
        />
      </label>
      <TeamBoard
        roster={catalog.characters}
        slots={slots}
        disabled={saving}
        onChange={(slots) => updateDraft({ name, slots })}
      />
      <div className="form-actions">
        <button
          type="submit"
          className="secondary"
          disabled={!dirty || saving || !name.trim()}
        >
          {saving ? '正在保存…' : '保存配队'}
        </button>
        <span role="status">
          {status || (dirty ? '当前配队有未保存的更改。' : '')}
        </span>
      </div>
      {(issue || rosterIssue) && <p role="alert">{issue || rosterIssue}</p>}
    </form>
  );
}
