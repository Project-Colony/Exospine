pub mod accounts;
pub mod compose;
pub mod contacts;
pub mod rules;
pub mod settings;

// Mail sub-modules
pub mod mail_folders;
pub mod mail_read;
pub mod mail_sync;
pub mod mail_flags;
pub mod mail_move;
pub mod mail_search;
pub mod mail_utils;
pub mod mail;

// Pin/snooze sub-modules
pub mod pin;
pub mod snooze;
pub mod flags;
pub mod followup;
pub mod tasks;
pub mod schedule;
pub mod receipts;

/// Every command the web UI can invoke.
pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // Account commands
        accounts::get_accounts,
        accounts::detect_provider,
        accounts::start_oauth,
        accounts::take_accounts_problem,
        accounts::add_account,
        accounts::remove_account,
        accounts::test_connection,
        // Mail commands — folders
        mail_folders::get_folders,
        // Mail commands — reading
        mail_read::get_mails,
        mail_read::get_mail_body,
        mail_read::get_mail_headers,
        mail_read::open_eml_file,
        mail_read::open_mail_window,
        // Mail commands — sync
        mail_sync::refresh_folder,
        mail_sync::sync_all_mails,
        mail_sync::start_idle,
        // Mail commands — flags
        mail_flags::mark_read,
        mail_flags::mark_unread,
        mail_flags::toggle_star,
        mail_flags::delete_mail,
        mail_flags::archive_mail,
        // Mail commands — move
        mail_move::move_mail,
        mail_move::sweep_sender,
        // Mail commands — search
        mail_search::search_local,
        // Mail commands — utils
        mail_utils::download_attachment,
        mail_utils::export_mail,
        mail_utils::get_security_log,
        mail_utils::add_category,
        mail_utils::remove_category,
        mail_utils::report_spam,
        mail_utils::report_not_spam,
        mail_utils::get_analytics,
        mail_utils::cleanup_old_messages,
        mail_utils::save_note,
        mail_utils::get_note,
        mail_utils::find_duplicates,
        // Compose commands
        compose::send_mail,
        compose::save_draft,
        compose::save_draft_version,
        compose::load_draft_versions,
        compose::delete_draft_versions,
        // Settings commands
        settings::get_settings,
        settings::save_settings,
        // Signature commands
        settings::get_signature,
        settings::save_signature,
        // Autostart command
        settings::set_autostart,
        // Contact commands
        contacts::search_contacts,
        contacts::get_all_contacts,
        contacts::update_contact,
        contacts::delete_contact,
        contacts::get_unread_count,
        // Rules commands
        rules::get_rules,
        rules::save_rule,
        rules::delete_rule,
        // Pin commands
        pin::pin_mail,
        pin::unpin_mail,
        // Snooze commands
        snooze::snooze_mail,
        snooze::unsnooze_mail,
        snooze::get_due_snoozed,
        // Schedule commands
        schedule::schedule_send,
        schedule::get_scheduled_emails,
        schedule::cancel_scheduled,
        schedule::send_due_scheduled,
        // Flag follow-up commands
        flags::flag_mail,
        flags::unflag_mail,
        // Follow-up tracker commands
        followup::add_followup,
        followup::get_followups,
        followup::resolve_followup,
        followup::delete_followup,
        followup::check_followups,
        // Task commands
        tasks::create_task,
        tasks::get_tasks,
        tasks::complete_task,
        tasks::delete_task,
        // Read receipt command
        receipts::send_read_receipt,
        // Mailto handler
        crate::url_handlers::get_pending_mailto,
        // Secure wipe
        settings::secure_wipe,
    ]
}
