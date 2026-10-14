// Exospine: what the mail list remembers between renders.

export const SORT_OPTIONS = [
  { key: 'date-desc', label: 'Date (newest)' },
  { key: 'date-asc', label: 'Date (oldest)' },
  { key: 'sender-az', label: 'Sender A-Z' },
  { key: 'subject-az', label: 'Subject A-Z' },
  { key: 'priority', label: 'Priority' },
];

export const list = {
  // The state and actions of the last full render, reused by scroll renders.
  lastState: null,
  lastActions: null,
  // Multi-select
  selectedIds: new Set(),
  currentSort: (() => {
    const saved = localStorage.getItem('exospine_mail_sort');
    return (saved && SORT_OPTIONS.some(s => s.key === saved)) ? saved : 'date-desc';
  })(),
  // Duplicate detection
  duplicateIds: new Set(), // IDs of messages that are duplicates
  duplicatePairs: [], // array of { original_id, duplicate_id }
  duplicateMode: false,
  // Thread view
  threadViewEnabled: false,
  expandedThreads: new Set(),
};
