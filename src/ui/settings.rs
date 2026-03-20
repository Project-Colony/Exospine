//! Settings panel view.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Render the settings panel.
pub fn view(app: &App) -> Element<'_, Message> {
    let title = text("Settings").size(20).color(theme::TEXT_DARK);

    let settings = &app.settings;

    // ── Display current settings ────────────────────────────────────
    let theme_row = row![
        text("Theme:").size(14).color(theme::TEXT_DARK_SECONDARY),
        text(&settings.theme).size(14).color(theme::TEXT_DARK),
    ]
    .spacing(10);

    let font_row = row![
        text("Font size:").size(14).color(theme::TEXT_DARK_SECONDARY),
        text(settings.font_size.to_string())
            .size(14)
            .color(theme::TEXT_DARK),
        text("px").size(14).color(theme::TEXT_DARK_SECONDARY),
    ]
    .spacing(10);

    let interval_row = row![
        text("Check interval:")
            .size(14)
            .color(theme::TEXT_DARK_SECONDARY),
        text(format!("{}s", settings.check_interval_secs))
            .size(14)
            .color(theme::TEXT_DARK),
    ]
    .spacing(10);

    let notifications_row = row![
        text("Notifications:")
            .size(14)
            .color(theme::TEXT_DARK_SECONDARY),
        text(if settings.show_notifications {
            "Enabled"
        } else {
            "Disabled"
        })
        .size(14)
        .color(theme::TEXT_DARK),
    ]
    .spacing(10);

    let close_btn = button(text("Close").size(13))
        .on_press(Message::CloseSettings)
        .padding([8, 20])
        .style(theme::accent_button_style);

    let content = column![
        title,
        Space::new().height(16),
        theme_row,
        font_row,
        interval_row,
        notifications_row,
        Space::new().height(20),
        close_btn,
    ]
    .spacing(10)
    .padding(24)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::content_pane_style)
        .into()
}
