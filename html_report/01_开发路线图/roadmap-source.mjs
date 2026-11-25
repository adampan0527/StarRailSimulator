// Only the documented roadmap format is supported. Unexpected structure should fail
// generation instead of silently hiding milestone data.
const statuses = new Map([
  ['未开始', 'not-started'],
  ['进行中', 'in-progress'],
  ['待验收', 'ready'],
  ['已验收', 'accepted'],
  ['阻塞', 'blocked'],
]);

const boardColumns = '| 里程碑 | 交付物 | 验收依赖 | 状态 | 验收记录 |';
const boardSeparator = /^\|(?:\s*---\s*\|){5}$/;

function check(condition, message) {
  if (!condition) throw new Error(`路线图格式错误：${message}`);
}

function readRows(lines) {
  check(lines[0] === boardColumns && boardSeparator.test(lines[1] ?? ''), '缺少进度看板表头');
  const rows = lines.slice(2).map((line) => {
    const cells = line.trim().split(/(?<!\\)\|/).slice(1, -1)
      .map((cell) => cell.trim().replaceAll('\\|', '|'));
    check(line.startsWith('|') && line.endsWith('|') && cells.length === 5 && cells.every(Boolean),
      `看板行应有五列：${line}`);
    const [reference, title, dependsOn, status, record] = cells;
    const match = reference.match(/^\[(M\d{2,})\]\(#(m\d{2,})\)$/);
    check(match && match[1].toLowerCase() === match[2], `无效的里程碑链接：${reference}`);
    check(statuses.has(status), `${match[1]} 的状态无效：${status}`);
    return {
      id: match[1], slug: match[2], title, status, tone: statuses.get(status), record,
      dependencies: dependsOn === '—' ? [] : dependsOn.split('、').map((value) => value.trim()),
    };
  });
  const ids = new Set(rows.map(({ id }) => id));
  check(rows.length > 0 && ids.size === rows.length, '看板没有里程碑或 ID 重复');
  for (const row of rows) {
    check(row.dependencies.every((id) => id !== row.id && ids.has(id)), `${row.id} 的依赖不存在或指向自身`);
  }
  return rows;
}

export function parseRoadmap(markdown) {
  const lines = markdown.replace(/^\uFEFF/, '').split(/\r?\n/);
  const overviewStart = lines.indexOf('## 进度看板');
  const detailsStart = lines.indexOf('## 里程碑与验收');
  const goalLine = lines.find((line) => line.startsWith('目标：'));
  check(goalLine && overviewStart > 0 && detailsStart > overviewStart, '缺少目标、进度看板或里程碑与验收');

  const [table, ...notes] = lines.slice(overviewStart + 1, detailsStart).join('\n').trim().split(/\n\s*\n/);
  const rows = readRows(table.split('\n'));
  const detailSource = lines.slice(detailsStart + 1).join('\n').trim();
  const sections = [...detailSource.matchAll(/^<a id="(m\d{2,})"><\/a>\s*\n+### (M\d{2,}) · (.+)$/gm)];
  check(sections.length === rows.length, '看板和详情的里程碑数量不一致');
  const bodies = new Map(sections.map((section, index) => [
    section[2],
    {
      slug: section[1],
      title: section[3].trim(),
      body: detailSource.slice(section.index + section[0].length, sections[index + 1]?.index).trim(),
    },
  ]));
  const milestones = rows.map((row) => {
    const detail = bodies.get(row.id);
    check(detail?.slug === row.slug && detail.title === row.title && detail.body,
      `${row.id} 缺少详情，或看板与详情的名称不一致`);
    return { ...row, body: detail.body };
  });

  return {
    goal: goalLine.slice('目标：'.length),
    notes,
    rules: detailSource.slice(0, sections[0].index).trim(),
    milestones,
  };
}
