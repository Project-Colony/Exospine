// Exospine — Tasks overlay (email-to-task feature)

import * as api from '../api.js';
import { showToast } from '../components/toast.js';

let _currentFilter = 'all'; // 'all' | 'active' | 'completed'

/**
 * Open the Tasks overlay.
 * @param {object} opts  { onSelectMail?: (mailId) => void }
 */
export function openTasks(opts = {}) {
  // Remove existing overlay
  const existing = document.getElementById('tasks-overlay');
  if (existing) existing.remove();

  const overlay = document.createElement('div');
  overlay.id = 'tasks-overlay';
  overlay.className = 'overlay';
  overlay.innerHTML = `
    <div class="overlay-content tasks-overlay-content">
      <div class="overlay-header">
        <h2>Tasks</h2>
        <button class="overlay-close" id="tasks-close" aria-label="Close">&times;</button>
      </div>
      <div class="tasks-filters">
        <button class="tasks-filter-btn${_currentFilter === 'all' ? ' active' : ''}" data-filter="all">All</button>
        <button class="tasks-filter-btn${_currentFilter === 'active' ? ' active' : ''}" data-filter="active">Active</button>
        <button class="tasks-filter-btn${_currentFilter === 'completed' ? ' active' : ''}" data-filter="completed">Completed</button>
        <button class="tasks-action-btn" id="tasks-delete-completed" title="Delete completed">Delete completed</button>
      </div>
      <div class="tasks-list" id="tasks-list">
        <div class="tasks-loading">Loading tasks...</div>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);

  // Close button
  document.getElementById('tasks-close').addEventListener('click', () => overlay.remove());

  // Close on overlay background click
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) overlay.remove();
  });

  // Filter buttons
  overlay.querySelectorAll('.tasks-filter-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      _currentFilter = btn.dataset.filter;
      overlay.querySelectorAll('.tasks-filter-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      loadAndRenderTasks(overlay, opts);
    });
  });

  // Delete completed
  document.getElementById('tasks-delete-completed').addEventListener('click', async () => {
    try {
      const tasks = await api.getTasks();
      const completed = tasks.filter(t => t.completed);
      for (const task of completed) {
        await api.deleteTask(task.id);
      }
      showToast(`Deleted ${completed.length} completed task(s).`, 'success');
      loadAndRenderTasks(overlay, opts);
    } catch (err) {
      showToast('Failed to delete tasks: ' + err, 'error');
    }
  });

  loadAndRenderTasks(overlay, opts);
}

async function loadAndRenderTasks(overlay, opts) {
  const listEl = overlay.querySelector('#tasks-list');
  try {
    let tasks = await api.getTasks();

    // Apply filter
    if (_currentFilter === 'active') {
      tasks = tasks.filter(t => !t.completed);
    } else if (_currentFilter === 'completed') {
      tasks = tasks.filter(t => t.completed);
    }

    if (tasks.length === 0) {
      listEl.innerHTML = '<div class="tasks-empty">No tasks.</div>';
      return;
    }

    let html = '';
    for (const task of tasks) {
      const checkedAttr = task.completed ? 'checked' : '';
      const completedClass = task.completed ? ' task-completed' : '';
      const dueStr = task.due_date ? `<span class="task-due">Due: ${esc(task.due_date)}</span>` : '';
      const linkedMail = task.mail_id ? `<button class="task-link-btn" data-mail-id="${esc(task.mail_id)}" title="Open linked email">Open email</button>` : '';
      html += `
        <div class="task-item${completedClass}" data-task-id="${esc(task.id)}">
          <label class="task-checkbox-label">
            <input type="checkbox" class="task-checkbox" data-task-id="${esc(task.id)}" ${checkedAttr} />
          </label>
          <div class="task-info">
            <div class="task-title">${esc(task.title)}</div>
            ${task.description ? `<div class="task-description">${esc(task.description)}</div>` : ''}
            <div class="task-meta">
              ${dueStr}
              ${linkedMail}
            </div>
          </div>
          <button class="task-delete-btn" data-task-id="${esc(task.id)}" title="Delete task">&times;</button>
        </div>
      `;
    }
    listEl.innerHTML = html;

    // Checkbox toggle
    listEl.querySelectorAll('.task-checkbox').forEach(cb => {
      cb.addEventListener('change', async () => {
        const taskId = cb.dataset.taskId;
        try {
          if (cb.checked) {
            await api.completeTask(taskId);
            showToast('Task completed.', 'success');
          }
          // Reload to reflect state
          loadAndRenderTasks(overlay, opts);
        } catch (err) {
          showToast('Failed to update task: ' + err, 'error');
        }
      });
    });

    // Delete button
    listEl.querySelectorAll('.task-delete-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const taskId = btn.dataset.taskId;
        try {
          await api.deleteTask(taskId);
          showToast('Task deleted.', 'success');
          loadAndRenderTasks(overlay, opts);
        } catch (err) {
          showToast('Failed to delete task: ' + err, 'error');
        }
      });
    });

    // Link to email
    listEl.querySelectorAll('.task-link-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const mailId = btn.dataset.mailId;
        if (opts.onSelectMail) {
          opts.onSelectMail(mailId);
          overlay.remove();
        }
      });
    });

  } catch (err) {
    listEl.innerHTML = `<div class="tasks-error">Failed to load tasks: ${esc(String(err))}</div>`;
  }
}

/**
 * Open the Awaiting Reply overlay showing all unresolved followups.
 * @param {object} opts  { onSelectMail?: (mailId) => void }
 */
export function openAwaitingReply(opts = {}) {
  const existing = document.getElementById('followups-overlay');
  if (existing) existing.remove();

  const overlay = document.createElement('div');
  overlay.id = 'followups-overlay';
  overlay.className = 'overlay';
  overlay.innerHTML = `
    <div class="overlay-content tasks-overlay-content">
      <div class="overlay-header">
        <h2>Awaiting Reply</h2>
        <button class="overlay-close" id="followups-close" aria-label="Close">&times;</button>
      </div>
      <div class="tasks-list" id="followups-list">
        <div class="tasks-loading">Loading...</div>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);

  document.getElementById('followups-close').addEventListener('click', () => overlay.remove());
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) overlay.remove();
  });

  loadAndRenderFollowups(overlay, opts);
}

