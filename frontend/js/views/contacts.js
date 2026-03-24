// Exospine — Contacts Management View

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { showDialog } from '../components/dialog.js';

const overlay = document.getElementById('settings-overlay');

/**
 * Open the contacts management overlay.
 */
export async function openContacts() {
  let contacts = [];
  let searchQuery = '';
  let selectedContact = null;
  let editMode = false;

  overlay.hidden = false;

  try {
    contacts = await api.getAllContacts();
  } catch (err) {
    contacts = [];
  }

  render();

  function getFilteredContacts() {
    if (!searchQuery) return contacts;
    const q = searchQuery.toLowerCase();
    return contacts.filter(c =>
      (c.name || '').toLowerCase().includes(q) ||
      (c.email || '').toLowerCase().includes(q) ||
      (c.company || '').toLowerCase().includes(q)
    );
  }

  function getGroupedContacts() {
    const filtered = getFilteredContacts();
    const groups = {};
    for (const c of filtered) {
      const letter = ((c.name || c.email || '?')[0] || '?').toUpperCase();
      const key = /[A-Z]/.test(letter) ? letter : '#';
      if (!groups[key]) groups[key] = [];
      groups[key].push(c);
    }
    // Sort groups alphabetically, # last
    const sorted = Object.keys(groups).sort((a, b) => {
      if (a === '#') return 1;
      if (b === '#') return -1;
      return a.localeCompare(b);
    });
    return sorted.map(key => ({ letter: key, contacts: groups[key] }));
  }

  function render() {
    const grouped = getGroupedContacts();
    const filtered = getFilteredContacts();
    const total = contacts.length;

    let listHtml = '';
    for (const group of grouped) {
      listHtml += `<div class="contacts-group-letter">${esc(group.letter)}</div>`;
      for (const c of group.contacts) {
        const isActive = selectedContact && selectedContact.email === c.email;
        const initial = (c.name || c.email || '?')[0].toUpperCase();
        listHtml += `
          <div class="contacts-item${isActive ? ' active' : ''}" data-email="${esc(c.email)}">
            <div class="contacts-item-avatar">${initial}</div>
            <div class="contacts-item-info">
              <div class="contacts-item-name">${esc(c.name || c.email)}</div>
              <div class="contacts-item-email">${esc(c.email)}</div>
            </div>
          </div>
        `;
      }
    }

    if (filtered.length === 0 && searchQuery) {
      listHtml = '<div class="contacts-empty">No contacts matching your search.</div>';
    } else if (filtered.length === 0) {
      listHtml = '<div class="contacts-empty">No contacts yet. Contacts are automatically extracted from emails.</div>';
    }

    // Detail panel
    let detailHtml = '';
    if (selectedContact) {
      const c = selectedContact;
      if (editMode) {
        detailHtml = `
          <div class="contacts-detail-edit">
            <div class="settings-section-title">Edit Contact</div>
            <div class="settings-row">
              <label>Name</label>
              <input type="text" id="contact-edit-name" value="${esc(c.name)}" class="calendar-input" />
            </div>
            <div class="settings-row">
              <label>Email</label>
              <input type="email" id="contact-edit-email" value="${esc(c.email)}" class="calendar-input" disabled style="opacity:0.6" />
            </div>
            <div class="settings-row">
              <label>Phone</label>
              <input type="tel" id="contact-edit-phone" value="${esc(c.phone || '')}" class="calendar-input" />
            </div>
            <div class="settings-row">
              <label>Company</label>
              <input type="text" id="contact-edit-company" value="${esc(c.company || '')}" class="calendar-input" />
            </div>
            <div class="settings-row">
              <label>Notes</label>
              <textarea id="contact-edit-notes" rows="4" class="calendar-input">${esc(c.notes || '')}</textarea>
            </div>
            <div style="display:flex;gap:6px;margin-top:12px;">
              <button class="btn btn-primary btn-sm" id="contact-save-btn">Save</button>
              <button class="btn btn-ghost btn-sm" id="contact-cancel-edit-btn">Cancel</button>
            </div>
          </div>
        `;
      } else {
        const initial = (c.name || c.email || '?')[0].toUpperCase();
        detailHtml = `
          <div class="contacts-detail-view">
            <div class="contacts-detail-header">
              <div class="contacts-detail-avatar">${initial}</div>
              <div>
                <div class="contacts-detail-name">${esc(c.name || 'No name')}</div>
                <div class="contacts-detail-email">${esc(c.email)}</div>
              </div>
            </div>
            <div class="contacts-detail-fields">
              ${c.phone ? `<div class="contacts-detail-field"><span class="contacts-field-label">Phone</span><span>${esc(c.phone)}</span></div>` : ''}
              ${c.company ? `<div class="contacts-detail-field"><span class="contacts-field-label">Company</span><span>${esc(c.company)}</span></div>` : ''}
              ${c.notes ? `<div class="contacts-detail-field"><span class="contacts-field-label">Notes</span><span style="white-space:pre-wrap;">${esc(c.notes)}</span></div>` : ''}
              <div class="contacts-detail-field"><span class="contacts-field-label">Emails exchanged</span><span>${c.frequency || 0}</span></div>
            </div>
            <div style="display:flex;gap:6px;margin-top:16px;">
              <button class="btn btn-ghost btn-sm" id="contact-edit-btn">Edit</button>
              <button class="btn btn-danger btn-sm" id="contact-delete-btn">Delete</button>
            </div>
          </div>
        `;
      }
    } else {
      detailHtml = '<div class="contacts-empty">Select a contact to view details.</div>';
    }

    overlay.innerHTML = `
      <div class="overlay-panel settings-panel contacts-panel">
        <div class="settings-header">
          <span class="settings-title">Contacts (${total})</span>
          <button class="compose-close" id="contacts-close" title="Close">\u00D7</button>
        </div>
        <div class="contacts-body">
          <div class="contacts-list-pane">
            <div class="contacts-search">
              <input type="text" id="contacts-search-input" placeholder="Search contacts..." value="${esc(searchQuery)}" class="calendar-input" />
            </div>
            <div class="contacts-list-scroll">
              ${listHtml}
            </div>
          </div>
          <div class="contacts-detail-pane">
            ${detailHtml}
          </div>
        </div>
        <div class="settings-footer">
          <button class="btn btn-ghost" id="contacts-done">Close</button>
        </div>
      </div>
    `;

    // Wire events — use event delegation for contact items to avoid
    // stacking per-element listeners on every render() call.
    const onKey = (e) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('keydown', onKey);

    const close = () => {
      document.removeEventListener('keydown', onKey);
      overlay.hidden = true;
      overlay.innerHTML = '';
    };

    overlay.querySelector('#contacts-close').addEventListener('click', close);
    overlay.querySelector('#contacts-done').addEventListener('click', close);
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) close();
    });

    // Search
    const searchInput = overlay.querySelector('#contacts-search-input');
    if (searchInput) {
      searchInput.addEventListener('input', () => {
        searchQuery = searchInput.value;
        render();
        // Re-focus the search input after render
        const newInput = overlay.querySelector('#contacts-search-input');
        if (newInput) {
          newInput.focus();
          newInput.setSelectionRange(newInput.value.length, newInput.value.length);
        }
      });
    }

    // Select contact — event delegation on container
    const contactsList = overlay.querySelector('.contacts-list-scroll');
    if (contactsList) {
      contactsList.addEventListener('click', (e) => {
        const item = e.target.closest('.contacts-item');
        if (!item) return;
        const email = item.dataset.email;
        selectedContact = contacts.find(c => c.email === email) || null;
        editMode = false;
        render();
      });
    }

    // Edit button
    const editBtn = overlay.querySelector('#contact-edit-btn');
    if (editBtn) {
      editBtn.addEventListener('click', () => {
        editMode = true;
        render();
      });
    }

    // Cancel edit
    const cancelEditBtn = overlay.querySelector('#contact-cancel-edit-btn');
    if (cancelEditBtn) {
      cancelEditBtn.addEventListener('click', () => {
        editMode = false;
        render();
      });
    }

    // Save contact
    const saveBtn = overlay.querySelector('#contact-save-btn');
    if (saveBtn) {
      saveBtn.addEventListener('click', async () => {
        const name = overlay.querySelector('#contact-edit-name').value.trim();
        const phone = overlay.querySelector('#contact-edit-phone').value.trim();
        const company = overlay.querySelector('#contact-edit-company').value.trim();
        const notes = overlay.querySelector('#contact-edit-notes').value.trim();

        try {
          await api.updateContact(selectedContact.email, name, phone, company, notes);
          // Update local state
          const idx = contacts.findIndex(c => c.email === selectedContact.email);
          if (idx >= 0) {
            contacts[idx] = { ...contacts[idx], name, phone, company, notes };
            selectedContact = contacts[idx];
          }
          editMode = false;
          showToast('Contact updated.', 'success');
          render();
        } catch (err) {
          showToast(`Failed to save: ${err}`, 'error');
        }
      });
    }

    // Delete contact
    const deleteBtn = overlay.querySelector('#contact-delete-btn');
    if (deleteBtn) {
      deleteBtn.addEventListener('click', async () => {
        const confirmed = await showDialog({
          title: 'Delete Contact',
          message: `Delete ${selectedContact.name || selectedContact.email}?`,
          confirmLabel: 'Delete',
          danger: true,
        });
        if (!confirmed) return;

        try {
          await api.deleteContact(selectedContact.email);
          contacts = contacts.filter(c => c.email !== selectedContact.email);
          selectedContact = null;
          editMode = false;
          showToast('Contact deleted.', 'info');
          render();
        } catch (err) {
          showToast(`Failed to delete: ${err}`, 'error');
        }
      });
    }
  }
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
