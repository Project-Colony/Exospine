// Exospine — Batch operations (multi-select)

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId, findMail } from './state.js';
import { toastError } from './components/toast_helpers.js';

// These callbacks are injected by app.js at init time
let _renderList = () => {};
let _removeMail = () => {};

export function setBatchOpsCallbacks({ renderList, removeMail }) {
  _renderList = renderList;
  _removeMail = removeMail;
}

/**
 * Generic batch operation helper.
 * Runs operationFn for each id, tracks successes and failures.
 */
async function batchOperation(ids, operationFn, actionName) {
  let count = 0;
  let failed = 0;
  for (const id of ids) {
    try {
      await operationFn(id);
      count++;
    } catch (e) {
      failed++;
      console.warn(`Batch ${actionName} failed for ${id}:`, e);
    }
  }
  if (failed > 0) showToast(`${failed} ${actionName} operations failed`, 'error');
  return count;
}

export async function handleBatchArchive(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  const count = await batchOperation(mailIds, async (id) => {
    await api.archiveMail(accountId, folder, id);
    _removeMail(id);
  }, 'archive');
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
  const count = await batchOperation(mailIds, async (id) => {
    await api.deleteMail(accountId, folder, id);
    _removeMail(id);
  }, 'delete');
  showToast(`${count} email(s) deleted.`, 'success');
}

export async function handleBatchMarkRead(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  await batchOperation(mailIds, async (id) => {
    await api.markRead(accountId, folder, id);
    const mail = findMail(id);
    if (mail) mail.is_read = true;
  }, 'mark-read');
  _renderList();
  showToast(`${mailIds.size || mailIds.length} email(s) marked as read.`, 'info');
}

export async function handleBatchMove(mailIds, targetFolder) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  const count = await batchOperation(mailIds, async (id) => {
    await api.moveMail(accountId, folder, id, targetFolder);
    _removeMail(id);
  }, 'move');
  showToast(`${count} email(s) moved to ${targetFolder}.`, 'success');
}
