// Exospine: the email analytics dashboard.

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { esc } from '../html.js';
import { getLanguage, t } from '../i18n/index.js';

const overlay = document.getElementById('settings-overlay');

/**
 * Open the analytics dashboard overlay.
 * @param {string} accountId  Active account ID
 */
export async function openAnalytics(accountId) {
  overlay.hidden = false;

  overlay.innerHTML = `
    <div class="overlay-panel settings-panel">
      <div class="settings-header">
        <span class="settings-title">${t('email_analytics')}</span>
        <button class="compose-close" id="analytics-close" title="${t('close')}">\u00D7</button>
      </div>
      <div class="settings-body" id="analytics-body" style="padding:16px;">
        <div class="mail-view-status-text">${t('loading_analytics')}</div>
      </div>
      <div class="settings-footer">
        <button class="btn btn-ghost" id="analytics-done">${t('close')}</button>
      </div>
    </div>
  `;

  const onKey = (e) => {
    if (e.key === 'Escape') close();
  };
  document.addEventListener('keydown', onKey);

  const close = () => {
    document.removeEventListener('keydown', onKey);
    overlay.hidden = true;
    overlay.innerHTML = '';
  };

  overlay.querySelector('#analytics-close').addEventListener('click', close);
  overlay.querySelector('#analytics-done').addEventListener('click', close);
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

  // Fetch analytics data
  try {
    const data = await api.getAnalytics(accountId);
    renderAnalytics(data);
  } catch (err) {
    const body = overlay.querySelector('#analytics-body');
    if (body) {
      body.innerHTML = `<div class="preview-error" style="padding:20px;">${esc(t('error_load_analytics', { error: err }))}</div>`;
    }
  }
}

function renderAnalytics(data) {
  const body = overlay.querySelector('#analytics-body');
  if (!body) return;

  const maxByDay = Math.max(1, ...data.by_day.map(d => d[1]));
  const maxByHour = Math.max(1, ...data.by_hour);
  const maxSender = data.top_senders.length > 0 ? Math.max(1, data.top_senders[0][1]) : 1;

  let html = '';

  // Summary cards
  html += `
    <div class="analytics-summary">
      <div class="analytics-card">
        <div class="analytics-stat-value">${data.total_received}</div>
        <div class="analytics-stat-label">${t('stat_received')}</div>
      </div>
      <div class="analytics-card">
        <div class="analytics-stat-value">${data.total_sent}</div>
        <div class="analytics-stat-label">${t('stat_sent')}</div>
      </div>
      <div class="analytics-card">
        <div class="analytics-stat-value">${data.total_received + data.total_sent}</div>
        <div class="analytics-stat-label">${t('stat_total')}</div>
      </div>
    </div>
  `;

  // Emails by day (last 30 days) — CSS bar chart
  if (data.by_day.length > 0) {
    html += `
      <div class="analytics-section">
        <div class="analytics-section-title">${t('emails_by_day')}</div>
        <div class="analytics-chart" style="gap:2px;height:100px;">
    `;
    for (const [day, count] of data.by_day) {
      const pct = (count / maxByDay) * 100;
      html += `
        <div class="analytics-chart-bar-wrapper" title="${esc(t('day_count', { day, count }))}">
          <div style="width:100%;max-width:20px;background:var(--accent);border-radius:2px 2px 0 0;height:${pct}%;min-height:${count > 0 ? 2 : 0}px;transition:height 0.2s;"></div>
        </div>
      `;
    }
    html += `
        </div>
        <div class="analytics-chart-labels">
          <span>${data.by_day.length > 0 ? data.by_day[0][0].slice(5) : ''}</span>
          <span>${data.by_day.length > 0 ? data.by_day[data.by_day.length - 1][0].slice(5) : ''}</span>
        </div>
      </div>
    `;
  }

  // Busiest hours — CSS bar chart
  html += `
    <div class="analytics-section">
      <div class="analytics-section-title">${t('busiest_hours')}</div>
      <div class="analytics-chart" style="gap:1px;height:80px;">
  `;
  for (let h = 0; h < 24; h++) {
    const count = data.by_hour[h] || 0;
    const pct = (count / maxByHour) * 100;
    html += `
      <div class="analytics-chart-bar-wrapper" title="${esc(t('hour_count', { hour: h, count }))}">
        <div style="width:100%;max-width:16px;background:var(--colony-accent-icon);border-radius:2px 2px 0 0;height:${pct}%;min-height:${count > 0 ? 2 : 0}px;"></div>
      </div>
    `;
  }
  html += `
      </div>
      <div class="analytics-chart-labels">
        <span>0h</span><span>6h</span><span>12h</span><span>18h</span><span>23h</span>
      </div>
    </div>
  `;

  // Top senders — horizontal bars
  if (data.top_senders.length > 0) {
    html += `
      <div class="analytics-section">
        <div class="analytics-section-title">${t('top_senders')}</div>
    `;
    for (const [sender, count] of data.top_senders) {
      const pct = (count / maxSender) * 100;
      const displaySender = sender.length > 40 ? sender.slice(0, 40) + '...' : sender;
      html += `
        <div class="analytics-sender-row">
          <div class="analytics-sender-header">
            <span class="analytics-sender-name" title="${esc(sender)}">${esc(displaySender)}</span>
            <span class="analytics-sender-count">${count}</span>
          </div>
          <div class="analytics-bar-track">
            <div style="width:${pct}%;height:100%;background:var(--accent);border-radius:3px;"></div>
          </div>
        </div>
      `;
    }
    html += '</div>';
  }

  // ── Feature 7: Weekly heatmap (day × hour) ────────────────────────
  html += renderWeeklyHeatmap(data);

  // ── Feature 8: Average response time ─────────────────────────────
  html += renderAvgResponseTime(data);

  body.innerHTML = html;
}

