//! Custom context menu overlay for Exospine.
//!
//! Since Iced 0.14 has no built-in context menu, this module renders a
//! positioned column of action buttons as a Stack layer. Clicking any
//! item performs the action and closes the menu.

use iced::widget::{button, column, container, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::message::Message;
use crate::state::{App, ContextMenu, ContextTarget, FolderType};
use crate::ui::theme;

/// Render the context menu overlay positioned near `menu.position`.
pub fn view<'a>(menu: &ContextMenu, app: &'a App) -> Element<'a, Message> {
    let menu_items: Element<'a, Message> = match &menu.target {
        ContextTarget::Mail(idx) => mail_menu(*idx, app),
        ContextTarget::Folder(name) => folder_menu(name, app),
    };

    let (x, y) = menu.position;

    // Wrap in a full-screen clickable backdrop to dismiss on outside click,
    // then position the menu using padding from top-left.
    let backdrop = button(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseContextMenu)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &iced::Theme, _status: button::Status| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: Color::TRANSPARENT,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    // Position the menu card using top/left padding within a full-screen container
    let positioned_menu = container(menu_items)
        .padding(Padding::new(0.0).top(y).left(x));

    // Layer: transparent backdrop behind, positioned menu on top
    iced::widget::Stack::with_children(vec![
        backdrop.into(),
        positioned_menu
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// ── Mail context menu ───────────────────────────────────────────────────

fn mail_menu<'a>(idx: usize, app: &'a App) -> Element<'a, Message> {
    let entry = app.mail_entries.get(idx);

    let read_label = if entry.map_or(false, |e| e.is_read) {
        "Mark Unread"
    } else {
        "Mark Read"
    };

    let star_label = if entry.map_or(false, |e| e.is_starred) {
        "Unstar"
    } else {
        "Star"
    };

    let mut items = column![]
        .spacing(2)
        .padding(Padding::from(4))
        .width(Length::Fixed(180.0));

    items = items.push(menu_item("Reply", Message::Reply));
    items = items.push(menu_item("Reply All", Message::ReplyAll));
    items = items.push(menu_item("Forward", Message::Forward));
    items = items.push(menu_separator());

    if entry.map_or(false, |e| e.is_read) {
        items = items.push(menu_item(read_label, Message::MarkAsUnread(idx)));
    } else {
        items = items.push(menu_item(read_label, Message::MarkAsRead(idx)));
    }
    items = items.push(menu_item(star_label, Message::ToggleStar(idx)));
    items = items.push(menu_separator());
    items = items.push(menu_item("Archive", Message::ArchiveMail(idx)));
    items = items.push(menu_item_danger("Delete", Message::DeleteMail(idx)));

    menu_card(items)
}

// ── Folder context menu ─────────────────────────────────────────────────

fn folder_menu<'a>(name: &str, app: &'a App) -> Element<'a, Message> {
    let folder_type = app
        .selected_account
        .and_then(|i| app.accounts.get(i))
        .and_then(|acct| acct.folders.iter().find(|f| f.name == name))
        .map(|f| &f.folder_type);

    let mut items = column![]
        .spacing(2)
        .padding(Padding::from(4))
        .width(Length::Fixed(180.0));

    let owned_name = name.to_owned();

    items = items.push(menu_item(
        "Rename",
        Message::RenameFolder(owned_name.clone(), owned_name.clone()),
    ));
    items = items.push(menu_item(
        "Delete Folder",
        Message::DeleteFolder(owned_name),
    ));

    if folder_type == Some(&FolderType::Trash) {
        items = items.push(menu_separator());
        items = items.push(menu_item_danger("Empty Trash", Message::EmptyTrash));
    }

    menu_card(items)
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Wrap a column of items in a styled card container.
fn menu_card(items: iced::widget::Column<'_, Message>) -> Element<'_, Message> {
    container(items)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.18, 0.22))),
            border: Border {
                color: Color::from_rgb(0.28, 0.30, 0.34),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// A single context menu action button.
fn menu_item(label: &str, msg: Message) -> Element<'_, Message> {
    button(
        text(label)
            .size(13)
            .color(theme::TEXT_PRIMARY),
    )
    .on_press(msg)
    .padding(Padding::from([6, 12]))
    .width(Length::Fill)
    .style(|_theme: &iced::Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.22, 0.26, 0.34),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: theme::TEXT_PRIMARY,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

/// A danger-styled context menu action button.
fn menu_item_danger(label: &str, msg: Message) -> Element<'_, Message> {
    button(
        text(label)
            .size(13)
            .color(theme::DANGER),
    )
    .on_press(msg)
    .padding(Padding::from([6, 12]))
    .width(Length::Fill)
    .style(|_theme: &iced::Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgba(0.85, 0.20, 0.20, 0.15),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: theme::DANGER,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

/// A thin horizontal separator line between menu sections.
fn menu_separator() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(1))
        .width(Length::Fill)
        .padding(Padding::from([4, 8]))
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.28, 0.30, 0.34))),
            ..Default::default()
        })
        .into()
}
