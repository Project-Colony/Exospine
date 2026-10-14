// Exospine: the reading pane's HTML, and the sandboxed document a mail body is shown in.

import { formatFullDate } from '../../date_format.js';
import * as api from '../../api.js';
import { showToast } from '../../components/toast.js';
import { t } from '../../i18n.js';
import { esc } from '../../html.js';
import { attachmentIcon, formatSize, getPreviewType } from './attachments.js';
import { dismissedReceipts, expandedQuoteIds, getMutedThreads, getZoom } from './state.js';

/** The reading pane's HTML for `mail` and its loaded `body`. */
export function mailViewHtml(mail, body, state) {
  const hasHtml = body && typeof body.html === 'string' && body.html.length > 0;
  const hasText = body && typeof body.text === 'string' && body.text.length > 0;
  // The backend keeps blocked remote image URLs in data-remote-src.
  const hasRemoteImages = hasHtml && body.html.includes('data-remote-src=');

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
    // Strip signature blocks only if they appear in the last 30% of the text
    const threshold = Math.floor(cleaned.length * 0.7);
    const sigDashMatch = cleaned.search(/\n--\s*\n/m);
    if (sigDashMatch >= 0 && sigDashMatch >= threshold) {
      cleaned = cleaned.slice(0, sigDashMatch);
    }
    const sigClosingMatch = cleaned.search(/\n(Regards|Best|Cheers|Thanks|Cordialement|Merci|Sincerely|Kind regards|Best regards)[,.]?\s*\n/im);
    if (sigClosingMatch >= 0 && sigClosingMatch >= threshold) {
      cleaned = cleaned.slice(0, sigClosingMatch);
    }
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
  if (mail.read_receipt_requested && mail.read_receipt_to && !dismissedReceipts.has(mail.id)) {
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
        <span id="zoom-level-display" aria-live="polite">${Math.round(getZoom())}%</span>
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

  return html;
}

// ── Feature: Collapse quoted text in plain text emails ─────────────
let _quoteIdCounter = 0;
function collapseQuotedText(text) {
  _quoteIdCounter = 0;

  function makeToggle() {
    const qid = 'quote-' + (_quoteIdCounter++);
    const isExpanded = expandedQuoteIds.has(qid);
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
 * Full document for a sandboxed mail iframe. `html` must be the sanitized
 * body from the backend. Its own CSP blocks every remote resource unless the
 * user chose to show remote images.
 */
export function mailDocument(html, { allowRemoteImages = false, title = '' } = {}) {
  const imgSrc = allowRemoteImages ? 'data: https:' : 'data:';
  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>${esc(title)}</title>
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src ${imgSrc}">
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
    img[data-remote-src] { background: #eee; min-width: 16px; min-height: 16px; }
    a { color: rgb(0, 120, 214); }
    pre, code { font-family: "Cascadia Code", "Fira Code", Consolas, monospace; }
    table { border-collapse: collapse; max-width: 100%; }
  </style>
</head>
<body>${addLazyLoading(html)}</body>
</html>`;
}

function addLazyLoading(html) {
  return html.replace(/<img(?![^>]*loading\s*=)/gi, '<img loading="lazy"');
}

/** Put blocked remote images back: the backend keeps their URL in data-remote-src. */
export function showRemoteImages(html) {
  // DOMParser documents are inert: nothing loads or runs while we rewrite.
  const doc = new DOMParser().parseFromString(html, 'text/html');
  doc.querySelectorAll('img[data-remote-src]').forEach((img) => {
    img.setAttribute('src', img.dataset.remoteSrc);
    img.removeAttribute('data-remote-src');
  });
  return doc.body.innerHTML;
}

/** Open links clicked in a mail iframe in the system browser, not inside the frame. */
export function openLinksExternally(iframe) {
  iframe.addEventListener('load', () => {
    iframe.contentDocument?.addEventListener('click', (e) => {
      const link = e.target.closest?.('a[href]');
      if (!link) return;
      e.preventDefault();
      api.openExternal(link.href).catch((err) => showToast(`Failed to open link: ${err}`, 'error'));
    });
  });
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
