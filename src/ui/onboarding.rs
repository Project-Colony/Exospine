//! First-launch onboarding wizard for Exospine.
//!
//! Displays a full-screen overlay that guides new users through
//! initial setup. The flow has three steps:
//!   0 — Welcome screen
//!   1 — Prompt to add first email account
//!   2 — Confirmation / "You're all set!"

use iced::widget::{button, center, column, container, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::message::Message;
use crate::state::App;
use crate::ui::theme;

/// Render the onboarding overlay for the current step.
pub fn view(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = match app.onboarding_step {
        0 => step_welcome(),
        1 => step_add_account(),
        2 => step_done(),
        _ => step_done(),
    };

    // Full-screen dark backdrop
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.10, 0.12, 0.15))),
            ..Default::default()
        })
        .into()
}

// ── Step 0: Welcome ─────────────────────────────────────────────────────

fn step_welcome() -> Element<'static, Message> {
    let logo = text("[ Exospine ]")
        .size(36)
        .color(theme::ACCENT);

    let title = text("Welcome to Exospine")
        .size(28)
        .color(theme::TEXT_PRIMARY);

    let description = text("A fast, modern email client built for power users.")
        .size(16)
        .color(theme::TEXT_SECONDARY);

    let get_started_btn = button(
        text("Get Started").size(16).color(Color::WHITE),
    )
    .on_press(Message::OnboardingNext)
    .padding(Padding::from([12, 32]))
    .style(theme::accent_button_style);

    let skip_btn = button(
        text("Skip").size(13).color(theme::TEXT_SECONDARY),
    )
    .on_press(Message::OnboardingSkip)
    .padding(Padding::from([8, 16]))
    .style(theme::ghost_button_style);

    let card = column![
        logo,
        Space::new().height(16),
        title,
        Space::new().height(8),
        description,
        Space::new().height(32),
        get_started_btn,
        Space::new().height(12),
        skip_btn,
    ]
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(480.0))
    .padding(Padding::from(48));

    let card_container = container(card)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.18, 0.22))),
            border: Border {
                color: Color::from_rgb(0.28, 0.30, 0.34),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

    center(card_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Step 1: Add Account ─────────────────────────────────────────────────

fn step_add_account() -> Element<'static, Message> {
    let title = text("Add Your First Email Account")
        .size(24)
        .color(theme::TEXT_PRIMARY);

    let description = text(
        "Connect an email account to start sending and receiving messages.",
    )
    .size(14)
    .color(theme::TEXT_SECONDARY);

    let add_btn = button(
        text("Add Account").size(16).color(Color::WHITE),
    )
    .on_press(Message::AddAccount)
    .padding(Padding::from([12, 32]))
    .style(theme::accent_button_style);

    let skip_btn = button(
        text("Skip for now").size(13).color(theme::TEXT_SECONDARY),
    )
    .on_press(Message::OnboardingNext)
    .padding(Padding::from([8, 16]))
    .style(theme::ghost_button_style);

    let card = column![
        title,
        Space::new().height(8),
        description,
        Space::new().height(32),
        add_btn,
        Space::new().height(12),
        skip_btn,
    ]
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(480.0))
    .padding(Padding::from(48));

    let card_container = container(card)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.18, 0.22))),
            border: Border {
                color: Color::from_rgb(0.28, 0.30, 0.34),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

    center(card_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Step 2: Done ────────────────────────────────────────────────────────

fn step_done() -> Element<'static, Message> {
    let title = text("You're All Set!")
        .size(28)
        .color(theme::TEXT_PRIMARY);

    let description = text("Exospine is ready to use. Enjoy your new email experience.")
        .size(14)
        .color(theme::TEXT_SECONDARY);

    let start_btn = button(
        text("Start").size(16).color(Color::WHITE),
    )
    .on_press(Message::OnboardingComplete)
    .padding(Padding::from([12, 32]))
    .style(theme::accent_button_style);

    let card = column![
        title,
        Space::new().height(8),
        description,
        Space::new().height(32),
        start_btn,
    ]
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(480.0))
    .padding(Padding::from(48));

    let card_container = container(card)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.18, 0.22))),
            border: Border {
                color: Color::from_rgb(0.28, 0.30, 0.34),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

    center(card_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
