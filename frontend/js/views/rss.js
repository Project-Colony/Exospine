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
      <div class="settings-panel" style="max-width:800px;">
        <div class="settings-header">
          <h2>RSS Reader</h2>
          <button class="settings-close" id="rss-close" aria-label="Close">&times;</button>
        </div>
        <div style="display:flex;gap:16px;padding:16px;min-height:400px;">
          <div style="width:220px;flex-shrink:0;">
            <h3 style="margin:0 0 8px;">Feeds</h3>
            <div id="rss-feed-list" style="margin-bottom:12px;">
              ${feeds.length === 0 ? '<div style="color:var(--pane-text-dim);font-size:13px;">No feeds added</div>' : ''}
              ${feeds.map((f, i) => `
                <div class="rss-feed-item ${selectedFeed === i ? 'active' : ''}" data-idx="${i}" style="padding:6px 8px;cursor:pointer;border-radius:4px;margin-bottom:2px;display:flex;justify-content:space-between;align-items:center;">
                  <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${esc(f.title || f.url)}</span>
                  <button data-del="${i}" style="background:none;border:none;color:var(--danger);cursor:pointer;font-size:14px;" title="Remove">&times;</button>
                </div>
              `).join('')}
            </div>
            <div style="display:flex;gap:4px;">
              <input id="rss-url" placeholder="Feed URL..." style="flex:1;padding:4px 8px;border:1px solid var(--separator);border-radius:4px;font-size:12px;background:var(--pane-bg);color:var(--pane-text);">
              <button id="rss-add" style="padding:4px 10px;border-radius:4px;background:var(--accent);color:#fff;border:none;cursor:pointer;font-size:12px;">Add</button>
            </div>
          </div>
          <div style="flex:1;overflow-y:auto;" id="rss-items">
            ${items.length === 0 ? '<div style="color:var(--pane-text-dim);text-align:center;padding-top:60px;">Select a feed to view items</div>' : ''}
            ${items.map(item => `
              <div style="padding:8px 0;border-bottom:1px solid var(--separator);">
                <a href="${esc(item.link)}" target="_blank" rel="noopener" style="color:var(--accent);text-decoration:none;font-weight:600;font-size:14px;">${esc(item.title)}</a>
                ${item.pubDate ? `<div style="font-size:11px;color:var(--pane-text-dim);margin-top:2px;">${esc(item.pubDate)}</div>` : ''}
                ${item.description ? `<div style="font-size:13px;margin-top:4px;color:var(--pane-text);">${esc(item.description.substring(0, 200))}${item.description.length > 200 ? '...' : ''}</div>` : ''}
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
      link: item.querySelector('link')?.textContent || '',
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
      link: entry.querySelector('link')?.getAttribute('href') || '',
      description: entry.querySelector('summary')?.textContent || entry.querySelector('content')?.textContent || '',
      pubDate: entry.querySelector('published')?.textContent || entry.querySelector('updated')?.textContent || '',
    }));
    return { title, items };
  }

  throw new Error('Unknown feed format');
}

function esc(s) {
  if (!s) return '';
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
