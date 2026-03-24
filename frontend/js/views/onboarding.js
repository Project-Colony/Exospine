// Exospine — Onboarding / Add Account view

import * as api from '../api.js';
import { showToast } from '../components/toast.js';

const overlay = document.getElementById('onboarding-overlay');

/**
 * Show the onboarding / add-account screen.
 * @param {object} opts
 * @param {boolean} [opts.isFirstRun=false]  true if no accounts exist
 * @param {function} opts.onAccountAdded     callback after account is added
 */
export function openOnboarding({ isFirstRun = false, onAccountAdded }) {
  overlay.hidden = false;

  overlay.innerHTML = `
    <div class="overlay-panel onboarding-panel">
      <div class="onboarding-body">
        <div class="onboarding-logo">Exospine</div>
        <div class="onboarding-subtitle">${isFirstRun ? 'Welcome. Add your first email account to get started.' : 'Add a new email account.'}</div>

        <div class="onboarding-providers">
          <button class="onboarding-provider-btn" data-provider="google">
            <svg width="18" height="18" viewBox="0 0 24 24"><path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"/><path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/><path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/><path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/></svg>
            Sign in with Google
          </button>
          <button class="onboarding-provider-btn" data-provider="microsoft">
            <svg width="18" height="18" viewBox="0 0 24 24"><rect fill="#F25022" x="1" y="1" width="10" height="10"/><rect fill="#7FBA00" x="13" y="1" width="10" height="10"/><rect fill="#00A4EF" x="1" y="13" width="10" height="10"/><rect fill="#FFB900" x="13" y="13" width="10" height="10"/></svg>
            Sign in with Microsoft
          </button>
        </div>

        <div class="onboarding-divider">or configure manually</div>

        <div class="onboarding-form" id="manual-form">
          <span class="onboarding-input-label">Email address</span>
          <input type="email" class="onboarding-input" id="ob-email" placeholder="you@example.com" />
          <span class="onboarding-input-label">Password / App password</span>
          <input type="password" class="onboarding-input" id="ob-password" placeholder="Password" />

          <div id="manual-advanced" hidden>
            <span class="onboarding-input-label">IMAP server</span>
            <input type="text" class="onboarding-input" id="ob-imap" placeholder="imap.example.com" style="margin-bottom:10px;" />
            <span class="onboarding-input-label">SMTP server</span>
            <input type="text" class="onboarding-input" id="ob-smtp" placeholder="smtp.example.com" style="margin-bottom:10px;" />
          </div>

          <div style="display:flex;gap:8px;margin-top:6px;">
            <button class="btn btn-ghost" style="flex:1;" id="ob-advanced-toggle">Advanced</button>
            <button class="btn btn-primary" style="flex:1;" id="ob-submit">Add Account</button>
          </div>
        </div>

        ${!isFirstRun ? '<button class="btn btn-ghost" id="ob-cancel" style="margin-top:14px;width:100%;">Cancel</button>' : ''}
      </div>
    </div>
  `;

  let onKey = null;

  const close = () => {
    if (onKey) {
      document.removeEventListener('keydown', onKey);
      onKey = null;
    }
    overlay.hidden = true;
    overlay.innerHTML = '';
  };

  // Cancel
  const cancelBtn = overlay.querySelector('#ob-cancel');
  if (cancelBtn) cancelBtn.addEventListener('click', close);

  if (!isFirstRun) {
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) close();
    });
    onKey = (e) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('keydown', onKey);
  }

  // Advanced toggle
  const advancedDiv = overlay.querySelector('#manual-advanced');
  overlay.querySelector('#ob-advanced-toggle').addEventListener('click', () => {
    advancedDiv.hidden = !advancedDiv.hidden;
  });

  // Auto-detect on email blur
  const emailInput = overlay.querySelector('#ob-email');
  emailInput.addEventListener('blur', async () => {
    const email = emailInput.value.trim();
    if (!email) return;
    try {
      const info = await api.detectProvider(email);
      if (info && info.imap_host) {
        const imapInput = overlay.querySelector('#ob-imap');
        const smtpInput = overlay.querySelector('#ob-smtp');
        if (imapInput && !imapInput.value) imapInput.value = info.imap_host;
        if (smtpInput && !smtpInput.value) smtpInput.value = info.smtp_host || '';
      }
      // Show ProtonMail Bridge hint if provider is ProtonMail
      const hintEl = overlay.querySelector('#protonmail-hint');
      if (info && info.provider_name === 'ProtonMail') {
        if (!hintEl) {
          const hint = document.createElement('div');
          hint.id = 'protonmail-hint';
          hint.className = 'onboarding-hint';
          hint.textContent = 'ProtonMail requires Proton Bridge running locally. Make sure it is started before adding your account.';
          const form = overlay.querySelector('#manual-form');
          if (form) form.insertBefore(hint, form.firstChild);
        }
      } else if (hintEl) {
        hintEl.remove();
      }
    } catch {
      // Ignore detection failures
    }
  });

  // OAuth providers
  overlay.querySelectorAll('[data-provider]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const provider = btn.dataset.provider;
      btn.disabled = true;
      btn.textContent = 'Redirecting\u2026';
      try {
        showToast('OAuth flow started. Complete it in the browser.', 'info');
        const result = await api.startOauth(provider);
        // startOauth returns { access_token, refresh_token, email, name }
        if (result && result.email) {
          // Add the OAuth account
          // Detect IMAP/SMTP settings from provider
          const providerInfo = await api.detectProvider(result.email);
          await api.addOauthAccount({
            access_token: result.access_token,
            refresh_token: result.refresh_token,
            email: result.email,
            name: result.name,
            imap_host: providerInfo?.imap_host || 'imap.gmail.com',
            imap_port: providerInfo?.imap_port || 993,
            smtp_host: providerInfo?.smtp_host || 'smtp.gmail.com',
            smtp_port: providerInfo?.smtp_port || 587,
          });
          showToast(`Account ${result.email} added!`, 'success');
          close();
          if (onAccountAdded) onAccountAdded();
        }
      } catch (err) {
        showToast(`OAuth failed: ${err}`, 'error');
        btn.disabled = false;
        btn.textContent = provider === 'google' ? 'Sign in with Google' : 'Sign in with Microsoft';
      }
    });
  });

  // Manual submit
  overlay.querySelector('#ob-submit').addEventListener('click', async () => {
    const email = overlay.querySelector('#ob-email').value.trim();
    const password = overlay.querySelector('#ob-password').value;
    const imapHost = overlay.querySelector('#ob-imap').value.trim();
    const smtpHost = overlay.querySelector('#ob-smtp').value.trim();

    if (!email || !password) {
      showToast('Email and password are required.', 'error');
      return;
    }

    const submitBtn = overlay.querySelector('#ob-submit');
    submitBtn.disabled = true;
    submitBtn.textContent = 'Adding\u2026';

    try {
      // Detect provider settings for IMAP/SMTP if not manually specified
      let detectedImap = imapHost;
      let detectedSmtp = smtpHost;
      let detectedImapPort = 993;
      let detectedSmtpPort = 587;
      if (!detectedImap || !detectedSmtp) {
        try {
          const info = await api.detectProvider(email);
          if (info) {
            if (!detectedImap) detectedImap = info.imap_host;
            if (!detectedSmtp) detectedSmtp = info.smtp_host;
            detectedImapPort = info.imap_port || 993;
            detectedSmtpPort = info.smtp_port || 587;
          }
        } catch {
          // Use defaults
        }
      }
      await api.addBasicAccount({
        name: email.split('@')[0],
        email,
        password,
        imap_host: detectedImap || 'imap.gmail.com',
        imap_port: detectedImapPort,
        smtp_host: detectedSmtp || 'smtp.gmail.com',
        smtp_port: detectedSmtpPort,
      });
      showToast('Account added successfully.', 'success');
      close();
      if (onAccountAdded) onAccountAdded();
    } catch (err) {
      showToast(`Failed to add account: ${err}`, 'error');
      submitBtn.disabled = false;
      submitBtn.textContent = 'Add Account';
    }
  });

  // Focus email field
  emailInput.focus();
}
