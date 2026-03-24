// Exospine — Mail list view (virtual scrolling + thread view + spam + categories)

import { formatDate } from '../date_format.js';
import { showContextMenu } from '../components/context_menu.js';
import { t } from '../i18n.js';
import { loadDisplayRules } from './settings.js';

const ITEM_HEIGHT = 80; // px per mail item
const THREAD_HEADER_HEIGHT = 80; // px for a collapsed thread header
let _scrollRAF = null;
let _scrollListener = null;
let _lastState = null;
let _lastActions = null;

// ── Multi-select state ──────────────────────────────────────────
const _selectedIds = new Set();

// ── Sort state ──────────────────────────────────────────────────
const SORT_OPTIONS = [
  { key: 'date-desc', label: 'Date (newest)' },
  { key: 'date-asc', label: 'Date (oldest)' },
  { key: 'sender-az', label: 'Sender A-Z' },
  { key: 'subject-az', label: 'Subject A-Z' },
];
let _currentSort = localStorage.getItem('exospine_mail_sort') || 'date-desc';

function sortMails(mails, sortKey) {
  const sorted = [...mails];
  switch (sortKey) {
    case 'date-asc':
      sorted.sort((a, b) => new Date(a.date) - new Date(b.date));
      break;
    case 'sender-az':
      sorted.sort((a, b) => (a.from_name || a.from || '').localeCompare(b.from_name || b.from || ''));
      break;
    case 'subject-az':
      sorted.sort((a, b) => (a.subject || '').localeCompare(b.subject || ''));
      break;
    case 'date-desc':
    default:
      sorted.sort((a, b) => new Date(b.date) - new Date(a.date));
      break;
  }
  return sorted;
}

/** Clear multi-select state */
export function clearMultiSelect() {
  _selectedIds.clear();
}

// ── Empty state SVG illustration ────────────────────────────────
const EMPTY_ENVELOPE_SVG = `<svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="10" y="20" width="60" height="44" rx="6" stroke="currentColor" stroke-width="2" fill="none"/>
  <path d="M10 26 L40 46 L70 26" stroke="currentColor" stroke-width="2" fill="none"/>
</svg>`;

// ── Scroll position preservation per folder ────────────────────────
const _folderScrollPositions = new Map();

// ── Predefined categories with colors ──────────────────────────────
const CATEGORIES = [
  { name: 'Important', color: '#e74c3c' },
  { name: 'Work', color: '#3498db' },
  { name: 'Personal', color: '#2ecc71' },
  { name: 'Finance', color: '#f39c12' },
  { name: 'Travel', color: '#9b59b6' },
];

// Category color lookup (includes Spam)
const CATEGORY_COLORS = {};
CATEGORIES.forEach((c) => { CATEGORY_COLORS[c.name] = c.color; });
CATEGORY_COLORS['Spam'] = '#95a5a6';

function getCategoryColor(name) {
  return CATEGORY_COLORS[name] || '#7f8c8d';
}

// ── Thread grouping (cached) ──────────────────────────────────────

let _cachedThreadMails = null; // reference to the mails array used for caching
let _cachedThreads = null;

/**
 * Group mails by thread_id. Returns an array of thread objects:
 * { thread_id, subject, messages: [mail], last_date, participants, unread_count, is_expanded }
 * Results are cached and only rebuilt when the mails array reference changes.
 */
function groupByThread(mails) {
  if (_cachedThreadMails === mails && _cachedThreads) return _cachedThreads;
  const map = new Map();
  for (const mail of mails) {
    // Only group mails that share a thread_id different from their own id.
    // If thread_id is empty or equals the mail's own id, treat as standalone.
    const tid = (mail.thread_id && mail.thread_id !== mail.id) ? mail.thread_id : mail.id;
    if (!map.has(tid)) {
      map.set(tid, []);
    }
    map.get(tid).push(mail);
  }

  const threads = [];
  for (const [thread_id, messages] of map) {
    // Sort messages within thread by date ascending
    messages.sort((a, b) => new Date(a.date) - new Date(b.date));
    const lastMsg = messages[messages.length - 1];
    const participantSet = new Set();
    let unreadCount = 0;
    for (const m of messages) {
      const fromName = m.from_name || m.from || 'Unknown';
      participantSet.add(fromName.split('<')[0].trim());
      if (!(m.is_read || m.read)) unreadCount++;
    }
    threads.push({
      thread_id,
      subject: lastMsg.subject || '(No subject)',
      messages,
      last_date: lastMsg.date,
      participants: [...participantSet],
      unread_count: unreadCount,
      is_expanded: false,
    });
  }

  // Sort threads by most recent message date descending
  threads.sort((a, b) => new Date(b.last_date) - new Date(a.last_date));
  _cachedThreadMails = mails;
  _cachedThreads = threads;
  return threads;
}

