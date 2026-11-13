//! Inline line icons (Lucide-style, 24×24). Tinted at draw time.

use iced::widget::{Svg, svg};
use iced::{Color, Length};

use crate::style;

macro_rules! stroke {
    ($body:expr) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            $body,
            "</svg>"
        )
    };
}

pub const GRID: &str = stroke!(r#"<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>"#);
pub const CLOCK: &str = stroke!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#);
pub const GEAR: &str = stroke!(r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>"#);
pub const SPARKLE: &str = stroke!(r#"<path d="M12 3l1.9 5.6L19.5 10.5l-5.6 1.9L12 18l-1.9-5.6L4.5 10.5l5.6-1.9z"/><path d="M19 3v4M17 5h4"/>"#);
pub const WAVE: &str = stroke!(r#"<path d="M2 12h2M6 8v8M10 4v16M14 7v10M18 10v4M22 12h0"/>"#);
pub const FOLDER: &str = stroke!(r#"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#);
pub const SEARCH: &str = stroke!(r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>"#);
pub const REFRESH: &str = stroke!(r#"<path d="M21 12a9 9 0 0 1-15.5 6.2L3 16"/><path d="M3 12A9 9 0 0 1 18.5 5.8L21 8"/><path d="M21 3v5h-5"/><path d="M3 21v-5h5"/>"#);
pub const EXTERNAL: &str = stroke!(r#"<path d="M14 4h6v6"/><path d="M20 4 10 14"/><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5"/>"#);
pub const FINDER: &str = stroke!(r#"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M12 11v5M9.5 13.5 12 16l2.5-2.5"/>"#);
pub const FILE: &str = stroke!(r#"<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>"#);
pub const CHEVRON_DOWN: &str = stroke!(r#"<path d="m6 9 6 6 6-6"/>"#);
pub const CHEVRON_LEFT: &str = stroke!(r#"<path d="m15 6-6 6 6 6"/>"#);
pub const CHEVRON_RIGHT: &str = stroke!(r#"<path d="m9 6 6 6-6 6"/>"#);
pub const X: &str = stroke!(r#"<path d="M18 6 6 18M6 6l12 12"/>"#);
pub const LIST: &str = stroke!(r#"<path d="M8 6h13M8 12h13M8 18h13"/><circle cx="3.5" cy="6" r="1"/><circle cx="3.5" cy="12" r="1"/><circle cx="3.5" cy="18" r="1"/>"#);
pub const TILES: &str = stroke!(r#"<rect x="3" y="4" width="8" height="7" rx="1.5"/><rect x="13" y="4" width="8" height="7" rx="1.5"/><rect x="3" y="13" width="8" height="7" rx="1.5"/><rect x="13" y="13" width="8" height="7" rx="1.5"/>"#);
pub const ALERT: &str = stroke!(r#"<path d="M10.3 3.9 2.4 17.5A2 2 0 0 0 4.1 20.5h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>"#);
pub const ARROW_RIGHT: &str = stroke!(r#"<path d="M5 12h14M13 6l6 6-6 6"/>"#);
pub const TAG: &str = stroke!(r#"<path d="M3 12V5a2 2 0 0 1 2-2h7l9 9-9 9z"/><circle cx="7.5" cy="7.5" r="1.3"/>"#);
pub const COPY: &str = stroke!(r#"<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/>"#);
pub const CHECK: &str = stroke!(r#"<path d="m5 12 5 5 9-10"/>"#);
/// Books on a shelf, the last one leaning (Librarian).
pub const LOGO: &str = stroke!(r#"<path d="M4 4v16M8 8v12M12 6v14M16 6l4 14"/>"#);

pub const PLAY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M7 4.8v14.4a1 1 0 0 0 1.5.86l12-7.2a1 1 0 0 0 0-1.72l-12-7.2A1 1 0 0 0 7 4.8z" fill="black"/></svg>"#;
pub const PAUSE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="6" y="4.5" width="4.2" height="15" rx="1.2" fill="black"/><rect x="13.8" y="4.5" width="4.2" height="15" rx="1.2" fill="black"/></svg>"#;

pub fn icon<'a>(src: &'static str, size: u16, color: Color) -> Svg<'a> {
    svg(svg::Handle::from_memory(src.as_bytes()))
        .width(Length::Fixed(size as f32))
        .height(Length::Fixed(size as f32))
        .style(style::tint(color))
}
