import preview from './enemy-preview.json';

export const enemies = new Map(
  preview.enemies.map((enemy) => [enemy.id, enemy]),
);
