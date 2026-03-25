// Exospine — Compose email view (rich text)

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { t } from '../i18n.js';

const overlay = document.getElementById('compose-overlay');

/** Fallback email signature used when no per-account signature is configured. */
const FALLBACK_SIGNATURE = '<br><br><div class="compose-signature">--<br>Sent with Exospine</div>';

// ── Undo Send state ─────────────────────────────────────────────────────
let _sendTimeout = null;

// ── Auto-save Drafts state ──────────────────────────────────────────────
let _autoSaveInterval = null;
let _currentDraftId = null;
let _lastContentHash = null;

/** Simple hash for dirty-checking auto-save content. */
function _hashContent(str) {
  let h = 0;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) - h + str.charCodeAt(i)) | 0;
  }
  return h;
}

function startAutoSave(getDraft) {
  stopAutoSave();
  // Generate a unique draft ID for version tracking
  if (!_currentDraftId) {
    _currentDraftId = crypto.randomUUID();
  }
  // Snapshot initial content hash
  const draft0 = getDraft();
  _lastContentHash = _hashContent(draft0.subject + '\0' + draft0.body);

  _autoSaveInterval = setInterval(async () => {
    const draft = getDraft();
    if (draft.body || draft.subject) {
      // Only save if content actually changed since last save
      const currentHash = _hashContent(draft.subject + '\0' + draft.body);
      if (currentHash === _lastContentHash) return;
      _lastContentHash = currentHash;

      try {
        await api.saveDraft(draft);
        // Also save a local version for history
        const bodyHtml = overlay.querySelector('#compose-body')?.innerHTML || '';
        await api.saveDraftVersion(_currentDraftId, draft.subject, draft.body, bodyHtml);
        const indicator = overlay.querySelector('#compose-autosave-indicator');
        if (indicator) {
          indicator.textContent = 'Draft saved';
          indicator.hidden = false;
          setTimeout(() => { if (indicator) indicator.hidden = true; }, 2000);
        }
      } catch {
        // Silent fail for auto-save
      }
    }
  }, 30000);
}

function stopAutoSave() {
  if (_autoSaveInterval) {
    clearInterval(_autoSaveInterval);
    _autoSaveInterval = null;
  }
  _currentDraftId = null;
}

// ── Templates / Quick Parts ─────────────────────────────────────────

const PREDEFINED_TEMPLATES = [
  {
    name: 'template_meeting',
    subject: 'Meeting Request',
    body: '<p>Hello,</p><p>I would like to schedule a meeting to discuss the following topics:</p><ul><li></li></ul><p>Please let me know your availability.</p><p>Best regards</p>',
  },
  {
    name: 'template_followup',
    subject: 'Follow-up',
    body: '<p>Hello,</p><p>I wanted to follow up on our previous conversation regarding:</p><p></p><p>Please let me know if you have any updates.</p><p>Best regards</p>',
  },
  {
    name: 'template_thankyou',
    subject: 'Thank You',
    body: '<p>Hello,</p><p>Thank you very much for your time and assistance with this matter.</p><p>I really appreciate your help.</p><p>Best regards</p>',
  },
  {
    name: 'template_ooo',
    subject: 'Out of Office',
    body: '<p>Hello,</p><p>Thank you for your email. I am currently out of the office and will return on [date].</p><p>For urgent matters, please contact [alternative contact].</p><p>Best regards</p>',
  },
];

