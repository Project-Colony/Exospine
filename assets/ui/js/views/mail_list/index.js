// Exospine: the mail list, with virtual scrolling, thread view, sorting and duplicate detection.

import * as api from '../../api.js';
import { showToast } from '../../components/toast.js';
import { t } from '../../i18n.js';
import { attachItemEvents } from './events.js';
import { renderMailItem, renderThreadHeader } from './items.js';
import { sortMails } from './sort.js';
import { list, SORT_OPTIONS } from './state.js';
import { groupByThread } from './threads.js';
import { loadDisplayRules } from '../settings.js';

const ITEM_HEIGHT = 80; // px per mail item
let _scrollRAF = null;
let _scrollListener = null;

// ── Empty state SVG illustration ────────────────────────────────
const EMPTY_ENVELOPE_SVG = `<svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="10" y="20" width="60" height="44" rx="6" stroke="currentColor" stroke-width="2" fill="none"/>
  <path d="M10 26 L40 46 L70 26" stroke="currentColor" stroke-width="2" fill="none"/>
</svg>`;

// ── Scroll position preservation per folder ────────────────────────
const _folderScrollPositions = new Map();

/**
 * Render the mail list panel with virtual scrolling.
 * @param {HTMLElement} el  #mail-list element
 * @param {object} state    app state
 * @param {object} actions  callback handlers
 */
