//! Reusable widget helpers for Exospine.

use iced::widget::{button, container, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::ui::theme;

/// Small rounded count badge (e.g. unread count).
///
/// Returns an empty column if `count` is zero so the badge disappears.
pub fn badge(count: u32) -> Element<'static, Message> {
    if count == 0 {
        return container(text("")).width(Length::Shrink).into();
    }
    container(
        text(count.to_string())
            .size(11)
            .color(iced::Color::WHITE),
    )
    .padding([1, 6])
    .style(theme::badge_style)
    .into()
}

/// A button displaying icon text that emits `msg` when pressed.
pub fn icon_button(icon: &str, msg: Message) -> Element<'_, Message> {
    button(text(icon).size(14))
        .on_press(msg)
        .padding([4, 8])
        .style(theme::ghost_button_style)
        .into()
}

/// Horizontal rule / divider line.
pub fn separator() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(1))
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(0.85, 0.85, 0.87))),
            ..Default::default()
        })
        .into()
}
