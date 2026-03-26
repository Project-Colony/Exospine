// Exospine — Mail reading pane view

import { formatFullDate } from '../date_format.js';
import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { t } from '../i18n.js';

// ===== SECTION: Muted Threads =====
// -- Muted threads (persisted in localStorage, cached in memory) --
let _mutedThreadsCache = null; // cached Set of muted thread IDs

function getMutedThreads() {
  if (_mutedThreadsCache) return _mutedThreadsCache;
  try {
    const raw = localStorage.getItem('exospine_muted_threads');
    _mutedThreadsCache = new Set(raw ? JSON.parse(raw) : []);
  } catch {
    _mutedThreadsCache = new Set();
  }
  return _mutedThreadsCache;
}
function setMutedThreads(ids) {
  const arr = Array.isArray(ids) ? ids : [...ids];
  _mutedThreadsCache = new Set(arr);
  localStorage.setItem('exospine_muted_threads', JSON.stringify(arr));
}
export function isThreadMuted(threadId) {
  return getMutedThreads().has(threadId);
}

// -- Reminder timers (in-memory, cleared on page reload) --
const _reminderTimers = new Map();

// ── Zoom state (persisted across mail selections) ──────────────────
let _zoomLevel = parseFloat(localStorage.getItem('exospine_zoom') || '100');
let _zoomSaveTimer = null;
// ===== SECTION: Zoom & Preview Cache =====
function _saveZoomDebounced(val) {
  clearTimeout(_zoomSaveTimer);
  _zoomSaveTimer = setTimeout(() => localStorage.setItem('exospine_zoom', String(val)), 1000);
}
const _dismissedReceipts = new Set(); // Track dismissed read receipts per session
const _expandedQuoteIds = new Set(); // Track expanded quoted text blocks across re-renders

// ── Attachment preview cache ───────────────────────────────────────
const _previewCache = new Map(); // "accountId:folder:uid:partIndex" -> data
const _PREVIEW_CACHE_MAX = 20;

function _previewCacheSet(key, value) {
  // LRU eviction: if at capacity, delete the oldest (first) entry
  if (_previewCache.size >= _PREVIEW_CACHE_MAX && !_previewCache.has(key)) {
    const oldest = _previewCache.keys().next().value;
    _previewCache.delete(oldest);
  }
  // Delete and re-insert to move to end (most recently used)
  _previewCache.delete(key);
  _previewCache.set(key, value);
}

function _previewCacheGet(key) {
  if (!_previewCache.has(key)) return undefined;
  const value = _previewCache.get(key);
  // Move to end (most recently used)
  _previewCache.delete(key);
  _previewCache.set(key, value);
  return value;
}

// ===== SECTION: Main Render =====
/**
 * Render the reading pane.
 * @param {HTMLElement} el  #mail-view element
 * @param {object} state    app state (selectedMail, mailBody)
 * @param {object} actions  { onReply, onReplyAll, onForward, onArchive, onDelete, onMarkUnread }
 */
