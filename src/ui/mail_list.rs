//! Mail list view: search bar and scrollable message list.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Font, Length};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Render the mail list panel.
pub fn view(app: &App) -> Element<'_, Message> {
    let mut content = column![].spacing(0).width(Length::Fill);

    // ── Search bar ──────────────────────────────────────────────────
    let search = text_input("Search mail...", &app.search_query)
        .on_input(Message::SearchChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::search_input_style);

    content = content.push(
        container(search).padding(8).width(Length::Fill),
    );

    // ── Toolbar: New Mail & Refresh ─────────────────────────────────
    let toolbar = row![
        button(
            row![text("+").size(13), text("New").size(13)]
                .spacing(4)
                .align_y(iced::Alignment::Center),
        )
        .on_press(Message::NewMail)
        .padding([4, 10])
        .style(theme::accent_button_style),
        Space::new().width(Length::Fill),
        button(text("\u{21BB}").size(14)) // ↻
            .on_press(Message::RefreshFolder)
            .padding([4, 8])
            .style(theme::ghost_button_style),
    ]
    .align_y(iced::Alignment::Center)
    .padding([2, 8]);

    content = content.push(toolbar);

    // ── Mail entries ────────────────────────────────────────────────
    if app.mail_entries.is_empty() {
        let empty = container(
            text("No messages")
                .size(14)
                .color(theme::TEXT_SECONDARY),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

        content = content.push(empty);
    } else {
        let mut list = column![].spacing(1);

        for (idx, entry) in app.mail_entries.iter().enumerate() {
            let is_selected = app.selected_mail == Some(idx);

            // Star icon
            let star_icon = if entry.is_starred {
                text("\u{2605}").size(14).color(theme::STAR_COLOR) // ★
            } else {
                text("\u{2606}").size(14).color(theme::TEXT_SECONDARY) // ☆
            };

            let star_btn = button(star_icon)
                .on_press(Message::ToggleStar(idx))
                .padding([2, 4])
                .style(theme::ghost_button_style);

            // Unread dot
            let unread_indicator = if !entry.is_read {
                text("\u{25CF}").size(8).color(theme::UNREAD_DOT) // ●
            } else {
                text(" ").size(8)
            };

            // Sender — bold if unread
            let sender_text = text(&entry.from).size(13).color(theme::TEXT_PRIMARY);
            let sender_text = if !entry.is_read {
                sender_text.font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                })
            } else {
                sender_text
            };

            // Subject
            let subject_text = text(&entry.subject).size(12).color(theme::TEXT_PRIMARY);
            let subject_text = if !entry.is_read {
                subject_text.font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                })
            } else {
                subject_text
            };

            // Date (short format)
            let date_str = entry.date.format("%b %d").to_string();
            let date_text = text(date_str).size(11).color(theme::TEXT_SECONDARY);

            // Preview snippet
            let preview_text = text(&entry.preview)
                .size(11)
                .color(theme::TEXT_SECONDARY);

            // Layout: [dot] [star] [sender + date] / [subject] / [preview]
            let header_row = row![
                unread_indicator,
                star_btn,
                sender_text,
                Space::new().width(Length::Fill),
                date_text,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);

            let entry_col = column![header_row, subject_text, preview_text]
                .spacing(2)
                .width(Length::Fill);

            let entry_btn = button(entry_col)
                .on_press(Message::SelectMail(idx))
                .width(Length::Fill)
                .padding([8, 10]);

            let entry_btn = if is_selected {
                entry_btn.style(|theme, status| {
                    let mut s = theme::ghost_button_style(theme, status);
                    s.background = Some(iced::Background::Color(theme::SELECTED_BG));
                    s
                })
            } else {
                entry_btn.style(theme::ghost_button_style)
            };

            list = list.push(entry_btn);
        }

        content = content.push(scrollable(list).height(Length::Fill));
    }

    container(content)
        .width(320)
        .height(Length::Fill)
        .style(theme::list_pane_style)
        .into()
}