// Track expanded threads
const _expandedThreads = new Set();

// Track thread view toggle state
let _threadViewEnabled = false;

/**
 * Render the mail list panel with virtual scrolling.
 * @param {HTMLElement} el  #mail-list element
 * @param {object} state    app state
 * @param {object} actions  callback handlers
 */
export function renderMailList(el, state, actions) {
  _lastState = state;
  _lastActions = actions;

  let html = '';

  // Sync progress bar
  if (state._syncing) {
    const pct = state._syncProgress || 0;
    const cls = pct > 0 ? '' : ' indeterminate';
    html += `
      <div class="sync-progress-bar${cls}" role="progressbar" aria-valuenow="${pct}" aria-valuemin="0" aria-valuemax="100">
        <div class="sync-progress-fill" style="width:${pct}%"></div>
      </div>
    `;
  }

  // Toolbar
  const sortLabel = (SORT_OPTIONS.find(s => s.key === _currentSort) || SORT_OPTIONS[0]).label;
  html += `
    <div class="mail-list-toolbar">
      <input
        type="text"
        class="mail-list-search"
        placeholder="${t('search_emails')}"
        id="ml-search"
        aria-label="${t('search_emails')}"
      />
      <select class="mail-list-sort" id="ml-sort" title="Sort emails" aria-label="Sort emails">
        ${SORT_OPTIONS.map(s => `<option value="${s.key}"${s.key === _currentSort ? ' selected' : ''}>${s.label}</option>`).join('')}
      </select>
      <button class="mail-list-btn${_threadViewEnabled ? ' active' : ''}" id="ml-thread-toggle" title="${_threadViewEnabled ? t('list_view') : t('thread_view')}" aria-label="${t('toggle_thread_view')}" aria-pressed="${_threadViewEnabled}">${_threadViewEnabled ? '\u2261' : '\u2630'}</button>
      <button class="mail-list-btn" id="ml-refresh" title="${t('refresh')}" aria-label="${t('refresh_emails')}">\u21BB</button>
      <button class="mail-list-btn primary" id="ml-compose" title="${t('compose_new_email')}" aria-label="${t('compose_new_email')}">${t('new_email')}</button>
    </div>
  `;

  // Batch action bar (multi-select)
  if (_selectedIds.size > 0) {
    html += `
      <div class="batch-action-bar" role="toolbar" aria-label="Batch actions">
        <span class="batch-count">${_selectedIds.size} selected</span>
        <button class="batch-btn" data-batch="archive" title="Archive selected">\uD83D\uDCE6 Archive</button>
        <button class="batch-btn" data-batch="delete" title="Delete selected">\uD83D\uDDD1 Delete</button>
        <button class="batch-btn" data-batch="mark-read" title="Mark selected as read">\u2709 Mark Read</button>
        <button class="batch-btn" data-batch="move" title="Move selected">Move to\u2026</button>
        <button class="batch-btn" data-batch="clear" title="Clear selection">\u2717 Clear</button>
      </div>
    `;
  }

  // Error state
  if (state._mailsError) {
    html += `
      <div class="mail-list-error" role="alert">
        <span class="error-icon">\u2717</span>
        <span>${t('failed_to_load_emails')}</span>
        <button class="retry-btn" id="ml-retry" aria-label="${t('retry')}">${t('retry')}</button>
      </div>
    `;
  }
  // Loading state
  else if (state.loading && state.mails.length === 0) {
    html += '<div class="mail-list-items" role="listbox" aria-label="Email list">';
    for (let i = 0; i < 8; i++) {
      html += `
        <div class="mail-item skeleton" aria-hidden="true">
          <div class="skeleton-line" style="width:60%"></div>
          <div class="skeleton-line" style="width:80%"></div>
          <div class="skeleton-line" style="width:40%"></div>
        </div>
      `;
    }
    html += '</div>';
  } else if (state.mails.length === 0) {
    html += `<div class="mail-list-empty" role="status">
      <div class="empty-state-illustration">${EMPTY_ENVELOPE_SVG}</div>
      <div class="empty-state-text">No emails yet</div>
    </div>`;
  } else {
    // Virtual scroll container
    html += '<div class="mail-list-items" id="ml-virtual-container" role="listbox" aria-label="Email list" style="overflow-y:auto;position:relative;">';
    html += '<div id="ml-virtual-content"></div>';
    html += '</div>';

    // Load more
    if (state.hasMore) {
      html += `
        <div class="mail-list-loadmore">
          <button id="ml-loadmore" ${state.loading ? 'disabled' : ''} aria-label="${t('load_more')}">
            ${state.loading ? t('loading') : t('load_more')}
          </button>
        </div>
      `;
    }
  }

  // Preserve scroll position — save current scroll to folder map before replacing DOM
  const oldContainer = el.querySelector('#ml-virtual-container');
  const prevFolder = el.dataset.currentFolder || '';
  if (oldContainer && prevFolder) {
    _folderScrollPositions.set(prevFolder, oldContainer.scrollTop);
    // Cap to max 50 folders to prevent unbounded memory growth
    if (_folderScrollPositions.size > 50) {
      const oldest = _folderScrollPositions.keys().next().value;
      _folderScrollPositions.delete(oldest);
    }
  }

  el.innerHTML = html;
  // Track which folder this render is for
  el.dataset.currentFolder = state.activeFolder || '';

  // Virtual scroll
  const container = el.querySelector('#ml-virtual-container');
  if (container) {
    // Restore scroll: use folder-keyed position if returning to a folder,
    // otherwise fall back to the previous same-render scroll position
    const folderScroll = _folderScrollPositions.get(state.activeFolder || '');
    const savedScroll = (state.activeFolder && state.activeFolder === prevFolder && oldContainer)
      ? (oldContainer ? oldContainer.scrollTop : 0)
      : (folderScroll || 0);
    container.scrollTop = savedScroll;
    if (_threadViewEnabled) {
      renderThreadedItems(container, state, actions);
    } else {
      renderVisibleItems(container, state, actions);
    }

    if (_scrollListener) container.removeEventListener('scroll', _scrollListener);
    _scrollListener = () => {
      if (_scrollRAF) cancelAnimationFrame(_scrollRAF);
      _scrollRAF = requestAnimationFrame(() => {
        if (_threadViewEnabled) {
          renderThreadedItems(container, _lastState, _lastActions);
        } else {
          renderVisibleItems(container, _lastState, _lastActions);
        }
      });
    };
    container.addEventListener('scroll', _scrollListener);
  }

  // Event: thread toggle
  const threadToggle = el.querySelector('#ml-thread-toggle');
  if (threadToggle) {
    threadToggle.addEventListener('click', () => {
      _threadViewEnabled = !_threadViewEnabled;
      renderMailList(el, _lastState, _lastActions);
    });
  }

  // Event: search
  const searchInput = el.querySelector('#ml-search');
  if (searchInput) {
    let debounce = null;
    let _searchAbortController = null;
    searchInput.addEventListener('input', () => {
      clearTimeout(debounce);
      // Cancel previous in-flight search
      if (_searchAbortController) {
        _searchAbortController.abort();
        _searchAbortController = null;
      }
      const query = searchInput.value.trim();
      // Show "Searching..." indicator in the mail list while debouncing
      const itemsContainer = el.querySelector('.mail-list-items');
      if (itemsContainer && query) {
        itemsContainer.innerHTML = '<div class="mail-list-loading"><div class="spinner"></div> Searching\u2026</div>';
      }
      debounce = setTimeout(() => {
        _searchAbortController = new AbortController();
        if (actions.onSearch) actions.onSearch(query);
      }, 500);
    });
  }

  // Event: refresh
  const refreshBtn = el.querySelector('#ml-refresh');
  if (refreshBtn && actions.onRefresh) {
    refreshBtn.addEventListener('click', actions.onRefresh);
  }

  // Event: compose
  const composeBtn = el.querySelector('#ml-compose');
  if (composeBtn && actions.onCompose) {
    composeBtn.addEventListener('click', actions.onCompose);
  }

  // Event: load more
  const loadMoreBtn = el.querySelector('#ml-loadmore');
  if (loadMoreBtn && actions.onLoadMore) {
    loadMoreBtn.addEventListener('click', actions.onLoadMore);
  }

  // Event: retry on error
  const retryBtn = el.querySelector('#ml-retry');
  if (retryBtn && actions.onRefresh) {
    retryBtn.addEventListener('click', actions.onRefresh);
  }

  // Event: sort dropdown
  const sortSelect = el.querySelector('#ml-sort');
  if (sortSelect) {
    sortSelect.addEventListener('change', () => {
      _currentSort = sortSelect.value;
      localStorage.setItem('exospine_mail_sort', _currentSort);
      renderMailList(el, _lastState, _lastActions);
    });
  }

  // Event: batch actions
  el.querySelectorAll('.batch-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const action = btn.dataset.batch;
      const ids = new Set(_selectedIds);
      if (action === 'archive' && actions.onBatchArchive) {
        actions.onBatchArchive(ids);
        _selectedIds.clear();
        renderMailList(el, _lastState, _lastActions);
      } else if (action === 'delete' && actions.onBatchDelete) {
        actions.onBatchDelete(ids);
        _selectedIds.clear();
        renderMailList(el, _lastState, _lastActions);
      } else if (action === 'mark-read' && actions.onBatchMarkRead) {
        actions.onBatchMarkRead(ids);
        _selectedIds.clear();
        renderMailList(el, _lastState, _lastActions);
      } else if (action === 'move') {
        const target = prompt('Move to folder:');
        if (target && actions.onBatchMove) {
          actions.onBatchMove(ids, target);
          _selectedIds.clear();
          renderMailList(el, _lastState, _lastActions);
        }
      } else if (action === 'clear') {
        _selectedIds.clear();
        renderMailList(el, _lastState, _lastActions);
      }
    });
  });
}

