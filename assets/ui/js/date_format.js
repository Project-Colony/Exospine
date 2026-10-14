// Exospine: dates, written in the interface language.

import { getLanguage, t } from './i18n/index.js';

/**
 * Format a date for display in the mail list.
 * Shows time for today, "Yesterday" for yesterday,
 * month+day for same year, or full date for older.
 */
export function formatDate(dateStr) {
  if (!dateStr) return '';
  try {
    const d = new Date(dateStr);
    if (isNaN(d.getTime())) return dateStr; // Invalid date, fallback to raw string
    const now = new Date();
    const isToday = d.toDateString() === now.toDateString();

    if (isToday) {
      return d.toLocaleTimeString(getLanguage(), { hour: '2-digit', minute: '2-digit' });
    }

    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (d.toDateString() === yesterday.toDateString()) {
      return t('yesterday');
    }

    // Same year
    if (d.getFullYear() === now.getFullYear()) {
      return d.toLocaleDateString(getLanguage(), { month: 'short', day: 'numeric' });
    }

    return d.toLocaleDateString(getLanguage(), { year: 'numeric', month: 'short', day: 'numeric' });
  } catch {
    return dateStr;
  }
}

/**
 * Format a date for the mail view header (full date/time).
 */
export function formatFullDate(dateStr) {
  if (!dateStr) return '';
  try {
    const d = new Date(dateStr);
    if (isNaN(d.getTime())) return dateStr; // Invalid date, fallback to raw string
    return d.toLocaleString(getLanguage(), {
      weekday: 'long',
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return dateStr;
  }
}
