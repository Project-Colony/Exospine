// Exospine: what the reading pane remembers from one mail to the next.

// Muted threads (persisted in localStorage, cached in memory)
let _mutedThreadsCache = null; // cached Set of muted thread IDs

export function getMutedThreads() {
  if (_mutedThreadsCache) return _mutedThreadsCache;
  try {
    const raw = localStorage.getItem('exospine_muted_threads');
    _mutedThreadsCache = new Set(raw ? JSON.parse(raw) : []);
  } catch {
    _mutedThreadsCache = new Set();
  }
  return _mutedThreadsCache;
}
export function setMutedThreads(ids) {
  const arr = Array.isArray(ids) ? ids : [...ids];
  _mutedThreadsCache = new Set(arr);
  localStorage.setItem('exospine_muted_threads', JSON.stringify(arr));
}
export function isThreadMuted(threadId) {
  return getMutedThreads().has(threadId);
}

// Reminder timers (in-memory, cleared on page reload)
export const reminderTimers = new Map();

// Zoom (persisted across mail selections)
let _zoomLevel = parseFloat(localStorage.getItem('exospine_zoom') || '100');
let _zoomSaveTimer = null;

export function getZoom() {
  return _zoomLevel;
}
export function setZoom(level) {
  _zoomLevel = level;
}
export function saveZoomDebounced(val) {
  clearTimeout(_zoomSaveTimer);
  _zoomSaveTimer = setTimeout(() => localStorage.setItem('exospine_zoom', String(val)), 1000);
}
export function cancelZoomSave() {
  if (_zoomSaveTimer) { clearTimeout(_zoomSaveTimer); _zoomSaveTimer = null; }
}

export const dismissedReceipts = new Set(); // Track dismissed read receipts per session
export const expandedQuoteIds = new Set(); // Track expanded quoted text blocks across re-renders