// ── Render helpers: category badges + spam indicator ───────────────

function renderCategoryBadges(mail) {
  const cats = mail.categories || [];
  if (cats.length === 0) return '';
  return cats
    .map(
      (c) =>
        `<span class="mail-category-badge" style="background:${getCategoryColor(c)}" title="${esc(c)}">${esc(c)}</span>`
    )
    .join('');
}

function renderSpamIndicator(mail) {
  const score = mail.spam_score || 0;
  if (score > 3.0) {
    return '<span class="mail-spam-indicator" title="Likely spam">\u26A0</span>';
  }
  if (score > 1.5) {
    return '<span class="mail-spam-indicator mild" title="Possibly spam">\u26A0</span>';
  }
  return '';
}

// ── Flat list rendering (original) ─────────────────────────────────

function renderVisibleItems(container, state, actions) {
  const contentEl = container.querySelector('#ml-virtual-content');
  if (!contentEl) return;

  // Cache display rules once per render call
  const displayRules = loadDisplayRules();

  // Apply user sort first, then pin to top
  const userSorted = sortMails(state.mails, _currentSort);
  const sortedMails = [...userSorted].sort((a, b) => {
    const aPinned = a.is_pinned ? 1 : 0;
    const bPinned = b.is_pinned ? 1 : 0;
    return bPinned - aPinned;
  });

  // Filter out snoozed emails (snoozed_until is in the future)
  const now = new Date().toISOString();
  const visibleMails = sortedMails.filter(
    (m) => !m.snoozed_until || m.snoozed_until <= now
  );

  const totalCount = visibleMails.length;
  const scrollTop = container.scrollTop;
  const viewportHeight = container.clientHeight;

  const startIdx = Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - 2);
  const endIdx = Math.min(totalCount, Math.ceil((scrollTop + viewportHeight) / ITEM_HEIGHT) + 2);

  const topSpacer = startIdx * ITEM_HEIGHT;
  const bottomSpacer = Math.max(0, (totalCount - endIdx) * ITEM_HEIGHT);

  let html = '';
  html += `<div style="height:${topSpacer}px;" aria-hidden="true"></div>`;

  for (let i = startIdx; i < endIdx; i++) {
    html += renderMailItem(visibleMails[i], state, false, displayRules);
  }

  html += `<div style="height:${bottomSpacer}px;" aria-hidden="true"></div>`;

  contentEl.innerHTML = html;
  attachItemEvents(contentEl, state, actions);
}

