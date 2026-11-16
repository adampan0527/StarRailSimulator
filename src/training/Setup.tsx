import { useState } from 'react';
import { describeError, training } from './client';
import type { Equipment, Profile, Setup, Team } from './client';

type Props = {
  setup: Setup;
  starting: boolean;
  issue: string;
  onProfile: (profile: Profile) => void;
  onTeam: (team: Team) => void;
  onStart: (enemies: string[]) => void;
};

export function TrainingSetup({
  setup,
  starting,
  issue,
  onProfile,
  onTeam,
  onStart,
}: Props) {
  const [profileId, setProfileId] = useState(setup.profiles[0].id);
  const [members, setMembers] = useState(setup.team.character_form_ids);
  const [enemies, setEnemies] = useState(() =>
    setup.enemies.map((enemy) => enemy.id),
  );
  const [teamMessage, setTeamMessage] = useState('');
  const [teamError, setTeamError] = useState('');
  const [saving, setSaving] = useState(false);
  const profile = setup.profiles.find((entry) => entry.id === profileId);
  const savedPreset = profile?.presets.find(
    (preset) => preset.name === profile.current_preset,
  );
  const savedMembers = setup.team.character_form_ids;
  const teamDirty =
    members.length !== savedMembers.length ||
    members.some((id, index) => id !== savedMembers[index]);

  function move(index: number, offset: number) {
    const next = [...members];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    setMembers(next);
    setTeamMessage('');
  }

  function updateMember(id: string, checked: boolean) {
    setMembers(
      checked ? [...members, id] : members.filter((member) => member !== id),
    );
    setTeamMessage('');
  }

  async function saveTeam() {
    setSaving(true);
    setTeamError('');
    try {
      onTeam(await training.saveTeam(members));
      setTeamMessage('队伍已保存。');
    } catch (error) {
      setTeamError(describeError(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="setup-grid">
      <section className="panel" aria-labelledby="loadout-title">
        <div className="section-heading">
          <h2 id="loadout-title">角色配装</h2>
        </div>
        <div
          className="profile-picker"
          role="group"
          aria-label="选择要配装的角色"
        >
          {setup.profiles.map((entry) => (
            <button
              className="choice"
              key={entry.id}
              type="button"
              aria-pressed={entry.id === profileId}
              onClick={() => setProfileId(entry.id)}
            >
              {entry.name}
            </button>
          ))}
        </div>
        {profile && savedPreset ? (
          <LoadoutForm
            key={profile.id}
            profile={profile}
            equipment={savedPreset.equipment}
            speedTiers={setup.boots_speed_tiers_micros}
            onProfile={onProfile}
          />
        ) : (
          <p role="alert">找不到当前配装。</p>
        )}
      </section>

      <section className="panel" aria-labelledby="team-title">
        <div className="section-heading">
          <h2 id="team-title">训练队伍</h2>
          <span className="tag">{members.length} / 4</span>
        </div>
        <fieldset className="check-grid" disabled={saving || starting}>
          <legend>上阵角色</legend>
          {setup.profiles.map(({ id, name }) => (
            <label key={id}>
              <input
                type="checkbox"
                checked={members.includes(id)}
                onChange={(event) => updateMember(id, event.target.checked)}
              />
              {name}
            </label>
          ))}
        </fieldset>
        <h3>站位顺序</h3>
        <ol className="lineup">
          {members.map((id, index) => {
            const name = setup.profiles.find((entry) => entry.id === id)?.name;
            return (
              <li key={id}>
                <span>{name}</span>
                <div className="move-buttons">
                  <button
                    type="button"
                    className="secondary"
                    aria-label={`前移${name}`}
                    disabled={index === 0 || saving || starting}
                    onClick={() => move(index, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    aria-label={`后移${name}`}
                    disabled={
                      index === members.length - 1 || saving || starting
                    }
                    onClick={() => move(index, 1)}
                  >
                    ↓
                  </button>
                </div>
              </li>
            );
          })}
        </ol>
        {members.length === 0 && (
          <p className="muted">请选择至少一名上阵角色。</p>
        )}
        <div className="form-actions">
          <button
            type="button"
            className="secondary"
            disabled={!teamDirty || members.length === 0 || saving || starting}
            onClick={saveTeam}
          >
            {saving ? '正在保存…' : '保存队伍'}
          </button>
          <span role="status">{teamMessage}</span>
        </div>
        {teamError && <p role="alert">{teamError}</p>}
        <fieldset className="check-grid separated" disabled={starting}>
          <legend>训练目标</legend>
          {setup.enemies.map(({ id, name }) => (
            <label key={id}>
              <input
                type="checkbox"
                checked={enemies.includes(id)}
                onChange={(event) =>
                  setEnemies(
                    event.target.checked
                      ? [...enemies, id]
                      : enemies.filter((entry) => entry !== id),
                  )
                }
              />
              {name}
            </label>
          ))}
        </fieldset>
        <div className="form-actions">
          <button
            type="button"
            disabled={starting || saving || teamDirty || enemies.length === 0}
            onClick={() => onStart(enemies)}
          >
            {starting ? '正在开战…' : '开始战斗'}
          </button>
          <span className="muted">
            {teamDirty ? '请先保存队伍。' : '使用当前已保存配装'}
          </span>
        </div>
        {issue && <p role="alert">{issue}</p>}
      </section>
    </div>
  );
}

type LoadoutProps = {
  profile: Profile;
  equipment: Equipment;
  speedTiers: number[];
  onProfile: (profile: Profile) => void;
};

function LoadoutForm({
  profile,
  equipment,
  speedTiers,
  onProfile,
}: LoadoutProps) {
  const [presetName, setPresetName] = useState(profile.current_preset);
  const [draft, setDraft] = useState(equipment);
  const [saving, setSaving] = useState(false);
  const [issue, setIssue] = useState('');
  const [message, setMessage] = useState('');
  const dirty =
    draft.light_cone_equipped !== equipment.light_cone_equipped ||
    draft.boots_speed_micros !== equipment.boots_speed_micros ||
    presetName !== profile.current_preset;

  async function applyPreset(request: () => Promise<Profile>, status: string) {
    setSaving(true);
    setIssue('');
    setMessage('');
    try {
      const updated = await request();
      const preset = updated.presets.find(
        (item) => item.name === updated.current_preset,
      );
      if (preset) {
        setPresetName(preset.name);
        setDraft(preset.equipment);
      }
      onProfile(updated);
      setMessage(status);
    } catch (error) {
      setIssue(describeError(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void applyPreset(
          () => training.saveLoadout(profile.id, presetName, draft),
          '配装已保存并启用。',
        );
      }}
    >
      <fieldset className="loadout-form" disabled={saving}>
        <legend>
          {profile.name} <span className="muted">Lv. {profile.level} · E0</span>
        </legend>
        <label>
          当前配装
          <select
            value={profile.current_preset}
            onChange={(event) => {
              const name = event.target.value;
              void applyPreset(
                () => training.selectLoadout(profile.id, name),
                '已切换配装。',
              );
            }}
          >
            {profile.presets.map((preset) => (
              <option key={preset.name}>{preset.name}</option>
            ))}
          </select>
        </label>
        <label>
          光锥
          <select
            value={draft.light_cone_equipped ? 'equipped' : 'none'}
            disabled={!profile.can_equip_light_cone}
            onChange={(event) => {
              setMessage('');
              setDraft({
                ...draft,
                light_cone_equipped: event.target.value === 'equipped',
              });
            }}
          >
            <option value="none">空位</option>
            {profile.can_equip_light_cone && (
              <option value="equipped">训练光锥 · Lv. 20 · S1</option>
            )}
          </select>
        </label>
        <label>
          脚部遗器
          <select
            value={draft.boots_speed_micros ?? 'none'}
            onChange={(event) => {
              setMessage('');
              setDraft({
                ...draft,
                boots_speed_micros:
                  event.target.value === 'none'
                    ? null
                    : Number(event.target.value),
              });
            }}
          >
            <option value="none">空位</option>
            {speedTiers.map((speed) => (
              <option key={speed} value={speed}>
                训练靴 +3 · 速度 +{(speed / 1_000_000).toFixed(1)}
              </option>
            ))}
          </select>
        </label>
        <label>
          方案名称
          <input
            value={presetName}
            onChange={(event) => {
              setMessage('');
              setPresetName(event.target.value);
            }}
            maxLength={48}
            required
          />
        </label>
      </fieldset>
      <div className="form-actions">
        <button
          type="submit"
          disabled={!dirty || saving || presetName.trim() === ''}
        >
          {saving ? '正在保存…' : '保存配装'}
        </button>
        <span role="status">
          {message || (dirty ? '配装有未保存的更改' : '')}
        </span>
      </div>
      {issue && <p role="alert">{issue}</p>}
    </form>
  );
}
