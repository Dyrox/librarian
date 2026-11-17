//! In-app context menu (styled like the rest of Librarian, not the OS menu) and
//! the tag UI bits that live in it, on cards and in the sidebar.

use std::path::PathBuf;

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, button, column, container, image, mouse_area, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Length, Point, Size};

use crate::icons::{self, icon};
use crate::scan::Project;
use crate::style::{self, *};
use crate::i18n::{self, t};
use crate::tags::{self, Tags};
use crate::{App, Daw, Filter, Message, fmt_dur, fmt_time, reveal_label};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    Sets,
    Bounces,
    Tags,
}

#[derive(Debug, Clone)]
pub struct MenuState {
    pub project: PathBuf,
    pub at: Point,
    pub sub: Option<Sub>,
    /// Scrolling since the menu opened. A two-finger click on a trackpad
    /// also sends a few tiny scroll events; only a real scroll closes it.
    pub scrolled: f32,
}

const MENU_W: f32 = 252.0;
const SUB_W: f32 = 288.0;
const ITEM_H: f32 = 30.0;
const SEP_H: f32 = 9.0;
const PAD: f32 = 5.0;
const SUB_MAX_H: f32 = 340.0;
pub const NEW_TAG_INPUT: &str = "librarian-new-tag";

/// Tag colour as text: brighter in dark mode so it reads on dark chips.
pub fn tag_fg(rgb: u32) -> Color {
    let c = style::rgb(rgb);
    if style::is_dark() {
        Color { r: c.r * 0.6 + 0.4, g: c.g * 0.6 + 0.4, b: c.b * 0.6 + 0.4, a: 1.0 }
    } else {
        Color { r: c.r * 0.85, g: c.g * 0.85, b: c.b * 0.85, a: 1.0 }
    }
}

pub fn tag_bg(rgb: u32) -> Color {
    Color { a: if style::is_dark() { 0.22 } else { 0.14 }, ..style::rgb(rgb) }
}

fn dot<'a>(rgb: u32, size: f32) -> Element<'a, Message> {
    let c = style::rgb(rgb);
    container(Space::new().width(size).height(size)).style(pill(c, c)).into()
}

enum Line<'a> {
    Item(Element<'a, Message>),
    Sep,
}

impl App {
    pub(crate) fn tag_key(&self, pr: &Project) -> String {
        Tags::key(self.root().unwrap_or(std::path::Path::new("")), &pr.dir)
    }

