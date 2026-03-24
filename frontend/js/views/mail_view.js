// Exospine — Mail reading pane view

import { formatFullDate } from '../date_format.js';
import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { t } from '../i18n.js';

// ── Zoom state (persisted across mail selections) ──────────────────
let _zoomLevel = parseFloat(localStorage.getItem('exospine_zoom') || '100');
let _zoomSaveTimer = null;
function _saveZoomDebounced(val) {
  clearTimeout(_zoomSaveTimer);
  _zoomSaveTimer = setTimeout(() => localStorage.setItem('exospine_zoom', String(val)), 1000);
}
const _dismissedReceipts = new Set(); // Track dismissed read receipts per session

// ── Attachment preview cache ───────────────────────────────────────
const _previewCache = new Map(); // "accountId:folder:uid:partIndex" -> data

/**
 * Render the reading pane.
 * @param {HTMLElement} el  #mail-view element
 * @param {object} state    app state (selectedMail, mailBody)
 * @param {object} actions  { onReply, onReplyAll, onForward, onArchive, onDelete, onMarkUnread }
 */
export function renderMailView(el, state, actions) {
  const mail = state.selectedMail;

  if (!mail) {
    el.innerHTML = `<div class="mail-view-empty" role="status" aria-live="polite">${t('select_email')}</div>`;
    return;
  }

  const body = state.mailBody;
  if (body === undefined) {
    el.innerHTML = '<div class="mail-view-body-text" style="color:var(--pane-text-dim);text-align:center;padding-top:40px;" role="status" aria-live="polite">Loading email body\u2026</div>';
    return;
  }
  const hasHtml = body && typeof body.html === 'string' && body.html.length > 0;
  const hasText = body && typeof body.text === 'string' && body.text.length > 0;
  const hasRemoteImages = hasHtml && containsRemoteImages(body.html);

  // Security data from backend
  const phishingWarnings = (body && body.phishing_warnings) || [];
  const authStatus = (body && body.auth_status) || null;
  const senderWarnings = (body && body.sender_warnings) || [];

  let html = '';

  // Header
  html += `
    <div class="mail-view-header">
      <div class="mail-view-subject">${esc(mail.subject || '(No subject)')}</div>
      <div class="mail-view-meta">
        <div class="mail-view-meta-row">
          <span class="mail-view-meta-label" id="mv-from-label">From</span>
          <span class="mail-view-meta-value" aria-labelledby="mv-from-label">${esc(mail.from || 'Unknown')} <button class="mail-view-copy-email" data-copy-email="${esc(mail.from || '')}" title="Copy email address" aria-label="Copy email address" style="background:none;border:none;cursor:pointer;font-size:13px;color:var(--accent);padding:0 4px;vertical-align:middle;">\u{1F4CB}</button></span>
        </div>
        <div class="mail-view-meta-row">
          <span class="mail-view-meta-label" id="mv-to-label">To</span>
          <span class="mail-view-meta-value" aria-labelledby="mv-to-label">${esc(mail.to || '')}</span>
        </div>
        ${mail.cc ? `
        <div class="mail-view-meta-row">
          <span class="mail-view-meta-label" id="mv-cc-label">CC</span>
          <span class="mail-view-meta-value" aria-labelledby="mv-cc-label">${esc(mail.cc)}</span>
        </div>
        ` : ''}
      </div>
      <div class="mail-view-date">${formatFullDate(mail.date)}</div>
    </div>
  `;

  // Security indicators
  html += renderSecurityBanners(authStatus, phishingWarnings, senderWarnings);

  // Read receipt banner
  if (mail.read_receipt_requested && mail.read_receipt_to && !_dismissedReceipts.has(mail.id)) {
    html += `
      <div class="mail-view-receipt-banner" id="receipt-banner" role="alert">
        <span>\u2709 The sender requested a read receipt.</span>
        <div style="display:flex;gap:6px;">
          <button id="send-receipt-btn" class="btn btn-sm btn-primary" aria-label="Send read receipt">Send receipt</button>
          <button id="ignore-receipt-btn" class="btn btn-sm btn-ghost" aria-label="Ignore read receipt request">Ignore</button>
        </div>
      </div>
    `;
  }

  // Toolbar
  html += `
    <div class="mail-view-toolbar" role="toolbar" aria-label="Email actions">
      <button class="mail-view-toolbar-btn" data-action="reply" aria-label="${t('reply')}">\u21A9 ${t('reply')}</button>
      <button class="mail-view-toolbar-btn" data-action="reply-all" aria-label="${t('reply_all')}">\u21A9\u21A9 ${t('reply_all')}</button>
      <button class="mail-view-toolbar-btn" data-action="forward" aria-label="${t('forward')}">\u21AA ${t('forward')}</button>
      <div class="mail-view-toolbar-spacer"></div>
      <button class="mail-view-toolbar-btn" data-action="print" aria-label="${t('print')}">\uD83D\uDDA8 ${t('print')}</button>
      <button class="mail-view-toolbar-btn" data-action="export" aria-label="${t('export')}">\u2B07 ${t('export')}</button>
      <button class="mail-view-toolbar-btn" data-action="view-headers" aria-label="${t('headers')}">\uD83D\uDD0D ${t('headers')}</button>
      <button class="mail-view-toolbar-btn" data-action="mark-unread" aria-label="${t('mark_unread')}">\u2709 ${t('mark_unread')}</button>
      <div style="position:relative;display:inline-block;">
        <button class="mail-view-toolbar-btn" data-action="snooze" aria-label="Snooze email" aria-expanded="false" aria-controls="snooze-dropdown">\u23F0 Snooze</button>
        <div class="snooze-dropdown" id="snooze-dropdown" hidden
             style="position:absolute;top:100%;right:0;background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:6px;padding:4px 0;z-index:100;min-width:180px;box-shadow:0 4px 12px rgba(0,0,0,0.3);">
          <div class="snooze-option" data-snooze="later-today" style="padding:6px 12px;cursor:pointer;font-size:13px;" role="button" tabindex="0" aria-label="Snooze for 4 hours">Later today (4 hours)</div>
          <div class="snooze-option" data-snooze="tomorrow" style="padding:6px 12px;cursor:pointer;font-size:13px;" role="button" tabindex="0" aria-label="Snooze until tomorrow morning">Tomorrow morning</div>
          <div class="snooze-option" data-snooze="next-week" style="padding:6px 12px;cursor:pointer;font-size:13px;" role="button" tabindex="0" aria-label="Snooze until next week">Next week</div>
          <div style="border-top:1px solid var(--pane-border);margin:4px 0;"></div>
          <div style="padding:6px 12px;">
            <label for="snooze-custom-time" style="font-size:11px;display:block;margin-bottom:4px;">Pick date & time:</label>
            <input type="datetime-local" id="snooze-custom-time" aria-label="Custom snooze date and time" style="width:100%;padding:4px;border-radius:4px;border:1px solid var(--pane-border);background:var(--pane-bg);color:var(--pane-text);font-size:12px;" />
            <button class="snooze-option" data-snooze="custom" style="padding:4px 8px;cursor:pointer;font-size:12px;margin-top:4px;width:100%;text-align:center;border:1px solid var(--pane-border);border-radius:4px;" aria-label="Set custom snooze time">Set</button>
          </div>
        </div>
      </div>
      <button class="mail-view-toolbar-btn" data-action="archive" aria-label="${t('archive')}">\uD83D\uDCE6 ${t('archive')}</button>
      <button class="mail-view-toolbar-btn danger" data-action="delete" aria-label="${t('delete')}">\uD83D\uDDD1 ${t('delete')}</button>
      <span class="toolbar-sep" style="width:1px;height:18px;background:var(--pane-border);margin:0 4px;" aria-hidden="true"></span>
      <div class="zoom-controls" style="display:flex;align-items:center;gap:2px;">
        <button class="mail-view-toolbar-btn" data-action="zoom-out" aria-label="Zoom out" title="Zoom out" style="padding:4px 8px;font-size:14px;">\u2212</button>
        <span id="zoom-level-display" style="font-size:11px;color:var(--pane-text-dim);min-width:36px;text-align:center;" aria-live="polite">${Math.round(_zoomLevel)}%</span>
        <button class="mail-view-toolbar-btn" data-action="zoom-in" aria-label="Zoom in" title="Zoom in" style="padding:4px 8px;font-size:14px;">+</button>
        <button class="mail-view-toolbar-btn" data-action="zoom-reset" aria-label="Reset zoom" title="Reset zoom" style="padding:4px 8px;font-size:11px;">100%</button>
      </div>
    </div>
  `;

  // Feature 1: Attachment bar + preview
  const attachments = mail.attachment_meta || [];
  if (attachments.length > 0 || mail.has_attachments) {
    html += '<div class="mail-view-attachments" role="region" aria-label="Attachments">';
    html += `<div class="mail-view-attachments-header">\uD83D\uDCCE ${attachments.length} attachment${attachments.length !== 1 ? 's' : ''}</div>`;
    html += '<div class="mail-view-attachments-list">';
    for (const att of attachments) {
      const icon = attachmentIcon(att.content_type);
      const size = formatSize(att.size);
      const previewType = getPreviewType(att);
      html += `
        <div class="mail-view-attachment-item${previewType ? ' previewable' : ''}" data-part-index="${att.part_index}" data-preview-type="${previewType || ''}" data-content-type="${esc(att.content_type || '')}" data-filename="${esc(att.filename || '')}" role="button" tabindex="0" aria-label="${esc(att.filename)} (${size})${previewType ? ', click to preview' : ''}">
          <span class="mail-view-attachment-icon" aria-hidden="true">${icon}</span>
          <span class="mail-view-attachment-name" title="${esc(att.filename)}">${esc(att.filename)}</span>
          <span class="mail-view-attachment-size">${size}</span>
          <button class="mail-view-attachment-download" data-part-index="${att.part_index}" aria-label="Download ${esc(att.filename)}" title="Download">\u2B07</button>
        </div>
      `;
    }
    html += '</div>';

    // Collapsible preview section
    html += `
      <div class="attachment-preview-section" id="attachment-preview-section" hidden aria-expanded="false">
        <button class="attachment-preview-close" id="attachment-preview-close" aria-label="Close preview">\u00D7 Close preview</button>
        <div class="attachment-preview-content" id="attachment-preview-content" aria-live="polite"></div>
      </div>
    `;
    html += '</div>';
  }

  // Image blocking banner (shown only for HTML with remote images)
  if (hasRemoteImages) {
    html += `
      <div class="mail-view-image-banner" id="image-banner" role="alert">
        <span>${t('remote_images_blocked')}</span>
        <button id="show-images-btn" aria-label="${t('show_images')}">${t('show_images')}</button>
      </div>
    `;
  }

  // Body — with quoted text collapsing for plain text
  html += '<div class="mail-view-body" aria-live="polite">';
  if (hasHtml) {
    html += '<iframe id="email-frame" sandbox="allow-same-origin" title="Email content"></iframe>';
  } else if (hasText) {
    html += `<div class="mail-view-body-text">${collapseQuotedText(esc(body.text))}</div>`;
  } else if (body === null) {
    html += '<div class="mail-view-body-text" style="color:var(--pane-text-dim);text-align:center;padding-top:40px;" role="status">Loading email body\u2026</div>';
  } else {
    html += '<div class="mail-view-body-text" style="color:var(--pane-text-dim);text-align:center;padding-top:40px;" role="status">No content available</div>';
  }
  html += '</div>';

  // Link safety: list all links found in the email
  if (hasHtml) {
    const links = extractLinks(body.html);
    if (links.length > 0) {
      const senderDomain = extractDomain(mail.from || '');
      html += '<details class="mail-view-links-section" aria-label="Links found in this email">';
      html += `<summary>Links in this email (${links.length})</summary>`;
      html += '<ul class="mail-view-links-list">';
      for (const link of links) {
        const linkDomain = extractDomainFromUrl(link.href);
        const mismatch = senderDomain && linkDomain && !linkDomain.endsWith(senderDomain);
        const cls = mismatch ? ' class="mail-view-link-domain-mismatch" title="Domain differs from sender"' : '';
        html += `<li${cls}>${esc(link.text || link.href)} &rarr; ${esc(link.href)}${mismatch ? ' \u26A0' : ''}</li>`;
      }
      html += '</ul></details>';

      // GitHub / Jira link preview cards
      const devLinks = extractDevLinks(links);
      if (devLinks.length > 0) {
        html += '<div class="mail-view-dev-links" style="margin-top:8px;">';
        for (const dl of devLinks) {
          const typeIcon = dl.type === 'pull' ? '\uD83D\uDD00' : (dl.type === 'issue' ? '\uD83D\uDCDD' : '\uD83D\uDCCB');
          const typeLabel = dl.type === 'pull' ? 'Pull Request' : (dl.type === 'issue' ? 'Issue' : 'Ticket');
          const platformLabel = dl.platform === 'github' ? 'GitHub' : 'Jira';
          html += `
            <a href="${esc(dl.url)}" target="_blank" rel="noopener noreferrer" class="dev-link-card" style="display:flex;align-items:center;gap:8px;padding:8px 12px;margin-bottom:6px;border:1px solid var(--pane-border);border-radius:6px;text-decoration:none;color:var(--pane-text);background:var(--pane-header-bg);transition:background 150ms ease;" onmouseenter="this.style.background='var(--accent-light)'" onmouseleave="this.style.background='var(--pane-header-bg)'">
              <span style="font-size:18px;" aria-hidden="true">${typeIcon}</span>
              <div style="flex:1;min-width:0;">
                <div style="font-weight:500;font-size:13px;">${esc(dl.repo)}${dl.number ? ' #' + esc(String(dl.number)) : ''}</div>
                <div style="font-size:11px;color:var(--pane-text-dim);">${platformLabel} ${typeLabel}</div>
              </div>
              <span style="font-size:11px;color:var(--accent);">Open &rarr;</span>
            </a>
          `;
        }
        html += '</div>';
      }
    }
  }

  el.innerHTML = html;

  // ── Copy email address on click ──
  el.querySelectorAll('.mail-view-copy-email').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const fromRaw = btn.dataset.copyEmail || '';
      // Extract email from "Name <email@example.com>" format
      const emailMatch = fromRaw.match(/<([^>]+)>/);
      const email = emailMatch ? emailMatch[1] : fromRaw.trim();
      navigator.clipboard.writeText(email).then(() => {
        showToast('Email copied.', 'info');
      }).catch(() => {
        showToast('Failed to copy email.', 'error');
      });
    });
  });

  // ── Quoted text toggle — delegated listener (no per-element leak) ──
  el.addEventListener('click', (e) => {
    const toggle = e.target.closest('.quoted-text-toggle');
    if (!toggle) return;
    const quotedBlock = toggle.nextElementSibling;
    if (quotedBlock) {
      const isHidden = quotedBlock.hidden;
      quotedBlock.hidden = !isHidden;
      toggle.setAttribute('aria-expanded', String(!isHidden));
      toggle.textContent = isHidden ? '\u22EF Hide quoted text' : '\u22EF Show quoted text';
    }
  });
  el.addEventListener('keydown', (e) => {
    const toggle = e.target.closest('.quoted-text-toggle');
    if (!toggle) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggle.click();
    }
  });

  // Inject HTML into iframe (with remote images stripped by default)
  if (hasHtml) {
    const iframe = el.querySelector('#email-frame');
    if (iframe) {
      const sanitized = hasRemoteImages ? stripRemoteImages(body.html) : body.html;
      writeToIframe(iframe, sanitized);

      const showBtn = el.querySelector('#show-images-btn');
      if (showBtn) {
        showBtn.addEventListener('click', () => {
          // Instead of rewriting the entire iframe (causes flash),
          // restore original src on all blocked images inside the iframe
          try {
            const iframeDoc = iframe.contentDocument || iframe.contentWindow?.document;
            if (iframeDoc) {
              const imgs = iframeDoc.querySelectorAll('img[data-original-src]');
              imgs.forEach(img => {
                img.src = img.getAttribute('data-original-src');
                img.removeAttribute('data-original-src');
              });
            } else {
              // Fallback: rewrite iframe if can't access document (sandbox restriction)
              writeToIframe(iframe, body.html);
            }
          } catch {
            // Sandbox may block access — fallback to full rewrite
            writeToIframe(iframe, body.html);
          }
          const banner = el.querySelector('#image-banner');
          if (banner) banner.remove();
        });
      }
    }
  }

  // Read receipt events
  const sendReceiptBtn = el.querySelector('#send-receipt-btn');
  if (sendReceiptBtn) {
    sendReceiptBtn.addEventListener('click', async () => {
      try {
        await api.sendReadReceipt(mail.id, mail.account_id, mail.read_receipt_to, mail.subject, mail.from);
        showToast('Read receipt sent.', 'success');
        _dismissedReceipts.add(mail.id);
        const banner = el.querySelector('#receipt-banner');
        if (banner) banner.remove();
      } catch (err) {
        showToast(`Failed to send receipt: ${err}`, 'error');
      }
    });
  }
  const ignoreReceiptBtn = el.querySelector('#ignore-receipt-btn');
  if (ignoreReceiptBtn) {
    ignoreReceiptBtn.addEventListener('click', () => {
      _dismissedReceipts.add(mail.id);
      const banner = el.querySelector('#receipt-banner');
      if (banner) banner.remove();
    });
  }

  // Zoom controls
  function applyZoom() {
    const iframe = el.querySelector('#email-frame');
    const textEl = el.querySelector('.mail-view-body-text');
    if (iframe) {
      iframe.style.transform = `scale(${_zoomLevel / 100})`;
      iframe.style.transformOrigin = 'top left';
      iframe.style.width = `${10000 / _zoomLevel}%`;
      iframe.style.height = `${10000 / _zoomLevel}%`;
    }
    if (textEl) {
      textEl.style.fontSize = `${14 * _zoomLevel / 100}px`;
    }
    const display = el.querySelector('#zoom-level-display');
    if (display) display.textContent = `${Math.round(_zoomLevel)}%`;
    _saveZoomDebounced(_zoomLevel);
  }
  applyZoom();

  // Toolbar events
  const toolbarActions = {
    'reply': actions.onReply,
    'reply-all': actions.onReplyAll,
    'forward': actions.onForward,
    'archive': () => { if (actions.onArchive) { actions.onArchive(mail); showToast('Email archived.', 'success'); } },
    'delete': () => { if (actions.onDelete) { actions.onDelete(mail); showToast('Email deleted.', 'success'); } },
    'mark-unread': () => { if (actions.onMarkUnread) { actions.onMarkUnread(mail); showToast('Email marked as unread.', 'success'); } },
    'print': () => printEmail(mail, body),
    'export': () => exportEmail(mail),
    'view-headers': () => viewHeaders(mail),
    'zoom-in': () => { _zoomLevel = Math.min(200, _zoomLevel + 10); applyZoom(); },
    'zoom-out': () => { _zoomLevel = Math.max(50, _zoomLevel - 10); applyZoom(); },
    'zoom-reset': () => { _zoomLevel = 100; applyZoom(); },
  };

  el.querySelectorAll('.mail-view-toolbar-btn').forEach((btn) => {
    const action = btn.dataset.action;
    if (action === 'snooze') {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const dropdown = el.querySelector('#snooze-dropdown');
        if (dropdown) {
          dropdown.hidden = !dropdown.hidden;
          btn.setAttribute('aria-expanded', String(!dropdown.hidden));
        }
      });
      return;
    }
    const handler = toolbarActions[action];
    if (handler) {
      btn.addEventListener('click', () => handler(mail));
    }
  });

  // Snooze dropdown options
  el.querySelectorAll('.snooze-option').forEach((opt) => {
    opt.addEventListener('click', async () => {
      const type = opt.dataset.snooze;
      let until;
      const now = new Date();

      if (type === 'later-today') {
        until = new Date(now.getTime() + 4 * 60 * 60 * 1000).toISOString();
      } else if (type === 'tomorrow') {
        const tomorrow = new Date(now);
        tomorrow.setDate(tomorrow.getDate() + 1);
        tomorrow.setHours(9, 0, 0, 0);
        until = tomorrow.toISOString();
      } else if (type === 'next-week') {
        const nextWeek = new Date(now);
        nextWeek.setDate(nextWeek.getDate() + 7);
        nextWeek.setHours(9, 0, 0, 0);
        until = nextWeek.toISOString();
      } else if (type === 'custom') {
        const timeInput = el.querySelector('#snooze-custom-time');
        if (!timeInput || !timeInput.value) {
          showToast('Please select a date and time.', 'error');
          return;
        }
        until = new Date(timeInput.value).toISOString();
      }

      if (!until) return;

      try {
        await api.snoozeMail(mail.id, until);
        showToast('Email snoozed.', 'success');
        if (actions.onSnooze) actions.onSnooze(mail);
        const dropdown = el.querySelector('#snooze-dropdown');
        if (dropdown) dropdown.hidden = true;
      } catch (err) {
        showToast(`Failed to snooze: ${err}`, 'error');
      }
    });
  });

  // Close snooze dropdown on outside click
  document.addEventListener('click', () => {
    const dropdown = el.querySelector('#snooze-dropdown');
    if (dropdown) {
      dropdown.hidden = true;
      const snoozeBtn = el.querySelector('[data-action="snooze"]');
      if (snoozeBtn) snoozeBtn.setAttribute('aria-expanded', 'false');
    }
  });

  // ── Attachment preview + download ──────────────────────────────
  el.querySelectorAll('.mail-view-attachment-download').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const partIndex = parseInt(btn.dataset.partIndex, 10);
      await handleDownloadAttachment(mail, partIndex);
    });
  });

  // Attachment preview click (on the item itself, not the download button)
  el.querySelectorAll('.mail-view-attachment-item.previewable').forEach((item) => {
    item.addEventListener('click', (e) => {
      if (e.target.closest('.mail-view-attachment-download')) return;
      const partIndex = parseInt(item.dataset.partIndex, 10);
      const previewType = item.dataset.previewType;
      const contentType = item.dataset.contentType;
      const filename = item.dataset.filename;
      openAttachmentPreview(el, mail, partIndex, previewType, contentType, filename);
    });
    item.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        if (!e.target.closest('.mail-view-attachment-download')) {
          item.click();
        }
      }
    });
  });

  // Attachment preview close button
  const previewCloseBtn = el.querySelector('#attachment-preview-close');
  if (previewCloseBtn) {
    previewCloseBtn.addEventListener('click', () => {
      const section = el.querySelector('#attachment-preview-section');
      if (section) {
        section.hidden = true;
        section.setAttribute('aria-expanded', 'false');
      }
    });
  }
}

