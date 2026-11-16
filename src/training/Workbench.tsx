import { useEffect, useRef, useState } from 'react';
import { Diagnostics } from '../desktop/Diagnostics';
import { BattlePanel } from './Battle';
import { History } from './History';
import { TrainingSetup } from './Setup';
import { describeError, errorCode, training } from './client';
import type { BattleView, Command, Profile, Setup, Team } from './client';

const pages = [
  { id: 'setup', name: '配装与组队' },
  { id: 'battle', name: '训练战斗' },
  { id: 'history', name: '历史回放' },
] as const;

type Page = (typeof pages)[number]['id'];

export function Workbench() {
  const [page, setPage] = useState<Page>('setup');
  const [setup, setSetup] = useState<Setup>();
  const [initialError, setInitialError] = useState('');
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
  }, []);

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

  function updateTeam(team: Team) {
    setSetup((current) => current && { ...current, team });
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
            <p>离线训练 · 合成规则</p>
          </div>
        </div>
        <nav aria-label="工作台">
          {pages.map(({ id, name }) => (
            <button
              key={id}
              type="button"
              className="nav-button"
              aria-current={page === id ? 'page' : undefined}
              disabled={busy || !setup}
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
        aria-label={pages.find(({ id }) => page === id)?.name}
      >
        {setup ? (
          <>
            <div hidden={page !== 'setup'}>
              <TrainingSetup
                setup={setup}
                starting={busy}
                issue={startIssue}
                onProfile={updateProfile}
                onTeam={updateTeam}
                onStart={(enemies) => void start(enemies)}
              />
            </div>
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
                  <button type="button" onClick={() => navigate('setup')}>
                    去准备队伍
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
          </>
        ) : (
          <section className="panel load-panel">
            {initialError ? (
              <>
                <h2>无法打开训练工作台</h2>
                <p role="alert">{initialError}</p>
                <button type="button" onClick={load}>
                  重试
                </button>
              </>
            ) : (
              <p role="status">正在读取角色档案…</p>
            )}
          </section>
        )}
      </main>
      <footer>
        <Diagnostics />
      </footer>
    </div>
  );
}
