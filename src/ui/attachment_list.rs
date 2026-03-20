//! UI component for displaying email attachments in the reading pane.
//!
//! Shows a compact list of attachments with an icon (based on content type),
//! the filename, the human-readable size, and a download button.
//!
//! ## Message variants used
//!
//! This component emits the following `crate::message::Message` variants:
//!
//! - `DownloadAttachment(mail_index, part_index)` — save an attachment to disk.
//! - `AttachmentDownloaded(Result<PathBuf, String>)` — async result of a download.
//! - `OpenAttachment(PathBuf)` — open a previously-downloaded file.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::mail::attachments::{self, AttachmentMeta};
use crate::message::Message;
use crate::ui::theme;

/// Render the attachment list for a given mail entry.
///
/// `attachments` — the metadata extracted via [`attachments::extract_metadata`].
/// `mail_index` — the index of the parent mail entry (used in messages).
///
/// Returns an empty container when there are no attachments so callers can
/// always include it in their layout without conditional checks.
pub fn view<'a>(attachments: &'a [AttachmentMeta], mail_index: usize) -> Element<'a, Message> {
    if attachments.is_empty() {
        return container(Space::new().width(0).height(0))
            .width(Length::Shrink)
            .into();
    }

    let header = text(format!("Attachments ({})", attachments.len()))
        .size(12)
        .color(theme::TEXT_DARK_SECONDARY);

    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(attachments.len() + 1);
    rows.push(header.into());

    for meta in attachments {
        let icon_label = attachments::icon_for_content_type(&meta.content_type);
        let size_label = attachments::format_size(meta.size);

        let attachment_row = row![
            // Icon hint based on content type
            text(icon_label)
                .size(12)
                .color(theme::TEXT_DARK_SECONDARY),
            // Filename
            text(&meta.filename)
                .size(12)
                .color(theme::TEXT_DARK),
            // Size
            text(format!("({})", size_label))
                .size(11)
                .color(theme::TEXT_DARK_SECONDARY),
            // Spacer pushes the button to the right
            Space::new().width(Length::Fill),
            // Download button
            button(text("Download").size(11))
                .on_press(Message::DownloadAttachment(mail_index, meta.part_index))
                .padding([3, 10])
                .style(theme::toolbar_button_style),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);

        rows.push(
            container(attachment_row)
                .width(Length::Fill)
                .padding([3, 0])
                .into(),
        );
    }

    let list = column(rows).spacing(2).width(Length::Fill);

    container(list)
        .width(Length::Fill)
        .padding([8, 12])
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.92, 0.93, 0.94,
            ))),
            border: iced::Border {
                color: iced::Color::from_rgb(0.85, 0.85, 0.87),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}
