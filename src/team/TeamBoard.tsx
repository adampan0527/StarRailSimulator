import { useId, useState } from 'react';
import {
  CharacterCardBody,
  CharacterStatGrid,
} from '../characters/CharacterCard';
import type { CharacterSummary, TeamSlots } from '../training/client';
import { CharacterPicker } from './CharacterPicker';

export function TeamBoard({
  roster,
  slots,
  onChange,
  disabled = false,
}: {
  roster: CharacterSummary[];
  slots: TeamSlots;
  onChange: (slots: TeamSlots) => void;
  disabled?: boolean;
}) {
  const [picking, setPicking] = useState<number>();
  const boardId = useId();
  const count = slots.filter((id) => id !== null).length;

  return (
    <section className="field-zone" aria-labelledby={boardId}>
      <div className="section-heading">
        <h3 id={boardId}>己方</h3>
        <span className="tag numeric">{count} / 4</span>
      </div>
      <ol className="ally-slots" aria-label="己方站位">
        {slots.map((id, position) => {
          const member = roster.find((item) => item.id === id);
          const statsId = `${boardId}-stats-${position}`;
          return (
            <li key={position}>
              <button
                type="button"
                className="choice ally-slot"
                data-filled={member ? 'true' : undefined}
                title={member?.name}
                aria-label={`站位 ${position + 1}：${member?.name ?? '空位'}`}
                aria-describedby={
                  member ? `${statsId}-traits ${statsId}` : undefined
                }
                aria-haspopup="dialog"
                disabled={disabled}
                onClick={() => setPicking(position)}
              >
                <span className="slot-number numeric">{position + 1}</span>
                <CharacterCardBody
                  character={member}
                  traitsId={`${statsId}-traits`}
                >
                  {member && (
                    <CharacterStatGrid
                      id={statsId}
                      element={member.element}
                      values={member.stats}
                    />
                  )}
                </CharacterCardBody>
              </button>
            </li>
          );
        })}
      </ol>
      {picking !== undefined && (
        <CharacterPicker
          index={picking}
          roster={roster}
          slots={slots}
          onClose={() => setPicking(undefined)}
          onSelect={(id) => {
            const next: TeamSlots = [...slots];
            next[picking] = id;
            onChange(next);
          }}
        />
      )}
    </section>
  );
}
