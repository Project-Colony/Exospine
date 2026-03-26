// Exospine — Main application entry point

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { renderSidebar, updateUnreadBadges } from './views/sidebar.js';
import { renderMailList } from './views/mail_list.js';
import { renderMailView, isThreadMuted } from './views/mail_view.js';
import { openCompose, makeReplyPrefill, makeForwardPrefill } from './views/compose.js';
import { openSettings, getMergedShortcuts, hashPin, restoreImportedTheme } from './views/settings.js';
import { openOnboarding } from './views/onboarding.js';
import { openAnalytics } from './views/analytics.js';
import { openCalendar } from './views/calendar.js';
import { openContacts } from './views/contacts.js';
import { openTasks, openAwaitingReply } from './views/tasks.js';
import { setLanguage } from './i18n.js';

// ---------------------------------------------------------------------------
// Interval tracking — all intervals are stored for cleanup on beforeunload
// ---------------------------------------------------------------------------
const _intervals = [];

// ---------------------------------------------------------------------------
// localStorage batch debounce — coalesce writes with 1s interval
// ---------------------------------------------------------------------------
const _lsBatchQueue = new Map(); // key -> value
let _lsBatchTimer = null;

function lsSetItem(key, value) {
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
let _isOffline = !navigator.onLine;
const _actionQueue = JSON.parse(localStorage.getItem('exospine_action_queue') || '[]');

function queueAction(action) {
  _actionQueue.push({ ...action, timestamp: Date.now() });
  lsSetItem('exospine_action_queue', JSON.stringify(_actionQueue));
  showToast('Action queued. Will sync when connected.', 'info');
}

async function executeAction(action) {
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

async function replayQueue() {
  if (_actionQueue.length === 0) return;
  showToast(`Replaying ${_actionQueue.length} queued action(s)...`, 'info');

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
      showToast(`${discarded.length} action(s) failed after ${MAX_RETRIES} retries and were discarded.`, 'error');
    }
    if (failed.length > 0) {
      _actionQueue.unshift(...failed);
      break; // stop on first batch with failures, will retry next time
    }
  }

  lsSetItem('exospine_action_queue', JSON.stringify(_actionQueue));
  if (_actionQueue.length === 0) {
    showToast('All queued actions synced.', 'success');
  }
}

function setupOfflineMode() {
  function updateOfflineBanner(offline) {
    _isOffline = offline;
    let banner = document.getElementById('offline-banner');
    if (offline) {
      if (!banner) {
        banner = document.createElement('div');
        banner.id = 'offline-banner';
        banner.className = 'offline-banner';
        banner.textContent = 'Offline \u2014 actions will sync when connected';
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

// ---------------------------------------------------------------------------
// Body cache — avoids redundant API calls for already-loaded mail bodies
// ---------------------------------------------------------------------------
const bodyCache = new Map(); // mailId -> { text, html }

// ---------------------------------------------------------------------------
// Prefetch — silently preload bodies for the first visible mails
// Rate-limited: track in-flight requests in a Set to avoid duplicates.
// ---------------------------------------------------------------------------
const _prefetchInFlight = new Set();

function prefetchBodies(mails) {
  const toPrefetch = mails.slice(0, 5);
  for (const mail of toPrefetch) {
    if (bodyCache.has(mail.id) || _prefetchInFlight.has(mail.id)) continue;
    _prefetchInFlight.add(mail.id);
    api.getMailBody(mail.id).then(body => {
      if (body) bodyCache.set(mail.id, body);
    }).catch(() => {}).finally(() => {
      _prefetchInFlight.delete(mail.id);
    });
  }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const state = {
  accounts: [],
  activeAccount: 0, // index into accounts[]
  activeFolder: 'INBOX',
  mails: [],
  selectedMail: null,
  mailBody: null,
  folders: [],
  page: 0,
  hasMore: true,
  loading: false,
  settings: {},
  // UI state for error/sync
  _mailsError: false,
  _foldersError: false,
  _syncing: false,
  _syncProgress: 0,
  // Unified inbox / search folder state
  _unifiedInbox: false,
  _activeSearchFolder: null,
};

// Helper: get the active account's UUID
function activeAccountId() {
  const acc = state.accounts[state.activeAccount];
  return acc ? acc.id : '';
}

// DOM references (with null guards)
const sidebarEl = document.getElementById('sidebar');
const mailListEl = document.getElementById('mail-list');
const mailViewEl = document.getElementById('mail-view');
const hamburgerBtn = document.getElementById('hamburger-btn');

if (!sidebarEl || !mailListEl || !mailViewEl) {
  console.error('Missing required DOM elements (#sidebar, #mail-list, or #mail-view)');
}

// ---------------------------------------------------------------------------
// Notification sound
// ---------------------------------------------------------------------------
let _soundEnabled = localStorage.getItem('exospine_sound_notifications') !== 'false';

function playNotificationSound(accountId) {
  if (!_soundEnabled) return;
  // Per-account override, then global setting, then default
  let sound;
  if (accountId) {
    const acctSound = localStorage.getItem('exospine_notif_sound_' + accountId);
    sound = acctSound || localStorage.getItem('exospine_notif_sound') || 'default';
  } else {
    sound = localStorage.getItem('exospine_notif_sound') || 'default';
  }
  if (sound === 'silent') return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (sound === 'chime') {
      // 660Hz then 880Hz sequence
      const osc1 = ctx.createOscillator();
      const gain1 = ctx.createGain();
      osc1.connect(gain1);
      gain1.connect(ctx.destination);
      osc1.frequency.value = 660;
      osc1.type = 'sine';
      gain1.gain.value = 0.3;
      osc1.start();
      gain1.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.2);
      osc1.stop(ctx.currentTime + 0.2);

      const osc2 = ctx.createOscillator();
      const gain2 = ctx.createGain();
      osc2.connect(gain2);
      gain2.connect(ctx.destination);
      osc2.frequency.value = 880;
      osc2.type = 'sine';
      gain2.gain.value = 0.3;
      osc2.start(ctx.currentTime + 0.2);
      gain2.gain.setValueAtTime(0.3, ctx.currentTime + 0.2);
      gain2.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc2.stop(ctx.currentTime + 0.5);
    } else if (sound === 'bell') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 440;
      osc.type = 'triangle';
      gain.gain.value = 0.4;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc.stop(ctx.currentTime + 0.5);
    } else if (sound === 'gentle') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 523;
      osc.type = 'sine';
      gain.gain.value = 0.15;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.4);
      osc.stop(ctx.currentTime + 0.4);
    } else {
      // Default: 880Hz sine
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 880;
      osc.type = 'sine';
      gain.gain.value = 0.3;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
      osc.stop(ctx.currentTime + 0.3);
    }
  } catch {}
}

function showDesktopNotification(title, body) {
  try {
    if (Notification.permission === 'granted') {
      new Notification(title, { body, icon: '' });
    } else if (Notification.permission !== 'denied') {
      Notification.requestPermission().then(p => {
        if (p === 'granted') new Notification(title, { body, icon: '' });
      });
    }
  } catch {}
}

// ---------------------------------------------------------------------------
// Focus mode
// ---------------------------------------------------------------------------
let _focusMode = false;

function toggleFocusMode() {
  _focusMode = !_focusMode;
  const app = document.getElementById('app');
  if (!app) return;
  if (_focusMode) {
    app.classList.add('focus-mode');
    // Add exit button
    let exitBtn = document.getElementById('exit-focus-btn');
    if (!exitBtn) {
      exitBtn = document.createElement('button');
      exitBtn.id = 'exit-focus-btn';
      exitBtn.className = 'exit-focus-btn';
      exitBtn.textContent = 'Exit focus';
      exitBtn.addEventListener('click', toggleFocusMode);
      app.prepend(exitBtn);
    }
  } else {
    app.classList.remove('focus-mode');
    const exitBtn = document.getElementById('exit-focus-btn');
    if (exitBtn) exitBtn.remove();
  }
}

