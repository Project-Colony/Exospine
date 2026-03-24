// Exospine — Calendar View

import { showToast } from '../components/toast.js';

const overlay = document.getElementById('settings-overlay');

const STORAGE_KEY = 'exospine_calendar_events';

const EVENT_COLORS = [
  '#0078d6', '#a05adc', '#dc503c', '#28a745',
  '#e69600', '#00b4a0', '#ff6b6b', '#6c5ce7',
];

function loadEvents() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]');
  } catch {
    return [];
  }
}

function saveEvents(events) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(events));
}

/**
 * Parse ICS/iCal text and extract events.
 * @param {string} icsText  Raw ICS content
 * @returns {Array} Parsed events
 */
function parseICS(icsText) {
  const events = [];
  const lines = icsText.replace(/\r\n /g, '').split(/\r?\n/);
  let inEvent = false;
  let current = {};

  for (const line of lines) {
    if (line === 'BEGIN:VEVENT') {
      inEvent = true;
      current = {};
    } else if (line === 'END:VEVENT') {
      inEvent = false;
      if (current.dtstart) {
        events.push({
          id: current.uid || crypto.randomUUID(),
          subject: current.summary || 'No Subject',
          description: current.description || '',
          date: parseICSDate(current.dtstart),
          endDate: current.dtend ? parseICSDate(current.dtend) : null,
          time: parseICSTime(current.dtstart),
          endTime: current.dtend ? parseICSTime(current.dtend) : null,
          color: EVENT_COLORS[Math.floor(Math.random() * EVENT_COLORS.length)],
          source: 'ics',
        });
      }
    } else if (inEvent) {
      const colonIdx = line.indexOf(':');
      if (colonIdx < 0) continue;
      const key = line.slice(0, colonIdx).split(';')[0].toLowerCase();
      const val = line.slice(colonIdx + 1);
      if (key === 'summary') current.summary = val;
      else if (key === 'description') current.description = val.replace(/\\n/g, '\n');
      else if (key === 'dtstart') current.dtstart = val;
      else if (key === 'dtend') current.dtend = val;
      else if (key === 'uid') current.uid = val;
    }
  }
  return events;
}

function parseICSDate(val) {
  // Format: 20260324T100000Z or 20260324
  const clean = val.replace(/[^0-9T]/g, '');
  const y = clean.slice(0, 4);
  const m = clean.slice(4, 6);
  const d = clean.slice(6, 8);
  return `${y}-${m}-${d}`;
}

function parseICSTime(val) {
  const clean = val.replace(/[^0-9T]/g, '');
  if (clean.length >= 13) {
    const h = clean.slice(9, 11);
    const mi = clean.slice(11, 13);
    return `${h}:${mi}`;
  }
  return '';
}

/**
 * Open the calendar overlay.
 */
