// Exospine — Settings panel view

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { showDialog } from '../components/dialog.js';
import { t } from '../i18n.js';

const overlay = document.getElementById('settings-overlay');

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
          <div id="s-custom-theme-section" ${settings.theme !== 'custom' ? 'hidden' : ''} style="margin-top:8px;padding:8px;border:1px solid var(--pane-border);border-radius:6px;">
            ${(() => {
              const ct = loadCustomThemeColors();
              return `
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Sidebar background</label>
                <input type="color" id="s-ct-sidebar-bg" value="${ct.sidebarBg || '#242933'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
              </div>
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Mail list background</label>
                <input type="color" id="s-ct-list-bg" value="${ct.listBg || '#2a2f3a'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
              </div>
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Reading pane background</label>
                <input type="color" id="s-ct-pane-bg" value="${ct.paneBg || '#ffffff'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
              </div>
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Accent color</label>
                <input type="color" id="s-ct-accent" value="${ct.accent || '#0078d6'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
              </div>
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Text primary</label>
                <input type="color" id="s-ct-text-primary" value="${ct.textPrimary || '#1e1e1e'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
              </div>
              <div class="settings-row" style="margin-bottom:6px;">
                <label>Text secondary</label>
                <input type="color" id="s-ct-text-secondary" value="${ct.textSecondary || '#646973'}" style="width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" />
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
              <div class="settings-signature-item" style="margin-bottom:12px;">
                <label style="display:block;margin-bottom:4px;font-weight:500;">${esc(acc.email || acc.name)}</label>
                <textarea class="settings-signature-textarea" data-sig-account="${esc(acc.id)}" rows="4" placeholder="Enter your signature..." style="width:100%;resize:vertical;font-family:inherit;font-size:13px;padding:6px 8px;border:1px solid var(--border);border-radius:4px;background:var(--pane-bg);color:var(--pane-text);">${esc(acc.signature || '')}</textarea>
              </div>
            `).join('')}
            ${state.accounts.length === 0 ? '<div style="color:var(--pane-text-dim);font-size:13px;">No accounts configured.</div>' : ''}
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('email_rules')}</div>
          <div id="s-rules-list" style="margin-bottom:8px;"></div>
          <button class="btn btn-ghost btn-sm" id="s-add-rule">+ Add Rule</button>
          <div id="s-rule-editor" hidden style="margin-top:8px;padding:8px;border:1px solid var(--pane-border);border-radius:6px;">
            <input type="hidden" id="s-rule-id" value="" />
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Name</label>
              <input type="text" id="s-rule-name" placeholder="Rule name" style="width:100%;padding:4px 6px;border:1px solid var(--pane-border);border-radius:4px;background:var(--pane-bg);color:var(--pane-text);" />
            </div>
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Condition</label>
              <select id="s-rule-cond-type" style="padding:4px;border-radius:4px;border:1px solid var(--pane-border);background:var(--pane-bg);color:var(--pane-text);">
                <option value="FromContains">From contains</option>
                <option value="SubjectContains">Subject contains</option>
                <option value="ToContains">To contains</option>
                <option value="HasAttachment">Has attachment</option>
              </select>
              <input type="text" id="s-rule-cond-value" placeholder="value" style="margin-left:4px;padding:4px 6px;border:1px solid var(--pane-border);border-radius:4px;background:var(--pane-bg);color:var(--pane-text);" />
            </div>
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Action</label>
              <select id="s-rule-action-type" style="padding:4px;border-radius:4px;border:1px solid var(--pane-border);background:var(--pane-bg);color:var(--pane-text);">
                <option value="MarkAsRead">Mark as read</option>
                <option value="Star">Star</option>
                <option value="MoveToFolder">Move to folder</option>
                <option value="Delete">Delete</option>
                <option value="AddCategory">Add category</option>
              </select>
              <input type="text" id="s-rule-action-value" placeholder="folder/category" style="margin-left:4px;padding:4px 6px;border:1px solid var(--pane-border);border-radius:4px;background:var(--pane-bg);color:var(--pane-text);" />
            </div>
            <div style="display:flex;gap:6px;">
              <button class="btn btn-primary btn-sm" id="s-rule-save">Save Rule</button>
              <button class="btn btn-ghost btn-sm" id="s-rule-cancel">Cancel</button>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Display Rules</div>
          <div id="s-display-rules-list" style="margin-bottom:8px;"></div>
          <button class="btn btn-ghost btn-sm" id="s-add-display-rule">+ Add Display Rule</button>
          <div id="s-display-rule-editor" hidden style="margin-top:8px;padding:8px;border:1px solid var(--pane-border);border-radius:6px;">
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Field</label>
              <select id="s-dr-field" style="padding:4px;border-radius:4px;border:1px solid var(--pane-border);background:var(--pane-bg);color:var(--pane-text);">
                <option value="from">From</option>
                <option value="subject">Subject</option>
              </select>
            </div>
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Contains</label>
              <input type="text" id="s-dr-value" placeholder="text to match" style="width:100%;padding:4px 6px;border:1px solid var(--pane-border);border-radius:4px;background:var(--pane-bg);color:var(--pane-text);" />
            </div>
            <div class="settings-row" style="margin-bottom:6px;">
              <label>Style</label>
              <select id="s-dr-style" style="padding:4px;border-radius:4px;border:1px solid var(--pane-border);background:var(--pane-bg);color:var(--pane-text);">
                <option value="highlight">Highlight row</option>
                <option value="bold">Bold text</option>
                <option value="italic">Italic text</option>
              </select>
              <input type="color" id="s-dr-color" value="#ff6b6b" style="margin-left:6px;width:40px;height:28px;border:1px solid var(--pane-border);border-radius:4px;cursor:pointer;" title="Highlight color" />
            </div>
            <div style="display:flex;gap:6px;">
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
            <span style="font-size:11px;color:var(--pane-text-dim);margin-left:8px;">${t('auto_delete_hint')}</span>
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Keyboard Shortcuts</div>
          <div id="s-shortcuts-list" style="margin-bottom:8px;">
            ${renderShortcutsList()}
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">Export / Import Settings</div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;">
            <button class="btn btn-ghost btn-sm" id="s-export-settings">Export Settings</button>
            <button class="btn btn-ghost btn-sm" id="s-import-settings">Import Settings</button>
            <input type="file" id="s-import-file" accept=".json" hidden />
          </div>
        </div>

        <div class="settings-section">
          <div class="settings-section-title">${t('accounts')}</div>
          <div class="settings-account-list" id="s-accounts">
            ${state.accounts.map((acc, i) => `
              <div class="settings-account-item">
                <span>${esc(acc.email || acc.name || `Account ${i + 1}`)}</span>
                <button class="btn btn-danger btn-sm" data-remove-account="${esc(acc.id || String(i))}">Remove</button>
              </div>
            `).join('')}
            ${state.accounts.length === 0 ? '<div style="color:var(--pane-text-dim);font-size:13px;">No accounts configured.</div>' : ''}
          </div>
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
        list.innerHTML = '<div style="color:var(--pane-text-dim);font-size:13px;padding:8px 0;">No security events recorded.</div>';
      } else {
        list.innerHTML = entries.map((entry) => {
          const escaped = esc(entry);
          // Highlight event type
          const highlighted = escaped.replace(
            /\] (\w+) \|/,
            '] <strong style="color:var(--accent);">$1</strong> |'
          );
          return `<div class="settings-security-log-entry">${highlighted}</div>`;
        }).join('');
      }
      container.hidden = false;
    } catch (err) {
      list.innerHTML = `<div style="color:var(--danger);font-size:13px;padding:8px 0;">Failed to load log: ${esc(String(err))}</div>`;
      container.hidden = false;
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
        list.innerHTML = '<div style="color:var(--pane-text-dim);font-size:13px;">No rules configured.</div>';
        return;
      }
      list.innerHTML = rules.map((r) => `
        <div class="settings-rule-item" style="display:flex;align-items:center;justify-content:space-between;padding:6px 0;border-bottom:1px solid var(--pane-border);">
          <div>
            <span style="font-weight:500;">${esc(r.name)}</span>
            <span style="font-size:12px;color:var(--pane-text-dim);margin-left:8px;">
              ${r.conditions.map((c) => c.type + (c.value ? ': ' + esc(c.value) : '')).join(', ')}
              \u2192 ${r.actions.map((a) => a.type + (a.value ? ': ' + esc(a.value) : '')).join(', ')}
            </span>
          </div>
          <button class="btn btn-danger btn-sm" data-delete-rule="${esc(r.id)}" style="margin-left:8px;">Delete</button>
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
      list.innerHTML = '<div style="color:var(--pane-text-dim);font-size:13px;">Failed to load rules.</div>';
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
      list.innerHTML = '<div style="color:var(--pane-text-dim);font-size:13px;">No display rules configured.</div>';
      return;
    }
    list.innerHTML = rules.map((r) => {
      const styleLabel = r.style === 'highlight' ? `Highlight (${r.color})` : r.style === 'bold' ? 'Bold' : 'Italic';
      const colorSwatch = r.style === 'highlight'
        ? `<span style="display:inline-block;width:12px;height:12px;border-radius:2px;background:${esc(r.color)};margin-right:4px;vertical-align:middle;"></span>`
        : '';
      return `
        <div style="display:flex;align-items:center;justify-content:space-between;padding:6px 0;border-bottom:1px solid var(--pane-border);">
          <div style="font-size:13px;">
            ${colorSwatch}If <b>${esc(r.field)}</b> contains "<b>${esc(r.value)}</b>" → ${styleLabel}
          </div>
          <button class="btn btn-danger btn-sm" data-delete-display-rule="${esc(r.id)}" style="margin-left:8px;">Delete</button>
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
    localStorage.setItem('exospine_sound_notifications', String(newSettings.sound_notifications));

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

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}

// ── Display Rules persistence (localStorage) ──────────────────────

const DISPLAY_RULES_KEY = 'exospine_display_rules';

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
    <div style="display:flex;align-items:center;justify-content:space-between;padding:4px 0;border-bottom:1px solid var(--pane-border);">
      <span style="font-size:13px;">${labels[action] || action}</span>
      <div style="display:flex;align-items:center;gap:6px;">
        <kbd data-shortcut-key="${action}" style="font-family:var(--font-mono);font-size:12px;padding:2px 6px;background:var(--pane-header-bg);border:1px solid var(--pane-border);border-radius:3px;">${key}</kbd>
        <button class="btn btn-ghost btn-sm" data-shortcut-change="${action}" style="font-size:11px;">Change</button>
      </div>
    </div>
  `).join('');
}
