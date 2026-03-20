//! Mail reading view: message header, body, and action buttons.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Render the mail reading pane.
pub fn view(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = match app.selected_mail {
        Some(idx) if idx < app.mail_entries.len() => {
            let entry = &app.mail_entries[idx];

            // ── Header ──────────────────────────────────────────
            let from_row = row![
                text("From: ").size(13).color(theme::TEXT_DARK_SECONDARY),
                text(&entry.from).size(13).color(theme::TEXT_DARK),
            ]
            .spacing(4);

            let to_str = entry.to.join(", ");
            let to_row = row![
                text("To: ").size(13).color(theme::TEXT_DARK_SECONDARY),
                text(to_str).size(13).color(theme::TEXT_DARK),
            ]
            .spacing(4);

            let subject_text = text(&entry.subject)
                .size(18)
                .color(theme::TEXT_DARK);

            let date_str = entry.date.format("%B %d, %Y at %H:%M").to_string();
            let date_text = text(date_str).size(12).color(theme::TEXT_DARK_SECONDARY);

            let header = column![subject_text, from_row, to_row, date_text].spacing(4);

            // ── Action buttons ──────────────────────────────────
            let actions = row![
                button(text("Reply").size(13))
                    .on_press(Message::Reply)
                    .padding([6, 14])
                    .style(theme::toolbar_button_style),
                button(text("Reply All").size(13))
                    .on_press(Message::ReplyAll)
                    .padding([6, 14])
                    .style(theme::toolbar_button_style),
                button(text("Forward").size(13))
                    .on_press(Message::Forward)
                    .padding([6, 14])
                    .style(theme::toolbar_button_style),
                Space::new().width(Length::Fill),
                button(text("Archive").size(13))
                    .on_press(Message::ArchiveMail(idx))
                    .padding([6, 14])
                    .style(theme::toolbar_button_style),
                button(text("Delete").size(13))
                    .on_press(Message::DeleteMail(idx))
                    .padding([6, 14])
                    .style(theme::danger_button_style),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);

            // ── Body ────────────────────────────────────────────
            let body = text(&entry.body_text)
                .size(14)
                .color(theme::TEXT_DARK);

            let full = column![
                header,
                container(Space::new().width(Length::Fill).height(1)).width(Length::Fill).style(|_theme: &iced::Theme| container::Style { background: Some(iced::Background::Color(iced::Color::from_rgb(0.85, 0.85, 0.87))), ..Default::default() }),
                actions,
                container(Space::new().width(Length::Fill).height(1)).width(Length::Fill).style(|_theme: &iced::Theme| container::Style { background: Some(iced::Background::Color(iced::Color::from_rgb(0.85, 0.85, 0.87))), ..Default::default() }),
                body,
            ]
            .spacing(12)
            .padding(20)
            .width(Length::Fill);

            scrollable(full).height(Length::Fill).into()
        }

        _ => {
            // No mail selected — centered placeholder
            container(
                text("Select a message")
                    .size(16)
                    .color(theme::TEXT_DARK_SECONDARY),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        }
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::content_pane_style)
        .into()
}
