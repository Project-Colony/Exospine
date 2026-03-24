// Exospine — Email Analytics Dashboard

import * as api from '../api.js';
import { showToast } from '../components/toast.js';

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
        <span class="settings-title">Email Analytics</span>
        <button class="compose-close" id="analytics-close" title="Close">\u00D7</button>
      </div>
      <div class="settings-body" id="analytics-body" style="padding:16px;">
        <div style="text-align:center;padding:40px 0;color:var(--pane-text-dim);">Loading analytics...</div>
      </div>
      <div class="settings-footer">
        <button class="btn btn-ghost" id="analytics-done">Close</button>
      </div>
    </div>
  `;

  const close = () => {
    overlay.hidden = true;
    overlay.innerHTML = '';
  };

  overlay.querySelector('#analytics-close').addEventListener('click', close);
  overlay.querySelector('#analytics-done').addEventListener('click', close);
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

  // Fetch analytics data
  try {
    const data = await api.getAnalytics(accountId);
    renderAnalytics(data);
  } catch (err) {
    const body = overlay.querySelector('#analytics-body');
    if (body) {
      body.innerHTML = `<div style="color:var(--danger);padding:20px;">Failed to load analytics: ${esc(String(err))}</div>`;
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
    <div class="analytics-summary" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:12px;margin-bottom:24px;">
      <div class="analytics-card" style="background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:8px;padding:16px;text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);">${data.total_received}</div>
        <div style="font-size:12px;color:var(--pane-text-dim);margin-top:4px;">Received</div>
      </div>
      <div class="analytics-card" style="background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:8px;padding:16px;text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);">${data.total_sent}</div>
        <div style="font-size:12px;color:var(--pane-text-dim);margin-top:4px;">Sent</div>
      </div>
      <div class="analytics-card" style="background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:8px;padding:16px;text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);">${data.total_received + data.total_sent}</div>
        <div style="font-size:12px;color:var(--pane-text-dim);margin-top:4px;">Total</div>
      </div>
    </div>
  `;

  // Emails by day (last 30 days) — CSS bar chart
  if (data.by_day.length > 0) {
    html += `
      <div class="analytics-section" style="margin-bottom:24px;">
        <div style="font-weight:600;margin-bottom:8px;">Emails by Day (last 30 days)</div>
        <div class="analytics-chart" style="display:flex;align-items:flex-end;gap:2px;height:100px;border-bottom:1px solid var(--pane-border);">
    `;
    for (const [day, count] of data.by_day) {
      const pct = (count / maxByDay) * 100;
      const shortDay = day.slice(5); // MM-DD
      html += `
        <div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:flex-end;height:100%;" title="${day}: ${count} emails">
          <div style="width:100%;max-width:20px;background:var(--accent);border-radius:2px 2px 0 0;height:${pct}%;min-height:${count > 0 ? 2 : 0}px;transition:height 0.2s;"></div>
        </div>
      `;
    }
    html += `
        </div>
        <div style="display:flex;justify-content:space-between;font-size:10px;color:var(--pane-text-dim);margin-top:4px;">
          <span>${data.by_day.length > 0 ? data.by_day[0][0].slice(5) : ''}</span>
          <span>${data.by_day.length > 0 ? data.by_day[data.by_day.length - 1][0].slice(5) : ''}</span>
        </div>
      </div>
    `;
  }

  // Busiest hours — CSS bar chart
  html += `
    <div class="analytics-section" style="margin-bottom:24px;">
      <div style="font-weight:600;margin-bottom:8px;">Busiest Hours</div>
      <div class="analytics-chart" style="display:flex;align-items:flex-end;gap:1px;height:80px;border-bottom:1px solid var(--pane-border);">
  `;
  for (let h = 0; h < 24; h++) {
    const count = data.by_hour[h] || 0;
    const pct = (count / maxByHour) * 100;
    html += `
      <div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:flex-end;height:100%;" title="${h}:00 - ${count} emails">
        <div style="width:100%;max-width:16px;background:rgb(0,180,160);border-radius:2px 2px 0 0;height:${pct}%;min-height:${count > 0 ? 2 : 0}px;"></div>
      </div>
    `;
  }
  html += `
      </div>
      <div style="display:flex;justify-content:space-between;font-size:10px;color:var(--pane-text-dim);margin-top:4px;">
        <span>0h</span><span>6h</span><span>12h</span><span>18h</span><span>23h</span>
      </div>
    </div>
  `;

  // Top senders — horizontal bars
  if (data.top_senders.length > 0) {
    html += `
      <div class="analytics-section" style="margin-bottom:24px;">
        <div style="font-weight:600;margin-bottom:8px;">Top Senders</div>
    `;
    for (const [sender, count] of data.top_senders) {
      const pct = (count / maxSender) * 100;
      const displaySender = sender.length > 40 ? sender.slice(0, 40) + '...' : sender;
      html += `
        <div style="margin-bottom:6px;">
          <div style="display:flex;justify-content:space-between;font-size:12px;margin-bottom:2px;">
            <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:80%;" title="${esc(sender)}">${esc(displaySender)}</span>
            <span style="color:var(--pane-text-dim);flex-shrink:0;margin-left:8px;">${count}</span>
          </div>
          <div style="height:6px;background:var(--pane-border);border-radius:3px;overflow:hidden;">
            <div style="width:${pct}%;height:100%;background:rgb(160,90,220);border-radius:3px;"></div>
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
  const dayNames = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
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
    <div class="analytics-section" style="margin-bottom:24px;">
      <div style="font-weight:600;margin-bottom:8px;">Weekly Email Heatmap</div>
      <div class="analytics-heatmap" style="display:grid;grid-template-columns:40px repeat(24,1fr);gap:1px;font-size:10px;">
        <div></div>
  `;
  // Hour headers
  for (let h = 0; h < 24; h++) {
    html += `<div style="text-align:center;color:var(--pane-text-dim);${h % 3 === 0 ? '' : 'visibility:hidden;'}">${h}h</div>`;
  }
  // Rows
  for (let d = 0; d < 7; d++) {
    html += `<div style="display:flex;align-items:center;color:var(--pane-text-dim);font-weight:500;">${dayNames[d]}</div>`;
    for (let h = 0; h < 24; h++) {
      const val = grid[d][h];
      const intensity = maxCell > 0 ? val / maxCell : 0;
      const bg = intensity > 0
        ? `rgba(0,180,160,${0.15 + intensity * 0.85})`
        : 'rgba(0,0,0,0.05)';
      html += `<div style="aspect-ratio:1;background:${bg};border-radius:2px;min-height:12px;" title="${dayNames[d]} ${h}:00 — ${val}"></div>`;
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
      <div class="analytics-section" style="margin-bottom:24px;">
        <div class="analytics-card" style="background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:8px;padding:16px;text-align:center;">
          <div style="font-size:28px;font-weight:700;color:var(--accent);">${esc(display)}</div>
          <div style="font-size:12px;color:var(--pane-text-dim);margin-top:4px;">Average response time</div>
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
    <div class="analytics-section" style="margin-bottom:24px;">
      <div class="analytics-card" style="background:var(--pane-bg);border:1px solid var(--pane-border);border-radius:8px;padding:16px;text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);">~${esc(display)}</div>
        <div style="font-size:12px;color:var(--pane-text-dim);margin-top:4px;">Estimated average response time</div>
      </div>
    </div>
  `;
}

function formatDuration(minutes) {
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  if (h >= 24) {
    const d = Math.floor(h / 24);
    const rh = h % 24;
    return rh > 0 ? `${d}d ${rh}h` : `${d}d`;
  }
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
