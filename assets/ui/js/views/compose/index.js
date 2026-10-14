// Exospine: the compose window (rich text and Markdown), with undo send,
// scheduled send, templates and draft versions.

import * as api from '../../api.js';
import { showToast } from '../../components/toast.js';
import { t, tn } from '../../i18n/index.js';
import { esc } from '../../html.js';
import { currentDraftId, startAutoSave, stopAutoSave } from './drafts.js';
import {
  fallbackSignature, buildInitialContent, htmlToPlainText, markdownToHtml, overlay,
  placeCursorAtStart, setupAutocomplete, setupFileDrop, setupImagePaste, setupToolbar,
} from './editor.js';
import { setupTemplatesDropdown } from './templates.js';

export { makeForwardPrefill, makeReplyPrefill } from './prefill.js';

// Undo Send: the send waiting out its 5 seconds
let _sendTimeout = null;

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
  let accountSignatureHtml = fallbackSignature();
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
        <button class="compose-close" id="compose-close" title="${t('close')}" aria-label="${t('close_compose')}">\u00D7</button>
      </div>
      <div class="compose-fields">
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-to">${t('to')}</label>
          <input type="text" class="compose-field-input" id="compose-to" value="${esc(prefill.to || '')}" placeholder="recipient@example.com" autocomplete="off" aria-label="${t('to_recipients')}" />
          <div class="compose-autocomplete" id="ac-to" hidden role="listbox" aria-label="${t('contact_suggestions')}"></div>
        </div>
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-cc">${t('cc')}</label>
          <input type="text" class="compose-field-input" id="compose-cc" value="${esc(prefill.cc || '')}" placeholder="" autocomplete="off" aria-label="${t('cc_recipients')}" />
          <div class="compose-autocomplete" id="ac-cc" hidden role="listbox" aria-label="${t('contact_suggestions')}"></div>
        </div>
        <div class="compose-field compose-field-relative">
          <label class="compose-field-label" for="compose-bcc">${t('bcc')}</label>
          <input type="text" class="compose-field-input" id="compose-bcc" value="${esc(prefill.bcc || '')}" placeholder="" autocomplete="off" aria-label="${t('bcc_recipients')}" />
          <div class="compose-autocomplete" id="ac-bcc" hidden role="listbox" aria-label="${t('contact_suggestions')}"></div>
        </div>
        <div class="compose-field">
          <label class="compose-field-label" for="compose-subject">${t('subject')}</label>
          <input type="text" class="compose-field-input" id="compose-subject" value="${esc(prefill.subject || '')}" placeholder="${t('subject')}" aria-label="${t('email_subject')}" />
        </div>
        <div class="compose-field">
          <label class="compose-field-label" for="compose-importance">${t('priority')}</label>
          <select id="compose-importance" class="compose-field-input compose-importance-select" aria-label="${t('email_priority')}">
            <option value="normal">${t('priority_normal')}</option>
            <option value="high">\u2757 ${t('priority_high')}</option>
            <option value="low">\u2193 ${t('priority_low')}</option>
          </select>
        </div>
      </div>
      <div class="compose-body-wrap">
        <div class="compose-toolbar" id="compose-toolbar" role="toolbar" aria-label="${t('formatting_toolbar')}">
          <button type="button" data-cmd="bold" title="${t('bold_shortcut')}" aria-label="${t('bold')}"><b>B</b></button>
          <button type="button" data-cmd="italic" title="${t('italic_shortcut')}" aria-label="${t('italic')}"><i>I</i></button>
          <button type="button" data-cmd="underline" title="${t('underline_shortcut')}" aria-label="${t('underline')}"><u>U</u></button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="insertUnorderedList" title="${t('bullet_list')}" aria-label="${t('bullet_list')}">\u2022</button>
          <button type="button" data-cmd="insertOrderedList" title="${t('numbered_list')}" aria-label="${t('numbered_list')}">1.</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <select data-cmd="fontSize" title="${t('font_size')}" class="toolbar-select" aria-label="${t('font_size')}">
            <option value="">${t('size')}</option>
            <option value="1">${t('size_small')}</option>
            <option value="3">${t('size_normal')}</option>
            <option value="5">${t('size_large')}</option>
          </select>
          <input type="color" data-cmd="foreColor" title="${t('text_color')}" class="toolbar-color" value="#000000" aria-label="${t('text_color')}" />
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="justifyLeft" title="${t('align_left')}" aria-label="${t('align_left')}">\u2261</button>
          <button type="button" data-cmd="justifyCenter" title="${t('align_center')}" aria-label="${t('align_center')}">\u2550</button>
          <button type="button" data-cmd="justifyRight" title="${t('align_right')}" aria-label="${t('align_right')}">\u2262</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button type="button" data-cmd="createLink" title="${t('insert_link')}" aria-label="${t('insert_link')}">\uD83D\uDD17</button>
          <button type="button" data-cmd="insertHorizontalRule" title="${t('horizontal_line')}" aria-label="${t('insert_horizontal_line')}">\u2014</button>
          <button type="button" data-cmd="removeFormat" title="${t('clear_formatting')}" aria-label="${t('clear_formatting')}">\u2715</button>
          <span class="toolbar-sep" aria-hidden="true"></span>
          <div class="toolbar-dropdown-wrapper">
            <button type="button" id="compose-templates-btn" title="${t('templates')}" aria-label="${t('templates')}" aria-expanded="false">\uD83D\uDCCB ${t('templates')}</button>
            <div id="compose-templates-dropdown" hidden class="compose-templates-dropdown"></div>
          </div>
          <span class="toolbar-sep"></span>
          <button type="button" id="compose-md-toggle" title="${t('toggle_markdown')}" class="text-xs font-bolder">MD</button>
        </div>
        <div id="compose-body" contenteditable="true" spellcheck="true" lang="auto" class="compose-editor">${buildInitialContent(prefill)}</div>
        <textarea id="compose-body-md" class="compose-editor-md" hidden placeholder="${esc(t('markdown_placeholder'))}"></textarea>
      </div>
      <div id="compose-attachments" class="compose-attachments" hidden>
        <div class="compose-attachments-label">${t('attachments_colon')}</div>
        <div id="compose-attachments-list" class="compose-attachments-list"></div>
      </div>
      <div id="compose-version-panel" hidden class="compose-version-panel">
        <div class="compose-version-panel-title">${t('version_history')}</div>
        <div id="compose-version-list" class="text-dim">${t('no_versions')}</div>
      </div>
      <div class="compose-status-bar" id="compose-status-bar">
        <span id="compose-word-count">${tn('word_count', 0)}</span>
        <span id="compose-char-count">${tn('char_count', 0)}</span>
      </div>
      <div class="compose-footer">
        <button class="btn btn-ghost" id="compose-discard">${t('discard')}</button>
        <button class="btn btn-ghost text-xs" id="compose-versions-btn" title="${t('version_history')}">\uD83D\uDD53 ${t('versions')}</button>
        <div class="compose-send-group">
          <button class="btn btn-primary" id="compose-send">\u2709 ${t('send')}</button>
          <div class="compose-field-relative">
            <button class="btn btn-ghost" id="compose-schedule-btn" title="${t('schedule_send')}">\u23F0</button>
            <div class="compose-schedule-dropdown" id="compose-schedule-dropdown" hidden>
              <label>${t('schedule_send_colon')}</label>
              <input type="datetime-local" id="compose-schedule-time" />
              <button class="btn btn-primary btn-sm w-full mt-6" id="compose-schedule-confirm">${t('schedule')}</button>
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
    if (wordEl) wordEl.textContent = tn('word_count', words);
    if (charEl) charEl.textContent = tn('char_count', chars);
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
        showToast(t('pick_date_time_first'), 'error');
        return;
      }
      const to = overlay.querySelector('#compose-to').value.trim();
      if (!to) {
        showToast(t('enter_recipient'), 'error');
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
        showToast(t('email_scheduled', { time: timeInput.value }), 'success');
        close();
      } catch (err) {
        showToast(t('error_schedule', { error: err }), 'error');
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
      if (!currentDraftId()) {
        versionList.innerHTML = `<div class="text-dim">${t('no_versions')}</div>`;
        return;
      }
      try {
        const versions = await api.loadDraftVersions(currentDraftId());
        if (!versions || versions.length === 0) {
          versionList.innerHTML = `<div class="text-dim">${t('no_versions')}</div>`;
          return;
        }
        versionList.innerHTML = versions.map((v) => {
          const ts = new Date(v.timestamp).toLocaleString();
          const preview = (v.subject || '').slice(0, 40) || t('version_no_subject');
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
        versionList.innerHTML = `<div class="text-danger">${t('error_load_versions')}</div>`;
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
      showToast(t('enter_recipient'), 'error');
      return;
    }

    // Basic email validation
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(to)) {
      showToast(t('invalid_email'), 'error');
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
    const toastEl = showToast(t('sent_undo'), 'info', 5500, () => {
      // Undo callback — cancel the pending send
      if (_sendTimeout) {
        clearTimeout(_sendTimeout);
        _sendTimeout = null;
      }
      showToast(t('send_cancelled'), 'info');
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
        showToast(t('email_delivered'), 'success');
        if (onSent) onSent();
      } catch (e) {
        showToast(t('error_send', { error: e }), 'error');
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
