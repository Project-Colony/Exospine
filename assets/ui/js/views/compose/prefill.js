// Exospine: what a reply or a forward starts with.

import { esc } from '../../html.js';
import { t } from '../../i18n/index.js';
import { parseInert } from './editor.js';

/**
 * Prepare a reply prefill object.
 * `body` is the get_mail_body response: its `html` is already sanitized by the
 * backend, and it is the only message HTML that goes into the editor.
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
      <div class="compose-quoted-header">${t('quoted_original')}<br>${t('quoted_from', { value: safeFrom })}<br>${t('quoted_date', { value: safeDate })}</div>
      <blockquote>${originalContent}</blockquote>
    </div>`;
  }

  // Also keep plain text body for fallback
  const quotedBody = body
    ? `\n\n${t('quoted_original')}\n${t('quoted_from', { value: safeFrom })}\n${t('quoted_date', { value: safeDate })}\n\n${body.text || stripHtml(body.html || '')}`
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
 * Prepare a forward prefill object. Same sanitized `body` as makeReplyPrefill.
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
      <div class="compose-quoted-header">${t('quoted_forwarded')}<br>${t('quoted_from', { value: fwdFrom })}<br>${t('quoted_to', { value: fwdTo })}<br>${t('quoted_date', { value: fwdDate })}<br>${t('quoted_subject', { value: fwdSubj })}</div>
      <blockquote>${originalContent}</blockquote>
    </div>`;
  }

  const forwardBody = body
    ? `\n\n${t('quoted_forwarded')}\n${t('quoted_from', { value: fwdFrom })}\n${t('quoted_to', { value: fwdTo })}\n${t('quoted_date', { value: fwdDate })}\n${t('quoted_subject', { value: fwdSubj })}\n\n${body.text || stripHtml(body.html || '')}`
    : '';

  return {
    to: '',
    subject: fwdSubject,
    body: forwardBody,
    bodyHtml,
  };
}

function stripHtml(html) {
  return parseInert(html).textContent || '';
}

/** Escape a string for safe inclusion in plain text (strips HTML tags). */
function escapeForText(str) {
  if (!str) return '';
  return str.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
