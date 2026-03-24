// Exospine — Sidebar view

import { showToast } from '../components/toast.js';
import { showContextMenu } from '../components/context_menu.js';
import * as api from '../api.js';
import { t } from '../i18n.js';

const FOLDER_ICONS = {
  'INBOX':                    '\uD83D\uDCE5',
  'Sent':                     '\uD83D\uDCE4',
  'Drafts':                   '\uD83D\uDCDD',
  'Trash':                    '\uD83D\uDDD1',
  'Spam':                     '\u26A0',
  'Archive':                  '\uD83D\uDCE6',
  'Starred':                  '\u2B50',
  // Gmail IMAP folder names
  '[Gmail]/Sent Mail':        '\uD83D\uDCE4',
  '[Gmail]/Drafts':           '\uD83D\uDCDD',
  '[Gmail]/Trash':            '\uD83D\uDDD1',
  '[Gmail]/Spam':             '\u26A0',
  '[Gmail]/All Mail':         '\uD83D\uDCE6',
  '[Gmail]/Starred':          '\u2B50',
  '[Gmail]/Important':        '\u2757',
};

const ACCOUNT_COLORS = [
  'rgb(0, 120, 214)',
  'rgb(160, 90, 220)',
  'rgb(220, 80, 60)',
  'rgb(40, 167, 69)',
  'rgb(230, 150, 0)',
  'rgb(0, 180, 160)',
];

// ── Search Folders (Virtual Folders) ────────────────────────────────

const PREDEFINED_SEARCH_FOLDERS = [
  { name: 'unread', query: 'is:unread', icon: '\uD83D\uDCE9' },
  { name: 'flagged', query: 'is:starred', icon: '\u2B50' },
  { name: 'has_attachments', query: 'has:attachment', icon: '\uD83D\uDCCE' },
  { name: 'this_week', query: 'date:week', icon: '\uD83D\uDCC5' },
];