// ---------------------------------------------------------------------------
// Auto dark mode based on system time
// ---------------------------------------------------------------------------
let _autoThemeInterval = null;

function applyAutoTheme() {
  const hour = new Date().getHours();
  const isDark = hour < 7 || hour >= 20; // Dark between 8pm-7am
  document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  if (isDark) {
    document.documentElement.removeAttribute('data-theme');
  } else {
    document.documentElement.setAttribute('data-theme', 'light');
  }
}

function startAutoTheme() {
  stopAutoTheme();
  applyAutoTheme();
  _autoThemeInterval = setInterval(applyAutoTheme, 5 * 60 * 1000);
  _intervals.push(_autoThemeInterval);
}

function stopAutoTheme() {
  if (_autoThemeInterval) {
    clearInterval(_autoThemeInterval);
    _autoThemeInterval = null;
  }
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------
function applyTheme(theme) {
  stopAutoTheme();
  clearCustomTheme();
  if (theme === 'auto') {
    startAutoTheme();
  } else if (theme === 'light') {
    document.documentElement.setAttribute('data-theme', 'light');
  } else if (theme === 'high-contrast') {
    document.documentElement.setAttribute('data-theme', 'high-contrast');
  } else if (theme === 'custom') {
    document.documentElement.removeAttribute('data-theme');
    // Read and apply custom theme colors from localStorage
    applyCustomTheme();
  } else {
    document.documentElement.removeAttribute('data-theme');
  }
}

/**
 * Apply custom theme colors from localStorage onto :root CSS variables.
 */
function applyCustomTheme() {
  try {
    const raw = localStorage.getItem('exospine_custom_theme');
    if (!raw) return;
    const colors = JSON.parse(raw);
    const root = document.documentElement;
    if (colors.sidebarBg) root.style.setProperty('--sidebar-bg', colors.sidebarBg);
    if (colors.listBg) root.style.setProperty('--list-bg', colors.listBg);
    if (colors.paneBg) root.style.setProperty('--pane-bg', colors.paneBg);
    if (colors.accent) {
      root.style.setProperty('--accent', colors.accent);
      root.style.setProperty('--list-active', colors.accent);
    }
    if (colors.textPrimary) {
      root.style.setProperty('--pane-text', colors.textPrimary);
      root.style.setProperty('--list-text', colors.textPrimary);
    }
    if (colors.textSecondary) {
      root.style.setProperty('--pane-text-dim', colors.textSecondary);
      root.style.setProperty('--list-text-dim', colors.textSecondary);
      root.style.setProperty('--sidebar-text-dim', colors.textSecondary);
    }
  } catch {
    // Ignore parse errors
  }
}

/**
 * Clear custom theme CSS variable overrides from :root inline styles.
 */
function clearCustomTheme() {
  const root = document.documentElement;
  const props = ['--sidebar-bg', '--list-bg', '--pane-bg', '--accent', '--list-active',
    '--pane-text', '--list-text', '--pane-text-dim', '--list-text-dim', '--sidebar-text-dim'];
  for (const p of props) {
    root.style.removeProperty(p);
  }
}

// ---------------------------------------------------------------------------
// RTL language detection
// ---------------------------------------------------------------------------
const RTL_LANGUAGES = ['ar', 'he', 'fa', 'ur', 'ps', 'sd', 'yi'];

function applyDirection(language) {
  const lang = (language || '').split('-')[0].toLowerCase();
  if (RTL_LANGUAGES.includes(lang)) {
    document.documentElement.dir = 'rtl';
    document.documentElement.lang = lang;
  } else {
    document.documentElement.dir = 'ltr';
    if (lang) document.documentElement.lang = lang;
  }
}

// ---------------------------------------------------------------------------
// Hamburger menu (responsive)
// ---------------------------------------------------------------------------
if (hamburgerBtn) {
  hamburgerBtn.addEventListener('click', () => {
    sidebarEl.classList.toggle('sidebar-open');
  });
  // Intentional: registered once at startup, needed for the entire app lifetime.
  document.addEventListener('click', (e) => {
    if (sidebarEl.classList.contains('sidebar-open') &&
        !sidebarEl.contains(e.target) &&
        e.target !== hamburgerBtn) {
      sidebarEl.classList.remove('sidebar-open');
    }
  });
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------
function sidebarActions() {
  return {
    onFolderSelect: selectFolder,
    onSwitchAccount: switchAccount,
    onAddAccount: () => openOnboarding({ isFirstRun: false, onAccountAdded: reloadAccounts }),
    onOpenAwaitingReply: () => openAwaitingReply({ onSelectMail: (mailId) => selectMail(mailId) }),
    onOpenTasks: () => openTasks({ onSelectMail: (mailId) => selectMail(mailId) }),
    onOpenAnalytics: () => openAnalytics(activeAccountId()),
    onOpenCalendar: () => openCalendar(),
    onOpenContacts: () => openContacts(),
    onOpenSettings: () => openSettings(state, {
      onSaved: (s) => {
        state.settings = s;
        applyTheme(s.theme);
        if (s.language) {
          setLanguage(s.language);
          applyDirection(s.language);
        }
        // Update sound notification setting
        if (s.sound_notifications !== undefined) {
          _soundEnabled = s.sound_notifications;
          localStorage.setItem('exospine_sound_notifications', String(s.sound_notifications));
        }
        renderAll();
      },
      onAccountRemoved: () => reloadAccounts(),
    }),
    onRetryFolders: async () => {
      state._foldersError = false;
      await loadFolders();
      renderAll();
    },
    onRefreshFolder: (folder) => {
      selectFolder(folder);
    },
    onMoveMail: (mailId) => {
      removeMail(mailId);
    },
    onUnifiedInbox: selectUnifiedInbox,
    onSearchFolder: selectSearchFolder,
  };
}

function mailListActions() {
  return {
    onSelect: selectMail,
    onCompose: () => openCompose({ accountId: activeAccountId() }, () => refreshCurrentFolder()),
    onRefresh: () => {
      state._mailsError = false;
      refreshCurrentFolder();
    },
    onLoadMore: loadMoreMails,
    onSearch: searchMails,
    onToggleStar: toggleMailStar,
    // Context menu actions
    onReply: handleReply,
    onReplyAll: handleReplyAll,
    onForward: handleForward,
    onArchive: handleArchive,
    onDelete: handleDelete,
    onMarkRead: handleMarkRead,
    onMarkUnread: handleMarkUnread,
    // Spam actions
    onReportSpam: handleReportSpam,
    onReportNotSpam: handleReportNotSpam,
    // Category actions
    onAddCategory: handleAddCategory,
    onRemoveCategory: handleRemoveCategory,
    // Pin action
    onTogglePin: toggleMailPin,
    // Flag follow-up actions
    onFlagMail: handleFlagMail,
    onUnflagMail: handleUnflagMail,
    // Sweep sender
    onSweepSender: handleSweepSender,
    // Batch actions (multi-select)
    onBatchArchive: handleBatchArchive,
    onBatchDelete: handleBatchDelete,
    onBatchMarkRead: handleBatchMarkRead,
    onBatchMove: handleBatchMove,
  };
}

function mailViewActions() {
  return {
    onReply: handleReply,
    onReplyAll: handleReplyAll,
    onForward: handleForward,
    onArchive: handleArchive,
    onDelete: handleDelete,
    onMarkUnread: handleMarkUnread,
    onSnooze: handleSnooze,
    onSelectThread: (mailId) => selectMail(mailId),
  };
}

// ---------------------------------------------------------------------------
// Safe render wrapper — error boundaries for views
// ---------------------------------------------------------------------------
function safeRender(fn, fallbackEl) {
  try { fn(); } catch (e) {
    console.error('Render error:', e);
    if (fallbackEl) fallbackEl.innerHTML = '<div style="padding:20px;color:red;">Something went wrong. <button onclick="location.reload()">Reload</button></div>';
  }
}

function renderAll() {
  safeRender(() => renderSidebar(sidebarEl, state, sidebarActions()), sidebarEl);
  safeRender(() => renderMailList(mailListEl, state, mailListActions()), mailListEl);
  safeRender(() => renderMailView(mailViewEl, state, mailViewActions()), mailViewEl);
}

function renderList() {
  safeRender(() => renderMailList(mailListEl, state, mailListActions()), mailListEl);
}

function renderView() {
  safeRender(() => renderMailView(mailViewEl, state, mailViewActions()), mailViewEl);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------
async function loadAccounts() {
  try {
    state.accounts = await api.getAccounts();
  } catch (err) {
    showToast(`Failed to load accounts: ${err}`, 'error');
    state.accounts = [];
  }
}

async function loadFolders() {
  try {
    const folders = await api.getFolders(activeAccountId());
    state.folders = Array.isArray(folders) ? folders : [];
    state._foldersError = false;
  } catch (err) {
    state._foldersError = true;
    state.folders = [];
  }
}

async function loadMails(append = false) {
  if (state.loading) return;
  state.loading = true;
  state._mailsError = false;
  if (!append) renderList(); // Show skeleton state

  try {
    const perPage = 50;
    const result = await api.getMails(activeAccountId(), state.activeFolder, state.page, perPage);

    const newMails = Array.isArray(result) ? result : (result.mails || []);
    if (append) {
      state.mails = [...state.mails, ...newMails];
    } else {
      state.mails = newMails;
    }
    state.hasMore = newMails.length >= perPage;
  } catch (err) {
    state._mailsError = true;
    if (!append) state.mails = [];
    state.hasMore = false;
  }

  state.loading = false;

  // After sync: if the selected mail no longer exists in the loaded mails, deselect it
  if (state.selectedMail && !state.mails.some((m) => m.id === state.selectedMail.id)) {
    state.selectedMail = null;
    state.mailBody = null;
    renderView();
  }

  renderList();
}

let _loadingMailId = null;

async function loadMailBody(mailId) {
  _loadingMailId = mailId;

  // Check body cache first (LRU: re-insert on access to move to end)
  if (bodyCache.has(mailId)) {
    const cached = bodyCache.get(mailId);
    bodyCache.delete(mailId);
    bodyCache.set(mailId, cached);
    state.mailBody = cached;
    renderView();
    return;
  }

  state.mailBody = null;
  renderView(); // Show loading state

  try {
    const body = await api.getMailBody(mailId);
    // Race condition guard: discard if a newer load was initiated
    if (_loadingMailId !== mailId) return;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.mailBody = body;
      bodyCache.set(mailId, body);
      // LRU eviction: limit cache to 100 entries
      if (bodyCache.size > 100) {
        const oldest = bodyCache.keys().next().value;
        bodyCache.delete(oldest);
      }
      renderView();
    }
  } catch (err) {
    if (_loadingMailId !== mailId) return;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      showToast(`Failed to load email body: ${err}`, 'error');
      state.mailBody = { text: '', html: '' };
      renderView();
    }
  }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
async function selectFolder(folder) {
  state.activeFolder = folder;
  state.page = 0;
  state.selectedMail = null;
  state.mailBody = null;
  state.hasMore = true;
  state._mailsError = false;
  state._unifiedInbox = false;
  state._activeSearchFolder = null;

  // Close sidebar on mobile
  sidebarEl.classList.remove('sidebar-open');

  renderSidebar(sidebarEl, state, sidebarActions());
  renderView();
  await loadMails();
  prefetchBodies(state.mails);
}

async function selectMail(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;

  const prevId = state.selectedMail ? state.selectedMail.id : null;
  state.selectedMail = mail;

  // Toggle selection CSS classes without full re-render (preserves scroll)
  updateMailItemSelection(prevId, mailId);

  // Mark as read if unread
  if (!mail.is_read) {
    try {
      await api.markRead(activeAccountId(), state.activeFolder, mailId);
      mail.is_read = true;
      // Update just the unread styling on this item
      const item = mailListEl.querySelector(`.mail-item[data-mail-id="${CSS.escape(mailId)}"]`);
      if (item) {
        item.classList.remove('unread');
        const dot = item.querySelector('.mail-item-unread-dot');
        if (dot) dot.remove();
      }
    } catch {
      // Non-critical, ignore
    }
  }

  await loadMailBody(mailId);
}

/** Toggle selected class on old/new mail items without re-rendering the list. */
function updateMailItemSelection(prevId, newId) {
  if (prevId) {
    const prev = mailListEl.querySelector(`.mail-item[data-mail-id="${CSS.escape(prevId)}"]`);
    if (prev) {
      prev.classList.remove('selected');
      prev.setAttribute('aria-selected', 'false');
    }
  }
  if (newId) {
    const next = mailListEl.querySelector(`.mail-item[data-mail-id="${CSS.escape(newId)}"]`);
    if (next) {
      next.classList.add('selected');
      next.setAttribute('aria-selected', 'true');
      next.scrollIntoView({ block: 'nearest' });
    }
  }
}

async function refreshCurrentFolder() {
  state.page = 0;
  state.hasMore = true;

  try {
    await api.refreshFolder(activeAccountId(), state.activeFolder);
  } catch (err) {
    showToast(`Refresh failed: ${err}`, 'error');
  }

  await loadMails();
  await loadFolders();
  // Only update badge numbers instead of full sidebar re-render
  updateUnreadBadges(sidebarEl, state);
}

async function loadMoreMails() {
  state.page += 1;
  await loadMails(true);
}

/**
 * Parse search query for operators: from:, subject:, to:, has:attachment.
 * Remaining text is treated as a general query.
 */
function parseSearchOperators(query) {
  const result = { text: '', from: '', subject: '', to: '', hasAttachment: false };
  const remaining = [];

  const tokens = query.match(/(?:[^\s"]+|"[^"]*")+/g) || [];
  for (const token of tokens) {
    const lower = token.toLowerCase();
    if (lower.startsWith('from:')) {
      result.from = token.slice(5).replace(/^"|"$/g, '');
    } else if (lower.startsWith('subject:')) {
      result.subject = token.slice(8).replace(/^"|"$/g, '');
    } else if (lower.startsWith('to:')) {
      result.to = token.slice(3).replace(/^"|"$/g, '');
    } else if (lower === 'has:attachment' || lower === 'has:attachments') {
      result.hasAttachment = true;
    } else {
      remaining.push(token);
    }
  }
  result.text = remaining.join(' ');
  return result;
}

async function searchMails(query) {
  if (!query) {
    state.page = 0;
    state.hasMore = true;
    await loadMails();
    return;
  }

  state.loading = true;
  renderList();

  try {
    const accountId = activeAccountId();
    const parsed = parseSearchOperators(query);
    const hasOperators = parsed.from || parsed.subject || parsed.to || parsed.hasAttachment;

    if (hasOperators) {
      const results = await api.searchLocal(
        parsed.text || '', accountId, state.activeFolder,
        parsed.from || null, parsed.subject || null, parsed.to || null,
        parsed.hasAttachment || false
      );
      state.mails = Array.isArray(results) ? results : (results.mails || []);
    } else {
      const results = await api.searchLocal(query, accountId, state.activeFolder);
      state.mails = Array.isArray(results) ? results : (results.mails || []);
    }
    state.hasMore = false;
  } catch (err) {
    showToast(`Search failed: ${err}`, 'error');
  }

  state.loading = false;
  renderList();
}

async function toggleMailStar(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  const newStarred = !(mail?.is_starred);
  // Optimistic update
  if (mail) mail.is_starred = newStarred;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.is_starred = newStarred;
  }
  renderList();

  try {
    await api.toggleStar(activeAccountId(), state.activeFolder, mailId, newStarred);
  } catch (err) {
    // Revert on failure
    if (mail) mail.is_starred = !newStarred;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.is_starred = !newStarred;
    }
    renderList();
    showToast(`Failed to toggle star: ${err}`, 'error');
  }
}

async function toggleMailPin(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;
  const newPinned = !mail.is_pinned;
  // Optimistic update
  mail.is_pinned = newPinned;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.is_pinned = newPinned;
  }
  renderList();

  try {
    if (newPinned) {
      await api.pinMail(mailId);
    } else {
      await api.unpinMail(mailId);
    }
  } catch (err) {
    mail.is_pinned = !newPinned;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.is_pinned = !newPinned;
    }
    renderList();
    showToast(`Failed to toggle pin: ${err}`, 'error');
  }
}