// ── Feature: Determine preview type from attachment ────────────────
function getPreviewType(att) {
  const ct = (att.content_type || '').toLowerCase();
  const fn = (att.filename || '').toLowerCase();

  if (ct.startsWith('image/') || /\.(png|jpe?g|gif|webp|svg|bmp)$/.test(fn)) {
    return 'image';
  }
  if (ct.includes('pdf') || fn.endsWith('.pdf')) {
    return 'pdf';
  }
  if (ct.startsWith('text/') || /\.(txt|csv|json|xml|md|log|ini|cfg|yaml|yml|toml)$/.test(fn)) {
    return 'text';
  }
  return null;
}

// ── Feature: Open attachment preview ───────────────────────────────
async function openAttachmentPreview(el, mail, partIndex, previewType, contentType, filename) {
  const section = el.querySelector('#attachment-preview-section');
  const content = el.querySelector('#attachment-preview-content');
  if (!section || !content) return;

  section.hidden = false;
  section.setAttribute('aria-expanded', 'true');
  content.innerHTML = '<div style="padding:16px;color:var(--pane-text-dim);">Loading preview...</div>';

  const cacheKey = `${mail.account_id}:${mail.folder}:${mail.uid}:${partIndex}`;

  try {
    let data = _previewCache.get(cacheKey);
    if (!data) {
      data = await api.getAttachmentContent(mail.account_id, mail.folder, mail.uid, partIndex);
      _previewCache.set(cacheKey, data);
    }

    if (previewType === 'image') {
      const src = typeof data === 'string' && data.startsWith('data:')
        ? data
        : `data:${contentType || 'image/png'};base64,${data}`;
      content.innerHTML = `
        <div class="attachment-preview-image-wrap">
          <img src="${src}" alt="${esc(filename)}" class="attachment-preview-thumbnail" tabindex="0" role="button" aria-label="Click to view full size: ${esc(filename)}" />
        </div>
      `;
      const img = content.querySelector('.attachment-preview-thumbnail');
      if (img) {
        const openLightbox = () => showImageLightbox(src, filename);
        img.addEventListener('click', openLightbox);
        img.addEventListener('keydown', (e) => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openLightbox(); }
        });
      }
    } else if (previewType === 'pdf') {
      const src = typeof data === 'string' && data.startsWith('data:')
        ? data
        : `data:application/pdf;base64,${data}`;
      content.innerHTML = `
        <iframe src="${src}" class="attachment-preview-pdf" title="PDF preview: ${esc(filename)}"></iframe>
      `;
    } else if (previewType === 'text') {
      const textContent = typeof data === 'string' && !data.startsWith('data:')
        ? atob(data)
        : data;
      content.innerHTML = `
        <pre class="attachment-preview-text" tabindex="0" aria-label="Text file content: ${esc(filename)}">${esc(textContent)}</pre>
      `;
    }
  } catch (err) {
    content.innerHTML = `<div style="padding:16px;color:var(--danger);">Failed to load preview: ${esc(String(err))}</div>`;
  }
}

