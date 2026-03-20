//! Reusable confirmation dialog for Exospine.
//!
//! Displays a modal overlay with a centered card containing a title,
//! message, and Cancel / Confirm action buttons. The confirm button
//! is styled as destructive (red) when `is_destructive` is set.
//!
//! **Integration note**: Add `confirm_dialog: Option<ConfirmDialog>` to
//! `App` state. Wire `Message::ShowConfirm`, `Message::ConfirmYes`,
//! and `Message::ConfirmCancel` in the application `update` function.

use iced::widget::{button, center, column, container, row, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::message::Message;
use crate::ui::theme;

// ── Action variants ────────────────────────────────────────────────────

/// Concrete actions that can be confirmed through the dialog.
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    /// Delete an account by its index in `App::accounts`.
    DeleteAccount(usize),
    /// Permanently empty the Trash folder.
    EmptyTrash,
    /// Discard the current compose draft.
    DiscardDraft,
    /// Permanently delete a mail entry by its index.
    DeleteMail(usize),
}

// ── Dialog descriptor ──────────────────────────────────────────────────

/// Full description of a confirmation dialog to present.
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    /// Title displayed at the top of the dialog card.
    pub title: String,
    /// Explanatory body text.
    pub message: String,
    /// Label on the confirm button (e.g. "Delete", "Empty Trash").
    pub confirm_label: String,
    /// Label on the cancel button.
    pub cancel_label: String,
    /// The action to perform when the user confirms.
    pub on_confirm: ConfirmAction,
    /// Whether the confirm action is destructive (styles the button red).
    pub is_destructive: bool,
}

impl ConfirmDialog {
    /// Convenience constructor for a destructive action.
    pub fn destructive(
        title: impl Into<String>,
        message: impl Into<String>,
        confirm_label: impl Into<String>,
        action: ConfirmAction,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: confirm_label.into(),
            cancel_label: "Cancel".into(),
            on_confirm: action,
            is_destructive: true,
        }
    }

    /// Convenience constructor for a non-destructive confirmation.
    pub fn normal(
        title: impl Into<String>,
        message: impl Into<String>,
        confirm_label: impl Into<String>,
        action: ConfirmAction,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: confirm_label.into(),
            cancel_label: "Cancel".into(),
            on_confirm: action,
            is_destructive: false,
        }
    }
}

// ── View ───────────────────────────────────────────────────────────────

/// Render the confirmation dialog as a full-screen modal overlay.
pub fn view(dialog: &ConfirmDialog) -> Element<'_, Message> {
    // -- Card contents --------------------------------------------------

    let title = text(&dialog.title)
        .size(18)
        .color(theme::TEXT_DARK);

    let body = text(&dialog.message)
        .size(14)
        .color(theme::TEXT_DARK_SECONDARY);

    let cancel_btn = button(
        text(&dialog.cancel_label)
            .size(14)
            .color(theme::TEXT_DARK),
    )
    .on_press(Message::ConfirmCancel)
    .padding(Padding::from([8, 20]))
    .style(|_theme: &iced::Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.90, 0.91, 0.93),
            button::Status::Pressed => Color::from_rgb(0.85, 0.86, 0.88),
            _ => Color::from_rgb(0.94, 0.94, 0.95),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: theme::TEXT_DARK,
            border: Border {
                color: Color::from_rgb(0.78, 0.78, 0.80),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    });

    let confirm_destructive = dialog.is_destructive;
    let confirm_btn = button(
        text(&dialog.confirm_label)
            .size(14)
            .color(Color::WHITE),
    )
    .on_press(Message::ConfirmYes(dialog.on_confirm.clone()))
    .padding(Padding::from([8, 20]))
    .style(move |_theme: &iced::Theme, status: button::Status| {
        if confirm_destructive {
            theme::danger_button_style(_theme, status)
        } else {
            theme::accent_button_style(_theme, status)
        }
    });

    let buttons = row![Space::new().width(Length::Fill), cancel_btn, confirm_btn]
        .spacing(10);

    let card = column![title, body, buttons]
        .spacing(16)
        .padding(Padding::from(24))
        .width(Length::Fixed(420.0));

    let card_container = container(card)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::WHITE)),
            border: Border {
                color: Color::from_rgb(0.80, 0.80, 0.82),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

    // -- Overlay backdrop -----------------------------------------------

    let centered = center(card_container)
        .width(Length::Fill)
        .height(Length::Fill);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
            ..Default::default()
        })
        .into()
}
