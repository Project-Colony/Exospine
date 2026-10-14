// Exospine: toast helpers.

import { t } from '../i18n/index.js';
import { showToast } from './toast.js';

/**
 * Show an error toast for a failed action.
 * @param {string} key - The i18n key of the message, which shows the error as {error}
 * @param {*} err - The error message or object
 */
export function toastError(key, err) {
  showToast(t(key, { error: err }), 'error');
}
