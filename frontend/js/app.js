// Exospine — Main application entry point

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { renderSidebar, updateUnreadBadges } from './views/sidebar.js';
import { renderMailList } from './views/mail_list.js';
import { renderMailView, isThreadMuted } from './views/mail_view.js';
import { openCompose, makeReplyPrefill, makeForwardPrefill } from './views/compose.js';
import { openSettings, hashPin, restoreImportedTheme } from './views/settings.js';
import { openOnboarding } from './views/onboarding.js';
import { openAnalytics } from './views/analytics.js';
import { openCalendar } from './views/calendar.js';
import { openContacts } from './views/contacts.js';
import { openTasks, openAwaitingReply } from './views/tasks.js';
import { setLanguage } from './i18n.js';

// Module imports
import { state, activeAccountId, sidebarEl, mailListEl, mailViewEl, hamburgerBtn, bodyCache, _prefetchInFlight, _intervals } from './state.js';
import { setupOfflineMode } from './offline.js';
import { handleArchive, handleDelete, handleMarkRead, handleMarkUnread, toggleMailStar, handleFlagMail, handleUnflagMail, handleSweepSender, handleReportSpam, handleReportNotSpam, handleAddCategory, handleRemoveCategory, setMailActionCallbacks } from './mail_actions.js';
import { handleBatchArchive, handleBatchDelete, handleBatchMarkRead, handleBatchMove, setBatchOpsCallbacks } from './batch_ops.js';
import { applyTheme } from './theme.js';
import { setupKeyboardShortcuts, setShortcutCallbacks } from './shortcuts.js';
import { playNotificationSound, setSoundEnabled, updateUnreadTitle } from './notifications_ui.js';
import { setupConnectionStatus, setupResizablePanels } from './panels.js';

// ---------------------------------------------------------------------------
// Prefetch — silently preload bodies for the first visible mails
// ---------------------------------------------------------------------------
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
          setSoundEnabled(s.sound_notifications);
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
// Wire up callbacks for extracted modules
// ---------------------------------------------------------------------------
setMailActionCallbacks({ renderList, renderView, removeMail });
setBatchOpsCallbacks({ renderList, removeMail });
setShortcutCallbacks({
  handleReply,
  handleReplyAll,
  handleForward,
  handleDelete,
  handleArchive,
  toggleMailStar,
  handleMarkUnread,
  selectMail,
  refreshCurrentFolder,
  toggleFocusMode,
  renderView,
  updateMailItemSelection,
});

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

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
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
