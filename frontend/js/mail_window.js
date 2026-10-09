// Exospine: one message in its own window, opened by the open_mail_window command.
// The body comes from get_mail_body, so it is the same sanitized HTML as the reading pane.
import { getMailBody } from './api.js';
import { esc, mailDocument, openLinksExternally, showRemoteImages } from './views/mail_view.js';

const params = new URLSearchParams(location.search);
document.getElementById('mail-subject').textContent = params.get('subject') || '';
for (const [field, label] of [['from', 'From'], ['to', 'To'], ['date', 'Date']]) {
  document.getElementById(`mail-${field}`).textContent = `${label}: ${params.get(field) || ''}`;
}

const frame = document.getElementById('email-frame');
openLinksExternally(frame);
try {
  const body = await getMailBody(params.get('id'));
  const allowRemoteImages = params.get('images') === '1';
  const content = body.html
    ? (allowRemoteImages ? showRemoteImages(body.html) : body.html)
    : `<pre style="white-space:pre-wrap;font-family:inherit;">${esc(body.text || '')}</pre>`;
  frame.srcdoc = mailDocument(content, { allowRemoteImages });
} catch (err) {
  frame.srcdoc = mailDocument(`<p>Failed to load the message: ${esc(String(err))}</p>`);
}
