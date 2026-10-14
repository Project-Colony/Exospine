// Exospine: clicks, keys, context menus and drags on the rows of the mail list.

import { showContextMenu } from '../../components/context_menu.js';
import { t } from '../../i18n/index.js';
import { CATEGORIES, categoryLabel } from './items.js';
import { list } from './state.js';

/**
 * Listen on `contentEl` for the rows inside it, once: the virtual scroll
 * replaces the rows but keeps the element. `rerender` redraws the whole list.
 */
export function attachItemEvents(contentEl, state, actions, rerender) {
  if (contentEl._itemEvents) return;
  contentEl._itemEvents = true;

  // Use event delegation: one click/keydown/contextmenu/dragstart/dragend listener
  // on the contentEl instead of N listeners on each item.

  contentEl.addEventListener('click', (e) => {
    // Checkbox toggle (multi-select)
    const checkbox = e.target.closest('.mail-item-checkbox');
    if (checkbox) {
      e.stopPropagation();
      const mailId = checkbox.dataset.checkId;
      if (list.selectedIds.has(mailId)) {
        list.selectedIds.delete(mailId);
      } else {
        list.selectedIds.add(mailId);
      }
      rerender();
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
        if (list.selectedIds.has(mailId)) {
          list.selectedIds.delete(mailId);
        } else {
          list.selectedIds.add(mailId);
        }
        rerender();
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
      { label: `\u21A9 ${t('reply')}`, action: () => { if (actions.onReply) actions.onReply(mail); } },
      { label: `\u21A9\u21A9 ${t('reply_all')}`, action: () => { if (actions.onReplyAll) actions.onReplyAll(mail); } },
      { label: `\u21AA ${t('forward')}`, action: () => { if (actions.onForward) actions.onForward(mail); } },
      { separator: true },
      {
        label: `\u2709 ${t(isRead ? 'mark_unread' : 'mark_read')}`,
        action: () => {
          if (isRead && actions.onMarkUnread) actions.onMarkUnread(mail);
          else if (!isRead && actions.onMarkRead) actions.onMarkRead(mail);
        },
      },
      {
        label: isStarred ? `\u2606 ${t('unstar')}` : `\u2605 ${t('star')}`,
        action: () => { if (actions.onToggleStar) actions.onToggleStar(mailId); },
      },
      { separator: true },
      {
        label: isSpam ? `\u2714 ${t('not_spam')}` : `\u26A0 ${t('report_spam')}`,
        action: () => {
          if (isSpam && actions.onReportNotSpam) actions.onReportNotSpam(mail);
          else if (!isSpam && actions.onReportSpam) actions.onReportSpam(mail);
        },
      },
      {
        label: `\uD83C\uDFF7 ${t('categorize')}`,
        action: () => {
          showCategoryMenu(e.clientX, e.clientY, mail, actions);
        },
      },
      { separator: true },
      { label: `\uD83D\uDCE6 ${t('archive')}`, action: () => { if (actions.onArchive) actions.onArchive(mail); } },
      { label: `\uD83D\uDDD1 ${t('delete')}`, action: () => { if (actions.onDelete) actions.onDelete(mail); }, danger: true },
      { separator: true },
      {
        label: `\uD83E\uDDF9 ${t('sweep_sender_menu')}`,
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

function showCategoryMenu(x, y, mail, actions) {
  const currentCats = mail.categories || [];
  const menuItems = CATEGORIES.map((cat) => {
    const has = currentCats.includes(cat.name);
    return {
      label: `<span class="category-dot" style="background:var(--accent-${cat.accent})"></span>${has ? '\u2714 ' : ''}${categoryLabel(cat.name)}`,
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

function showFlagMenu(x, y, mailId, actions) {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).toISOString();
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).toISOString();
  const endOfWeek = new Date(now.getFullYear(), now.getMonth(), now.getDate() + (7 - now.getDay())).toISOString();

  const menuItems = [
    { label: `\u2691 ${t('flag_today')}`, action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, today); } },
    { label: `\u2691 ${t('tomorrow')}`, action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, tomorrow); } },
    { label: `\u2691 ${t('flag_this_week')}`, action: () => { if (actions.onFlagMail) actions.onFlagMail(mailId, endOfWeek); } },
    { separator: true },
    {
      label: `\u2691 ${t('pick_date')}`,
      action: () => {
        const dateStr = prompt(t('due_date_prompt'));
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
