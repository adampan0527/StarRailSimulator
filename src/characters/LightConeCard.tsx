import { useState } from 'react';
import type { ReactNode } from 'react';
import { CombatTraitIcon, combatTraitName } from './CombatTrait';
import type { LightConeSummary } from './client';

const artwork = import.meta.glob<string>('/.m03-data/4.7.0/light-cones/*.png', {
  eager: true,
  import: 'default',
  query: '?url&no-inline',
});

function LightConeArtwork({ dataId, name }: { dataId: string; name: string }) {
  const [failed, setFailed] = useState(false);
  const source = artwork[`/.m03-data/4.7.0/light-cones/${dataId}.png`];
  return (
    <span className="light-cone-artwork" aria-hidden="true">
      {source && !failed ? (
        <img
          src={source}
          alt=""
          width={128}
          height={128}
          loading="lazy"
          onError={() => setFailed(true)}
        />
      ) : (
        <span className="portrait-fallback">{Array.from(name)[0]}</span>
      )}
    </span>
  );
}

export function LightConeCard({
  data,
  dataId,
  children,
}: {
  data?: LightConeSummary;
  dataId: string;
  children?: ReactNode;
}) {
  const name = data?.name ?? dataId;
  return (
    <span className="light-cone-card" data-rarity={data?.rarity}>
      <LightConeArtwork key={dataId} dataId={dataId} name={name} />
      <span className="light-cone-identity">
        <strong>{name}</strong>
        {data && (
          <>
            <span
              className="light-cone-stars"
              role="img"
              aria-label={`${data.rarity} 星`}
            >
              <span aria-hidden="true">{'★'.repeat(data.rarity)}</span>
            </span>
            <span className="combat-trait-inline light-cone-path">
              <CombatTraitIcon kind="path" id={data.path} decorative />
              {combatTraitName('path', data.path)}
            </span>
          </>
        )}
        {children && <span className="light-cone-details">{children}</span>}
      </span>
    </span>
  );
}
