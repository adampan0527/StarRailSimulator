import { useEffect, useId, useRef, useState } from 'react';
import { CharacterCardBody } from '../characters/CharacterCard';
import {
  CharacterFilters,
  matchesCharacterTraits,
} from '../characters/CharacterFilters';
import { combatTraitName } from '../characters/CombatTrait';
import { compareCharacterRelease } from '../characters/releaseOrder';
import type { CharacterSummary, TeamSlots } from '../training/client';

export function CharacterPicker({
  index,
  roster,
  slots,
  onSelect,
  onClose,
}: {
  index: number;
  roster: CharacterSummary[];
  slots: TeamSlots;
  onSelect: (id: string | null) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const title = useId();
  const [traits, setTraits] = useState({ element: '', path: '' });
  const visible = roster
    .filter((item) => matchesCharacterTraits(item, traits))
    .sort(compareCharacterRelease);

  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  function choose(id: string | null) {
    onSelect(id);
    dialog.current?.close();
  }

  return (
    <dialog
      className="picker-dialog"
      ref={dialog}
      aria-labelledby={title}
      onClose={onClose}
    >
      <div className="section-heading">
        <h2 id={title}>选择站位 {index + 1} 的角色</h2>
        <button
          type="button"
          className="secondary"
          onClick={() => dialog.current?.close()}
        >
          关闭
        </button>
      </div>
      <CharacterFilters roster={roster} value={traits} onChange={setTraits} />
      <button type="button" className="secondary" onClick={() => choose(null)}>
        设为空位
      </button>
      <p className="muted" role="status">
        {visible.length
          ? `共 ${visible.length} 个角色`
          : '没有符合筛选的角色。'}
      </p>
      <ul className="picker-results">
        {visible.map((item) => {
          const occupied = slots.includes(item.id) && slots[index] !== item.id;
          return (
            <li key={item.id}>
              <button
                className="choice picker-card"
                type="button"
                disabled={occupied}
                aria-label={`${item.name} · ${combatTraitName('element', item.element)} · ${combatTraitName('path', item.path)}${occupied ? ' · 已在队伍中' : ''}`}
                aria-pressed={slots[index] === item.id}
                onClick={() => choose(item.id)}
              >
                <CharacterCardBody character={item} />
                {occupied && (
                  <span className="muted picker-occupied">已在队伍中</span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
    </dialog>
  );
}
