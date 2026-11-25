import { layout } from './layout.mjs';
import { escapeHtml, roadmapText } from './roadmap-text.mjs';

export function statusBadge({ tone, status }) {
  return `<span class="status status--${tone}">${escapeHtml(status)}</span>`;
}

export function dependencyLinks(dependencies, pathPrefix) {
  if (dependencies.length === 0) return '无';
  return dependencies.map((id) => `<a href="${pathPrefix}${id.toLowerCase()}.html">${id}</a>`).join('、');
}

function pageLink(milestone, direction) {
  if (!milestone) return '';
  const label = direction === 'previous' ? '上一里程碑' : '下一里程碑';
  return `<a class="page-link page-link--${direction}" href="./${milestone.slug}.html">
    <span>${label} · ${milestone.id}</span>
    <strong>${escapeHtml(milestone.title)}</strong>
  </a>`;
}

export function milestonePage(milestone, { previous, next, outputDirectory }) {
  const { inline, blocks } = roadmapText(outputDirectory);
  const { id, title, dependencies, body, record } = milestone;
  const adjacent = [pageLink(previous, 'previous'), pageLink(next, 'next')]
    .filter(Boolean)
    .map((link) => `      ${link}`)
    .join('\n');

  return layout({
    title: `${id} · ${title}`,
    base: '../',
    content: `    <nav class="breadcrumbs" aria-label="当前位置">
      <a href="../index.html#board"><span aria-hidden="true">←</span> 返回里程碑看板</a>
    </nav>
    <article class="detail glass">
      <header class="detail-header">
        <p class="eyebrow"><span class="mono">${id}</span> <span aria-hidden="true">/</span> 里程碑</p>
        <h1>${escapeHtml(title)}</h1>
        <dl class="detail-meta">
          <div><dt>当前状态</dt><dd>${statusBadge(milestone)}</dd></div>
          <div><dt>验收依赖</dt><dd>${dependencyLinks(dependencies, './')}</dd></div>
          <div><dt>验收记录</dt><dd>${record === '—' ? '暂无' : inline(record)}</dd></div>
        </dl>
      </header>
      <section class="prose detail-body" aria-labelledby="criteria">
        <h2 id="criteria">交付范围与验收</h2>
        ${blocks(body)}
      </section>
    </article>
    <nav class="detail-pagination" aria-label="相邻里程碑">
${adjacent}
    </nav>`,
  });
}
