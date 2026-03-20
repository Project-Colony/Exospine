//! Mail reading view: message header, body, and action buttons.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

use crate::mail::html_render;
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
            let has_html = entry.body_html.is_some();
            let html_toggle_label = if app.show_html { "Show Text" } else { "Show HTML" };

            let mut actions = row![
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
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);

            // Only show the HTML toggle if the mail has an HTML body
            if has_html {
                actions = actions.push(
                    button(text(html_toggle_label).size(13))
                        .on_press(Message::ToggleHtmlView)
                        .padding([6, 14])
                        .style(theme::toolbar_button_style),
                );
            }

            actions = actions.push(Space::new().width(Length::Fill));

            // Show "Allow Images" button when viewing HTML with images blocked
            if app.show_html && has_html && !app.allow_external_images {
                // Check if there are any blocked images in the sanitized content
                let sanitized = html_render::sanitize_html(
                    entry.body_html.as_deref().unwrap_or(""),
                );
                if sanitized.contains("[Image blocked:") {
                    actions = actions.push(
                        button(text("Allow Images").size(13))
                            .on_press(Message::AllowExternalImages)
                            .padding([6, 14])
                            .style(theme::toolbar_button_style),
                    );
                }
            }

            actions = actions
                .push(
                    button(text("Archive").size(13))
                        .on_press(Message::ArchiveMail(idx))
                        .padding([6, 14])
                        .style(theme::toolbar_button_style),
                )
                .push(
                    button(text("Delete").size(13))
                        .on_press(Message::DeleteMail(idx))
                        .padding([6, 14])
                        .style(theme::danger_button_style),
                );

            // ── Body ────────────────────────────────────────────
            let body_content = if app.show_html {
                if let Some(ref html) = entry.body_html {
                    let sanitized = html_render::sanitize_html(html);
                    let display_html = if app.allow_external_images {
                        html_render::allow_images(&sanitized)
                    } else {
                        sanitized
                    };
                    // Convert sanitized HTML to readable text via html2text
                    let rendered = html2text::from_read(display_html.as_bytes(), 80)
                        .unwrap_or_else(|_| display_html);
                    rendered
                } else {
                    entry.body_text.clone()
                }
            } else {
                entry.body_text.clone()
            };

            let body = text(body_content)
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