export function openCalendar() {
  let currentYear = new Date().getFullYear();
  let currentMonth = new Date().getMonth(); // 0-indexed
  let selectedDay = null;

  overlay.hidden = false;
  render();

  function render() {
    const events = loadEvents();
    const monthName = new Date(currentYear, currentMonth, 1).toLocaleString('default', { month: 'long', year: 'numeric' });
    const firstDay = new Date(currentYear, currentMonth, 1).getDay();
    const daysInMonth = new Date(currentYear, currentMonth + 1, 0).getDate();
    const today = new Date();
    const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`;

    let calendarGrid = '';
    // Day headers
    const dayNames = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    for (const dn of dayNames) {
      calendarGrid += `<div class="calendar-day-header">${dn}</div>`;
    }
    // Empty cells before first day
    for (let i = 0; i < firstDay; i++) {
      calendarGrid += '<div class="calendar-day empty"></div>';
    }
    // Day cells
    for (let d = 1; d <= daysInMonth; d++) {
      const dateStr = `${currentYear}-${String(currentMonth + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
      const dayEvents = events.filter(ev => ev.date === dateStr);
      const isToday = dateStr === todayStr;
      const isSelected = selectedDay === d;
      let pillsHtml = '';
      for (const ev of dayEvents.slice(0, 3)) {
        pillsHtml += `<div class="calendar-event-pill" style="background:${esc(ev.color || '#0078d6')}" title="${esc(ev.subject)}">${esc(truncate(ev.subject, 12))}</div>`;
      }
      if (dayEvents.length > 3) {
        pillsHtml += `<div class="calendar-event-pill calendar-event-more">+${dayEvents.length - 3}</div>`;
      }
      calendarGrid += `
        <div class="calendar-day${isToday ? ' today' : ''}${isSelected ? ' selected' : ''}" data-day="${d}" data-date="${dateStr}">
          <span class="calendar-day-num">${d}</span>
          <div class="calendar-day-events">${pillsHtml}</div>
        </div>
      `;
    }

    // Selected day detail
    let dayDetailHtml = '';
    if (selectedDay !== null) {
      const dateStr = `${currentYear}-${String(currentMonth + 1).padStart(2, '0')}-${String(selectedDay).padStart(2, '0')}`;
      const dayEvents = events.filter(ev => ev.date === dateStr);
      dayDetailHtml = `
        <div class="calendar-day-detail">
          <div class="calendar-day-detail-title">${selectedDay} ${new Date(currentYear, currentMonth).toLocaleString('default', { month: 'long' })} ${currentYear}</div>
          ${dayEvents.length === 0 ? '<div class="calendar-no-events">No events</div>' : ''}
          ${dayEvents.map(ev => `
            <div class="calendar-event-item">
              <div class="calendar-event-color" style="background:${esc(ev.color || '#0078d6')}"></div>
              <div class="calendar-event-info">
                <div class="calendar-event-subject">${esc(ev.subject)}</div>
                ${ev.time ? `<div class="calendar-event-time">${esc(ev.time)}${ev.endTime ? ' - ' + esc(ev.endTime) : ''}</div>` : ''}
                ${ev.description ? `<div class="calendar-event-desc">${esc(ev.description)}</div>` : ''}
              </div>
              <button class="calendar-event-delete" data-delete-event="${esc(ev.id)}" title="Delete">\u00D7</button>
            </div>
          `).join('')}
          <button class="btn btn-ghost btn-sm calendar-add-event-btn" data-action="add-event">+ Add Event</button>
        </div>
      `;
    }

    // New event form
    const newEventFormHtml = `
      <div class="calendar-new-event" id="calendar-new-event-form" hidden>
        <div class="settings-section-title">New Event</div>
        <div class="settings-row">
          <label>Subject</label>
          <input type="text" id="cal-ev-subject" placeholder="Event subject" class="calendar-input" />
        </div>
        <div class="settings-row">
          <label>Date</label>
          <input type="date" id="cal-ev-date" class="calendar-input" />
        </div>
        <div class="settings-row">
          <label>Time</label>
          <input type="time" id="cal-ev-time" class="calendar-input" />
        </div>
        <div class="settings-row">
          <label>End Time</label>
          <input type="time" id="cal-ev-endtime" class="calendar-input" />
        </div>
        <div class="settings-row">
          <label>Description</label>
          <textarea id="cal-ev-desc" rows="3" class="calendar-input" placeholder="Optional description"></textarea>
        </div>
        <div style="display:flex;gap:6px;margin-top:8px;">
          <button class="btn btn-primary btn-sm" id="cal-ev-save">Save</button>
          <button class="btn btn-ghost btn-sm" id="cal-ev-cancel">Cancel</button>
        </div>
      </div>
    `;

    overlay.innerHTML = `
      <div class="overlay-panel settings-panel calendar-panel">
        <div class="settings-header">
          <span class="settings-title">Calendar</span>
          <button class="compose-close" id="calendar-close" title="Close">\u00D7</button>
        </div>
        <div class="settings-body" style="padding:16px;overflow-y:auto;">
          <div class="calendar-nav">
            <button class="btn btn-ghost btn-sm" id="cal-prev">\u276E</button>
            <span class="calendar-month-title">${esc(monthName)}</span>
            <button class="btn btn-ghost btn-sm" id="cal-next">\u276F</button>
            <button class="btn btn-ghost btn-sm" id="cal-today" style="margin-left:auto;">Today</button>
            <button class="btn btn-ghost btn-sm" id="cal-import-ics" title="Import ICS file">Import ICS</button>
          </div>
          <div class="calendar-grid">
            ${calendarGrid}
          </div>
          ${dayDetailHtml}
          ${newEventFormHtml}
        </div>
        <div class="settings-footer">
          <button class="btn btn-ghost" id="calendar-done">Close</button>
        </div>
      </div>
    `;

    // Wire events
    const close = () => {
      overlay.hidden = true;
      overlay.innerHTML = '';
    };

    overlay.querySelector('#calendar-close').addEventListener('click', close);
    overlay.querySelector('#calendar-done').addEventListener('click', close);
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

    // Month navigation
    overlay.querySelector('#cal-prev').addEventListener('click', () => {
      currentMonth--;
      if (currentMonth < 0) { currentMonth = 11; currentYear--; }
      selectedDay = null;
      render();
    });
    overlay.querySelector('#cal-next').addEventListener('click', () => {
      currentMonth++;
      if (currentMonth > 11) { currentMonth = 0; currentYear++; }
      selectedDay = null;
      render();
    });
    overlay.querySelector('#cal-today').addEventListener('click', () => {
      currentYear = new Date().getFullYear();
      currentMonth = new Date().getMonth();
      selectedDay = new Date().getDate();
      render();
    });

    // Day click
    overlay.querySelectorAll('.calendar-day[data-day]').forEach(cell => {
      cell.addEventListener('click', () => {
        selectedDay = parseInt(cell.dataset.day, 10);
        render();
      });
    });

    // Delete event
    overlay.querySelectorAll('[data-delete-event]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const evId = btn.dataset.deleteEvent;
        const evts = loadEvents().filter(ev => ev.id !== evId);
        saveEvents(evts);
        showToast('Event deleted.', 'info');
        render();
      });
    });

    // Add event button
    const addEventBtn = overlay.querySelector('[data-action="add-event"]');
    if (addEventBtn) {
      addEventBtn.addEventListener('click', () => {
        const form = overlay.querySelector('#calendar-new-event-form');
        if (form) {
          form.hidden = false;
          const dateStr = `${currentYear}-${String(currentMonth + 1).padStart(2, '0')}-${String(selectedDay).padStart(2, '0')}`;
          const dateInput = overlay.querySelector('#cal-ev-date');
          if (dateInput) dateInput.value = dateStr;
        }
      });
    }

    // Save event
    const saveBtn = overlay.querySelector('#cal-ev-save');
    if (saveBtn) {
      saveBtn.addEventListener('click', () => {
        const subject = overlay.querySelector('#cal-ev-subject').value.trim();
        const date = overlay.querySelector('#cal-ev-date').value;
        const time = overlay.querySelector('#cal-ev-time').value;
        const endTime = overlay.querySelector('#cal-ev-endtime').value;
        const desc = overlay.querySelector('#cal-ev-desc').value.trim();

        if (!subject || !date) {
          showToast('Subject and date are required.', 'error');
          return;
        }

        const evts = loadEvents();
        evts.push({
          id: crypto.randomUUID(),
          subject,
          date,
          time: time || '',
          endTime: endTime || '',
          description: desc,
          color: EVENT_COLORS[evts.length % EVENT_COLORS.length],
          source: 'manual',
        });
        saveEvents(evts);
        showToast('Event created.', 'success');
        render();
      });
    }

    // Cancel event form
    const cancelBtn = overlay.querySelector('#cal-ev-cancel');
    if (cancelBtn) {
      cancelBtn.addEventListener('click', () => {
        const form = overlay.querySelector('#calendar-new-event-form');
        if (form) form.hidden = true;
      });
    }

    // Import ICS
    overlay.querySelector('#cal-import-ics').addEventListener('click', () => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.ics,.ical,text/calendar';
      input.addEventListener('change', () => {
        const file = input.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = () => {
          const parsed = parseICS(reader.result);
          if (parsed.length === 0) {
            showToast('No events found in ICS file.', 'error');
            return;
          }
          const evts = loadEvents();
          evts.push(...parsed);
          saveEvents(evts);
          showToast(`Imported ${parsed.length} event(s).`, 'success');
          render();
        };
        reader.readAsText(file);
      });
      input.click();
    });
  }
}

/**
 * Try to parse ICS invitations from an email body.
 * Called externally when viewing an email with text/calendar parts.
 * @param {string} icsContent
 */
export function importICSFromEmail(icsContent) {
  const parsed = parseICS(icsContent);
  if (parsed.length === 0) return 0;
  const evts = loadEvents();
  // Deduplicate by ID
  const existingIds = new Set(evts.map(e => e.id));
  const newEvents = parsed.filter(e => !existingIds.has(e.id));
  if (newEvents.length === 0) return 0;
  evts.push(...newEvents);
  saveEvents(evts);
  return newEvents.length;
}

function truncate(str, max) {
  if (!str) return '';
  return str.length > max ? str.slice(0, max) + '...' : str;
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