// ── Thread view rendering ──────────────────────────────────────────

function renderThreadedItems(container, state, actions) {
  const contentEl = container.querySelector('#ml-virtual-content');
  if (!contentEl) return;

  // Cache display rules once per render call
  const displayRules = loadDisplayRules();

  const threads = groupByThread(state.mails);

  // Build a flat list of "rows" for virtual scrolling
  const rows = [];
  for (const thread of threads) {
    if (thread.messages.length === 1) {
      // Single-message thread: render as a normal mail item
      rows.push({ type: 'mail', mail: thread.messages[0] });
    } else {
      const expanded = _expandedThreads.has(thread.thread_id);
      rows.push({ type: 'thread-header', thread, expanded });
      if (expanded) {
        for (const msg of thread.messages) {
          rows.push({ type: 'thread-child', mail: msg, thread_id: thread.thread_id });
        }
      }
    }
  }

  const totalHeight = rows.length * ITEM_HEIGHT;
  const scrollTop = container.scrollTop;
  const viewportHeight = container.clientHeight;

  const startIdx = Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - 2);
  const endIdx = Math.min(rows.length, Math.ceil((scrollTop + viewportHeight) / ITEM_HEIGHT) + 2);

  const topSpacer = startIdx * ITEM_HEIGHT;
  const bottomSpacer = Math.max(0, (rows.length - endIdx) * ITEM_HEIGHT);

  let html = '';
  html += `<div style="height:${topSpacer}px;" aria-hidden="true"></div>`;

  for (let i = startIdx; i < endIdx; i++) {
    const row = rows[i];
    if (row.type === 'mail') {
      html += renderMailItem(row.mail, state, false, displayRules);
    } else if (row.type === 'thread-header') {
      html += renderThreadHeader(row.thread, row.expanded);
    } else if (row.type === 'thread-child') {
      html += renderMailItem(row.mail, state, true, displayRules);
    }
  }

  html += `<div style="height:${bottomSpacer}px;" aria-hidden="true"></div>`;

  contentEl.innerHTML = html;

  // Attach delegated events (includes thread header clicks via delegation)
  attachItemEvents(contentEl, state, actions);

  // Thread header expand/collapse via delegation on contentEl
  contentEl.addEventListener('click', (e) => {
    const header = e.target.closest('.thread-header[data-thread-id]');
    if (!header) return;
    if (e.target.closest('.mail-item-star')) return;
    const tid = header.dataset.threadId;
    if (_expandedThreads.has(tid)) {
      _expandedThreads.delete(tid);
    } else {
      _expandedThreads.add(tid);
    }
    renderThreadedItems(container, _lastState, _lastActions);
  });
}

