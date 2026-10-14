// Exospine — Toast helper utilities

import { showToast } from './toast.js';

/**
 * Show an error toast for a failed action.
 * @param {string} action - What was being done, e.g. "archive", "load accounts"
 * @param {*} err - The error message or object
 */
export function toastError(action, err) {
  showToast(`Failed to ${action}: ${err}`, 'error');
}
