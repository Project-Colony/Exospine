// Exospine: attachments in the reading pane: icons, sizes, download and preview.

import * as api from '../../api.js';
import { showToast } from '../../components/toast.js';
import { esc } from '../../html.js';
import { t } from '../../i18n/index.js';

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

export function getPreviewType(att) {
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
export async function openAttachmentPreview(el, mail, partIndex, previewType, contentType, filename) {
  const section = el.querySelector('#attachment-preview-section');
  const content = el.querySelector('#attachment-preview-content');
  if (!section || !content) return;

  section.hidden = false;
  section.setAttribute('aria-expanded', 'true');
  content.innerHTML = `<div class="preview-loading">${t('loading_preview')}</div>`;

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
          <img src="${src}" alt="${esc(filename)}" class="attachment-preview-thumbnail" tabindex="0" role="button" aria-label="${esc(t('view_full_size', { name: filename }))}" />
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
        <iframe src="${src}" class="attachment-preview-pdf" title="${esc(t('pdf_preview', { name: filename }))}"></iframe>
      `;
    } else if (previewType === 'text') {
      const textContent = typeof data === 'string' && !data.startsWith('data:')
        ? atob(data)
        : data;
      content.innerHTML = `
        <pre class="attachment-preview-text" tabindex="0" aria-label="${esc(t('text_file_content', { name: filename }))}">${esc(textContent)}</pre>
      `;
    }
  } catch (err) {
    content.innerHTML = `<div class="preview-error">${esc(t('error_load_preview', { error: err }))}</div>`;
  }
}

// ── Feature: Image lightbox ────────────────────────────────────────
function showImageLightbox(src, filename) {
  const overlay = document.createElement('div');
  overlay.className = 'lightbox-overlay';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', t('image_preview', { name: filename }));
  overlay.innerHTML = `
    <button class="lightbox-close" aria-label="${t('close_image_preview')}">\u00D7</button>
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

/**
 * Download an attachment via the backend command.
 */
export async function handleDownloadAttachment(mail, partIndex) {
  try {
    showToast(t('downloading_attachment'), 'info');
    const filePath = await api.downloadAttachment(
      mail.account_id,
      mail.folder,
      mail.uid,
      partIndex,
    );
    showToast(t('saved_to', { path: filePath }), 'success');
  } catch (err) {
    showToast(t('error_download', { error: err }), 'error');
  }
}

export function attachmentIcon(contentType) {
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

export function formatSize(bytes) {
  if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return bytes + ' B';
}