function renderThreadHeader(thread, expanded) {
  const hasUnread = thread.unread_count > 0;
  const participants = thread.participants.slice(0, 3).map(esc).join(', ');
  const extra = thread.participants.length > 3 ? ` +${thread.participants.length - 3}` : '';
  const count = thread.messages.length;
  const lastMail = thread.messages[thread.messages.length - 1];
  const catBadges = renderCategoryBadges(lastMail);
  const spamInd = renderSpamIndicator(lastMail);

  return `
    <div class="thread-header mail-item${hasUnread ? ' unread' : ''}" data-thread-id="${esc(thread.thread_id)}" role="option" tabindex="0" aria-expanded="${expanded}" aria-label="${esc(thread.subject)}, ${count} messages${hasUnread ? ', ' + thread.unread_count + ' unread' : ''}" style="height:${ITEM_HEIGHT}px;box-sizing:border-box;">
      <div class="thread-expand-icon" aria-hidden="true">${expanded ? '\u25BC' : '\u25B6'}</div>
      <div class="mail-item-body">
        <div class="mail-item-top">
          <span class="mail-item-from">${participants}${esc(extra)}</span>
          <span class="thread-count">(${count})</span>
          <span class="mail-item-date">${formatDate(thread.last_date)}</span>
        </div>
        <div class="mail-item-subject">${spamInd}${esc(thread.subject)}${catBadges ? ' ' + catBadges : ''}</div>
        <div class="mail-item-preview">${thread.unread_count > 0 ? `${thread.unread_count} unread` : esc(lastMail.preview || '')}</div>
      </div>
    </div>
  `;
}

// ── Sender avatar ──────────────────────────────────────────────────

function senderAvatar(from) {
  const name = (from || '').split('<')[0].trim() || from || '';
  const initial = name[0]?.toUpperCase() || '?';
  const colors = ['#e74c3c','#3498db','#2ecc71','#f39c12','#9b59b6','#1abc9c','#e67e22','#34495e'];
  let hash = 0;
  for (const c of (from || '')) hash = (hash * 31 + c.charCodeAt(0)) & 0xffffffff;
  const color = colors[Math.abs(hash) % colors.length];
  return `<div class="mail-avatar" style="background:${color}">${esc(initial)}</div>`;
}

// ── Single mail item rendering ─────────────────────────────────────

function renderImportanceIcon(mail) {
  const imp = mail.importance || 'normal';
  if (imp === 'high') return '<span class="mail-importance-icon high" title="High importance">\u2757</span>';
  if (imp === 'low') return '<span class="mail-importance-icon low" title="Low importance">\u2193</span>';
  return '';
}

function renderFlagIcon(mail) {
  const flagged = !!mail.flag_due_date;
  if (!flagged) return `<div class="mail-item-flag" data-flag-id="${esc(mail.id)}" title="Flag for follow-up" role="button" tabindex="0" style="cursor:pointer;font-size:13px;padding:2px 4px;opacity:0.25;">\u2691</div>`;
  // Color: red if overdue, orange if due today, default blue
  const now = new Date().toISOString().slice(0, 10);
  const due = mail.flag_due_date.slice(0, 10);
  let color = 'var(--accent)';
  if (due < now) color = 'var(--danger)';
  else if (due === now) color = 'var(--warning)';
  return `<div class="mail-item-flag flagged" data-flag-id="${esc(mail.id)}" title="Flagged: due ${esc(mail.flag_due_date)}" role="button" tabindex="0" style="cursor:pointer;font-size:13px;padding:2px 4px;color:${color};">\u2691</div>`;
}