export function renderMailView(el, state, actions) {
  const mail = state.selectedMail;

  if (!mail) {
    el.innerHTML = `<div class="mail-view-empty" role="status" aria-live="polite">
      <div class="empty-state-illustration">
        <svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg">
          <rect x="10" y="20" width="60" height="44" rx="6" stroke="currentColor" stroke-width="2" fill="none"/>
          <path d="M10 26 L40 46 L70 26" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </div>
      <div class="empty-state-text">Select an email to read</div>
    </div>`;
    return;
  }

  const body = state.mailBody;
  if (body === undefined) {
    el.innerHTML = '<div class="mail-view-body-text mail-view-status-text" role="status" aria-live="polite">Loading email body\u2026</div>';
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

  // Generate 1-line summary from plain text body
  const _summaryText = (() => {
    const plainText = (body && body.text) || '';
    if (!plainText) return '';
    // Strip greeting lines like "Hi Name," / "Hello," / "Dear ..."
    let cleaned = plainText.replace(/^(Hi|Hello|Hey|Dear|Bonjour|Salut)\s*[^,\n]*[,!]?\s*\n?/i, '');
    // Strip signature blocks (lines starting with --, lines like "Regards," etc.)
    cleaned = cleaned.replace(/(\n--\s*\n[\s\S]*$)/m, '');
    cleaned = cleaned.replace(/\n(Regards|Best|Cheers|Thanks|Cordialement|Merci|Sincerely|Kind regards|Best regards)[,.]?\s*\n[\s\S]*$/im, '');
    cleaned = cleaned.trim();
    if (!cleaned) return '';
    // Take first sentence (up to 150 chars)
    const firstSentence = cleaned.match(/^[^\n]*?[.!?](?:\s|$)/);
    let summary = firstSentence ? firstSentence[0].trim() : cleaned.split('\n')[0].trim();
    if (summary.length > 150) summary = summary.slice(0, 147) + '...';
    return summary;
  })();

  // Header
  html += `
    <div class="mail-view-header">
      <div class="mail-view-subject">${esc(mail.subject || '(No subject)')}</div>
      ${_summaryText ? `<div class="mail-view-summary">${esc(_summaryText)}</div>` : ''}
      <div class="mail-view-meta">
        <div class="mail-view-meta-row">
          <span class="mail-view-meta-label" id="mv-from-label">From</span>
          <span class="mail-view-meta-value" aria-labelledby="mv-from-label"><span class="mail-view-from-hover" data-from-email="${esc(mail.from || '')}">${esc(mail.from || 'Unknown')}</span> <button class="mail-view-copy-email" data-copy-email="${esc(mail.from || '')}" title="Copy email address" aria-label="Copy email address">\u{1F4CB}</button></span>
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
      <div class="mail-view-reading-time" aria-label="Reading time">${(() => { const txt = (body && body.text) || (body && body.html && body.html.replace(/<[^>]+>/g, "")); if (!txt) return ""; const words = txt.trim().split(/\s+/).length; const mins = Math.max(1, Math.round(words / 200)); return "~" + mins + " min read"; })()}</div>
    </div>
  `;

  // Security indicators
  html += renderSecurityBanners(authStatus, phishingWarnings, senderWarnings);

  // Unsubscribe banner
  const unsubscribeUrl = (body && body.unsubscribe_url) || mail.unsubscribe_url || null;
  if (unsubscribeUrl) {
    const isMailto = unsubscribeUrl.startsWith('mailto:');
    html += `
      <div class="mail-view-unsubscribe-banner" role="status">
        <span>\u2709 This is a mailing list.</span>
        <a href="#" id="unsubscribe-link" class="unsubscribe-link" data-unsub-url="${esc(unsubscribeUrl)}" data-unsub-mailto="${isMailto}">Unsubscribe</a>
      </div>
    `;
  }

  // Read receipt banner
  if (mail.read_receipt_requested && mail.read_receipt_to && !_dismissedReceipts.has(mail.id)) {
    html += `
      <div class="mail-view-receipt-banner" id="receipt-banner" role="alert">
        <span>\u2709 The sender requested a read receipt.</span>
        <div class="receipt-actions">
          <button id="send-receipt-btn" class="btn btn-sm btn-primary" aria-label="Send read receipt">Send receipt</button>
          <button id="ignore-receipt-btn" class="btn btn-sm btn-ghost" aria-label="Ignore read receipt request">Ignore</button>
        </div>
      </div>
    `;
  }

  // Toolbar
  html += `
    <div class="mail-view-toolbar" role="toolbar" aria-label="Email actions">
      <button class="mail-view-toolbar-btn" data-action="reply" title="${t('reply')}" aria-label="${t('reply')}">\u21A9 ${t('reply')}</button>
      <button class="mail-view-toolbar-btn" data-action="reply-all" title="${t('reply_all')}" aria-label="${t('reply_all')}">\u21A9\u21A9 ${t('reply_all')}</button>
      <button class="mail-view-toolbar-btn" data-action="forward" title="${t('forward')}" aria-label="${t('forward')}">\u21AA ${t('forward')}</button>
      <div class="mail-view-toolbar-spacer"></div>
      <button class="mail-view-toolbar-btn" data-action="print" title="${t('print')}" aria-label="${t('print')}">\uD83D\uDDA8 ${t('print')}</button>
      <button class="mail-view-toolbar-btn" data-action="export" title="${t('export')}" aria-label="${t('export')}">\u2B07 ${t('export')}</button>
      <button class="mail-view-toolbar-btn" data-action="view-headers" title="${t('headers')}" aria-label="${t('headers')}">\uD83D\uDD0D ${t('headers')}</button>
      <button class="mail-view-toolbar-btn" data-action="mute-thread" title="Mute thread" aria-label="Mute thread">${(() => { const tid = mail.thread_id || mail.id; const muted = getMutedThreads().has(tid); return muted ? '\u{1F507} Unmute' : '\u{1F515} Mute'; })()}</button>
      <div class="toolbar-dropdown-wrapper">
        <button class="mail-view-toolbar-btn" data-action="remind" title="Remind me" aria-label="Remind me" aria-expanded="false" aria-controls="remind-dropdown">\u{23F0} Remind</button>
        <div class="remind-dropdown toolbar-dropdown-panel" id="remind-dropdown" hidden>
          <div class="remind-option dropdown-option" data-remind="30" role="button" tabindex="0">In 30 minutes</div>
          <div class="remind-option dropdown-option" data-remind="60" role="button" tabindex="0">In 1 hour</div>
          <div class="remind-option dropdown-option" data-remind="180" role="button" tabindex="0">In 3 hours</div>
          <div class="remind-option dropdown-option" data-remind="tomorrow" role="button" tabindex="0">Tomorrow</div>
        </div>
      </div>
      <button class="mail-view-toolbar-btn" data-action="track-reply" title="Track reply" aria-label="Track reply">\u{1F552} Track reply</button>
      <button class="mail-view-toolbar-btn" data-action="create-task" title="Create task" aria-label="Create task">\u2611 Create task</button>
      <button class="mail-view-toolbar-btn" data-action="mark-unread" title="${t('mark_unread')}" aria-label="${t('mark_unread')}">\u2709 ${t('mark_unread')}</button>
      <div class="toolbar-dropdown-wrapper">
        <button class="mail-view-toolbar-btn" data-action="snooze" title="Snooze email" aria-label="Snooze email" aria-expanded="false" aria-controls="snooze-dropdown">\u23F0 Snooze</button>
        <div class="snooze-dropdown toolbar-dropdown-panel" id="snooze-dropdown" hidden>
          <div class="snooze-option dropdown-option" data-snooze="later-today" role="button" tabindex="0" aria-label="Snooze for 4 hours">Later today (4 hours)</div>
          <div class="snooze-option dropdown-option" data-snooze="tomorrow" role="button" tabindex="0" aria-label="Snooze until tomorrow morning">Tomorrow morning</div>
          <div class="snooze-option dropdown-option" data-snooze="next-week" role="button" tabindex="0" aria-label="Snooze until next week">Next week</div>
          <div class="separator-top mt-4 mb-4"></div>
          <div class="snooze-custom-section">
            <label for="snooze-custom-time">Pick date & time:</label>
            <input type="datetime-local" id="snooze-custom-time" aria-label="Custom snooze date and time" />
            <button class="snooze-option snooze-custom-btn" data-snooze="custom" aria-label="Set custom snooze time">Set</button>
          </div>
        </div>
      </div>
      <button class="mail-view-toolbar-btn" data-action="archive" title="${t('archive')}" aria-label="${t('archive')}">\uD83D\uDCE6 ${t('archive')}</button>
      <button class="mail-view-toolbar-btn danger" data-action="delete" title="${t('delete')}" aria-label="${t('delete')}">\uD83D\uDDD1 ${t('delete')}</button>
      <button class="mail-view-toolbar-btn" data-action="open-new-window" title="Open in new window" aria-label="Open in new window">\u{1F5D7} New window</button>
      <span class="toolbar-sep" aria-hidden="true"></span>
      <div class="zoom-controls">
        <button class="mail-view-toolbar-btn" data-action="zoom-out" aria-label="Zoom out" title="Zoom out">\u2212</button>
        <span id="zoom-level-display" aria-live="polite">${Math.round(_zoomLevel)}%</span>
        <button class="mail-view-toolbar-btn" data-action="zoom-in" aria-label="Zoom in" title="Zoom in">+</button>
        <button class="mail-view-toolbar-btn" data-action="zoom-reset" aria-label="Reset zoom" title="Reset zoom">100%</button>
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

  // Conversation timeline (if threaded)
  if (mail.thread_id && state.mails) {
    const threadMails = state.mails.filter(m =>
      m.thread_id === mail.thread_id || (mail.thread_id === mail.id && m.id === mail.id)
    );
    if (threadMails.length > 1) {
      // Sort by date ascending
      const sorted = [...threadMails].sort((a, b) => new Date(a.date) - new Date(b.date));
      html += '<div class="conversation-timeline" role="region" aria-label="Conversation timeline">';
      html += '<div class="conversation-timeline-title">Conversation</div>';
      for (const tm of sorted) {
        const isCurrent = tm.id === mail.id;
        const fromName = (tm.from_name || tm.from || 'Unknown').split('<')[0].trim();
        const dateStr = formatFullDate(tm.date);
        const preview = (tm.preview || '').slice(0, 60);
        html += `
          <div class="conversation-timeline-item${isCurrent ? ' current' : ''}" data-timeline-id="${esc(tm.id)}" role="button" tabindex="0" title="${esc(fromName)} - ${esc(dateStr)}">
            <div class="conversation-timeline-dot"></div>
            <div class="conversation-timeline-content">
              <div class="conversation-timeline-from">${esc(fromName)}</div>
              <div class="conversation-timeline-date">${esc(dateStr)}</div>
              <div class="conversation-timeline-preview">${esc(preview)}</div>
            </div>
          </div>
        `;
      }
      html += '</div>';
    }
  }

  // Body — with quoted text collapsing for plain text
  html += '<div class="mail-view-body" aria-live="polite">';
  if (hasHtml) {
    html += '<iframe id="email-frame" sandbox="allow-same-origin" title="Email content"></iframe>';
  } else if (hasText) {
    html += `<div class="mail-view-body-text">${collapseQuotedText(esc(body.text))}</div>`;
  } else if (body === null) {
    html += '<div class="mail-view-body-text mail-view-status-text" role="status">Loading email body\u2026</div>';
  } else {
    html += '<div class="mail-view-body-text mail-view-status-text" role="status">No content available</div>';
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
        html += '<div class="mail-view-dev-links mt-8">';
        for (const dl of devLinks) {
          const typeIcon = dl.type === 'pull' ? '\uD83D\uDD00' : (dl.type === 'issue' ? '\uD83D\uDCDD' : '\uD83D\uDCCB');
          const typeLabel = dl.type === 'pull' ? 'Pull Request' : (dl.type === 'issue' ? 'Issue' : 'Ticket');
          const platformLabel = dl.platform === 'github' ? 'GitHub' : 'Jira';
          html += `
            <a href="${esc(dl.url)}" target="_blank" rel="noopener noreferrer" class="dev-link-card">
              <span class="dev-link-card-icon" aria-hidden="true">${typeIcon}</span>
              <div class="dev-link-card-body">
                <div class="dev-link-card-title">${esc(dl.repo)}${dl.number ? ' #' + esc(String(dl.number)) : ''}</div>
                <div class="dev-link-card-meta">${platformLabel} ${typeLabel}</div>
              </div>
              <span class="dev-link-card-arrow">Open &rarr;</span>
            </a>
          `;
        }
        html += '</div>';
      }
    }
  }


  // Quick reply templates
  html += '<div class="quick-reply-templates" id="quick-reply-templates">';
  html += '<span class="quick-reply-templates-label">Quick reply:</span>';
  const templates = ["Thanks!", "Got it!", "I\'ll check", "Will do!"];
  for (const tpl of templates) {
    html += `<button class="quick-reply-pill" data-reply-text="${esc(tpl)}">${esc(tpl)}</button>`;
  }
  html += '</div>';

  // Internal Comments section (threaded comments on this thread)
  const _threadKey = mail.thread_id || mail.id;
  html += `
    <details class="mail-view-notes-section mail-view-comments-section" id="comments-section">
      <summary class="notes-section-toggle">\uD83D\uDCDD Comments <span id="comments-indicator" class="notes-indicator" hidden>\u2022</span></summary>
      <div class="notes-section-body">
        <div class="comments-timeline" id="comments-timeline" aria-label="Internal comments"></div>
        <div class="comments-input-row">
          <textarea id="comment-input" class="mail-note-textarea comment-input" placeholder="Add a comment..." rows="2" aria-label="New comment"></textarea>
          <button class="comment-add-btn" id="comment-add-btn" title="Add comment" aria-label="Add comment">Add</button>
        </div>
      </div>
    </details>
  `;

  // Sender profile card (hidden, shown on hover)
  html += '<div class="sender-profile-card" id="sender-profile-card" hidden></div>';

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
    const qid = toggle.dataset.quoteId;
    const quotedBlock = toggle.nextElementSibling;
    if (quotedBlock) {
      const isHidden = quotedBlock.hidden;
      quotedBlock.hidden = !isHidden;
      toggle.setAttribute('aria-expanded', String(!isHidden));
      toggle.textContent = isHidden ? '\u22EF Hide quoted text' : '\u22EF Show quoted text';
      // Persist expanded state for re-renders
      if (qid) {
        if (!isHidden) {
          _expandedQuoteIds.delete(qid);
        } else {
          _expandedQuoteIds.add(qid);
        }
      }
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
          // restore original src on all blocked images inside the iframe.
          // Add loading="lazy" to prevent all images from loading simultaneously.
          try {
            const iframeDoc = iframe.contentDocument || iframe.contentWindow?.document;
            if (iframeDoc) {
              const imgs = iframeDoc.querySelectorAll('img[data-original-src]');
              imgs.forEach(img => {
                img.setAttribute('loading', 'lazy');
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

  // Unsubscribe link handler
  const unsubLink = el.querySelector('#unsubscribe-link');
  if (unsubLink) {
    unsubLink.addEventListener('click', async (e) => {
      e.preventDefault();
      const url = unsubLink.dataset.unsubUrl;
      const isMailto = unsubLink.dataset.unsubMailto === 'true';
      if (isMailto) {
        // For mailto, attempt to open in default mail client or send empty email
        try {
          // Use Tauri shell plugin to open mailto link
          if (window.__TAURI__) {
            const { open } = await import('@tauri-apps/plugin-shell');
            await open(url);
          } else {
            window.open(url);
          }
          showToast('Unsubscribe email client opened.', 'info');
        } catch {
          window.open(url);
        }
      } else {
        // For https, open in browser
        try {
          if (window.__TAURI__) {
            const { open } = await import('@tauri-apps/plugin-shell');
            await open(url);
          } else {
            window.open(url, '_blank');
          }
          showToast('Unsubscribe page opened in browser.', 'info');
        } catch {
          window.open(url, '_blank');
        }
      }
    });
  }

  // Internal Comments: load and render threaded comments (stored in localStorage)
  const commentsTimeline = el.querySelector('#comments-timeline');
  const commentInput = el.querySelector('#comment-input');
  const commentAddBtn = el.querySelector('#comment-add-btn');
  const commentsIndicator = el.querySelector('#comments-indicator');
  const threadKey = mail.thread_id || mail.id;

  function loadComments(tid) {
    try {
      const raw = localStorage.getItem('exospine_comments_' + tid);
      return raw ? JSON.parse(raw) : [];
    } catch { return []; }
  }
  function saveComments(tid, comments) {
    localStorage.setItem('exospine_comments_' + tid, JSON.stringify(comments));
  }
  function renderComments() {
    const comments = loadComments(threadKey);
    if (commentsIndicator) commentsIndicator.hidden = comments.length === 0;
    if (!commentsTimeline) return;
    if (comments.length === 0) {
      commentsTimeline.innerHTML = '<div class="comments-empty">No comments yet.</div>';
      return;
    }
    let html = '';
    for (const c of comments) {
      const d = new Date(c.timestamp);
      const timeStr = d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
      html += `
        <div class="comment-bubble" data-comment-ts="${c.timestamp}">
          <div class="comment-meta">
            <span class="comment-author">${esc(c.author || 'You')}</span>
            <span class="comment-time">${esc(timeStr)}</span>
            <button class="comment-delete-btn" data-delete-ts="${c.timestamp}" title="Delete comment" aria-label="Delete comment">\u00D7</button>
          </div>
          <div class="comment-text">${esc(c.text)}</div>
        </div>
      `;
    }
    commentsTimeline.innerHTML = html;
    // Scroll to bottom of timeline
    commentsTimeline.scrollTop = commentsTimeline.scrollHeight;
    // Attach delete handlers
    commentsTimeline.querySelectorAll('.comment-delete-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const ts = btn.dataset.deleteTs;
        const all = loadComments(threadKey);
        const filtered = all.filter(c => c.timestamp !== ts);
        saveComments(threadKey, filtered);
        renderComments();
      });
    });
  }
  renderComments();

  if (commentAddBtn && commentInput) {
    const addComment = () => {
      const text = commentInput.value.trim();
      if (!text) return;
      const comments = loadComments(threadKey);
      comments.push({
        text,
        timestamp: new Date().toISOString(),
        author: 'You',
      });
      saveComments(threadKey, comments);
      commentInput.value = '';
      renderComments();
    };
    commentAddBtn.addEventListener('click', addComment);
    commentInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        addComment();
      }
    });
  }

  // Also migrate old notes to a comment if present (one-time migration)
  if (mail.id) {
    api.getNote(mail.id).then((note) => {
      if (note && note.trim()) {
        const existing = loadComments(threadKey);
        const alreadyMigrated = existing.some(c => c.text === note.trim() && c.author === 'Migrated note');
        if (!alreadyMigrated) {
          existing.unshift({
            text: note.trim(),
            timestamp: new Date().toISOString(),
            author: 'Migrated note',
          });
          saveComments(threadKey, existing);
          renderComments();
        }
      }
    }).catch(() => {});
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
    'track-reply': async () => {
      // Extract sender email from the "from" field
      const fromEmail = (mail.from || '').match(/<([^>]+)>/)?.[1] || (mail.from || '').trim();
      if (!fromEmail) { showToast('No sender to track.', 'error'); return; }
      // Default due date: 3 days from now
      const due = new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString();
      try {
        await api.addFollowup(mail.id, fromEmail, due);
        showToast('Tracking reply from ' + fromEmail, 'success');
      } catch (err) {
        showToast('Failed to track: ' + err, 'error');
      }
    },
    'create-task': async () => {
      const title = mail.subject || '(No subject)';
      const description = (mail.preview || '').slice(0, 200);
      const dueDateInput = prompt('Due date (YYYY-MM-DD) or leave empty:');
      const dueDate = dueDateInput || '';
      try {
        await api.createTask(title, description, mail.id, dueDate);
        showToast('Task created: ' + title, 'success');
      } catch (err) {
        showToast('Failed to create task: ' + err, 'error');
      }
    },
    'open-new-window': () => {
      const subject = mail.subject || '(No subject)';
      const htmlContent = (body && body.html) || '';
      const textContent = (body && body.text) || '';
      const content = htmlContent || `<pre style="white-space:pre-wrap;font-family:sans-serif;">${esc(textContent)}</pre>`;
      const fullHtml = `<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>${esc(subject)}</title>
<style>body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;margin:20px;color:#1e1e1e;background:#fff;}
.mail-header{border-bottom:1px solid #ddd;padding-bottom:12px;margin-bottom:16px;}
.mail-subject{font-size:20px;font-weight:bold;margin-bottom:8px;}
.mail-meta{font-size:13px;color:#666;}</style></head><body>
<div class="mail-header">
  <div class="mail-subject">${esc(subject)}</div>
  <div class="mail-meta">From: ${esc(mail.from || 'Unknown')}</div>
  <div class="mail-meta">To: ${esc(mail.to || '')}</div>
  <div class="mail-meta">Date: ${esc(mail.date || '')}</div>
</div>
<div class="mail-body">${content}</div>
</body></html>`;
      const newWin = window.open('', '_blank', 'width=800,height=600,menubar=no,toolbar=no');
      if (newWin) {
        newWin.document.write(fullHtml);
        newWin.document.close();
        newWin.document.title = subject;
      } else {
        showToast('Pop-up blocked. Please allow pop-ups for Exospine.', 'error');
      }
    },
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

  // ── Conversation timeline click ────────────────────────────────
  el.querySelectorAll('.conversation-timeline-item').forEach((item) => {
    item.addEventListener('click', () => {
      const id = item.dataset.timelineId;
      if (id && actions.onSelectThread) {
        actions.onSelectThread(id);
      }
    });
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

  // -- Feature 1: Mute thread toggle --
  const muteBtn = el.querySelector('[data-action="mute-thread"]');
  if (muteBtn) {
    muteBtn.addEventListener('click', () => {
      const tid = mail.thread_id || mail.id;
      const muted = getMutedThreads();
      if (muted.has(tid)) {
        muted.delete(tid);
        showToast('Thread unmuted.', 'info');
      } else {
        muted.add(tid);
        showToast('Thread muted. No notifications for this thread.', 'info');
      }
      setMutedThreads(muted);
      muteBtn.textContent = muted.has(tid) ? '\u{1F507} Unmute' : '\u{1F515} Mute';
    });
  }

  // -- Feature 4: Remind dropdown toggle + handlers --
  const remindBtn = el.querySelector('[data-action="remind"]');
  if (remindBtn) {
    remindBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      const dropdown = el.querySelector('#remind-dropdown');
      if (dropdown) {
        dropdown.hidden = !dropdown.hidden;
        remindBtn.setAttribute('aria-expanded', String(!dropdown.hidden));
      }
    });
  }
  el.querySelectorAll('.remind-option').forEach((opt) => {
    opt.addEventListener('click', () => {
      const val = opt.dataset.remind;
      let delayMs;
      if (val === 'tomorrow') {
        const tomorrow = new Date();
        tomorrow.setDate(tomorrow.getDate() + 1);
        tomorrow.setHours(9, 0, 0, 0);
        delayMs = Math.max(0, tomorrow.getTime() - Date.now());
      } else {
        delayMs = parseInt(val, 10) * 60 * 1000;
      }
      const mailSubject = mail.subject || '(No subject)';
      const mailId = mail.id;
      // Clear previous reminder for same mail
      if (_reminderTimers.has(mailId)) clearTimeout(_reminderTimers.get(mailId));
      const timer = setTimeout(() => {
        _reminderTimers.delete(mailId);
        try {
          if (Notification.permission === 'granted') {
            new Notification('Email Reminder', { body: mailSubject });
          } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(p => {
              if (p === 'granted') new Notification('Email Reminder', { body: mailSubject });
            });
          }
        } catch {}
        showToast('Reminder: ' + mailSubject, 'info');
      }, delayMs);
      _reminderTimers.set(mailId, timer);
      showToast('Reminder set!', 'success');
      const dropdown = el.querySelector('#remind-dropdown');
      if (dropdown) dropdown.hidden = true;
    });
  });
  // Close remind dropdown on outside click
  document.addEventListener('click', () => {
    const dropdown = el.querySelector('#remind-dropdown');
    if (dropdown) {
      dropdown.hidden = true;
      const btn = el.querySelector('[data-action="remind"]');
      if (btn) btn.setAttribute('aria-expanded', 'false');
    }
  });

  // -- Feature 3: Quick reply template pills --
  el.querySelectorAll('.quick-reply-pill').forEach((pill) => {
    pill.addEventListener('click', async () => {
      const text = pill.dataset.replyText;
      if (!text || !mail) return;
      pill.disabled = true;
      try {
        const draft = {
          account_id: mail.account_id || mail._accountId,
          to: mail.from || '',
          cc: '',
          bcc: '',
          subject: (mail.subject || '').startsWith('Re:') ? mail.subject : 'Re: ' + (mail.subject || ''),
          body: text,
          in_reply_to: mail.id,
        };
        await api.sendMail(draft);
        showToast('Reply sent: ' + text, 'success');
      } catch (err) {
        showToast('Failed to send reply: ' + err, 'error');
      }
      pill.disabled = false;
    });
  });

  // -- Feature 6 + 8: Sender profile card on hover --
  const fromHover = el.querySelector('.mail-view-from-hover');
  const profileCard = el.querySelector('#sender-profile-card');
  if (fromHover && profileCard) {
    let _profileTimeout = null;
    fromHover.addEventListener('mouseenter', async () => {
      clearTimeout(_profileTimeout);
      const fromRaw = fromHover.dataset.fromEmail || '';
      const emailMatch = fromRaw.match(/<([^>]+)>/);
      const email = emailMatch ? emailMatch[1] : fromRaw.trim();
      const nameMatch = fromRaw.match(/^([^<]+)</);
      const name = nameMatch ? nameMatch[1].trim() : email;
      const initial = (name || '?')[0].toUpperCase();
      const colors = ['#e74c3c','#3498db','#2ecc71','#f39c12','#9b59b6','#1abc9c','#e67e22','#34495e'];
      let hash = 0;
      for (const c of email) hash = (hash * 31 + c.charCodeAt(0)) & 0xffffffff;
      const color = colors[Math.abs(hash) % colors.length];

      // Feature 8: Count emails from this sender in current state
      let emailCount = 0;
      if (state.mails) {
        for (const m of state.mails) {
          const mFrom = (m.from || '').toLowerCase();
          if (mFrom.includes(email.toLowerCase())) emailCount++;
        }
      }

      profileCard.innerHTML = `
        <div class="sender-card-avatar" style="background:${color}">${esc(initial)}</div>
        <div class="sender-card-info">
          <div class="sender-card-name">${esc(name)}</div>
          <div class="sender-card-email">${esc(email)}</div>
          ${emailCount > 0 ? `<div class="sender-card-stats">${emailCount} email${emailCount !== 1 ? 's' : ''} in view</div>` : ''}
        </div>
        <button class="sender-card-copy" data-copy-sender="${esc(email)}" title="Copy email">\u{1F4CB}</button>
      `;
      profileCard.hidden = false;

      // Position card near the from field
      const rect = fromHover.getBoundingClientRect();
      const elRect = el.getBoundingClientRect();
      profileCard.style.top = (rect.bottom - elRect.top + 4) + 'px';
      profileCard.style.left = (rect.left - elRect.left) + 'px';

      // Copy button handler
      const copyBtn = profileCard.querySelector('.sender-card-copy');
      if (copyBtn) {
        copyBtn.onclick = () => {
          navigator.clipboard.writeText(copyBtn.dataset.copySender).then(() => {
            showToast('Email copied.', 'info');
          }).catch(() => {});
        };
      }
    });
    fromHover.addEventListener('mouseleave', () => {
      _profileTimeout = setTimeout(() => { profileCard.hidden = true; }, 300);
    });
    profileCard.addEventListener('mouseenter', () => { clearTimeout(_profileTimeout); });
    profileCard.addEventListener('mouseleave', () => { profileCard.hidden = true; });
  }

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

// ===== SECTION: Attachment Preview =====
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
  content.innerHTML = '<div class="preview-loading">Loading preview...</div>';

  const cacheKey = `${mail.account_id}:${mail.folder}:${mail.uid}:${partIndex}`;

  try {
    let data = _previewCacheGet(cacheKey);
    if (!data) {
      data = await api.getAttachmentContent(mail.account_id, mail.folder, mail.uid, partIndex);
      _previewCacheSet(cacheKey, data);
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
    content.innerHTML = `<div class="preview-error">Failed to load preview: ${esc(String(err))}</div>`;
  }
}

// ── Feature: Image lightbox ────────────────────────────────────────
// ===== SECTION: Image Lightbox =====
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
let _quoteIdCounter = 0;
// ===== SECTION: Quoted Text & Conversation Cleanup =====
function collapseQuotedText(text) {
  _quoteIdCounter = 0;

  function makeToggle() {
    const qid = 'quote-' + (_quoteIdCounter++);
    const isExpanded = _expandedQuoteIds.has(qid);
    const label = isExpanded ? '\u22EF Hide quoted text' : '\u22EF Show quoted text';
    return {
      toggle: `<div class="quoted-text-toggle" data-quote-id="${qid}" role="button" tabindex="0" aria-expanded="${isExpanded}" aria-label="${isExpanded ? 'Hide' : 'Show'} quoted text">${label}</div>`,
      blockAttr: isExpanded ? '' : ' hidden',
      qid,
    };
  }

  // Pattern 1: "On ... wrote:" followed by the rest
  if (/(On .+wrote:[\s\S]+$)/m.test(text)) {
    const qt = makeToggle();
    text = text.replace(
      /(On .+wrote:[\s\S]+$)/m,
      qt.toggle + '<div class="quoted-text" data-quote-id="' + qt.qid + '"' + qt.blockAttr + '>$1</div>'
    );
  }

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
          const qt = makeToggle();
          result.push(qt.toggle);
          result.push('<div class="quoted-text" data-quote-id="' + qt.qid + '"' + qt.blockAttr + '>' + quotedLines.join('\n') + '</div>');
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
        const qt = makeToggle();
        result.push(qt.toggle);
        result.push('<div class="quoted-text" data-quote-id="' + qt.qid + '"' + qt.blockAttr + '>' + quotedLines.join('\n') + '</div>');
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
// ===== SECTION: Print =====
function printEmail(mail, body) {
  if (!body) return;
  const content = body.html || `<pre style="white-space:pre-wrap;font-family:sans-serif;">${esc(body.text || '')}</pre>`;
  const w = window.open('', '_blank');
  if (!w) {
    showToast('Pop-up blocked. Please allow pop-ups.', 'error');
    return;
  }
  w.document.write(`<!DOCTYPE html><html><head><meta charset="utf-8"><title>${esc(mail.subject || '')}</title><style>body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;margin:20px;font-size:14px;color:#1e1e1e;}img{max-width:100%;height:auto;}</style></head><body>${content}</body></html>`);
  w.document.close();
  w.onafterprint = () => w.close();
  w.print();
  // Fallback: close after 30s in case onafterprint doesn't fire
  setTimeout(() => { try { w.close(); } catch {} }, 30000);
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
// ===== SECTION: Iframe & HTML Rendering =====
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

// ===== SECTION: Attachment Helpers =====
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

// ===== SECTION: Utility Helpers =====
function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}

// ===== SECTION: Security Banners =====
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

// ===== SECTION: Link Safety & Dev Links =====
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
