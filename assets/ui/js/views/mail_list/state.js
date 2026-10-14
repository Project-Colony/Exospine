// Exospine: what the mail list remembers between renders.

export const SORT_OPTIONS = [
  { key: 'date-desc', label: 'sort_date_desc' },
  { key: 'date-asc', label: 'sort_date_asc' },
  { key: 'sender-az', label: 'sort_sender' },
  { key: 'subject-az', label: 'sort_subject' },
  { key: 'priority', label: 'sort_priority' },
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
