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
  let failed = 0;
  for (const id of mailIds) {
    try {
      await api.archiveMail(accountId, folder, id);
      _removeMail(id);
      count++;
    } catch (e) { failed++; console.warn('Batch archive failed:', e); }
  }
  showToast(`${count} email(s) archived.`, 'success');
  if (failed > 0) showToast(`${failed} operations failed`, 'error');
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
  let failed = 0;
  for (const id of mailIds) {
    try {
      await api.deleteMail(accountId, folder, id);
      _removeMail(id);
      count++;
    } catch (e) { failed++; console.warn('Batch delete failed:', e); }
  }
  showToast(`${count} email(s) deleted.`, 'success');
  if (failed > 0) showToast(`${failed} operations failed`, 'error');
}

export async function handleBatchMarkRead(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let failed = 0;
  for (const id of mailIds) {
    try {
      await api.markRead(accountId, folder, id);
      const mail = state.mails.find((m) => m.id === id);
      if (mail) mail.is_read = true;
    } catch (e) { failed++; console.warn('Batch mark-read failed:', e); }
  }
  _renderList();
  showToast(`${mailIds.size || mailIds.length} email(s) marked as read.`, 'info');
  if (failed > 0) showToast(`${failed} operations failed`, 'error');
}

export async function handleBatchMove(mailIds, targetFolder) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  let count = 0;
  let failed = 0;
  for (const id of mailIds) {
    try {
      await api.moveMail(accountId, folder, id, targetFolder);
      _removeMail(id);
      count++;
    } catch (e) { failed++; console.warn('Batch move failed:', e); }
  }
  showToast(`${count} email(s) moved to ${targetFolder}.`, 'success');
  if (failed > 0) showToast(`${failed} operations failed`, 'error');
}
