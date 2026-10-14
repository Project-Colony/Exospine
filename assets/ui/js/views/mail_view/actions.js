// Exospine: what the reading pane's buttons, menus and links do.

import { formatFullDate } from '../../date_format.js';
import * as api from '../../api.js';
import { showToast } from '../../components/toast.js';
import { esc } from '../../html.js';
import { handleDownloadAttachment, openAttachmentPreview } from './attachments.js';
import { mailDocument, openLinksExternally, showRemoteImages } from './render.js';
import {
  dismissedReceipts, expandedQuoteIds, getMutedThreads, getZoom, reminderTimers,
  saveZoomDebounced, setMutedThreads, setZoom,
} from './state.js';

/** Wire the controls of the reading pane once `mailViewHtml` is in `el`. */
export function wireMailView(el, mail, body, state, actions) {
  const hasHtml = body && typeof body.html === 'string' && body.html.length > 0;

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
          expandedQuoteIds.delete(qid);
        } else {
          expandedQuoteIds.add(qid);
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

  // Inject the sanitized HTML into the sandboxed iframe (remote images blocked by default)
  let remoteImagesShown = false;
  if (hasHtml) {
    const iframe = el.querySelector('#email-frame');
    if (iframe) {
      openLinksExternally(iframe);
      iframe.srcdoc = mailDocument(body.html);

      const showBtn = el.querySelector('#show-images-btn');
      if (showBtn) {
        showBtn.addEventListener('click', () => {
          remoteImagesShown = true;
          iframe.srcdoc = mailDocument(showRemoteImages(body.html), { allowRemoteImages: true });
          const banner = el.querySelector('#image-banner');
          if (banner) banner.style.display = 'none';
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
        dismissedReceipts.add(mail.id);
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
      dismissedReceipts.add(mail.id);
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
      iframe.style.transform = `scale(${getZoom() / 100})`;
      iframe.style.transformOrigin = 'top left';
      iframe.style.width = `${10000 / getZoom()}%`;
      iframe.style.height = `${10000 / getZoom()}%`;
    }
    if (textEl) {
      textEl.style.fontSize = `${14 * getZoom() / 100}px`;
    }
    const display = el.querySelector('#zoom-level-display');
    if (display) display.textContent = `${Math.round(getZoom())}%`;
    saveZoomDebounced(getZoom());
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
    'print': () => printEmail(mail, body, remoteImagesShown),
    'export': () => exportEmail(mail),
    'view-headers': () => viewHeaders(mail),
    'zoom-in': () => { setZoom(Math.min(200, getZoom() + 10)); applyZoom(); },
    'zoom-out': () => { setZoom(Math.max(50, getZoom() - 10)); applyZoom(); },
    'zoom-reset': () => { setZoom(100); applyZoom(); },
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
      const query = new URLSearchParams({
        id: mail.id,
        subject: mail.subject || '(No subject)',
        from: mail.from || 'Unknown',
        to: String(mail.to || ''),
        date: formatFullDate(mail.date),
        images: remoteImagesShown ? '1' : '',
      });
      api.openMailWindow(mail.subject || '(No subject)', query.toString())
        .catch((err) => showToast(`Failed to open window: ${err}`, 'error'));
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
      if (reminderTimers.has(mailId)) clearTimeout(reminderTimers.get(mailId));
      const timer = setTimeout(() => {
        reminderTimers.delete(mailId);
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
      reminderTimers.set(mailId, timer);
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

/**
 * Print the current email from a hidden sandboxed iframe (no scripts).
 */
function printEmail(mail, body, remoteImagesShown) {
  if (!body) return;
  const content = body.html
    ? (remoteImagesShown ? showRemoteImages(body.html) : body.html)
    : `<pre style="white-space:pre-wrap;font-family:sans-serif;">${esc(body.text || '')}</pre>`;
  const frame = document.createElement('iframe');
  frame.setAttribute('sandbox', 'allow-same-origin allow-modals');
  frame.title = 'Print preview';
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0;';
  frame.addEventListener('load', () => {
    const win = frame.contentWindow;
    // print() blocks on some engines and not on others: clean up on afterprint, with a fallback
    win.addEventListener('afterprint', () => frame.remove(), { once: true });
    setTimeout(() => frame.remove(), 60000);
    win.print();
  }, { once: true });
  frame.srcdoc = mailDocument(content, { allowRemoteImages: remoteImagesShown, title: mail.subject });
  document.body.appendChild(frame);
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
