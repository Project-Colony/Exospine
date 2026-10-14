// Exospine: adding an account, and the first-run welcome.

import * as api from '../api.js';
import { showToast } from '../components/toast.js';
import { t } from '../i18n/index.js';

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
        <div class="onboarding-subtitle">${t(isFirstRun ? 'onboarding_welcome' : 'onboarding_add')}</div>

        <div class="onboarding-providers">
          <button class="onboarding-provider-btn" data-provider="google">
            <img src="img/google.svg" width="18" height="18" alt="">
            ${t('sign_in_google')}
          </button>
          <button class="onboarding-provider-btn" data-provider="microsoft">
            <img src="img/microsoft.svg" width="18" height="18" alt="">
            ${t('sign_in_microsoft')}
          </button>
        </div>

        <div class="onboarding-divider">${t('configure_manually')}</div>

        <div class="onboarding-form" id="manual-form">
          <span class="onboarding-input-label">${t('email_address')}</span>
          <input type="email" class="onboarding-input" id="ob-email" placeholder="you@example.com" />
          <span class="onboarding-input-label">${t('password_or_app_password')}</span>
          <input type="password" class="onboarding-input" id="ob-password" placeholder="${t('password')}" />

          <div id="manual-advanced" hidden>
            <span class="onboarding-input-label">${t('imap_server')}</span>
            <input type="text" class="onboarding-input onboarding-input-mb" id="ob-imap" placeholder="imap.example.com" />
            <span class="onboarding-input-label">${t('smtp_server')}</span>
            <input type="text" class="onboarding-input onboarding-input-mb" id="ob-smtp" placeholder="smtp.example.com" />
          </div>

          <div class="onboarding-actions">
            <button class="btn btn-ghost" id="ob-advanced-toggle">${t('advanced')}</button>
            <button class="btn btn-primary" id="ob-submit">${t('add_account')}</button>
          </div>
        </div>

        ${!isFirstRun ? `<button class="btn btn-ghost w-full mt-16" id="ob-cancel">${t('cancel')}</button>` : ''}
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
          hint.textContent = t('protonmail_hint');
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
      btn.textContent = t('redirecting');
      try {
        showToast(t('oauth_started'), 'info');
        // The backend keeps the tokens and returns only the new account.
        const account = await api.startOauth(provider);
        showToast(t('account_added_named', { email: account.email }), 'success');
        close();
        if (onAccountAdded) onAccountAdded();
      } catch (err) {
        showToast(t('error_oauth', { error: err }), 'error');
        btn.disabled = false;
        btn.textContent = t(provider === 'google' ? 'sign_in_google' : 'sign_in_microsoft');
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
      showToast(t('email_password_required'), 'error');
      return;
    }

    const submitBtn = overlay.querySelector('#ob-submit');
    submitBtn.disabled = true;
    submitBtn.textContent = t('adding');

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
      showToast(t('account_added'), 'success');
      close();
      if (onAccountAdded) onAccountAdded();
    } catch (err) {
      showToast(t('error_add_account', { error: err }), 'error');
      submitBtn.disabled = false;
      submitBtn.textContent = t('add_account');
    }
  });

  // Focus email field
  emailInput.focus();
}
