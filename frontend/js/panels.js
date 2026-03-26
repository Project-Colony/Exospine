// Exospine — Resizable panels and connection status

import * as api from './api.js';
import { _intervals } from './state.js';
import { lsSetItem } from './offline.js';

// ---------------------------------------------------------------------------
// Connection status indicator
// ---------------------------------------------------------------------------
export function setupConnectionStatus() {
  const dot = document.getElementById('connection-dot');
  const text = document.getElementById('connection-text');
  if (!dot || !text) return;

  function updateStatus(online) {
    if (online) {
      dot.className = 'connection-dot connected';
      text.textContent = 'Connected';
    } else {
      dot.className = 'connection-dot disconnected';
      text.textContent = 'Disconnected';
    }
  }

  updateStatus(navigator.onLine);
  window.addEventListener('online', () => updateStatus(true));
  window.addEventListener('offline', () => updateStatus(false));

  // Periodic health check every 30 seconds
  _intervals.push(setInterval(async () => {
    try {
      const result = await api.getAccounts();
      updateStatus(Array.isArray(result));
    } catch {
      updateStatus(false);
    }
  }, 30000));
}

// ---------------------------------------------------------------------------
// Resizable panels
// ---------------------------------------------------------------------------
export function setupResizablePanels() {
  const sidebar = document.getElementById('sidebar');
  const mailList = document.getElementById('mail-list');
  const content = document.getElementById('content');
  if (!sidebar || !mailList || !content) return;

  // Restore saved widths
  const savedSidebarW = localStorage.getItem('exospine_sidebar_width');
  const savedListW = localStorage.getItem('exospine_list_width');
  if (savedSidebarW) {
    sidebar.style.width = savedSidebarW + 'px';
    sidebar.style.minWidth = savedSidebarW + 'px';
  }
  if (savedListW) {
    mailList.style.width = savedListW + 'px';
    mailList.style.minWidth = savedListW + 'px';
  }

  // Create splitters
  const splitter1 = document.createElement('div');
  splitter1.className = 'panel-splitter';
  splitter1.setAttribute('role', 'separator');
  splitter1.setAttribute('aria-label', 'Resize sidebar');
  sidebar.after(splitter1);

  const splitter2 = document.createElement('div');
  splitter2.className = 'panel-splitter';
  splitter2.setAttribute('role', 'separator');
  splitter2.setAttribute('aria-label', 'Resize mail list');
  mailList.after(splitter2);

  function makeDraggable(splitter, targetEl, minW, maxW, storageKey) {
    let startX, startW;
    const onMouseMove = (e) => {
      const delta = e.clientX - startX;
      const newW = Math.min(maxW, Math.max(minW, startW + delta));
      targetEl.style.width = newW + 'px';
      targetEl.style.minWidth = newW + 'px';
    };
    const onMouseUp = () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      document.body.classList.remove('col-resizing');
      lsSetItem(storageKey, String(parseInt(targetEl.style.width, 10)));
    };
    splitter.addEventListener('mousedown', (e) => {
      e.preventDefault();
      startX = e.clientX;
      startW = targetEl.getBoundingClientRect().width;
      document.body.classList.add('col-resizing');
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });
  }

  makeDraggable(splitter1, sidebar, 120, 350, 'exospine_sidebar_width');
  makeDraggable(splitter2, mailList, 200, 600, 'exospine_list_width');
}
