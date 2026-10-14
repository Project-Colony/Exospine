// Exospine: the reading pane.

import { t } from '../../i18n/index.js';
import { cancelZoomSave } from './state.js';
import { mailViewHtml } from './render.js';
import { wireMailView } from './actions.js';

export { isThreadMuted } from './state.js';
export { mailDocument, openLinksExternally, showRemoteImages } from './render.js';

/**
 * Render the reading pane.
 * @param {HTMLElement} el  #mail-view element
 * @param {object} state    app state (selectedMail, mailBody)
 * @param {object} actions  { onReply, onReplyAll, onForward, onArchive, onDelete, onMarkUnread }
 */
export function renderMailView(el, state, actions) {
  // Clear any pending zoom save timer on re-render
  cancelZoomSave();

  const mail = state.selectedMail;

  if (!mail) {
    el.innerHTML = `<div class="mail-view-empty" role="status" aria-live="polite">
      <div class="empty-state-illustration">
        <svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg">
          <rect x="10" y="20" width="60" height="44" rx="6" stroke="currentColor" stroke-width="2" fill="none"/>
          <path d="M10 26 L40 46 L70 26" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </div>
      <div class="empty-state-text">${t('select_email')}</div>
    </div>`;
    return;
  }

  const body = state.mailBody;
  if (body === undefined) {
    el.innerHTML = `<div class="mail-view-body-text mail-view-status-text" role="status" aria-live="polite">${t('loading_body')}</div>`;
    return;
  }
  el.innerHTML = mailViewHtml(mail, body, state);
  wireMailView(el, mail, body, state, actions);
}