export function renderMailList(el, state, actions) {
  list.lastState = state;
  list.lastActions = actions;

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
  const sortLabel = (SORT_OPTIONS.find(s => s.key === list.currentSort) || SORT_OPTIONS[0]).label;
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
        ${SORT_OPTIONS.map(s => `<option value="${s.key}"${s.key === list.currentSort ? ' selected' : ''}>${s.label}</option>`).join('')}
      </select>
      <button class="mail-list-btn${list.threadViewEnabled ? ' active' : ''}" id="ml-thread-toggle" title="${list.threadViewEnabled ? t('list_view') : t('thread_view')}" aria-label="${t('toggle_thread_view')}" aria-pressed="${list.threadViewEnabled}">${list.threadViewEnabled ? '\u2261' : '\u2630'}</button>
      <button class="mail-list-btn${list.duplicateMode ? ' active' : ''}" id="ml-find-duplicates" title="Find duplicates" aria-label="Find duplicate emails">\uD83D\uDD0D Dupes</button>
      <button class="mail-list-btn" id="ml-refresh" title="${t('refresh')}" aria-label="${t('refresh_emails')}">\u21BB</button>
      <button class="mail-list-btn primary" id="ml-compose" title="${t('compose_new_email')}" aria-label="${t('compose_new_email')}">${t('new_email')}</button>
    </div>
  `;

  // Duplicate action bar
  if (list.duplicateMode && list.duplicatePairs.length > 0) {
    html += `
      <div class="batch-action-bar duplicate-action-bar" role="toolbar" aria-label="Duplicate actions">
        <span class="batch-count">${list.duplicatePairs.length} duplicate${list.duplicatePairs.length !== 1 ? 's' : ''} found</span>
        <button class="batch-btn danger" id="ml-delete-duplicates" title="Delete all duplicates (keep newest)">\uD83D\uDDD1 Delete duplicates</button>
        <button class="batch-btn" id="ml-dismiss-duplicates" title="Dismiss">\u2717 Dismiss</button>
      </div>
    `;
  }

  // Batch action bar (multi-select)
  if (list.selectedIds.size > 0) {
    html += `
      <div class="batch-action-bar" role="toolbar" aria-label="Batch actions">
        <span class="batch-count">${list.selectedIds.size} selected</span>
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
          <div class="skeleton-line skeleton-line-60"></div>
          <div class="skeleton-line skeleton-line-80"></div>
          <div class="skeleton-line skeleton-line-40"></div>
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
    html += '<div class="mail-list-items" id="ml-virtual-container" role="listbox" aria-label="Email list">';
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
    if (list.threadViewEnabled) {
      renderThreadedItems(container, state, actions);
    } else {
      renderVisibleItems(container, state, actions);
    }

    if (_scrollListener) container.removeEventListener('scroll', _scrollListener);
    _scrollListener = () => {
      if (_scrollRAF) cancelAnimationFrame(_scrollRAF);
      _scrollRAF = requestAnimationFrame(() => {
        if (list.threadViewEnabled) {
          renderThreadedItems(container, list.lastState, list.lastActions);
        } else {
          renderVisibleItems(container, list.lastState, list.lastActions);
        }
      });
    };
    container.addEventListener('scroll', _scrollListener);
  }

  // Event: thread toggle
  const threadToggle = el.querySelector('#ml-thread-toggle');
  if (threadToggle) {
    threadToggle.addEventListener('click', () => {
      list.threadViewEnabled = !list.threadViewEnabled;
      renderMailList(el, list.lastState, list.lastActions);
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

  // Event: delete duplicates
  const deleteDupesBtn = el.querySelector('#ml-delete-duplicates');
  if (deleteDupesBtn) {
    deleteDupesBtn.addEventListener('click', async () => {
      if (!confirm(`Delete ${list.duplicatePairs.length} duplicate email(s)? This keeps the newest copy.`)) return;
      let deleted = 0;
      for (const pair of list.duplicatePairs) {
        const mail = state.mails.find(m => m.id === pair.duplicate_id);
        if (mail && actions.onDelete) {
          try {
            actions.onDelete(mail);
            deleted++;
          } catch {}
        }
      }
      list.duplicateMode = false;
      list.duplicateIds.clear();
      list.duplicatePairs = [];
      showToast(`Deleted ${deleted} duplicate(s).`, 'success');
      renderMailList(el, list.lastState, list.lastActions);
    });
  }
  const dismissDupesBtn = el.querySelector('#ml-dismiss-duplicates');
  if (dismissDupesBtn) {
    dismissDupesBtn.addEventListener('click', () => {
      list.duplicateMode = false;
      list.duplicateIds.clear();
      list.duplicatePairs = [];
      renderMailList(el, list.lastState, list.lastActions);
    });
  }

  // Event: find duplicates
  const dupeBtn = el.querySelector('#ml-find-duplicates');
  if (dupeBtn) {
    dupeBtn.addEventListener('click', async () => {
      if (list.duplicateMode) {
        // Toggle off
        list.duplicateMode = false;
        list.duplicateIds.clear();
        list.duplicatePairs = [];
        renderMailList(el, list.lastState, list.lastActions);
        return;
      }
      const accountId = state.activeAccountId;
      const folder = state.activeFolder;
      if (!accountId || !folder) {
        showToast('Select an account and folder first.', 'error');
        return;
      }
      try {
        dupeBtn.disabled = true;
        dupeBtn.textContent = 'Scanning...';
        const pairs = await api.findDuplicates(accountId, folder);
        list.duplicatePairs = pairs;
        list.duplicateIds = new Set();
        for (const p of pairs) {
          list.duplicateIds.add(p.duplicate_id);
        }
        list.duplicateMode = true;
        if (pairs.length === 0) {
          showToast('No duplicates found.', 'info');
        } else {
          showToast(`Found ${pairs.length} duplicate${pairs.length !== 1 ? 's' : ''}.`, 'info');
        }
      } catch (err) {
        showToast('Failed to find duplicates: ' + err, 'error');
      }
      renderMailList(el, list.lastState, list.lastActions);
    });
  }

  // Event: sort dropdown
  const sortSelect = el.querySelector('#ml-sort');
  if (sortSelect) {
    sortSelect.addEventListener('change', () => {
      list.currentSort = sortSelect.value;
      localStorage.setItem('exospine_mail_sort', list.currentSort);
      renderMailList(el, list.lastState, list.lastActions);
    });
  }

  // Event: batch actions
  el.querySelectorAll('.batch-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const action = btn.dataset.batch;
      const ids = new Set(list.selectedIds);
      if (action === 'archive' && actions.onBatchArchive) {
        actions.onBatchArchive(ids);
        list.selectedIds.clear();
        renderMailList(el, list.lastState, list.lastActions);
      } else if (action === 'delete' && actions.onBatchDelete) {
        actions.onBatchDelete(ids);
        list.selectedIds.clear();
        renderMailList(el, list.lastState, list.lastActions);
      } else if (action === 'mark-read' && actions.onBatchMarkRead) {
        actions.onBatchMarkRead(ids);
        list.selectedIds.clear();
        renderMailList(el, list.lastState, list.lastActions);
      } else if (action === 'move') {
        const target = prompt('Move to folder:');
        if (target && actions.onBatchMove) {
          actions.onBatchMove(ids, target);
          list.selectedIds.clear();
          renderMailList(el, list.lastState, list.lastActions);
        }
      } else if (action === 'clear') {
        list.selectedIds.clear();
        renderMailList(el, list.lastState, list.lastActions);
      }
    });
  });
}

