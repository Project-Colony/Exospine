// Exospine: saving the draft being composed every 30 seconds, with a version history.

import * as api from '../../api.js';
import { t } from '../../i18n/index.js';
import { overlay } from './editor.js';

let _autoSaveInterval = null;
let _currentDraftId = null;
let _lastContentHash = null;

/** The id under which this compose session's versions are saved, if any. */
export function currentDraftId() {
  return _currentDraftId;
}

/** djb2 hash for dirty-checking auto-save content. */
function _hashContent(str) {
  let h = 5381;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) + h + str.charCodeAt(i)) | 0; // h * 33 + c
  }
  return h;
}

export function startAutoSave(getDraft) {
  stopAutoSave();
  // Generate a unique draft ID for version tracking
  if (!_currentDraftId) {
    _currentDraftId = crypto.randomUUID();
  }
  // Snapshot initial content hash
  const draft0 = getDraft();
  _lastContentHash = _hashContent(draft0.subject + '\0' + draft0.body);

  _autoSaveInterval = setInterval(async () => {
    const draft = getDraft();
    if (draft.body || draft.subject) {
      // Only save if content actually changed since last save
      const currentHash = _hashContent(draft.subject + '\0' + draft.body);
      if (currentHash === _lastContentHash) return;
      _lastContentHash = currentHash;

      try {
        await api.saveDraft(draft);
        // Also save a local version for history
        const bodyHtml = overlay.querySelector('#compose-body')?.innerHTML || '';
        await api.saveDraftVersion(_currentDraftId, draft.subject, draft.body, bodyHtml);
        const indicator = overlay.querySelector('#compose-autosave-indicator');
        if (indicator) {
          indicator.textContent = t('draft_saved');
          indicator.hidden = false;
          setTimeout(() => { if (indicator) indicator.hidden = true; }, 2000);
        }
      } catch (e) {
        console.warn('Auto-save failed:', e);
        const indicator = overlay.querySelector('#compose-autosave-indicator');
        if (indicator) {
          indicator.textContent = t('save_failed');
          indicator.hidden = false;
        }
      }
    }
  }, 30000);
}

export function stopAutoSave() {
  if (_autoSaveInterval) {
    clearInterval(_autoSaveInterval);
    _autoSaveInterval = null;
  }
  _currentDraftId = null;
}
