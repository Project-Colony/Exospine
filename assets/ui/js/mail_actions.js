// Exospine — Individual mail actions

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId, findMail, updateMail } from './state.js';
import { toastError } from './components/toast_helpers.js';
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
    showToast('Email archived. [Undo]', 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Archive', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        _renderList();
        showToast('Archive undone.', 'info');
      } catch (e) {
        showToast(`Undo failed: ${e}`, 'error');
      }
    });
  } catch (err) {
    toastError('archive', err);
  }
}

export async function handleDelete(mail) {
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
    _removeMail(mail.id);
    showToast('Email deleted. [Undo]', 'success', 5000, async () => {
      try {
        await api.moveMail(accountId, 'Trash', mail.id, originalFolder);
        state.mails.unshift(mailCopy);
        _renderList();
        showToast('Delete undone.', 'info');
      } catch (e) {
        showToast(`Undo failed: ${e}`, 'error');
      }
    });
  } catch (err) {
    toastError('delete', err);
  }
}

export async function handleMarkRead(mail) {
  try {
    await api.markRead(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = true;
    _renderList();
  } catch (err) {
    toastError('mark as read', err);
  }
}

export async function handleMarkUnread(mail) {
  try {
    await api.markUnread(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = false;
    _renderList();
    showToast('Marked as unread.', 'info');
  } catch (err) {
    toastError('mark unread', err);
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
    toastError('toggle star', err);
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
    toastError('flag mail', err);
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
    toastError('unflag mail', err);
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
    const toRemove = state.mails.filter(m => extractEmail(m.from || '') === senderEmail);
    for (const m of toRemove) _removeMail(m.id);
    return;
  }

  try {
    const count = await api.sweepSender(activeAccountId(), state.activeFolder, senderEmail);
    showToast(`Deleted ${count || 'all'} emails from ${senderEmail}.`, 'success');
    // Remove matching mails from local state
    const toRemove = state.mails.filter(m => extractEmail(m.from || '') === senderEmail).map(m => m.id);
    for (const id of toRemove) _removeMail(id);
  } catch (err) {
    showToast(`Sweep failed: ${err}`, 'error');
  }
}

export async function handleReportSpam(mail) {
  try {
    await api.reportSpam(mail.id);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes('Spam')) mail.categories.push('Spam');
    _renderList();
    showToast('Reported as spam.', 'info');
  } catch (err) {
    toastError('report spam', err);
  }
}

export async function handleReportNotSpam(mail) {
  try {
    await api.reportNotSpam(mail.id);
    if (mail.categories) {
      mail.categories = mail.categories.filter((c) => c !== 'Spam');
    }
    _renderList();
    showToast('Marked as not spam.', 'info');
  } catch (err) {
    toastError('unmark spam', err);
  }
}

export async function handleAddCategory(mail, category) {
  try {
    await api.addCategory(mail.id, category);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes(category)) mail.categories.push(category);
    _renderList();
  } catch (err) {
    toastError('add category', err);
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
    toastError('remove category', err);
  }
}
