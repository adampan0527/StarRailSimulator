import type { CharacterSummary } from '../training/client';
import { CombatTraitFilter } from './CombatTrait';

export type CharacterTraits = Pick<CharacterSummary, 'element' | 'path'>;

export function matchesCharacterTraits(
  character: CharacterSummary,
  { element, path }: CharacterTraits,
) {
  return (
    (!element || character.element === element) &&
    (!path || character.path === path)
  );
}

export function CharacterFilters({
  roster,
  value,
  onChange,
}: {
  roster: CharacterSummary[];
  value: CharacterTraits;
  onChange: (value: CharacterTraits) => void;
}) {
  return (
    <div className="character-trait-filters">
      <CombatTraitFilter
        kind="element"
        options={roster.map(({ element }) => element)}
        value={value.element}
        onChange={(element) => onChange({ ...value, element })}
      />
      <CombatTraitFilter
        kind="path"
        options={roster.map(({ path }) => path)}
        value={value.path}
        onChange={(path) => onChange({ ...value, path })}
      />
    </div>
  );
}
