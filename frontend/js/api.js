// Exospine — Tauri invoke API wrapper

// Wait for Tauri IPC to be available
function getInvoke() {
  if (window.__TAURI__ && window.__TAURI__.core) return window.__TAURI__.core.invoke;
  if (window.__TAURI__ && window.__TAURI__.invoke) return window.__TAURI__.invoke;
  if (window.__TAURI_INTERNALS__) return window.__TAURI_INTERNALS__.invoke;
  throw new Error('Tauri IPC not available');
}
const invoke = (...args) => getInvoke()(...args);

export const getAccounts = () => invoke('get_accounts');

export const getMails = (accountId, folder, page, perPage) =>
  invoke('get_mails', { accountId, folder, page: page || 0, perPage: perPage || 50 });

export const getMailBody = (mailId) => invoke('get_mail_body', { mailId });

/** Open one message in its own window (mail.html, query built by the caller). */
export const openMailWindow = (title, query) => invoke('open_mail_window', { title, query });

/** Open an http(s) or mailto link in the system browser or mail client. */
export const openExternal = (url) => invoke('plugin:shell|open', { path: url });

export const getFolders = (accountId) => invoke('get_folders', { accountId });

export const refreshFolder = (accountId, folder) =>
  invoke('refresh_folder', { accountId, folder });

export const syncAllMails = (accountId, folder) =>
  invoke('sync_all_mails', { accountId, folder });

export const markRead = (accountId, folder, mailUid) =>
  invoke('mark_read', { accountId, folder, mailUid });

export const markUnread = (accountId, folder, mailUid) =>
  invoke('mark_unread', { accountId, folder, mailUid });

export const toggleStar = (accountId, folder, mailUid, isStarred) =>
  invoke('toggle_star', { accountId, folder, mailUid, isStarred });

export const deleteMail = (accountId, folder, mailUid) =>
  invoke('delete_mail', { accountId, folder, mailUid });

export const archiveMail = (accountId, folder, mailUid) =>
  invoke('archive_mail', { accountId, folder, mailUid });

export const sendMail = (draft) => invoke('send_mail', { draft });

export const saveDraft = (draft) => invoke('save_draft', { draft });

export const getSignature = (accountId) => invoke('get_signature', { accountId });

export const saveSignature = (accountId, signature, signatureHtml) =>
  invoke('save_signature', { account_id: accountId, signature, signature_html: signatureHtml || null });

export const getSettings = () => invoke('get_settings');

export const saveSettings = (settings) => invoke('save_settings', { settings });

export const startOauth = (provider) => invoke('start_oauth', { provider });

export const addOauthAccount = (params) => invoke('add_oauth_account', { params });

export const addBasicAccount = (params) => invoke('add_account', { params });

export const detectProvider = (email) => invoke('detect_provider', { email });

export const removeAccount = (accountId) => invoke('remove_account', { accountId });

export const searchLocal = (query, accountId, folder, filterFrom = null, filterSubject = null, filterTo = null, filterHasAttachment = false) =>
  invoke('search_local', { query, accountId, folder, filterFrom, filterSubject, filterTo, filterHasAttachment });

export const downloadAttachment = (accountId, folder, mailUid, partIndex) =>
  invoke('download_attachment', { accountId, folder, mailUid, partIndex });

export const getAttachmentContent = (accountId, folder, mailUid, partIndex) =>
  invoke('get_attachment_content', { accountId, folder, mailUid, partIndex });

export const moveMail = (accountId, folder, mailUid, targetFolder) =>
  invoke('move_mail', { accountId, folder, mailUid, targetFolder });

export const exportMail = (accountId, folder, mailUid) =>
  invoke('export_mail', { accountId, folder, mailUid });

export const getMailHeaders = (accountId, folder, mailUid) =>
  invoke('get_mail_headers', { accountId, folder, mailUid });

export const getSecurityLog = (count) =>
  invoke('get_security_log', { count: count || 20 });

// Category/label commands
export const addCategory = (mailId, category) =>
  invoke('add_category', { mailId, category });

export const removeCategory = (mailId, category) =>
  invoke('remove_category', { mailId, category });

// Spam commands
export const reportSpam = (mailId) => invoke('report_spam', { mailId });

