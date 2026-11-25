import { layout } from './layout.mjs';
import { dependencyLinks, statusBadge } from './milestone.mjs';
import { progressPanel } from './progress.mjs';
import { escapeHtml, roadmapText } from './roadmap-text.mjs';

function boardRow(milestone, inline) {
  const { id, slug, title, dependencies, record } = milestone;
  const path = './milestones/';
  return `<li class="board-row">
    <a class="milestone-link" href="${path}${slug}.html">
      <span class="milestone-id mono">${id}</span>
      <span>${escapeHtml(title)}</span>
      <span class="row-arrow" aria-hidden="true">↗</span>
    </a>
    <span class="board-cell"><span class="board-label">验收依赖</span>${dependencyLinks(dependencies, path)}</span>
    <span class="board-cell"><span class="board-label">状态</span>${statusBadge(milestone)}</span>
    <span class="board-cell board-record"><span class="board-label">验收记录</span>${record === '—' ? '暂无' : inline(record)}</span>
  </li>`;
}

export function overviewPage({ goal, notes, rules, milestones }, outputDirectory) {
  const { inline, blocks } = roadmapText(outputDirectory);
  const [nextStep, ...statusNotes] = notes;
  return layout({
    title: '开发路线图',
    overview: true,
    content: `    <section class="hero" aria-labelledby="report-title">
      <div class="hero-copy">
        <p class="eyebrow"><span class="mono">01</span> <span aria-hidden="true">/</span> 项目汇报</p>
        <h1 id="report-title">开发路线图</h1>
        <p class="hero-lead">${inline(goal)}</p>
        <a class="button-link" href="#board">查看里程碑 <span aria-hidden="true">↓</span></a>
      </div>
      ${progressPanel(milestones)}
    </section>

    <section class="board-section" id="board" aria-labelledby="board-title">
      <header class="section-header">
        <div>
          <p class="eyebrow">交付计划</p>
          <h2 id="board-title">里程碑看板</h2>
        </div>
        <p class="count-label">共 ${milestones.length} 个里程碑</p>
      </header>
      ${nextStep ? `<div class="sequence-note"><strong>开发顺序</strong>${blocks(nextStep)}</div>` : ''}
      <div class="board glass">
        <div class="board-head" aria-hidden="true">
          <span>里程碑 / 交付物</span><span>验收依赖</span><span>状态</span><span>验收记录</span>
        </div>
        <ul class="board-list" role="list" aria-label="全部里程碑">
          ${milestones.map((milestone) => boardRow(milestone, inline)).join('\n')}
        </ul>
      </div>
    </section>

    <section class="guide-section prose" aria-labelledby="guide-title">
      <h2 id="guide-title">状态与验收规则</h2>
      ${statusNotes.map(blocks).join('\n')}
      ${blocks(rules)}
      <p class="source-link"><a href="../../docs/roadmap.md">查看路线图源文档</a></p>
    </section>`,
  });
}