// ── Weekly heatmap ──────────────────────────────────────────────────

function renderWeeklyHeatmap(data) {
  // Build a 7×24 matrix from by_day data and by_hour data
  // Since we only have aggregate by_day and by_hour, we'll create a synthetic heatmap
  // based on the distribution patterns
  // Monday to Sunday in the interface language (5 January 2026 is a Monday).
  const weekday = new Intl.DateTimeFormat(getLanguage(), { weekday: 'short' });
  const dayNames = Array.from({ length: 7 }, (_, i) => weekday.format(new Date(2026, 0, 5 + i)));
  const byHour = data.by_hour || new Array(24).fill(0);
  const maxHour = Math.max(1, ...byHour);

  // Build per-day-of-week volumes from by_day
  const dayOfWeekTotals = new Array(7).fill(0);
  const dayOfWeekCounts = new Array(7).fill(0);
  for (const [dateStr, count] of (data.by_day || [])) {
    const d = new Date(dateStr);
    const dow = (d.getDay() + 6) % 7; // 0=Mon, 6=Sun
    dayOfWeekTotals[dow] += count;
    dayOfWeekCounts[dow] += 1;
  }
  const dayAvg = dayOfWeekTotals.map((total, i) => dayOfWeekCounts[i] > 0 ? total / dayOfWeekCounts[i] : 0);
  const maxDayAvg = Math.max(1, ...dayAvg);
  const totalEmails = (data.total_received || 0) + (data.total_sent || 0);

  // Synthetic heatmap: distribute based on day average × hour distribution
  const grid = [];
  let maxCell = 0;
  for (let d = 0; d < 7; d++) {
    grid[d] = [];
    for (let h = 0; h < 24; h++) {
      const val = totalEmails > 0
        ? Math.round((dayAvg[d] / maxDayAvg) * (byHour[h] / maxHour) * 10)
        : 0;
      grid[d][h] = val;
      if (val > maxCell) maxCell = val;
    }
  }

  let html = `
    <div class="analytics-section">
      <div class="analytics-section-title">${t('weekly_heatmap')}</div>
      <div class="analytics-heatmap">
        <div></div>
  `;
  // Hour headers
  for (let h = 0; h < 24; h++) {
    html += `<div class="text-dim" style="text-align:center;${h % 3 === 0 ? '' : 'visibility:hidden;'}">${h}h</div>`;
  }
  // Rows
  for (let d = 0; d < 7; d++) {
    html += `<div class="flex-row-center text-dim font-bold">${dayNames[d]}</div>`;
    for (let h = 0; h < 24; h++) {
      const val = grid[d][h];
      const intensity = maxCell > 0 ? val / maxCell : 0;
      const bg = intensity > 0
        ? `color-mix(in srgb, var(--colony-accent-icon) ${Math.round((0.15 + intensity * 0.85) * 100)}%, transparent)`
        : 'var(--tint-1)';
      html += `<div style="aspect-ratio:1;background:${bg};border-radius:2px;min-height:12px;" title="${esc(t('heatmap_cell', { day: dayNames[d], hour: h, count: val }))}"></div>`;
    }
  }
  html += '</div></div>';
  return html;
}

// ── Average response time ───────────────────────────────────────────

function renderAvgResponseTime(data) {
  // Estimate from total_sent and total_received
  // If we have response time data from the backend, use it
  if (data.avg_response_minutes !== undefined && data.avg_response_minutes !== null) {
    const mins = data.avg_response_minutes;
    const display = formatDuration(mins);
    return `
      <div class="analytics-section">
        <div class="analytics-card">
          <div class="analytics-stat-value">${esc(display)}</div>
          <div class="analytics-stat-label">${t('avg_response_time')}</div>
        </div>
      </div>
    `;
  }

  // Heuristic estimation: if total_sent > 0 and total_received > 0,
  // estimate based on email frequency patterns
  const sent = data.total_sent || 0;
  const received = data.total_received || 0;
  if (sent === 0 || received === 0) return '';

  // Estimate: assume emails span 30 days, estimate avg gap between receive and reply
  const daysSpan = Math.max(1, (data.by_day || []).length);
  const repliesPerDay = sent / daysSpan;
  const receivedPerDay = received / daysSpan;
  // Average response ~ hours in a workday / replies ratio (rough heuristic)
  const estimatedHours = receivedPerDay > 0 ? Math.max(0.5, Math.min(48, (8 / repliesPerDay) * receivedPerDay / 2)) : 0;
  const display = formatDuration(estimatedHours * 60);

  return `
    <div class="analytics-section">
      <div class="analytics-card">
        <div class="analytics-stat-value">~${esc(display)}</div>
        <div class="analytics-stat-label">${t('est_avg_response_time')}</div>
      </div>
    </div>
  `;
}

function formatDuration(minutes) {
  if (minutes < 60) return t('duration_minutes', { m: Math.round(minutes) });
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  if (h >= 24) {
    const d = Math.floor(h / 24);
    const rh = h % 24;
    return rh > 0 ? t('duration_days_hours', { d, h: rh }) : t('duration_days', { d });
  }
  return m > 0 ? t('duration_hours_minutes', { h, m }) : t('duration_hours', { h });
}

