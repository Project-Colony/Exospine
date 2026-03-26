// Exospine — Keyboard shortcuts

import { getMergedShortcuts } from './views/settings.js';
import { openCompose } from './views/compose.js';
import { state, activeAccountId, sidebarEl, mailListEl, mailViewEl } from './state.js';

// These callbacks are injected by app.js at init time
let _handleReply = () => {};
let _handleReplyAll = () => {};
let _handleForward = () => {};
let _handleDelete = () => {};
let _handleArchive = () => {};
let _toggleMailStar = () => {};
let _handleMarkUnread = () => {};
let _selectMail = () => {};
let _refreshCurrentFolder = () => {};
let _toggleFocusMode = () => {};
let _renderView = () => {};
let _updateMailItemSelection = () => {};

export function setShortcutCallbacks(cbs) {
  _handleReply = cbs.handleReply;
  _handleReplyAll = cbs.handleReplyAll;
  _handleForward = cbs.handleForward;
  _handleDelete = cbs.handleDelete;
  _handleArchive = cbs.handleArchive;
  _toggleMailStar = cbs.toggleMailStar;
  _handleMarkUnread = cbs.handleMarkUnread;
  _selectMail = cbs.selectMail;
  _refreshCurrentFolder = cbs.refreshCurrentFolder;
  _toggleFocusMode = cbs.toggleFocusMode;
  _renderView = cbs.renderView;
  _updateMailItemSelection = cbs.updateMailItemSelection;
}

export function setupKeyboardShortcuts() {
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
      _toggleFocusMode();
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
        openCompose({ accountId: activeAccountId() }, () => _refreshCurrentFolder());
      },
      [shortcuts.reply]: () => { if (state.selectedMail) _handleReply(state.selectedMail); },
      [shortcuts.replyAll]: () => { if (state.selectedMail) _handleReplyAll(state.selectedMail); },
      [shortcuts.forward]: () => { if (state.selectedMail) _handleForward(state.selectedMail); },
      [shortcuts.delete]: () => { if (state.selectedMail) _handleDelete(state.selectedMail); },
      [shortcuts.archive]: () => { if (state.selectedMail) _handleArchive(state.selectedMail); },
      [shortcuts.star]: () => { if (state.selectedMail) _toggleMailStar(state.selectedMail.id); },
      [shortcuts.unread]: () => { if (state.selectedMail) _handleMarkUnread(state.selectedMail); },
      [shortcuts.next]: () => {
        if (!state.mails.length) return;
        const idx = state.selectedMail
          ? state.mails.findIndex((m) => m.id === state.selectedMail.id) : -1;
        if (idx + 1 < state.mails.length) _selectMail(state.mails[idx + 1].id);
      },
      [shortcuts.prev]: () => {
        if (!state.mails.length) return;
        const idx = state.selectedMail
          ? state.mails.findIndex((m) => m.id === state.selectedMail.id)
          : state.mails.length;
        if (idx - 1 >= 0) _selectMail(state.mails[idx - 1].id);
      },
      [shortcuts.refresh]: () => { _refreshCurrentFolder(); },
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
      if (idx + 1 < state.mails.length) _selectMail(state.mails[idx + 1].id);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (!state.mails.length) return;
      const idx = state.selectedMail
        ? state.mails.findIndex((m) => m.id === state.selectedMail.id)
        : state.mails.length;
      if (idx - 1 >= 0) _selectMail(state.mails[idx - 1].id);
      return;
    }

    // Non-configurable shortcuts
    switch (e.key) {
      case 'Enter': {
        if (!state.selectedMail && state.mails.length > 0) {
          _selectMail(state.mails[0].id);
        }
        break;
      }

      case 'Escape': {
        if (state.selectedMail) {
          e.preventDefault();
          const prevId = state.selectedMail.id;
          state.selectedMail = null;
          state.mailBody = null;
          _updateMailItemSelection(prevId, null);
          _renderView();
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
