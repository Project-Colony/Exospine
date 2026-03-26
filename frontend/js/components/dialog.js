// Exospine — Confirmation dialog component

const container = document.getElementById('dialog-container');
container.hidden = true;

/**
 * Show a confirmation dialog.
 * @param {object} opts
 * @param {string} opts.title
 * @param {string} opts.message
 * @param {string} [opts.confirmLabel='Confirm']
 * @param {string} [opts.cancelLabel='Cancel']
 * @param {boolean} [opts.danger=false]
 * @returns {Promise<boolean>} true if confirmed
 */
export function showDialog({ title, message, confirmLabel = 'Confirm', cancelLabel = 'Cancel', danger = false }) {
  return new Promise((resolve) => {
    container.hidden = false;

    container.innerHTML = `
      <div class="dialog-box" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title" aria-describedby="dialog-message">
        <div class="dialog-title" id="dialog-title">${escapeHtml(title)}</div>
        <div class="dialog-message" id="dialog-message">${escapeHtml(message)}</div>
        <div class="dialog-actions">
          <button class="btn btn-ghost" data-action="cancel" aria-label="${escapeHtml(cancelLabel)}">${escapeHtml(cancelLabel)}</button>
          <button class="btn ${danger ? 'btn-danger' : 'btn-primary'}" data-action="confirm" aria-label="${escapeHtml(confirmLabel)}">${escapeHtml(confirmLabel)}</button>
        </div>
      </div>
    `;

    let _resolved = false;

    // Escape to cancel
    const onKey = (e) => {
      if (e.key === 'Escape') cleanup(false);
    };

    const onBackdrop = (e) => {
      if (e.target === container) cleanup(false);
    };

    const cleanup = (result) => {
      if (_resolved) return;
      _resolved = true;
      container.hidden = true;
      container.innerHTML = '';
      document.removeEventListener('keydown', onKey);
      container.removeEventListener('click', onBackdrop);
      resolve(result);
    };

    container.querySelector('[data-action="confirm"]').addEventListener('click', () => cleanup(true));
    container.querySelector('[data-action="cancel"]').addEventListener('click', () => cleanup(false));

    // Close on backdrop click
    container.addEventListener('click', onBackdrop);

    document.addEventListener('keydown', onKey);

    // Focus confirm button
    container.querySelector('[data-action="confirm"]').focus();
  });
}

function escapeHtml(str) {
  const d = document.createElement('div');
  d.textContent = str;
  return d.innerHTML;
}
