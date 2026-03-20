//! Sidebar view: account list, folder trees, and bottom action buttons.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::{App, FolderType};
use crate::ui::{theme, widgets};

/// Returns the icon string for a folder type.
fn folder_icon(ft: &FolderType) -> &'static str {
    match ft {
        FolderType::Inbox => "\u{1F4E7}",   // 📧
        FolderType::Sent => "\u{1F4E4}",    // 📤
        FolderType::Drafts => "\u{1F4DD}",  // 📝
        FolderType::Trash => "\u{1F5D1}",   // 🗑
        FolderType::Starred => "\u{2B50}",  // ⭐
        FolderType::Custom => "\u{1F4C1}",  // 📁
    }
}

/// Render the sidebar panel.
pub fn view(app: &App) -> Element<'_, Message> {
    let mut sidebar_col = column![].spacing(2).width(Length::Fill);

    // ── Account sections with folder trees ──────────────────────────
    for (acct_idx, account) in app.accounts.iter().enumerate() {
        let is_selected_account = app.selected_account == Some(acct_idx);

        // Account header button
        let acct_label = text(&account.name)
            .size(13)
            .color(if is_selected_account {
                theme::ACCENT
            } else {
                theme::TEXT_PRIMARY
            });

        let acct_btn = button(
            row![
                text("\u{1F4E8}").size(14), // 📨
                acct_label,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::SelectAccount(acct_idx))
        .width(Length::Fill)
        .padding([6, 10])
        .style(theme::ghost_button_style);

        sidebar_col = sidebar_col.push(acct_btn);

        // Show folders only for the selected account
        if is_selected_account {
            for folder in &account.folders {
                let is_selected_folder = app.selected_folder.as_deref() == Some(&folder.name);

                let icon = text(folder_icon(&folder.folder_type)).size(13);
                let name = text(&folder.name)
                    .size(13)
                    .color(if is_selected_folder {
                        theme::ACCENT
                    } else {
                        theme::TEXT_PRIMARY
                    });

                let badge_el = widgets::badge(folder.unread_count);

                let folder_row = row![icon, name, Space::new().width(Length::Fill), badge_el]
                    .spacing(6)
                    .align_y(iced::Alignment::Center);

                let folder_btn = button(folder_row)
                    .on_press(Message::SelectFolder(folder.name.clone()))
                    .width(Length::Fill)
                    .padding(iced::Padding::new(4.0).right(10).left(24));

                let folder_btn = if is_selected_folder {
                    folder_btn.style(|theme, status| {
                        let mut s = theme::ghost_button_style(theme, status);
                        s.background =
                            Some(iced::Background::Color(theme::SELECTED_BG));
                        s
                    })
                } else {
                    folder_btn.style(theme::ghost_button_style)
                };

                sidebar_col = sidebar_col.push(folder_btn);
            }
        }

        // Thin separator between accounts
        sidebar_col = sidebar_col.push(
            container(Space::new().width(Length::Fill).height(1))
                .width(Length::Fill)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme::SEPARATOR)),
                    ..Default::default()
                }),
        );
    }

    // ── Bottom action buttons ───────────────────────────────────────
    let bottom = column![
        button(
            row![text("+").size(14), text("Add Account").size(13)]
                .spacing(6)
                .align_y(iced::Alignment::Center),
        )
        .on_press(Message::AddAccount)
        .width(Length::Fill)
        .padding([6, 10])
        .style(theme::ghost_button_style),
        button(
            row![text("\u{2699}").size(14), text("Settings").size(13)] // ⚙
                .spacing(6)
                .align_y(iced::Alignment::Center),
        )
        .on_press(Message::OpenSettings)
        .width(Length::Fill)
        .padding([6, 10])
        .style(theme::ghost_button_style),
    ]
    .spacing(2);

    let content = column![
        scrollable(sidebar_col).height(Length::Fill),
        container(bottom).padding([8, 0]),
    ]
    .height(Length::Fill);

    container(content)
        .width(220)
        .height(Length::Fill)
        .padding([8, 0])
        .style(theme::sidebar_style)
        .into()
}
