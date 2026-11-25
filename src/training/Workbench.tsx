import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { CharacterPage } from '../characters/CharacterPage';
import { LightConePage } from '../characters/LightConePage';
import { RelicPage } from '../characters/RelicPage';
import { catalogError, characters } from '../characters/client';
import type { Catalog, SavedTeam } from '../characters/client';
import { Diagnostics } from '../desktop/Diagnostics';
import { RelicExportPage } from '../relic-export/RelicExportPage';
import {
  accountError,
  gameAccounts,
  setAccountScope,
} from '../relic-export/accounts';
import type { AccountSelection } from '../relic-export/accounts';
import {
  enemyFormationError,
  enemyFormations,
} from '../team/enemyFormationClient';
import type { EnemyFormation } from '../team/enemyFormationClient';
import { TeamWorkspace } from '../team/TeamWorkspace';
import type { FormationSide } from '../team/TeamWorkspace';
import { BattlePanel } from './Battle';
import { History } from './History';
import { TrainingLoadouts, TrainingSetup } from './Setup';
import type { SimulationTab } from './SimulationWorkspace';
import { describeError, errorCode, training } from './client';
import type { BattleView, Command, Profile, Setup, TeamSlots } from './client';

const pages = [
  { id: 'setup', name: '编队' },
  { id: 'simulation', name: '战斗模拟' },
  { id: 'characters', name: '角色' },
  { id: 'relics', name: '遗器' },
  { id: 'light-cones', name: '光锥库' },
  { id: 'relic-export', name: '账号导入' },
] as const;

const EnemyFormations = lazy(() => import('../team/EnemyFormations'));
const SimulationWorkspace = lazy(() => import('./SimulationWorkspace'));

type Page = (typeof pages)[number]['id'] | 'battle' | 'training-loadout';