function loadCustomTemplates() {
  try {
    const raw = localStorage.getItem('exospine_templates');
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveCustomTemplates(templates) {
  localStorage.setItem('exospine_templates', JSON.stringify(templates));
}

/**
 * Open the compose overlay.
 * @param {object} [prefill]  optional { to, cc, bcc, subject, body, bodyHtml, accountId } for reply/forward
 * @param {function} [onSent] callback after successful send
 */
export async function openCompose(prefill = {}, onSent = null) {
  // Clean up any lingering auto-save from a previous compose session
  stopAutoSave();
  overlay.hidden = false;

  // Fetch per-account signature (fall back to default)
  let accountSignatureHtml = FALLBACK_SIGNATURE;
  if (prefill.accountId) {
    try {
      const sig = await api.getSignature(prefill.accountId);
      if (sig && sig.signature) {
        const sigHtml = sig.signature_html
          || ('<br><br><div class="compose-signature">--<br>' + esc(sig.signature).replace(/\n/g, '<br>') + '</div>');
        accountSignatureHtml = sigHtml;
      }
    } catch {
      // Use fallback
    }
  }

  // Store signature for buildInitialContent
  prefill._signatureHtml = accountSignatureHtml;

  overlay.innerHTML = `
    <div class="overlay-panel compose-panel">
      <div class="compose-header">
        <span class="compose-title">${prefill.subject ? t('reply_title') : t('new_email_title')}</span>
        <span class="compose-autosave-indicator" id="compose-autosave-indicator" hidden></span>
        <button class="compose-close" id="compose-close" title="Close" aria-label="Close compose window">\u00D7</button>
      </div>
      <div class="compose-fields">
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-to">${t('to')}</label>
          <input type="text" class="compose-field-input" id="compose-to" value="${esc(prefill.to || '')}" placeholder="recipient@example.com" autocomplete="off" aria-label="To recipients" />
          <div class="compose-autocomplete" id="ac-to" hidden role="listbox" aria-label="Contact suggestions"></div>
        </div>
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-cc">${t('cc')}</label>
          <input type="text" class="compose-field-input" id="compose-cc" value="${esc(prefill.cc || '')}" placeholder="" autocomplete="off" aria-label="CC recipients" />
          <div class="compose-autocomplete" id="ac-cc" hidden role="listbox" aria-label="Contact suggestions"></div>
        </div>
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-bcc">${t('bcc')}</label>
          <input type="text" class="compose-field-input" id="compose-bcc" value="${esc(prefill.bcc || '')}" placeholder="" autocomplete="off" aria-label="BCC recipients" />
          <div class="compose-autocomplete" id="ac-bcc" hidden role="listbox" aria-label="Contact suggestions"></div>
        </div>
        <div class="compose-field">
          <label class="compose-field-label" for="compose-subject">${t('subject')}</label>
          <input type="text" class="compose-field-input" id="compose-subject" value="${esc(prefill.subject || '')}" placeholder="${t('subject')}" aria-label="Email subject" />
        </div>
        <div class="compose-field">
          <label class="compose-field-label" for="compose-importance">Priority</label>
          <select id="compose-importance" class="compose-field-input compose-importance-select" aria-label="Email priority">
            <option value="normal">Normal</option>
            <option value="high">\u2757 High</option>
            <option value="low">\u2193 Low</option>
          </select>
        </div>
      </div>
      <div class="compose-body-wrap">
        <div class="compose-toolbar" id="compose-toolbar" role="toolbar" aria-label="Formatting toolbar">
          <button type="button" data-cmd="bold" title="Bold (Ctrl+B)" aria-label="Bold"><b>B</b></button>
          <button type="button" data-cmd="italic" title="Italic (Ctrl+I)" aria-label="Italic"><i>I</i></button>
          <button type="button" data-cmd="underline" title="Underline (Ctrl+U)" aria-label="Underline"><u>U</u></button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="insertUnorderedList" title="Bullet list" aria-label="Bullet list">\u2022</button>
          <button type="button" data-cmd="insertOrderedList" title="Numbered list" aria-label="Numbered list">1.</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <select data-cmd="fontSize" title="Font size" class="toolbar-select" aria-label="Font size">
            <option value="">Size</option>
            <option value="1">Small</option>
            <option value="3">Normal</option>
            <option value="5">Large</option>
          </select>
          <input type="color" data-cmd="foreColor" title="Text color" class="toolbar-color" value="#000000" aria-label="Text color" />
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="justifyLeft" title="Align left" aria-label="Align left">\u2261</button>
          <button type="button" data-cmd="justifyCenter" title="Align center" aria-label="Align center">\u2550</button>
          <button type="button" data-cmd="justifyRight" title="Align right" aria-label="Align right">\u2262</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="createLink" title="Insert link" aria-label="Insert link">\uD83D\uDD17</button>
          <button type="button" data-cmd="insertHorizontalRule" title="Horizontal line" aria-label="Insert horizontal line">\u2014</button>
          <button type="button" data-cmd="removeFormat" title="Clear formatting" aria-label="Clear formatting">\u2715</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <div class="toolbar-dropdown-wrapper">
            <button type="button" id="compose-templates-btn" title="${t('templates')}" aria-label="${t('templates')}" aria-expanded="false">\uD83D\uDCCB ${t('templates')}</button>
            <div id="compose-templates-dropdown" hidden class="compose-templates-dropdown"></div>
          </div>
          <span class="toolbar-sep"></span>
          <button type="button" id="compose-md-toggle" title="Toggle Rich Text / Markdown" class="text-xs font-bolder">MD</button>
        </div>
        <div id="compose-body" contenteditable="true" spellcheck="true" lang="auto" class="compose-editor">${buildInitialContent(prefill)}</div>
        <textarea id="compose-body-md" class="compose-editor-md" hidden placeholder="Write in Markdown...\n\n**bold**, *italic*, \`code\`, [link](url), # headings, - lists"></textarea>
      </div>
      <div id="compose-attachments" class="compose-attachments" hidden>
        <div class="compose-attachments-label">Attachments:</div>
        <div id="compose-attachments-list" class="compose-attachments-list"></div>
      </div>
      <div id="compose-version-panel" hidden class="compose-version-panel">
        <div class="compose-version-panel-title">Version History</div>
        <div id="compose-version-list" class="text-dim">No versions saved yet.</div>
      </div>
      <div class="compose-status-bar" id="compose-status-bar">
        <span id="compose-word-count">0 words</span>
        <span id="compose-char-count">0 characters</span>
      </div>
      <div class="compose-footer">
        <button class="btn btn-ghost" id="compose-discard">${t('discard')}</button>
        <button class="btn btn-ghost text-xs" id="compose-versions-btn" title="Version history">\uD83D\uDD53 Versions</button>
        <div class="compose-send-group">
          <button class="btn btn-primary" id="compose-send">\u2709 ${t('send')}</button>
          <div class="compose-field-relative">
            <button class="btn btn-ghost" id="compose-schedule-btn" title="Schedule send">\u23F0</button>
            <div class="compose-schedule-dropdown" id="compose-schedule-dropdown" hidden>
              <label>Schedule send:</label>
              <input type="datetime-local" id="compose-schedule-time" />
              <button class="btn btn-primary btn-sm w-full mt-6" id="compose-schedule-confirm">Schedule</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  // Wire up the toolbar
  setupToolbar();

  // Wire up inline image paste
  setupImagePaste();

  // Wire up word/character count
  const _composeBody = overlay.querySelector('#compose-body');
  const _composeMd = overlay.querySelector('#compose-body-md');
  function updateWordCharCount() {
    let text = '';
    if (_composeMd && !_composeMd.hidden) {
      text = _composeMd.value || '';
    } else if (_composeBody) {
      text = _composeBody.innerText || '';
    }
    const chars = text.length;
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    const wordEl = overlay.querySelector('#compose-word-count');
    const charEl = overlay.querySelector('#compose-char-count');
    if (wordEl) wordEl.textContent = `${words} word${words !== 1 ? 's' : ''}`;
    if (charEl) charEl.textContent = `${chars} character${chars !== 1 ? 's' : ''}`;
  }
  // Debounce word/char count updates with requestAnimationFrame
  let _wcRAF = null;
  const debouncedWCC = () => {
    if (_wcRAF) cancelAnimationFrame(_wcRAF);
    _wcRAF = requestAnimationFrame(updateWordCharCount);
  };
  if (_composeBody) _composeBody.addEventListener('input', debouncedWCC);
  if (_composeMd) _composeMd.addEventListener('input', debouncedWCC);
  updateWordCharCount();

  // Wire up drag-and-drop file attachments
  const _attachments = [];
  setupFileDrop(overlay, _attachments);

  // Wire up contact auto-complete on To/CC/BCC
  const _acCleanups = [
    setupAutocomplete('compose-to', 'ac-to'),
    setupAutocomplete('compose-cc', 'ac-cc'),
    setupAutocomplete('compose-bcc', 'ac-bcc'),
  ];

  // Templates dropdown
  setupTemplatesDropdown(prefill);

  // Markdown mode toggle
  let _markdownMode = false;
  const mdToggle = overlay.querySelector('#compose-md-toggle');
  const richEditor = overlay.querySelector('#compose-body');
  const mdEditor = overlay.querySelector('#compose-body-md');
  const composeToolbar = overlay.querySelector('#compose-toolbar');
  if (mdToggle && richEditor && mdEditor) {
    mdToggle.addEventListener('click', () => {
      _markdownMode = !_markdownMode;
      if (_markdownMode) {
        // Switch to markdown: copy content to textarea as plain text
        mdEditor.value = htmlToPlainText(richEditor.innerHTML);
        richEditor.hidden = true;
        mdEditor.hidden = false;
        mdToggle.classList.add('md-toggle-active');
        // Hide rich text toolbar buttons (except MD toggle)
        if (composeToolbar) {
          for (const child of composeToolbar.children) {
            if (child !== mdToggle && !child.contains(mdToggle)) {
              child.classList.add('toolbar-item-disabled');
            }
          }
        }
      } else {
        // Switch to rich text: convert markdown to HTML
        const html = markdownToHtml(mdEditor.value);
        richEditor.innerHTML = html;
        richEditor.hidden = false;
        mdEditor.hidden = true;
        mdToggle.classList.remove('md-toggle-active');
        if (composeToolbar) {
          for (const child of composeToolbar.children) {
            child.classList.remove('toolbar-item-disabled');
          }
        }
      }
    });
  }

  // Schedule send toggle
  const schedBtn = overlay.querySelector('#compose-schedule-btn');
  const schedDropdown = overlay.querySelector('#compose-schedule-dropdown');
  if (schedBtn && schedDropdown) {
    schedBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      schedDropdown.hidden = !schedDropdown.hidden;
    });
    // Close dropdown on outside click — use overlay-scoped delegation instead of permanent document listener
    overlay.addEventListener('click', (e) => {
      if (!schedDropdown.contains(e.target) && e.target !== schedBtn) {
        schedDropdown.hidden = true;
      }
    });
    schedDropdown.addEventListener('click', (e) => e.stopPropagation());
  }

  // Schedule confirm
  const schedConfirm = overlay.querySelector('#compose-schedule-confirm');
  if (schedConfirm) {
    schedConfirm.addEventListener('click', async () => {
      const timeInput = overlay.querySelector('#compose-schedule-time');
      if (!timeInput || !timeInput.value) {
        showToast('Please select a date and time.', 'error');
        return;
      }
      const to = overlay.querySelector('#compose-to').value.trim();
      if (!to) {
        showToast('Please enter a recipient.', 'error');
        return;
      }
      const bodyHtml = overlay.querySelector('#compose-body').innerHTML;
      const draft = {
        account_id: prefill.accountId || '',
        to,
        cc: overlay.querySelector('#compose-cc').value.trim() || '',
        bcc: overlay.querySelector('#compose-bcc').value.trim() || '',
        subject: overlay.querySelector('#compose-subject').value.trim() || '',
        body: htmlToPlainText(bodyHtml),
        body_html: bodyHtml,
        reply_to: prefill.replyTo || null,
        attachments: _attachments.map(a => ({ name: a.name, type: a.type, size: a.size, data: a.data })),
      };
      const scheduledTime = new Date(timeInput.value).toISOString();
      try {
        await api.scheduleSend(draft, scheduledTime);
        showToast(`Email scheduled for ${timeInput.value}`, 'success');
        close();
      } catch (err) {
        showToast(`Failed to schedule: ${err}`, 'error');
      }
    });
  }

  // Focus first empty field
  const toInput = overlay.querySelector('#compose-to');
  const subjectInput = overlay.querySelector('#compose-subject');
  const bodyEditor = overlay.querySelector('#compose-body');
  if (!toInput.value) {
    toInput.focus();
  } else if (!subjectInput.value) {
    subjectInput.focus();
  } else {
    // Place cursor at start of editor (before signature)
    placeCursorAtStart(bodyEditor);
  }

  // Escape listener (stored for cleanup in all exit paths)
  let onKey = null;

  // Helper to build draft from current form state
  const getDraft = () => ({
    account_id: prefill.accountId || '',
    to: overlay.querySelector('#compose-to')?.value.trim() || '',
    cc: overlay.querySelector('#compose-cc')?.value.trim() || '',
    bcc: overlay.querySelector('#compose-bcc')?.value.trim() || '',
    subject: overlay.querySelector('#compose-subject')?.value.trim() || '',
    body: htmlToPlainText(overlay.querySelector('#compose-body')?.innerHTML || ''),
    reply_to: prefill.replyTo || null,
    attachments: _attachments.map(a => ({ name: a.name, type: a.type, size: a.size, data: a.data })),
  });

  // Start auto-save (every 30 seconds)
  startAutoSave(getDraft);

  // Close — cleans up Escape listener, auto-save, autocomplete, and pending undo-send in ALL exit paths
  const close = () => {
    stopAutoSave();
    if (onKey) {
      document.removeEventListener('keydown', onKey);
      onKey = null;
    }
    if (_sendTimeout) {
      clearTimeout(_sendTimeout);
      _sendTimeout = null;
    }
    // Clean up autocomplete pending timeouts
    for (const cleanup of _acCleanups) {
      if (cleanup) cleanup();
    }
    overlay.hidden = true;
    overlay.innerHTML = '';
  };

  overlay.querySelector('#compose-close').addEventListener('click', close);
  overlay.querySelector('#compose-discard').addEventListener('click', close);

  // Version history toggle
  const versionsBtn = overlay.querySelector('#compose-versions-btn');
  const versionPanel = overlay.querySelector('#compose-version-panel');
  if (versionsBtn && versionPanel) {
    versionsBtn.addEventListener('click', async () => {
      if (!versionPanel.hidden) {
        versionPanel.hidden = true;
        return;
      }
      versionPanel.hidden = false;
      const versionList = overlay.querySelector('#compose-version-list');
      if (!_currentDraftId) {
        versionList.innerHTML = '<div class="text-dim">No versions saved yet.</div>';
        return;
      }
      try {
        const versions = await api.loadDraftVersions(_currentDraftId);
        if (!versions || versions.length === 0) {
          versionList.innerHTML = '<div class="text-dim">No versions saved yet.</div>';
          return;
        }
        versionList.innerHTML = versions.map((v) => {
          const ts = new Date(v.timestamp).toLocaleString();
          const preview = (v.subject || '').slice(0, 40) || '(no subject)';
          return `
            <div class="compose-version-item" data-version="${v.version}">
              <div class="compose-version-title">v${v.version} - ${esc(ts)}</div>
              <div class="compose-version-preview">${esc(preview)}</div>
            </div>
          `;
        }).join('');
        // Restore click handlers
        versionList.querySelectorAll('.compose-version-item').forEach((item) => {
          item.addEventListener('click', () => {
            const ver = parseInt(item.dataset.version, 10);
            const version = versions.find((v) => v.version === ver);
            if (version) {
              const editor = overlay.querySelector('#compose-body');
              const subjectInput = overlay.querySelector('#compose-subject');
              if (editor && version.body_html) {
                editor.innerHTML = version.body_html;
              } else if (editor) {
                editor.textContent = version.body;
              }
              if (subjectInput && version.subject) {
                subjectInput.value = version.subject;
              }
              versionPanel.hidden = true;
            }
          });
        });
      } catch {
        versionList.innerHTML = '<div class="text-danger">Failed to load versions.</div>';
      }
    });
  }

  // Close on backdrop
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

  // Escape to close
  onKey = (e) => {
    if (e.key === 'Escape') close();
  };
  document.addEventListener('keydown', onKey);

  // Send with Undo (5 second delay)
  overlay.querySelector('#compose-send').addEventListener('click', () => {
    const to = overlay.querySelector('#compose-to').value.trim();
    const cc = overlay.querySelector('#compose-cc').value.trim();
    const bcc = overlay.querySelector('#compose-bcc').value.trim();
    const subject = overlay.querySelector('#compose-subject').value.trim();
    const importance = overlay.querySelector('#compose-importance')?.value || 'normal';

    // Handle markdown mode: convert markdown to HTML before sending
    let bodyHtml, bodyPlain;
    if (_markdownMode) {
      const mdText = overlay.querySelector('#compose-body-md')?.value || '';
      bodyHtml = markdownToHtml(mdText);
      bodyPlain = mdText;
    } else {
      bodyHtml = overlay.querySelector('#compose-body').innerHTML;
      bodyPlain = htmlToPlainText(bodyHtml);
    }

    if (!to) {
      showToast('Please enter a recipient.', 'error');
      return;
    }

    // Basic email validation
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(to)) {
      showToast('Invalid email address.', 'error');
      return;
    }

    const draft = {
      account_id: prefill.accountId || '',
      to,
      cc: cc || '',
      bcc: bcc || '',
      subject,
      body: bodyPlain,
      body_html: bodyHtml,
      reply_to: prefill.replyTo || null,
      attachments: _attachments.map(a => ({ name: a.name, type: a.type, size: a.size, data: a.data })),
      importance,
    };

    // Stop auto-save and close compose immediately
    stopAutoSave();
    close();

    // Show undo toast for 5 seconds
    const toastEl = showToast('Email sent. Click to undo.', 'info', 5500, () => {
      // Undo callback — cancel the pending send
      if (_sendTimeout) {
        clearTimeout(_sendTimeout);
        _sendTimeout = null;
      }
      showToast('Send cancelled.', 'info');
      // Reopen compose with the draft content
      openCompose({
        accountId: draft.account_id,
        to: draft.to,
        cc: draft.cc,
        bcc: draft.bcc,
        subject: draft.subject,
        bodyHtml: draft.body_html,
        replyTo: draft.reply_to,
      }, onSent);
    });

    // Actually send after 5 seconds
    _sendTimeout = setTimeout(async () => {
      _sendTimeout = null;
      try {
        await api.sendMail(draft);
        if (toastEl && toastEl._dismiss) toastEl._dismiss();
        showToast('Email delivered!', 'success');
        if (onSent) onSent();
      } catch (e) {
        showToast(`Send failed: ${e}`, 'error');
      }
    }, 5000);
  });

  // Ctrl+Enter to send
  bodyEditor.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      overlay.querySelector('#compose-send').click();
    }
  });
}

// ── Rich text toolbar ──────────────────────────────────────────────────

function setupToolbar() {
  const toolbar = overlay.querySelector('#compose-toolbar');
  if (!toolbar) return;

  // Button commands
  toolbar.querySelectorAll('button[data-cmd]').forEach((btn) => {
    btn.addEventListener('mousedown', (e) => {
      e.preventDefault(); // keep focus in editor
    });
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const cmd = btn.dataset.cmd;

      if (cmd === 'createLink') {
        const url = prompt('Enter URL:', 'https://');
        if (url) document.execCommand('createLink', false, url);
        return;
      }

      document.execCommand(cmd, false, null);
    });
  });

  // Font size select
  const fontSizeSelect = toolbar.querySelector('select[data-cmd="fontSize"]');
  if (fontSizeSelect) {
    fontSizeSelect.addEventListener('mousedown', (e) => e.stopPropagation());
    fontSizeSelect.addEventListener('change', () => {
      const val = fontSizeSelect.value;
      if (val) {
        document.execCommand('fontSize', false, val);
      }
      fontSizeSelect.value = '';
    });
  }

  // Text color picker
  const colorInput = toolbar.querySelector('input[data-cmd="foreColor"]');
  if (colorInput) {
    colorInput.addEventListener('input', () => {
      document.execCommand('foreColor', false, colorInput.value);
    });
  }
}

// ── Templates dropdown ────────────────────────────────────────────────

function setupTemplatesDropdown(prefill) {
  const btn = overlay.querySelector('#compose-templates-btn');
  const dropdown = overlay.querySelector('#compose-templates-dropdown');
  if (!btn || !dropdown) return;

  function renderDropdown() {
    const customTemplates = loadCustomTemplates();
    const allTemplates = [
      ...PREDEFINED_TEMPLATES.map((tpl) => ({ ...tpl, displayName: t(tpl.name) })),
      ...customTemplates.map((tpl) => ({ ...tpl, displayName: tpl.name, custom: true })),
    ];

    let html = '';
    for (const tpl of allTemplates) {
      html += `<div class="compose-template-item" data-tpl-subject="${esc(tpl.subject)}" data-tpl-body="${esc(tpl.body)}">
        <span>${esc(tpl.displayName)}</span>
        ${tpl.custom ? `<button class="compose-template-delete" data-delete-tpl="${esc(tpl.name)}" title="${t('delete')}">\u00D7</button>` : ''}
      </div>`;
    }
    html += `<div class="compose-template-item compose-template-save">
      \uD83D\uDCBE ${t('save_as_template')}
    </div>`;

    dropdown.innerHTML = html;

    // Insert template on click
    dropdown.querySelectorAll('.compose-template-item[data-tpl-subject]').forEach((item) => {
      item.addEventListener('click', (e) => {
        if (e.target.closest('.compose-template-delete')) return;
        const subjectInput = overlay.querySelector('#compose-subject');
        const bodyEditor = overlay.querySelector('#compose-body');
        if (subjectInput && !subjectInput.value) {
          subjectInput.value = item.dataset.tplSubject;
        }
        if (bodyEditor) {
          bodyEditor.innerHTML = item.dataset.tplBody;
        }
        dropdown.hidden = true;
      });
    });

    // Delete custom template
    dropdown.querySelectorAll('.compose-template-delete').forEach((delBtn) => {
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        const name = delBtn.dataset.deleteTpl;
        const customs = loadCustomTemplates().filter((tpl) => tpl.name !== name);
        saveCustomTemplates(customs);
        renderDropdown();
      });
    });

    // Save as template
    const saveBtn = dropdown.querySelector('.compose-template-save');
    if (saveBtn) {
      saveBtn.addEventListener('click', () => {
        const name = prompt('Template name:');
        if (!name) return;
        const subject = overlay.querySelector('#compose-subject')?.value || '';
        const body = overlay.querySelector('#compose-body')?.innerHTML || '';
        const customs = loadCustomTemplates();
        customs.push({ name, subject, body });
        saveCustomTemplates(customs);
        showToast(t('draft_saved'), 'success');
        dropdown.hidden = true;
      });
    }
  }

  btn.addEventListener('mousedown', (e) => e.preventDefault());
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    renderDropdown();
    dropdown.hidden = !dropdown.hidden;
    btn.setAttribute('aria-expanded', String(!dropdown.hidden));
  });

  // Use a single delegated listener on the overlay instead of adding a new
  // document-level listener every time the compose window opens.
  overlay.addEventListener('click', (e) => {
    if (!btn.contains(e.target) && !dropdown.contains(e.target)) {
      dropdown.hidden = true;
      btn.setAttribute('aria-expanded', 'false');
    }
  });
  dropdown.addEventListener('click', (e) => e.stopPropagation());
}

// ── Inline image paste ─────────────────────────────────────────────────

function setupImagePaste() {
  const editor = overlay.querySelector('#compose-body');
  if (!editor) return;

  editor.addEventListener('paste', (e) => {
    const items = e.clipboardData && e.clipboardData.items;
    if (!items) return;

    for (const item of items) {
      if (item.type.startsWith('image/')) {
        e.preventDefault();
        const blob = item.getAsFile();
        if (!blob) continue;

        const reader = new FileReader();
        reader.onload = () => {
          const dataUri = reader.result;
          document.execCommand('insertImage', false, dataUri);
        };
        reader.readAsDataURL(blob);
        return; // handle only first image
      }
    }
  });

  // Also support drag-and-drop of images
  editor.addEventListener('drop', (e) => {
    const files = e.dataTransfer && e.dataTransfer.files;
    if (!files || files.length === 0) return;

    for (const file of files) {
      if (file.type.startsWith('image/')) {
        e.preventDefault();
        const reader = new FileReader();
        reader.onload = () => {
          const dataUri = reader.result;
          document.execCommand('insertImage', false, dataUri);
        };
        reader.readAsDataURL(file);
        return;
      }
    }
  });
}

// ── Drag-and-drop file attachments ─────────────────────────────────────

function setupFileDrop(overlay, attachments) {
  const editor = overlay.querySelector('#compose-body');
  const bodyWrap = overlay.querySelector('.compose-body-wrap');
  if (!editor || !bodyWrap) return;

  function formatAttSize(bytes) {
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return bytes + ' B';
  }

  function renderAttachmentPills() {
    const container = overlay.querySelector('#compose-attachments');
    const list = overlay.querySelector('#compose-attachments-list');
    if (!container || !list) return;

    if (attachments.length === 0) {
      container.hidden = true;
      return;
    }

    container.hidden = false;
    list.innerHTML = attachments.map((att, idx) =>
      `<div class="compose-attachment-pill" data-att-idx="${idx}">
        <span class="compose-attachment-pill-name" title="${esc(att.name)}">${esc(att.name)}</span>
        <span class="compose-attachment-pill-size">${formatAttSize(att.size)}</span>
        <button class="compose-attachment-pill-remove" data-att-idx="${idx}" title="Remove">\u00D7</button>
      </div>`
    ).join('');

    list.querySelectorAll('.compose-attachment-pill-remove').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const idx = parseInt(btn.dataset.attIdx, 10);
        attachments.splice(idx, 1);
        renderAttachmentPills();
      });
    });
  }

  function addFiles(files) {
    for (const file of files) {
      const reader = new FileReader();
      reader.onload = () => {
        attachments.push({
          name: file.name,
          type: file.type || 'application/octet-stream',
          size: file.size,
          data: reader.result,
        });
        renderAttachmentPills();
      };
      reader.readAsDataURL(file);
    }
  }

  // Prevent default browser behavior on the whole compose panel
  bodyWrap.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    bodyWrap.classList.add('compose-drop-active');
  });

  bodyWrap.addEventListener('dragleave', (e) => {
    e.preventDefault();
    bodyWrap.classList.remove('compose-drop-active');
  });

  bodyWrap.addEventListener('drop', (e) => {
    bodyWrap.classList.remove('compose-drop-active');
    const files = e.dataTransfer && e.dataTransfer.files;
    if (!files || files.length === 0) return;

    e.preventDefault();
    e.stopPropagation();
    addFiles(Array.from(files));
  });
}

// ── Helpers ────────────────────────────────────────────────────────────

/** Build initial HTML content for the editor (body + signature). */
function buildInitialContent(prefill) {
  let content = '';
  const signature = prefill._signatureHtml || FALLBACK_SIGNATURE;

  if (prefill.bodyHtml) {
    content = prefill.bodyHtml;
  } else if (prefill.body) {
    // Convert plain text prefill to HTML (preserve newlines)
    content = esc(prefill.body).replace(/\n/g, '<br>');
  }

  // Add signature: for new emails place cursor before it; for replies/forwards append after quoted text
  if (!prefill.body && !prefill.bodyHtml) {
    content = '<br>' + signature;
  } else {
    // Insert signature between cursor area and quoted text
    content = '<br>' + signature + content;
  }

  return content;
}

/** Place the cursor at the very start of the contenteditable div. */
function placeCursorAtStart(el) {
  el.focus();
  const range = document.createRange();
  range.setStart(el, 0);
  range.collapse(true);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
}

/** Convert HTML to plain text (strip tags, decode entities). */
function htmlToPlainText(html) {
  const tmp = document.createElement('div');
  tmp.innerHTML = html;

  // Convert <br> to newlines
  tmp.querySelectorAll('br').forEach((br) => br.replaceWith('\n'));

  // Convert block elements to newlines
  tmp.querySelectorAll('p, div, li, h1, h2, h3, h4, h5, h6').forEach((el) => {
    el.prepend('\n');
  });

  // Convert <hr> to ---
  tmp.querySelectorAll('hr').forEach((hr) => hr.replaceWith('\n---\n'));

  // Convert <a> to text [link](url)
  tmp.querySelectorAll('a[href]').forEach((a) => {
    const text = a.textContent;
    const href = a.getAttribute('href');
    if (text === href) {
      a.replaceWith(href);
    } else {
      a.replaceWith(`${text} (${href})`);
    }
  });

  // Convert list items
  tmp.querySelectorAll('ol').forEach((ol) => {
    let i = 1;
    ol.querySelectorAll(':scope > li').forEach((li) => {
      li.prepend(`${i++}. `);
    });
  });
  tmp.querySelectorAll('ul').forEach((ul) => {
    ul.querySelectorAll(':scope > li').forEach((li) => {
      li.prepend('- ');
    });
  });

  let text = tmp.textContent || tmp.innerText || '';
  // Collapse multiple blank lines
  text = text.replace(/\n{3,}/g, '\n\n').trim();
  return text;
}

/**
 * Prepare a reply prefill object.
 */
export function makeReplyPrefill(mail, body, replyAll = false) {
  const subject = mail.subject || '';
  const reSubject = subject.startsWith('Re:') ? subject : `Re: ${subject}`;

  const safeFrom = escapeForText(mail.from || '');
  const safeDate = escapeForText(mail.date || '');

  // Build HTML quoted body for rich text editor
  let bodyHtml = '';
  if (body) {
    const originalContent = body.html || esc(body.text || '').replace(/\n/g, '<br>');
    bodyHtml = `<br><br><div class="compose-quoted">
      <div class="compose-quoted-header">--- Original message ---<br>From: ${safeFrom}<br>Date: ${safeDate}</div>
      <blockquote>${originalContent}</blockquote>
    </div>`;
  }

  // Also keep plain text body for fallback
  const quotedBody = body
    ? `\n\n--- Original message ---\nFrom: ${safeFrom}\nDate: ${safeDate}\n\n${body.text || stripHtml(body.html || '')}`
    : '';

  const prefill = {
    to: mail.from || '',
    subject: reSubject,
    body: quotedBody,
    bodyHtml,
  };

  if (replyAll && mail.cc) {
    prefill.cc = mail.cc;
  }

  return prefill;
}

/**
 * Prepare a forward prefill object.
 */
export function makeForwardPrefill(mail, body) {
  const subject = mail.subject || '';
  const fwdSubject = subject.startsWith('Fwd:') ? subject : `Fwd: ${subject}`;

  const fwdFrom = escapeForText(mail.from || '');
  const fwdTo = escapeForText(mail.to || '');
  const fwdDate = escapeForText(mail.date || '');
  const fwdSubj = escapeForText(mail.subject || '');

  // Build HTML forwarded body
  let bodyHtml = '';
  if (body) {
    const originalContent = body.html || esc(body.text || '').replace(/\n/g, '<br>');
    bodyHtml = `<br><br><div class="compose-quoted">
      <div class="compose-quoted-header">--- Forwarded message ---<br>From: ${fwdFrom}<br>To: ${fwdTo}<br>Date: ${fwdDate}<br>Subject: ${fwdSubj}</div>
      <blockquote>${originalContent}</blockquote>
    </div>`;
  }

  const forwardBody = body
    ? `\n\n--- Forwarded message ---\nFrom: ${fwdFrom}\nTo: ${fwdTo}\nDate: ${fwdDate}\nSubject: ${fwdSubj}\n\n${body.text || stripHtml(body.html || '')}`
    : '';

  return {
    to: '',
    subject: fwdSubject,
    body: forwardBody,
    bodyHtml,
  };
}

function stripHtml(html) {
  const tmp = document.createElement('div');
  tmp.innerHTML = html;
  return tmp.textContent || tmp.innerText || '';
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}

/** Escape a string for safe inclusion in plain text (strips HTML tags). */
function escapeForText(str) {
  if (!str) return '';
  return str.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ── Contact Auto-complete ──────────────────────────────────────────────

function setupAutocomplete(inputId, dropdownId) {
  const input = overlay.querySelector(`#${inputId}`);
  const dropdown = overlay.querySelector(`#${dropdownId}`);
  if (!input || !dropdown) return null;

  let debounce = null;
  let blurTimeout = null;

  input.addEventListener('input', () => {
    clearTimeout(debounce);
    debounce = setTimeout(async () => {
      // Get the text after the last comma (for multi-recipient fields)
      const val = input.value;
      const parts = val.split(',');
      const query = (parts[parts.length - 1] || '').trim();

      if (query.length < 2) {
        dropdown.hidden = true;
        return;
      }

      try {
        const contacts = await api.searchContacts(query);
        if (!contacts || contacts.length === 0) {
          dropdown.hidden = true;
          return;
        }

        dropdown.innerHTML = contacts
          .map(
            (c) =>
              `<div class="compose-autocomplete-item" data-email="${esc(c.email)}" data-name="${esc(c.name)}">
                <span class="compose-autocomplete-name">${esc(c.name || c.email)}</span>
                ${c.name ? `<span class="compose-autocomplete-email">&lt;${esc(c.email)}&gt;</span>` : ''}
              </div>`
          )
          .join('');
        dropdown.hidden = false;

        dropdown.querySelectorAll('.compose-autocomplete-item').forEach((item) => {
          item.addEventListener('mousedown', (e) => {
            e.preventDefault();
            const email = item.dataset.email;
            const name = item.dataset.name;

            // Validate email format before inserting
            if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
              return; // Skip invalid email entries
            }

            const display = name ? `${name} <${email}>` : email;

            // Replace the current part (after last comma) with the selected contact
            const parts = input.value.split(',');
            parts[parts.length - 1] = ' ' + display;
            input.value = parts.join(',') + ', ';
            dropdown.hidden = true;
            input.focus();
          });
        });
      } catch {
        dropdown.hidden = true;
      }
    }, 200);
  });

  input.addEventListener('blur', () => {
    // Small delay to allow click on dropdown items
    blurTimeout = setTimeout(() => {
      dropdown.hidden = true;
    }, 150);
  });

  // Return cleanup function to cancel pending timeouts
  return () => {
    clearTimeout(debounce);
    clearTimeout(blurTimeout);
  };
}

// ── Markdown to HTML converter ─────────────────────────────────────────

/**
 * Convert basic Markdown to HTML.
 * Supports: **bold**, *italic*, `code`, ```code blocks```, [text](url),
 * # headings (h1-h6), - unordered lists, 1. ordered lists, > blockquotes, --- hr.
 */
function markdownToHtml(md) {
  if (!md) return '';

  let html = esc(md);

  // Code blocks (``` ... ```)
  html = html.replace(/```([\s\S]*?)```/g, '<pre><code>$1</code></pre>');

  // Inline code
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Bold (**text** or __text__)
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/__(.+?)__/g, '<strong>$1</strong>');

  // Italic (*text* or _text_)
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  html = html.replace(/_(.+?)_/g, '<em>$1</em>');

  // Links [text](url)
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');

  // Headings (# to ######)
  html = html.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>');
  html = html.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>');
  html = html.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>');
  html = html.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');

  // Horizontal rule
  html = html.replace(/^---+$/gm, '<hr>');

  // Blockquotes
  html = html.replace(/^&gt;\s?(.+)$/gm, '<blockquote>$1</blockquote>');
  // Merge consecutive blockquotes
  html = html.replace(/<\/blockquote>\n<blockquote>/g, '\n');

  // Unordered lists (- item or * item)
  html = html.replace(/^[\-\*]\s+(.+)$/gm, '<li>$1</li>');
  html = html.replace(/((?:<li>.*<\/li>\n?)+)/g, '<ul>$1</ul>');

  // Ordered lists (1. item)
  html = html.replace(/^\d+\.\s+(.+)$/gm, '<li>$1</li>');
  // Wrap consecutive <li> not inside <ul> into <ol>
  html = html.replace(/(<li>.*<\/li>\n?)+/g, (match) => {
    if (match.includes('<ul>')) return match;
    return '<ol>' + match + '</ol>';
  });

  // Convert remaining newlines to <br>
  html = html.replace(/\n/g, '<br>');

  // Clean up <br> inside block elements
  html = html.replace(/<br>(<\/?(h[1-6]|ul|ol|li|blockquote|pre|hr))/g, '$1');
  html = html.replace(/(<(h[1-6]|ul|ol|li|blockquote|pre|hr)[^>]*>)<br>/g, '$1');

  return html;
}
