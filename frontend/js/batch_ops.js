// Exospine — Batch operations (multi-select)

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId } from './state.js';

// These callbacks are injected by app.js at init time
let _renderList = () => {};
let _removeMail = () => {};

export function setBatchOpsCallbacks({ renderList, removeMail }) {
  _renderList = renderList;
  _removeMail = removeMail;
}

export async function handleBatchArchive(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.archiveMail(accountId, folder, id);
      _removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) archived.`, 'success');
}

export async function handleBatchDelete(mailIds) {
  const confirmed = await showDialog({
    title: 'Delete Emails',
    message: `Move ${mailIds.size || mailIds.length} email(s) to Trash?`,
    confirmLabel: 'Delete',
    danger: true,
  });
  if (!confirmed) return;

  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.deleteMail(accountId, folder, id);
      _removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) deleted.`, 'success');
}

export async function handleBatchMarkRead(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  for (const id of mailIds) {
    try {
      await api.markRead(accountId, folder, id);
      const mail = state.mails.find((m) => m.id === id);
      if (mail) mail.is_read = true;
    } catch {}
  }
  _renderList();
  showToast(`${mailIds.size || mailIds.length} email(s) marked as read.`, 'info');
}

export async function handleBatchMove(mailIds, targetFolder) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  for (const id of mailIds) {
    try {
      await api.moveMail(accountId, folder, id, targetFolder);
      _removeMail(id);
      count++;
    } catch {}
  }
  showToast(`${count} email(s) moved to ${targetFolder}.`, 'success');
}
