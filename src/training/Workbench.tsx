import { useEffect, useRef, useState } from 'react';
import { CharacterPage } from '../characters/CharacterPage';
import { LightConePage } from '../characters/LightConePage';
import { RelicPage } from '../characters/RelicPage';
import { catalogError, characters } from '../characters/client';
import type { Catalog } from '../characters/client';
import { Diagnostics } from '../desktop/Diagnostics';
import { TeamBoard } from '../team/TeamBoard';
import { BattlePanel } from './Battle';
import { History } from './History';
import { TrainingLoadouts, TrainingSetup } from './Setup';
import { describeError, errorCode, training } from './client';
import type { BattleView, Command, Profile, Setup, TeamSlots } from './client';

const pages = [
  { id: 'setup', name: '组队' },
  { id: 'characters', name: '角色' },
  { id: 'relics', name: '遗器' },
  { id: 'light-cones', name: '光锥库' },
  { id: 'battle', name: '训练战斗' },
  { id: 'history', name: '历史回放' },
] as const;

type Page = (typeof pages)[number]['id'] | 'training-loadout';

export function Workbench() {
  const [page, setPage] = useState<Page>('setup');
  const [setup, setSetup] = useState<Setup>();
  const [initialError, setInitialError] = useState('');
  const [catalog, setCatalog] = useState<Catalog>();
  const [catalogIssue, setCatalogIssue] = useState('');
  const [rosterIssue, setRosterIssue] = useState('');
  const [catalogBusy, setCatalogBusy] = useState(true);
  const [trainingMode, setTrainingMode] = useState(false);
  const [battle, setBattle] = useState<BattleView>();
  const [startIssue, setStartIssue] = useState('');
  const [actionIssue, setActionIssue] = useState('');
  const [busy, setBusy] = useState(false);
  const main = useRef<HTMLElement>(null);
  const requestedFocus = useRef(false);

  async function load() {
    setInitialError('');
    try {
      setSetup(await training.setup());
    } catch (error) {
      setInitialError(describeError(error));
    }
  }

  useEffect(() => {
    void load();
    void loadCatalog();
  }, []);

  async function loadCatalog(file?: File) {
    setCatalogBusy(true);
    setCatalogIssue('');
    try {
      if (file && file.size > 8 * 1024 * 1024) {
        setCatalogIssue('数据文件超过 8 MB，请选择项目生成的 catalog.json。');
        return;
      }
      setCatalog(
        file
          ? await characters.importCatalog(await file.text())
          : await characters.index(),
      );
      setTrainingMode(false);
    } catch (error) {
      setCatalogIssue(catalogError(error));
    } finally {
      setCatalogBusy(false);
    }
  }

  useEffect(() => {
    if (!requestedFocus.current) return;
    requestedFocus.current = false;
    main.current?.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }, [page]);

  function navigate(next: Page) {
    if (next !== page) {
      requestedFocus.current = true;
      setPage(next);
      if (next === 'setup' && catalog) void refreshRoster();
    }
  }

  async function refreshRoster() {
    setRosterIssue('');
    try {
      const roster = await characters.roster();
      setCatalog((current) => current && { ...current, characters: roster });
    } catch (error) {
      setRosterIssue(catalogError(error));
    }
  }

  function updateProfile(profile: Profile) {
    setSetup(
      (current) =>
        current && {
          ...current,
          profiles: current.profiles.map((item) =>
            item.id === profile.id ? profile : item,
          ),
        },
    );
  }

  function updateTeam(slots: TeamSlots) {
    setSetup((current) => current && { ...current, team_slots: slots });
  }

  function updateCatalogTeam(slots: TeamSlots) {
    setCatalog((current) => current && { ...current, team_slots: slots });
  }

  async function start(enemies: string[]) {
    setBusy(true);
    setStartIssue('');
    try {
      setBattle(await training.start(enemies));
      setActionIssue('');
      navigate('battle');
    } catch (error) {
      setStartIssue(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function act(command: Command) {
    if (!battle) return;
    setBusy(true);
    setActionIssue('');
    try {
      setBattle(await training.act(battle.id, command));
    } catch (error) {
      setActionIssue(describeError(error));
      if (errorCode(error) === 'stale_action') {
        try {
          setBattle(await training.replay(battle.id));
        } catch (reloadError) {
          setActionIssue(describeError(reloadError));
        }
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="shell">
      <header className="app-header">
        <div className="app-identity">
          <span className="app-mark" aria-hidden="true" />
          <div>
            <h1>星穹铁道战斗模拟器</h1>
            <p>
              {catalog
                ? `离线工作台 · 数据 ${catalog.game_version}`
                : '离线训练 · 合成规则'}
            </p>
          </div>
        </div>
        <nav aria-label="工作台">
          {pages.map(({ id, name }) => (
            <button
              key={id}
              type="button"
              className="nav-button"
              aria-current={
                page === id || (id === 'setup' && page === 'training-loadout')
                  ? 'page'
                  : undefined
              }
              disabled={busy}
              onClick={() => navigate(id)}
            >
              {name}
            </button>
          ))}
        </nav>
      </header>
      <main
        ref={main}
        tabIndex={-1}
        aria-label={pages.find(({ id }) => page === id)?.name ?? '训练配装'}
      >
        {page === 'setup' && (
          <>
            <div className="team-mode">
              {catalog ? (
                <div role="group" aria-label="队伍类型">
                  <button
                    type="button"
                    className="choice"
                    aria-pressed={!trainingMode}
                    disabled={busy}
                    onClick={() => setTrainingMode(false)}
                  >
                    正式服队伍
                  </button>
                  <button
                    type="button"
                    className="choice"
                    aria-pressed={trainingMode}
                    disabled={busy}
                    onClick={() => setTrainingMode(true)}
                  >
                    合成训练
                  </button>
                </div>
              ) : (
                !catalogBusy && (
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => navigate('characters')}
                  >
                    载入正式服数据
                  </button>
                )
              )}
            </div>
            {catalog && (
              <div hidden={trainingMode}>
                <TeamBoard
                  roster={catalog.characters}
                  savedSlots={catalog.team_slots}
                  save={characters.saveTeam}
                  onSaved={updateCatalogTeam}
                  issue={rosterIssue}
                />
              </div>
            )}
            <div hidden={catalog ? !trainingMode : catalogBusy}>
              {setup ? (
                <TrainingSetup
                  setup={setup}
                  starting={busy}
                  issue={startIssue}
                  onTeam={updateTeam}
                  onStart={(enemies) => void start(enemies)}
                  onLoadout={() => navigate('training-loadout')}
                />
              ) : (
                <TrainingStatus
                  error={initialError}
                  onRetry={() => void load()}
                />
              )}
            </div>
            {!catalog && catalogBusy && (
              <p role="status" className="panel">
                正在读取数据…
              </p>
            )}
          </>
        )}
        {page === 'training-loadout' && (
          <>
            <div className="team-mode">
              <button
                type="button"
                className="secondary"
                onClick={() => navigate('setup')}
              >
                返回训练队伍
              </button>
            </div>
            {setup && (
              <TrainingLoadouts setup={setup} onProfile={updateProfile} />
            )}
          </>
        )}
        {(page === 'characters' ||
          page === 'relics' ||
          page === 'light-cones') &&
          (catalog ? (
            page === 'characters' ? (
              <CharacterPage catalog={catalog} />
            ) : page === 'relics' ? (
              <RelicPage catalog={catalog} />
            ) : (
              <LightConePage catalog={catalog} />
            )
          ) : (
            <section className="panel">
              <h2>正式服数据</h2>
              {catalogBusy ? (
                <p role="status">正在读取数据…</p>
              ) : (
                <>
                  <p role="status">{catalogIssue}</p>
                  <label className="catalog-file">
                    选择数据文件
                    <input
                      type="file"
                      accept=".json,application/json"
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        if (file) void loadCatalog(file);
                      }}
                    />
                  </label>
                  <p className="muted">
                    选择项目生成的 <code>catalog.json</code>，载入后可离线使用。
                  </p>
                </>
              )}
            </section>
          ))}
        {page === 'battle' &&
          (battle ? (
            <BattlePanel
              key={battle.id}
              battle={battle}
              playing
              busy={busy}
              issue={actionIssue}
              onAct={(command) => void act(command)}
            />
          ) : (
            <section className="panel">
              <h2>尚未开始训练</h2>
              <p>选择阵容和训练目标后即可开战。</p>
              <button
                type="button"
                onClick={() => {
                  setTrainingMode(true);
                  navigate('setup');
                }}
              >
                去准备训练队伍
              </button>
            </section>
          ))}
        {page === 'history' && (
          <History
            onResume={(entry) => {
              setBattle(entry);
              setActionIssue('');
              navigate('battle');
            }}
          />
        )}
      </main>
      <footer>
        <Diagnostics />
      </footer>
    </div>
  );
}

function TrainingStatus({
  error,
  onRetry,
}: {
  error: string;
  onRetry: () => void;
}) {
  return (
    <section className="panel">
      {error ? (
        <>
          <p role="alert">{error}</p>
          <button type="button" onClick={onRetry}>
            重试
          </button>
        </>
      ) : (
        <p role="status">正在读取训练队伍…</p>
      )}
    </section>
  );
}
