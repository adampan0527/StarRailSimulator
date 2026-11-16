import { posix } from 'node:path';

const inlineToken = /\[([^\]]+)\]\(([^)]+)\)|\*\*([^*]+)\*\*|`([^`]+)`/g;
const listItem = /^- (?:(\[[ xX]\]) )?(.+)$/;

export function escapeHtml(value) {
  const entities = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  return value.replace(/[&<>"']/g, (character) => entities[character]);
}

function linkFrom(outputDirectory, target) {
  if (/^https?:\/\//i.test(target)) return target;
  if (/^#m\d{2,}$/i.test(target)) {
    return posix.relative(outputDirectory, `html_report/01_开发路线图/milestones/${target.slice(1).toLowerCase()}.html`);
  }
  if (target.startsWith('#')) return target;
  if (/^(?:[\w+.-]+:|[/\\])/.test(target)) throw new Error(`不支持的路线图链接：${target}`);
  const [, pathname, suffix] = target.match(/^([^?#]+)(.*)$/);
  return posix.relative(outputDirectory, posix.join('docs', pathname)) + suffix;
}

export function roadmapText(outputDirectory) {
  function inline(text) {
    let previousEnd = 0;
    let html = '';
    for (const token of text.matchAll(inlineToken)) {
      const [syntax, label, target, strong, code] = token;
      html += escapeHtml(text.slice(previousEnd, token.index));
      if (label) {
        html += `<a href="${escapeHtml(linkFrom(outputDirectory, target))}">${escapeHtml(label)}</a>`;
      } else if (strong) {
        html += `<strong>${escapeHtml(strong)}</strong>`;
      } else {
        html += `<code>${escapeHtml(code)}</code>`;
      }
      previousEnd = token.index + syntax.length;
    }
    return html + escapeHtml(text.slice(previousEnd));
  }

  function blocks(text) {
    const lines = text.split('\n');
    const html = [];
    for (let index = 0; index < lines.length;) {
      const line = lines[index];
      if (!line.trim()) {
        index++;
        continue;
      }
      if (line.startsWith('#### ')) {
        html.push(`<h3>${inline(line.slice(5))}</h3>`);
        index++;
        continue;
      }
      const first = line.match(listItem);
      if (first) {
        const checklist = Boolean(first[1]);
        const items = [];
        let item = first;
        while (item && Boolean(item[1]) === checklist) {
          const content = inline(item[2]);
          if (checklist) {
            const checked = item[1] !== '[ ]';
            items.push(`<li><span class="check-mark${checked ? ' check-mark--done' : ''}" aria-hidden="true">${checked ? '✓' : ''}</span><span><span class="sr-only">${checked ? '已通过' : '未验收'}：</span>${content}</span></li>`);
          } else {
            items.push(`<li>${content}</li>`);
          }
          item = lines[++index]?.match(listItem);
        }
        html.push(checklist
          ? `<h3>验收清单</h3>\n<ul class="checklist">\n${items.join('\n')}\n</ul>`
          : `<ul>\n${items.join('\n')}\n</ul>`);
        continue;
      }
      const paragraph = [line];
      index++;
      while (lines[index]?.trim() && !lines[index].startsWith('#### ') && !listItem.test(lines[index])) {
        paragraph.push(lines[index++]);
      }
      html.push(`<p>${inline(paragraph.join(' '))}</p>`);
    }
    return html.join('\n');
  }

  return { inline, blocks };
}
