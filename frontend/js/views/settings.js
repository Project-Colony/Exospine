// Exospine — Settings panel view

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { showDialog } from '../components/dialog.js';
import { t } from '../i18n.js';

const overlay = document.getElementById('settings-overlay');

// ===== SECTION: Main Settings Dialog =====
/**
 * Open the settings panel.
 * @param {object} state  app state
 * @param {object} actions  { onSaved, onAccountRemoved }
 */
export async function openSettings(state, actions) {
  let settings;
  try {
    settings = await api.getSettings();
  } catch (err) {
    // Use defaults if backend not ready
    settings = {
      theme: 'dark',
      font_size: 14,
      check_interval: 5,
      notifications: true,
      reading_pane: 'right',
      language: 'en',
    };
  }

  overlay.hidden = false;

  overlay.innerHTML = `
    <div class="overlay-panel settings-panel">
      <div class="settings-header">
        <span class="settings-title">${t('settings_title')}</span>
        <button class="compose-close" id="settings-close" title="Close" aria-label="Close settings">\u00D7</button>
      </div>
      <div class="settings-body">
        <div class="settings-section">
          <div class="settings-section-title">${t('appearance')}</div>
          <div class="settings-row">
            <label>${t('theme')}</label>
            <select id="s-theme">
              <option value="dark" ${settings.theme === 'dark' ? 'selected' : ''}>${t('dark')}</option>
              <option value="light" ${settings.theme === 'light' ? 'selected' : ''}>${t('light')}</option>
              <option value="auto" ${settings.theme === 'auto' ? 'selected' : ''}>Auto</option>
              <option value="high-contrast" ${settings.theme === 'high-contrast' ? 'selected' : ''}>${t('high_contrast')}</option>
              <option value="custom" ${settings.theme === 'custom' ? 'selected' : ''}>Custom</option>
            </select>
          </div>
          <div id="s-custom-theme-section" ${settings.theme !== 'custom' ? 'hidden' : ''} class="settings-rule-editor">
            ${(() => {
              const ct = loadCustomThemeColors();
              return `
              <div class="settings-row settings-row-mb6">
                <label>Sidebar background</label>
                <input type="color" id="s-ct-sidebar-bg" value="${ct.sidebarBg || '#242933'}" class="settings-color-input" />
              </div>
              <div class="settings-row settings-row-mb6">
                <label>Mail list background</label>
                <input type="color" id="s-ct-list-bg" value="${ct.listBg || '#2a2f3a'}" class="settings-color-input" />
              </div>
              <div class="settings-row settings-row-mb6">
                <label>Reading pane background</label>
                <input type="color" id="s-ct-pane-bg" value="${ct.paneBg || '#ffffff'}" class="settings-color-input" />
              </div>
              <div class="settings-row settings-row-mb6">
                <label>Accent color</label>
                <input type="color" id="s-ct-accent" value="${ct.accent || '#0078d6'}" class="settings-color-input" />
              </div>
              <div class="settings-row settings-row-mb6">
                <label>Text primary</label>
                <input type="color" id="s-ct-text-primary" value="${ct.textPrimary || '#1e1e1e'}" class="settings-color-input" />
              </div>
              <div class="settings-row settings-row-mb6">
                <label>Text secondary</label>
                <input type="color" id="s-ct-text-secondary" value="${ct.textSecondary || '#646973'}" class="settings-color-input" />
              </div>
              `;
            })()}
          </div>
          <div class="settings-row">
            <label>${t('font_size')}</label>
            <input type="number" id="s-fontsize" min="10" max="22" value="${settings.font_size || 14}" />
          </div>
          <div class="settings-row">
            <label>${t('reading_pane')}</label>
            <select id="s-pane">
              <option value="right" ${settings.reading_pane === 'right' ? 'selected' : ''}>${t('right')}</option>
              <option value="bottom" ${settings.reading_pane === 'bottom' ? 'selected' : ''}>${t('bottom')}</option>
            </select>
          </div>
          <div class="settings-row" style="flex-direction:column;align-items:flex-start;gap:8px;">
            <label>Import CSS Theme</label>
            <div class="flex-row gap-6" style="flex-wrap:wrap;">
              <button class="btn btn-ghost btn-sm" id="s-import-theme">Import .css Theme</button>
              <input type="file" id="s-import-theme-file" accept=".css" hidden />
              ${localStorage.getItem('exospine_imported_theme_css') ? '<button class="btn btn-ghost btn-sm text-danger" id="s-remove-imported-theme">Remove imported theme</button>' : ''}
            </div>
            ${localStorage.getItem('exospine_imported_theme_css') ? '<div class="text-xxs text-dim mt-4">An imported CSS theme is currently active.</div>' : ''}
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('behavior')}</div>
          <div class="settings-row">
            <label>${t('check_interval')}</label>
            <input type="number" id="s-interval" min="1" max="60" value="${settings.check_interval || 5}" />
          </div>
          <div class="settings-row">
            <label>${t('notifications')}</label>
            <label class="settings-toggle">
              <input type="checkbox" id="s-notif" ${settings.notifications !== false ? 'checked' : ''} />
              <span class="settings-toggle-track"></span>
            </label>
          </div>
          <div class="settings-row">
            <label>Sound notifications</label>
            <label class="settings-toggle">
              <input type="checkbox" id="s-sound-notif" ${localStorage.getItem('exospine_sound_notifications') !== 'false' ? 'checked' : ''} />
              <span class="settings-toggle-track"></span>
            </label>
          </div>
          <div class="settings-row">
            <label>Start with Windows</label>
            <label class="settings-toggle">
              <input type="checkbox" id="s-autostart" ${localStorage.getItem('exospine_autostart') === 'true' ? 'checked' : ''} />
              <span class="settings-toggle-track"></span>
            </label>
          </div>
          <div class="settings-row">
            <label>Notification sound</label>
            <div class="flex-row gap-6">
              <select id="s-notif-sound">
                ${['default','chime','bell','gentle','silent'].map(s => `<option value="${s}" ${(localStorage.getItem('exospine_notif_sound') || 'default') === s ? 'selected' : ''}>${s[0].toUpperCase() + s.slice(1)}</option>`).join('')}
              </select>
              <button class="btn btn-ghost btn-sm" id="s-test-sound">Test</button>
            </div>
          </div>
          <div class="settings-row">
            <label>${t('language')}</label>
            <select id="s-lang">
              <option value="en" ${settings.language === 'en' ? 'selected' : ''}>English</option>
              <option value="fr" ${settings.language === 'fr' ? 'selected' : ''}>Fran\u00E7ais</option>
              <option value="de" ${settings.language === 'de' ? 'selected' : ''}>Deutsch</option>
              <option value="ja" ${settings.language === 'ja' ? 'selected' : ''}>Japanese</option>
            </select>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Sidebar Widgets</div>
          ${(() => {
            const wPrefs = JSON.parse(localStorage.getItem('exospine_widget_prefs') || '{}');
            return ['clock', 'date', 'unread'].map(w => {
              const enabled = wPrefs[w] !== false;
              const label = w === 'clock' ? 'Clock' : w === 'date' ? 'Date' : 'Unread Count';
              return `<div class="settings-row">
                <label>${label}</label>
                <label class="settings-toggle">
                  <input type="checkbox" class="s-widget-toggle" data-widget="${w}" ${enabled ? 'checked' : ''} />
                  <span class="settings-toggle-track"></span>
                </label>
              </div>`;
            }).join('');
          })()}
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('security')}</div>
          <div class="settings-row">
            <label>${t('audit_log')}</label>
            <button class="btn btn-ghost btn-sm" id="s-view-security-log">${t('view_security_log')}</button>
          </div>
          <div id="s-security-log-container" class="settings-security-log" hidden>
            <div class="settings-security-log-list" id="s-security-log-list"></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('signatures')}</div>
          <div id="s-signatures">
            ${state.accounts.map((acc) => `
              <div class="settings-signature-item mb-12">
                <label class="font-bold mb-4" style="display:block;">${esc(acc.email || acc.name)}</label>
                <textarea class="settings-sig-textarea" data-sig-account="${esc(acc.id)}" rows="4" placeholder="Enter your signature...">${esc(acc.signature || '')}</textarea>
              </div>
            `).join('')}
            ${state.accounts.length === 0 ? '<div class="text-dim text-sm">No accounts configured.</div>' : ''}
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('email_rules')}</div>
          <div id="s-rules-list" class="mb-8"></div>
          <button class="btn btn-ghost btn-sm" id="s-add-rule">+ Add Rule</button>
          <div id="s-rule-editor" hidden class="settings-rule-editor">
            <input type="hidden" id="s-rule-id" value="" />
            <div class="settings-row settings-row-mb6">
              <label>Name</label>
              <input type="text" id="s-rule-name" placeholder="Rule name" class="settings-text-input w-full" />
            </div>
            <div class="settings-row settings-row-mb6">
              <label>Condition</label>
              <select id="s-rule-cond-type" class="settings-select">
                <option value="FromContains">From contains</option>
                <option value="SubjectContains">Subject contains</option>
                <option value="ToContains">To contains</option>
                <option value="HasAttachment">Has attachment</option>
              </select>
              <input type="text" id="s-rule-cond-value" placeholder="value" class="settings-text-input ml-4" />
            </div>
            <div class="settings-row settings-row-mb6">
              <label>Action</label>
              <select id="s-rule-action-type" class="settings-select">
                <option value="MarkAsRead">Mark as read</option>
                <option value="Star">Star</option>
                <option value="MoveToFolder">Move to folder</option>
                <option value="Delete">Delete</option>
                <option value="AddCategory">Add category</option>
              </select>
              <input type="text" id="s-rule-action-value" placeholder="folder/category" class="settings-text-input ml-4" />
            </div>
            <div class="flex-row gap-6">
              <button class="btn btn-primary btn-sm" id="s-rule-save">Save Rule</button>
              <button class="btn btn-ghost btn-sm" id="s-rule-cancel">Cancel</button>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Display Rules</div>
          <div id="s-display-rules-list" class="mb-8"></div>
          <button class="btn btn-ghost btn-sm" id="s-add-display-rule">+ Add Display Rule</button>
          <div id="s-display-rule-editor" hidden class="settings-rule-editor">
            <div class="settings-row settings-row-mb6">
              <label>Field</label>
              <select id="s-dr-field" class="settings-select">
                <option value="from">From</option>
                <option value="subject">Subject</option>
              </select>
            </div>
            <div class="settings-row settings-row-mb6">
              <label>Contains</label>
              <input type="text" id="s-dr-value" placeholder="text to match" class="settings-text-input w-full" />
            </div>
            <div class="settings-row settings-row-mb6">
              <label>Style</label>
              <select id="s-dr-style" class="settings-select">
                <option value="highlight">Highlight row</option>
                <option value="bold">Bold text</option>
                <option value="italic">Italic text</option>
              </select>
              <input type="color" id="s-dr-color" value="#ff6b6b" class="settings-color-input ml-6" title="Highlight color" />
            </div>
            <div class="flex-row gap-6">
              <button class="btn btn-primary btn-sm" id="s-dr-save">Save</button>
              <button class="btn btn-ghost btn-sm" id="s-dr-cancel">Cancel</button>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('data_retention')}</div>
          <div class="settings-row">
            <label>${t('auto_delete_label')}</label>
            <input type="number" id="s-retention-days" min="0" max="3650" value="${parseInt(localStorage.getItem('exospine_retention_days') || '0', 10)}" style="width:80px;" />
            <span class="text-xxs text-dim ml-8">${t('auto_delete_hint')}</span>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Keyboard Shortcuts</div>
          <div id="s-shortcuts-list" class="mb-8">
            ${renderShortcutsList()}
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Email Templates</div>
          <div class="settings-subsection">
            <div class="text-sm font-bold mb-6">Your Templates</div>
            <div id="s-user-templates-list" class="mb-8"></div>
          </div>
          <div class="settings-subsection">
            <div class="text-sm font-bold mb-6">Community Templates</div>
            <div id="s-community-templates-list" class="mb-8"></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Export / Import Settings</div>
          <div class="flex-row gap-8" style="flex-wrap:wrap;">
            <button class="btn btn-ghost btn-sm" id="s-export-settings">Export Settings</button>
            <button class="btn btn-ghost btn-sm" id="s-import-settings">Import Settings</button>
            <input type="file" id="s-import-file" accept=".json" hidden />
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Security</div>
          <div class="settings-row">
            <label>App PIN Lock</label>
            <button class="btn btn-ghost btn-sm" id="s-set-pin">${localStorage.getItem('exospine_pin_hash') ? 'Change PIN' : 'Set PIN'}</button>
            ${localStorage.getItem('exospine_pin_hash') ? '<button class="btn btn-ghost btn-sm text-danger ml-6" id="s-remove-pin">Remove PIN</button>' : ''}
          </div>
          <div class="settings-row mt-8">
            <label>Secure Wipe</label>
            <button class="btn btn-sm btn-danger" id="s-secure-wipe" style="font-weight:600;">Wipe All Data</button>
          </div>
          <div class="text-xxs text-dim mt-4">
            Permanently deletes all messages, accounts, and credentials. This cannot be undone.
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('accounts')}</div>
          <div class="settings-account-list" id="s-accounts">
            ${state.accounts.map((acc, i) => {
              const acctSoundKey = 'exospine_notif_sound_' + (acc.id || String(i));
              const acctSound = localStorage.getItem(acctSoundKey) || '';
              return `
              <div class="settings-account-item" style="flex-direction:column;align-items:stretch;gap:6px;">
                <div class="flex-row-center" style="justify-content:space-between;">
                  <span>${esc(acc.email || acc.name || 'Account ' + (i + 1))}</span>
                  <button class="btn btn-danger btn-sm" data-remove-account="${esc(acc.id || String(i))}">Remove</button>
                </div>
                <div class="flex-row-center gap-6" style="padding-left:8px;">
                  <label class="text-xxs text-dim">Notification sound:</label>
                  <select class="s-acct-sound" data-acct-sound-id="${esc(acc.id || String(i))}" style="font-size:12px;">
                    <option value="" ${acctSound === '' ? 'selected' : ''}>Global default</option>
                    <option value="default" ${acctSound === 'default' ? 'selected' : ''}>Default beep</option>
                    <option value="chime" ${acctSound === 'chime' ? 'selected' : ''}>Chime</option>
                    <option value="bell" ${acctSound === 'bell' ? 'selected' : ''}>Bell</option>
                    <option value="gentle" ${acctSound === 'gentle' ? 'selected' : ''}>Gentle</option>
                    <option value="silent" ${acctSound === 'silent' ? 'selected' : ''}>Silent</option>
                  </select>
                </div>
              </div>
              `;
            }).join('')}
            ${state.accounts.length === 0 ? '<div class="text-dim text-sm">No accounts configured.</div>' : ''}
          </div>
        </div>
        <div class="settings-section settings-section-about">
          <div class="font-bolder mb-4" style="font-size:18px;">Exospine v0.2.0</div>
          <div class="text-sm text-dim mb-6">Built with Rust + Tauri</div>
          <div class="mb-6"><a href="https://github.com/MotherSphere/Exospine-Private" target="_blank" rel="noopener" class="text-accent text-sm">GitHub Repository</a></div>
          <div class="text-xs text-dim">&copy; 2026 MotherSphere</div>
        </div>
      </div>
      <div class="settings-footer">
        <button class="btn btn-ghost" id="settings-cancel">${t('close')}</button>
        <button class="btn btn-primary" id="settings-save">${t('save')}</button>
      </div>
    </div>
  `;

  const close = () => {
    overlay.hidden = true;
    overlay.innerHTML = '';
  };

  overlay.querySelector('#settings-close').addEventListener('click', close);
  overlay.querySelector('#settings-cancel').addEventListener('click', close);

  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

  const onKey = (e) => {
    if (e.key === 'Escape') {
      close();
      document.removeEventListener('keydown', onKey);
    }
  };
  document.addEventListener('keydown', onKey);

  // Security log
  overlay.querySelector('#s-view-security-log').addEventListener('click', async () => {
    const container = overlay.querySelector('#s-security-log-container');
    const list = overlay.querySelector('#s-security-log-list');
    if (!container.hidden) {
      container.hidden = true;
      return;
    }
    try {
      const entries = await api.getSecurityLog(20);
      if (entries.length === 0) {
        list.innerHTML = '<div class="text-dim text-sm" style="padding:8px 0;">No security events recorded.</div>';
      } else {
        list.innerHTML = entries.map((entry) => {
          const escaped = esc(entry);
          // Highlight event type
          const highlighted = escaped.replace(
            /\] (\w+) \|/,
            '] <strong class="text-accent">$1</strong> |'
          );
          return `<div class="settings-security-log-entry">${highlighted}</div>`;
        }).join('');
      }
      container.hidden = false;
    } catch (err) {
      list.innerHTML = `<div class="text-danger text-sm" style="padding:8px 0;">Failed to load log: ${esc(String(err))}</div>`;
      container.hidden = false;
    }
  });

  // ── Set PIN ──────────────────────────────────────────────────────
  overlay.querySelector('#s-set-pin')?.addEventListener('click', async () => {
    const pin = prompt('Enter a 4-6 digit PIN:');
    if (!pin || !/^\d{4,6}$/.test(pin)) {
      showToast('PIN must be 4-6 digits.', 'error');
      return;
    }
    const confirm = prompt('Confirm PIN:');
    if (pin !== confirm) {
      showToast('PINs do not match.', 'error');
      return;
    }
    // Hash the PIN using a simple SHA-256 via SubtleCrypto
    const hash = await hashPin(pin);
    localStorage.setItem('exospine_pin_hash', hash);
    localStorage.removeItem('exospine_pin_attempts');
    showToast('PIN set successfully. You will be asked for it on next startup.', 'success');
  });

  overlay.querySelector('#s-remove-pin')?.addEventListener('click', () => {
    localStorage.removeItem('exospine_pin_hash');
    localStorage.removeItem('exospine_pin_attempts');
    showToast('PIN removed.', 'info');
  });

  // ── Secure Wipe ─────────────────────────────────────────────────
  overlay.querySelector('#s-secure-wipe')?.addEventListener('click', async () => {
    const confirmed = await showDialog({
      title: 'SECURE WIPE',
      message: 'This will PERMANENTLY DELETE all your emails, accounts, credentials, and settings. This action CANNOT be undone.\n\nAre you absolutely sure?',
      confirmLabel: 'WIPE EVERYTHING',
      danger: true,
    });
    if (!confirmed) return;

    // Double confirmation
    const doubleConfirm = await showDialog({
      title: 'Final Confirmation',
      message: 'Last chance. All data will be destroyed.',
      confirmLabel: 'Yes, wipe all data',
      danger: true,
    });
    if (!doubleConfirm) return;

    try {
      await api.secureWipe();
      // Clear all localStorage
      const keysToRemove = [];
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key && key.startsWith('exospine_')) keysToRemove.push(key);
      }
      keysToRemove.forEach((k) => localStorage.removeItem(k));
      showToast('All data wiped. Restarting...', 'success');
      setTimeout(() => location.reload(), 2000);
    } catch (err) {
      showToast(`Wipe failed: ${err}`, 'error');
    }
  });

  // ── Custom theme: show/hide section when theme selector changes ──
  overlay.querySelector('#s-theme').addEventListener('change', (e) => {
    const section = overlay.querySelector('#s-custom-theme-section');
    if (section) section.hidden = e.target.value !== 'custom';
  });

  // ── Keyboard shortcuts: change binding ──────────────────────────
  overlay.querySelectorAll('[data-shortcut-change]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const action = btn.dataset.shortcutChange;
      btn.textContent = 'Press a key...';
      btn.disabled = true;

      const handler = (e) => {
        e.preventDefault();
        e.stopPropagation();

        // Build key descriptor
        let keyDesc = '';
        if (e.ctrlKey || e.metaKey) keyDesc += 'Ctrl+';
        if (e.shiftKey) keyDesc += 'Shift+';
        if (e.altKey) keyDesc += 'Alt+';
        // Ignore modifier-only presses
        if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
        keyDesc += e.key;

        // Check for conflicts with existing shortcuts
        const merged = getMergedShortcuts();
        const conflictAction = Object.entries(merged).find(
          ([a, k]) => a !== action && k === keyDesc
        );
        if (conflictAction) {
          const labels = {
            compose: 'Compose', reply: 'Reply', replyAll: 'Reply All',
            forward: 'Forward', delete: 'Delete', archive: 'Archive',
            star: 'Star', unread: 'Mark Unread', next: 'Next Email',
            prev: 'Previous Email', refresh: 'Refresh', focusMode: 'Focus Mode',
          };
          const conflictLabel = labels[conflictAction[0]] || conflictAction[0];
          showToast(`Warning: "${keyDesc}" is already used by "${conflictLabel}". Overriding.`, 'warning');
        }

        const bindings = loadCustomKeybindings();
        bindings[action] = keyDesc;
        saveCustomKeybindings(bindings);

        // Update displayed key
        const keySpan = overlay.querySelector(`[data-shortcut-key="${action}"]`);
        if (keySpan) keySpan.textContent = keyDesc;

        btn.textContent = 'Change';
        btn.disabled = false;
        document.removeEventListener('keydown', handler, true);
        showToast(`Shortcut for "${action}" set to ${keyDesc}`, 'success');
      };

      document.addEventListener('keydown', handler, true);
    });
  });

  // ── Export / Import Settings ────────────────────────────────────
  overlay.querySelector('#s-export-settings').addEventListener('click', async () => {
    try {
      const exportData = {
        version: 1,
        exportedAt: new Date().toISOString(),
        settings: {
          theme: overlay.querySelector('#s-theme').value,
          font_size: parseInt(overlay.querySelector('#s-fontsize').value, 10) || 14,
          check_interval: parseInt(overlay.querySelector('#s-interval').value, 10) || 5,
          notifications: overlay.querySelector('#s-notif').checked,
          reading_pane: overlay.querySelector('#s-pane').value,
          language: overlay.querySelector('#s-lang').value,
        },
        customTheme: loadCustomThemeColors(),
        keybindings: loadCustomKeybindings(),
        displayRules: loadDisplayRules(),
        retentionDays: parseInt(localStorage.getItem('exospine_retention_days') || '0', 10),
        customTemplates: JSON.parse(localStorage.getItem('exospine_templates') || '[]'),
        searchFolders: JSON.parse(localStorage.getItem('exospine_search_folders') || '[]'),
      };

      // Try to include email rules from backend
      try {
        const rules = await api.getRules();
        exportData.emailRules = rules || [];
      } catch {
        exportData.emailRules = [];
      }

      const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `exospine-settings-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
      showToast('Settings exported.', 'success');
    } catch (err) {
      showToast(`Export failed: ${err}`, 'error');
    }
  });

  overlay.querySelector('#s-import-settings').addEventListener('click', () => {
    overlay.querySelector('#s-import-file').click();
  });

  overlay.querySelector('#s-import-file').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (!file) return;

    try {
      const text = await file.text();
      const data = JSON.parse(text);

      if (!data.version || !data.settings) {
        showToast('Invalid settings file.', 'error');
        return;
      }

      // Apply settings to backend
      await api.saveSettings(data.settings);

      // Apply custom theme
      if (data.customTheme) {
        saveCustomThemeColors(data.customTheme);
      }

      // Apply keybindings
      if (data.keybindings) {
        saveCustomKeybindings(data.keybindings);
      }

      // Apply display rules
      if (data.displayRules) {
        localStorage.setItem(DISPLAY_RULES_KEY, JSON.stringify(data.displayRules));
      }

      // Apply retention days
      if (data.retentionDays !== undefined) {
        localStorage.setItem('exospine_retention_days', String(data.retentionDays));
      }

      // Apply custom templates
      if (data.customTemplates) {
        localStorage.setItem('exospine_templates', JSON.stringify(data.customTemplates));
      }

      // Apply search folders
      if (data.searchFolders) {
        localStorage.setItem('exospine_search_folders', JSON.stringify(data.searchFolders));
      }

      // Apply email rules to backend
      if (data.emailRules && Array.isArray(data.emailRules)) {
        for (const rule of data.emailRules) {
          try {
            await api.saveRule(rule);
          } catch {
            // Skip failed rules
          }
        }
      }

      showToast('Settings imported. Please reload to apply all changes.', 'success');
      close();
      if (actions.onSaved) actions.onSaved(data.settings);
    } catch (err) {
      showToast(`Import failed: ${err}`, 'error');
    }

    // Reset input
    e.target.value = '';
  });

  // ── Rules management ──────────────────────────────────────────────
  loadAndRenderRules();

  overlay.querySelector('#s-add-rule').addEventListener('click', () => {
    overlay.querySelector('#s-rule-id').value = '';
    overlay.querySelector('#s-rule-name').value = '';
    overlay.querySelector('#s-rule-cond-type').value = 'FromContains';
    overlay.querySelector('#s-rule-cond-value').value = '';
    overlay.querySelector('#s-rule-action-type').value = 'MarkAsRead';
    overlay.querySelector('#s-rule-action-value').value = '';
    overlay.querySelector('#s-rule-editor').hidden = false;
  });

  overlay.querySelector('#s-rule-cancel').addEventListener('click', () => {
    overlay.querySelector('#s-rule-editor').hidden = true;
  });

  overlay.querySelector('#s-rule-save').addEventListener('click', async () => {
    const ruleId = overlay.querySelector('#s-rule-id').value || crypto.randomUUID();
    const name = overlay.querySelector('#s-rule-name').value.trim() || 'Untitled rule';
    const condType = overlay.querySelector('#s-rule-cond-type').value;
    const condValue = overlay.querySelector('#s-rule-cond-value').value.trim();
    const actionType = overlay.querySelector('#s-rule-action-type').value;
    const actionValue = overlay.querySelector('#s-rule-action-value').value.trim();

    const condition = condType === 'HasAttachment'
      ? { type: 'HasAttachment' }
      : { type: condType, value: condValue };

    const actionObj = (actionType === 'MarkAsRead' || actionType === 'Star' || actionType === 'Delete')
      ? { type: actionType }
      : { type: actionType, value: actionValue };

    const rule = {
      id: ruleId,
      name,
      conditions: [condition],
      actions: [actionObj],
      enabled: true,
    };

    try {
      await api.saveRule(rule);
      showToast('Rule saved.', 'success');
      overlay.querySelector('#s-rule-editor').hidden = true;
      loadAndRenderRules();
    } catch (err) {
      showToast(`Failed to save rule: ${err}`, 'error');
    }
  });

  async function loadAndRenderRules() {
    const list = overlay.querySelector('#s-rules-list');
    if (!list) return;
    try {
      const rules = await api.getRules();
      if (!rules || rules.length === 0) {
        list.innerHTML = '<div class="text-dim text-sm">No rules configured.</div>';
        return;
      }
      list.innerHTML = rules.map((r) => `
        <div class="settings-rule-item">
          <div>
            <span class="font-bold">${esc(r.name)}</span>
            <span class="text-xs text-dim ml-8">
              ${r.conditions.map((c) => c.type + (c.value ? ': ' + esc(c.value) : '')).join(', ')}
              \u2192 ${r.actions.map((a) => a.type + (a.value ? ': ' + esc(a.value) : '')).join(', ')}
            </span>
          </div>
          <button class="btn btn-danger btn-sm ml-8" data-delete-rule="${esc(r.id)}">Delete</button>
        </div>
      `).join('');

      list.querySelectorAll('[data-delete-rule]').forEach((btn) => {
        btn.addEventListener('click', async () => {
          try {
            await api.deleteRule(btn.dataset.deleteRule);
            showToast('Rule deleted.', 'info');
            loadAndRenderRules();
          } catch (err) {
            showToast(`Failed to delete rule: ${err}`, 'error');
          }
        });
      });
    } catch {
      list.innerHTML = '<div class="text-dim text-sm">Failed to load rules.</div>';
    }
  }

  // ── Display Rules (conditional formatting, stored in localStorage) ──
  loadAndRenderDisplayRules();

  overlay.querySelector('#s-add-display-rule').addEventListener('click', () => {
    overlay.querySelector('#s-dr-field').value = 'from';
    overlay.querySelector('#s-dr-value').value = '';
    overlay.querySelector('#s-dr-style').value = 'highlight';
    overlay.querySelector('#s-dr-color').value = '#ff6b6b';
    overlay.querySelector('#s-display-rule-editor').hidden = false;
  });

  overlay.querySelector('#s-dr-cancel').addEventListener('click', () => {
    overlay.querySelector('#s-display-rule-editor').hidden = true;
  });

  overlay.querySelector('#s-dr-save').addEventListener('click', () => {
    const field = overlay.querySelector('#s-dr-field').value;
    const value = overlay.querySelector('#s-dr-value').value.trim();
    const style = overlay.querySelector('#s-dr-style').value;
    const color = overlay.querySelector('#s-dr-color').value;

    if (!value) {
      showToast('Please enter a value to match.', 'error');
      return;
    }

    const rules = loadDisplayRules();
    rules.push({ id: crypto.randomUUID(), field, value, style, color });
    saveDisplayRules(rules);
    overlay.querySelector('#s-display-rule-editor').hidden = true;
    loadAndRenderDisplayRules();
    showToast('Display rule added.', 'success');
  });

  function loadAndRenderDisplayRules() {
    const list = overlay.querySelector('#s-display-rules-list');
    if (!list) return;
    const rules = loadDisplayRules();
    if (rules.length === 0) {
      list.innerHTML = '<div class="text-dim text-sm">No display rules configured.</div>';
      return;
    }
    list.innerHTML = rules.map((r) => {
      const styleLabel = r.style === 'highlight' ? `Highlight (${r.color})` : r.style === 'bold' ? 'Bold' : 'Italic';
      const colorSwatch = r.style === 'highlight'
        ? `<span class="dr-color-swatch" style="background:${esc(r.color)}"></span>`
        : '';
      return `
        <div class="settings-rule-item">
          <div class="text-sm">
            ${colorSwatch}If <b>${esc(r.field)}</b> contains "<b>${esc(r.value)}</b>" &rarr; ${styleLabel}
          </div>
          <button class="btn btn-danger btn-sm ml-8" data-delete-display-rule="${esc(r.id)}">Delete</button>
        </div>
      `;
    }).join('');

    list.querySelectorAll('[data-delete-display-rule]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const rules = loadDisplayRules().filter((r) => r.id !== btn.dataset.deleteDisplayRule);
        saveDisplayRules(rules);
        loadAndRenderDisplayRules();
        showToast('Display rule removed.', 'info');
      });
    });
  }

  // ── CSS Theme Import ─────────────────────────────────────────────
  overlay.querySelector('#s-import-theme')?.addEventListener('click', () => {
    overlay.querySelector('#s-import-theme-file').click();
  });

  overlay.querySelector('#s-import-theme-file')?.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    try {
      const cssText = await file.text();
      localStorage.setItem('exospine_imported_theme_css', cssText);
      applyImportedThemeCSS(cssText);
      showToast('CSS theme imported and applied.', 'success');
    } catch (err) {
      showToast(`Failed to import theme: ${err}`, 'error');
    }
    e.target.value = '';
  });

  overlay.querySelector('#s-remove-imported-theme')?.addEventListener('click', () => {
    localStorage.removeItem('exospine_imported_theme_css');
    removeImportedThemeCSS();
    showToast('Imported theme removed.', 'info');
  });

  // ── Notification Sound Picker ──────────────────────────────────────
  overlay.querySelector('#s-notif-sound')?.addEventListener('change', (e) => {
    localStorage.setItem('exospine_notif_sound', e.target.value);
  });

  overlay.querySelector('#s-test-sound')?.addEventListener('click', () => {
    const sound = overlay.querySelector('#s-notif-sound')?.value || 'default';
    playNotifSoundPreview(sound);
  });

  // ── Widget Toggles ─────────────────────────────────────────────────
  overlay.querySelectorAll('.s-widget-toggle').forEach((cb) => {
    cb.addEventListener('change', () => {
      const prefs = JSON.parse(localStorage.getItem('exospine_widget_prefs') || '{}');
      prefs[cb.dataset.widget] = cb.checked;
      localStorage.setItem('exospine_widget_prefs', JSON.stringify(prefs));
    });
  });

  // ── Autostart Toggle ─────────────────────────────────────────────────
  overlay.querySelector('#s-autostart')?.addEventListener('change', async (e) => {
    const enabled = e.target.checked;
    try {
      await api.setAutostart(enabled);
      localStorage.setItem('exospine_autostart', String(enabled));
      showToast(enabled ? 'Exospine will start with Windows.' : 'Autostart disabled.', 'success');
    } catch (err) {
      e.target.checked = !enabled; // revert toggle
      showToast(`Failed to set autostart: ${err}`, 'error');
    }
  });

  // ── Per-Account Notification Sound ──────────────────────────────────
  overlay.querySelectorAll('.s-acct-sound').forEach((sel) => {
    sel.addEventListener('change', () => {
      const acctId = sel.dataset.acctSoundId;
      const val = sel.value;
      if (val) {
        localStorage.setItem('exospine_notif_sound_' + acctId, val);
      } else {
        localStorage.removeItem('exospine_notif_sound_' + acctId);
      }
    });
  });

  // ── Email Templates (User + Community) ─────────────────────────────
  const COMMUNITY_TEMPLATES = [
    {
      id: 'community_thank_you',
      name: 'Professional Thank You',
      subject: 'Thank You',
      body: '<p>Dear [Name],</p><p>Thank you for taking the time to [reason]. I truly appreciate your [effort/time/support].</p><p>Please do not hesitate to reach out if there is anything I can assist you with.</p><p>Best regards,<br/>[Your Name]</p>',
    },
    {
      id: 'community_meeting_request',
      name: 'Meeting Request',
      subject: 'Meeting Request: [Topic]',
      body: '<p>Dear [Name],</p><p>I would like to schedule a meeting to discuss [topic]. Would you be available on [date] at [time]?</p><p>The meeting should take approximately [duration]. Please let me know if this works for you or suggest an alternative time.</p><p>Best regards,<br/>[Your Name]</p>',
    },
    {
      id: 'community_project_update',
      name: 'Project Update',
      subject: 'Project Update: [Project Name]',
      body: '<p>Hi team,</p><p>Here is a quick update on [Project Name]:</p><ul><li><strong>Completed:</strong> [items]</li><li><strong>In Progress:</strong> [items]</li><li><strong>Next Steps:</strong> [items]</li></ul><p>Please reach out if you have any questions.</p><p>Best regards,<br/>[Your Name]</p>',
    },
    {
      id: 'community_invoice_followup',
      name: 'Invoice Follow-up',
      subject: 'Follow-up: Invoice #[Number]',
      body: '<p>Dear [Name],</p><p>I am writing to follow up on Invoice #[Number] dated [date], in the amount of [amount]. According to our records, this invoice is currently outstanding.</p><p>Could you please confirm the status of this payment? If you have already processed it, please disregard this message.</p><p>Thank you for your attention to this matter.</p><p>Best regards,<br/>[Your Name]</p>',
    },
    {
      id: 'community_introduction',
      name: 'Introduction',
      subject: 'Introduction: [Your Name]',
      body: '<p>Dear [Name],</p><p>My name is [Your Name] and I am [your role] at [Company]. I am reaching out because [reason for contact].</p><p>I would love the opportunity to [propose next step]. Would you be open to a brief call or meeting?</p><p>Looking forward to hearing from you.</p><p>Best regards,<br/>[Your Name]</p>',
    },
  ];

  function loadUserTemplates() {
    try { return JSON.parse(localStorage.getItem('exospine_templates') || '[]'); } catch { return []; }
  }
  function saveUserTemplates(templates) {
    localStorage.setItem('exospine_templates', JSON.stringify(templates));
  }

  function renderUserTemplates() {
    const list = overlay.querySelector('#s-user-templates-list');
    if (!list) return;
    const templates = loadUserTemplates();
    if (templates.length === 0) {
      list.innerHTML = '<div class="text-dim text-sm">No custom templates yet. Import from Community Templates below.</div>';
      return;
    }
    list.innerHTML = templates.map((t, i) => `
      <div class="settings-rule-item">
        <div class="text-sm"><b>${esc(t.name)}</b> <span class="text-dim">&mdash; ${esc(t.subject)}</span></div>
        <div class="flex-row gap-4">
          <button class="btn btn-ghost btn-sm" data-export-template="${i}">Export</button>
          <button class="btn btn-danger btn-sm" data-delete-template="${i}">Delete</button>
        </div>
      </div>
    `).join('');

    list.querySelectorAll('[data-delete-template]').forEach(btn => {
      btn.addEventListener('click', () => {
        const templates = loadUserTemplates();
        templates.splice(parseInt(btn.dataset.deleteTemplate, 10), 1);
        saveUserTemplates(templates);
        renderUserTemplates();
        showToast('Template deleted.', 'info');
      });
    });

    list.querySelectorAll('[data-export-template]').forEach(btn => {
      btn.addEventListener('click', () => {
        const templates = loadUserTemplates();
        const tmpl = templates[parseInt(btn.dataset.exportTemplate, 10)];
        if (!tmpl) return;
        const blob = new Blob([JSON.stringify(tmpl, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `template-${(tmpl.name || 'export').replace(/\s+/g, '_').toLowerCase()}.json`;
        a.click();
        URL.revokeObjectURL(url);
        showToast('Template exported.', 'success');
      });
    });
  }

  function renderCommunityTemplates() {
    const list = overlay.querySelector('#s-community-templates-list');
    if (!list) return;
    list.innerHTML = COMMUNITY_TEMPLATES.map((t) => `
      <div class="settings-rule-item">
        <div>
          <div class="text-sm font-bold">${esc(t.name)}</div>
          <div class="text-xxs text-dim">${esc(t.subject)}</div>
        </div>
        <button class="btn btn-ghost btn-sm" data-import-community="${esc(t.id)}">Import</button>
      </div>
    `).join('');

    list.querySelectorAll('[data-import-community]').forEach(btn => {
      btn.addEventListener('click', () => {
        const tmpl = COMMUNITY_TEMPLATES.find(t => t.id === btn.dataset.importCommunity);
        if (!tmpl) return;
        const templates = loadUserTemplates();
        // Avoid duplicates by name
        if (templates.some(t => t.name === tmpl.name)) {
          showToast('Template already imported.', 'info');
          return;
        }
        templates.push({ name: tmpl.name, subject: tmpl.subject, body: tmpl.body });
        saveUserTemplates(templates);
        renderUserTemplates();
        showToast(`"${tmpl.name}" imported to your templates.`, 'success');
      });
    });
  }

  renderUserTemplates();
  renderCommunityTemplates();

  // Save
  overlay.querySelector('#settings-save').addEventListener('click', async () => {
    const intervalMin = parseInt(overlay.querySelector('#s-interval').value, 10) || 5;
    const newSettings = {
      theme: overlay.querySelector('#s-theme').value,
      font_size: parseInt(overlay.querySelector('#s-fontsize').value, 10) || 14,
      check_interval_secs: intervalMin * 60,
      show_notifications: overlay.querySelector('#s-notif').checked,
      reading_pane: overlay.querySelector('#s-pane').value,
      density: 'normal',
      language: overlay.querySelector('#s-lang').value,
    };

    // Save sound notification setting
    const soundEnabled = overlay.querySelector('#s-sound-notif')?.checked !== false;
    localStorage.setItem('exospine_sound_notifications', String(soundEnabled));
    newSettings.sound_notifications = soundEnabled;

    // Save notification sound choice
    const notifSound = overlay.querySelector('#s-notif-sound')?.value || 'default';
    localStorage.setItem('exospine_notif_sound', notifSound);

    // Save widget preferences
    const widgetPrefs = {};
    overlay.querySelectorAll('.s-widget-toggle').forEach(cb => {
      widgetPrefs[cb.dataset.widget] = cb.checked;
    });
    localStorage.setItem('exospine_widget_prefs', JSON.stringify(widgetPrefs));

    // Save custom theme colors if custom theme is selected
    if (newSettings.theme === 'custom') {
      const customColors = {
        sidebarBg: overlay.querySelector('#s-ct-sidebar-bg')?.value || '#242933',
        listBg: overlay.querySelector('#s-ct-list-bg')?.value || '#2a2f3a',
        paneBg: overlay.querySelector('#s-ct-pane-bg')?.value || '#ffffff',
        accent: overlay.querySelector('#s-ct-accent')?.value || '#0078d6',
        textPrimary: overlay.querySelector('#s-ct-text-primary')?.value || '#1e1e1e',
        textSecondary: overlay.querySelector('#s-ct-text-secondary')?.value || '#646973',
      };
      saveCustomThemeColors(customColors);
    }

    // Save data retention setting
    const retentionDays = parseInt(overlay.querySelector('#s-retention-days')?.value || '0', 10);
    localStorage.setItem('exospine_retention_days', String(retentionDays >= 0 ? retentionDays : 0));

    try {
      await api.saveSettings(newSettings);

      // Save signatures for each account
      const sigTextareas = overlay.querySelectorAll('[data-sig-account]');
      for (const textarea of sigTextareas) {
        const accountId = textarea.dataset.sigAccount;
        const signature = textarea.value;
        try {
          await api.saveSignature(accountId, signature, null);
        } catch (sigErr) {
          showToast(`Failed to save signature for ${accountId}: ${sigErr}`, 'error');
        }
      }

      // Apply theme immediately (auto theme handled by onSaved callback)
      if (newSettings.theme === 'light') {
        document.documentElement.setAttribute('data-theme', 'light');
      } else if (newSettings.theme === 'high-contrast') {
        document.documentElement.setAttribute('data-theme', 'high-contrast');
      } else if (newSettings.theme === 'auto') {
        // Auto theme will be applied by onSaved -> applyTheme
      } else {
        document.documentElement.removeAttribute('data-theme');
      }
      showToast('Settings saved.', 'success');
      close();
      if (actions.onSaved) actions.onSaved(newSettings);
    } catch (err) {
      showToast(`Failed to save: ${err}`, 'error');
    }
  });

  // Remove account
  overlay.querySelectorAll('[data-remove-account]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const accountId = btn.dataset.removeAccount;
      const confirmed = await showDialog({
        title: 'Remove Account',
        message: 'Are you sure you want to remove this account? All local data for this account will be deleted.',
        confirmLabel: 'Remove',
        danger: true,
      });

      if (!confirmed) return;

      try {
        await api.removeAccount(accountId);
        showToast('Account removed.', 'success');
        close();
        if (actions.onAccountRemoved) actions.onAccountRemoved(accountId);
      } catch (err) {
        showToast(`Failed to remove account: ${err}`, 'error');
      }
    });
  });
}

// ===== SECTION: Utility Helpers =====
function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}

// ── Display Rules persistence (localStorage) ──────────────────────

const DISPLAY_RULES_KEY = 'exospine_display_rules';

// ===== SECTION: Display Rules =====
export function loadDisplayRules() {
  try {
    return JSON.parse(localStorage.getItem(DISPLAY_RULES_KEY) || '[]');
  } catch {
    return [];
  }
}

function saveDisplayRules(rules) {
  localStorage.setItem(DISPLAY_RULES_KEY, JSON.stringify(rules));
}

// ── Custom Theme persistence (localStorage) ────────────────────────

const CUSTOM_THEME_KEY = 'exospine_custom_theme';

// ===== SECTION: Custom Theme Colors =====
function loadCustomThemeColors() {
  try {
    return JSON.parse(localStorage.getItem(CUSTOM_THEME_KEY) || '{}');
  } catch {
    return {};
  }
}

function saveCustomThemeColors(colors) {
  localStorage.setItem(CUSTOM_THEME_KEY, JSON.stringify(colors));
}

// ── Keyboard Shortcuts persistence (localStorage) ──────────────────

const KEYBINDINGS_KEY = 'exospine_keybindings';

const DEFAULT_SHORTCUTS = {
  compose: 'Ctrl+n',
  reply: 'r',
  replyAll: 'Shift+R',
  forward: 'f',
  delete: 'Delete',
  archive: 'e',
  star: 's',
  unread: 'u',
  next: 'j',
  prev: 'k',
  refresh: 'F5',
  focusMode: 'Ctrl+Shift+F',
};

// ===== SECTION: Keyboard Shortcuts Configuration =====
export function loadCustomKeybindings() {
  try {
    return JSON.parse(localStorage.getItem(KEYBINDINGS_KEY) || '{}');
  } catch {
    return {};
  }
}

function saveCustomKeybindings(bindings) {
  localStorage.setItem(KEYBINDINGS_KEY, JSON.stringify(bindings));
}

/**
 * Get merged shortcuts (custom overrides defaults).
 */
export function getMergedShortcuts() {
  const custom = loadCustomKeybindings();
  return { ...DEFAULT_SHORTCUTS, ...custom };
}

/**
 * Hash a PIN using SHA-256 via Web Crypto API.
 * @param {string} pin
 * @returns {Promise<string>} hex-encoded hash
 */
// ===== SECTION: PIN Security =====
export async function hashPin(pin) {
  const encoder = new TextEncoder();
  const data = encoder.encode('exospine_pin_salt_' + pin);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
}

function renderShortcutsList() {
  const merged = getMergedShortcuts();
  const labels = {
    compose: 'Compose',
    reply: 'Reply',
    replyAll: 'Reply All',
    forward: 'Forward',
    delete: 'Delete',
    archive: 'Archive',
    star: 'Star',
    unread: 'Mark Unread',
    next: 'Next Email',
    prev: 'Previous Email',
    refresh: 'Refresh',
    focusMode: 'Focus Mode',
  };

  return Object.entries(merged).map(([action, key]) => `
    <div class="shortcut-row">
      <span class="text-sm">${labels[action] || action}</span>
      <div class="flex-row-center gap-6">
        <kbd data-shortcut-key="${action}" class="shortcut-key">${key}</kbd>
        <button class="btn btn-ghost btn-sm text-xxs" data-shortcut-change="${action}">Change</button>
      </div>
    </div>
  `).join('');
}

// ── Imported CSS Theme helpers ──────────────────────────────────────

const IMPORTED_THEME_STYLE_ID = 'exospine-imported-theme';

// ===== SECTION: Theme Import/Export =====
export function applyImportedThemeCSS(cssText) {
  removeImportedThemeCSS();
  if (!cssText) return;
  const style = document.createElement('style');
  style.id = IMPORTED_THEME_STYLE_ID;
  style.textContent = cssText;
  document.head.appendChild(style);
}

export function removeImportedThemeCSS() {
  const existing = document.getElementById(IMPORTED_THEME_STYLE_ID);
  if (existing) existing.remove();
}

/**
 * Restore imported CSS theme from localStorage on startup.
 * Should be called once during init.
 */
export function restoreImportedTheme() {
  const css = localStorage.getItem('exospine_imported_theme_css');
  if (css) applyImportedThemeCSS(css);
}

// ── Notification sound preview ──────────────────────────────────────

// ===== SECTION: Notification Sound Preview =====
function playNotifSoundPreview(sound) {
  if (sound === 'silent') return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (sound === 'default') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 880;
      osc.type = 'sine';
      gain.gain.value = 0.3;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
      osc.stop(ctx.currentTime + 0.3);
    } else if (sound === 'chime') {
      // 660Hz then 880Hz sequence
      const osc1 = ctx.createOscillator();
      const gain1 = ctx.createGain();
      osc1.connect(gain1);
      gain1.connect(ctx.destination);
      osc1.frequency.value = 660;
      osc1.type = 'sine';
      gain1.gain.value = 0.3;
      osc1.start();
      gain1.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.2);
      osc1.stop(ctx.currentTime + 0.2);

      const osc2 = ctx.createOscillator();
      const gain2 = ctx.createGain();
      osc2.connect(gain2);
      gain2.connect(ctx.destination);
      osc2.frequency.value = 880;
      osc2.type = 'sine';
      gain2.gain.value = 0.3;
      osc2.start(ctx.currentTime + 0.2);
      gain2.gain.setValueAtTime(0.3, ctx.currentTime + 0.2);
      gain2.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc2.stop(ctx.currentTime + 0.5);
    } else if (sound === 'bell') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 440;
      osc.type = 'triangle';
      gain.gain.value = 0.4;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc.stop(ctx.currentTime + 0.5);
    } else if (sound === 'gentle') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 523;
      osc.type = 'sine';
      gain.gain.value = 0.15;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.4);
      osc.stop(ctx.currentTime + 0.4);
    }
  } catch {}
}