function renderMailItem(mail, state, isThreadChild = false, cachedRules = null) {
  const isSelected = state.selectedMail && state.selectedMail.id === mail.id;
  const isMultiSelected = _selectedIds.has(mail.id);
  const isUnread = !(mail.is_read || mail.read);
  const isStarred = mail.is_starred || mail.starred;
  const isPinned = mail.is_pinned || false;
  const catBadges = renderCategoryBadges(mail);
  const spamInd = renderSpamIndicator(mail);
  const importanceInd = renderImportanceIcon(mail);
  const flagIcon = renderFlagIcon(mail);
  const indent = isThreadChild ? ' thread-child' : '';
  // Account badge for unified inbox view
  const accountBadge = mail._accountEmail
    ? `<span class="mail-item-account-badge" title="${esc(mail._accountEmail)}">${esc((mail._accountName || mail._accountEmail || '')[0] || '?')}</span>`
    : '';

  // Apply conditional display rules (use pre-cached rules if provided)
  const displayStyle = cachedRules !== null ? getDisplayRuleStyleWith(mail, cachedRules) : getDisplayRuleStyle(mail);

  // Multi-select checkbox (visible when multi-select is active or on hover via CSS)
  const checkboxVisible = _selectedIds.size > 0;
  const checkbox = `<input type="checkbox" class="mail-item-checkbox${checkboxVisible ? ' visible' : ''}" data-check-id="${esc(mail.id)}" ${isMultiSelected ? 'checked' : ''} tabindex="-1" aria-label="Select email" />`;

  return `
    <div class="mail-item${isSelected ? ' selected' : ''}${isUnread ? ' unread' : ''}${isPinned ? ' pinned' : ''}${isMultiSelected ? ' multi-selected' : ''}${indent}" data-mail-id="${esc(mail.id)}" draggable="true" role="option" aria-selected="${isSelected}" tabindex="0" style="height:${ITEM_HEIGHT}px;box-sizing:border-box;${displayStyle}">
      ${checkbox}
      ${senderAvatar(mail.from_name || mail.from || 'Unknown')}
      ${isUnread ? '<div class="mail-item-unread-dot" aria-hidden="true"></div>' : ''}
      ${importanceInd}
      <div class="mail-item-pin${isPinned ? ' pinned' : ''}" data-pin-id="${esc(mail.id)}" title="${isPinned ? 'Unpin' : 'Pin to top'}" role="button" aria-label="${isPinned ? 'Unpin' : 'Pin'} email" tabindex="0" style="cursor:pointer;font-size:14px;padding:2px 4px;opacity:${isPinned ? '1' : '0.3'};">
        \uD83D\uDCCC
      </div>
      <div class="mail-item-star${isStarred ? ' starred' : ''}" data-star-id="${esc(mail.id)}" title="Toggle star" role="button" aria-label="${isStarred ? 'Unstar' : 'Star'} email" tabindex="0">
        ${isStarred ? '\u2605' : '\u2606'}
      </div>
      ${flagIcon}
      <div class="mail-item-body">
        <div class="mail-item-top">
          ${accountBadge}
          <span class="mail-item-from">${esc(mail.from_name || mail.from || 'Unknown')}</span>
          <span class="mail-item-date">${formatDate(mail.date)}</span>
        </div>
        <div class="mail-item-subject">${spamInd}${esc(mail.subject || '(No subject)')}${catBadges ? ' ' + catBadges : ''}</div>
        <div class="mail-item-preview">${esc(mail.preview || '')}</div>
      </div>
      <div class="mail-item-quick-actions" aria-label="Quick actions">
        <button class="quick-action-btn" data-quick="archive" data-quick-id="${esc(mail.id)}" title="Archive" aria-label="Archive">\uD83D\uDCE6</button>
        <button class="quick-action-btn" data-quick="delete" data-quick-id="${esc(mail.id)}" title="Delete" aria-label="Delete">\uD83D\uDDD1</button>
        <button class="quick-action-btn" data-quick="star" data-quick-id="${esc(mail.id)}" title="Star" aria-label="Star">${isStarred ? '\u2605' : '\u2606'}</button>
      </div>
    </div>
  `;
}

// ── Event delegation for mail items (single listener on contentEl) ──

