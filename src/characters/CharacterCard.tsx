import { useState } from 'react';
import type {
  CharacterStats,
  CharacterSummary,
  ScaledStat,
} from '../training/client';

const elements: Record<string, string> = {
  physical: '物理',
  fire: '火',
  ice: '冰',
  thunder: '雷',
  wind: '风',
  quantum: '量子',
  imaginary: '虚数',
};
const paths: Record<string, string> = {
  destruction: '毁灭',
  hunt: '巡猎',
  erudition: '智识',
  harmony: '同谐',
  nihility: '虚无',
  preservation: '存护',
  abundance: '丰饶',
  remembrance: '记忆',
  elation: '欢愉',
};

export const elementName = (id: string) => elements[id] ?? id;
export const pathName = (id: string) => paths[id] ?? id;

const localPortraits = import.meta.glob<string>(
  '/.m03-data/4.7.0/avatars/*.png',
  { eager: true, import: 'default', query: '?url' },
);

export function CharacterPortrait({
  character,
}: {
  character?: CharacterSummary;
}) {
  const [failed, setFailed] = useState<string[]>([]);
  const source = character?.source_ids
    .map((id) => localPortraits[`/.m03-data/4.7.0/avatars/${id}.png`])
    .find((url) => url && !failed.includes(url));
  return (
    <span className="character-portrait" aria-hidden="true">
      {source ? (
        <img
          src={source}
          alt=""
          width={256}
          height={256}
          loading="lazy"
          onError={() => setFailed((previous) => [...previous, source])}
        />
      ) : (
        <span className="portrait-fallback">
          {character ? Array.from(character.name)[0] : '+'}
        </span>
      )}
    </span>
  );
}

function Stat({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <span className="character-stat">
      <span className="character-stat-name">{label}</span>
      {children}
    </span>
  );
}

function PrimaryStat({
  label,
  value,
  speed = false,
}: {
  label: string;
  value: ScaledStat | null;
  speed?: boolean;
}) {
  const format = (value: number) =>
    speed ? Number(value.toFixed(2)) : Math.round(value);
  const bonus = value ? format(value.total - value.base) : 0;
  return (
    <Stat label={label}>
      <strong className="numeric">{value ? format(value.total) : '—'}</strong>
      {value && bonus !== 0 && (
        <span className="character-stat-breakdown numeric">
          <span aria-label={`基础值 ${format(value.base)}`}>
            {format(value.base)}
          </span>
          <span className={bonus > 0 ? 'stat-bonus' : 'stat-penalty'}>
            {bonus > 0 ? '+' : ''}
            {bonus}
          </span>
        </span>
      )}
    </Stat>
  );
}

function RatioStat({ label, value }: { label: string; value: number | null }) {
  const percent = value === null ? null : (value * 100).toFixed(1);
  const trailingZero = percent?.endsWith('.0');
  return (
    <Stat label={label}>
      <strong className="numeric">
        {percent === null ? (
          '—'
        ) : (
          <>
            {trailingZero ? percent.slice(0, -2) : percent}
            {trailingZero && <span className="stat-trailing-zero">.0</span>}%
          </>
        )}
      </strong>
    </Stat>
  );
}

export function CharacterStatGrid({
  values,
  element,
  id,
}: {
  values: CharacterStats;
  element: string;
  id?: string;
}) {
  return (
    <span className="character-stats" id={id}>
      <PrimaryStat label="生命值" value={values.hp} />
      <RatioStat label="击破特攻" value={values.break_effect} />
      <PrimaryStat label="攻击力" value={values.attack} />
      <RatioStat label="欢愉度" value={values.elation} />
      <PrimaryStat label="防御力" value={values.defense} />
      <RatioStat label="治疗量加成" value={values.healing_boost} />
      <PrimaryStat label="速度" value={values.speed} speed />
      <RatioStat label="能量恢复效率" value={values.energy_regeneration} />
      <RatioStat label="暴击率" value={values.crit_rate} />
      <RatioStat label="效果命中" value={values.effect_hit_rate} />
      <RatioStat label="暴击伤害" value={values.crit_damage} />
      <RatioStat label="效果抵抗" value={values.effect_resistance} />
      <RatioStat
        label={`${elementName(element)}属性伤害提高`}
        value={values.element_damage}
      />
    </span>
  );
}

export function CharacterCardBody({
  character,
  children,
}: {
  character?: CharacterSummary;
  children?: React.ReactNode;
}) {
  return (
    <span className="character-card-body">
      <CharacterPortrait character={character} />
      <strong className="character-card-name">
        {character?.name ?? '空位'}
      </strong>
      {character && (
        <>
          <span className="character-card-meta">
            {elementName(character.element)} · {pathName(character.path)}
          </span>
          {children}
        </>
      )}
    </span>
  );
}
