// Exospine: mail list sort orders, including the priority score.

import { list } from './state.js';

// Priority scoring (cached per sort pass)
let _repliedToSenders = null;
let _repliedToSendersTime = 0;

/** Build a Set of sender emails you have replied to (from sent folders). */
function getRepliedToSenders(mails) {
  const now = Date.now();
  // Cache for 30 seconds to avoid repeated scans
  if (_repliedToSenders && now - _repliedToSendersTime < 30000) return _repliedToSenders;
  const senders = new Set();
  for (const m of mails) {
    // Sent mails have folder containing 'Sent' or are in [Gmail]/Sent Mail
    const folder = (m.folder || '').toLowerCase();
    if (folder.includes('sent')) {
      // The "to" field of sent mails = people you've replied to
      const to = (m.to || '').toLowerCase();
      // Extract email addresses from "Name <email>" or plain email
      const matches = to.match(/[\w.+-]+@[\w.-]+/g);
      if (matches) matches.forEach(e => senders.add(e));
    }
  }
  _repliedToSenders = senders;
  _repliedToSendersTime = now;
  return senders;
}

/** Compute priority score for a single mail. */
function computePriorityScore(mail, repliedSenders, currentUserEmail) {
  let score = 0;

  // +3 if from a contact you've replied to before
  const fromEmail = ((mail.from || '').match(/[\w.+-]+@[\w.-]+/) || [])[0] || '';
  if (fromEmail && repliedSenders.has(fromEmail.toLowerCase())) {
    score += 3;
  }

  // +2 if you're in the To field (not CC/BCC)
  if (currentUserEmail) {
    const toField = (mail.to || '').toLowerCase();
    if (toField.includes(currentUserEmail.toLowerCase())) {
      score += 2;
    }
  }

  // +1 if has attachments
  if (mail.has_attachments || (mail.attachment_meta && mail.attachment_meta.length > 0)) {
    score += 1;
  }

  // +1 if marked important (X-Priority or importance header)
  if (mail.importance === 'high' || mail.x_priority === '1' || mail.x_priority === '2') {
    score += 1;
  }

  // -1 if from a mailing list (has List-Unsubscribe)
  if (mail.list_unsubscribe || mail.unsubscribe_url) {
    score -= 1;
  }

  // -2 if spam score > 1.5
  if ((mail.spam_score || 0) > 1.5) {
    score -= 2;
  }

  return score;
}

export function sortMails(mails, sortKey) {
  const sorted = [...mails];
  switch (sortKey) {
    case 'date-asc':
      sorted.sort((a, b) => new Date(a.date) - new Date(b.date));
      break;
    case 'sender-az':
      sorted.sort((a, b) => (a.from_name || a.from || '').localeCompare(b.from_name || b.from || ''));
      break;
    case 'subject-az':
      sorted.sort((a, b) => (a.subject || '').localeCompare(b.subject || ''));
      break;
    case 'priority': {
      // Build replied-to senders set from all available mails
      const allMails = (list.lastState && list.lastState.allMails) || mails;
      const repliedSenders = getRepliedToSenders(allMails);
      const currentEmail = (list.lastState && list.lastState.accounts && list.lastState.accounts[list.lastState.activeAccount])
        ? (list.lastState.accounts[list.lastState.activeAccount].email || '')
        : '';
      // Cache scores in a Map for this sort pass
      const scores = new Map();
      for (const m of sorted) {
        scores.set(m.id, computePriorityScore(m, repliedSenders, currentEmail));
      }
      sorted.sort((a, b) => {
        const diff = scores.get(b.id) - scores.get(a.id);
        if (diff !== 0) return diff;
        // Tie-break by date descending
        return new Date(b.date) - new Date(a.date);
      });
      break;
    }
    case 'date-desc':
    default:
      sorted.sort((a, b) => new Date(b.date) - new Date(a.date));
      break;
  }
  return sorted;
}
