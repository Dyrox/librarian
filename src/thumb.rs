//! Minimal "screenshot" of a Live set's arrangement, drawn from parsed data:
//! one row per track that has clips, clips in Live's own colours, MIDI notes
//! drawn inside the clips like Live's clip overview.

use std::path::PathBuf;
use std::sync::Arc;

use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

use crate::Message;
use crate::als::Arrangement;
use crate::style;

pub struct Thumb {
    pub arrangement: Option<Arc<Arrangement>>,
    /// Identity of what is drawn, so a recycled widget state redraws.
    pub key: PathBuf,
}

#[derive(Default)]
pub struct State {
    cache: Cache,
    key: Option<PathBuf>,
    had_data: bool,
}

const GRID: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.045 };

fn clip_color(rgb: u32) -> Color {
    style::rgb(rgb)
}

fn darken(c: Color, f: f32) -> Color {
    Color { r: c.r * f, g: c.g * f, b: c.b * f, a: c.a }
}

impl canvas::Program<Message> for Thumb {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        _event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let has = self.arrangement.is_some();
        if state.key.as_ref() != Some(&self.key) || state.had_data != has {
            state.cache.clear();
            state.key = Some(self.key.clone());
            state.had_data = has;
            return Some(canvas::Action::request_redraw());
        }
        None
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // The cache can be stale for one frame if the widget was recycled for a
        // different project; drawing uncached in that case avoids a flash.
        if state.key.as_ref() != Some(&self.key) || state.had_data != self.arrangement.is_some() {
            let mut frame = Frame::new(renderer, bounds.size());
            self.paint(&mut frame, bounds.size());
            return vec![frame.into_geometry()];
        }
        vec![state.cache.draw(renderer, bounds.size(), |frame| self.paint(frame, bounds.size()))]
    }
}

impl Thumb {
    fn paint(&self, frame: &mut Frame, size: Size) {
        let (w, h) = (size.width, size.height);
        frame.fill(&Path::rounded_rectangle(Point::ORIGIN, size, 8.0.into()), style::p().thumb_bg);

        let Some(arr) = self.arrangement.as_deref() else { return };
        if arr.tracks.is_empty() || arr.length <= 0.0 {
            // An empty arrangement (session-view only set): a few faint lanes.
            for i in 0..4 {
                let y = 12.0 + i as f32 * (h - 24.0) / 4.0;
                frame.fill_rectangle(Point::new(12.0, y + 4.0), Size::new(w - 24.0, 2.0), GRID);
            }
            return;
        }

        let pad_x = 10.0;
        let pad_y = 8.0;
        let strip = 3.0; // track-colour strip, like Live's track headers
        let left = pad_x + strip + 4.0;
        let inner_w = (w - left - pad_x).max(1.0);
        let inner_h = (h - 2.0 * pad_y).max(1.0);
        // Round the visible length up to whole 8-bar phrases (4/4).
        let phrase = 32.0;
        let length = ((arr.length / phrase).ceil() * phrase).max(phrase);
        let x_of = |beat: f32| left + beat / length * inner_w;

        // Phrase grid.
        let mut b = phrase;
        while b < length {
            frame.fill_rectangle(Point::new(x_of(b).round(), pad_y), Size::new(1.0, inner_h), GRID);
            b += phrase;
        }

        let n = arr.tracks.len();
        let row = inner_h / n as f32;
        let gap = if row > 6.0 { (row * 0.18).min(3.0) } else { row * 0.12 };
        let clip_h = (row - gap).max(0.8);
        let radius = (clip_h * 0.18).min(2.5);

        for (i, t) in arr.tracks.iter().enumerate() {
            let y = pad_y + i as f32 * row + gap / 2.0;
            frame.fill_rectangle(Point::new(pad_x, y), Size::new(strip, clip_h), clip_color(t.color));

            for c in &t.clips {
                let x0 = x_of(c.start);
                let x1 = x_of(c.end);
                let cw = (x1 - x0 - 0.6).max(0.8);
                let base = clip_color(c.color);
                let fill = if c.disabled { Color { a: 0.25, ..base } } else { Color { a: 0.92, ..base } };
                frame.fill(&Path::rounded_rectangle(Point::new(x0, y), Size::new(cw, clip_h), radius.into()), fill);

                if !t.midi || c.notes.is_empty() || clip_h < 5.0 {
                    continue;
                }
                // Notes, scaled to this clip's own pitch range.
                let (lo, hi) = c.notes.iter().fold((u8::MAX, 0u8), |(lo, hi), n| (lo.min(n.key), hi.max(n.key)));
                let span = (hi - lo) as f32 + 1.0;
                let body_y = y + clip_h * 0.12;
                let body_h = clip_h * 0.76;
                let nh = (body_h / span).clamp(0.6, 2.5);
                let ink = darken(base, 0.35);
                for note in &c.notes {
                    let nx = x_of(note.start);
                    let nw = (note.len / length * inner_w).max(0.7);
                    let ny = body_y + (1.0 - ((note.key - lo) as f32 + 0.5) / span) * body_h - nh / 2.0;
                    frame.fill_rectangle(Point::new(nx, ny), Size::new(nw.min(x1 - nx), nh), ink);
                }
            }
        }
    }
}
