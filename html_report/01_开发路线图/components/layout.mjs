import { escapeHtml } from './roadmap-text.mjs';

export function layout({ title, content, base = './', overview = false }) {
  return `<!doctype html>
<!-- 由 build.mjs 生成。内容维护请参阅 docs/roadmap.md。 -->
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <title>${escapeHtml(title)} · StarRailSimulator</title>
  <link rel="icon" href="${base}assets/mark.svg" type="image/svg+xml">
  <link rel="stylesheet" href="${base}styles/base.css">
  <link rel="stylesheet" href="${base}styles/roadmap.css">
</head>
<body>
  <a class="skip-link" href="#main">跳到主要内容</a>
  <header class="site-header" id="top">
    <div class="shell header-inner">
      <a class="brand" href="${base}index.html">
        <img src="${base}assets/mark.svg" width="36" height="36" alt="">
        <span>StarRailSimulator</span>
      </a>
      <nav aria-label="汇报导航">
        <a href="${base}index.html"${overview ? ' aria-current="page"' : ''}>路线图</a>
        <a href="${base}index.html#board">里程碑</a>
      </nav>
    </div>
  </header>
  <main class="shell" id="main" tabindex="-1">
${content}
  </main>
  <footer class="shell site-footer">
    <span>StarRailSimulator <span aria-hidden="true">/</span> 开发汇报 01</span>
    <a href="#top">回到顶部 <span aria-hidden="true">↑</span></a>
  </footer>
</body>
</html>
`;
}
