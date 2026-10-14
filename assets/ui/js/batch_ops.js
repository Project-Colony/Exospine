// Exospine: actions on several selected emails at once.

import * as api from './api.js';
import { showToast } from './components/toast.js';
import { showDialog } from './components/dialog.js';
import { state, activeAccountId, findMail } from './state.js';
import { t, tn } from './i18n/index.js';

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
  if (failed > 0) showToast(tn('batch_failed', failed), 'error');
  return count;
}

export async function handleBatchArchive(mailIds) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  const count = await batchOperation(mailIds, async (id) => {
    await api.archiveMail(accountId, folder, id);
    _removeMail(id);
  }, 'archive');
  showToast(tn('batch_archived', count), 'success');
}

export async function handleBatchDelete(mailIds) {
  const confirmed = await showDialog({
    title: t('delete_emails_title'),
    message: tn('delete_emails_confirm', mailIds.size || mailIds.length),
    confirmLabel: t('delete'),
    danger: true,
  });
  if (!confirmed) return;

  const accountId = activeAccountId();
  const folder = state.activeFolder;
  const count = await batchOperation(mailIds, async (id) => {
    await api.deleteMail(accountId, folder, id);
    _removeMail(id);
  }, 'delete');
  showToast(tn('batch_deleted', count), 'success');
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
  showToast(tn('batch_marked_read', mailIds.size || mailIds.length), 'info');
}

export async function handleBatchMove(mailIds, targetFolder) {
  const accountId = activeAccountId();
  const folder = state.activeFolder;
  const count = await batchOperation(mailIds, async (id) => {
    await api.moveMail(accountId, folder, id, targetFolder);
    _removeMail(id);
  }, 'move');
  showToast(tn('batch_moved', count, { folder: targetFolder }), 'success');
}
