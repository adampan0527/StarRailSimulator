import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { CharacterSummary } from '../training/client';
import {
  CharacterArtwork,
  CharacterCardBody,
  CharacterStatGrid,
} from './CharacterCard';
import { CharacterFilters, matchesCharacterTraits } from './CharacterFilters';
import { CharacterLightCone, sameLightCone } from './CharacterLightCone';
import { CharacterRelics } from './CharacterRelics';
import { CombatTraitIcon } from './CombatTrait';
import {
  catalogError,
  characters,
  relicSlots,
  statName,
  statValue,
} from './client';
import type {
  Catalog,
  CharacterProfile,
  InventoryItem,
  InventoryLightCone,
  RelicSlot,
} from './client';
import { compareCharacterRelease } from './releaseOrder';

type CharacterEditorData = {
  profile: CharacterProfile;
  inventory: InventoryItem[];
  lightCones: InventoryLightCone[];
};

export function CharacterPage({ catalog }: { catalog: Catalog }) {
  const [selected, setSelected] = useState<string>();
  const [query, setQuery] = useState('');
  const [traits, setTraits] = useState({ element: '', path: '' });
  const [locked, setLocked] = useState(false);
  const editorHeading = useRef<HTMLHeadingElement>(null);
  const listPosition = useRef<{ card: HTMLButtonElement; scrollY: number }>(
    null,
  );
  const active = catalog.characters.find((entry) => entry.id === selected);
  const activeId = active?.id;
  const nameFilter = query.trim().toLowerCase();
  const results = catalog.characters
    .filter(
      (entry) =>
        entry.name.toLowerCase().includes(nameFilter) &&
        matchesCharacterTraits(entry, traits),
    )
    .sort(compareCharacterRelease);

  useLayoutEffect(() => {
    if (activeId) {
      editorHeading.current?.focus({ preventScroll: true });
      window.scrollTo(0, 0);
    } else if (listPosition.current) {
      const { card, scrollY } = listPosition.current;
      card.focus({ preventScroll: true });
      window.scrollTo(0, scrollY);
      listPosition.current = null;
    }
  }, [activeId]);

  return (
    <>
      <section
        className="panel"
        aria-labelledby="characters-title"
        hidden={!!active}
      >
        <div className="section-heading">
          <div>
            <h2 id="characters-title">角色列表</h2>
            <p className="muted" role="status">
              {results.length
                ? `${results.length} 个角色`
                : catalog.characters.length
                  ? '没有符合条件的角色。'
                  : '角色目录为空。'}
            </p>
          </div>
          <label className="character-search">
            搜索角色
            <input
              type="search"
              value={query}
              placeholder="输入角色名称"
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
        </div>
        {catalog.characters.length > 0 && (
          <CharacterFilters
            roster={catalog.characters}
            value={traits}
            onChange={setTraits}
          />
        )}
        {results.length > 0 && (
          <ul className="character-roster" role="list">
            {results.map((entry) => (
              <li key={entry.id}>
                <button
                  className="choice character-roster-card"
                  type="button"
                  onClick={(event) => {
                    listPosition.current = {
                      card: event.currentTarget,
                      scrollY: window.scrollY,
                    };
                    setSelected(entry.id);
                  }}
                >
                  <CharacterCardBody character={entry} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
      {active && (
        <section className="panel" aria-labelledby="character-editor-title">
          <div className="section-heading">
            <h2 id="character-editor-title" ref={editorHeading} tabIndex={-1}>
              {active.name} · 角色配置
            </h2>
            <button
              type="button"
              className="secondary"
              disabled={locked}
              onClick={() => setSelected(undefined)}
            >
              返回角色列表
            </button>
          </div>
          <CharacterEditor
            key={active.id}
            active={active}
            catalog={catalog}
            onLock={setLocked}
          />
        </section>
      )}
    </>
  );
}

function CharacterEditor({
  active,
  catalog,
  onLock,
}: {
  active: CharacterSummary;
  catalog: Catalog;
  onLock: (locked: boolean) => void;
}) {
  const [loaded, setLoaded] = useState<CharacterEditorData>();
  const [issue, setIssue] = useState('');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let canceled = false;
    void Promise.all([
      characters.profile(active.id),
      characters.inventory(),
      characters.lightConeInventory(),
    ])
      .then(([profile, inventory, lightCones]) => {
        if (!canceled) setLoaded({ profile, inventory, lightCones });
      })
      .catch((error: unknown) => {
        if (!canceled) setIssue(catalogError(error));
      });
    return () => {
      canceled = true;
    };
  }, [active.id, attempt]);

  if (issue)
    return (
      <div>
        <p role="alert">{issue}</p>
        <button
          type="button"
          onClick={() => {
            setIssue('');
            setAttempt(attempt + 1);
          }}
        >
          重试
        </button>
      </div>
    );
  if (!loaded) return <p role="status">正在读取角色档案…</p>;
  return (
    <CharacterForm
      active={active}
      catalog={catalog}
      initial={loaded}
      onLock={onLock}
    />
  );
}

function CharacterForm({
  active,
  catalog,
  initial,
  onLock,
}: {
  active: CharacterSummary;
  catalog: Catalog;
  initial: CharacterEditorData;
  onLock: (locked: boolean) => void;
}) {
  const [profile, setProfile] = useState(initial.profile);
  const [items, setItems] = useState(initial.inventory);
  const [lightCones, setLightCones] = useState(initial.lightCones);
  const [draft, setDraft] = useState(profile.edit);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const dirty =
    draft.eidolon !== profile.edit.eidolon ||
    draft.affinitive_eidolon !== profile.edit.affinitive_eidolon ||
    !sameLightCone(draft.light_cone, profile.edit.light_cone) ||
    relicSlots.some(
      ([slot]) => draft.relics[slot] !== profile.edit.relics[slot],
    );

  useEffect(() => {
    onLock(dirty || saving);
    return () => onLock(false);
  }, [dirty, saving, onLock]);

  function equip(slot: RelicSlot, value: number | null) {
    const relics = { ...draft.relics };
    if (value !== null) relics[slot] = value;
    else delete relics[slot];
    setDraft({ ...draft, relics });
    setStatus('');
  }

  async function save() {
    setSaving(true);
    setStatus('');
    setError('');
    try {
      const [updated, inventory, cones] = await characters.save(
        active.id,
        draft,
      );
      setProfile(updated);
      setItems(inventory);
      setLightCones(cones);
      setDraft(updated.edit);
      setStatus('角色配置已保存。');
    } catch (error) {
      setError(catalogError(error));
      const [relics, cones] = await Promise.allSettled([
        characters.inventory(),
        characters.lightConeInventory(),
      ]);
      if (relics.status === 'fulfilled') setItems(relics.value);
      if (cones.status === 'fulfilled') setLightCones(cones.value);
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      className="character-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (dirty && !saving) void save();
      }}
    >
      <div className="character-identity">
        <CharacterArtwork character={active} />
        <div className="field-stack">
          <p className="muted combat-trait-inline">
            <CombatTraitIcon kind="element" id={active.element} />
            <CombatTraitIcon kind="path" id={active.path} />
            <span>Lv. {profile.level}</span>
          </p>
          <div>
            <p className="muted">角色属性 · 已保存配装</p>
            <CharacterStatGrid
              element={active.element}
              values={profile.stats}
            />
            {Object.keys(profile.trace_bonuses).length > 0 && (
              <details className="trace-bonuses">
                <summary>查看已解锁行迹加成</summary>
                <dl>
                  {Object.entries(profile.trace_bonuses).map(([id, value]) => (
                    <div key={id}>
                      <dt>{statName(catalog, id)}</dt>
                      <dd>+{statValue(id, value)}</dd>
                    </div>
                  ))}
                </dl>
              </details>
            )}
          </div>
        </div>
      </div>
      <fieldset className="character-fields" disabled={saving}>
        <div className="character-eidolons">
          <CharacterLightCone
            catalog={catalog}
            active={active}
            inventory={lightCones}
            saved={profile.edit.light_cone}
            value={draft.light_cone}
            onChange={(light_cone) => {
              setDraft({ ...draft, light_cone });
              setStatus('');
            }}
          />
          <label>
            星魂等级
            <select
              value={draft.eidolon}
              onChange={(event) => {
                setDraft({ ...draft, eidolon: Number(event.target.value) });
                setStatus('');
              }}
            >
              <option value={0}>E0 · 未解锁星魂</option>
              {profile.eidolons.map((eidolon, index) => (
                <option key={index} value={index + 1}>
                  E{index + 1} · {eidolon.name}
                </option>
              ))}
            </select>
          </label>
          <details className="eidolon-details">
            <summary>查看星魂效果</summary>
            <ol>
              {profile.eidolons.map((eidolon, index) => (
                <li key={index}>
                  <strong>{eidolon.name}</strong>{' '}
                  <span className="muted">
                    {draft.eidolon > index ? '已解锁' : '未解锁'}
                  </span>
                  <p>{eidolon.description}</p>
                </li>
              ))}
            </ol>
          </details>
          {profile.special_eidolon && (
            <div className="field-stack">
              <label className="check-label">
                <input
                  type="checkbox"
                  checked={draft.affinitive_eidolon}
                  onChange={(event) => {
                    setDraft({
                      ...draft,
                      affinitive_eidolon: event.target.checked,
                    });
                    setStatus('');
                  }}
                />
                奇缘星魂 · {profile.special_eidolon.name}
              </label>
              <p className="muted">{profile.special_eidolon.description}</p>
            </div>
          )}
        </div>
        <CharacterRelics
          catalog={catalog}
          items={items}
          characterId={active.id}
          equipment={draft.relics}
          onEquip={equip}
        />
      </fieldset>
      <div className="form-actions">
        <button type="submit" disabled={!dirty || saving}>
          {saving ? '正在保存…' : '保存角色配置'}
        </button>
        {dirty && (
          <button
            type="button"
            className="secondary"
            disabled={saving}
            onClick={() => {
              setDraft(profile.edit);
              setError('');
            }}
          >
            还原更改
          </button>
        )}
        <span role="status">{status || (dirty ? '有未保存的更改。' : '')}</span>
      </div>
      {error && <p role="alert">{error}</p>}
    </form>
  );
}
