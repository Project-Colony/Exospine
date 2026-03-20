//! Minimalist text-based progress indicator for Exospine.
//!
//! Since Iced 0.14 does not ship a native progress bar widget, this
//! module provides a simple text-based alternative:
//!
//! - **Indeterminate** (`None`): displays "Loading..." with a spinner.
//! - **Determinate** (`Some(pct)`): displays a text bar like
//!   `[████░░░░░░] 42%`.

use iced::widget::{container, row, text};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::message::Message;
use crate::ui::theme;

/// Width (in characters) of the text-based progress bar.
const BAR_WIDTH: usize = 20;

/// Filled block character.
const FILLED: char = '█';

/// Empty block character.
const EMPTY: char = '░';

/// Render a progress indicator.
///
/// * `label` – descriptive text shown to the left (e.g. "Syncing…").
/// * `progress` – `None` for indeterminate, `Some(0.0..=1.0)` for a
///   percentage-based bar.
pub fn view<'a>(label: &'a str, progress: Option<f32>) -> Element<'a, Message> {
    let label_text = text(label)
        .size(13)
        .color(theme::TEXT_PRIMARY);

    let indicator: Element<'a, Message> = match progress {
        None => {
            text("⟳ Loading...")
                .size(13)
                .color(theme::TEXT_SECONDARY)
                .into()
        }
        Some(pct) => {
            let clamped = pct.clamp(0.0, 1.0);
            let filled = (clamped * BAR_WIDTH as f32).round() as usize;
            let empty = BAR_WIDTH.saturating_sub(filled);
            let bar: String = std::iter::repeat(FILLED)
                .take(filled)
                .chain(std::iter::repeat(EMPTY).take(empty))
                .collect();
            let pct_display = (clamped * 100.0).round() as u8;

            text(format!("[{bar}] {pct_display}%"))
                .size(13)
                .color(theme::ACCENT)
                .into()
        }
    };

    let content = row![label_text, indicator]
        .spacing(10)
        .padding(Padding::from([6, 10]));

    container(content)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.25))),
            border: Border {
                color: theme::SEPARATOR,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Shrink)
        .into()
}