async function handleFlagMail(mailId, dueDate) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;
  // Optimistic update
  mail.flag_due_date = dueDate;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.flag_due_date = dueDate;
  }
  renderList();

  try {
    await api.flagMail(mailId, dueDate);
  } catch (err) {
    mail.flag_due_date = null;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.flag_due_date = null;
    }
    renderList();
    showToast(`Failed to flag mail: ${err}`, 'error');
  }
}

async function handleUnflagMail(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;
  const prev = mail.flag_due_date;
  mail.flag_due_date = null;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.flag_due_date = null;
  }
  renderList();

  try {
    await api.unflagMail(mailId);
  } catch (err) {
    mail.flag_due_date = prev;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.flag_due_date = prev;
    }
    renderList();
    showToast(`Failed to unflag mail: ${err}`, 'error');
  }
}

function handleSnooze(mail) {
  // Mail was snoozed via the toolbar dropdown. Remove it from current view.
  if (mail.snoozed_until) {
    // Snoozed_until was already set by the snooze command.
    // Just update local state and re-render.
    const localMail = state.mails.find((m) => m.id === mail.id);
    if (localMail) localMail.snoozed_until = mail.snoozed_until;
  }
  removeMail(mail.id);
}

function handleReply(mail) {
  const prefill = makeReplyPrefill(mail, state.mailBody);
  prefill.accountId = activeAccountId();
  prefill.replyTo = mail.id;
  openCompose(prefill, () => refreshCurrentFolder());
}

