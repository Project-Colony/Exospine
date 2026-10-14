// Exospine: the HTML of one row of the mail list.

import { formatDate } from '../../date_format.js';
import { esc } from '../../html.js';
import { t, tn } from '../../i18n/index.js';
import { loadDisplayRules } from '../settings.js';
import { list } from './state.js';

// ── Predefined categories with colors ──────────────────────────────
export const CATEGORIES = [
  { name: 'Important', color: '#e74c3c' },
  { name: 'Work', color: '#3498db' },
  { name: 'Personal', color: '#2ecc71' },
  { name: 'Finance', color: '#f39c12' },
  { name: 'Travel', color: '#9b59b6' },
];

// Category color lookup (includes Spam)
const CATEGORY_COLORS = {};
CATEGORIES.forEach((c) => { CATEGORY_COLORS[c.name] = c.color; });
CATEGORY_COLORS['Spam'] = '#95a5a6';

/** A category's name for display; the stored name stays the English one. */
export function categoryLabel(name) {
  const key = `category_${String(name).toLowerCase()}`;
  const label = t(key);
  return label === key ? name : label;
}

function getCategoryColor(name) {
  return CATEGORY_COLORS[name] || '#7f8c8d';
}

// ── Account colors for unified inbox left border ────────────────
const ACCOUNT_COLORS = [
  'rgb(0, 120, 214)',
  'rgb(160, 90, 220)',
  'rgb(220, 80, 60)',
  'rgb(40, 167, 69)',
  'rgb(230, 150, 0)',
  'rgb(0, 180, 160)',
];

/** Return a CSS class name for the account color border (CSP-safe). */
function getAccountColorClass(accountEmail, accounts) {
  if (!accountEmail || !accounts || accounts.length <= 1) return '';
  const idx = accounts.findIndex(a => a.email === accountEmail);
  if (idx < 0) return '';
  return 'acct-color-' + (idx % ACCOUNT_COLORS.length);
}

function renderCategoryBadges(mail) {
  const cats = mail.categories || [];
  if (cats.length === 0) return '';
  return cats
    .map(
      (c) =>
        `<span class="mail-category-badge" style="background:${getCategoryColor(c)}" title="${esc(categoryLabel(c))}">${esc(categoryLabel(c))}</span>`
    )
    .join('');
}

function renderSpamIndicator(mail) {
  const score = mail.spam_score || 0;
  if (score > 3.0) {
    return `<span class="mail-spam-indicator" title="${t('likely_spam')}">\u26A0</span>`;
  }
  if (score > 1.5) {
    return `<span class="mail-spam-indicator mild" title="${t('possibly_spam')}">\u26A0</span>`;
  }
  return '';
}

export function renderThreadHeader(thread, expanded) {
  const hasUnread = thread.unread_count > 0;
  const participants = thread.participants.slice(0, 3).map(esc).join(', ');
  const extra = thread.participants.length > 3 ? ` +${thread.participants.length - 3}` : '';
  const count = thread.messages.length;
  const lastMail = thread.messages[thread.messages.length - 1];
  const catBadges = renderCategoryBadges(lastMail);
  const spamInd = renderSpamIndicator(lastMail);

  return `
    <div class="thread-header mail-item${hasUnread ? ' unread' : ''}" data-thread-id="${esc(thread.thread_id)}" role="option" tabindex="0" aria-expanded="${expanded}" aria-label="${esc(tn('thread_label', count, { subject: thread.subject }))}${hasUnread ? ', ' + t('thread_unread', { count: thread.unread_count }) : ''}">
      <div class="thread-expand-icon" aria-hidden="true">${expanded ? '\u25BC' : '\u25B6'}</div>
      <div class="mail-item-body">
        <div class="mail-item-top">
          <span class="mail-item-from">${participants}${esc(extra)}</span>
          <span class="thread-count">(${count})</span>
          <span class="mail-item-date">${formatDate(thread.last_date)}</span>
        </div>
        <div class="mail-item-subject">${spamInd}${esc(thread.subject)}${catBadges ? ' ' + catBadges : ''}</div>
        <div class="mail-item-preview">${thread.unread_count > 0 ? t('thread_unread', { count: thread.unread_count }) : esc(lastMail.preview || '')}</div>
      </div>
    </div>
  `;
}

