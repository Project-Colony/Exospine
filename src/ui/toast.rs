//! Toast notification system for Exospine.
//!
//! Displays ephemeral notifications stacked at the bottom of the screen.
//! Each toast auto-dismisses after a configurable duration and can also
//! be dismissed manually via a close button.
//!
//! **Integration note**: Add `next_toast_id: u64` and `toasts: Vec<Toast>`
//! fields to `App` state. Wire `Message::DismissToast` and
//! `Message::TickToasts` in the application `update` function.

use std::time::Instant;

use iced::widget::{button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::message::Message;
use crate::ui::theme;

// ── Toast level ────────────────────────────────────────────────────────

/// Severity / visual style of a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Success,
    Error,
    Info,
}

impl ToastLevel {
    /// Accent color for the left border indicator.
    #[inline]
    pub const fn color(self) -> Color {
        match self {
            Self::Success => Color::from_rgb(0.20, 0.78, 0.35),
            Self::Error => Color::from_rgb(0.85, 0.20, 0.20),
            Self::Info => Color::from_rgb(0.0, 0.47, 0.84),
        }
    }

    /// Short prefix label shown before the message text.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Success => "✓",
            Self::Error => "✗",
            Self::Info => "ℹ",
        }
    }
}

// ── Toast ──────────────────────────────────────────────────────────────

/// A single toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Unique identifier used to dismiss a specific toast.
    pub id: u64,
    /// Human-readable notification text.
    pub message: String,
    /// Visual severity level.
    pub level: ToastLevel,
    /// When this toast was created (for expiry calculation).
    pub created_at: Instant,
    /// Seconds before auto-dismiss.
    pub duration_secs: u64,
}

impl Toast {
    /// Default auto-dismiss duration.
    const DEFAULT_DURATION_SECS: u64 = 5;

    /// Create a new toast. The caller must supply an incrementing `id`
    /// (typically `App::next_toast_id`).
    pub fn new(id: u64, message: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            id,
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration_secs: Self::DEFAULT_DURATION_SECS,
        }
    }

    /// Create a toast with a custom duration.
    pub fn with_duration(mut self, secs: u64) -> Self {
        self.duration_secs = secs;
        self
    }

    /// Returns `true` when this toast has outlived its display duration.
    #[inline]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= self.duration_secs
    }
}

// ── View ───────────────────────────────────────────────────────────────

/// Render the toast notification stack.
///
/// Toasts are stacked vertically from the bottom of the screen. Each
/// toast has a colored left border indicating severity, the message
/// text, and a dismiss button.
pub fn view(toasts: &[Toast]) -> Element<'_, Message> {
    if toasts.is_empty() {
        return container(column![]).into();
    }

    let items = toasts.iter().map(|t| toast_row(t));
    let stack = items.fold(column![].spacing(6), |col, item| col.push(item));

    container(stack)
        .width(Length::Fixed(360.0))
        .padding(Padding::new(0.0).right(12).bottom(12))
        .into()
}

/// Render a single toast notification row.
fn toast_row(t: &Toast) -> Element<'_, Message> {
    let accent = t.level.color();

    let indicator = container(text(""))
        .width(Length::Fixed(4.0))
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(accent)),
            ..Default::default()
        });

    let label = text(t.level.label()).size(14);
    let msg = text(&t.message)
        .size(13)
        .color(theme::TEXT_PRIMARY);

    let dismiss = button(text("×").size(14).color(theme::TEXT_SECONDARY))
        .on_press(Message::DismissToast(t.id))
        .padding(Padding::from([2, 6]))
        .style(|_theme: &iced::Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: theme::TEXT_SECONDARY,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 2.0.into(),
                },
                ..Default::default()
            }
        });

    let content = row![label, msg]
        .spacing(8)
        .width(Length::Fill);

    let inner = row![indicator, content, dismiss]
        .spacing(8)
        .padding(Padding::from([8, 10]))
        .width(Length::Fill);

    container(inner)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.18, 0.22))),
            border: Border {
                color: Color::from_rgb(0.28, 0.30, 0.34),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

// ── Helpers for App state ──────────────────────────────────────────────

/// Remove expired toasts from the list. Call this from `Message::TickToasts`.
pub fn remove_expired(toasts: &mut Vec<Toast>) {
    toasts.retain(|t| !t.is_expired());
}

/// Dismiss a specific toast by id. Call this from `Message::DismissToast`.
pub fn dismiss(toasts: &mut Vec<Toast>, id: u64) {
    toasts.retain(|t| t.id != id);
}
