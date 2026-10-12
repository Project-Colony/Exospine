// Exospine — Global application state

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
export const state = {
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
export function activeAccountId() {
  const acc = state.accounts[state.activeAccount];
  return acc ? acc.id : '';
}

// DOM references (with null guards)
export const sidebarEl = document.getElementById('sidebar');
export const mailListEl = document.getElementById('mail-list');
export const mailViewEl = document.getElementById('mail-view');
export const hamburgerBtn = document.getElementById('hamburger-btn');

if (!sidebarEl || !mailListEl || !mailViewEl) {
  console.error('Missing required DOM elements (#sidebar, #mail-list, or #mail-view)');
}

// ---------------------------------------------------------------------------
// Mail lookup helpers
// ---------------------------------------------------------------------------

/** Find a mail by ID in the current mails list. */
export function findMail(mailId) {
  return state.mails.find(m => m.id === mailId);
}

/** Update a mail (and selectedMail if matching) with the given properties. */
export function updateMail(mailId, updates) {
  const mail = findMail(mailId);
  if (mail) Object.assign(mail, updates);
  if (state.selectedMail && state.selectedMail.id === mailId) {
    Object.assign(state.selectedMail, updates);
  }
  return mail;
}

// Body cache — avoids redundant API calls for already-loaded mail bodies
export const bodyCache = new Map(); // mailId -> { text, html }

// Prefetch — silently preload bodies for the first visible mails
// Rate-limited: track in-flight requests in a Set to avoid duplicates.
export const _prefetchInFlight = new Set();

// Interval tracking — all intervals are stored for cleanup on beforeunload
export const _intervals = [];
