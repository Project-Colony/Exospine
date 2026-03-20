//! Keyboard shortcut handler for Exospine.
//!
//! Maps keyboard events to application [`Message`] variants.
//!
//! # Required Message variants (add to message.rs if missing)
//!
//! - `DeleteSelected` — delete the currently selected mail entry
//! - `FocusSearch` — move keyboard focus to the search bar
//! - `EscapePressed` — close compose/settings or clear selection
//! - `SelectNextMail` — select the next mail entry in the list
//! - `SelectPrevMail` — select the previous mail entry in the list

use iced::keyboard;

use crate::message::Message;

/// Map a keyboard event to an application message.
///
/// Returns `None` when the key combination has no binding.
pub fn handle_key_event(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    match (key.as_ref(), modifiers) {
        // Ctrl+N / Cmd+N → New mail
        (keyboard::Key::Character("n"), m) if m.command() => Some(Message::NewMail),

        // Ctrl+Shift+R / Cmd+Shift+R → Reply All (checked before Reply)
        (keyboard::Key::Character("r"), m) if m.command() && m.shift() => Some(Message::ReplyAll),

        // Ctrl+R / Cmd+R → Reply
        (keyboard::Key::Character("r"), m) if m.command() && !m.shift() => Some(Message::Reply),

        // Ctrl+F / Cmd+F → Forward
        (keyboard::Key::Character("f"), m) if m.command() => Some(Message::Forward),

        // Delete → Delete selected mail
        (keyboard::Key::Named(keyboard::key::Named::Delete), _) => {
            Some(Message::DeleteSelected)
        }

        // F5 → Refresh folder
        (keyboard::Key::Named(keyboard::key::Named::F5), _) => Some(Message::RefreshFolder),

        // Ctrl+E / Cmd+E → Focus search bar
        (keyboard::Key::Character("e"), m) if m.command() => Some(Message::FocusSearch),

        // Escape → Close compose/settings
        (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
            Some(Message::EscapePressed)
        }

        // Arrow Down → Next mail
        (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
            Some(Message::SelectNextMail)
        }

        // Arrow Up → Previous mail
        (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => {
            Some(Message::SelectPrevMail)
        }

        // Ctrl+Enter / Cmd+Enter → Send mail
        (keyboard::Key::Named(keyboard::key::Named::Enter), m) if m.command() => {
            Some(Message::SendMail)
        }

        _ => None,
    }
}