async function loadAndRenderFollowups(overlay, opts) {
  const listEl = overlay.querySelector('#followups-list');
  try {
    const followups = await api.getFollowups();
    const unresolved = followups.filter(f => !f.resolved);

    if (unresolved.length === 0) {
      listEl.innerHTML = '<div class="tasks-empty">No pending follow-ups.</div>';
      return;
    }

    let html = '';
    for (const f of unresolved) {
      const overdue = f.due_date && new Date(f.due_date) < new Date();
      const overdueClass = overdue ? ' task-overdue' : '';
      html += `
        <div class="task-item${overdueClass}" data-mail-id="${esc(f.mail_id)}">
          <div class="task-info">
            <div class="task-title">Waiting for reply from: ${esc(f.expected_from)}</div>
            <div class="task-meta">
              <span class="task-due">${f.due_date ? 'Due: ' + esc(f.due_date) : 'No due date'}</span>
              ${overdue ? '<span class="task-overdue-badge">OVERDUE</span>' : ''}
              <button class="task-link-btn" data-mail-id="${esc(f.mail_id)}" title="Open email">Open email</button>
            </div>
          </div>
          <div class="followup-actions">
            <button class="task-action-btn followup-resolve-btn" data-mail-id="${esc(f.mail_id)}" title="Mark as resolved">Resolve</button>
            <button class="task-delete-btn" data-mail-id="${esc(f.mail_id)}" title="Delete">&times;</button>
          </div>
        </div>
      `;
    }
    listEl.innerHTML = html;

    // Resolve button
    listEl.querySelectorAll('.followup-resolve-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        try {
          await api.resolveFollowup(btn.dataset.mailId);
          showToast('Follow-up resolved.', 'success');
          loadAndRenderFollowups(overlay, opts);
        } catch (err) {
          showToast('Failed to resolve: ' + err, 'error');
        }
      });
    });

    // Delete button
    listEl.querySelectorAll('.task-delete-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        try {
          await api.deleteFollowup(btn.dataset.mailId);
          showToast('Follow-up removed.', 'success');
          loadAndRenderFollowups(overlay, opts);
        } catch (err) {
          showToast('Failed to delete: ' + err, 'error');
        }
      });
    });

    // Open email
    listEl.querySelectorAll('.task-link-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        if (opts.onSelectMail) {
          opts.onSelectMail(btn.dataset.mailId);
          overlay.remove();
        }
      });
    });

  } catch (err) {
    listEl.innerHTML = `<div class="tasks-error">Failed to load follow-ups: ${esc(String(err))}</div>`;
  }
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML;
}