function attachItemEvents(contentEl, state, actions) {
  // Use event delegation: one click/keydown/contextmenu/dragstart/dragend listener
  // on the contentEl instead of N listeners on each item.

  contentEl.addEventListener('click', (e) => {
    // Checkbox toggle (multi-select)
    const checkbox = e.target.closest('.mail-item-checkbox');
    if (checkbox) {
      e.stopPropagation();
      const mailId = checkbox.dataset.checkId;
      if (_selectedIds.has(mailId)) {
        _selectedIds.delete(mailId);
      } else {
        _selectedIds.add(mailId);
      }
      renderMailList(contentEl.closest('#mail-list'), _lastState, _lastActions);
      return;
    }

    // Quick action buttons
    const quickBtn = e.target.closest('.quick-action-btn');
    if (quickBtn) {
      e.stopPropagation();
      const quickAction = quickBtn.dataset.quick;
      const mailId = quickBtn.dataset.quickId;
      const mail = state.mails.find((m) => m.id === mailId);
      if (!mail) return;
      if (quickAction === 'archive' && actions.onArchive) actions.onArchive(mail);
      else if (quickAction === 'delete' && actions.onDelete) actions.onDelete(mail);
      else if (quickAction === 'star' && actions.onToggleStar) actions.onToggleStar(mailId);
      return;
    }

    // Star toggle
    const star = e.target.closest('.mail-item-star');
    if (star) {
      e.stopPropagation();
      const mailId = star.dataset.starId;
      if (actions.onToggleStar) actions.onToggleStar(mailId);
      return;
    }

    // Pin toggle
    const pin = e.target.closest('.mail-item-pin');
    if (pin) {
      e.stopPropagation();
      const mailId = pin.dataset.pinId;
      if (actions.onTogglePin) actions.onTogglePin(mailId);
      return;
    }

    // Flag toggle
    const flag = e.target.closest('.mail-item-flag');
    if (flag) {
      e.stopPropagation();
      const mailId = flag.dataset.flagId;
      const mail = state.mails.find((m) => m.id === mailId);
      if (!mail) return;
      if (mail.flag_due_date) {
        if (actions.onUnflagMail) actions.onUnflagMail(mailId);
      } else {
        const rect = flag.getBoundingClientRect();
        showFlagMenu(rect.left, rect.bottom, mailId, actions);
      }
      return;
    }

    // Mail item select (Ctrl+Click = multi-select toggle)
    const item = e.target.closest('.mail-item[data-mail-id]');
    if (item) {
      const mailId = item.dataset.mailId;
      if (e.ctrlKey || e.metaKey) {
        // Toggle multi-select
        if (_selectedIds.has(mailId)) {
          _selectedIds.delete(mailId);
        } else {
          _selectedIds.add(mailId);
        }
        renderMailList(contentEl.closest('#mail-list'), _lastState, _lastActions);
        return;
      }
      if (actions.onSelect) actions.onSelect(mailId);
    }
  });

  contentEl.addEventListener('keydown', (e) => {
    const star = e.target.closest('.mail-item-star');
    if (star && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      e.stopPropagation();
      const mailId = star.dataset.starId;
      if (actions.onToggleStar) actions.onToggleStar(mailId);
      return;
    }

    const pin = e.target.closest('.mail-item-pin');
    if (pin && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      e.stopPropagation();
      const mailId = pin.dataset.pinId;
      if (actions.onTogglePin) actions.onTogglePin(mailId);
      return;
    }

    const flag = e.target.closest('.mail-item-flag');
    if (flag && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      e.stopPropagation();
      const mailId = flag.dataset.flagId;
      const mail = state.mails.find((m) => m.id === mailId);
      if (!mail) return;
      if (mail.flag_due_date) {
        if (actions.onUnflagMail) actions.onUnflagMail(mailId);
      } else {
        const rect = flag.getBoundingClientRect();
        showFlagMenu(rect.left, rect.bottom, mailId, actions);
      }
      return;
    }

    const item = e.target.closest('.mail-item[data-mail-id]');
    if (item && e.key === 'Enter') {
      e.preventDefault();
      if (actions.onSelect) actions.onSelect(item.dataset.mailId);
    }
  });

  contentEl.addEventListener('contextmenu', (e) => {
    const item = e.target.closest('.mail-item[data-mail-id]');
    if (!item) return;
    e.preventDefault();
    const mailId = item.dataset.mailId;
    const mail = state.mails.find((m) => m.id === mailId);
    if (!mail) return;
    const isRead = mail.is_read || mail.read;
    const isStarred = mail.is_starred || mail.starred;
    const isSpam = (mail.categories || []).includes('Spam');

    const menuItems = [
      { label: '\u21A9 Reply', action: () => { if (actions.onReply) actions.onReply(mail); } },
      { label: '\u21A9\u21A9 Reply All', action: () => { if (actions.onReplyAll) actions.onReplyAll(mail); } },
      { label: '\u21AA Forward', action: () => { if (actions.onForward) actions.onForward(mail); } },
      { separator: true },
      {
        label: isRead ? '\u2709 Mark Unread' : '\u2709 Mark Read',
        action: () => {
          if (isRead && actions.onMarkUnread) actions.onMarkUnread(mail);
          else if (!isRead && actions.onMarkRead) actions.onMarkRead(mail);
        },
      },
      {
        label: isStarred ? '\u2606 Unstar' : '\u2605 Star',
        action: () => { if (actions.onToggleStar) actions.onToggleStar(mailId); },
      },
      { separator: true },
      {
        label: isSpam ? '\u2714 Not Spam' : '\u26A0 Report Spam',
        action: () => {
          if (isSpam && actions.onReportNotSpam) actions.onReportNotSpam(mail);
          else if (!isSpam && actions.onReportSpam) actions.onReportSpam(mail);
        },
      },
      {
        label: '\uD83C\uDFF7 Categorize...',
        action: () => {
          showCategoryMenu(e.clientX, e.clientY, mail, actions);
        },
      },
      { separator: true },
      { label: '\uD83D\uDCE6 Archive', action: () => { if (actions.onArchive) actions.onArchive(mail); } },
      { label: '\uD83D\uDDD1 Delete', action: () => { if (actions.onDelete) actions.onDelete(mail); }, danger: true },
      { separator: true },
      {
        label: '\uD83E\uDDF9 Sweep: Delete all from this sender',
        action: () => { if (actions.onSweepSender) actions.onSweepSender(mail); },
        danger: true,
      },
    ];

    showContextMenu(e.clientX, e.clientY, menuItems);
  });

  contentEl.addEventListener('dragstart', (e) => {
    const item = e.target.closest('.mail-item[data-mail-id]');
    if (!item) return;
    const mailId = item.dataset.mailId;
    const mail = state.mails.find((m) => m.id === mailId);
    if (!mail) return;
    e.dataTransfer.setData('application/x-exospine-mail', JSON.stringify({
      mailId: mail.id,
      accountId: mail.account_id,
      folder: mail.folder || state.activeFolder,
    }));
    e.dataTransfer.effectAllowed = 'move';
    item.classList.add('dragging');
  });

  contentEl.addEventListener('dragend', (e) => {
    const item = e.target.closest('.mail-item[data-mail-id]');
    if (item) item.classList.remove('dragging');
  });
}