function renderVisibleItems(container, state, actions) {
  const contentEl = container.querySelector('#ml-virtual-content');
  if (!contentEl) return;

  // Cache display rules once per render call
  const displayRules = loadDisplayRules();

  // Sort once: pinned first, then by user sort order
  const sortedMails = sortMails(state.mails, list.currentSort).sort((a, b) => {
    const pinDiff = (b.is_pinned ? 1 : 0) - (a.is_pinned ? 1 : 0);
    return pinDiff;
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

  // DOM reuse: if the visible item count matches existing children (minus 2 spacers),
  // update content in-place instead of full innerHTML replacement.
  const existingChildren = contentEl.children;
  const visibleCount = endIdx - startIdx;
  const canReuse = existingChildren.length === visibleCount + 2
    && existingChildren[0]?.getAttribute('aria-hidden') === 'true';

  if (canReuse) {
    // Update spacer heights
    existingChildren[0].style.height = topSpacer + 'px';
    existingChildren[existingChildren.length - 1].style.height = bottomSpacer + 'px';
    // Update each item in-place
    for (let i = 0; i < visibleCount; i++) {
      const newHtml = renderMailItem(visibleMails[startIdx + i], state, false, displayRules);
      const child = existingChildren[i + 1];
      // Only update if content changed (avoid unnecessary reflow)
      if (child._mailId !== visibleMails[startIdx + i].id) {
        child.outerHTML = newHtml;
      }
    }
    // Re-tag mail IDs for next comparison
    for (let i = 0; i < visibleCount; i++) {
      if (existingChildren[i + 1]) {
        existingChildren[i + 1]._mailId = visibleMails[startIdx + i].id;
      }
    }
    attachItemEvents(contentEl, state, actions, () => renderMailList(contentEl.closest('#mail-list'), list.lastState, list.lastActions));
  } else {
    let html = '';
    html += `<div style="height:${topSpacer}px;" aria-hidden="true"></div>`;

    for (let i = startIdx; i < endIdx; i++) {
      html += renderMailItem(visibleMails[i], state, false, displayRules);
    }

    html += `<div style="height:${bottomSpacer}px;" aria-hidden="true"></div>`;

    contentEl.innerHTML = html;
    // Tag mail IDs for DOM reuse on next scroll
    for (let i = 0; i < visibleCount; i++) {
      if (contentEl.children[i + 1]) {
        contentEl.children[i + 1]._mailId = visibleMails[startIdx + i].id;
      }
    }
    attachItemEvents(contentEl, state, actions, () => renderMailList(contentEl.closest('#mail-list'), list.lastState, list.lastActions));
  }
}

function renderThreadedItems(container, state, actions) {
  const contentEl = container.querySelector('#ml-virtual-content');
  if (!contentEl) return;

  // Cache display rules once per render call
  const displayRules = loadDisplayRules();

  const preSorted = sortMails(state.mails, list.currentSort);
  const threads = groupByThread(preSorted);

  // Build a flat list of "rows" for virtual scrolling
  const rows = [];
  for (const thread of threads) {
    if (thread.messages.length === 1) {
      // Single-message thread: render as a normal mail item
      rows.push({ type: 'mail', mail: thread.messages[0] });
    } else {
      const expanded = list.expandedThreads.has(thread.thread_id);
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
  attachItemEvents(contentEl, state, actions, () => renderMailList(contentEl.closest('#mail-list'), list.lastState, list.lastActions));

  // Thread header expand/collapse via delegation on contentEl, attached once
  if (contentEl._threadEvents) return;
  contentEl._threadEvents = true;
  contentEl.addEventListener('click', (e) => {
    const header = e.target.closest('.thread-header[data-thread-id]');
    if (!header) return;
    if (e.target.closest('.mail-item-star')) return;
    const tid = header.dataset.threadId;
    if (list.expandedThreads.has(tid)) {
      list.expandedThreads.delete(tid);
    } else {
      list.expandedThreads.add(tid);
    }
    renderThreadedItems(container, list.lastState, list.lastActions);
  });
}