function handleReplyAll(mail) {
  const prefill = makeReplyPrefill(mail, state.mailBody, true);
  prefill.accountId = activeAccountId();
  prefill.replyTo = mail.id;
  openCompose(prefill, () => refreshCurrentFolder());
}

function handleForward(mail) {
  const prefill = makeForwardPrefill(mail, state.mailBody);
  prefill.accountId = activeAccountId();
  openCompose(prefill, () => refreshCurrentFolder());
}

async function handleArchive(mail) {
  const originalFolder = state.activeFolder;
  const accountId = activeAccountId();
  const mailCopy = { ...mail };

  try {
    await api.archiveMail(accountId, originalFolder, mail.id);
    removeMail(mail.id);
    showToast('Email archived. [Undo]', 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Archive', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        renderList();
        showToast('Archive undone.', 'info');
      } catch (e) {
        showToast(`Undo failed: ${e}`, 'error');
      }
    });
  } catch (err) {
    showToast(`Failed to archive: ${err}`, 'error');
  }
}

async function handleDelete(mail) {
  const confirmed = await showDialog({
    title: 'Delete Email',
    message: 'Move this email to Trash?',
    confirmLabel: 'Delete',
    danger: true,
  });
  if (!confirmed) return;

  const originalFolder = state.activeFolder;
  const accountId = activeAccountId();
  const mailCopy = { ...mail };

  try {
    await api.deleteMail(accountId, originalFolder, mail.id);
    removeMail(mail.id);
    showToast('Email deleted. [Undo]', 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Trash', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        renderList();
        showToast('Delete undone.', 'info');
      } catch (e) {
        showToast(`Undo failed: ${e}`, 'error');
      }
    });
  } catch (err) {
    showToast(`Failed to delete: ${err}`, 'error');
  }
}

async function handleMarkUnread(mail) {
  try {
    await api.markUnread(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = false;
    renderList();
    showToast('Marked as unread.', 'info');
  } catch (err) {
    showToast(`Failed to mark unread: ${err}`, 'error');
  }
}

async function handleMarkRead(mail) {
  try {
    await api.markRead(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = true;
    renderList();
  } catch (err) {
    showToast(`Failed to mark as read: ${err}`, 'error');
  }
}

async function handleReportSpam(mail) {
  try {
    await api.reportSpam(mail.id);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes('Spam')) mail.categories.push('Spam');
    renderList();
    showToast('Reported as spam.', 'info');
  } catch (err) {
    showToast(`Failed to report spam: ${err}`, 'error');
  }
}

async function handleReportNotSpam(mail) {
  try {
    await api.reportNotSpam(mail.id);
    if (mail.categories) {
      mail.categories = mail.categories.filter((c) => c !== 'Spam');
    }
    renderList();
    showToast('Marked as not spam.', 'info');
  } catch (err) {
    showToast(`Failed to unmark spam: ${err}`, 'error');
  }
}

async function handleAddCategory(mail, category) {
  try {
    await api.addCategory(mail.id, category);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes(category)) mail.categories.push(category);
    renderList();
  } catch (err) {
    showToast(`Failed to add category: ${err}`, 'error');
  }
}

async function handleRemoveCategory(mail, category) {
  try {
    await api.removeCategory(mail.id, category);
    if (mail.categories) {
      mail.categories = mail.categories.filter((c) => c !== category);
    }
    renderList();
  } catch (err) {
    showToast(`Failed to remove category: ${err}`, 'error');
  }
}

async function handleSweepSender(mail) {
  // Extract sender email from the "from" field
  const fromField = mail.from || '';
  const emailMatch = fromField.match(/<([^>]+)>/) || [null, fromField];
  const senderEmail = (emailMatch[1] || fromField).trim();
  if (!senderEmail) {
    showToast('Cannot determine sender email.', 'error');
    return;
  }

  const confirmed = await showDialog({
    title: 'Sweep Sender',
    message: `Delete ALL emails from "${senderEmail}" in this folder?`,
    confirmLabel: 'Delete All',
    danger: true,
  });
  if (!confirmed) return;

  if (_isOffline) {
    queueAction({ type: 'sweep_sender', accountId: activeAccountId(), folder: state.activeFolder, senderEmail });
    // Optimistic local removal
    const toRemove = state.mails.filter(m => (m.from || '').includes(senderEmail));
    for (const m of toRemove) removeMail(m.id);
    return;
  }

  try {
    const count = await api.sweepSender(activeAccountId(), state.activeFolder, senderEmail);
    showToast(`Deleted ${count || 'all'} emails from ${senderEmail}.`, 'success');
    // Remove matching mails from local state
    const toRemove = state.mails.filter(m => (m.from || '').includes(senderEmail)).map(m => m.id);
    for (const id of toRemove) removeMail(id);
  } catch (err) {
    showToast(`Sweep failed: ${err}`, 'error');
  }
}

// ---------------------------------------------------------------------------
// Batch actions (multi-select)
// ---------------------------------------------------------------------------
async function handleBatchArchive(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.archiveMail(accountId, folder, id);
      removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) archived.`, 'success');
}

async function handleBatchDelete(mailIds) {
  const confirmed = await showDialog({
    title: 'Delete Emails',
    message: `Move ${mailIds.size || mailIds.length} email(s) to Trash?`,
    confirmLabel: 'Delete',
    danger: true,
  });
  if (!confirmed) return;

  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.deleteMail(accountId, folder, id);
      removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) deleted.`, 'success');
}