function loadCustomSearchFolders() {
  try {
    const raw = localStorage.getItem('exospine_search_folders');
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveCustomSearchFolders(folders) {
  localStorage.setItem('exospine_search_folders', JSON.stringify(folders));
}

// ── Sidebar render cache ─────────────────────────────────────────────
let _sidebarCacheKey = null;

/** Build a lightweight fingerprint of sidebar-relevant state. */
function _sidebarFingerprint(state) {
  return JSON.stringify({
    aa: state.activeAccount,
    af: state.activeFolder,
    ui: state._unifiedInbox,
    asf: state._activeSearchFolder,
    fe: state._foldersError,
    accts: state.accounts.map(a => a.id),
    folders: state.folders.map(f => typeof f === 'string' ? f : f.name + ':' + (f.unread_count || f.unread || 0)),
  });
}

/**
 * Render the sidebar.
 * @param {HTMLElement} el  #sidebar element
 * @param {object} state    app state
 * @param {object} actions  { onFolderSelect, onAddAccount, onOpenSettings, onUnifiedInbox, onSearchFolder }
 */
export function renderSidebar(el, state, actions) {
  // Skip full re-render if nothing relevant changed
  const key = _sidebarFingerprint(state);
  if (key === _sidebarCacheKey) return;
  _sidebarCacheKey = key;
  // Bounds check: clamp activeAccount to valid range
  if (state.activeAccount >= state.accounts.length) {
    state.activeAccount = Math.max(0, state.accounts.length - 1);
  }
  const account = state.accounts[state.activeAccount];
  const color = ACCOUNT_COLORS[state.activeAccount % ACCOUNT_COLORS.length];
  const initial = account ? (account.name || account.email || '?')[0].toUpperCase() : '?';

  let html = '';

  // Account switcher (if multiple accounts)
  if (state.accounts.length > 1) {
    html += '<div class="sidebar-account-switcher" role="listbox" aria-label="Account switcher">';
    for (let i = 0; i < state.accounts.length; i++) {
      const acc = state.accounts[i];
      const accColor = ACCOUNT_COLORS[i % ACCOUNT_COLORS.length];
      const accInitial = (acc.name || acc.email || '?')[0].toUpperCase();
      const isActive = i === state.activeAccount;
      html += `
        <div class="sidebar-account-item${isActive ? ' active' : ''}" data-account-index="${i}" role="option" aria-selected="${isActive}" tabindex="0">
          <div class="sidebar-account-icon" style="background:${accColor}" aria-hidden="true">${accInitial}</div>
          <div class="sidebar-account-item-info">
            <div class="sidebar-account-name">${esc(acc.name || acc.email)}</div>
            <div class="sidebar-account-email">${esc(acc.email || '')}</div>
          </div>
        </div>
      `;
    }
    html += '</div>';
  } else if (account) {
    // Single account header
    html += `
      <div class="sidebar-account" role="banner" aria-label="Current account">
        <div class="sidebar-account-icon" style="background:${color}" aria-hidden="true">${initial}</div>
        <div>
          <div class="sidebar-account-name">${esc(account.name || account.email)}</div>
          <div class="sidebar-account-email">${esc(account.email || '')}</div>
        </div>
      </div>
    `;
  }

  // Unified Inbox — shown when multiple accounts exist
  if (state.accounts.length > 1) {
    const isUnified = state._unifiedInbox === true;
    html += `
      <div class="sidebar-folders" role="listbox" aria-label="Virtual folders">
        <div class="sidebar-folder unified-inbox${isUnified ? ' active' : ''}" data-action="unified-inbox" role="option" aria-selected="${isUnified}" tabindex="0">
          <span class="sidebar-folder-icon" aria-hidden="true">\uD83D\uDCEC</span>
          <span class="sidebar-folder-label">${esc(t('all_inboxes'))}</span>
        </div>
      </div>
    `;
  }

  // Folders — error state
  if (state._foldersError) {
    html += `
      <div class="sidebar-error">
        <span>${t('failed_to_load_emails')}</span>
        <button class="retry-btn" id="sidebar-retry-folders" aria-label="${t('retry')}">${t('retry')}</button>
      </div>
    `;
  } else {
    // Folders
    html += '<div class="sidebar-folders" role="listbox" aria-label="Mail folders">';
    for (const folder of state.folders) {
      const name = typeof folder === 'string' ? folder : folder.name;
      const unread = typeof folder === 'object' ? (folder.unread_count || folder.unread || 0) : 0;
      const isActive = name === state.activeFolder && !state._unifiedInbox;
      const icon = FOLDER_ICONS[name] || '\uD83D\uDCC1';
      const displayName = folderDisplayName(name);

      html += `
        <div class="sidebar-folder${isActive ? ' active' : ''}" data-folder="${esc(name)}" role="option" aria-selected="${isActive}" tabindex="0">
          <span class="sidebar-folder-icon" aria-hidden="true">${icon}</span>
          <span class="sidebar-folder-label">${esc(displayName)}</span>
          ${unread > 0 ? `<span class="sidebar-folder-badge" aria-label="${unread} unread">${unread}</span>` : ''}
        </div>
      `;
    }
    html += '</div>';
  }

  // Search Folders section
  const customSearchFolders = loadCustomSearchFolders();
  const allSearchFolders = [...PREDEFINED_SEARCH_FOLDERS, ...customSearchFolders];

  html += '<div class="sidebar-section-title">' + esc(t('search_folders')) + '</div>';
  html += '<div class="sidebar-folders sidebar-search-folders" role="listbox" aria-label="Search folders">';
  for (const sf of allSearchFolders) {
    const isActive = state._activeSearchFolder === sf.name;
    const icon = sf.icon || '\uD83D\uDD0D';
    const displayName = t(sf.name) || sf.name;
    html += `
      <div class="sidebar-folder search-folder${isActive ? ' active' : ''}" data-search-folder="${esc(sf.name)}" data-search-query="${esc(sf.query)}" role="option" aria-selected="${isActive}" tabindex="0">
        <span class="sidebar-folder-icon" aria-hidden="true">${icon}</span>
        <span class="sidebar-folder-label">${esc(displayName)}</span>
        ${sf.custom ? `<button class="sidebar-search-folder-delete" data-delete-search="${esc(sf.name)}" title="${t('delete')}" aria-label="Delete search folder">\u00D7</button>` : ''}
      </div>
    `;
  }
  html += `
    <div class="sidebar-folder search-folder-add" data-action="add-search-folder" role="button" tabindex="0">
      <span class="sidebar-folder-icon" aria-hidden="true">+</span>
      <span class="sidebar-folder-label">${esc(t('new_search_folder'))}</span>
    </div>
  `;
  html += '</div>';

  // Bottom buttons
  html += `
    <div class="sidebar-bottom">
      <button class="sidebar-btn" id="sidebar-add-account" aria-label="${t('add_account')}">
        <span aria-hidden="true">+</span>
        <span>${t('add_account')}</span>
      </button>
      <button class="sidebar-btn" id="sidebar-analytics" aria-label="Analytics">
        <span aria-hidden="true">\uD83D\uDCCA</span>
        <span>Analytics</span>
      </button>
      <button class="sidebar-btn" id="sidebar-calendar" aria-label="Calendar">
        <span aria-hidden="true">\uD83D\uDCC5</span>
        <span>Calendar</span>
      </button>
      <button class="sidebar-btn" id="sidebar-contacts" aria-label="Contacts">
        <span aria-hidden="true">\uD83D\uDC64</span>
        <span>Contacts</span>
      </button>
      <button class="sidebar-btn" id="sidebar-settings" aria-label="${t('settings')}">
        <span aria-hidden="true">\u2699</span>
        <span>${t('settings')}</span>
      </button>
    </div>
  `;

  el.innerHTML = html;

  // Event listeners — account switcher
  el.querySelectorAll('.sidebar-account-item').forEach((item) => {
    const handler = () => {
      const index = parseInt(item.dataset.accountIndex, 10);
      if (!isNaN(index) && actions.onSwitchAccount) {
        actions.onSwitchAccount(index);
      }
    };
    item.addEventListener('click', handler);
    item.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handler();
      }
    });
  });

  // Event listener — unified inbox
  const unifiedBtn = el.querySelector('[data-action="unified-inbox"]');
  if (unifiedBtn) {
    const handler = () => {
      if (actions.onUnifiedInbox) actions.onUnifiedInbox();
    };
    unifiedBtn.addEventListener('click', handler);
    unifiedBtn.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
    });
  }

  // Event listeners — folders
  el.querySelectorAll('.sidebar-folder[data-folder]').forEach((item) => {
    const folderName = item.dataset.folder;

    const handler = () => {
      if (folderName && actions.onFolderSelect) {
        actions.onFolderSelect(folderName);
      }
    };
    item.addEventListener('click', handler);
    item.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handler();
      }
    });

    // Feature 3: Context menu on folder right-click
    item.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const menuItems = [
        {
          label: '\u21BB ' + t('refresh'),
          action: () => {
            if (actions.onRefreshFolder) actions.onRefreshFolder(folderName);
          },
        },
        {
          label: '\u2709 Mark all as read',
          action: () => {
            showToast('Mark all as read is not yet implemented.', 'info');
          },
        },
      ];
      showContextMenu(e.clientX, e.clientY, menuItems);
    });

    // Feature 4: Drag & Drop — folder as drop target
    item.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      item.classList.add('drag-over');
    });

    item.addEventListener('dragleave', () => {
      item.classList.remove('drag-over');
    });

    item.addEventListener('drop', async (e) => {
      e.preventDefault();
      item.classList.remove('drag-over');

      const raw = e.dataTransfer.getData('application/x-exospine-mail');
      if (!raw) return;

      let data;
      try {
        data = JSON.parse(raw);
      } catch (parseErr) {
        console.error('Invalid drag data:', parseErr);
        return;
      }

      try {
        const targetFolder = folderName;

        // Don't move to the same folder
        if (data.folder === targetFolder) return;

        await api.moveMail(data.accountId, data.folder, data.mailId, targetFolder);
        showToast(`Moved to ${folderDisplayName(targetFolder)}`, 'success');

        // Notify the app to refresh
        if (actions.onMoveMail) actions.onMoveMail(data.mailId);
      } catch (err) {
        showToast(`Move failed: ${err}`, 'error');
      }
    });
  });

  // Event listeners — search folders
  el.querySelectorAll('.sidebar-folder[data-search-folder]').forEach((item) => {
    const sfName = item.dataset.searchFolder;
    const sfQuery = item.dataset.searchQuery;
    const handler = () => {
      if (actions.onSearchFolder) actions.onSearchFolder(sfName, sfQuery);
    };
    item.addEventListener('click', handler);
    item.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
    });
  });

  // Delete custom search folders
  el.querySelectorAll('.sidebar-search-folder-delete').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const name = btn.dataset.deleteSearch;
      const custom = loadCustomSearchFolders().filter((f) => f.name !== name);
      saveCustomSearchFolders(custom);
      renderSidebar(el, state, actions);
    });
  });

  // Add search folder
  const addSfBtn = el.querySelector('[data-action="add-search-folder"]');
  if (addSfBtn) {
    const handler = () => {
      const name = prompt('Search folder name:');
      if (!name) return;
      const query = prompt('Search query (e.g. "from:user@example.com"):');
      if (!query) return;
      const custom = loadCustomSearchFolders();
      custom.push({ name, query, icon: '\uD83D\uDD0D', custom: true });
      saveCustomSearchFolders(custom);
      renderSidebar(el, state, actions);
    };
    addSfBtn.addEventListener('click', handler);
    addSfBtn.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
    });
  }

  const addBtn = el.querySelector('#sidebar-add-account');
  if (addBtn && actions.onAddAccount) {
    addBtn.addEventListener('click', actions.onAddAccount);
  }

  const analyticsBtn = el.querySelector('#sidebar-analytics');
  if (analyticsBtn && actions.onOpenAnalytics) {
    analyticsBtn.addEventListener('click', actions.onOpenAnalytics);
  }

  const calendarBtn = el.querySelector('#sidebar-calendar');
  if (calendarBtn && actions.onOpenCalendar) {
    calendarBtn.addEventListener('click', actions.onOpenCalendar);
  }

  const contactsBtn = el.querySelector('#sidebar-contacts');
  if (contactsBtn && actions.onOpenContacts) {
    contactsBtn.addEventListener('click', actions.onOpenContacts);
  }

  const settingsBtn = el.querySelector('#sidebar-settings');
  if (settingsBtn && actions.onOpenSettings) {
    settingsBtn.addEventListener('click', actions.onOpenSettings);
  }

  // Retry folders
  const retryBtn = el.querySelector('#sidebar-retry-folders');
  if (retryBtn && actions.onRetryFolders) {
    retryBtn.addEventListener('click', actions.onRetryFolders);
  }
}

