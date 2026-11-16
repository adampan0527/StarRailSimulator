// First obtainable dates on the Asia server for the 4.7.0 roster.
const debutBatches = {
  '2026-11-11': { 'character.1511': 'Aeon ★ Aha' },
  '2026-09-28': { 'character.1503': 'Pearl' },
  '2026-09-12': { 'character.1513': 'Aventurine • Waveflair' },
  '2026-08-26': { 'character.1512': 'Robin • Summeretto' },
  '2026-07-24': {
    'character.1509': 'Gilgamesh',
    'character.1508': 'Rin Tohsaka',
  },
  '2026-07-15': { 'character.1510': 'Himeko • Nova' },
  '2026-06-01': { 'character.1507': 'Mortenax Blade' },
  '2026-05-13': { 'character.1505': 'Evanescia' },
  '2026-04-22': {
    'character.1506': 'Silver Wolf LV.999',
    'trailblazer.elation': 'Trailblazer (Elation)',
  },
  '2026-03-25': { 'character.1504': 'Ashveil' },
  '2026-03-03': { 'character.1501': 'Sparxie' },
  '2026-02-13': { 'character.1502': 'Yao Guang' },
  '2025-12-17': { 'character.1321': 'The Dahlia' },
  '2025-11-05': { 'character.1415': 'Cyrene' },
  '2025-10-15': { 'character.1414': 'Dan Heng • Permansor Terrae' },
  '2025-09-24': { 'character.1413': 'Evernight' },
  '2025-09-02': { 'character.1412': 'Cerydra' },
  '2025-08-13': { 'character.1410': 'Hysilens' },
  '2025-07-11': {
    'character.1015': 'Archer',
    'character.1014': 'Saber',
  },
  '2025-07-02': { 'character.1408': 'Phainon' },
  '2025-06-11': { 'character.1406': 'Cipher' },
  '2025-05-21': { 'character.1409': 'Hyacine' },
  '2025-04-30': { 'character.1405': 'Anaxa' },
  '2025-04-09': { 'character.1407': 'Castorice' },
  '2025-03-19': { 'character.1404': 'Mydei' },
  '2025-02-26': { 'character.1403': 'Tribbie' },
  '2025-02-05': { 'character.1402': 'Aglaea' },
  '2025-01-15': {
    'character.1401': 'The Herta',
    'trailblazer.remembrance': 'Trailblazer (Remembrance)',
  },
  '2024-12-25': { 'character.1225': 'Fugue' },
  '2024-12-04': { 'character.1313': 'Sunday' },
  '2024-10-23': { 'character.1317': 'Rappa' },
  '2024-10-02': { 'character.1222': 'Lingsha' },
  '2024-09-10': {
    'character.1220': 'Feixiao',
    'character.1223': 'Moze',
  },
  '2024-08-21': { 'character.1218': 'Jiaoqiu' },
  '2024-07-31': {
    'march_7th.hunt': 'March 7th',
    'character.1221': 'Yunli',
  },
  '2024-07-10': { 'character.1314': 'Jade' },
  '2024-06-19': { 'character.1310': 'Firefly' },
  '2024-05-29': { 'character.1315': 'Boothill' },
  '2024-05-08': {
    'character.1309': 'Robin',
    'trailblazer.harmony': 'Trailblazer (Harmony)',
  },
  '2024-04-17': { 'character.1304': 'Aventurine' },
  '2024-03-27': {
    'character.1308': 'Acheron',
    'character.1301': 'Gallagher',
  },
  '2024-02-29': { 'character.1306': 'Sparkle' },
  '2024-02-06': {
    'character.1307': 'Black Swan',
    'character.1312': 'Misha',
  },
  '2024-01-17': { 'character.1305': 'Dr. Ratio' },
  '2023-12-27': {
    'character.1303': 'Ruan Mei',
    'character.1214': 'Xueyi',
  },
  '2023-12-06': {
    'character.1302': 'Argenti',
    'character.1215': 'Hanya',
  },
  '2023-11-15': { 'character.1217': 'Huohuo' },
  '2023-10-27': {
    'character.1210': 'Guinaifen',
    'character.1112': 'Topaz & Numby',
  },
  '2023-10-11': { 'character.1212': 'Jingliu' },
  '2023-09-20': {
    'character.1208': 'Fu Xuan',
    'character.1110': 'Lynx',
  },
  '2023-08-30': { 'character.1213': 'Dan Heng • Imbibitor Lunae' },
  '2023-08-09': {
    'character.1005': 'Kafka',
    'character.1111': 'Luka',
  },
  '2023-07-19': { 'character.1205': 'Blade' },
  '2023-06-28': {
    'character.1203': 'Luocha',
    'character.1207': 'Yukong',
  },
  '2023-06-07': { 'character.1006': 'Silver Wolf' },
  '2023-05-17': { 'character.1204': 'Jing Yuan' },
  '2023-04-26': {
    'character.1008': 'Arlan',
    'character.1009': 'Asta',
    'character.1211': 'Bailu',
    'character.1101': 'Bronya',
    'character.1107': 'Clara',
    'dan_heng.hunt': 'Dan Heng',
    'character.1104': 'Gepard',
    'character.1013': 'Herta',
    'character.1003': 'Himeko',
    'character.1109': 'Hook',
    'march_7th.preservation': 'March 7th',
    'natasha.abundance': 'Natasha',
    'character.1106': 'Pela',
    'character.1201': 'Qingque',
    'character.1108': 'Sampo',
    'character.1102': 'Seele',
    'character.1103': 'Serval',
    'character.1206': 'Sushang',
    'character.1202': 'Tingyun',
    'trailblazer.destruction': 'Trailblazer (Destruction)',
    'trailblazer.preservation': 'Trailblazer (Preservation)',
    'character.1004': 'Welt',
    'character.1209': 'Yanqing',
  },
};

const debutById = new Map(
  Object.entries(debutBatches).flatMap(([date, characters]) =>
    Object.entries(characters).map(([id, englishName]) => [
      id,
      { date, englishName },
    ]),
  ),
);
const englishAlphabetical = new Intl.Collator('en', { sensitivity: 'base' });

export function compareCharacterRelease(
  first: { id: string },
  second: { id: string },
) {
  const firstDebut = debutById.get(first.id);
  const secondDebut = debutById.get(second.id);
  if (!firstDebut || !secondDebut) {
    // Keep imported entries without a known date at the end in catalog order.
    return Number(!!secondDebut) - Number(!!firstDebut);
  }
  return (
    secondDebut.date.localeCompare(firstDebut.date, 'en') ||
    englishAlphabetical.compare(firstDebut.englishName, secondDebut.englishName)
  );
}
