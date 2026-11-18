const icons = import.meta.glob<string>('/.m03-data/4.7.0/relic-icons/*.png', {
  eager: true,
  import: 'default',
  query: '?url&no-inline',
});

export function relicIcon(id: string): string | undefined {
  return icons[`/.m03-data/4.7.0/relic-icons/${id}.png`];
}
