// Exospine: the compose editor: formatting toolbar, pasted and dropped files,
// address completion, and conversions between HTML, Markdown and plain text.

import * as api from '../../api.js';
import { esc } from '../../html.js';

/** The compose overlay element, which every part of the compose view draws into. */
export const overlay = document.getElementById('compose-overlay');

/** Fallback email signature used when no per-account signature is configured. */
export const FALLBACK_SIGNATURE = '<br><br><div class="compose-signature">--<br>Sent with Exospine</div>';

export function setupToolbar() {
  const toolbar = overlay.querySelector('#compose-toolbar');
  if (!toolbar) return;

  // Button commands
  toolbar.querySelectorAll('button[data-cmd]').forEach((btn) => {
    btn.addEventListener('mousedown', (e) => {
      e.preventDefault(); // keep focus in editor
    });
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const cmd = btn.dataset.cmd;

      if (cmd === 'createLink') {
        const url = prompt('Enter URL:', 'https://');
        if (url) document.execCommand('createLink', false, url);
        return;
      }

      document.execCommand(cmd, false, null);
    });
  });

  // Font size select
  const fontSizeSelect = toolbar.querySelector('select[data-cmd="fontSize"]');
  if (fontSizeSelect) {
    fontSizeSelect.addEventListener('mousedown', (e) => e.stopPropagation());
    fontSizeSelect.addEventListener('change', () => {
      const val = fontSizeSelect.value;
      if (val) {
        document.execCommand('fontSize', false, val);
      }
      fontSizeSelect.value = '';
    });
  }

  // Text color picker
  const colorInput = toolbar.querySelector('input[data-cmd="foreColor"]');
  if (colorInput) {
    colorInput.addEventListener('input', () => {
      document.execCommand('foreColor', false, colorInput.value);
    });
  }
}

export function setupImagePaste() {
  const editor = overlay.querySelector('#compose-body');
  if (!editor) return;

  editor.addEventListener('paste', (e) => {
    const items = e.clipboardData && e.clipboardData.items;
    if (!items) return;

    for (const item of items) {
      if (item.type.startsWith('image/')) {
        e.preventDefault();
        const blob = item.getAsFile();
        if (!blob) continue;

        const reader = new FileReader();
        reader.onload = () => {
          const dataUri = reader.result;
          document.execCommand('insertImage', false, dataUri);
        };
        reader.readAsDataURL(blob);
        return; // handle only first image
      }
    }
  });

  // Also support drag-and-drop of images
  editor.addEventListener('drop', (e) => {
    const files = e.dataTransfer && e.dataTransfer.files;
    if (!files || files.length === 0) return;

    for (const file of files) {
      if (file.type.startsWith('image/')) {
        e.preventDefault();
        const reader = new FileReader();
        reader.onload = () => {
          const dataUri = reader.result;
          document.execCommand('insertImage', false, dataUri);
        };
        reader.readAsDataURL(file);
        return;
      }
    }
  });
}

export function setupFileDrop(overlay, attachments) {
  const editor = overlay.querySelector('#compose-body');
  const bodyWrap = overlay.querySelector('.compose-body-wrap');
  if (!editor || !bodyWrap) return;

  function formatAttSize(bytes) {
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return bytes + ' B';
  }

  function renderAttachmentPills() {
    const container = overlay.querySelector('#compose-attachments');
    const list = overlay.querySelector('#compose-attachments-list');
    if (!container || !list) return;

    if (attachments.length === 0) {
      container.hidden = true;
      return;
    }

    container.hidden = false;
    list.innerHTML = attachments.map((att, idx) =>
      `<div class="compose-attachment-pill" data-att-idx="${idx}">
        <span class="compose-attachment-pill-name" title="${esc(att.name)}">${esc(att.name)}</span>
        <span class="compose-attachment-pill-size">${formatAttSize(att.size)}</span>
        <button class="compose-attachment-pill-remove" data-att-idx="${idx}" title="Remove">\u00D7</button>
      </div>`
    ).join('');

    list.querySelectorAll('.compose-attachment-pill-remove').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const idx = parseInt(btn.dataset.attIdx, 10);
        attachments.splice(idx, 1);
        renderAttachmentPills();
      });
    });
  }

  function addFiles(files) {
    for (const file of files) {
      const reader = new FileReader();
      reader.onload = () => {
        attachments.push({
          name: file.name,
          type: file.type || 'application/octet-stream',
          size: file.size,
          data: reader.result,
        });
        renderAttachmentPills();
      };
      reader.readAsDataURL(file);
    }
  }

  // Prevent default browser behavior on the whole compose panel
  bodyWrap.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    bodyWrap.classList.add('compose-drop-active');
  });

  bodyWrap.addEventListener('dragleave', (e) => {
    e.preventDefault();
    bodyWrap.classList.remove('compose-drop-active');
  });

  bodyWrap.addEventListener('drop', (e) => {
    bodyWrap.classList.remove('compose-drop-active');
    const files = e.dataTransfer && e.dataTransfer.files;
    if (!files || files.length === 0) return;

    e.preventDefault();
    e.stopPropagation();
    addFiles(Array.from(files));
  });
}

