import { useState } from 'react';
import type { ReactNode } from 'react';
import type { Catalog } from '../characters/client';
import { enemies } from '../team/enemyCatalog';
import type { EnemyFormation } from '../team/enemyFormationClient';
import type { FormationSide } from '../team/TeamWorkspace';

const tabs = [
  { id: 'match', name: '选队' },
  { id: 'history', name: '战斗记录' },
] as const;

export type SimulationTab = (typeof tabs)[number]['id'];

export default function SimulationWorkspace({
  catalog,
  catalogLoading,
  accountRevision,
  formations,
  formationIssue,
  tab,
  onTabChange,
  onEdit,
  history,
}: {
  catalog?: Catalog;
  catalogLoading: boolean;
  accountRevision: number;
  formations?: EnemyFormation[];
  formationIssue: string;
  tab: SimulationTab;
  onTabChange: (tab: SimulationTab) => void;
  onEdit: (side: FormationSide) => void;
  history: ReactNode;
}) {
  return (
    <section
      className="simulation-workspace"
      aria-labelledby="simulation-title"
    >
      <div className="panel">
        <h2 id="simulation-title">战斗模拟</h2>
        <div className="workspace-tabs" role="group" aria-label="战斗模拟视图">
          {tabs.map(({ id, name }) => (
            <button
              className="choice"
              key={id}
              type="button"
              aria-pressed={tab === id}
              onClick={() => onTabChange(id)}
            >
              {name}
            </button>
          ))}
        </div>
      </div>
      <div hidden={tab !== 'match'}>
        <MatchSelection
          key={accountRevision}
          catalog={catalog}
          catalogLoading={catalogLoading}
          formations={formations}
          formationIssue={formationIssue}
          onEdit={onEdit}
        />
      </div>
      <div hidden={tab !== 'history'}>{history}</div>
    </section>
  );
}

function MatchSelection({
  catalog,
  catalogLoading,
  formations,
  formationIssue,
  onEdit,
}: {
  catalog?: Catalog;
  catalogLoading: boolean;
  formations?: EnemyFormation[];
  formationIssue: string;
  onEdit: (side: FormationSide) => void;
}) {
  const [allyId, setAllyId] = useState('');
  const [enemyId, setEnemyId] = useState('');
  const ally = catalog?.teams.find((team) => team.id === allyId);
  const enemy = formations?.find(
    (formation) => String(formation.id) === enemyId,
  );
  const allies = ally?.slots.flatMap((id, position) =>
    id
      ? [
          {
            position,
            name:
              catalog?.characters.find((entry) => entry.id === id)?.name ?? id,
          },
        ]
      : [],
  );
  const opponents = enemy?.slots.flatMap((slot, position) =>
    slot
      ? [
          {
            position,
            name: enemies.get(slot.enemy_id)?.name ?? `敌人 ${slot.enemy_id}`,
            level: slot.level,
          },
        ]
      : [],
  );
  return (
    <section className="panel" aria-labelledby="match-title">
      <h3 id="match-title">选择对阵双方</h3>
      <div className="simulation-match">
        <div className="field-zone">
          <label>
            己方编队
            <select
              value={ally?.id ?? ''}
              disabled={!catalog?.teams.length}
              onChange={(event) => setAllyId(event.target.value)}
            >
              <option value="">选择己方编队</option>
              {catalog?.teams.map((team) => (
                <option key={team.id} value={team.id}>
                  {team.name}
                </option>
              ))}
            </select>
          </label>
          {allies && (
            <ul className="simulation-members" aria-label="己方成员">
              {allies.map(({ position, name }) => (
                <li key={position}>{name}</li>
              ))}
            </ul>
          )}
          {ally && !allies?.length && (
            <p className="muted">这套编队没有上阵角色。</p>
          )}
          {!catalog?.teams.length &&
            (catalogLoading ? (
              <p role="status">正在读取己方编队…</p>
            ) : (
              <button
                className="secondary"
                type="button"
                onClick={() => onEdit('ally')}
              >
                前往己方编队
              </button>
            ))}
        </div>
        <div className="field-zone">
          <label>
            敌方编队
            <select
              value={enemy?.id ?? ''}
              disabled={!formations?.length}
              onChange={(event) => setEnemyId(event.target.value)}
            >
              <option value="">选择敌方编队</option>
              {formations?.map((formation) => (
                <option key={formation.id} value={formation.id}>
                  {formation.name}
                </option>
              ))}
            </select>
          </label>
          {opponents && (
            <ul className="simulation-members" aria-label="敌方成员">
              {opponents.map(({ position, name, level }) => (
                <li key={position}>
                  {name} <span className="muted">{level} 级</span>
                </li>
              ))}
            </ul>
          )}
          {formationIssue && <p role="alert">{formationIssue}</p>}
          {!formations?.length &&
            (!formations && !formationIssue ? (
              <p role="status">正在读取敌方编队…</p>
            ) : (
              <button
                className="secondary"
                type="button"
                onClick={() => onEdit('enemy')}
              >
                前往敌方编队
              </button>
            ))}
        </div>
      </div>
      <p className="muted">
        正式敌方行动规则待接入，目前可选择和查看双方编队。
      </p>
    </section>
  );
}
