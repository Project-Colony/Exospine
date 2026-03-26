// Exospine — Individual mail actions

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId } from './state.js';
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
    showToast(`Failed to archive: ${err}`, 'error');
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
    showToast(`Failed to delete: ${err}`, 'error');
  }
}

export async function handleMarkRead(mail) {
  try {
    await api.markRead(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = true;
    _renderList();
  } catch (err) {
    showToast(`Failed to mark as read: ${err}`, 'error');
  }
}

export async function handleMarkUnread(mail) {
  try {
    await api.markUnread(activeAccountId(), state.activeFolder, mail.id);
    mail.is_read = false;
    _renderList();
    showToast('Marked as unread.', 'info');
  } catch (err) {
    showToast(`Failed to mark unread: ${err}`, 'error');
  }
}

export async function toggleMailStar(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  const newStarred = !(mail?.is_starred);
  // Optimistic update
  if (mail) mail.is_starred = newStarred;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.is_starred = newStarred;
  }
  _renderList();

  try {
    await api.toggleStar(activeAccountId(), state.activeFolder, mailId, newStarred);
  } catch (err) {
    // Revert on failure
    if (mail) mail.is_starred = !newStarred;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.is_starred = !newStarred;
    }
    _renderList();
    showToast(`Failed to toggle star: ${err}`, 'error');
  }
}

export async function handleFlagMail(mailId, dueDate) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;
  // Optimistic update
  mail.flag_due_date = dueDate;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.flag_due_date = dueDate;
  }
  _renderList();

  try {
    await api.flagMail(mailId, dueDate);
  } catch (err) {
    mail.flag_due_date = null;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.flag_due_date = null;
    }
    _renderList();
    showToast(`Failed to flag mail: ${err}`, 'error');
  }
}

export async function handleUnflagMail(mailId) {
  const mail = state.mails.find((m) => m.id === mailId);
  if (!mail) return;
  const prev = mail.flag_due_date;
  mail.flag_due_date = null;
  if (state.selectedMail && state.selectedMail.id === mailId) {
    state.selectedMail.flag_due_date = null;
  }
  _renderList();

  try {
    await api.unflagMail(mailId);
  } catch (err) {
    mail.flag_due_date = prev;
    if (state.selectedMail && state.selectedMail.id === mailId) {
      state.selectedMail.flag_due_date = prev;
    }
    _renderList();
    showToast(`Failed to unflag mail: ${err}`, 'error');
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
    showToast(`Failed to report spam: ${err}`, 'error');
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
    showToast(`Failed to unmark spam: ${err}`, 'error');
  }
}

export async function handleAddCategory(mail, category) {
  try {
    await api.addCategory(mail.id, category);
    if (!mail.categories) mail.categories = [];
    if (!mail.categories.includes(category)) mail.categories.push(category);
    _renderList();
  } catch (err) {
    showToast(`Failed to add category: ${err}`, 'error');
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
    showToast(`Failed to remove category: ${err}`, 'error');
  }
}