// ── Feature: Image lightbox ────────────────────────────────────────
function showImageLightbox(src, filename) {
  const overlay = document.createElement('div');
  overlay.className = 'lightbox-overlay';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', `Image preview: ${filename}`);
  overlay.innerHTML = `
    <button class="lightbox-close" aria-label="Close image preview">\u00D7</button>
    <img src="${src}" alt="${esc(filename)}" class="lightbox-image" />
  `;

  document.body.appendChild(overlay);

  const close = () => overlay.remove();
  overlay.querySelector('.lightbox-close').addEventListener('click', close);
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

  requestAnimationFrame(() => {
    overlay.querySelector('.lightbox-close').focus();
  });
}

// ── Feature: Collapse quoted text in plain text emails ─────────────
function collapseQuotedText(text) {
  // Pattern 1: "On ... wrote:" followed by the rest
  text = text.replace(
    /(On .+wrote:[\s\S]+$)/m,
    '<div class="quoted-text-toggle" role="button" tabindex="0" aria-expanded="false" aria-label="Show quoted text">\u22EF Show quoted text</div><div class="quoted-text" hidden>$1</div>'
  );

  // Pattern 2: Lines starting with ">" (if not already handled)
  if (!text.includes('quoted-text-toggle')) {
    const lines = text.split('\n');
    let inQuote = false;
    let quoteStart = -1;
    const result = [];

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const isQuotedLine = /^(&gt;|>)\s?/.test(line);

      if (isQuotedLine && !inQuote) {
        inQuote = true;
        quoteStart = i;
      } else if (!isQuotedLine && inQuote) {
        const quotedLines = lines.slice(quoteStart, i);
        if (quotedLines.length >= 3) {
          result.push('<div class="quoted-text-toggle" role="button" tabindex="0" aria-expanded="false" aria-label="Show quoted text">\u22EF Show quoted text</div>');
          result.push('<div class="quoted-text" hidden>' + quotedLines.join('\n') + '</div>');
        } else {
          result.push(...quotedLines);
        }
        inQuote = false;
        result.push(line);
      } else if (!isQuotedLine) {
        result.push(line);
      }
    }

    // Handle trailing quoted block
    if (inQuote) {
      const quotedLines = lines.slice(quoteStart);
      if (quotedLines.length >= 3) {
        result.push('<div class="quoted-text-toggle" role="button" tabindex="0" aria-expanded="false" aria-label="Show quoted text">\u22EF Show quoted text</div>');
        result.push('<div class="quoted-text" hidden>' + quotedLines.join('\n') + '</div>');
      } else {
        result.push(...quotedLines);
      }
    }

    text = result.join('\n');
  }

  return text;
}