/**
 * Lightweight unread badge update — only patches badge text in existing DOM
 * without re-rendering the entire sidebar. Call this instead of renderSidebar
 * when only unread counts changed.
 * @param {HTMLElement} el  #sidebar element
 * @param {object} state    app state (only state.folders is read)
 */
export function updateUnreadBadges(el, state) {
  for (const folder of state.folders) {
    const name = typeof folder === 'string' ? folder : folder.name;
    const unread = typeof folder === 'object' ? (folder.unread_count || folder.unread || 0) : 0;
    const folderEl = el.querySelector(`.sidebar-folder[data-folder="${CSS.escape(name)}"]`);
    if (!folderEl) continue;

    const existing = folderEl.querySelector('.sidebar-folder-badge');
    if (unread > 0) {
      if (existing) {
        // Just update the number text
        if (existing.textContent !== String(unread)) {
          existing.textContent = unread;
          existing.setAttribute('aria-label', `${unread} unread`);
        }
      } else {
        // Add badge
        const badge = document.createElement('span');
        badge.className = 'sidebar-folder-badge';
        badge.setAttribute('aria-label', `${unread} unread`);
        badge.textContent = unread;
        folderEl.appendChild(badge);
      }
    } else {
      // Remove badge if count is 0
      if (existing) existing.remove();
    }
  }
  // Update fingerprint cache so a subsequent full renderSidebar knows badges are current
  _sidebarCacheKey = _sidebarFingerprint(state);
}

