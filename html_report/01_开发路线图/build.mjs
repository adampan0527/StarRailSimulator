import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { parseRoadmap } from './roadmap-source.mjs';
import { milestonePage } from './components/milestone.mjs';
import { overviewPage } from './components/overview.mjs';

const reportDirectory = 'html_report/01_开发路线图';
const milestoneDirectory = new URL('./milestones/', import.meta.url);

async function readIfPresent(url) {
  try {
    return await readFile(url, 'utf8');
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}

async function staleMilestonePages(pages) {
  const files = await readdir(milestoneDirectory).catch((error) => {
    if (error.code === 'ENOENT') return [];
    throw error;
  });
  return files.map((file) => `milestones/${file}`)
    .filter((file) => /^milestones\/m\d{2,}\.html$/.test(file) && !pages.has(file));
}

async function build(args) {
  if (args.length > 1 || (args.length === 1 && args[0] !== '--check')) {
    throw new Error('用法：node build.mjs [--check]');
  }
  const checkOnly = args[0] === '--check';
  const source = await readFile(new URL('../../docs/roadmap.md', import.meta.url), 'utf8');
  const roadmap = parseRoadmap(source);
  const pages = new Map([['index.html', overviewPage(roadmap, reportDirectory)]]);
  roadmap.milestones.forEach((milestone, index, milestones) => {
    pages.set(`milestones/${milestone.slug}.html`, milestonePage(milestone, {
      previous: milestones[index - 1],
      next: milestones[index + 1],
      outputDirectory: `${reportDirectory}/milestones`,
    }));
  });
  const stale = await staleMilestonePages(pages);
  if (!checkOnly) await mkdir(milestoneDirectory, { recursive: true });
  const changed = [];
  for (const [file, html] of pages) {
    const url = new URL(file, import.meta.url);
    const existing = await readIfPresent(url);
    if (existing?.replaceAll('\r\n', '\n') === html) continue;
    changed.push(file);
    if (!checkOnly) await writeFile(url, html, 'utf8');
  }
  if (checkOnly && (changed.length || stale.length)) {
    throw new Error(`HTML 汇报未同步，请重新运行 build.mjs：${[...changed, ...stale].join('、')}`);
  }
  if (!checkOnly) {
    for (const file of stale) await rm(new URL(file, import.meta.url));
  }
  console.log(checkOnly ? `已核对 ${pages.size} 个 HTML 页面。` : `路线图包含 ${pages.size} 个 HTML 页面，更新 ${changed.length} 个，移除过期页 ${stale.length} 个。`);
}

build(process.argv.slice(2)).catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