/** Build initial HTML content for the editor (body + signature). */
export function buildInitialContent(prefill) {
  let content = '';
  const signature = prefill._signatureHtml || FALLBACK_SIGNATURE;

  if (prefill.bodyHtml) {
    content = prefill.bodyHtml;
  } else if (prefill.body) {
    // Convert plain text prefill to HTML (preserve newlines)
    content = esc(prefill.body).replace(/\n/g, '<br>');
  }

  // Add signature: for new emails place cursor before it; for replies/forwards append after quoted text
  if (!prefill.body && !prefill.bodyHtml) {
    content = '<br>' + signature;
  } else {
    // Insert signature between cursor area and quoted text
    content = '<br>' + signature + content;
  }

  return content;
}

/** Place the cursor at the very start of the contenteditable div. */
export function placeCursorAtStart(el) {
  el.focus();
  const range = document.createRange();
  range.setStart(el, 0);
  range.collapse(true);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
}

/** Convert HTML to plain text (strip tags, decode entities). */
export function htmlToPlainText(html) {
  const tmp = parseInert(html);

  // Convert <br> to newlines
  tmp.querySelectorAll('br').forEach((br) => br.replaceWith('\n'));

  // Convert block elements to newlines
  tmp.querySelectorAll('p, div, li, h1, h2, h3, h4, h5, h6').forEach((el) => {
    el.prepend('\n');
  });

  // Convert <hr> to ---
  tmp.querySelectorAll('hr').forEach((hr) => hr.replaceWith('\n---\n'));

  // Convert <a> to text [link](url)
  tmp.querySelectorAll('a[href]').forEach((a) => {
    const text = a.textContent;
    const href = a.getAttribute('href');
    if (text === href) {
      a.replaceWith(href);
    } else {
      a.replaceWith(`${text} (${href})`);
    }
  });

  // Convert list items
  tmp.querySelectorAll('ol').forEach((ol) => {
    let i = 1;
    ol.querySelectorAll(':scope > li').forEach((li) => {
      li.prepend(`${i++}. `);
    });
  });
  tmp.querySelectorAll('ul').forEach((ul) => {
    ul.querySelectorAll(':scope > li').forEach((li) => {
      li.prepend('- ');
    });
  });

  let text = tmp.textContent || '';
  // Collapse multiple blank lines
  text = text.replace(/\n{3,}/g, '\n\n').trim();
  return text;
}

/**
 * Parse HTML into the body of a DOMParser document. Unlike innerHTML on a
 * detached element of this page, nothing in it loads or runs.
 */
export function parseInert(html) {
  return new DOMParser().parseFromString(html, 'text/html').body;
}

