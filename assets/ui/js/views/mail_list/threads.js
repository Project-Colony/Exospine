// Exospine: grouping the mail list by conversation.

import { t } from '../../i18n/index.js';

let _cachedThreadKey = null; // content-based cache key: "length:firstId:lastId"
let _cachedThreads = null;

/** Compute a lightweight cache key based on array content. */
function _threadCacheKey(mails) {
  if (!mails || mails.length === 0) return '0::';
  return `${mails.length}:${mails[0].id}:${mails[mails.length - 1].id}`;
}

/**
 * Group mails by thread_id. Returns an array of thread objects:
 * { thread_id, subject, messages: [mail], last_date, participants, unread_count, is_expanded }
 * Results are cached and only rebuilt when the content signature changes.
 */
export function groupByThread(mails) {
  const key = _threadCacheKey(mails);
  if (key === _cachedThreadKey && _cachedThreads) return _cachedThreads;
  const map = new Map();
  for (const mail of mails) {
    // Only group mails that share a thread_id different from their own id.
    // If thread_id is empty or equals the mail's own id, treat as standalone.
    const tid = (mail.thread_id && mail.thread_id !== mail.id) ? mail.thread_id : mail.id;
    if (!map.has(tid)) {
      map.set(tid, []);
    }
    map.get(tid).push(mail);
  }

  const threads = [];
  for (const [thread_id, messages] of map) {
    // Sort messages within thread by date ascending
    messages.sort((a, b) => new Date(a.date) - new Date(b.date));
    const lastMsg = messages[messages.length - 1];
    const participantSet = new Set();
    let unreadCount = 0;
    for (const m of messages) {
      const fromName = m.from_name || m.from || t('unknown_sender');
      participantSet.add(fromName.split('<')[0].trim());
      if (!(m.is_read || m.read)) unreadCount++;
    }
    threads.push({
      thread_id,
      subject: lastMsg.subject || t('no_subject'),
      messages,
      last_date: lastMsg.date,
      participants: [...participantSet],
      unread_count: unreadCount,
      is_expanded: false,
    });
  }

  // Sort threads by most recent message date descending
  threads.sort((a, b) => new Date(b.last_date) - new Date(a.last_date));
  _cachedThreadKey = key;
  _cachedThreads = threads;
  return threads;
}