function senderAvatar(from) {
  const name = (from || '').split('<')[0].trim() || from || '';
  const initial = name[0]?.toUpperCase() || '?';
  const colors = ['#e74c3c','#3498db','#2ecc71','#f39c12','#9b59b6','#1abc9c','#e67e22','#34495e'];
  let hash = 0;
  for (const c of (from || '')) hash = (hash * 31 + c.charCodeAt(0)) & 0xffffffff;
  const color = colors[Math.abs(hash) % colors.length];
  return `<div class="mail-avatar" style="background:${color}">${esc(initial)}</div>`;
}

function renderImportanceIcon(mail) {
  const imp = mail.importance || 'normal';
  if (imp === 'high') return `<span class="mail-importance-icon high" title="${t('high_importance')}">\u2757</span>`;
  if (imp === 'low') return `<span class="mail-importance-icon low" title="${t('low_importance')}">\u2193</span>`;
  return '';
}

function renderFlagIcon(mail) {
  const flagged = !!mail.flag_due_date;
  if (!flagged) return `<div class="mail-item-flag" data-flag-id="${esc(mail.id)}" title="${t('flag_for_follow_up')}" role="button" tabindex="0">\u2691</div>`;
  // Color class: red if overdue, orange if due today, default blue
  const now = new Date().toISOString().slice(0, 10);
  const due = mail.flag_due_date.slice(0, 10);
  let colorClass = 'flag-default';
  if (due < now) colorClass = 'flag-overdue';
  else if (due === now) colorClass = 'flag-due-today';
  return `<div class="mail-item-flag flagged ${colorClass}" data-flag-id="${esc(mail.id)}" title="${esc(t('flagged_due', { date: mail.flag_due_date }))}" role="button" tabindex="0">\u2691</div>`;
}

export function renderMailItem(mail, state, isThreadChild = false, cachedRules = null) {
  const isSelected = state.selectedMail && state.selectedMail.id === mail.id;
  const isMultiSelected = list.selectedIds.has(mail.id);
  const isUnread = !(mail.is_read || mail.read);
  const isStarred = mail.is_starred || mail.starred;
  const isPinned = mail.is_pinned || false;
  const isDuplicate = list.duplicateMode && list.duplicateIds.has(mail.id);
  const catBadges = renderCategoryBadges(mail);
  const spamInd = renderSpamIndicator(mail);
  const importanceInd = renderImportanceIcon(mail);
  const flagIcon = renderFlagIcon(mail);
  const indent = isThreadChild ? ' thread-child' : '';
  // Account badge for unified inbox view
  const accountBadge = mail._accountEmail
    ? `<span class="mail-item-account-badge" title="${esc(mail._accountEmail)}">${esc((mail._accountName || mail._accountEmail || '')[0] || '?')}</span>`
    : '';

  // Multi-select checkbox (visible when multi-select is active or on hover via CSS)
  const checkboxVisible = list.selectedIds.size > 0;
  const checkbox = `<input type="checkbox" class="mail-item-checkbox${checkboxVisible ? ' visible' : ''}" data-check-id="${esc(mail.id)}" ${isMultiSelected ? 'checked' : ''} tabindex="-1" aria-label="${t('select_email_checkbox')}" />`;

  const dupeBadge = isDuplicate ? `<span class="mail-duplicate-badge" title="${t('duplicate_email')}">\u2716 ${t('dupe')}</span>` : '';

  return `
    <div class="mail-item${isSelected ? ' selected' : ''}${isUnread ? ' unread' : ''}${isPinned ? ' pinned' : ''}${isMultiSelected ? ' multi-selected' : ''}${isDuplicate ? ' duplicate' : ''}${indent}${getDisplayRuleClasses(mail, cachedRules)}${mail._accountEmail ? ' ' + getAccountColorClass(mail._accountEmail, (typeof state !== 'undefined' && state.accounts) || []) : ''}" data-mail-id="${esc(mail.id)}" draggable="true" role="option" aria-selected="${isSelected}" tabindex="0">
      ${checkbox}
      ${senderAvatar(mail.from_name || mail.from || t('unknown_sender'))}
      ${isUnread ? '<div class="mail-item-unread-dot" aria-hidden="true"></div>' : ''}
      ${importanceInd}
      ${dupeBadge}
      <div class="mail-item-pin${isPinned ? ' pinned' : ''}" data-pin-id="${esc(mail.id)}" title="${t(isPinned ? 'unpin' : 'pin_to_top')}" role="button" aria-label="${t(isPinned ? 'unpin_email' : 'pin_email')}" tabindex="0">
        \uD83D\uDCCC
      </div>
      <div class="mail-item-star${isStarred ? ' starred' : ''}" data-star-id="${esc(mail.id)}" title="${t('toggle_star')}" role="button" aria-label="${t(isStarred ? 'unstar_email' : 'star_email')}" tabindex="0">
        ${isStarred ? '\u2605' : '\u2606'}
      </div>
      ${flagIcon}
      <div class="mail-item-body">
        <div class="mail-item-top">
          ${accountBadge}
          <span class="mail-item-from">${esc(mail.from_name || mail.from || t('unknown_sender'))}</span>
          <span class="mail-item-date">${formatDate(mail.date)}</span>
        </div>
        <div class="mail-item-subject">${spamInd}${esc(mail.subject || t('no_subject'))}${catBadges ? ' ' + catBadges : ''}</div>
        <div class="mail-item-preview">${esc(mail.preview || '')}</div>
      </div>
      <div class="mail-item-quick-actions" aria-label="${t('quick_actions')}">
        <button class="quick-action-btn" data-quick="archive" data-quick-id="${esc(mail.id)}" title="${t('archive')}" aria-label="${t('archive')}">\uD83D\uDCE6</button>
        <button class="quick-action-btn" data-quick="delete" data-quick-id="${esc(mail.id)}" title="${t('delete')}" aria-label="${t('delete')}">\uD83D\uDDD1</button>
        <button class="quick-action-btn" data-quick="star" data-quick-id="${esc(mail.id)}" title="${t('star')}" aria-label="${t('star')}">${isStarred ? '\u2605' : '\u2606'}</button>
      </div>
    </div>
  `;
}

