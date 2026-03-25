// Exospine — Toast notification component

const container = document.getElementById('toast-container');

/**
 * Show a toast notification.
 * @param {string} message
 * @param {'success'|'error'|'info'} type
 * @param {number} duration  ms before auto-dismiss (default 5000)
 * @param {function} [onClick] optional click callback (e.g. for Undo actions)
 * @returns {HTMLElement} the toast element (can be used to dismiss programmatically)
 */
export function showToast(message, type = 'info', duration = 5000, onClick = null) {
  const el = document.createElement('div');
  el.className = `toast ${type}`;
  el.setAttribute('role', 'alert');
  el.setAttribute('aria-live', 'assertive');
  if (onClick) el.classList.add('toast-clickable');

  const icon = type === 'success' ? '\u2713' : type === 'error' ? '\u2717' : '\u24D8';

  el.innerHTML = `
    <span aria-hidden="true">${icon}</span>
    <span>${escapeHtml(message)}</span>
    <button class="toast-dismiss" title="Dismiss" aria-label="Dismiss notification">\u00D7</button>
  `;

  let autoDismissTimer = null;

  const dismiss = () => {
    if (autoDismissTimer) clearTimeout(autoDismissTimer);
    el.classList.add('toast-dismissing');
    el.addEventListener('animationend', () => el.remove());
  };

  el.querySelector('.toast-dismiss').addEventListener('click', (e) => {
    e.stopPropagation();
    dismiss();
  });

  // Click callback (for undo, etc.)
  if (onClick) {
    el.addEventListener('click', (e) => {
      if (e.target.closest('.toast-dismiss')) return;
      dismiss();
      onClick();
    });
  }

  container.appendChild(el);

  if (duration > 0) {
    autoDismissTimer = setTimeout(dismiss, duration);
  }

  // Expose dismiss so callers can cancel programmatically
  el._dismiss = dismiss;
  return el;
}

function escapeHtml(str) {
  const d = document.createElement('div');
  d.textContent = str;
  return d.innerHTML;
}