async function handleBatchMarkRead(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  for (const id of mailIds) {
    try {
      await api.markRead(accountId, folder, id);
      const mail = state.mails.find((m) => m.id === id);
      if (mail) mail.is_read = true;
    } catch {}
  }
  renderList();
  showToast(`${mailIds.size || mailIds.length} email(s) marked as read.`, 'info');
}

async function handleBatchMove(mailIds, targetFolder) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.moveMail(accountId, folder, id, targetFolder);
      removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) moved to ${targetFolder}.`, 'success');
}

function removeMail(mailId) {
  state.mails = state.mails.filter((m) => m.id !== mailId);
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail = null;
    state.mailBody = null;
  }
  renderList();
  renderView();
}

async function reloadAccounts() {
  await loadAccounts();
  if (state.accounts.length === 0) {
    openOnboarding({ isFirstRun: true, onAccountAdded: reloadAccounts });
    return;
  }
  state.activeAccount = 0;
  state.activeFolder = 'INBOX';
  state.page = 0;
  state.selectedMail = null;
  state.mailBody = null;
  await loadFolders();
  await loadMails();
  prefetchBodies(state.mails);
  renderAll();
}

async function switchAccount(index) {
  if (index === state.activeAccount) return;
  if (index < 0 || index >= state.accounts.length) return;

  state.activeAccount = index;
  state.activeFolder = 'INBOX';
  state.page = 0;
  state.selectedMail = null;
  state.mailBody = null;
  state.hasMore = true;
  state._mailsError = false;

  await loadFolders();
  renderAll();
  await loadMails();
  prefetchBodies(state.mails);
}

// ---------------------------------------------------------------------------
// Unified Inbox — fetch from ALL accounts' INBOX and merge by date
// ---------------------------------------------------------------------------
async function selectUnifiedInbox() {
  state._unifiedInbox = true;
  state._activeSearchFolder = null;
  state.activeFolder = 'INBOX';
  state.page = 0;
  state.selectedMail = null;
  state.mailBody = null;
  state.hasMore = false;
  state._mailsError = false;

  sidebarEl.classList.remove('sidebar-open');
  renderSidebar(sidebarEl, state, sidebarActions());
  renderView();

  state.loading = true;
  renderList();

  try {
    const allMails = [];
    for (const account of state.accounts) {
      try {
        const result = await api.getMails(account.id, 'INBOX', 0, 50);
        const mails = Array.isArray(result) ? result : (result.mails || []);
        // Tag each mail with its account info for the badge
        for (const m of mails) {
          m._accountEmail = account.email;
          m._accountName = account.name || account.email;
          m._accountId = account.id;
        }
        allMails.push(...mails);
      } catch {
        // Skip failing accounts
      }
    }
    // Sort by date descending
    allMails.sort((a, b) => new Date(b.date) - new Date(a.date));
    state.mails = allMails;
  } catch (err) {
    state._mailsError = true;
    state.mails = [];
  }

  state.loading = false;
  renderList();
  prefetchBodies(state.mails);
}

// ---------------------------------------------------------------------------
// Search Folders — run a virtual query
// ---------------------------------------------------------------------------
async function selectSearchFolder(name, query) {
  state._unifiedInbox = false;
  state._activeSearchFolder = name;
  state.selectedMail = null;
  state.mailBody = null;
  state.hasMore = false;
  state._mailsError = false;

  sidebarEl.classList.remove('sidebar-open');
  renderSidebar(sidebarEl, state, sidebarActions());
  renderView();

  state.loading = true;
  renderList();

  try {
    const accountId = activeAccountId();
    const results = await api.searchLocal(query, accountId, state.activeFolder);
    state.mails = Array.isArray(results) ? results : (results.mails || []);
  } catch (err) {
    state._mailsError = true;
    state.mails = [];
  }

  state.loading = false;
  renderList();
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------
function setupKeyboardShortcuts() {
  /**
   * Build a key descriptor string from a KeyboardEvent, matching the format
   * stored by the shortcut editor (e.g. "Ctrl+n", "Shift+R", "F5", "Delete").
   */
  function eventToKeyDesc(e) {
    let desc = '';
    if (e.ctrlKey || e.metaKey) desc += 'Ctrl+';
    if (e.shiftKey) desc += 'Shift+';
    if (e.altKey) desc += 'Alt+';
    desc += e.key;
    return desc;
  }

  // Global keyboard shortcuts listener — intentional: registered once at
  // app startup and needed for the entire app lifetime. No cleanup needed.
  document.addEventListener('keydown', (e) => {
    const shortcuts = getMergedShortcuts();
    const keyDesc = eventToKeyDesc(e);

    // Focus mode shortcut (works even in inputs)
    if (keyDesc === shortcuts.focusMode) {
      e.preventDefault();
      toggleFocusMode();
      return;
    }

    // Skip if user is typing in an input/textarea
    const tag = e.target.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    // Don't intercept if overlay is open
    const anyOverlayOpen =
      !document.getElementById('compose-overlay').hidden ||
      !document.getElementById('settings-overlay').hidden ||
      !document.getElementById('onboarding-overlay').hidden ||
      !document.getElementById('dialog-container').hidden;
    if (anyOverlayOpen) return;

    // Configurable shortcut actions
    const shortcutActions = {
      [shortcuts.compose]: () => {
        openCompose({ accountId: activeAccountId() }, () => refreshCurrentFolder());
      },
      [shortcuts.reply]: () => { if (state.selectedMail) handleReply(state.selectedMail); },
      [shortcuts.replyAll]: () => { if (state.selectedMail) handleReplyAll(state.selectedMail); },
      [shortcuts.forward]: () => { if (state.selectedMail) handleForward(state.selectedMail); },
      [shortcuts.delete]: () => { if (state.selectedMail) handleDelete(state.selectedMail); },
      [shortcuts.archive]: () => { if (state.selectedMail) handleArchive(state.selectedMail); },
      [shortcuts.star]: () => { if (state.selectedMail) toggleMailStar(state.selectedMail.id); },
      [shortcuts.unread]: () => { if (state.selectedMail) handleMarkUnread(state.selectedMail); },
      [shortcuts.next]: () => {
        if (!state.mails.length) return;
        const idx = state.selectedMail
          ? state.mails.findIndex((m) => m.id === state.selectedMail.id) : -1;
        if (idx + 1 < state.mails.length) selectMail(state.mails[idx + 1].id);
      },
      [shortcuts.prev]: () => {
        if (!state.mails.length) return;
        const idx = state.selectedMail
          ? state.mails.findIndex((m) => m.id === state.selectedMail.id)
          : state.mails.length;
        if (idx - 1 >= 0) selectMail(state.mails[idx - 1].id);
      },
      [shortcuts.refresh]: () => { refreshCurrentFolder(); },
    };

    // Check if the current key matches any configured shortcut
    if (shortcutActions[keyDesc]) {
      e.preventDefault();
      shortcutActions[keyDesc]();
      return;
    }

    // Also check alternative keys for next/prev (ArrowDown/ArrowUp always work)
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (!state.mails.length) return;
      const idx = state.selectedMail
        ? state.mails.findIndex((m) => m.id === state.selectedMail.id) : -1;
      if (idx + 1 < state.mails.length) selectMail(state.mails[idx + 1].id);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (!state.mails.length) return;
      const idx = state.selectedMail
        ? state.mails.findIndex((m) => m.id === state.selectedMail.id)
        : state.mails.length;
      if (idx - 1 >= 0) selectMail(state.mails[idx - 1].id);
      return;
    }

    // Non-configurable shortcuts
    switch (e.key) {
      case 'Enter': {
        if (!state.selectedMail && state.mails.length > 0) {
          selectMail(state.mails[0].id);
        }
        break;
      }

      case 'Escape': {
        if (state.selectedMail) {
          e.preventDefault();
          const prevId = state.selectedMail.id;
          state.selectedMail = null;
          state.mailBody = null;
          updateMailItemSelection(prevId, null);
          renderView();
        }
        break;
      }

      case 'Tab': {
        e.preventDefault();
        const zones = [sidebarEl, mailListEl, mailViewEl];
        const current = zones.findIndex(z => z.contains(document.activeElement));
        const nextZone = e.shiftKey
          ? zones[(current - 1 + zones.length) % zones.length]
          : zones[(current + 1) % zones.length];

        const focusable = nextZone.querySelector(
          'button:not([disabled]), [tabindex="0"], input, select, textarea, a[href]'
        );
        if (focusable) focusable.focus();
        break;
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Tauri event listeners
// ---------------------------------------------------------------------------
function setupTauriEvents() {
  // Listen for events from the Rust backend
  try {
    const { listen } = window.__TAURI__.event;

    listen('sync-progress', (event) => {
      const { folder, current, total } = event.payload || {};
      if (folder && total) {
        state._syncing = true;
        state._syncProgress = Math.round((current / total) * 100);
        // Update document title with progress for Windows taskbar hover
        document.title = `Exospine - Syncing ${state._syncProgress}%`;
        renderList();
      }
      if (current >= total) {
        state._syncing = false;
        state._syncProgress = 0;
        // Restore default title
        document.title = 'Exospine';
        renderList();
      }
    });

    listen('new-mail', (event) => {
      const { count, folder, thread_id, accountId: evtAccountId } = event.payload || {};
      if (count && folder === state.activeFolder) {
        // Feature 1: Skip notification for muted threads
        if (thread_id && isThreadMuted(thread_id)) {
          refreshCurrentFolder();
          return;
        }
        playNotificationSound(evtAccountId);
        showToast(`${count} new email${count > 1 ? 's' : ''} received.`, 'info');
        refreshCurrentFolder();
      }
    });

    listen('toast', (event) => {
      const { message, level } = event.payload || {};
      if (message) {
        showToast(message, level || 'info');
      }
    });

    listen('oauth-complete', () => {
      showToast('Account linked successfully.', 'success');
      reloadAccounts();
    });
  } catch {
    // Tauri events API not available (running outside Tauri or dev mode)
    console.debug('Tauri event API not available. Running in standalone mode.');
  }
}

// ---------------------------------------------------------------------------
// Auto-lock after inactivity (15 minutes)
// ---------------------------------------------------------------------------
const LOCK_TIMEOUT_MS = 15 * 60 * 1000; // 15 minutes
let lastActivity = Date.now();
let isLocked = false;

function resetActivity() {
  lastActivity = Date.now();
}

// Track user activity — these global listeners are intentional: they are
// registered once at app startup and needed for the entire app lifetime
// (auto-lock after inactivity). No cleanup is necessary.
document.addEventListener('mousemove', resetActivity);
document.addEventListener('keydown', resetActivity);
document.addEventListener('mousedown', resetActivity);
document.addEventListener('scroll', resetActivity, true);
document.addEventListener('touchstart', resetActivity);

function clearSensitiveData() {
  bodyCache.clear();
  state.mailBody = null;
  state.selectedMail = null;
  _prefetchInFlight.clear();
}

function showLockScreen() {
  if (isLocked) return;
  isLocked = true;
  clearSensitiveData();
  const el = document.getElementById('lock-screen');
  if (el) el.hidden = false;
}

function hideLockScreen() {
  isLocked = false;
  lastActivity = Date.now();
  const el = document.getElementById('lock-screen');
  if (el) el.hidden = true;
}

// Unlock button
const lockUnlockBtn = document.getElementById('lock-screen-unlock');
if (lockUnlockBtn) {
  lockUnlockBtn.addEventListener('click', hideLockScreen);
}

// Check inactivity every 30 seconds
_intervals.push(setInterval(() => {
  if (!isLocked && Date.now() - lastActivity > LOCK_TIMEOUT_MS) {
    showLockScreen();
  }
}, 30000));

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Connection status indicator
// ---------------------------------------------------------------------------
function setupConnectionStatus() {
  const dot = document.getElementById('connection-dot');
  const text = document.getElementById('connection-text');
  if (!dot || !text) return;

  function updateStatus(online) {
    if (online) {
      dot.className = 'connection-dot connected';
      text.textContent = 'Connected';
    } else {
      dot.className = 'connection-dot disconnected';
      text.textContent = 'Disconnected';
    }
  }

  updateStatus(navigator.onLine);
  window.addEventListener('online', () => updateStatus(true));
  window.addEventListener('offline', () => updateStatus(false));

  // Periodic health check every 30 seconds
  _intervals.push(setInterval(async () => {
    try {
      await api.getAccounts();
      updateStatus(true);
    } catch {
      updateStatus(false);
    }
  }, 30000));
}

// ---------------------------------------------------------------------------
// Resizable panels
// ---------------------------------------------------------------------------
function setupResizablePanels() {
  const sidebar = document.getElementById('sidebar');
  const mailList = document.getElementById('mail-list');
  const content = document.getElementById('content');
  if (!sidebar || !mailList || !content) return;

  // Restore saved widths
  const savedSidebarW = localStorage.getItem('exospine_sidebar_width');
  const savedListW = localStorage.getItem('exospine_list_width');
  if (savedSidebarW) {
    sidebar.style.width = savedSidebarW + 'px';
    sidebar.style.minWidth = savedSidebarW + 'px';
  }
  if (savedListW) {
    mailList.style.width = savedListW + 'px';
    mailList.style.minWidth = savedListW + 'px';
  }

  // Create splitters
  const splitter1 = document.createElement('div');
  splitter1.className = 'panel-splitter';
  splitter1.setAttribute('role', 'separator');
  splitter1.setAttribute('aria-label', 'Resize sidebar');
  sidebar.after(splitter1);

  const splitter2 = document.createElement('div');
  splitter2.className = 'panel-splitter';
  splitter2.setAttribute('role', 'separator');
  splitter2.setAttribute('aria-label', 'Resize mail list');
  mailList.after(splitter2);

  function makeDraggable(splitter, targetEl, minW, maxW, storageKey) {
    let startX, startW;
    const onMouseMove = (e) => {
      const delta = e.clientX - startX;
      const newW = Math.min(maxW, Math.max(minW, startW + delta));
      targetEl.style.width = newW + 'px';
      targetEl.style.minWidth = newW + 'px';
    };
    const onMouseUp = () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      document.body.classList.remove('col-resizing');
      lsSetItem(storageKey, String(parseInt(targetEl.style.width, 10)));
    };
    splitter.addEventListener('mousedown', (e) => {
      e.preventDefault();
      startX = e.clientX;
      startW = targetEl.getBoundingClientRect().width;
      document.body.classList.add('col-resizing');
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });
  }

  makeDraggable(splitter1, sidebar, 120, 350, 'exospine_sidebar_width');
  makeDraggable(splitter2, mailList, 200, 600, 'exospine_list_width');
}

// ---------------------------------------------------------------------------
// Drag-and-drop .eml file support
// ---------------------------------------------------------------------------
function setupEmlDragDrop() {
  const app = document.getElementById('app') || document.body;

  app.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
    app.classList.add('eml-drag-hover');
  });

  app.addEventListener('dragleave', (e) => {
    e.preventDefault();
    app.classList.remove('eml-drag-hover');
  });

  app.addEventListener('drop', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    app.classList.remove('eml-drag-hover');

    const files = Array.from(e.dataTransfer.files || []);
    const emlFiles = files.filter((f) => f.name.toLowerCase().endsWith('.eml'));

    if (emlFiles.length === 0) return;

    for (const file of emlFiles) {
      // Tauri v2: file.path gives the native path for dropped files
      const filePath = file.path || file.name;
      if (!filePath || filePath === file.name) {
        showToast('Cannot read dropped file: native path not available.', 'error');
        continue;
      }
      try {
        const entry = await api.openEmlFile(filePath);
        if (entry) {
          // Display in the reading pane
          state.selectedMail = entry;
          state.mailBody = { text: entry.body_text, html: entry.body_html };
          renderMailView(mailViewEl, entry, state.mailBody, {});
          showToast(`Opened: ${entry.subject || '(no subject)'}`, 'info');
        }
      } catch (err) {
        showToast(`Failed to open .eml file: ${err}`, 'error');
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Lazy folder sync — load other folders' counts in background one by one
// ---------------------------------------------------------------------------
async function lazyLoadFolderCounts() {
  const accountId = activeAccountId();
  if (!accountId || !state.folders.length) return;

  for (const folder of state.folders) {
    const folderName = typeof folder === 'object' ? folder.name : folder;
    // Skip the active folder — already loaded
    if (folderName === state.activeFolder) continue;

    // Add syncing indicator to this folder in the sidebar
    const folderEl = sidebarEl.querySelector(`.sidebar-folder[data-folder="${CSS.escape(folderName)}"]`);
    if (folderEl && !folderEl.querySelector('.folder-sync-indicator')) {
      const indicator = document.createElement('span');
      indicator.className = 'folder-sync-indicator';
      indicator.setAttribute('aria-label', 'Syncing');
      indicator.innerHTML = '<span class="spinner" style="width:10px;height:10px;border-width:1.5px;"></span>';
      folderEl.appendChild(indicator);
    }

    try {
      // Refresh this folder to get updated counts
      await api.refreshFolder(accountId, folderName);
    } catch {
      // Non-critical, ignore
    }

    // Remove syncing indicator
    if (folderEl) {
      const indicator = folderEl.querySelector('.folder-sync-indicator');
      if (indicator) indicator.remove();
    }
  }

  // Reload folders to get updated counts and update sidebar badges
  await loadFolders();
  updateUnreadBadges(sidebarEl, state);
}

// ---------------------------------------------------------------------------
// Dynamic favicon with unread badge
// ---------------------------------------------------------------------------
let _baseFaviconData = null;

function updateFavicon(unreadCount) {
  const canvas = document.createElement('canvas');
  canvas.width = 32;
  canvas.height = 32;
  const ctx = canvas.getContext('2d');

  // Draw base icon (simple envelope shape)
  ctx.fillStyle = '#00bcd4';
  ctx.fillRect(2, 6, 28, 20);
  ctx.fillStyle = '#00838f';
  ctx.beginPath();
  ctx.moveTo(2, 6);
  ctx.lineTo(16, 18);
  ctx.lineTo(30, 6);
  ctx.closePath();
  ctx.fill();

  // Draw unread badge if count > 0
  if (unreadCount > 0) {
    const text = unreadCount > 99 ? '99+' : String(unreadCount);
    const badgeRadius = text.length > 2 ? 10 : 8;
    const badgeX = 32 - badgeRadius;
    const badgeY = badgeRadius;

    ctx.beginPath();
    ctx.arc(badgeX, badgeY, badgeRadius, 0, 2 * Math.PI);
    ctx.fillStyle = '#e53935';
    ctx.fill();
    ctx.strokeStyle = '#1a1a2e';
    ctx.lineWidth = 1;
    ctx.stroke();

    ctx.fillStyle = '#fff';
    ctx.font = `bold ${text.length > 2 ? 8 : 10}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, badgeX, badgeY + 1);
  }

  // Apply as favicon
  let link = document.querySelector('link[rel="icon"]');
  if (!link) {
    link = document.createElement('link');
    link.rel = 'icon';
    document.head.appendChild(link);
  }
  link.href = canvas.toDataURL('image/png');
}

