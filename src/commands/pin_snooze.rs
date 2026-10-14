//! Tauri commands for pin, snooze, and scheduled send features.
//!
//! This module is a facade that re-exports all related commands
//! from their respective sub-modules.

#[allow(unused_imports)]
pub use super::pin::{pin_mail, unpin_mail};
#[allow(unused_imports)]
pub use super::snooze::{get_due_snoozed, snooze_mail, unsnooze_mail};
#[allow(unused_imports)]
pub use super::flags::{flag_mail, unflag_mail};
#[allow(unused_imports)]
pub use super::followup::{add_followup, check_followups, delete_followup, get_followups, resolve_followup};
#[allow(unused_imports)]
pub use super::tasks::{complete_task, create_task, delete_task, get_tasks};
#[allow(unused_imports)]
pub use super::schedule::{cancel_scheduled, get_scheduled_emails, schedule_send, send_due_scheduled};
#[allow(unused_imports)]
pub use super::receipts::send_read_receipt;
