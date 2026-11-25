import { useRef, useState } from 'react';
import EnemyLineup from './EnemyLineup';
import { enemyFormationError, enemyFormations } from './enemyFormationClient';
import type {
  EnemyFormation,
  EnemyFormationDraft,
} from './enemyFormationClient';

const emptyDraft: EnemyFormationDraft = {
  name: '',
  slots: [null, null, null, null, null],
};

export default function EnemyFormations({
  formations,
  onSaved,
}: {
  formations: EnemyFormation[];
  onSaved: (formation: EnemyFormation) => void;
}) {
  const [selected, setSelected] = useState<number | null>();
  const [drafts, setDrafts] = useState(
    new Map<number | null, EnemyFormationDraft>(),
  );
  const [saving, setSaving] = useState(false);
  const [issue, setIssue] = useState('');
  const [status, setStatus] = useState('');
  const nameInput = useRef<HTMLInputElement>(null);
  const id = selected === undefined ? (formations[0]?.id ?? null) : selected;
  const saved = formations.find((item) => item.id === id);
  const draft = drafts.get(id) ?? saved ?? emptyDraft;
  const dirty = draft !== (saved ?? emptyDraft);

  function select(next: number | null) {
    setSelected(next);
    setIssue('');
    setStatus('');
  }

  function edit(next: EnemyFormationDraft) {
    setDrafts((current) => new Map(current).set(id, next));
    setIssue('');
    setStatus('');
  }

  async function save() {
    setSaving(true);
    setIssue('');
    try {
      const updated = await enemyFormations.save(id, {
        name: draft.name,
        slots: draft.slots,
      });
      onSaved(updated);
      setDrafts((current) => {
        const remaining = new Map(current);
        remaining.delete(id);
        return remaining;
      });
      setSelected(updated.id);
      setStatus('敌方编队已保存。');
    } catch (error) {
      setIssue(enemyFormationError(error));
    } finally {
      setSaving(false);
    }
  }

  const canSave = dirty && draft.name.trim() && draft.slots.some(Boolean);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave && !saving) void save();
      }}
    >
      <div className="team-controls" role="group" aria-label="敌方编队管理">
        <select
          className="team-select"
          aria-label="已保存敌方编队"
          value={id ?? ''}
          disabled={saving || formations.length === 0}
          onChange={(event) => select(Number(event.target.value) || null)}
        >
          {(id === null || drafts.has(null)) && (
            <option value="">新编队</option>
          )}
          {formations.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="secondary"
          disabled={saving || id === null}
          onClick={() => {
            select(null);
            nameInput.current?.focus();
          }}
        >
          新建敌方编队
        </button>
      </div>
      <label className="team-name">
        编队名称
        <input
          ref={nameInput}
          value={draft.name}
          maxLength={48}
          required
          disabled={saving}
          onChange={(event) => edit({ ...draft, name: event.target.value })}
        />
      </label>
      <EnemyLineup
        key={id}
        lineup={draft.slots}
        disabled={saving}
        onChange={(slots) => edit({ ...draft, slots })}
      />
      <div className="form-actions">
        <button type="submit" disabled={saving || !canSave}>
          {saving ? '正在保存…' : '保存敌方编队'}
        </button>
        <span role="status">{status || (dirty ? '有未保存的更改。' : '')}</span>
      </div>
      {issue && <p role="alert">{issue}</p>}
    </form>
  );
}
