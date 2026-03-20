//! Settings panel view.

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Helper: create a section header.
fn section_header(label: &str) -> Element<'_, Message> {
    text(label)
        .size(16)
        .color(theme::ACCENT)
        .into()
}

/// Helper: styled selector button. Highlighted when `active` is true.
fn selector_btn<'a>(label: &'a str, active: bool, msg: Message) -> Element<'a, Message> {
    let btn = button(text(label).size(13).color(if active {
        iced::Color::WHITE
    } else {
        theme::TEXT_DARK
    }))
    .on_press(msg)
    .padding([6, 16]);

    if active {
        btn.style(theme::accent_button_style).into()
    } else {
        btn.style(theme::toolbar_button_style).into()
    }
}

/// Render the full settings panel.
pub fn view(app: &App) -> Element<'_, Message> {
    let settings = &app.settings;

    let title = text("Settings").size(22).color(theme::TEXT_DARK);

    // ── Appearance ──────────────────────────────────────────────────

    let theme_row = row![
        text("Theme").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        selector_btn(
            "Dark",
            settings.theme == "dark",
            Message::SettingsThemeChanged("dark".to_string()),
        ),
        selector_btn(
            "Light",
            settings.theme == "light",
            Message::SettingsThemeChanged("light".to_string()),
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let font_size_row = row![
        text("Font size (px)").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        container(
            text_input("14", &settings.font_size.to_string())
                .on_input(|val| Message::SettingsFontSizeChanged(val))
                .width(60)
                .size(13)
                .style(theme::light_input_style)
        ),
        text("px").size(13).color(theme::TEXT_DARK_SECONDARY),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let density_row = row![
        text("Display density").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        selector_btn(
            "Compact",
            settings.density == "compact",
            Message::SettingsDensityChanged("compact".to_string()),
        ),
        selector_btn(
            "Normal",
            settings.density == "normal",
            Message::SettingsDensityChanged("normal".to_string()),
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let appearance_section = column![
        section_header("Appearance"),
        theme_row,
        font_size_row,
        density_row,
    ]
    .spacing(10);

    // ── Sync ────────────────────────────────────────────────────────

    let interval_row = row![
        text("Check interval").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        container(
            text_input("300", &settings.check_interval_secs.to_string())
                .on_input(|val| Message::SettingsCheckIntervalChanged(val))
                .width(80)
                .size(13)
                .style(theme::light_input_style)
        ),
        text("seconds").size(13).color(theme::TEXT_DARK_SECONDARY),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let notif_label = if settings.show_notifications {
        "Enabled"
    } else {
        "Disabled"
    };
    let notif_row = row![
        text("Notifications").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        button(text(notif_label).size(13))
            .on_press(Message::SettingsToggleNotifications)
            .padding([6, 16])
            .style(if settings.show_notifications {
                theme::accent_button_style
            } else {
                theme::toolbar_button_style
            }),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let sync_section = column![
        section_header("Sync"),
        interval_row,
        notif_row,
    ]
    .spacing(10);

    // ── Reading ─────────────────────────────────────────────────────

    let reading_row = row![
        text("Reading pane").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        selector_btn(
            "Right",
            settings.reading_pane == "right",
            Message::SettingsReadingPaneChanged("right".to_string()),
        ),
        selector_btn(
            "Bottom",
            settings.reading_pane == "bottom",
            Message::SettingsReadingPaneChanged("bottom".to_string()),
        ),
        selector_btn(
            "Off",
            settings.reading_pane == "off",
            Message::SettingsReadingPaneChanged("off".to_string()),
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let reading_section = column![
        section_header("Reading"),
        reading_row,
    ]
    .spacing(10);

    // ── Language ─────────────────────────────────────────────────────

    let lang_row = row![
        text("Language").size(14).color(theme::TEXT_DARK_SECONDARY),
        Space::new().width(Length::Fill),
        selector_btn(
            "EN",
            settings.language == "en",
            Message::SettingsLanguageChanged("en".to_string()),
        ),
        selector_btn(
            "FR",
            settings.language == "fr",
            Message::SettingsLanguageChanged("fr".to_string()),
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let language_section = column![
        section_header("Language"),
        lang_row,
    ]
    .spacing(10);

    // ── Accounts ────────────────────────────────────────────────────

    let mut accounts_col = column![section_header("Accounts")].spacing(6);

    if app.accounts.is_empty() {
        accounts_col = accounts_col.push(
            text("No accounts configured.")
                .size(13)
                .color(theme::TEXT_DARK_SECONDARY),
        );
    } else {
        for (i, account) in app.accounts.iter().enumerate() {
            let account_row = row![
                text(format!("{} ({})", account.name, account.email))
                    .size(13)
                    .color(theme::TEXT_DARK),
                Space::new().width(Length::Fill),
                button(text("Remove").size(12))
                    .on_press(Message::RemoveAccount(i))
                    .padding([4, 12])
                    .style(theme::danger_button_style),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            accounts_col = accounts_col.push(account_row);
        }
    }

    // ── About ───────────────────────────────────────────────────────

    let about_section = column![
        section_header("About"),
        text("Exospine v0.1.0").size(13).color(theme::TEXT_DARK),
        text("Portable Outlook-like email client built with Rust + Iced")
            .size(12)
            .color(theme::TEXT_DARK_SECONDARY),
        text("https://github.com/MotherSphere/Exospine-Private")
            .size(12)
            .color(theme::ACCENT),
    ]
    .spacing(4);

    // ── Action buttons ──────────────────────────────────────────────

    let actions_row = row![
        button(text("Save").size(13))
            .on_press(Message::SaveSettings)
            .padding([8, 24])
            .style(theme::accent_button_style),
        button(text("Close").size(13))
            .on_press(Message::CloseSettings)
            .padding([8, 24])
            .style(theme::toolbar_button_style),
    ]
    .spacing(12);

    // ── Assemble ────────────────────────────────────────────────────

    let content = column![
        title,
        Space::new().height(12),
        appearance_section,
        Space::new().height(16),
        sync_section,
        Space::new().height(16),
        reading_section,
        Space::new().height(16),
        language_section,
        Space::new().height(16),
        accounts_col,
        Space::new().height(16),
        about_section,
        Space::new().height(20),
        actions_row,
    ]
    .spacing(4)
    .padding(24)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::content_pane_style)
        .into()
}
