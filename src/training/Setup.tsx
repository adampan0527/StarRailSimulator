import { useState } from 'react';
import { TeamBoard } from '../team/TeamBoard';
import { AnalysisResult } from './AnalysisResult';
import { describeError, training } from './client';
import type {
  AnalysisKind,
  Equipment,
  Profile,
  Setup,
  TeamSlots,
  TrainingAnalysis,
} from './client';

export function TrainingSetup({
  setup,
  starting,
  issue,
  onTeam,
  onStart,
  onLoadout,
  onResume,
}: {
  setup: Setup;
  starting: boolean;
  issue: string;
  onTeam: (slots: TeamSlots) => void;
  onStart: (enemies: string[]) => void;
  onLoadout: () => void;
  onResume?: () => void;
}) {
  const [enemies, setEnemies] = useState(() =>
    setup.enemies.map(({ id }) => id),
  );
  const [slots, setSlots] = useState(setup.team_slots);
  const [saving, setSaving] = useState(false);
  const [saveIssue, setSaveIssue] = useState('');
  const [analysis, setAnalysis] = useState<TrainingAnalysis>();
  const [running, setRunning] = useState<AnalysisKind>();
  const [analysisIssue, setAnalysisIssue] = useState('');
  const dirty = slots.some((id, index) => id !== setup.team_slots[index]);
  const locked = starting || saving || running !== undefined;
  const ready = !dirty && slots.some((id) => id !== null) && enemies.length > 0;

  async function saveTeam() {
    setSaving(true);
    setSaveIssue('');
    try {
      onTeam(await training.saveTeam(slots));
    } catch (error) {
      setSaveIssue(describeError(error));
    } finally {
      setSaving(false);
    }
  }

  function clearAnalysis() {
    setAnalysis(undefined);
    setAnalysisIssue('');
  }

  async function analyze(kind: AnalysisKind) {
    setRunning(kind);
    clearAnalysis();
    try {
      setAnalysis(await training.analyze(enemies, kind));
    } catch (error) {
      setAnalysisIssue(describeError(error));
    } finally {
      setRunning(undefined);
    }
  }

  return (
    <>
      <section
        className="field-zone enemy-zone"
        aria-labelledby="training-enemy-title"
      >
        <h3 id="training-enemy-title">敌方 · 训练靶机</h3>
        <div className="enemy-slots" role="group" aria-label="选择训练靶机">
          {setup.enemies.map(({ id, name }) => (
            <button
              key={id}
              type="button"
              className="choice"
              aria-pressed={enemies.includes(id)}
              disabled={locked}
              onClick={() => {
                clearAnalysis();
                setEnemies((current) =>
                  current.includes(id)
                    ? current.filter((enemy) => enemy !== id)
                    : [...current, id],
                );
              }}
            >
              {name}
            </button>
          ))}
        </div>
      </section>
      <TeamBoard
        roster={setup.profiles}
        slots={slots}
        onChange={(slots) => {
          setSlots(slots);
          clearAnalysis();
        }}
        disabled={locked}
      />
      <div className="form-actions">
        <button
          type="button"
          className="secondary"
          disabled={locked}
          onClick={() => {
            clearAnalysis();
            onLoadout();
          }}
        >
          训练配装
        </button>
        <button
          type="button"
          className="secondary"
          disabled={!dirty || locked}
          onClick={() => void saveTeam()}
        >
          {saving ? '正在保存…' : '保存站位'}
        </button>
        <span role="status">{dirty ? '有未保存的站位。' : ''}</span>
      </div>
      <div className="form-actions">
        <button
          type="button"
          disabled={!ready || locked}
          onClick={() => onStart(enemies)}
        >
          {starting ? '正在开战…' : '开始战斗'}
        </button>
        {onResume && (
          <button
            type="button"
            className="secondary"
            disabled={locked}
            onClick={onResume}
          >
            继续战斗
          </button>
        )}
        <button
          type="button"
          className="secondary"
          disabled={!ready || locked}
          onClick={() => void analyze('dpr')}
        >
          {running === 'dpr' ? '正在计算…' : '计算 DPR'}
        </button>
        <button
          type="button"
          className="secondary"
          disabled={!ready || locked}
          onClick={() => void analyze('fastest_victory')}
        >
          {running === 'fastest_victory' ? '正在搜索…' : '计算最优解'}
        </button>
      </div>
      {analysis && <AnalysisResult analysis={analysis} />}
      {(saveIssue || issue) && <p role="alert">{saveIssue || issue}</p>}
      {analysisIssue && <p role="alert">{analysisIssue}</p>}
    </>
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