export const reportNotSpam = (mailId) => invoke('report_not_spam', { mailId });

// Contact auto-complete
export const searchContacts = (query) => invoke('search_contacts', { query });

// Email rules/filters
export const getRules = () => invoke('get_rules');
export const saveRule = (rule) => invoke('save_rule', { rule });
export const deleteRule = (ruleId) => invoke('delete_rule', { ruleId });

// Pin emails
export const pinMail = (mailId) => invoke('pin_mail', { mailId });
export const unpinMail = (mailId) => invoke('unpin_mail', { mailId });

// Snooze emails
export const snoozeMail = (mailId, until) => invoke('snooze_mail', { mailId, until });
export const unsnoozeMail = (mailId) => invoke('unsnooze_mail', { mailId });
export const getDueSnoozed = () => invoke('get_due_snoozed');

// Scheduled send
export const scheduleSend = (draft, scheduledTime) =>
  invoke('schedule_send', { draft, scheduledTime });
export const getScheduledEmails = () => invoke('get_scheduled_emails');
export const cancelScheduled = (scheduleId) => invoke('cancel_scheduled', { scheduleId });
export const sendDueScheduled = () => invoke('send_due_scheduled');

// Flag follow-up
export const flagMail = (mailId, dueDate) => invoke('flag_mail', { mailId, dueDate });
export const unflagMail = (mailId) => invoke('unflag_mail', { mailId });

// Read receipt
export const sendReadReceipt = (mailId, accountId, receiptTo, originalSubject, originalFrom) =>
  invoke('send_read_receipt', { mailId, accountId, receiptTo, originalSubject, originalFrom });

// Analytics
export const getAnalytics = (accountId) => invoke('get_analytics', { accountId });

// Draft versioning
export const saveDraftVersion = (draftId, subject, body, bodyHtml) =>
  invoke('save_draft_version', { draftId, subject, body, bodyHtml });
export const loadDraftVersions = (draftId) =>
  invoke('load_draft_versions', { draftId });
export const deleteDraftVersions = (draftId) =>
  invoke('delete_draft_versions', { draftId });

// Data retention
export const cleanupOldMessages = (days) => invoke('cleanup_old_messages', { days });

// Sweep sender (delete all from sender)
export const sweepSender = (accountId, folder, senderEmail) =>
  invoke('sweep_sender', { accountId, folder, senderEmail });

// IMAP IDLE
export const startIdle = (accountId) => invoke('start_idle', { accountId });

// Contact management
export const getAllContacts = () => invoke('get_all_contacts');
export const updateContact = (email, name, phone, company, notes) =>
  invoke('update_contact', { email, name, phone, company, notes });
export const deleteContact = (email) => invoke('delete_contact', { email });

// Unread count
export const getUnreadCount = () => invoke('get_unread_count');

// Open .eml file
export const openEmlFile = (path) => invoke('open_eml_file', { path });

// Secure wipe
export const secureWipe = () => invoke('secure_wipe');

// Autostart
export const setAutostart = (enabled) => invoke('set_autostart', { enabled });

// Follow-up tracker
export const addFollowup = (mailId, expectedFrom, dueDate) =>
  invoke('add_followup', { mailId, expectedFrom, dueDate });
export const getFollowups = () => invoke('get_followups');
export const resolveFollowup = (mailId) => invoke('resolve_followup', { mailId });
export const deleteFollowup = (mailId) => invoke('delete_followup', { mailId });
export const checkFollowups = (accountId) => invoke('check_followups', { accountId });

// Tasks
export const createTask = (title, description, mailId, dueDate) =>
  invoke('create_task', { title, description, mailId: mailId || '', dueDate: dueDate || '' });
export const getTasks = () => invoke('get_tasks');
export const completeTask = (id) => invoke('complete_task', { id });
export const deleteTask = (id) => invoke('delete_task', { id });

// Notes
export const saveNote = (mailId, note) => invoke('save_note', { mailId, note });
export const getNote = (mailId) => invoke('get_note', { mailId });

// Duplicate detection
export const findDuplicates = (accountId, folder) =>
  invoke('find_duplicates', { accountId, folder });

// Mailto handler
export const getPendingMailto = () => invoke('get_pending_mailto');
