// Exospine RSS Reader
import { showToast } from '../components/toast.js';
import { t } from '../i18n.js';

const LS_KEY = 'exospine_rss_feeds';

function loadFeeds() {
  try { return JSON.parse(localStorage.getItem(LS_KEY)) || []; } catch { return []; }
}
function saveFeeds(feeds) { localStorage.setItem(LS_KEY, JSON.stringify(feeds)); }

export function openRss() {
  const overlay = document.getElementById('settings-overlay');
  if (!overlay) return;
  overlay.hidden = false;

  let feeds = loadFeeds();
  let selectedFeed = null;
  let items = [];

  function render() {
    overlay.innerHTML = `
      <div class="settings-panel rss-panel">
        <div class="settings-header">
          <h2>RSS Reader</h2>
          <button class="settings-close" id="rss-close" aria-label="Close">&times;</button>
        </div>
        <div class="rss-layout">
          <div class="rss-sidebar">
            <h3 class="rss-sidebar h3">Feeds</h3>
            <div id="rss-feed-list" class="rss-feed-list">
              ${feeds.length === 0 ? '<div class="text-dim text-sm">No feeds added</div>' : ''}
              ${feeds.map((f, i) => `
                <div class="rss-feed-item ${selectedFeed === i ? 'active' : ''}" data-idx="${i}">
                  <span class="rss-feed-item-title">${esc(f.title || f.url)}</span>
                  <button data-del="${i}" class="rss-feed-del" title="Remove">&times;</button>
                </div>
              `).join('')}
            </div>
            <div class="rss-add-row">
              <input id="rss-url" placeholder="Feed URL..." class="rss-url-input">
              <button id="rss-add" class="rss-add-btn">Add</button>
            </div>
          </div>
          <div class="rss-content" id="rss-items">
            ${items.length === 0 ? '<div class="mail-view-status-text">Select a feed to view items</div>' : ''}
            ${items.map(item => `
              <div class="rss-item">
                ${item.link
                  ? `<a href="${esc(item.link)}" target="_blank" rel="noopener" class="rss-item-link">${esc(item.title)}</a>`
                  : `<span class="rss-item-link">${esc(item.title)}</span>`}
                ${item.pubDate ? `<div class="rss-item-date">${esc(item.pubDate)}</div>` : ''}
                ${item.description ? `<div class="rss-item-desc">${esc(item.description.substring(0, 200))}${item.description.length > 200 ? '...' : ''}</div>` : ''}
              </div>
            `).join('')}
          </div>
        </div>
      </div>
    `;

    // Events
    overlay.querySelector('#rss-close')?.addEventListener('click', close);
    overlay.querySelector('#rss-add')?.addEventListener('click', addFeed);
    overlay.querySelectorAll('[data-idx]').forEach(el => {
      el.addEventListener('click', (e) => {
        if (e.target.closest('[data-del]')) return;
        selectFeed(parseInt(el.dataset.idx));
      });
    });
    overlay.querySelectorAll('[data-del]').forEach(el => {
      el.addEventListener('click', () => {
        feeds.splice(parseInt(el.dataset.del), 1);
        saveFeeds(feeds);
        selectedFeed = null;
        items = [];
        render();
      });
    });
  }

  async function addFeed() {
    const input = overlay.querySelector('#rss-url');
    const url = input?.value.trim();
    if (!url) return;
    try {
      const parsed = await fetchFeed(url);
      feeds.push({ url, title: parsed.title || url });
      saveFeeds(feeds);
      selectedFeed = feeds.length - 1;
      items = parsed.items;
      render();
    } catch (e) {
      showToast(`Failed to load feed: ${e}`, 'error');
    }
  }

  async function selectFeed(idx) {
    selectedFeed = idx;
    const feed = feeds[idx];
    if (!feed) return;
    try {
      const parsed = await fetchFeed(feed.url);
      feed.title = parsed.title || feed.url;
      saveFeeds(feeds);
      items = parsed.items;
      render();
    } catch (e) {
      showToast(`Failed to load feed: ${e}`, 'error');
    }
  }

  function close() {
    overlay.hidden = true;
    overlay.innerHTML = '';
  }

  const onKey = (e) => { if (e.key === 'Escape') { close(); document.removeEventListener('keydown', onKey); } };
  document.addEventListener('keydown', onKey);

  render();
}

async function fetchFeed(url) {
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const text = await resp.text();
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, 'text/xml');

  // RSS 2.0
  const channel = doc.querySelector('channel');
  if (channel) {
    const title = channel.querySelector('title')?.textContent || '';
    const items = [...channel.querySelectorAll('item')].map(item => ({
      title: item.querySelector('title')?.textContent || '',
      link: webLink(item.querySelector('link')?.textContent, url),
      description: item.querySelector('description')?.textContent || '',
      pubDate: item.querySelector('pubDate')?.textContent || '',
    }));
    return { title, items };
  }

  // Atom
  const feed = doc.querySelector('feed');
  if (feed) {
    const title = feed.querySelector('title')?.textContent || '';
    const items = [...feed.querySelectorAll('entry')].map(entry => ({
      title: entry.querySelector('title')?.textContent || '',
      link: webLink(entry.querySelector('link')?.getAttribute('href'), url),
      description: entry.querySelector('summary')?.textContent || entry.querySelector('content')?.textContent || '',
      pubDate: entry.querySelector('published')?.textContent || entry.querySelector('updated')?.textContent || '',
    }));
    return { title, items };
  }

  throw new Error('Unknown feed format');
}

/** Feed links are untrusted: keep only http(s) URLs, resolved against the feed URL. */
function webLink(raw, feedUrl) {
  if (!raw?.trim()) return '';
  try {
    const link = new URL(raw.trim(), feedUrl);
    return link.protocol === 'http:' || link.protocol === 'https:' ? link.href : '';
  } catch {
    return '';
  }
}

function esc(s) {
  if (!s) return '';
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
