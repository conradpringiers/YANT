use iced::theme::Palette;
use iced::widget::button;
use iced::{Border, Color, Font, Shadow, Theme};

pub const BG: Color = Color::from_rgb8(0x16, 0x18, 0x1C);
pub const TEXT: Color = Color::from_rgb8(0xD8, 0xDE, 0xE6);
pub const MUTED: Color = Color::from_rgb8(0x8B, 0x94, 0x9E);
pub const FAINT: Color = Color::from_rgb8(0x5C, 0x65, 0x70);
pub const ACCENT: Color = Color::from_rgb8(0x6E, 0xA8, 0xD8);
pub const GREEN: Color = Color::from_rgb8(0x7D, 0xB5, 0x8A);
pub const AMBER: Color = Color::from_rgb8(0xC7, 0xA7, 0x6B);
pub const RED: Color = Color::from_rgb8(0xC9, 0x7A, 0x7A);
pub const LINE: Color = Color::from_rgb8(0x2A, 0x2E, 0x35);

pub const BOLD: Font = Font {
    weight: iced::font::Weight::Semibold,
    ..Font::DEFAULT
};

pub const MONO: Font = Font::MONOSPACE;

pub fn theme() -> Theme {
    Theme::custom(
        "YANT",
        Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: GREEN,
            warning: AMBER,
            danger: RED,
        },
    )
}

pub fn app_style(_theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: BG,
        text_color: TEXT,
    }
}

pub fn nav(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |_theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);

        button::Style {
            background: None,
            text_color: if active {
                ACCENT
            } else if hovered {
                TEXT
            } else {
                MUTED
            },
            border: Border::default(),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}
