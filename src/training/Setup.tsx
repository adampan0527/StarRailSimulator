import { useState } from 'react';
import { TeamBoard } from '../team/TeamBoard';
import { describeError, training } from './client';
import type { Equipment, Profile, Setup, TeamSlots } from './client';

export function TrainingSetup({
  setup,
  starting,
  issue,
  onTeam,
  onStart,
  onLoadout,
}: {
  setup: Setup;
  starting: boolean;
  issue: string;
  onTeam: (slots: TeamSlots) => void;
  onStart: (enemies: string[]) => void;
  onLoadout: () => void;
}) {
  const [enemies, setEnemies] = useState(() =>
    setup.enemies.map(({ id }) => id),
  );
  return (
    <TeamBoard
      roster={setup.profiles}
      savedSlots={setup.team_slots}
      save={training.saveTeam}
      onSaved={onTeam}
      busy={starting}
      issue={issue}
      training={{
        targets: setup.enemies,
        selected: enemies,
        toggle: (id) =>
          setEnemies((current) =>
            current.includes(id)
              ? current.filter((enemy) => enemy !== id)
              : [...current, id],
          ),
        start: () => onStart(enemies),
        onLoadout,
      }}
    />
  );
}

export function TrainingLoadouts({
  setup,
  onProfile,
}: {
  setup: Setup;
  onProfile: (profile: Profile) => void;
}) {
  const [profileId, setProfileId] = useState(setup.profiles[0]?.id);
  const profile = setup.profiles.find((entry) => entry.id === profileId);
  const savedPreset = profile?.presets.find(
    (preset) => preset.name === profile.current_preset,
  );
  return (
    <section className="panel" aria-labelledby="loadout-title">
      <h2 id="loadout-title">训练配装</h2>
      <div className="profile-picker" role="group" aria-label="选择训练角色">
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
