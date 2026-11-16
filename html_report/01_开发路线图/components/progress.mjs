export function progressPanel(milestones) {
  const total = milestones.length;
  const accepted = milestones.filter(({ tone }) => tone === 'accepted').length;
  const active = milestones.filter(({ tone }) => tone === 'in-progress' || tone === 'ready').length;
  const blocked = milestones.filter(({ tone }) => tone === 'blocked').length;

  return `<section class="progress-panel" aria-labelledby="progress-title">
    <div class="progress-panel-inner">
      <h2 class="eyebrow" id="progress-title">首发验收进度</h2>
      <p class="progress-total"><strong>${String(accepted).padStart(2, '0')}</strong> <span>/ ${String(total).padStart(2, '0')}</span></p>
      <progress value="${accepted}" max="${total}" aria-label="已验收的里程碑">${accepted} / ${total}</progress>
      <dl class="progress-stats">
        <div><dt>在推进</dt><dd>${active}</dd></div>
        <div><dt>未开始</dt><dd>${total - accepted - active - blocked}</dd></div>
        <div><dt>阻塞</dt><dd>${blocked}</dd></div>
      </dl>
    </div>
  </section>`;
}
