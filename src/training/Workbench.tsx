import { useEffect, useRef, useState } from 'react';
import { CharacterPage } from '../characters/CharacterPage';
import { LightConePage } from '../characters/LightConePage';
import { RelicPage } from '../characters/RelicPage';
import { catalogError, characters } from '../characters/client';
import type { Catalog, SavedTeam } from '../characters/client';
import { Diagnostics } from '../desktop/Diagnostics';
import { RelicExportPage } from '../relic-export/RelicExportPage';
import { TeamWorkspace } from '../team/TeamWorkspace';
import { BattlePanel } from './Battle';
import { History } from './History';
import { TrainingLoadouts, TrainingSetup } from './Setup';
import { describeError, errorCode, training } from './client';
import type { BattleView, Command, Profile, Setup, TeamSlots } from './client';

const pages = [
  { id: 'setup', name: '组队' },
  { id: 'characters', name: '角色' },
  { id: 'relics', name: '遗器' },
  { id: 'relic-export', name: '账号导入' },
  { id: 'light-cones', name: '光锥库' },
  { id: 'history', name: '历史回放' },
] as const;

type Page = (typeof pages)[number]['id'] | 'battle' | 'training-loadout';

export function Workbench() {
  const [page, setPage] = useState<Page>('setup');
  const [trainingMode, setTrainingMode] = useState(false);
  const [setup, setSetup] = useState<Setup>();
  const [initialError, setInitialError] = useState('');
  const [catalog, setCatalog] = useState<Catalog>();
  const [catalogIssue, setCatalogIssue] = useState('');
  const [rosterIssue, setRosterIssue] = useState('');
  const [catalogBusy, setCatalogBusy] = useState(true);
  const [battle, setBattle] = useState<BattleView>();
  const [startIssue, setStartIssue] = useState('');
  const [actionIssue, setActionIssue] = useState('');
  const [busy, setBusy] = useState(false);
  const main = useRef<HTMLElement>(null);
  const requestedFocus = useRef(false);
  const loadingTraining = useRef(false);

  async function load() {
    if (loadingTraining.current) return;
    loadingTraining.current = true;
    setInitialError('');
    try {
      setSetup(await training.setup());
    } catch (error) {
      setInitialError(describeError(error));
    } finally {
      loadingTraining.current = false;
    }
  }

  function selectBattlefield(isTraining: boolean) {
    setTrainingMode(isTraining);
    if (isTraining && !setup) void load();
  }

  useEffect(() => {
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

  function updateCatalogTeam(team: SavedTeam) {
    setCatalog((current) => {
      if (!current) return current;
      const teams = current.teams.some(({ id }) => id === team.id)
        ? current.teams.map((item) => (item.id === team.id ? team : item))
        : [...current.teams, team];
      return { ...current, teams };
    });
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
                : '离线工作台'}
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
                page === id ||
                (id === 'setup' &&
                  (page === 'battle' || page === 'training-loadout'))
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
        aria-label={
          pages.find(({ id }) => page === id)?.name ??
          (page === 'battle' ? '战斗' : '训练配装')
        }
      >
        <div hidden={page !== 'setup'}>
          <TeamWorkspace
            catalog={catalog}
            catalogStatus={
              catalogBusy ? (
                <p role="status">正在读取角色库…</p>
              ) : (
                <section className="field-zone">
                  <h3>己方</h3>
                  {catalogIssue && <p role="alert">{catalogIssue}</p>}
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => navigate('characters')}
                  >
                    前往角色库
                  </button>
                </section>
              )
            }
            onSaved={updateCatalogTeam}
            rosterIssue={rosterIssue}
            training={trainingMode}
            onModeChange={selectBattlefield}
            busy={busy}
            trainingContent={
              setup ? (
                <TrainingSetup
                  setup={setup}
                  starting={busy}
                  issue={startIssue}
                  onTeam={updateTeam}
                  onStart={(enemies) => void start(enemies)}
                  onLoadout={() => navigate('training-loadout')}
                  onResume={
                    battle?.snapshot.outcome === 'ongoing'
                      ? () => navigate('battle')
                      : undefined
                  }
                />
              ) : (
                trainingMode && (
                  <TrainingStatus
                    error={initialError}
                    onRetry={() => void load()}
                  />
                )
              )
            }
          />
        </div>
        {page === 'training-loadout' && (
          <>
            <div className="team-mode">
              <button
                type="button"
                className="secondary"
                onClick={() => navigate('setup')}
              >
                返回组队
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
        <div hidden={page !== 'relic-export'}>
          <RelicExportPage />
        </div>
        {page === 'battle' && battle && (
          <>
            <div className="team-mode">
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => navigate('setup')}
              >
                返回组队
              </button>
            </div>
            <BattlePanel
              key={battle.id}
              battle={battle}
              playing
              busy={busy}
              issue={actionIssue}
              onAct={(command) => void act(command)}
            />
          </>
        )}
        {page === 'history' && (
          <History
            onResume={(entry) => {
              setBattle(entry);
              setActionIssue('');
              selectBattlefield(true);
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
    <section className="field-zone">
      {error ? (
        <>
          <p role="alert">{error}</p>
          <button type="button" onClick={onRetry}>
            重试
          </button>
        </>
      ) : (
        <p role="status">正在读取合成战斗…</p>
      )}
    </section>
  );
}
