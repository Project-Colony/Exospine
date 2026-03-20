//! Outlook-inspired dark theme for Exospine.
//!
//! Provides color constants and helper functions for styling
//! Iced containers, buttons, and text elements.

use iced::Color;
use iced::widget::{button, container, text_input};
use iced::{Background, Border};

// ── Color palette ───────────────────────────────────────────────────────

/// Dark blue-gray sidebar background.
pub const SIDEBAR_BG: Color = Color::from_rgb(0.14, 0.16, 0.20);

/// Slightly lighter list pane background.
pub const LIST_BG: Color = Color::from_rgb(0.18, 0.20, 0.24);

/// Light content pane background.
pub const CONTENT_BG: Color = Color::from_rgb(0.96, 0.96, 0.97);

/// Outlook-style accent blue.
pub const ACCENT: Color = Color::from_rgb(0.0, 0.47, 0.84);

/// Lighter accent for hover states.
pub const ACCENT_LIGHT: Color = Color::from_rgb(0.20, 0.56, 0.90);

/// Primary text on dark backgrounds.
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.93, 0.93, 0.95);

/// Secondary / muted text on dark backgrounds.
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.60, 0.63, 0.68);

/// Primary text on light backgrounds.
pub const TEXT_DARK: Color = Color::from_rgb(0.13, 0.13, 0.15);

/// Secondary text on light backgrounds.
pub const TEXT_DARK_SECONDARY: Color = Color::from_rgb(0.45, 0.45, 0.50);

/// Unread indicator dot color.
pub const UNREAD_DOT: Color = Color::from_rgb(0.0, 0.47, 0.84);

/// Star / flagged icon color.
pub const STAR_COLOR: Color = Color::from_rgb(0.95, 0.75, 0.10);

/// Danger / delete action color.
pub const DANGER: Color = Color::from_rgb(0.85, 0.20, 0.20);

/// Selected item highlight on dark backgrounds.
pub const SELECTED_BG: Color = Color::from_rgb(0.22, 0.26, 0.34);

/// Hover background on dark surfaces.
pub const HOVER_BG: Color = Color::from_rgb(0.20, 0.22, 0.28);

/// Divider / separator line color.
pub const SEPARATOR: Color = Color::from_rgb(0.30, 0.32, 0.36);

/// Badge background.
pub const BADGE_BG: Color = Color::from_rgb(0.0, 0.47, 0.84);

// ── Container styles ────────────────────────────────────────────────────

/// Dark sidebar container style.
pub fn sidebar_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SIDEBAR_BG)),
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

/// Mail list pane container style.
pub fn list_pane_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(LIST_BG)),
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

/// Content / reading pane container style.
pub fn content_pane_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(CONTENT_BG)),
        text_color: Some(TEXT_DARK),
        ..Default::default()
    }
}

/// Selected item highlight container.
pub fn selected_item_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SELECTED_BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: ACCENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Badge container (small rounded pill).
pub fn badge_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BADGE_BG)),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

// ── Button styles ───────────────────────────────────────────────────────

/// Accent-colored primary action button.
pub fn accent_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => ACCENT_LIGHT,
        button::Status::Pressed => ACCENT,
        _ => ACCENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Transparent button for sidebar and icon buttons.
pub fn ghost_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => HOVER_BG,
        button::Status::Pressed => SELECTED_BG,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_PRIMARY,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Danger button (delete, discard).
pub fn danger_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(0.95, 0.30, 0.30),
        button::Status::Pressed => DANGER,
        _ => DANGER,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Toolbar button on light background.
pub fn toolbar_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(0.90, 0.91, 0.93),
        button::Status::Pressed => Color::from_rgb(0.85, 0.86, 0.88),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_DARK,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Search / text input style on dark background.
pub fn search_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::from_rgb(0.14, 0.16, 0.20)),
        border: Border {
            color: SEPARATOR,
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: TEXT_SECONDARY,
        placeholder: TEXT_SECONDARY,
        value: TEXT_PRIMARY,
        selection: ACCENT,
    }
}

/// Text input style on light background.
pub fn light_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::WHITE),
        border: Border {
            color: Color::from_rgb(0.78, 0.78, 0.80),
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: TEXT_DARK_SECONDARY,
        placeholder: TEXT_DARK_SECONDARY,
        value: TEXT_DARK,
        selection: ACCENT,
    }
}