export function setupAutocomplete(inputId, dropdownId) {
  const input = overlay.querySelector(`#${inputId}`);
  const dropdown = overlay.querySelector(`#${dropdownId}`);
  if (!input || !dropdown) return null;

  let debounce = null;
  let blurTimeout = null;

  input.addEventListener('input', () => {
    clearTimeout(debounce);
    debounce = setTimeout(async () => {
      // Get the text after the last comma (for multi-recipient fields)
      const val = input.value;
      const parts = val.split(',');
      const query = (parts[parts.length - 1] || '').trim();

      if (query.length < 2) {
        dropdown.hidden = true;
        return;
      }

      try {
        const contacts = await api.searchContacts(query);
        if (!contacts || contacts.length === 0) {
          dropdown.hidden = true;
          return;
        }

        dropdown.innerHTML = contacts
          .map(
            (c) =>
              `<div class="compose-autocomplete-item" data-email="${esc(c.email)}" data-name="${esc(c.name)}">
                <span class="compose-autocomplete-name">${esc(c.name || c.email)}</span>
                ${c.name ? `<span class="compose-autocomplete-email">&lt;${esc(c.email)}&gt;</span>` : ''}
              </div>`
          )
          .join('');
        dropdown.hidden = false;

        dropdown.querySelectorAll('.compose-autocomplete-item').forEach((item) => {
          item.addEventListener('mousedown', (e) => {
            e.preventDefault();
            const email = item.dataset.email;
            const name = item.dataset.name;

            // Validate email format before inserting
            if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
              return; // Skip invalid email entries
            }

            const display = name ? `${name} <${email}>` : email;

            // Replace the current part (after last comma) with the selected contact
            const parts = input.value.split(',');
            parts[parts.length - 1] = ' ' + display;
            input.value = parts.join(',') + ', ';
            dropdown.hidden = true;
            input.focus();
          });
        });
      } catch {
        dropdown.hidden = true;
      }
    }, 200);
  });

  input.addEventListener('blur', () => {
    // Small delay to allow click on dropdown items
    blurTimeout = setTimeout(() => {
      dropdown.hidden = true;
    }, 150);
  });

  // Return cleanup function to cancel pending timeouts
  return () => {
    clearTimeout(debounce);
    clearTimeout(blurTimeout);
  };
}

/**
 * Convert basic Markdown to HTML.
 * Supports: **bold**, *italic*, `code`, ```code blocks```, [text](url),
 * # headings (h1-h6), - unordered lists, 1. ordered lists, > blockquotes, --- hr.
 */
export function markdownToHtml(md) {
  if (!md) return '';

  let html = esc(md);

  // Code blocks (``` ... ```)
  html = html.replace(/```([\s\S]*?)```/g, '<pre><code>$1</code></pre>');

  // Inline code
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Bold (**text** or __text__)
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/__(.+?)__/g, '<strong>$1</strong>');

  // Italic (*text* or _text_)
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  html = html.replace(/_(.+?)_/g, '<em>$1</em>');

  // Links [text](url)
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');

  // Headings (# to ######)
  html = html.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>');
  html = html.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>');
  html = html.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>');
  html = html.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');

  // Horizontal rule
  html = html.replace(/^---+$/gm, '<hr>');

  // Blockquotes
  html = html.replace(/^&gt;\s?(.+)$/gm, '<blockquote>$1</blockquote>');
  // Merge consecutive blockquotes
  html = html.replace(/<\/blockquote>\n<blockquote>/g, '\n');

  // Unordered lists (- item or * item)
  html = html.replace(/^[\-\*]\s+(.+)$/gm, '<li>$1</li>');
  html = html.replace(/((?:<li>.*<\/li>\n?)+)/g, '<ul>$1</ul>');

  // Ordered lists (1. item)
  html = html.replace(/^\d+\.\s+(.+)$/gm, '<li>$1</li>');
  // Wrap consecutive <li> not inside <ul> into <ol>
  html = html.replace(/(<li>.*<\/li>\n?)+/g, (match) => {
    if (match.includes('<ul>')) return match;
    return '<ol>' + match + '</ol>';
  });

  // Convert remaining newlines to <br>
  html = html.replace(/\n/g, '<br>');

  // Clean up <br> inside block elements
  html = html.replace(/<br>(<\/?(h[1-6]|ul|ol|li|blockquote|pre|hr))/g, '$1');
  html = html.replace(/(<(h[1-6]|ul|ol|li|blockquote|pre|hr)[^>]*>)<br>/g, '$1');

  return html;
}