/**
 * Download an attachment via the backend command.
 */
async function handleDownloadAttachment(mail, partIndex) {
  try {
    showToast('Downloading attachment...', 'info');
    const filePath = await api.downloadAttachment(
      mail.account_id,
      mail.folder,
      mail.uid,
      partIndex,
    );
    showToast(`Saved to ${filePath}`, 'success');
  } catch (err) {
    showToast(`Download failed: ${err}`, 'error');
  }
}

/**
 * Print the current email.
 */
function printEmail(mail, body) {
  if (!body) return;
  const content = body.html || `<pre style="white-space:pre-wrap;font-family:sans-serif;">${esc(body.text || '')}</pre>`;
  const printWindow = window.open('', '_blank');
  if (!printWindow) {
    showToast('Pop-up blocked. Please allow pop-ups.', 'error');
    return;
  }
  printWindow.document.write(`<!DOCTYPE html><html><head><meta charset="utf-8"><title>${esc(mail.subject || '')}</title><style>body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;margin:20px;font-size:14px;color:#1e1e1e;}img{max-width:100%;height:auto;}</style></head><body>${content}</body></html>`);
  printWindow.document.close();
  printWindow.print();
  printWindow.close();
}

/**
 * Export the email as .eml file.
 */
async function exportEmail(mail) {
  try {
    showToast('Exporting email...', 'info');
    const filePath = await api.exportMail(
      mail.account_id,
      mail.folder,
      mail.uid,
    );
    showToast(`Exported to ${filePath}`, 'success');
  } catch (err) {
    showToast(`Export failed: ${err}`, 'error');
  }
}