// ---------------------------------------------------------------------------
// PIN lock screen on startup
// ---------------------------------------------------------------------------
async function showPinLockScreen() {
  const pinHash = localStorage.getItem('exospine_pin_hash');
  if (!pinHash) return true; // No PIN set, proceed

  return new Promise((resolve) => {
    const lockEl = document.getElementById('lock-screen');
    if (!lockEl) { resolve(true); return; }

    let attempts = parseInt(localStorage.getItem('exospine_pin_attempts') || '0', 10);
    let lockedUntil = parseInt(localStorage.getItem('exospine_pin_locked_until') || '0', 10);

    lockEl.hidden = false;
    lockEl.innerHTML = `
      <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;background:var(--sidebar-bg, #1a1a2e);color:var(--pane-text, #e0e0e0);">
        <div style="font-size:28px;font-weight:700;margin-bottom:8px;">Exospine</div>
        <div style="font-size:14px;color:var(--pane-text-dim, #888);margin-bottom:24px;">Enter your PIN to unlock</div>
        <div id="pin-error" style="color:#e53935;font-size:13px;min-height:20px;margin-bottom:8px;"></div>
        <input type="password" id="pin-input" maxlength="6" inputmode="numeric" pattern="[0-9]*"
          style="width:180px;text-align:center;font-size:24px;letter-spacing:8px;padding:10px;border:2px solid var(--pane-border, #333);border-radius:8px;background:var(--pane-bg, #222);color:var(--pane-text, #eee);outline:none;"
          placeholder="****" />
        <button id="pin-submit" style="margin-top:16px;padding:8px 32px;font-size:14px;font-weight:600;background:var(--accent, #0078d6);color:#fff;border:none;border-radius:6px;cursor:pointer;">
          Unlock
        </button>
      </div>
    `;

    const input = lockEl.querySelector('#pin-input');
    const errorEl = lockEl.querySelector('#pin-error');
    const submitBtn = lockEl.querySelector('#pin-submit');

    function checkLockout() {
      if (lockedUntil > Date.now()) {
        const remaining = Math.ceil((lockedUntil - Date.now()) / 1000);
        errorEl.textContent = 'Too many attempts. Try again in ' + remaining + 's';
        input.disabled = true;
        submitBtn.disabled = true;
        setTimeout(checkLockout, 1000);
        return true;
      }
      input.disabled = false;
      submitBtn.disabled = false;
      errorEl.textContent = '';
      return false;
    }

    if (checkLockout()) {
      // Already locked out
    }

    input.focus();

    async function tryUnlock() {
      if (lockedUntil > Date.now()) return;
      const pin = input.value;
      if (!pin) return;

      const hash = await hashPin(pin);
      if (hash === pinHash) {
        // Success
        localStorage.setItem('exospine_pin_attempts', '0');
        localStorage.removeItem('exospine_pin_locked_until');
        lockEl.hidden = true;
        lockEl.innerHTML = '';
        resolve(true);
      } else {
        attempts++;
        localStorage.setItem('exospine_pin_attempts', String(attempts));
        input.value = '';

        if (attempts >= 3) {
          lockedUntil = Date.now() + 30000; // 30 seconds
          localStorage.setItem('exospine_pin_locked_until', String(lockedUntil));
          localStorage.setItem('exospine_pin_attempts', '0');
          attempts = 0;
          checkLockout();
        } else {
          errorEl.textContent = 'Wrong PIN. ' + (3 - attempts) + ' attempt(s) remaining.';
        }
        input.focus();
      }
    }

    submitBtn.addEventListener('click', tryUnlock);
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') tryUnlock();
    });
  });
}

