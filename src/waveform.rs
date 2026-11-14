//! Clickable waveform overview drawn on a canvas.

use std::cell::Cell;
use std::path::PathBuf;

use iced::mouse;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, Path};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

use crate::Message;
use crate::peaks::Peaks;
use crate::style;

pub struct Waveform<'a> {
    pub peaks: Option<&'a Peaks>,
    /// Playback progress 0..=1 when this file is the one playing.
    pub progress: Option<f32>,
    pub file: PathBuf,
    pub project: PathBuf,
    pub bar: f32,
    pub gap: f32,
}

#[derive(Default)]
pub struct State {
    hover: Option<f32>,
    /// The drawn bars, reused until something visible changes: redrawing
    /// every waveform on every frame made scrolling stutter.
    cache: canvas::Cache,
    drawn: Cell<Option<Key>>,
}

/// Everything the picture depends on. Progress and hover are rounded to
/// whole pixels so playback only redraws when a bar actually changes.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    peaks: usize,
    len: usize,
    progress: i32,
    hover: i32,
    size: (u32, u32),
    dark: bool,
}

impl canvas::Program<Message> for Waveform<'_> {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let pos = cursor.position_in(bounds);
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new = pos.map(|p| (p.x / bounds.width).clamp(0.0, 1.0));
                if new != state.hover {
                    state.hover = new;
                    return Some(Action::request_redraw());
                }
                None
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                state.hover = None;
                Some(Action::request_redraw())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = pos?;
                let frac = (p.x / bounds.width).clamp(0.0, 1.0);
                Some(
                    Action::publish(Message::SeekTo(self.project.clone(), self.file.clone(), frac))
                        .and_capture(),
                )
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let w = bounds.width;
        let px = |f: Option<f32>| f.map_or(-1, |f| (f * w).round() as i32);
        let key = Key {
            peaks: self.peaks.map_or(0, |p| p.values.as_ptr() as usize),
            len: self.peaks.map_or(0, |p| p.values.len()),
            progress: px(self.progress),
            hover: px(state.hover),
            size: (bounds.width.to_bits(), bounds.height.to_bits()),
            dark: style::is_dark(),
        };
        if state.drawn.get() != Some(key) {
            state.cache.clear();
            state.drawn.set(Some(key));
        }
        vec![state.cache.draw(renderer, bounds.size(), |frame| self.paint(frame, state.hover))]
    }

    fn mouse_interaction(&self, _: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}

impl Waveform<'_> {
    fn paint(&self, frame: &mut Frame, hover: Option<f32>) {
        let w = frame.width();
        let h = frame.height();
        let step = self.bar + self.gap;
        let n = ((w + self.gap) / step).floor().max(1.0) as usize;
        let progress = self.progress.unwrap_or(0.0);
        let playing = self.progress.is_some();

        for i in 0..n {
            let v = match self.peaks {
                Some(p) if !p.values.is_empty() => {
                    let len = p.values.len();
                    let a = i * len / n;
                    let b = ((i + 1) * len / n).max(a + 1).min(len);
                    p.values[a..b].iter().cloned().fold(0.0, f32::max)
                }
                // Placeholder while peaks are computed.
                _ => 0.08,
            };
            let x = i as f32 * step;
            let center = (x + self.bar / 2.0) / w;
            let bh = (v.powf(0.8) * h).max(2.0).min(h);
            let y = (h - bh) / 2.0;
            let color = if playing && center <= progress {
                style::p().text
            } else if hover.is_some_and(|hv| center <= hv) {
                style::p().wave_hover
            } else {
                style::p().wave_idle
            };
            let rect = Path::rounded_rectangle(
                Point::new(x, y),
                Size::new(self.bar, bh),
                (self.bar / 2.0).into(),
            );
            frame.fill(&rect, color);
        }

        if let Some(hv) = hover {
            let x = (hv * w).round();
            frame.fill_rectangle(Point::new(x, 0.0), Size::new(1.0, h), Color { a: 0.5, ..style::p().text });
        }
    }

}
