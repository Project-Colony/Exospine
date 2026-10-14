// Exospine: one message in its own window, opened by the open_mail_window command.
// The body comes from get_mail_body, so it is the same sanitized HTML as the reading pane.
import { getMailBody } from './api.js';
import { esc } from './html.js';
import { restoreAppearance } from './appearance.js';
import { setLanguage, t } from './i18n/index.js';
import { mailDocument, openLinksExternally, showRemoteImages } from './views/mail_view/index.js';

const params = new URLSearchParams(location.search);
restoreAppearance();
setLanguage(params.get('lang'));
document.getElementById('mail-subject').textContent = params.get('subject') || '';
for (const field of ['from', 'to', 'date']) {
  document.getElementById(`mail-${field}`).textContent = t(`popout_${field}`, { value: params.get(field) || '' });
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
  frame.srcdoc = mailDocument(`<p>${esc(t('error_load_message', { error: err }))}</p>`);
}
