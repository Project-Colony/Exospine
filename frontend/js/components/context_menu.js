// Exospine — Context menu component

let _activeMenu = null;

/**
 * Show a context menu at the given position.
 * @param {number} x  Mouse X coordinate
 * @param {number} y  Mouse Y coordinate
 * @param {Array<{label: string, action: Function, danger?: boolean, separator?: boolean}>} items
 */
export function showContextMenu(x, y, items) {
  closeContextMenu();

  const menu = document.createElement('div');
  menu.className = 'context-menu';
  menu.setAttribute('role', 'menu');
  menu.setAttribute('aria-label', 'Context menu');

  for (const item of items) {
    if (item.separator) {
      const sep = document.createElement('div');
      sep.className = 'context-menu-separator';
      sep.setAttribute('role', 'separator');
      menu.appendChild(sep);
      continue;
    }

    const btn = document.createElement('div');
    btn.className = 'context-menu-item' + (item.danger ? ' danger' : '');
    btn.setAttribute('role', 'menuitem');
    btn.setAttribute('tabindex', '0');
    btn.textContent = item.label;

    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      closeContextMenu();
      if (item.action) item.action();
    });

    btn.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        closeContextMenu();
        if (item.action) item.action();
      }
    });

    menu.appendChild(btn);
  }

  document.body.appendChild(menu);

  // Position: ensure it stays within the viewport
  const rect = menu.getBoundingClientRect();
  const maxX = window.innerWidth - rect.width - 4;
  const maxY = window.innerHeight - rect.height - 4;
  menu.style.left = Math.min(x, maxX) + 'px';
  menu.style.top = Math.min(y, maxY) + 'px';

  _activeMenu = menu;

  // Close on click outside
  setTimeout(() => {
    document.addEventListener('click', _onOutsideClick);
    document.addEventListener('contextmenu', _onOutsideClick);
    document.addEventListener('keydown', _onEscape);
  }, 0);

  // Focus the first item
  const firstItem = menu.querySelector('.context-menu-item');
  if (firstItem) firstItem.focus();
}

/**
 * Close the active context menu if any.
 */
export function closeContextMenu() {
  if (_activeMenu) {
    _activeMenu.remove();
    _activeMenu = null;
  }
  document.removeEventListener('click', _onOutsideClick);
  document.removeEventListener('contextmenu', _onOutsideClick);
  document.removeEventListener('keydown', _onEscape);
}

function _onOutsideClick(e) {
  if (_activeMenu && !_activeMenu.contains(e.target)) {
    closeContextMenu();
  }
}

function _onEscape(e) {
  if (e.key === 'Escape') {
    closeContextMenu();
  }
}