/**
 * Return a human-friendly display name for IMAP folder paths.
 * Strips common prefixes like "[Gmail]/" for cleaner sidebar display,
 * and decodes IMAP modified UTF-7 encoded names.
 */
function folderDisplayName(name) {
  let display = name;
  if (display.startsWith('[Gmail]/')) display = display.slice(8);
  else if (display.startsWith('[Google Mail]/')) display = display.slice(14);
  else if (display.startsWith('INBOX/')) display = display.slice(6);
  else if (display.startsWith('INBOX.')) display = display.slice(6);

  // Decode IMAP modified UTF-7 (e.g. &AOk- → é)
  return decodeModifiedUtf7(display);
}

/**
 * Decode IMAP Modified UTF-7 (RFC 3501 Section 5.1.3).
 * Encoded sections start with '&' and end with '-'.
 * '&-' is a literal ampersand.
 * Other '&...encoded...-' sections use modified Base64 → UTF-16BE.
 */
function decodeModifiedUtf7(str) {
  return str.replace(/&([^-]*)-/g, (match, encoded) => {
    if (encoded === '') return '&'; // &- → literal &

    // Modified UTF-7 uses ',' instead of '/' in Base64
    const b64 = encoded.replace(/,/g, '/');
    // Decode base64 to bytes
    let binaryStr;
    try {
      binaryStr = atob(b64);
    } catch {
      return match; // Invalid base64, return as-is
    }

    // Interpret bytes as UTF-16BE
    let result = '';
    for (let i = 0; i < binaryStr.length; i += 2) {
      const hi = binaryStr.charCodeAt(i);
      const lo = i + 1 < binaryStr.length ? binaryStr.charCodeAt(i + 1) : 0;
      result += String.fromCharCode((hi << 8) | lo);
    }
    return result;
  });
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
