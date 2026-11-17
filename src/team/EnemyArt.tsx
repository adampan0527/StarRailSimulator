import { useState } from 'react';

const avatars = import.meta.glob<string>(
  '/.m03-data/4.7.0/enemy-avatars/*.png',
  {
    eager: true,
    import: 'default',
    query: '?url&no-inline',
  },
);
const artworks = import.meta.glob<string>(
  '/.m03-data/4.7.0/enemy-art/Monster_*.webp',
  {
    eager: true,
    import: 'default',
    query: '?url&no-inline',
  },
);

export function EnemyAvatar({ file }: { file: string | null }) {
  const source = file && avatars[`/.m03-data/4.7.0/enemy-avatars/${file}.png`];
  const [failed, setFailed] = useState('');
  return (
    <span className="enemy-avatar" aria-hidden="true">
      {source && source !== failed ? (
        <img
          src={source}
          alt=""
          loading="lazy"
          onError={() => setFailed(source)}
        />
      ) : (
        <span>—</span>
      )}
    </span>
  );
}

export function EnemyArtwork({
  file,
  name,
}: {
  file: string | null;
  name: string;
}) {
  const source = file && artworks[`/.m03-data/4.7.0/enemy-art/${file}.webp`];
  const [failed, setFailed] = useState('');
  if (!source) return null;
  return (
    <details className="enemy-artwork">
      <summary>立绘</summary>
      {source === failed ? (
        <p className="muted">无法显示立绘。</p>
      ) : (
        <img
          src={source}
          alt={`${name}立绘`}
          loading="lazy"
          onError={() => setFailed(source)}
        />
      )}
    </details>
  );
}
