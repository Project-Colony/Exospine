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


// ── Folder order persistence (drag reorder) ─────────────────────
function loadFolderOrder(section) {
  try {
    const raw = localStorage.getItem('exospine_folder_order_' + section);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

function saveFolderOrder(section, order) {
  localStorage.setItem('exospine_folder_order_' + section, JSON.stringify(order));
}

function applyFolderOrder(folders, section) {
  const order = loadFolderOrder(section);
  if (!order) return folders;
  const nameOf = (f) => typeof f === 'string' ? f : f.name;
  const orderMap = new Map(order.map((n, i) => [n, i]));
  const sorted = [...folders].sort((a, b) => {
    const ai = orderMap.has(nameOf(a)) ? orderMap.get(nameOf(a)) : 9999;
    const bi = orderMap.has(nameOf(b)) ? orderMap.get(nameOf(b)) : 9999;
    return ai - bi;
  });
  return sorted;
}

// ── Collapsible sidebar sections ─────────────────────────────────────
const FAVORITE_FOLDERS = ['INBOX', 'Sent', '[Gmail]/Sent Mail', '[Gmail]/Starred'];

function loadCollapsedSections() {
  try {
    const raw = localStorage.getItem('exospine_sidebar_collapsed');
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveCollapsedSections(collapsed) {
  localStorage.setItem('exospine_sidebar_collapsed', JSON.stringify(collapsed));
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
          <div class="sidebar-account-icon acct-color-${i % ACCOUNT_COLORS.length}" aria-hidden="true">${accInitial}</div>
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
        <div class="sidebar-account-icon acct-color-${state.activeAccount % ACCOUNT_COLORS.length}" aria-hidden="true">${initial}</div>
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
    // Group folders into collapsible sections: Favorites and Folders
    const collapsed = loadCollapsedSections();
    const favFolders = [];
    const otherFolders = [];

    for (const folder of state.folders) {
      const name = typeof folder === 'string' ? folder : folder.name;
      if (FAVORITE_FOLDERS.includes(name)) {
        favFolders.push(folder);
      } else {
        otherFolders.push(folder);
      }
    }

    // Apply saved folder order
    const orderedFavFolders = applyFolderOrder(favFolders, 'favorites');
    const orderedOtherFolders = applyFolderOrder(otherFolders, 'folders');

    // Helper to render a folder item
    function renderFolderItem(folder) {
      const name = typeof folder === 'string' ? folder : folder.name;
      const unread = typeof folder === 'object' ? (folder.unread_count || folder.unread || 0) : 0;
      const isActive = name === state.activeFolder && !state._unifiedInbox;
      const icon = FOLDER_ICONS[name] || '\uD83D\uDCC1';
      const displayName = folderDisplayName(name);
      return `
        <div class="sidebar-folder${isActive ? ' active' : ''}" data-folder="${esc(name)}" role="option" aria-selected="${isActive}" tabindex="0" draggable="true">
          <span class="sidebar-folder-icon" aria-hidden="true">${icon}</span>
          <span class="sidebar-folder-label">${esc(displayName)}</span>
          ${unread > 0 ? `<span class="sidebar-folder-badge" aria-label="${unread} unread">${unread}</span>` : ''}
        </div>
      `;
    }

    // Favorites section
    const favCollapsed = collapsed['favorites'] || false;
    html += `<div class="sidebar-section-header" data-section="favorites" role="button" tabindex="0" aria-expanded="${!favCollapsed}" title="Toggle Favorites">
      <span class="sidebar-section-arrow">${favCollapsed ? '\u25B6' : '\u25BC'}</span>
      <span>Favorites</span>
    </div>`;
    html += `<div class="sidebar-folders sidebar-section-content${favCollapsed ? ' collapsed' : ''}" role="listbox" aria-label="Favorite folders">`;
    for (const folder of orderedFavFolders) {
      html += renderFolderItem(folder);
    }
    html += '</div>';

    // Other Folders section
    const foldersCollapsed = collapsed['folders'] || false;
    html += `<div class="sidebar-section-header" data-section="folders" role="button" tabindex="0" aria-expanded="${!foldersCollapsed}" title="Toggle Folders">
      <span class="sidebar-section-arrow">${foldersCollapsed ? '\u25B6' : '\u25BC'}</span>
      <span>Folders</span>
    </div>`;
    html += `<div class="sidebar-folders sidebar-section-content${foldersCollapsed ? ' collapsed' : ''}" role="listbox" aria-label="Mail folders">`;
    for (const folder of orderedOtherFolders) {
      html += renderFolderItem(folder);
    }
    html += '</div>';
  }

  // Search Folders section (collapsible)
  const customSearchFolders = loadCustomSearchFolders();
  const allSearchFolders = [...PREDEFINED_SEARCH_FOLDERS, ...customSearchFolders];
  const collapsedSections = loadCollapsedSections();
  const searchCollapsed = collapsedSections['search-folders'] || false;

  html += `<div class="sidebar-section-header" data-section="search-folders" role="button" tabindex="0" aria-expanded="${!searchCollapsed}" title="Toggle Search Folders">
    <span class="sidebar-section-arrow">${searchCollapsed ? '\u25B6' : '\u25BC'}</span>
    <span>${esc(t('search_folders'))}</span>
  </div>`;
  html += `<div class="sidebar-folders sidebar-search-folders sidebar-section-content${searchCollapsed ? ' collapsed' : ''}" role="listbox" aria-label="Search folders">`;
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
      <button class="sidebar-btn" id="sidebar-add-account" title="${t('add_account')}" aria-label="${t('add_account')}">
        <span aria-hidden="true">+</span>
        <span>${t('add_account')}</span>
      </button>
      <button class="sidebar-btn" id="sidebar-awaiting-reply" title="Awaiting Reply" aria-label="Awaiting Reply">
        <span aria-hidden="true">\u{1F552}</span>
        <span>Awaiting Reply</span>
      </button>
      <button class="sidebar-btn" id="sidebar-tasks" title="Tasks" aria-label="Tasks">
        <span aria-hidden="true">\u2611</span>
        <span>Tasks</span>
      </button>
      <button class="sidebar-btn" id="sidebar-analytics" title="Analytics" aria-label="Analytics">
        <span aria-hidden="true">\uD83D\uDCCA</span>
        <span>Analytics</span>
      </button>
      <button class="sidebar-btn" id="sidebar-calendar" title="Calendar" aria-label="Calendar">
        <span aria-hidden="true">\uD83D\uDCC5</span>
        <span>Calendar</span>
      </button>
      <button class="sidebar-btn" id="sidebar-contacts" title="Contacts" aria-label="Contacts">
        <span aria-hidden="true">\uD83D\uDC64</span>
        <span>Contacts</span>
      </button>
      <button class="sidebar-btn" id="sidebar-settings" title="${t('settings')}" aria-label="${t('settings')}">
        <span aria-hidden="true">\u2699</span>
        <span>${t('settings')}</span>
      </button>
    </div>
  `;

  el.innerHTML = html;

  // ── Event delegation: ONE set of listeners on the sidebar container ──
  // Remove previous delegated listeners if any (stored on el itself)
  if (el._sidebarClickHandler) el.removeEventListener('click', el._sidebarClickHandler);
  if (el._sidebarKeydownHandler) el.removeEventListener('keydown', el._sidebarKeydownHandler);
  if (el._sidebarContextmenuHandler) el.removeEventListener('contextmenu', el._sidebarContextmenuHandler);
  if (el._sidebarDragoverHandler) el.removeEventListener('dragover', el._sidebarDragoverHandler);
  if (el._sidebarDragleaveHandler) el.removeEventListener('dragleave', el._sidebarDragleaveHandler);
  if (el._sidebarDropHandler) el.removeEventListener('drop', el._sidebarDropHandler);
  if (el._sidebarFolderDragstartHandler) el.removeEventListener('dragstart', el._sidebarFolderDragstartHandler);
  if (el._sidebarFolderDragendHandler) el.removeEventListener('dragend', el._sidebarFolderDragendHandler);
  if (el._sidebarFolderReorderDragover) el.removeEventListener('dragover', el._sidebarFolderReorderDragover);
  if (el._sidebarFolderReorderDrop) el.removeEventListener('drop', el._sidebarFolderReorderDrop);

  el._sidebarClickHandler = (e) => {
    // Collapsible section headers
    const sectionHeader = e.target.closest('.sidebar-section-header');
    if (sectionHeader) {
      const section = sectionHeader.dataset.section;
      const collapsed = loadCollapsedSections();
      collapsed[section] = !collapsed[section];
      saveCollapsedSections(collapsed);
      // Force re-render by clearing cache
      _sidebarCacheKey = null;
      renderSidebar(el, state, actions);
      return;
    }

    // Account switcher
    const acctItem = e.target.closest('.sidebar-account-item');
    if (acctItem) {
      const index = parseInt(acctItem.dataset.accountIndex, 10);
      if (!isNaN(index) && actions.onSwitchAccount) actions.onSwitchAccount(index);
      return;
    }

    // Unified inbox
    const unifiedBtn = e.target.closest('[data-action="unified-inbox"]');
    if (unifiedBtn) {
      if (actions.onUnifiedInbox) actions.onUnifiedInbox();
      return;
    }

    // Delete custom search folder
    const deleteBtn = e.target.closest('.sidebar-search-folder-delete');
    if (deleteBtn) {
      e.stopPropagation();
      const name = deleteBtn.dataset.deleteSearch;
      const custom = loadCustomSearchFolders().filter((f) => f.name !== name);
      saveCustomSearchFolders(custom);
      renderSidebar(el, state, actions);
      return;
    }

    // Folder select
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (folderItem) {
      const folderName = folderItem.dataset.folder;
      if (folderName && actions.onFolderSelect) actions.onFolderSelect(folderName);
      return;
    }

    // Search folder select
    const sfItem = e.target.closest('.sidebar-folder[data-search-folder]');
    if (sfItem) {
      if (actions.onSearchFolder) actions.onSearchFolder(sfItem.dataset.searchFolder, sfItem.dataset.searchQuery);
      return;
    }

    // Add search folder
    const addSfBtn = e.target.closest('[data-action="add-search-folder"]');
    if (addSfBtn) {
      const name = prompt('Search folder name:');
      if (!name) return;
      const query = prompt('Search query (e.g. "from:user@example.com"):');
      if (!query) return;
      const custom = loadCustomSearchFolders();
      custom.push({ name, query, icon: '\uD83D\uDD0D', custom: true });
      saveCustomSearchFolders(custom);
      renderSidebar(el, state, actions);
      return;
    }

    // Bottom buttons
    if (e.target.closest('#sidebar-add-account') && actions.onAddAccount) { actions.onAddAccount(); return; }
    if (e.target.closest('#sidebar-awaiting-reply') && actions.onOpenAwaitingReply) { actions.onOpenAwaitingReply(); return; }
    if (e.target.closest('#sidebar-tasks') && actions.onOpenTasks) { actions.onOpenTasks(); return; }
    if (e.target.closest('#sidebar-analytics') && actions.onOpenAnalytics) { actions.onOpenAnalytics(); return; }
    if (e.target.closest('#sidebar-calendar') && actions.onOpenCalendar) { actions.onOpenCalendar(); return; }
    if (e.target.closest('#sidebar-contacts') && actions.onOpenContacts) { actions.onOpenContacts(); return; }
    if (e.target.closest('#sidebar-settings') && actions.onOpenSettings) { actions.onOpenSettings(); return; }
    if (e.target.closest('#sidebar-retry-folders') && actions.onRetryFolders) { actions.onRetryFolders(); return; }
  };

  el._sidebarKeydownHandler = (e) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    const target = e.target.closest('.sidebar-account-item, [data-action="unified-inbox"], .sidebar-folder[data-folder], .sidebar-folder[data-search-folder], [data-action="add-search-folder"]');
    if (!target) return;
    e.preventDefault();
    // Simulate click — the click handler above handles all cases
    target.click();
  };

  el._sidebarContextmenuHandler = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (!folderItem) return;
    e.preventDefault();
    const folderName = folderItem.dataset.folder;
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
  };

  el._sidebarDragoverHandler = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (!folderItem) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    folderItem.classList.add('drag-over');
  };

  el._sidebarDragleaveHandler = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (folderItem) folderItem.classList.remove('drag-over');
  };

  el._sidebarDropHandler = async (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (!folderItem) return;
    e.preventDefault();
    folderItem.classList.remove('drag-over');

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
      const targetFolder = folderItem.dataset.folder;
      if (data.folder === targetFolder) return;
      await api.moveMail(data.accountId, data.folder, data.mailId, targetFolder);
      showToast(`Moved to ${folderDisplayName(targetFolder)}`, 'success');
      if (actions.onMoveMail) actions.onMoveMail(data.mailId);
    } catch (err) {
      showToast(`Move failed: ${err}`, 'error');
    }
  };

  el.addEventListener('click', el._sidebarClickHandler);
  el.addEventListener('keydown', el._sidebarKeydownHandler);
  el.addEventListener('contextmenu', el._sidebarContextmenuHandler);
  el.addEventListener('dragover', el._sidebarDragoverHandler);
  el.addEventListener('dragleave', el._sidebarDragleaveHandler);
  el.addEventListener('drop', el._sidebarDropHandler);

  // ── Feature 7: Drag reorder folders within sections ──────────
  el._sidebarFolderDragstartHandler = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder][draggable="true"]');
    if (!folderItem) return;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/x-folder-reorder', folderItem.dataset.folder);
    folderItem.classList.add('dragging');
  };

  el._sidebarFolderDragendHandler = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (folderItem) folderItem.classList.remove('dragging');
    el.querySelectorAll('.sidebar-folder.drag-over-reorder').forEach(f => f.classList.remove('drag-over-reorder'));
  };

  el._sidebarFolderReorderDragover = (e) => {
    const folderItem = e.target.closest('.sidebar-folder[data-folder]');
    if (!folderItem) return;
    if (!e.dataTransfer.types.includes('text/x-folder-reorder')) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    el.querySelectorAll('.sidebar-folder.drag-over-reorder').forEach(f => f.classList.remove('drag-over-reorder'));
    folderItem.classList.add('drag-over-reorder');
  };

  el._sidebarFolderReorderDrop = (e) => {
    const targetFolder = e.target.closest('.sidebar-folder[data-folder]');
    if (!targetFolder) return;
    const draggedName = e.dataTransfer.getData('text/x-folder-reorder');
    if (!draggedName) return;
    e.preventDefault();
    targetFolder.classList.remove('drag-over-reorder');

    const targetName = targetFolder.dataset.folder;
    if (draggedName === targetName) return;

    // Determine which section both folders are in
    const section = targetFolder.closest('.sidebar-section-content');
    if (!section) return;
    const sectionHeader = section.previousElementSibling;
    if (!sectionHeader) return;
    const sectionName = sectionHeader.dataset.section;
    if (!sectionName || sectionName === 'search-folders') return;

    // Get all folder names in this section in current DOM order
    const items = section.querySelectorAll('.sidebar-folder[data-folder]');
    const order = [];
    for (const item of items) order.push(item.dataset.folder);

    // Reorder: move dragged to target position
    const fromIdx = order.indexOf(draggedName);
    const toIdx = order.indexOf(targetName);
    if (fromIdx < 0 || toIdx < 0) return;
    order.splice(fromIdx, 1);
    order.splice(toIdx, 0, draggedName);

    saveFolderOrder(sectionName, order);
    // Force re-render
    _sidebarCacheKey = null;
    renderSidebar(el, state, actions);
  };

  el.addEventListener('dragstart', el._sidebarFolderDragstartHandler);
  el.addEventListener('dragend', el._sidebarFolderDragendHandler);
  // Separate listeners for folder reorder (uses different data type)
  el.addEventListener('dragover', el._sidebarFolderReorderDragover);
  el.addEventListener('drop', el._sidebarFolderReorderDrop);

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
