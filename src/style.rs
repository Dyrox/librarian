//! Visual language: soft neutral chrome, a rounded content panel, black (or
//! white, in dark mode) accents — loosely modelled on Motrix's layout.
//! The palette follows the system appearance; read it anywhere with `p()`.

use std::sync::atomic::{AtomicBool, Ordering};

use iced::border::Radius;
use iced::overlay::menu;
use iced::widget::{button, container, pick_list, scrollable, svg, text_input, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector, font};

pub const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

pub struct Palette {
    pub chrome: Color,
    pub panel: Color,
    pub surface: Color,
    pub card_hover: Color,
    pub border: Color,
    pub border_hover: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_2: Color,
    pub text_3: Color,
    pub nav_active: Color,
    pub nav_hover: Color,
    pub wave_idle: Color,
    pub wave_hover: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub on_accent: Color,
    pub green_bg: Color,
    pub green: Color,
    pub blue_bg: Color,
    pub blue: Color,
    pub fl_bg: Color,
    pub fl: Color,
    pub danger: Color,
    pub danger_bg: Color,
    pub warn: Color,
    pub warn_bg: Color,
    pub thumb_bg: Color,
    pub shadow: f32,
}

pub const LIGHT: Palette = Palette {
    chrome: rgb(0xE7ECEC),
    panel: rgb(0xFFFFFF),
    surface: rgb(0xF6F7F8),
    card_hover: rgb(0xFBFBFC),
    border: rgb(0xE6E8EB),
    border_hover: rgb(0xDDE0E4),
    border_strong: rgb(0xD6DADE),
    text: rgb(0x111317),
    text_2: rgb(0x6B7280),
    text_3: rgb(0x9CA3AF),
    nav_active: rgb(0xD8DEDF),
    nav_hover: rgb(0xDFE4E5),
    wave_idle: rgb(0xCDD3D8),
    wave_hover: rgb(0x9AA3AB),
    accent: rgb(0x111317),
    accent_hover: rgb(0x2A2D33),
    on_accent: rgb(0xFFFFFF),
    green_bg: rgb(0xE3F5EA),
    green: rgb(0x1F8A4C),
    blue_bg: rgb(0xEAF1FE),
    blue: rgb(0x3B82F6),
    fl_bg: rgb(0xFFF1E6),
    fl: rgb(0xD9600B),
    danger: rgb(0xB42318),
    danger_bg: rgb(0xFDECEA),
    warn: rgb(0xB54708),
    warn_bg: rgb(0xFEF0DC),
    thumb_bg: rgb(0x1E1F22),
    shadow: 0.06,
};

pub const DARK: Palette = Palette {
    chrome: rgb(0x18191C),
    panel: rgb(0x222327),
    surface: rgb(0x2A2C30),
    card_hover: rgb(0x26282C),
    border: rgb(0x2F3136),
    border_hover: rgb(0x3A3D43),
    border_strong: rgb(0x45484F),
    text: rgb(0xECEDEF),
    text_2: rgb(0xA1A5AD),
    text_3: rgb(0x6E737B),
    nav_active: rgb(0x2C2E33),
    nav_hover: rgb(0x232529),
    wave_idle: rgb(0x474A51),
    wave_hover: rgb(0x7A7F88),
    accent: rgb(0xF2F3F5),
    accent_hover: rgb(0xFFFFFF),
    on_accent: rgb(0x111317),
    green_bg: rgb(0x1C3326),
    green: rgb(0x5FD38D),
    blue_bg: rgb(0x1D2A44),
    blue: rgb(0x8AB4FF),
    fl_bg: rgb(0x3A2716),
    fl: rgb(0xFFA057),
    danger: rgb(0xFF8A80),
    danger_bg: rgb(0x3A1F1F),
    warn: rgb(0xF5B35C),
    warn_bg: rgb(0x3A2C17),
    thumb_bg: rgb(0x141517),
    shadow: 0.25,
};

static IS_DARK: AtomicBool = AtomicBool::new(false);

pub fn set_dark(dark: bool) {
    IS_DARK.store(dark, Ordering::Relaxed);
}

pub fn is_dark() -> bool {
    IS_DARK.load(Ordering::Relaxed)
}

pub fn p() -> &'static Palette {
    if is_dark() { &DARK } else { &LIGHT }
}

