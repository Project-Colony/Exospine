// Exospine — Notification sound, desktop notifications, favicon, unread title

import * as api from './api.js';

// ---------------------------------------------------------------------------
// Notification sound
// ---------------------------------------------------------------------------
let _soundEnabled = localStorage.getItem('exospine_sound_notifications') !== 'false';

export function setSoundEnabled(enabled) {
  _soundEnabled = enabled;
}

export function isSoundEnabled() {
  return _soundEnabled;
}

export function playNotificationSound(accountId) {
  if (!_soundEnabled) return;
  // Per-account override, then global setting, then default
  let sound;
  if (accountId) {
    const acctSound = localStorage.getItem('exospine_notif_sound_' + accountId);
    sound = acctSound || localStorage.getItem('exospine_notif_sound') || 'default';
  } else {
    sound = localStorage.getItem('exospine_notif_sound') || 'default';
  }
  if (sound === 'silent') return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (sound === 'chime') {
      // 660Hz then 880Hz sequence
      const osc1 = ctx.createOscillator();
      const gain1 = ctx.createGain();
      osc1.connect(gain1);
      gain1.connect(ctx.destination);
      osc1.frequency.value = 660;
      osc1.type = 'sine';
      gain1.gain.value = 0.3;
      osc1.start();
      gain1.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.2);
      osc1.stop(ctx.currentTime + 0.2);

      const osc2 = ctx.createOscillator();
      const gain2 = ctx.createGain();
      osc2.connect(gain2);
      gain2.connect(ctx.destination);
      osc2.frequency.value = 880;
      osc2.type = 'sine';
      gain2.gain.value = 0.3;
      osc2.start(ctx.currentTime + 0.2);
      gain2.gain.setValueAtTime(0.3, ctx.currentTime + 0.2);
      gain2.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc2.stop(ctx.currentTime + 0.5);
    } else if (sound === 'bell') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 440;
      osc.type = 'triangle';
      gain.gain.value = 0.4;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc.stop(ctx.currentTime + 0.5);
    } else if (sound === 'gentle') {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 523;
      osc.type = 'sine';
      gain.gain.value = 0.15;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.4);
      osc.stop(ctx.currentTime + 0.4);
    } else {
      // Default: 880Hz sine
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 880;
      osc.type = 'sine';
      gain.gain.value = 0.3;
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
      osc.stop(ctx.currentTime + 0.3);
    }
  } catch {}
}

// ---------------------------------------------------------------------------
// Desktop notifications
// ---------------------------------------------------------------------------
export function showDesktopNotification(title, body) {
  try {
    if (Notification.permission === 'granted') {
      new Notification(title, { body, icon: '' });
    } else if (Notification.permission !== 'denied') {
      Notification.requestPermission().then(p => {
        if (p === 'granted') new Notification(title, { body, icon: '' });
      });
    }
  } catch {}
}

// ---------------------------------------------------------------------------
// Dynamic favicon with unread badge
// ---------------------------------------------------------------------------
export function updateFavicon(unreadCount) {
  const canvas = document.createElement('canvas');
  canvas.width = 32;
  canvas.height = 32;
  const ctx = canvas.getContext('2d');

  // Draw base icon (simple envelope shape)
  ctx.fillStyle = '#00bcd4';
  ctx.fillRect(2, 6, 28, 20);
  ctx.fillStyle = '#00838f';
  ctx.beginPath();
  ctx.moveTo(2, 6);
  ctx.lineTo(16, 18);
  ctx.lineTo(30, 6);
  ctx.closePath();
  ctx.fill();

  // Draw unread badge if count > 0
  if (unreadCount > 0) {
    const text = unreadCount > 99 ? '99+' : String(unreadCount);
    const badgeRadius = text.length > 2 ? 10 : 8;
    const badgeX = 32 - badgeRadius;
    const badgeY = badgeRadius;

    ctx.beginPath();
    ctx.arc(badgeX, badgeY, badgeRadius, 0, 2 * Math.PI);
    ctx.fillStyle = '#e53935';
    ctx.fill();
    ctx.strokeStyle = '#1a1a2e';
    ctx.lineWidth = 1;
    ctx.stroke();

    ctx.fillStyle = '#fff';
    ctx.font = `bold ${text.length > 2 ? 8 : 10}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, badgeX, badgeY + 1);
  }

  // Apply as favicon
  let link = document.querySelector('link[rel="icon"]');
  if (!link) {
    link = document.createElement('link');
    link.rel = 'icon';
    document.head.appendChild(link);
  }
  link.href = canvas.toDataURL('image/png');
}

// ---------------------------------------------------------------------------
// Update document title with unread count
// ---------------------------------------------------------------------------
export async function updateUnreadTitle() {
  try {
    const count = await api.getUnreadCount();
    document.title = count > 0 ? `Exospine (${count} unread)` : 'Exospine';
    updateFavicon(count);
  } catch {
    // Silent fail
  }
}
