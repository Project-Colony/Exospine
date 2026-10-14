// Exospine: actions on one email.

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId, findMail, updateMail } from './state.js';
import { toastError } from './components/toast_helpers.js';
import { t, tn } from './i18n/index.js';
import { _isOffline, queueAction } from './offline.js';

// These callbacks are injected by app.js at init time
let _renderList = () => {};
let _renderView = () => {};
let _removeMail = () => {};

export function setMailActionCallbacks({ renderList, renderView, removeMail }) {
  _renderList = renderList;
  _renderView = renderView;
  _removeMail = removeMail;
}

export async function handleArchive(mail) {
  const originalFolder = state.activeFolder;
  const accountId = activeAccountId();
  const mailCopy = { ...mail };

  try {
    await api.archiveMail(accountId, originalFolder, mail.id);
    _removeMail(mail.id);
    showToast(t('archived_undo'), 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Archive', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        _renderList();
        showToast(t('archive_undone'), 'info');
      } catch (e) {
        showToast(t('undo_failed', { error: e }), 'error');
      }
    });
  } catch (err) {
    toastError('error_archive', err);
  }
}

export async function handleDelete(mail) {
  const confirmed = await showDialog({
    title: t('delete_email_title'),
    message: t('delete_email_confirm'),
    confirmLabel: t('delete'),
    danger: true,
  });
  if (!confirmed) return;

  const originalFolder = state.activeFolder;
  const accountId = activeAccountId();
  const mailCopy = { ...mail };

  try {
    await api.deleteMail(accountId, originalFolder, mail.id);
    _removeMail(mail.id);
    showToast(t('deleted_undo'), 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Trash', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        _renderList();
        showToast(t('delete_undone'), 'info');
      } catch (e) {
        showToast(t('undo_failed', { error: e }), 'error');
      }
    });
  } catch (err) {
    toastError('error_delete', err);
  }
}

export async function handleMarkRead(mail) {
  try {
    await api.markRead(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = true;
    _renderList();
  } catch (err) {
    toastError('error_mark_read', err);
  }
}

export async function handleMarkUnread(mail) {
  try {
    await api.markUnread(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = false;
    _renderList();
    showToast(t('marked_unread'), 'info');
  } catch (err) {
    toastError('error_mark_unread', err);
  }
}

export async function toggleMailStar(mailId) {
  const mail = findMail(mailId);
  const newStarred = !(mail?.is_starred);
  // Optimistic update
  updateMail(mailId, { is_starred: newStarred });
  _renderList();

  try {
    await api.toggleStar(activeAccountId(), state.activeFolder, mailId, newStarred);
  } catch (err) {
    // Revert on failure
    updateMail(mailId, { is_starred: !newStarred });
    _renderList();
    toastError('error_toggle_star', err);
  }
}

export async function handleFlagMail(mailId, dueDate) {
  const mail = findMail(mailId);
  if (!mail) return;
  // Optimistic update
  updateMail(mailId, { flag_due_date: dueDate });
  _renderList();

  try {
    await api.flagMail(mailId, dueDate);
  } catch (err) {
    updateMail(mailId, { flag_due_date: null });
    _renderList();
    toastError('error_flag', err);
  }
}

export async function handleUnflagMail(mailId) {
  const mail = findMail(mailId);
  if (!mail) return;
  const prev = mail.flag_due_date;
  updateMail(mailId, { flag_due_date: null });
  _renderList();

  try {
    await api.unflagMail(mailId);
  } catch (err) {
    updateMail(mailId, { flag_due_date: prev });
    _renderList();
    toastError('error_unflag', err);
  }
}

export async function handleSweepSender(mail) {
  // Extract exact sender email from the "from" field
  const extractEmail = (from) => {
    const match = from.match(/<([^>]+)>/);
    return match ? match[1].toLowerCase() : from.toLowerCase().trim();
  };

  const fromField = mail.from || '';
  const senderEmail = extractEmail(fromField);
  if (!senderEmail) {
    showToast(t('sender_unknown'), 'error');
    return;
  }

  const confirmed = await showDialog({
    title: t('sweep_sender_title'),
    message: t('sweep_sender_confirm', { sender: senderEmail }),
    confirmLabel: t('delete_all'),
    danger: true,
  });
  if (!confirmed) return;

  if (_isOffline) {
    queueAction({ type: 'sweep_sender', accountId: activeAccountId(), folder: state.activeFolder, senderEmail });
    // Optimistic local removal
    const toRemove = state.mails.filter(m => extractEmail(m.from || '') === senderEmail);
    for (const m of toRemove) _removeMail(m.id);
    return;
  }

  try {
    const count = await api.sweepSender(activeAccountId(), state.activeFolder, senderEmail);
    showToast(count ? tn('swept', count, { sender: senderEmail }) : t('swept_all', { sender: senderEmail }), 'success');
    // Remove matching mails from local state
    const toRemove = state.mails.filter(m => extractEmail(m.from || '') === senderEmail).map(m => m.id);
    for (const id of toRemove) _removeMail(id);
  } catch (err) {
    showToast(t('error_sweep', { error: err }), 'error');
  }
}

export async function handleReportSpam(mail) {
  try {
    await api.reportSpam(mail.id);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes('Spam')) mail.categories.push('Spam');
    _renderList();
    showToast(t('reported_spam'), 'info');
  } catch (err) {
    toastError('error_report_spam', err);
  }
}

export async function handleReportNotSpam(mail) {
  try {
    await api.reportNotSpam(mail.id);
    if (mail.categories) {
      mail.categories = mail.categories.filter((c) => c !== 'Spam');
    }
    _renderList();
    showToast(t('marked_not_spam'), 'info');
  } catch (err) {
    toastError('error_not_spam', err);
  }
}

export async function handleAddCategory(mail, category) {
  try {
    await api.addCategory(mail.id, category);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes(category)) mail.categories.push(category);
    _renderList();
  } catch (err) {
    toastError('error_add_category', err);
  }
}

export async function handleRemoveCategory(mail, category) {
  try {
    await api.removeCategory(mail.id, category);
    if (mail.categories) {
      mail.categories = mail.categories.filter((c) => c !== category);
    }
    _renderList();
  } catch (err) {
    toastError('error_remove_category', err);
  }
}