export function Workbench() {
  const [page, setPage] = useState<Page>('setup');
  const [formationSide, setFormationSide] = useState<FormationSide>('ally');
  const [simulationTab, setSimulationTab] = useState<SimulationTab>('match');
  const [formations, setFormations] = useState<EnemyFormation[]>();
  const [formationIssue, setFormationIssue] = useState('');
  const [setup, setSetup] = useState<Setup>();
  const [initialError, setInitialError] = useState('');
  const [catalog, setCatalog] = useState<Catalog>();
  const [catalogIssue, setCatalogIssue] = useState('');
  const [rosterIssue, setRosterIssue] = useState('');
  const [catalogBusy, setCatalogBusy] = useState(true);
  const [accountSelection, setAccountSelection] = useState<AccountSelection>();
  const [accountIssue, setAccountIssue] = useState('');
  const [archiveRevision, setArchiveRevision] = useState(0);
  const [battle, setBattle] = useState<BattleView>();
  const [startIssue, setStartIssue] = useState('');
  const [actionIssue, setActionIssue] = useState('');
  const [busy, setBusy] = useState(false);
  const main = useRef<HTMLElement>(null);
  const requestedFocus = useRef(false);
  const loadingTraining = useRef(false);
  const currentRevision = useRef(0);
  const catalogRequest = useRef(0);

  const activeAccount = accountSelection?.accounts.find(
    ({ uid }) => uid === accountSelection.active_uid,
  );

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

  function selectSimulation(tab: SimulationTab) {
    setSimulationTab(tab);
    if (tab === 'training' && !setup) void load();
  }

  useEffect(() => {
    void loadAccounts();
    void loadEnemyFormations();
  }, []);

  async function loadEnemyFormations() {
    setFormationIssue('');
    try {
      setFormations(await enemyFormations.list());
    } catch (error) {
      setFormationIssue(enemyFormationError(error));
    }
  }

  function updateEnemyFormation(formation: EnemyFormation) {
    setFormations((current) => {
      const saved = current ?? [];
      return saved.some(({ id }) => id === formation.id)
        ? saved.map((item) => (item.id === formation.id ? formation : item))
        : [...saved, formation];
    });
  }

  async function loadAccounts() {
    setAccountIssue('');
    try {
      updateAccountSelection(await gameAccounts.list());
    } catch (error) {
      const message = accountError(error);
      setAccountIssue(message);
      setCatalogIssue(message);
      setCatalogBusy(false);
    }
  }

  function updateAccountSelection(next: AccountSelection, refresh = false) {
    const switched = accountSelection?.active_uid !== next.active_uid;
    setAccountScope(next.active_uid);
    setAccountSelection(next);
    if (switched) {
      currentRevision.current += 1;
      setArchiveRevision(currentRevision.current);
      setCatalog(undefined);
      setRosterIssue('');
    }
    if (switched || refresh) void loadCatalog();
  }

  async function loadCatalog(file?: File) {
    const request = ++catalogRequest.current;
    setCatalogBusy(true);
    setCatalogIssue('');
    try {
      if (file && file.size > 8 * 1024 * 1024) {
        setCatalogIssue('数据文件超过 8 MB，请选择项目生成的 catalog.json。');
        return;
      }
      const json = await file?.text();
      if (request !== catalogRequest.current) return;
      const next =
        json === undefined
          ? await characters.index()
          : await characters.importCatalog(json);
      if (request === catalogRequest.current) setCatalog(next);
    } catch (error) {
      if (request === catalogRequest.current)
        setCatalogIssue(catalogError(error));
    } finally {
      if (request === catalogRequest.current) setCatalogBusy(false);
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
      if (archiveRevision === currentRevision.current)
        setCatalog((current) => current && { ...current, characters: roster });
    } catch (error) {
      if (archiveRevision === currentRevision.current)
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
    if (archiveRevision !== currentRevision.current) return;
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
              {activeAccount && ` · ${activeAccount.remark}`}
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
                (id === 'simulation' &&
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
            accountRevision={archiveRevision}
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
            side={formationSide}
            onSideChange={setFormationSide}
            enemyContent={
              formations ? (
                <Suspense fallback={<p role="status">正在打开敌方编队…</p>}>
                  <EnemyFormations
                    formations={formations}
                    onSaved={updateEnemyFormation}
                  />
                </Suspense>
              ) : formationIssue ? (
                <>
                  <p role="alert">{formationIssue}</p>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void loadEnemyFormations()}
                  >
                    重试
                  </button>
                </>
              ) : (
                <p role="status">正在读取敌方编队…</p>
              )
            }
          />
        </div>
        <div hidden={page !== 'simulation'}>
          <Suspense fallback={<p role="status">正在打开战斗模拟…</p>}>
            <SimulationWorkspace
              catalog={catalog}
              catalogLoading={catalogBusy}
              accountRevision={archiveRevision}
              formations={formations}
              formationIssue={formationIssue}
              tab={simulationTab}
              onTabChange={selectSimulation}
              onEdit={(side) => {
                setFormationSide(side);
                navigate('setup');
              }}
              training={
                <section className="panel" aria-label="合成训练">
                  {setup ? (
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
                    simulationTab === 'training' && (
                      <TrainingStatus
                        error={initialError}
                        onRetry={() => void load()}
                      />
                    )
                  )}
                </section>
              }
              history={
                page === 'simulation' &&
                simulationTab === 'history' && (
                  <History
                    onResume={(entry) => {
                      setBattle(entry);
                      setActionIssue('');
                      navigate('battle');
                    }}
                  />
                )
              }
            />
          </Suspense>
        </div>
        {page === 'training-loadout' && (
          <>
            <div className="team-mode">
              <button
                type="button"
                className="secondary"
                onClick={() => navigate('simulation')}
              >
                返回战斗模拟
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
              <CharacterPage key={archiveRevision} catalog={catalog} />
            ) : page === 'relics' ? (
              <RelicPage key={archiveRevision} catalog={catalog} />
            ) : (
              <LightConePage key={archiveRevision} catalog={catalog} />
            )
          ) : (
            <section className="panel">
              <h2>正式服数据</h2>
              {catalogBusy ? (
                <p role="status">正在读取数据…</p>
              ) : (
                <>
                  <p role="status">{catalogIssue}</p>
                  {accountSelection && (
                    <>
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
                        选择项目生成的 <code>catalog.json</code>
                        ，载入后可离线使用。
                      </p>
                    </>
                  )}
                </>
              )}
            </section>
          ))}
        <div hidden={page !== 'relic-export'}>
          <RelicExportPage
            selection={accountSelection}
            accountIssue={accountIssue}
            onRetry={() => void loadAccounts()}
            onSelection={updateAccountSelection}
            onImported={({ selection, uid }) => {
              updateAccountSelection(
                selection,
                uid === selection.active_uid || uid === null,
              );
            }}
          />
        </div>
        {page === 'battle' && battle && (
          <>
            <div className="team-mode">
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => navigate('simulation')}
              >
                返回战斗模拟
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