/**
 * Write HTML content into a sandboxed iframe.
 */
function writeToIframe(iframe, htmlContent) {
  // Strip <script> tags to prevent console spam from sandbox blocking
  const cleaned = htmlContent.replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '');
  const lazyHtml = addLazyLoading(cleaned);

  const wrapped = `
    <!DOCTYPE html>
    <html>
    <head>
      <meta charset="utf-8">
      <style>
        body {
          margin: 0;
          padding: 16px 24px;
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
          font-size: 14px;
          line-height: 1.6;
          color: #1e1e1e;
          word-break: break-word;
          overflow-wrap: break-word;
        }
        img { max-width: 100%; height: auto; }
        a { color: rgb(0, 120, 214); }
        pre, code { font-family: "Cascadia Code", "Fira Code", Consolas, monospace; }
        table { border-collapse: collapse; max-width: 100%; }
      </style>
    </head>
    <body>${lazyHtml}</body>
    </html>
  `;
  iframe.srcdoc = wrapped;
}

function addLazyLoading(html) {
  return html.replace(/<img(?![^>]*loading\s*=)/gi, '<img loading="lazy"');
}

// LRU cache for containsRemoteImages results (keyed by html string hash)
const _remoteImgCache = new Map();
const _REMOTE_IMG_CACHE_MAX = 50;

