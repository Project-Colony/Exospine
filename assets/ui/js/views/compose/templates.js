// Exospine: the built-in and saved mail templates of the compose view.

import { showToast } from '../../components/toast.js';
import { t } from '../../i18n/index.js';
import { esc } from '../../html.js';
import { overlay } from './editor.js';

// The built-in templates: their name, subject and body are i18n keys.
const PREDEFINED_TEMPLATES = [
  {
    name: 'template_meeting',
    subject: 'template_meeting_subject',
    body: 'template_meeting_body',
  },
  {
    name: 'template_followup',
    subject: 'template_followup_subject',
    body: 'template_followup_body',
  },
  {
    name: 'template_thankyou',
    subject: 'template_thankyou_subject',
    body: 'template_thankyou_body',
  },
  {
    name: 'template_ooo',
    subject: 'template_ooo_subject',
    body: 'template_ooo_body',
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

export function setupTemplatesDropdown(prefill) {
  const btn = overlay.querySelector('#compose-templates-btn');
  const dropdown = overlay.querySelector('#compose-templates-dropdown');
  if (!btn || !dropdown) return;

  function renderDropdown() {
    const customTemplates = loadCustomTemplates();
    const allTemplates = [
      ...PREDEFINED_TEMPLATES.map((tpl) => ({ subject: t(tpl.subject), body: t(tpl.body), displayName: t(tpl.name) })),
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
        const name = prompt(t('template_name_prompt'));
        if (!name) return;
        const subject = overlay.querySelector('#compose-subject')?.value || '';
        const body = overlay.querySelector('#compose-body')?.innerHTML || '';
        const customs = loadCustomTemplates();
        customs.push({ name, subject, body });
        saveCustomTemplates(customs);
        showToast(t('template_saved'), 'success');
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
