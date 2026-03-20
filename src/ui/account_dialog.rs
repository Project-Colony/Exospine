//! "Add Account" modal dialog.
//!
//! Renders a form for entering email credentials with auto-detected
//! provider settings, connection testing, and account creation.
//!
//! The dialog state lives in [`crate::state::AccountDialogState`] and
//! is stored as `App::account_dialog: Option<AccountDialogState>`.
//! The dialog message variants live in `message.rs` under the
//! "Account dialog" section (`Dialog*` variants).

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::AccountDialogState;
use crate::ui::theme;

/// Render the "Add Account" dialog panel.
///
/// Uses the main `Message` enum directly so it integrates with the
/// existing Iced update loop without mapping.
pub fn view(dialog: &AccountDialogState) -> Element<'_, Message> {
    let title = text("Add Account").size(20).color(theme::TEXT_DARK);

    // ── Provider badge ──────────────────────────────────────────────
    let provider_label = dialog.provider.provider.label();
    let badge = container(
        text(provider_label)
            .size(11)
            .color(iced::Color::WHITE),
    )
    .padding([2, 8])
    .style(theme::badge_style);

    let header = row![title, Space::new().width(Length::Fill), badge]
        .align_y(iced::Alignment::Center)
        .spacing(8);

    // ── Form fields ─────────────────────────────────────────────────
    let email_input = labeled_input(
        "Email",
        "user@example.com",
        &dialog.email,
        Message::DialogEmailChanged,
    );

    let name_input = labeled_input(
        "Display Name",
        "John Doe",
        &dialog.display_name,
        Message::DialogNameChanged,
    );

    let password_input = {
        let label = text("Password").size(12).color(theme::TEXT_DARK_SECONDARY);
        let input = text_input("Password", &dialog.password)
            .on_input(Message::DialogPasswordChanged)
            .secure(true)
            .padding([8, 12])
            .size(13)
            .style(theme::light_input_style);
        column![label, input].spacing(4)
    };

    // IMAP row: host + port side by side.
    let imap_row = {
        let label = text("IMAP Server").size(12).color(theme::TEXT_DARK_SECONDARY);
        let host = text_input("imap.example.com", &dialog.imap_host)
            .on_input(Message::DialogImapHostChanged)
            .padding([8, 12])
            .size(13)
            .style(theme::light_input_style)
            .width(Length::Fill);
        let port = text_input("993", &dialog.imap_port)
            .on_input(Message::DialogImapPortChanged)
            .padding([8, 12])
            .size(13)
            .style(theme::light_input_style)
            .width(70);
        column![label, row![host, port].spacing(6)].spacing(4)
    };

    // SMTP row: host + port side by side.
    let smtp_row = {
        let label = text("SMTP Server").size(12).color(theme::TEXT_DARK_SECONDARY);
        let host = text_input("smtp.example.com", &dialog.smtp_host)
            .on_input(Message::DialogSmtpHostChanged)
            .padding([8, 12])
            .size(13)
            .style(theme::light_input_style)
            .width(Length::Fill);
        let port = text_input("587", &dialog.smtp_port)
            .on_input(Message::DialogSmtpPortChanged)
            .padding([8, 12])
            .size(13)
            .style(theme::light_input_style)
            .width(70);
        column![label, row![host, port].spacing(6)].spacing(4)
    };

    // ── Status text ─────────────────────────────────────────────────
    let status: Element<'_, Message> = if dialog.testing {
        text("Testing connection...")
            .size(12)
            .color(theme::TEXT_DARK_SECONDARY)
            .into()
    } else {
        match &dialog.test_result {
            None => Space::new().height(0).into(),
            Some(Ok(())) => text("Connection successful!")
                .size(12)
                .color(iced::Color::from_rgb(0.15, 0.65, 0.15))
                .into(),
            Some(Err(msg)) => text(msg.as_str())
                .size(12)
                .color(theme::DANGER)
                .into(),
        }
    };

    // ── Action buttons ──────────────────────────────────────────────
    let mut test_btn = button(text("Test Connection").size(13))
        .padding([8, 16])
        .style(theme::toolbar_button_style);
    if dialog.can_test() {
        test_btn = test_btn.on_press(Message::DialogTestConnection);
    }

    let mut submit_btn = button(text("Add Account").size(13))
        .padding([8, 20])
        .style(theme::accent_button_style);
    if dialog.can_submit() {
        submit_btn = submit_btn.on_press(Message::DialogSubmitAccount);
    }

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CancelAddAccount)
        .padding([8, 14])
        .style(theme::ghost_button_style);

    let actions = row![
        test_btn,
        Space::new().width(Length::Fill),
        cancel_btn,
        submit_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // ── Assemble form ───────────────────────────────────────────────
    let form = column![
        header,
        email_input,
        name_input,
        password_input,
        imap_row,
        smtp_row,
        status,
        actions,
    ]
    .spacing(12)
    .padding(24)
    .max_width(500);

    container(form)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(theme::content_pane_style)
        .into()
}

/// Helper: a label + text input pair.
fn labeled_input<'a>(
    label_text: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: fn(String) -> Message,
) -> Element<'a, Message> {
    let label = text(label_text)
        .size(12)
        .color(theme::TEXT_DARK_SECONDARY);
    let input = text_input(placeholder, value)
        .on_input(on_input)
        .padding([8, 12])
        .size(13)
        .style(theme::light_input_style);
    column![label, input].spacing(4).into()
}
