// Exospine: offline mode with an action queue.

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { t, tn } from './i18n/index.js';

// ---------------------------------------------------------------------------
// localStorage batch debounce — coalesce writes with 1s interval
// ---------------------------------------------------------------------------
const _lsBatchQueue = new Map(); // key -> value
let _lsBatchTimer = null;

export function lsSetItem(key, value) {
  _lsBatchQueue.set(key, value);
  if (!_lsBatchTimer) {
    _lsBatchTimer = setTimeout(() => {
      for (const [k, v] of _lsBatchQueue) {
        localStorage.setItem(k, v);
      }
      _lsBatchQueue.clear();
      _lsBatchTimer = null;
    }, 1000);
  }
}

// ---------------------------------------------------------------------------
// Offline mode with action queue
// ---------------------------------------------------------------------------
export let _isOffline = !navigator.onLine;

export const _actionQueue = JSON.parse(localStorage.getItem('exospine_action_queue') || '[]');

export function queueAction(action) {
  _actionQueue.push({ ...action, timestamp: Date.now() });
  lsSetItem('exospine_action_queue', JSON.stringify(_actionQueue));
  showToast(t('action_queued'), 'info');
}

export async function executeAction(action) {
  switch (action.type) {
    case 'send_mail':
      await api.sendMail(action.draft);
      break;
    case 'delete_mail':
      await api.deleteMail(action.accountId, action.folder, action.mailUid);
      break;
    case 'archive_mail':
      await api.archiveMail(action.accountId, action.folder, action.mailUid);
      break;
    case 'move_mail':
      await api.moveMail(action.accountId, action.folder, action.mailUid, action.targetFolder);
      break;
    case 'toggle_star':
      await api.toggleStar(action.accountId, action.folder, action.mailUid, action.isStarred);
      break;
    case 'mark_read':
      await api.markRead(action.accountId, action.folder, action.mailUid);
      break;
    case 'mark_unread':
      await api.markUnread(action.accountId, action.folder, action.mailUid);
      break;
    case 'sweep_sender':
      await api.sweepSender(action.accountId, action.folder, action.senderEmail);
      break;
    default:
      console.warn('Unknown queued action type:', action.type);
  }
}

export async function replayQueue() {
  if (_actionQueue.length === 0) return;
  showToast(tn('replaying_queue', _actionQueue.length), 'info');

  const MAX_RETRIES = 3;

  // Process in batches of 5 with Promise.allSettled
  while (_actionQueue.length > 0) {
    const batch = _actionQueue.splice(0, 5);
    const results = await Promise.allSettled(batch.map((a) => executeAction(a)));

    // Re-queue any that failed (in order), with retry count
    const failed = [];
    const discarded = [];
    for (let i = 0; i < results.length; i++) {
      if (results[i].status === 'rejected') {
        const action = batch[i];
        action._retryCount = (action._retryCount || 0) + 1;
        if (action._retryCount >= MAX_RETRIES) {
          discarded.push(action);
        } else {
          failed.push(action);
        }
      }
    }
    if (discarded.length > 0) {
      showToast(tn('queue_discarded', discarded.length, { retries: MAX_RETRIES }), 'error');
    }
    if (failed.length > 0) {
      _actionQueue.push(...failed);
      break; // stop on first batch with failures, will retry next time
    }
  }

  lsSetItem('exospine_action_queue', JSON.stringify(_actionQueue));
  if (_actionQueue.length === 0) {
    showToast(t('queue_synced'), 'success');
  }
}

export function setupOfflineMode() {
  function updateOfflineBanner(offline) {
    _isOffline = offline;
    let banner = document.getElementById('offline-banner');
    if (offline) {
      if (!banner) {
        banner = document.createElement('div');
        banner.id = 'offline-banner';
        banner.className = 'offline-banner';
        banner.textContent = t('offline_banner');
        document.body.prepend(banner);
      }
    } else {
      if (banner) banner.remove();
    }
  }

  updateOfflineBanner(!navigator.onLine);
  window.addEventListener('online', () => {
    updateOfflineBanner(false);
    replayQueue();
  });
  window.addEventListener('offline', () => updateOfflineBanner(true));
}