// Cache display rules to avoid reading localStorage on every render
let _displayRulesCache = null;
let _displayRulesCacheTime = 0;

/**
 * Return CSS class names for display rules (CSP-safe replacement for inline styles).
 * For highlight rules with dynamic colors, uses data-attributes + CSS custom properties
 * applied via a <style> element injected in ensureDisplayRuleStyles().
 */
function getDisplayRuleClasses(mail, cachedRules) {
  const rules = cachedRules !== null ? cachedRules : (() => {
    const now = Date.now();
    if (!_displayRulesCache || now - _displayRulesCacheTime > 5000) {
      _displayRulesCache = loadDisplayRules();
      _displayRulesCacheTime = now;
    }
    return _displayRulesCache;
  })();
  if (!rules || rules.length === 0) return '';

  const classes = [];
  for (const rule of rules) {
    let fieldValue = '';
    if (rule.field === 'from') {
      fieldValue = (mail.from_name || mail.from || '').toLowerCase();
    } else if (rule.field === 'subject') {
      fieldValue = (mail.subject || '').toLowerCase();
    }

    if (fieldValue.includes((rule.value || '').toLowerCase())) {
      if (rule.style === 'highlight') {
        // Use a rule-specific class; ensureDisplayRuleStyles() creates the CSS
        const ruleClass = 'dr-hl-' + (rule.id || rule.value || '').replace(/[^a-zA-Z0-9]/g, '_');
        classes.push(ruleClass);
        _ensureDisplayRuleStyle(ruleClass, rule.color);
      } else if (rule.style === 'bold') {
        classes.push('dr-bold');
      } else if (rule.style === 'italic') {
        classes.push('dr-italic');
      }
    }
  }

  return classes.length > 0 ? ' ' + classes.join(' ') : '';
}

/** Cache of injected display rule highlight styles to avoid duplicates. */
const _drStyleCache = new Set();

/** Inject a <style> rule for a highlight display rule (once per rule). */

/** Inject a <style> rule for a highlight display rule (once per rule). */
function _ensureDisplayRuleStyle(className, color) {
  if (_drStyleCache.has(className)) return;
  _drStyleCache.add(className);
  let styleEl = document.getElementById('exospine-dr-styles');
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = 'exospine-dr-styles';
    document.head.appendChild(styleEl);
  }
  styleEl.sheet.insertRule(
    `.${className} { background-color: ${color}22; border-left: 3px solid ${color}; }`,
    styleEl.sheet.cssRules.length
  );
}