function containsRemoteImages(html) {
  // Use the first 200 chars + length as a cheap cache key
  const key = html.length + ':' + html.slice(0, 200);
  if (_remoteImgCache.has(key)) return _remoteImgCache.get(key);
  const result = /<img[^>]+src\s*=\s*["']https?:\/\//i.test(html);
  _remoteImgCache.set(key, result);
  // LRU eviction
  if (_remoteImgCache.size > _REMOTE_IMG_CACHE_MAX) {
    _remoteImgCache.delete(_remoteImgCache.keys().next().value);
  }
  return result;
}

function stripRemoteImages(html) {
  return html.replace(
    /(<img[^>]+)src\s*=\s*["'](https?:\/\/[^"']*)["']/gi,
    '$1src="data:image/svg+xml,%3Csvg xmlns=\'http://www.w3.org/2000/svg\' width=\'200\' height=\'100\'%3E%3Crect fill=\'%23eee\' width=\'200\' height=\'100\'/%3E%3Ctext x=\'50%25\' y=\'50%25\' dominant-baseline=\'middle\' text-anchor=\'middle\' fill=\'%23999\' font-size=\'12\'%3EImage blocked%3C/text%3E%3C/svg%3E" data-original-src="$2"'
  );
}

function attachmentIcon(contentType) {
  if (!contentType) return '\uD83D\uDCC4';
  if (contentType.startsWith('image/')) return '\uD83D\uDDBC';
  if (contentType.startsWith('audio/')) return '\uD83C\uDFB5';
  if (contentType.startsWith('video/')) return '\uD83C\uDFAC';
  if (contentType.includes('pdf')) return '\uD83D\uDCC4';
  if (contentType.includes('zip') || contentType.includes('compressed') || contentType.includes('archive')) return '\uD83D\uDDDC';
  if (contentType.includes('spreadsheet') || contentType.includes('excel')) return '\uD83D\uDCCA';
  if (contentType.includes('document') || contentType.includes('word')) return '\uD83D\uDCC3';
  if (contentType.includes('text/')) return '\uD83D\uDCC4';
  return '\uD83D\uDCC4';
}

function formatSize(bytes) {
  if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return bytes + ' B';
}

async function viewHeaders(mail) {
  try {
    showToast('Fetching headers...', 'info');
    const raw = await api.getMailHeaders(mail.account_id, mail.folder, mail.uid);

    const importantHeaders = [
      'From', 'To', 'Reply-To', 'Subject', 'Date',
      'Authentication-Results', 'Received-SPF', 'DKIM-Signature',
      'ARC-Authentication-Results', 'Received', 'Return-Path',
      'X-Mailer', 'X-Originating-IP',
    ];
    const pattern = new RegExp(
      '^(' + importantHeaders.join('|') + ')(:)',
      'gmi'
    );

    const highlighted = esc(raw).replace(
      pattern,
      '<span class="header-highlight">$1$2</span>'
    );

    const overlay = document.createElement('div');
    overlay.className = 'headers-modal-overlay';
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.setAttribute('aria-label', 'Email headers');
    overlay.innerHTML = `
      <div class="headers-modal">
        <div class="headers-modal-header">
          <span class="headers-modal-title">Email Headers</span>
          <button class="headers-modal-close" aria-label="Close headers">\u00D7</button>
        </div>
        <div class="headers-modal-body">
          <pre class="headers-modal-content">${highlighted}</pre>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    const close = () => overlay.remove();
    overlay.querySelector('.headers-modal-close').addEventListener('click', close);
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

    requestAnimationFrame(() => {
      overlay.querySelector('.headers-modal-close').focus();
    });
  } catch (err) {
    showToast(`Failed to fetch headers: ${err}`, 'error');
  }
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}

function renderSecurityBanners(authStatus, phishingWarnings, senderWarnings) {
  let out = '';

  if (authStatus) {
    if (authStatus.is_authenticated) {
      const details = [];
      if (authStatus.spf) details.push(`SPF: ${authStatus.spf}`);
      if (authStatus.dkim) details.push(`DKIM: ${authStatus.dkim}`);
      if (authStatus.dmarc) details.push(`DMARC: ${authStatus.dmarc}`);
      out += `
        <div class="mail-security-banner mail-security-ok" role="status" title="${esc(details.join(', '))}">
          <span class="mail-security-icon" aria-hidden="true">\u{1F6E1}</span>
          <span>Authenticated sender (${esc(details.join(', '))})</span>
        </div>
      `;
    } else {
      const details = [];
      if (authStatus.spf) details.push(`SPF: ${authStatus.spf}`);
      if (authStatus.dkim) details.push(`DKIM: ${authStatus.dkim}`);
      if (authStatus.dmarc) details.push(`DMARC: ${authStatus.dmarc}`);
      out += `
        <div class="mail-security-banner mail-security-warn" role="alert">
          <span class="mail-security-icon" aria-hidden="true">\u26A0</span>
          <span>Authentication failed (${esc(details.join(', '))})</span>
        </div>
      `;
    }
  }

  for (const warning of senderWarnings) {
    out += `
      <div class="mail-security-banner mail-security-danger" role="alert">
        <span class="mail-security-icon" aria-hidden="true">\u26A0</span>
        <span>${esc(warning)}</span>
      </div>
    `;
  }

  if (phishingWarnings.length > 0) {
    out += `
      <div class="mail-security-banner mail-security-danger" role="alert">
        <span class="mail-security-icon" aria-hidden="true">\u26D4</span>
        <span><strong>Suspicious links detected (${phishingWarnings.length})</strong></span>
      </div>
      <div class="mail-security-phishing-details">
    `;
    for (const w of phishingWarnings) {
      const severityClass = w.severity === 'high' ? 'mail-security-danger'
        : w.severity === 'medium' ? 'mail-security-warn' : 'mail-security-info';
      out += `
        <div class="mail-security-phishing-item ${severityClass}">
          <span class="mail-security-phishing-reason">[${esc(w.reason)}]</span>
          <span class="mail-security-phishing-text" title="Displayed: ${esc(w.displayed_text)}">${esc(w.displayed_text)}</span>
          <span class="mail-security-phishing-arrow" aria-hidden="true">\u2192</span>
          <span class="mail-security-phishing-url" title="${esc(w.actual_url)}">${esc(w.actual_url)}</span>
        </div>
      `;
    }
    out += '</div>';
  }

  return out;
}

// ---------------------------------------------------------------------------
// Link safety helpers
// ---------------------------------------------------------------------------

function extractLinks(html) {
  const links = [];
  const re = /<a\s[^>]*href\s*=\s*["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi;
  let match;
  while ((match = re.exec(html)) !== null) {
    const href = match[1];
    if (href.startsWith('mailto:') || href.startsWith('tel:') || href.startsWith('#') || href.startsWith('data:')) continue;
    const text = match[2].replace(/<[^>]+>/g, '').trim();
    links.push({ href, text });
  }
  const seen = new Set();
  return links.filter(l => {
    if (seen.has(l.href)) return false;
    seen.add(l.href);
    return true;
  });
}

function extractDomain(from) {
  const match = from.match(/@([^>]+)/);
  if (!match) return '';
  const domain = match[1].toLowerCase().trim();
  const parts = domain.split('.');
  return parts.length >= 2 ? parts.slice(-2).join('.') : domain;
}

function extractDomainFromUrl(url) {
  try {
    const u = new URL(url);
    const parts = u.hostname.toLowerCase().split('.');
    return parts.length >= 2 ? parts.slice(-2).join('.') : u.hostname;
  } catch {
    return '';
  }
}

// ---------------------------------------------------------------------------
// GitHub / Jira link detection (URL parsing only, no API calls)
// ---------------------------------------------------------------------------

function extractDevLinks(links) {
  const results = [];
  const seen = new Set();

  for (const link of links) {
    const href = link.href;

    // GitHub: issues and pull requests
    // Matches github.com/user/repo/issues/123 and github.com/user/repo/pull/456
    const ghMatch = href.match(/github\.com\/([^/]+\/[^/]+)\/(issues|pull)\/(\d+)/);
    if (ghMatch) {
      const key = `github:${ghMatch[1]}:${ghMatch[2]}:${ghMatch[3]}`;
      if (!seen.has(key)) {
        seen.add(key);
        results.push({
          platform: 'github',
          repo: ghMatch[1],
          type: ghMatch[2] === 'pull' ? 'pull' : 'issue',
          number: parseInt(ghMatch[3], 10),
          url: href,
        });
      }
      continue;
    }

    // Jira: PROJECT-123 style tickets in URLs
    // Matches jira.example.com/browse/PROJECT-123 or atlassian.net/browse/PROJECT-123
    const jiraMatch = href.match(/(?:jira\.|atlassian\.net).*\/browse\/([A-Z][A-Z0-9]+-\d+)/);
    if (jiraMatch) {
      const key = `jira:${jiraMatch[1]}`;
      if (!seen.has(key)) {
        seen.add(key);
        results.push({
          platform: 'jira',
          repo: jiraMatch[1],
          type: 'ticket',
          number: null,
          url: href,
        });
      }
    }
  }

  return results;
}