    /// Small coloured chips for a project's tags (cards and tiles).
    pub(crate) fn tag_chips<'a>(&'a self, pr: &Project, size: f32) -> Option<Element<'a, Message>> {
        let key = self.tag_key(pr);
        let names = self.tags.of(&key);
        if names.is_empty() {
            return None;
        }
        let mut r = row![].spacing(4).align_y(Alignment::Center);
        for n in names {
            let color = self.tags.def(n).map(|d| d.color).unwrap_or(0x8A8F98);
            r = r.push(
                container(
                    row![dot(color, 6.0), text(tags::label(n)).size(size).font(MEDIUM).wrapping(Wrapping::None)]
                        .spacing(5)
                        .align_y(Alignment::Center),
                )
                .padding([2, 7])
                .style(pill(tag_bg(color), tag_fg(color))),
            );
        }
        Some(r.into())
    }

    /// A banner across the top of a grid thumbnail, one band per tag in its
    /// colour, so a project's state reads at a glance.
    pub(crate) fn tag_banner<'a>(&'a self, pr: &Project) -> Option<Element<'a, Message>> {
        let key = self.tag_key(pr);
        let names = self.tags.of(&key);
        if names.is_empty() {
            return None;
        }
        let last = names.len() - 1;
        let mut r = row![].width(Length::Fill);
        for (i, n) in names.iter().enumerate() {
            let c = style::rgb(self.tags.def(n).map(|d| d.color).unwrap_or(0x8A8F98));
            // Dark text on light colours (Idea's yellow), white otherwise.
            let lum = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
            let fg = if lum > 0.6 { style::rgb(0x1A1A1A) } else { Color::WHITE };
            let radius = iced::border::Radius {
                top_left: if i == 0 { 8.0 } else { 0.0 },
                top_right: if i == last { 8.0 } else { 0.0 },
                bottom_right: 0.0,
                bottom_left: 0.0,
            };
            r = r.push(
                container(text(tags::label(n)).size(11).font(SEMIBOLD).wrapping(Wrapping::None))
                    .padding([3, 10])
                    .width(Length::Fill)
                    .center_x(Length::Fill)
                    .clip(true)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Color { a: 0.92, ..c }.into()),
                        text_color: Some(fg),
                        border: iced::Border { radius, ..Default::default() },
                        ..Default::default()
                    }),
            );
        }
        Some(r.into())
    }

    /// Expanded card: every tag as a toggle.
    pub(crate) fn tag_toggles<'a>(&'a self, pr: &Project) -> Element<'a, Message> {
        let p = p();
        let key = self.tag_key(pr);
        let mut r = row![].spacing(6).align_y(Alignment::Center);
        for d in &self.tags.defs {
            let on = self.tags.has(&key, &d.name);
            let label = row![dot(d.color, 7.0), text(tags::label(&d.name)).size(12).font(MEDIUM).wrapping(Wrapping::None)]
                .spacing(6)
                .align_y(Alignment::Center);
            let (color, name) = (d.color, d.name.clone());
            r = r.push(
                button(label)
                    .padding([4, 10])
                    .style(move |_, status| {
                        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                        button::Style {
                            background: Some(if on { tag_bg(color) } else if hovered { p.surface } else { Color::TRANSPARENT }.into()),
                            text_color: if on { tag_fg(color) } else { p.text_2 },
                            border: iced::Border {
                                color: if on { Color { a: 0.5, ..style::rgb(color) } } else { p.border },
                                width: 1.0,
                                radius: 999.0.into(),
                            },
                            shadow: Default::default(),
                            snap: true,
                        }
                    })
                    .on_press(Message::ToggleTag(pr.dir.clone(), name)),
            );
        }
        container(r).clip(true).into()
    }

    /// Sidebar "TAGS" section.
    pub(crate) fn tags_nav(&self) -> Element<'_, Message> {
        let p = p();
        let mut col = Column::new().spacing(2);
        for d in &self.tags.defs {
            let count = self.projects.iter().filter(|pr| self.tags.has(&self.tag_key(pr), &d.name)).count();
            if count == 0 && !d.builtin {
                continue;
            }
            let active = !self.settings_open && self.filter == Filter::Tag(d.name.clone());
            col = col.push(
                button(
                    row![
                        container(dot(d.color, 9.0)).width(16).center_x(16),
                        text(tags::label(&d.name))
                            .size(14)
                            .font(if active { MEDIUM } else { iced::Font::DEFAULT })
                            .wrapping(Wrapping::None)
                            .width(Length::Fill),
                        text(count.to_string()).size(12).color(p.text_3),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([7, 10])
                .style(nav(active))
                .on_press(Message::SetFilter(Filter::Tag(d.name.clone()))),
            );
        }
        // Everything that hasn't been sorted yet.
        let count = self.projects.iter().filter(|pr| self.tags.of(&self.tag_key(pr)).is_empty()).count();
        let active = !self.settings_open && self.filter == Filter::Untagged;
        let ring = container(Space::new().width(9).height(9)).style(|_: &iced::Theme| container::Style {
            border: style::border(style::p().text_3, 1.5, 999.0),
            ..Default::default()
        });
        col = col.push(
            button(
                row![
                    container(ring).width(16).center_x(16),
                    text(t("Untagged"))
                        .size(14)
                        .font(if active { MEDIUM } else { iced::Font::DEFAULT })
                        .wrapping(Wrapping::None)
                        .width(Length::Fill),
                    text(count.to_string()).size(12).color(p.text_3),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([7, 10])
            .style(nav(active))
            .on_press(Message::SetFilter(Filter::Untagged)),
        );
        col.into()
    }

    // ------------------------------------------------------------ the menu

    fn item<'a>(
        &self,
        glyph: Option<Element<'a, Message>>,
        label: String,
        trailing: Option<Element<'a, Message>>,
        on_press: Option<Message>,
        hover: Option<Sub>,
        highlighted: bool,
    ) -> Element<'a, Message> {
        let p = p();
        let enabled = on_press.is_some() || hover.is_some();
        let label = text(label)
            .size(13)
            .color(if enabled { p.text } else { p.text_3 })
            .wrapping(Wrapping::None)
            .width(Length::Fill)
            .into();
        self.item_el(glyph, label, trailing, on_press, hover, highlighted)
    }

    fn item_el<'a>(
        &self,
        glyph: Option<Element<'a, Message>>,
        label: Element<'a, Message>,
        trailing: Option<Element<'a, Message>>,
        on_press: Option<Message>,
        hover: Option<Sub>,
        highlighted: bool,
    ) -> Element<'a, Message> {
        let slot: Element<'a, Message> = match glyph {
            Some(g) => container(g).width(18).center_x(18).into(),
            None => Space::new().width(18).into(),
        };
        let mut r = row![slot, label]
        .spacing(9)
        .align_y(Alignment::Center);
        if let Some(t) = trailing {
            r = r.push(t);
        }
        let btn = button(container(r.clip(true)).center_y(Length::Fill))
            .width(Length::Fill)
            .height(ITEM_H)
            .padding([0, 9])
            .style(menu_item(highlighted))
            .on_press_maybe(on_press.or(hover.map(|s| Message::OpenSub(Some(s)))));
        mouse_area(btn).on_enter(Message::OpenSub(hover)).into()
    }

    fn chevron<'a>(&self) -> Option<Element<'a, Message>> {
        Some(icon(icons::CHEVRON_RIGHT, 12, p().text_3).into())
    }

    fn icon_el<'a>(&self, src: &'static str) -> Option<Element<'a, Message>> {
        Some(icon(src, 14, p().text_2).into())
    }

    /// Main menu lines and the y offset of each submenu trigger.
    fn main_lines<'a>(&'a self, pr: &'a Project, sub: Option<Sub>) -> (Vec<Line<'a>>, Vec<(Sub, f32)>) {
        let mut lines: Vec<Line<'a>> = Vec::new();
        // (submenu, index of its trigger line) → turned into y offsets at the end.
        let mut anchors: Vec<(Sub, usize)> = Vec::new();
        let wrap = |m: Message| Message::FromMenu(Box::new(m));

        if let Some(set) = pr.main_set() {
            let glyph: Option<Element<'a, Message>> = match self.daw_icons.get(&pr.daw) {
                Some(h) => Some(image(h.clone()).width(16).height(16).into()),
                None => self.icon_el(icons::EXTERNAL),
            };
            lines.push(Line::Item(self.item(glyph, i18n::open_in(pr.daw.name()), None, Some(wrap(Message::Open(set.path.clone()))), None, false)));
        }
        if pr.sets.len() > 1 {
            anchors.push((Sub::Sets, lines.len()));
            let label = t(if pr.daw == Daw::FlStudio { "Other Project Files" } else { "Other Sets" });
            lines.push(Line::Item(self.item(self.icon_el(icons::FILE), label.into(), self.chevron(), None, Some(Sub::Sets), sub == Some(Sub::Sets))));
        }
        lines.push(Line::Sep);

        if let Some(d) = self.current_demo(pr) {
            let playing = self.audio.current.as_deref() == Some(d.path.as_path()) && !self.audio.is_paused();
            lines.push(Line::Item(self.item(
                self.icon_el(if playing { icons::PAUSE } else { icons::PLAY }),
                t(if playing { "Pause" } else { "Play" }).into(),
                Some(text(t("Space")).size(11).color(p().text_3).into()),
                Some(wrap(Message::PlayProject(pr.dir.clone()))),
                None,
                false,
            )));
            anchors.push((Sub::Bounces, lines.len()));
            lines.push(Line::Item(self.item(
                self.icon_el(icons::WAVE),
                i18n::bounces_n(pr.versions.len()),
                self.chevron(),
                None,
                Some(Sub::Bounces),
                sub == Some(Sub::Bounces),
            )));
            if self.new_count(pr) > 0 {
                lines.push(Line::Item(self.item(
                    self.icon_el(icons::CHECK),
                    t("Mark Bounces as Heard").into(),
                    None,
                    Some(wrap(Message::MarkHeard(pr.dir.clone()))),
                    None,
                    false,
                )));
            }
        } else {
            lines.push(Line::Item(self.item(self.icon_el(icons::WAVE), t("No bounces yet").into(), None, None, None, false)));
        }
        lines.push(Line::Sep);

        anchors.push((Sub::Tags, lines.len()));
        lines.push(Line::Item(self.item(self.icon_el(icons::TAG), t("Tags").into(), self.chevron(), None, Some(Sub::Tags), sub == Some(Sub::Tags))));
        lines.push(Line::Sep);
        lines.push(Line::Item(self.item(self.icon_el(icons::FINDER), reveal_label().into(), None, Some(wrap(Message::Reveal(pr.dir.clone()))), None, false)));
        lines.push(Line::Item(self.item(self.icon_el(icons::COPY), t("Copy Path").into(), None, Some(wrap(Message::CopyPath(pr.dir.clone()))), None, false)));
        let offsets = anchors
            .into_iter()
            .map(|(sub, i)| (sub, PAD + lines[..i].iter().map(|l| if matches!(l, Line::Sep) { SEP_H } else { ITEM_H }).sum::<f32>()))
            .collect();
        (lines, offsets)
    }

    fn sub_lines<'a>(&'a self, pr: &'a Project, sub: Sub) -> Vec<Line<'a>> {
        let p = p();
        let wrap = |m: Message| Message::FromMenu(Box::new(m));
        let mut lines = Vec::new();
        match sub {
            Sub::Sets => {
                for s in &pr.sets {
                    lines.push(Line::Item(self.item(
                        self.icon_el(icons::FILE),
                        s.name.clone(),
                        Some(text(fmt_time(s.modified)).size(11).color(p.text_3).into()),
                        Some(wrap(Message::Open(s.path.clone()))),
                        Some(Sub::Sets),
                        false,
                    )));
                }
            }
            Sub::Bounces => {
                let sel = self.version_index(pr);
                for (i, v) in pr.versions.iter().enumerate() {
                    let playing = v.files.iter().any(|f| self.audio.current.as_deref() == Some(f.path.as_path()));
                    let glyph: Option<Element<'a, Message>> = if playing {
                        Some(dot(0x22A06B, 7.0))
                    } else if i == sel {
                        Some(icon(icons::CHECK, 13, p.text_2).into())
                    } else {
                        None
                    };
                    let dur = self.duration_of(&v.file().path).map(|d| format!(" · {}", fmt_dur(d))).unwrap_or_default();
                    lines.push(Line::Item(self.item(
                        glyph,
                        v.label.clone(),
                        Some(text(format!("{}{}", fmt_time(v.modified), dur)).size(11).color(p.text_3).into()),
                        Some(wrap(Message::PlayFile(pr.dir.clone(), v.file().path.clone()))),
                        Some(Sub::Bounces),
                        false,
                    )));
                }
            }
            Sub::Tags => {
                let key = self.tag_key(pr);
                for d in &self.tags.defs {
                    let on = self.tags.has(&key, &d.name);
                    let trailing: Option<Element<'a, Message>> = if d.builtin {
                        None
                    } else {
                        Some(
                            button(icon(icons::X, 11, p.text_3))
                                .padding(3)
                                .style(ghost)
                                .on_press(Message::DeleteTag(d.name.clone()))
                                .into(),
                        )
                    };
                    let glyph: Option<Element<'a, Message>> = on.then(|| icon(icons::CHECK, 13, p.text).into());
                    let label = row![
                        dot(d.color, 8.0),
                        text(tags::label(&d.name)).size(13).wrapping(Wrapping::None).width(Length::Fill),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into();
                    lines.push(Line::Item(self.item_el(
                        glyph,
                        label,
                        trailing,
                        Some(Message::ToggleTag(pr.dir.clone(), d.name.clone())),
                        Some(Sub::Tags),
                        false,
                    )));
                }
                lines.push(Line::Sep);
                lines.push(Line::Item(
                    container(
                        row![
                            icon(icons::TAG, 13, p.text_3),
                            text_input(t("New tag…"), &self.tag_draft)
                                .id(NEW_TAG_INPUT)
                                .on_input(Message::TagDraft)
                                .on_submit(Message::CreateTag(pr.dir.clone()))
                                .size(13)
                                .padding([5, 0])
                                .style(search),
                        ]
                        .spacing(9)
                        .align_y(Alignment::Center),
                    )
                    .padding([0, 12])
                    .height(ITEM_H)
                    .center_y(ITEM_H)
                    .into(),
                ));
            }
        }
        lines
    }

    fn panel<'a>(lines: Vec<Line<'a>>, width: f32, scroll_h: Option<f32>) -> Element<'a, Message> {
        let col = Column::with_children(lines.into_iter().map(|l| match l {
            Line::Item(e) => e,
            Line::Sep => container(container(Space::new().height(1)).width(Length::Fill).style(divider))
                .padding([4, 6])
                .into(),
        }));
        let body: Element<'a, Message> = match scroll_h {
            Some(h) => scrollable(col)
                .height(h)
                .direction(Direction::Vertical(Scrollbar::new().width(4).scroller_width(4)))
                .style(scroll)
                .into(),
            None => col.into(),
        };
        // Swallow clicks on the panel's padding so they don't close the menu.
        mouse_area(container(body).padding(PAD).width(width).style(menu_panel))
            .on_press(Message::Noop)
            .on_right_press(Message::Noop)
            .into()
    }

    fn lines_height(lines: &[Line]) -> f32 {
        lines.iter().map(|l| if matches!(l, Line::Sep) { SEP_H } else { ITEM_H }).sum::<f32>() + 2.0 * PAD
    }

    /// The floating menu layer (on top of everything), if open.
    pub(crate) fn context_menu(&self) -> Option<Element<'_, Message>> {
        let m = self.menu.as_ref()?;
        let pr = self.project(&m.project)?;
        let win = if self.window_size.width > 0.0 { self.window_size } else { Size::new(1180.0, 800.0) };

        let (lines, anchors) = self.main_lines(pr, m.sub);
        let main_h = Self::lines_height(&lines);
        // Open just off the cursor (like macOS), so right-clicking the same
        // spot again lands on the project, not the menu, and reopens it.
        let x = (m.at.x + 2.0).min(win.width - MENU_W - 8.0).max(8.0);
        let y = (m.at.y + 2.0).min(win.height - main_h - 8.0).max(8.0);

        let mut layer = iced::widget::Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(column![Space::new().height(y), row![Space::new().width(x), Self::panel(lines, MENU_W, None)]]);

        if let Some(sub) = m.sub {
            let sub_lines = self.sub_lines(pr, sub);
            let full_h = Self::lines_height(&sub_lines);
            let scroll_h = (full_h > SUB_MAX_H).then_some(SUB_MAX_H - 2.0 * PAD);
            let h = full_h.min(SUB_MAX_H);
            let anchor = anchors.iter().find(|(s, _)| *s == sub).map(|(_, a)| *a).unwrap_or(0.0);
            let sx = if x + MENU_W + 4.0 + SUB_W <= win.width - 8.0 { x + MENU_W + 4.0 } else { (x - SUB_W - 4.0).max(8.0) };
            let sy = (y + anchor - PAD).min(win.height - h - 8.0).max(8.0);
            layer = layer.push(column![
                Space::new().height(sy),
                row![Space::new().width(sx), Self::panel(sub_lines, SUB_W, scroll_h)]
            ]);
        }

        // A left click outside closes the menu (and is swallowed, like macOS).
        // Right clicks pass through: on a project they reopen the menu right
        // there; anywhere else `DismissMenu` (see `subscription`) closes it.
        let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(Message::CloseMenu);
        Some(iced::widget::stack![backdrop, layer].into())
    }
}
