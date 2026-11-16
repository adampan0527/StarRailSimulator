export type CombatTraitKind = 'element' | 'path';

const names = {
  element: new Map([
    ['physical', '物理'],
    ['fire', '火'],
    ['ice', '冰'],
    ['thunder', '雷'],
    ['wind', '风'],
    ['quantum', '量子'],
    ['imaginary', '虚数'],
  ]),
  path: new Map([
    ['destruction', '毁灭'],
    ['hunt', '巡猎'],
    ['erudition', '智识'],
    ['harmony', '同谐'],
    ['nihility', '虚无'],
    ['preservation', '存护'],
    ['abundance', '丰饶'],
    ['remembrance', '记忆'],
    ['elation', '欢愉'],
  ]),
};

const icons = import.meta.glob<string>('./icons/*/*.png', {
  eager: true,
  import: 'default',
  query: '?url&no-inline',
});

export const combatTraitName = (kind: CombatTraitKind, id: string) =>
  names[kind].get(id) ?? id;

export function CombatTraitIcon({
  kind,
  id,
  decorative = false,
}: {
  kind: CombatTraitKind;
  id: string;
  decorative?: boolean;
}) {
  const label = decorative ? undefined : combatTraitName(kind, id);
  const source = icons[`./icons/${kind}/${id}.png`];
  return source ? (
    <img
      className="combat-trait-icon"
      data-kind={kind}
      src={source}
      alt={label ?? ''}
      title={label}
    />
  ) : (
    <span
      className="combat-trait-icon"
      role={decorative ? undefined : 'img'}
      aria-hidden={decorative || undefined}
      aria-label={label}
      title={label}
    >
      ?
    </span>
  );
}

export function CombatTraitFilter({
  kind,
  options,
  value,
  onChange,
}: {
  kind: CombatTraitKind;
  options: string[];
  value: string;
  onChange: (id: string) => void;
}) {
  const category = kind === 'element' ? '属性' : '命途';
  const available = new Set(options);
  const ordered = [...names[kind].keys()].filter((id) => available.delete(id));
  ordered.push(...[...available].sort());

  return (
    <fieldset className="combat-trait-filter">
      <legend>{category}</legend>
      <div className="combat-trait-options">
        <button
          type="button"
          className="secondary"
          aria-label={`全部${category}`}
          aria-pressed={!value}
          onClick={() => onChange('')}
        >
          全部
        </button>
        {ordered.map((id) => {
          const label = combatTraitName(kind, id);
          return (
            <button
              key={id}
              type="button"
              className="secondary"
              aria-label={label}
              title={label}
              aria-pressed={value === id}
              onClick={() => onChange(value === id ? '' : id)}
            >
              <CombatTraitIcon kind={kind} id={id} decorative />
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}