async function init() {
  // Register service worker for offline caching
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
  }

  // PIN lock screen — block until unlocked
  const pinOk = await showPinLockScreen();
  if (!pinOk) return;

  setupKeyboardShortcuts();
  setupTauriEvents();
  setupConnectionStatus();
  setupResizablePanels();
  setupOfflineMode();
  setupEmlDragDrop();
  restoreImportedTheme();

  // Request notification permission early
  try { if (Notification.permission === 'default') Notification.requestPermission(); } catch {}

  // Load accounts first — critical path
  await loadAccounts();

  if (state.accounts.length === 0) {
    // Load settings for theming even on onboarding
    try {
      state.settings = await api.getSettings();
      applyTheme(state.settings.theme);
      if (state.settings.language) {
        setLanguage(state.settings.language);
        applyDirection(state.settings.language);
      }
    } catch {}
    openOnboarding({ isFirstRun: true, onAccountAdded: reloadAccounts });
    renderAll();
    // Hide splash screen
    const splash2 = document.getElementById('splash-screen');
    if (splash2) {
      splash2.classList.add('splash-fade-out');
      splash2.addEventListener('animationend', () => splash2.remove());
    }
    return;
  }

  // Prioritize rendering the mail list BEFORE loading settings, contacts, etc.
  // Only load the active folder (INBOX) mails first
  await loadFolders();
  renderAll();
  await loadMails();
  prefetchBodies(state.mails);

  // Hide splash screen
  const splash = document.getElementById('splash-screen');
  if (splash) {
    splash.classList.add('splash-fade-out');
    splash.addEventListener('animationend', () => splash.remove());
  }

  // Load settings after mail list is visible — non-blocking
  try {
    state.settings = await api.getSettings();
    applyTheme(state.settings.theme);
    if (state.settings.language) {
      setLanguage(state.settings.language);
      applyDirection(state.settings.language);
    }
  } catch {
    // Settings not available yet, use defaults
  }

  // Defer non-critical loads to idle time
  const idleCb = typeof requestIdleCallback === 'function' ? requestIdleCallback : (fn) => setTimeout(fn, 100);

  idleCb(() => {
    // Data retention: cleanup old messages if configured
    try {
      const retentionDays = parseInt(localStorage.getItem('exospine_retention_days') || '0', 10);
      if (retentionDays > 0) {
        api.cleanupOldMessages(retentionDays).catch(() => {});
      }
    } catch {}
  });

  // Lazy folder sync: load other folders' counts in background one by one
  lazyLoadFolderCounts();

  // Background sync: fetch all remaining mails from IMAP without blocking the UI
  const syncAccountId = activeAccountId();
  const syncFolder = state.activeFolder;
  if (syncAccountId) {
    state._syncing = true;
    state._syncProgress = 0;
    renderList();

    api.syncAllMails(syncAccountId, syncFolder).then(async (count) => {
      state._syncing = false;
      state._syncProgress = 0;
      document.title = 'Exospine'; // Restore title after sync
      if (count > 0) {
        playNotificationSound(syncAccountId);
        showToast(`Synced ${count} mails in background.`, 'info');
        await loadMails();
        // Update folder unread counts in sidebar
        await loadFolders();
        renderSidebar(sidebarEl, state, sidebarActions());
      } else {
        renderList();
      }
    }).catch((err) => {
      state._syncing = false;
      state._syncProgress = 0;
      document.title = 'Exospine'; // Restore title on sync error
      renderList();
      console.warn('Background sync failed:', err);
    });
  }

  // Check for pending mailto: link (if launched from OS mailto handler)
  try {
    const mailto = await api.getPendingMailto();
    if (mailto && mailto.to) {
      openCompose({
        accountId: activeAccountId(),
        to: mailto.to,
        cc: mailto.cc || '',
        bcc: mailto.bcc || '',
        subject: mailto.subject || '',
        body: mailto.body || '',
      }, () => refreshCurrentFolder());
    }
  } catch {
    // No pending mailto, ignore
  }

  // Background task: update window title and favicon with unread count every 30 seconds
  async function updateUnreadTitle() {
    try {
      const count = await api.getUnreadCount();
      document.title = count > 0 ? `Exospine (${count} unread)` : 'Exospine';
      updateFavicon(count);
    } catch {
      // Silent fail
    }
  }
  updateUnreadTitle();
  _intervals.push(setInterval(updateUnreadTitle, 30000));

  // Background task: periodic check for new mails using check_interval_secs from settings
  const checkIntervalMs = ((state.settings && state.settings.check_interval) || 60) * 1000;
  let _lastMailCount = (state.mails || []).length;
  let _lastMailFirstId = (state.mails && state.mails[0]) ? state.mails[0].id : null;
  _intervals.push(setInterval(async () => {
    try {
      const accountId = activeAccountId();
      if (!accountId || !state.activeFolder) return;
      const refreshed = await api.refreshFolder(accountId, state.activeFolder);
      if (refreshed && refreshed.length > 0) {
        // Refresh detected new mails on server
        await loadMails();
        // Update folder unread counts in sidebar
        await loadFolders();
        renderSidebar(sidebarEl, state, sidebarActions());
        const currentCount = (state.mails || []).length;
        const currentFirstId = (state.mails && state.mails[0]) ? state.mails[0].id : null;
        const newCount = Math.max(0, currentCount - _lastMailCount);
        if (newCount > 0 || currentFirstId !== _lastMailFirstId) {
          if (newCount > 0) {
            // Feature 1: Check muted threads — still refresh but skip sound/toast
            const anyMuted = state.mails.slice(0, newCount).some(m => isThreadMuted(m.thread_id || m.id));
            if (!anyMuted) {
              playNotificationSound(activeAccountId());
              showToast(`${newCount} new email${newCount > 1 ? 's' : ''} received.`, 'info');
            }
          }
        }
        _lastMailCount = currentCount;
        _lastMailFirstId = currentFirstId;
      }
    } catch {
      // Silent fail for background refresh
    }
  }, checkIntervalMs));

  // Background task: check for due scheduled emails and snoozed emails every 60 seconds
  _intervals.push(setInterval(async () => {
    try {
      // Send due scheduled emails
      const sent = await api.sendDueScheduled();
      if (sent > 0) {
        showToast(`${sent} scheduled email${sent > 1 ? 's' : ''} sent.`, 'success');
      }
    } catch {
      // Silent fail for background task
    }

    try {
      // Check for due snoozed emails
      const dueSnoozed = await api.getDueSnoozed();
      if (dueSnoozed && dueSnoozed.length > 0) {
        // Unsnooze them
        for (const mailId of dueSnoozed) {
          try {
            await api.unsnoozeMail(mailId);
          } catch {
            // Silent
          }
        }
        showToast(`${dueSnoozed.length} snoozed email${dueSnoozed.length > 1 ? 's' : ''} returned.`, 'info');
        // Reload mail list to show unsnoozed mails
        await loadMails();
        // Update folder unread counts in sidebar
        await loadFolders();
        renderSidebar(sidebarEl, state, sidebarActions());
      }
    } catch {
      // Silent fail
    }
  }, 60000));

  // Background task: check followups every 5 minutes and show toast if overdue
  _intervals.push(setInterval(async () => {
    try {
      const accountId = activeAccountId();
      if (!accountId) return;
      // Auto-resolve followups that have been replied to
      const resolved = await api.checkFollowups(accountId);
      if (resolved && resolved.length > 0) {
        showToast(`${resolved.length} follow-up(s) resolved (reply received).`, 'success');
      }
      // Check for overdue followups
      const followups = await api.getFollowups();
      const now = new Date();
      const overdue = (followups || []).filter(f => !f.resolved && f.due_date && new Date(f.due_date) < now);
      if (overdue.length > 0) {
        showToast(`${overdue.length} follow-up(s) are overdue!`, 'warning');
      }
    } catch {
      // Silent fail
    }
  }, 5 * 60 * 1000));

  // Cleanup all intervals on page unload
  window.addEventListener('beforeunload', () => {
    _intervals.forEach(id => clearInterval(id));
  });
}

// Start
init().catch((err) => {
  console.error('Exospine init failed:', err);
  showToast(`Initialization error: ${err}`, 'error', 0);
});
