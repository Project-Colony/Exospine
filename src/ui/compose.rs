//! Compose view: new mail / reply / forward form.

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Format a file size in bytes to a human-readable string.
fn format_file_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Render the compose panel. Returns an empty container when not composing.
pub fn view(app: &App) -> Element<'_, Message> {
    let draft = match &app.composing {
        Some(d) => d,
        None => {
            return container(Space::new())
                .width(0)
                .height(0)
                .into();
        }
    };

    let title = text("Compose Message")
        .size(18)
        .color(theme::TEXT_DARK);

    // ── Form fields ─────────────────────────────────────────────────
    let to_input = text_input("To", &draft.to)
        .on_input(Message::ComposeToChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    let cc_input = text_input("Cc", &draft.cc)
        .on_input(Message::ComposeCcChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    let bcc_input = text_input("Bcc", &draft.bcc)
        .on_input(Message::ComposeBccChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    let subject_input = text_input("Subject", &draft.subject)
        .on_input(Message::ComposeSubjectChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    let body_input = text_input("Write your message...", &draft.body)
        .on_input(Message::ComposeBodyChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    // ── Attachments section ─────────────────────────────────────────
    let attachment_path_input = text_input("File path to attach...", &app.compose_attachment_path)
        .on_input(Message::ComposeAttachmentPathChanged)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);

    let add_btn = button(text("Add").size(13))
        .on_press(Message::AddAttachment)
        .padding([8, 14])
        .style(theme::toolbar_button_style);

    let attachment_input_row = row![
        attachment_path_input,
        add_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let mut attachments_col = column![
        text("Attachments").size(13).color(theme::TEXT_DARK_SECONDARY),
        attachment_input_row,
    ]
    .spacing(6);

    for (i, att) in draft.attachments.iter().enumerate() {
        let size_display = format_file_size(att.size);
        let info = text(format!("{} ({})", att.filename, size_display))
            .size(12)
            .color(theme::TEXT_DARK);
        let remove_btn = button(text("Remove").size(11))
            .on_press(Message::RemoveAttachment(i))
            .padding([4, 10])
            .style(theme::danger_button_style);
        let att_row = row![info, Space::new().width(Length::Fill), remove_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center);
        attachments_col = attachments_col.push(att_row);
    }

    // ── Action buttons ──────────────────────────────────────────────
    let actions = row![
        button(text("Send").size(13))
            .on_press(Message::SendMail)
            .padding([8, 20])
            .style(theme::accent_button_style),
        button(text("Save Draft").size(13))
            .on_press(Message::SaveDraft)
            .padding([8, 14])
            .style(theme::toolbar_button_style),
        Space::new().width(Length::Fill),
        button(text("Discard").size(13))
            .on_press(Message::DiscardDraft)
            .padding([8, 14])
            .style(theme::danger_button_style),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let form = column![
        title,
        to_input,
        cc_input,
        bcc_input,
        subject_input,
        body_input,
        attachments_col,
        actions,
    ]
    .spacing(10)
    .padding(20)
    .width(Length::Fill);

    container(form)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::content_pane_style)
        .into()
}
