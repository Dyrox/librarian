//! Reads an Ableton Live set (.als = gzipped XML): summary facts plus a
//! lightweight copy of the arrangement (tracks, clips, MIDI notes) that is
//! enough to draw a "screenshot" of the set.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use flate2::read::GzDecoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlsMeta {
    /// e.g. "Live 12.2.1"
    pub version: String,
    pub tempo: Option<f32>,
    pub audio_tracks: u32,
    pub midi_tracks: u32,
    pub group_tracks: u32,
    pub return_tracks: u32,
    /// FL Studio: channel-rack size and pattern count (0 for Live).
    pub channels: u32,
    pub patterns: u32,
}

impl AlsMeta {
    pub fn track_count(&self) -> u32 {
        self.audio_tracks + self.midi_tracks
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arrangement {
    /// Only tracks that have at least one arrangement clip, in set order.
    pub tracks: Vec<ArrTrack>,
    /// End of the last clip, in beats.
    pub length: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArrTrack {
    pub name: String,
    /// 0xRRGGBB
    pub color: u32,
    pub midi: bool,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clip {
    pub start: f32,
    pub end: f32,
    /// 0xRRGGBB
    pub color: u32,
    pub disabled: bool,
    /// Absolute arrangement positions (beats).
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Note {
    pub start: f32,
    pub len: f32,
    pub key: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlsInfo {
    pub meta: AlsMeta,
    pub arrangement: Arrangement,
}

/// Cap per clip so pathological sets don't bloat the cache.
const MAX_NOTES_PER_CLIP: usize = 3000;

#[derive(Default)]
struct ClipBuilder {
    depth: usize,
    midi: bool,
    start: f32,
    end: f32,
    color: Option<u32>,
    disabled: bool,
    loop_start: f32,
    loop_end: f32,
    start_relative: f32,
    loop_on: bool,
    /// (time within clip, duration, key)
    raw_notes: Vec<(f32, f32, u8)>,
    pending: Vec<(f32, f32)>,
}

impl ClipBuilder {
    fn finish(self, track_color: u32) -> Clip {
        let mut notes = Vec::new();
        let clip_len = (self.end - self.start).max(0.0);
        if self.midi && clip_len > 0.0 {
            let period = self.loop_end - self.loop_start;
            // Where playback starts inside the clip's own timeline.
            let origin = if self.loop_on { self.loop_start + self.start_relative } else { self.loop_start };
            'outer: for &(t, d, key) in &self.raw_notes {
                if self.loop_on && period > 0.0 {
                    if t < self.loop_start || t >= self.loop_end {
                        continue;
                    }
                    // First occurrence at or after the origin, then every period.
                    let mut pos = t - origin;
                    while pos < 0.0 {
                        pos += period;
                    }
                    while pos < clip_len {
                        notes.push(Note { start: self.start + pos, len: d.min(clip_len - pos), key });
                        if notes.len() >= MAX_NOTES_PER_CLIP {
                            break 'outer;
                        }
                        pos += period;
                    }
                } else {
                    let pos = t - origin;
                    if pos >= 0.0 && pos < clip_len {
                        notes.push(Note { start: self.start + pos, len: d.min(clip_len - pos), key });
                        if notes.len() >= MAX_NOTES_PER_CLIP {
                            break;
                        }
                    }
                }
            }
        }
        Clip {
            start: self.start,
            end: self.end,
            color: self.color.unwrap_or(track_color),
            disabled: self.disabled,
            notes,
        }
    }
}

pub fn read(path: &Path) -> Option<AlsInfo> {
    let file = File::open(path).ok()?;
    let gz = BufReader::with_capacity(256 * 1024, GzDecoder::new(file));
    let mut reader = Reader::from_reader(gz);
    reader.config_mut().check_end_names = false;

    let mut meta = AlsMeta::default();
    let mut tracks: Vec<ArrTrack> = Vec::new();
    let mut track: Option<ArrTrack> = None;
    let mut clip: Option<ClipBuilder> = None;

    // Element-name stack; only names, no attributes.
    let mut stack: Vec<Vec<u8>> = Vec::with_capacity(64);
    let mut buf = Vec::with_capacity(4096);

    loop {
        let ev = match reader.read_event_into(&mut buf) {
            Ok(ev) => ev,
            Err(_) => break,
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let name = e.name().as_ref().as_bytes().to_vec();
                let depth = stack.len();
                let parent = stack.last().map(Vec::as_slice).unwrap_or(b"");

                if depth == 0 && name == b"Ableton" {
                    if let Some(c) = attr(e, b"Creator") {
                        meta.version = c.trim_start_matches("Ableton ").to_string();
                    }
                }

                // <Ableton><LiveSet><Tracks><XxxTrack>
                if depth == 3 && stack[2] == b"Tracks" && stack[1] == b"LiveSet" {
                    match name.as_slice() {
                        b"AudioTrack" => meta.audio_tracks += 1,
                        b"MidiTrack" => meta.midi_tracks += 1,
                        b"GroupTrack" => meta.group_tracks += 1,
                        b"ReturnTrack" => meta.return_tracks += 1,
                        _ => {}
                    }
                    if !is_empty && (name == b"AudioTrack" || name == b"MidiTrack") {
                        track = Some(ArrTrack { midi: name == b"MidiTrack", ..Default::default() });
                    }
                }

                if let Some(t) = track.as_mut() {
                    // Direct children / grandchildren of the track element (depth 3).
                    // Live 11+ writes <Color>, Live 10 and older <ColorIndex>.
                    if depth == 4 && (name == b"Color" || name == b"ColorIndex") {
                        t.color = palette(value_u8(e).unwrap_or(0));
                    } else if depth == 5 && parent == b"Name" && name == b"EffectiveName" {
                        t.name = attr(e, b"Value").unwrap_or_default();
                    }

                    // Arrangement clips only: MainSequencer › … › ArrangerAutomation › Events › Clip
                    // (skips session clips, take lanes and frozen audio).
                    if clip.is_none()
                        && (name == b"MidiClip" || name == b"AudioClip")
                        && depth >= 4
                        && parent == b"Events"
                        && stack[depth - 2] == b"ArrangerAutomation"
                        && stack[depth - 4] == b"MainSequencer"
                        && !is_empty
                    {
                        let start = attr(e, b"Time").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                        clip = Some(ClipBuilder {
                            depth,
                            midi: name == b"MidiClip",
                            start,
                            end: start,
                            ..Default::default()
                        });
                    } else if let Some(c) = clip.as_mut() {
                        let rel = depth - c.depth; // 1 = direct child of the clip
                        if rel == 1 {
                            match name.as_slice() {
                                b"CurrentStart" => c.start = value_f32(e).unwrap_or(c.start),
                                b"CurrentEnd" => c.end = value_f32(e).unwrap_or(c.end),
                                b"Color" | b"ColorIndex" => c.color = value_u8(e).map(palette),
                                b"Disabled" => c.disabled = attr(e, b"Value").as_deref() == Some("true"),
                                _ => {}
                            }
                        } else if rel == 2 && parent == b"Loop" {
                            match name.as_slice() {
                                b"LoopStart" => c.loop_start = value_f32(e).unwrap_or(0.0),
                                b"LoopEnd" => c.loop_end = value_f32(e).unwrap_or(0.0),
                                b"StartRelative" => c.start_relative = value_f32(e).unwrap_or(0.0),
                                b"LoopOn" => c.loop_on = attr(e, b"Value").as_deref() == Some("true"),
                                _ => {}
                            }
                        } else if c.midi && name == b"MidiNoteEvent" {
                            let enabled = attr(e, b"IsEnabled").as_deref() != Some("false");
                            let t = attr(e, b"Time").and_then(|v| v.parse().ok());
                            let d = attr(e, b"Duration").and_then(|v| v.parse().ok());
                            if let (true, Some(t), Some(d)) = (enabled, t, d) {
                                c.pending.push((t, d));
                            }
                        } else if c.midi && name == b"MidiKey" && parent == b"KeyTrack" {
                            let key = value_u8(e).unwrap_or(60);
                            let pending = std::mem::take(&mut c.pending);
                            c.raw_notes.extend(pending.into_iter().map(|(t, d)| (t, d, key)));
                        }
                    }
                }

                // MainTrack (Live 12) / MasterTrack (≤ 11) … Mixer > Tempo > Manual
                if meta.tempo.is_none()
                    && name == b"Manual"
                    && depth >= 3
                    && stack[depth - 1] == b"Tempo"
                    && stack[depth - 2] == b"Mixer"
                    && (stack[2] == b"MainTrack" || stack[2] == b"MasterTrack")
                {
                    meta.tempo = value_f32(e);
                    // Tracks precede the main track, so nothing else is needed.
                    break;
                }

                if !is_empty {
                    stack.push(name);
                }
            }
            Event::End(_) => {
                stack.pop();
                let depth = stack.len();
                if clip.as_ref().is_some_and(|c| c.depth == depth) {
                    let c = clip.take().unwrap();
                    if let Some(t) = track.as_mut() {
                        let built = c.finish(t.color);
                        if built.end > built.start {
                            t.clips.push(built);
                        }
                    }
                }
                if depth == 3 {
                    if let Some(t) = track.take() {
                        if !t.clips.is_empty() {
                            tracks.push(t);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if meta.version.is_empty() && meta.track_count() == 0 {
        return None;
    }
    let length = tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.end))
        .fold(0.0f32, f32::max);
    Some(AlsInfo { meta, arrangement: Arrangement { tracks, length } })
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref().as_bytes() == key)
        .map(|a| a.value.into_owned())
}

fn value_f32(e: &BytesStart) -> Option<f32> {
    attr(e, b"Value")?.parse().ok()
}

pub(crate) fn value_u8(e: &BytesStart) -> Option<u8> {
    attr(e, b"Value")?.parse::<i64>().ok().map(|v| v.clamp(0, 255) as u8)
}

// ---- compact binary encoding for the on-disk cache -------------------------

// Bump when parsing changes so stale thumbnails are rebuilt.
const MAGIC: &[u8; 4] = b"LAI5";

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn rgb(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes()[..3]);
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn rgb(&mut self) -> Option<u32> {
        let b = self.take(3)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

impl AlsInfo {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(4096));
        w.0.extend_from_slice(MAGIC);
        let m = &self.meta;
        w.str(&m.version);
        w.f32(m.tempo.unwrap_or(f32::NAN));
        for n in [m.audio_tracks, m.midi_tracks, m.group_tracks, m.return_tracks, m.channels, m.patterns] {
            w.u32(n);
        }
        let a = &self.arrangement;
        w.f32(a.length);
        w.u32(a.tracks.len() as u32);
        for t in &a.tracks {
            w.str(&t.name);
            w.rgb(t.color);
            w.u8(t.midi as u8);
            w.u32(t.clips.len() as u32);
            for c in &t.clips {
                w.f32(c.start);
                w.f32(c.end);
                w.rgb(c.color);
                w.u8(c.disabled as u8);
                w.u32(c.notes.len() as u32);
                for n in &c.notes {
                    w.f32(n.start);
                    w.f32(n.len);
                    w.u8(n.key);
                }
            }
        }
        w.0
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        let mut r = R(b);
        if r.take(4)? != MAGIC {
            return None;
        }
        let version = r.str()?;
        let tempo = Some(r.f32()?).filter(|t| !t.is_nan());
        let meta = AlsMeta {
            version,
            tempo,
            audio_tracks: r.u32()?,
            midi_tracks: r.u32()?,
            group_tracks: r.u32()?,
            return_tracks: r.u32()?,
            channels: r.u32()?,
            patterns: r.u32()?,
        };
        let length = r.f32()?;
        let nt = r.u32()? as usize;
        let mut tracks = Vec::with_capacity(nt.min(1024));
        for _ in 0..nt {
            let name = r.str()?;
            let color = r.rgb()?;
            let midi = r.u8()? != 0;
            let nc = r.u32()? as usize;
            let mut clips = Vec::with_capacity(nc.min(4096));
            for _ in 0..nc {
                let start = r.f32()?;
                let end = r.f32()?;
                let color = r.rgb()?;
                let disabled = r.u8()? != 0;
                let nn = r.u32()? as usize;
                let mut notes = Vec::with_capacity(nn.min(MAX_NOTES_PER_CLIP));
                for _ in 0..nn {
                    notes.push(Note { start: r.f32()?, len: r.f32()?, key: r.u8()? });
                }
                clips.push(Clip { start, end, color, disabled, notes });
            }
            tracks.push(ArrTrack { name, color, midi, clips });
        }
        Some(Self { meta, arrangement: Arrangement { tracks, length } })
    }
}

/// Live 10+ clip/track colour palette (index → RGB).
pub const PALETTE: [u32; 70] = [
    0xFF94A6, 0xFFA529, 0xCC9927, 0xF7F47C, 0xBFFB00, 0x1AFF2F, 0x25FFA8, 0x5CFFE8, 0x8BC5FF, 0x5480E4,
    0x92A7FF, 0xD86CE4, 0xE553A0, 0xFFFFFF, 0xFF3636, 0xF66C03, 0x99724B, 0xFFF034, 0x87FF67, 0x3DC300,
    0x00BFAF, 0x19E9FF, 0x10A4EE, 0x007DC0, 0x886CE4, 0xB677C6, 0xFF39D4, 0xD0D0D0, 0xE2675A, 0xFFA374,
    0xD3AD71, 0xEDFFAE, 0xD2E498, 0xBAD074, 0x9BC48D, 0xD4FDE1, 0xCDF1F8, 0xB9C1E3, 0xCDBBE4, 0xAE98E5,
    0xE5DCE1, 0xA9A9A9, 0xC6928B, 0xB78256, 0x99836A, 0xBFBA69, 0xA6BE00, 0x7DB04D, 0x88C2BA, 0x9BB3C4,
    0x85A5C2, 0x8393CC, 0xA595B5, 0xBF9FBE, 0xBC7196, 0x7B7B7B, 0xAF3333, 0xA95131, 0x724F41, 0xDBC300,
    0x85961F, 0x539F31, 0x0A9C8E, 0x236384, 0x1A2F96, 0x2F52A2, 0x624BAD, 0xA34BAD, 0xCC2E6E, 0x3C3C3C,
];

pub fn palette(index: u8) -> u32 {
    PALETTE.get(index as usize).copied().unwrap_or(0xA9A9A9)
}