// ── Category picker menu ───────────────────────────────────────────

function showCategoryMenu(x, y, mail, actions) {
  const currentCats = mail.categories || [];
  const menuItems = CATEGORIES.map((cat) => {
    const has = currentCats.includes(cat.name);
    return {
      label: `<span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:${cat.color};margin-right:6px;"></span>${has ? '\u2714 ' : ''}${cat.name}`,
      html: true,
      action: () => {
        if (has) {
          if (actions.onRemoveCategory) actions.onRemoveCategory(mail, cat.name);
        } else {
          if (actions.onAddCategory) actions.onAddCategory(mail, cat.name);
        }
      },
    };
  });
  showContextMenu(x, y, menuItems);
}

// ── Flag picker menu ──────────────────────────────────────────────

function showFlagMenu(x, y, mailId, actions) {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).toISOString();
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).toISOString();
  const endOfWeek = new Date(now.getFullYear(), now.getMonth(), now.getDate() + (7 - now.getDay())).toISOString();

  const menuItems = [
    { label: '\u2691 Today', action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, today); } },
    { label: '\u2691 Tomorrow', action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, tomorrow); } },
    { label: '\u2691 This week', action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, endOfWeek); } },
    { separator: true },
    {
      label: '\u2691 Pick date...',
      action: () => {
        const dateStr = prompt('Enter due date (YYYY-MM-DD):');
        if (dateStr) {
          const parsed = new Date(dateStr);
          if (!isNaN(parsed.getTime())) {
            if (actions.onFlagMail) actions.onFlagMail(mailId, parsed.toISOString());
          }
        }
      },
    },
  ];
  showContextMenu(x, y, menuItems);
}

// ── Conditional Display Rules ────────────────────────────────────

// Cache display rules to avoid reading localStorage on every render
let _displayRulesCache = null;
let _displayRulesCacheTime = 0;

/** Apply display rules using the time-based cache (for backward compat / single-item calls). */
function getDisplayRuleStyle(mail) {
  // Refresh cache every 5 seconds
  const now = Date.now();
  if (!_displayRulesCache || now - _displayRulesCacheTime > 5000) {
    _displayRulesCache = loadDisplayRules();
    _displayRulesCacheTime = now;
  }
  return getDisplayRuleStyleWith(mail, _displayRulesCache);
}

/** Apply display rules from a pre-loaded rules array (avoids per-item cache lookup). */
function getDisplayRuleStyleWith(mail, rules) {
  if (!rules || rules.length === 0) return '';

  const styles = [];
  for (const rule of rules) {
    let fieldValue = '';
    if (rule.field === 'from') {
      fieldValue = (mail.from_name || mail.from || '').toLowerCase();
    } else if (rule.field === 'subject') {
      fieldValue = (mail.subject || '').toLowerCase();
    }

    if (fieldValue.includes((rule.value || '').toLowerCase())) {
      if (rule.style === 'highlight') {
        styles.push(`background-color:${rule.color}22;border-left:3px solid ${rule.color}`);
      } else if (rule.style === 'bold') {
        styles.push('font-weight:700');
      } else if (rule.style === 'italic') {
        styles.push('font-style:italic');
      }
    }
  }

  return styles.join(';');
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
