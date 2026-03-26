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
pub mod mail; // facade re-exporting the above

// Pin/snooze sub-modules
pub mod pin;
pub mod snooze;
pub mod flags;
pub mod followup;
pub mod tasks;
pub mod schedule;
pub mod receipts;
pub mod pin_snooze; // facade re-exporting the above