pub const BOLD: Font = Font { weight: font::Weight::Bold, ..Font::DEFAULT };
pub const SEMIBOLD: Font = Font { weight: font::Weight::Semibold, ..Font::DEFAULT };
pub const MEDIUM: Font = Font { weight: font::Weight::Medium, ..Font::DEFAULT };

fn border(color: Color, width: f32, radius: f32) -> Border {
    Border { color, width, radius: Radius::from(radius) }
}

fn shadow(alpha: f32, y: f32, blur: f32) -> Shadow {
    Shadow { color: Color { a: alpha, ..Color::BLACK }, offset: Vector::new(0.0, y), blur_radius: blur }
}

pub fn panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(p().panel.into()),
        border: border(p().border, 1.0, 16.0),
        shadow: shadow(p().shadow * 0.8, 1.0, 8.0),
        ..Default::default()
    }
}

pub fn pill(bg: Color, fg: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(fg),
        border: border(Color::TRANSPARENT, 0.0, 999.0),
        ..Default::default()
    }
}

pub fn outline_pill(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(p().text_2),
        border: border(p().border, 1.0, 999.0),
        ..Default::default()
    }
}

pub fn empty_wave(_: &Theme) -> container::Style {
    container::Style {
        background: Some(p().surface.into()),
        text_color: Some(p().text_3),
        border: border(p().border, 1.0, 10.0),
        ..Default::default()
    }
}

pub fn divider(_: &Theme) -> container::Style {
    container::Style { background: Some(p().border.into()), ..Default::default() }
}

pub fn card(expanded: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let p = p();
        button::Style {
            background: Some(Background::Color(if hovered || expanded { p.card_hover } else { p.panel })),
            text_color: p.text,
            border: border(
                if expanded { p.border_strong } else if hovered { p.border_hover } else { p.border },
                1.0,
                14.0,
            ),
            shadow: if hovered || expanded { shadow(p.shadow, 2.0, 10.0) } else { Shadow::default() },
            snap: true,
        }
    }
}

/// Grid tile: a container (it's wrapped in a mouse area for click/double-click).
pub fn tile(selected: bool, hovered: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let p = p();
        container::Style {
            background: Some(if hovered || selected { p.card_hover } else { p.panel }.into()),
            text_color: Some(p.text),
            border: border(
                if selected { p.accent } else if hovered { p.border_hover } else { p.border },
                if selected { 1.5 } else { 1.0 },
                14.0,
            ),
            shadow: if hovered || selected { shadow(p.shadow, 2.0, 10.0) } else { Shadow::default() },
            snap: true,
        }
    }
}

pub fn nav(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        let bg = if active {
            Some(p.nav_active.into())
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(p.nav_hover.into())
        } else {
            None
        };
        button::Style {
            background: bg,
            text_color: p.text,
            border: border(Color::TRANSPARENT, 0.0, 10.0),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

/// Solid circular / pill button in the accent colour.
pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let p = p();
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => p.accent_hover,
        button::Status::Disabled => p.text_3,
        _ => p.accent,
    };
    button::Style {
        background: Some(bg.into()),
        text_color: p.on_accent,
        border: border(Color::TRANSPARENT, 0.0, 999.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Play button floating on a (dark) thumbnail.
pub fn on_thumb(_: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Color { a: if hovered { 1.0 } else { 0.92 }, ..Color::WHITE }.into()),
        text_color: rgb(0x111317),
        border: border(Color::TRANSPARENT, 0.0, 999.0),
        shadow: shadow(0.35, 2.0, 8.0),
        snap: true,
    }
}

/// Panel-coloured button with a hairline border (Motrix's "Open").
pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    let p = p();
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(if hovered { p.surface } else { p.panel }.into()),
        text_color: p.text,
        border: border(if hovered { p.border_strong } else { p.border }, 1.0, 10.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn ghost(_: &Theme, status: button::Status) -> button::Style {
    let p = p();
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: if hovered { Some(p.surface.into()) } else { None },
        text_color: p.text,
        border: border(Color::TRANSPARENT, 0.0, 8.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn segment(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        let hovered = matches!(status, button::Status::Hovered);
        button::Style {
            background: if active { Some(p.panel.into()) } else { None },
            text_color: if active || hovered { p.text } else { p.text_2 },
            border: border(if active { p.border } else { Color::TRANSPARENT }, 1.0, 999.0),
            shadow: if active { shadow(p.shadow, 1.0, 3.0) } else { Shadow::default() },
            snap: true,
        }
    }
}

pub fn segment_track(_: &Theme) -> container::Style {
    container::Style {
        background: Some(p().surface.into()),
        border: border(p().border, 1.0, 999.0),
        ..Default::default()
    }
}

pub fn search_box(_: &Theme) -> container::Style {
    container::Style {
        background: Some(p().surface.into()),
        border: border(p().border, 1.0, 999.0),
        ..Default::default()
    }
}

pub fn search(_: &Theme, _: text_input::Status) -> text_input::Style {
    let p = p();
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: border(Color::TRANSPARENT, 0.0, 0.0),
        icon: p.text_3,
        placeholder: p.text_3,
        value: p.text,
        selection: Color { a: 0.3, ..p.blue },
    }
}

pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut s = scrollable::default(theme, status);
    let hovered = matches!(status, scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. });
    let p = p();
    s.container = container::Style::default();
    s.vertical_rail.background = None;
    s.vertical_rail.border = Border::default();
    s.vertical_rail.scroller.background =
        Background::Color(if hovered { p.wave_hover } else { p.wave_idle });
    s.vertical_rail.scroller.border = border(Color::TRANSPARENT, 0.0, 999.0);
    s
}

