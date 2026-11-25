import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { catalogError, characters } from '../characters/client';
import type {
  Catalog,
  SavedTeam,
  TeamDprReadiness,
} from '../characters/client';
import { DprResults } from '../training/Dpr';
import { TeamBoard } from './TeamBoard';

type Draft = Pick<SavedTeam, 'name' | 'slots'>;
type DprCheck =
  | { kind: 'checking' }
  | { kind: 'checked'; value: TeamDprReadiness }
  | { kind: 'error'; message: string };

const emptyTeam: Draft = { name: '', slots: [null, null, null, null] };

export type FormationSide = 'ally' | 'enemy';

export function TeamWorkspace({
  catalog,
  accountRevision,
  catalogStatus,
  onSaved,
  rosterIssue,
  enemyContent,
  side,
  onSideChange,
}: {
  catalog?: Catalog;
  accountRevision: number;
  catalogStatus: ReactNode;
  onSaved: (team: SavedTeam) => void;
  rosterIssue: string;
  enemyContent: ReactNode;
  side: FormationSide;
  onSideChange: (side: FormationSide) => void;
}) {
  return (
    <div className="team-workspace">
      <div className="side-switcher" role="tablist" aria-label="配置阵营">
        <button
          id="side-tab-ally"
          type="button"
          role="tab"
          className="side-tab"
          aria-selected={side === 'ally'}
          aria-controls="side-panel-ally"
          onClick={() => onSideChange('ally')}
        >
          己方
        </button>
        <button
          id="side-tab-enemy"
          type="button"
          role="tab"
          className="side-tab"
          aria-selected={side === 'enemy'}
          aria-controls="side-panel-enemy"
          onClick={() => onSideChange('enemy')}
        >
          敌方
        </button>
      </div>
      <section className="panel team-board" aria-labelledby="team-title">
        <h2 id="team-title">编队</h2>
        <div
          id="side-panel-ally"
          role="tabpanel"
          aria-labelledby="side-tab-ally"
          className="team-field"
          hidden={side !== 'ally'}
        >
          {catalog ? (
            <NamedTeam
              key={accountRevision}
              catalog={catalog}
              onSaved={onSaved}
              rosterIssue={rosterIssue}
            />
          ) : (
            catalogStatus
          )}
        </div>
        <div
          id="side-panel-enemy"
          role="tabpanel"
          aria-labelledby="side-tab-enemy"
          className="team-field"
          hidden={side !== 'enemy'}
        >
          {enemyContent}
        </div>
      </section>
    </div>
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
  const [dpr, setDpr] = useState<DprCheck>();
  const nameInput = useRef<HTMLInputElement>(null);
  const teamId = selected ?? catalog.teams[0]?.id ?? '';
  const saved = catalog.teams.find(({ id }) => id === teamId);
  const { name, slots } = drafts.get(teamId) ?? saved ?? emptyTeam;
  const dirty =
    name !== (saved?.name ?? '') ||
    slots.some((id, position) => id !== (saved?.slots[position] ?? null));
  const locked = saving || dpr?.kind === 'checking';
  const canCalculate = Boolean(saved && !dirty && slots.some(Boolean));

  function selectTeam(id: string) {
    setSelected(id);
    setIssue('');
    setStatus('');
    setDpr(undefined);
  }

  function updateDraft(draft: Draft) {
    setDrafts((current) => new Map(current).set(teamId, draft));
    setIssue('');
    setStatus('');
    setDpr(undefined);
  }

  async function checkDpr() {
    if (!canCalculate || locked) return;
    setDpr({ kind: 'checking' });
    try {
      const value = await characters.dprReadiness(teamId);
      setDpr({ kind: 'checked', value });
    } catch (error) {
      setDpr({ kind: 'error', message: catalogError(error) });
    }
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
          disabled={locked || catalog.teams.length === 0}
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
          disabled={locked || !teamId}
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
          disabled={locked}
          onChange={(event) => updateDraft({ name: event.target.value, slots })}
        />
      </label>
      <TeamBoard
        roster={catalog.characters}
        slots={slots}
        disabled={locked}
        onChange={(slots) => updateDraft({ name, slots })}
      />
      <div className="form-actions">
        <button
          type="submit"
          className="secondary"
          disabled={!dirty || locked || !name.trim()}
        >
          {saving ? '正在保存…' : '保存配队'}
        </button>
        <span role="status">
          {status || (dirty ? '当前配队有未保存的更改。' : '')}
        </span>
      </div>
      {(issue || rosterIssue) && <p role="alert">{issue || rosterIssue}</p>}
      <div className="form-actions">
        <button
          type="button"
          disabled={!canCalculate || locked}
          onClick={() => void checkDpr()}
        >
          {dpr?.kind === 'checking' ? '正在计算…' : '计算 DPR'}
        </button>
        <span className="muted">固定靶场</span>
      </div>
      {dpr?.kind === 'checked' &&
        (dpr.value.result ? (
          <>
            <p className="muted">
              固定靶场估算，计入配装静态属性。光锥与遗器的战斗触发效果暂未计入。
            </p>
            <DprResults
              result={dpr.value.result}
              unitName={(id) =>
                catalog.characters.find((character) => character.id === id)
                  ?.name ?? id
              }
            />
          </>
        ) : (
          <p role="status">
            以下角色的当前配置暂不支持固定靶场计算。
            {dpr.value.unsupported_characters.join('、')}
          </p>
        ))}
      {dpr?.kind === 'error' && <p role="alert">{dpr.message}</p>}
    </form>
  );
}
