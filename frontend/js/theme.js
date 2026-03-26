// Exospine — Theme management

import { _intervals } from './state.js';

// ---------------------------------------------------------------------------
// Auto dark mode based on system time
// ---------------------------------------------------------------------------
let _autoThemeInterval = null;

export function applyAutoTheme() {
  const hour = new Date().getHours();
  const isDark = hour < 7 || hour >= 20; // Dark between 8pm-7am
  document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  if (isDark) {
    document.documentElement.removeAttribute('data-theme');
  } else {
    document.documentElement.setAttribute('data-theme', 'light');
  }
}

export function startAutoTheme() {
  stopAutoTheme();
  applyAutoTheme();
  _autoThemeInterval = setInterval(applyAutoTheme, 5 * 60 * 1000);
  _intervals.push(_autoThemeInterval);
}

export function stopAutoTheme() {
  if (_autoThemeInterval) {
    clearInterval(_autoThemeInterval);
    _autoThemeInterval = null;
  }
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------
export function applyTheme(theme) {
  stopAutoTheme();
  clearCustomTheme();
  if (theme === 'auto') {
    startAutoTheme();
  } else if (theme === 'light') {
    document.documentElement.setAttribute('data-theme', 'light');
  } else if (theme === 'high-contrast') {
    document.documentElement.setAttribute('data-theme', 'high-contrast');
  } else if (theme === 'custom') {
    document.documentElement.removeAttribute('data-theme');
    // Read and apply custom theme colors from localStorage
    applyCustomTheme();
  } else {
    document.documentElement.removeAttribute('data-theme');
  }
}

/**
 * Apply custom theme colors from localStorage onto :root CSS variables.
 */
export function applyCustomTheme() {
  try {
    const raw = localStorage.getItem('exospine_custom_theme');
    if (!raw) return;
    const colors = JSON.parse(raw);
    const root = document.documentElement;
    if (colors.sidebarBg) root.style.setProperty('--sidebar-bg', colors.sidebarBg);
    if (colors.listBg) root.style.setProperty('--list-bg', colors.listBg);
    if (colors.paneBg) root.style.setProperty('--pane-bg', colors.paneBg);
    if (colors.accent) {
      root.style.setProperty('--accent', colors.accent);
      root.style.setProperty('--list-active', colors.accent);
    }
    if (colors.textPrimary) {
      root.style.setProperty('--pane-text', colors.textPrimary);
      root.style.setProperty('--list-text', colors.textPrimary);
    }
    if (colors.textSecondary) {
      root.style.setProperty('--pane-text-dim', colors.textSecondary);
      root.style.setProperty('--list-text-dim', colors.textSecondary);
      root.style.setProperty('--sidebar-text-dim', colors.textSecondary);
    }
  } catch {
    // Ignore parse errors
  }
}

/**
 * Clear custom theme CSS variable overrides from :root inline styles.
 */
export function clearCustomTheme() {
  const root = document.documentElement;
  const props = ['--sidebar-bg', '--list-bg', '--pane-bg', '--accent', '--list-active',
    '--pane-text', '--list-text', '--pane-text-dim', '--list-text-dim', '--sidebar-text-dim'];
  for (const p of props) {
    root.style.removeProperty(p);
  }
}