pub fn tint(color: Color) -> impl Fn(&Theme, svg::Status) -> svg::Style {
    move |_, _| svg::Style { color: Some(color) }
}

pub fn issue_card(_: &Theme) -> container::Style {
    let p = p();
    container::Style {
        background: Some(p.panel.into()),
        text_color: Some(p.text),
        border: border(p.border, 1.0, 12.0),
        ..Default::default()
    }
}

/// Bounce-version chip: filled when selected.
pub fn version_pill(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(if active { p.accent } else if hovered { p.surface } else { Color::TRANSPARENT }.into()),
            text_color: if active { p.on_accent } else { p.text_2 },
            border: border(if active { p.accent } else { p.border }, 1.0, 999.0),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn menu_panel(_: &Theme) -> container::Style {
    let p = p();
    container::Style {
        background: Some(p.panel.into()),
        text_color: Some(p.text),
        border: border(p.border_strong, 1.0, 11.0),
        shadow: shadow(if is_dark() { 0.5 } else { 0.16 }, 8.0, 28.0),
        ..Default::default()
    }
}

pub fn menu_item(highlighted: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if hovered || highlighted { Some(p.nav_active.into()) } else { None },
            text_color: p.text,
            border: border(Color::TRANSPARENT, 0.0, 7.0),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

/// iOS/macOS-style switch: accent track when on.
pub fn switch(_: &Theme, status: toggler::Status) -> toggler::Style {
    let p = p();
    let (on, hovered) = match status {
        toggler::Status::Active { is_toggled } => (is_toggled, false),
        toggler::Status::Hovered { is_toggled } => (is_toggled, true),
        toggler::Status::Disabled { is_toggled } => (is_toggled, false),
    };
    toggler::Style {
        background: Background::Color(if on { p.green } else if hovered { p.border_strong } else { p.border_hover }),
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Background::Color(Color::WHITE),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: None,
        border_radius: None,
        padding_ratio: 0.1,
    }
}

/// Drop-down that looks like a `secondary` button.
pub fn dropdown(_: &Theme, status: pick_list::Status) -> pick_list::Style {
    let p = p();
    let hot = !matches!(status, pick_list::Status::Active);
    pick_list::Style {
        text_color: p.text,
        placeholder_color: p.text_3,
        handle_color: p.text_2,
        background: Background::Color(if hot { p.surface } else { p.panel }),
        border: border(if hot { p.border_strong } else { p.border }, 1.0, 10.0),
    }
}

pub fn dropdown_menu(_: &Theme) -> menu::Style {
    let p = p();
    menu::Style {
        background: Background::Color(p.panel),
        border: border(p.border_strong, 1.0, 10.0),
        text_color: p.text,
        selected_text_color: p.text,
        selected_background: Background::Color(p.nav_active),
        shadow: shadow(if is_dark() { 0.5 } else { 0.16 }, 8.0, 28.0),
    }
}

/// A key name like "Space" or "⌘ ,".
pub fn keycap(_: &Theme) -> container::Style {
    let p = p();
    container::Style {
        background: Some(p.surface.into()),
        text_color: Some(p.text_2),
        border: border(p.border, 1.0, 6.0),
        ..Default::default()
    }
}
