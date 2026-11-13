//! Reads an FL Studio project (.flp): a small header followed by a flat
//! stream of typed events. We pull out version/tempo/channel counts and a
//! copy of the (first) arrangement's playlist: tracks, pattern and audio
//! clips, and the notes inside patterns — enough to draw a "screenshot".
//!
//! Event encoding: id < 64 → 1 data byte, < 128 → u16, < 192 → u32,
//! otherwise a LEB128 length followed by that many bytes.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::als::{AlsInfo, AlsMeta, ArrTrack, Arrangement, Clip, Note};

// Event ids (FL 20 – 25).
const CHANNEL_NEW: u8 = 64; // u16 iid
const PATTERN_NEW: u8 = 65; // u16 iid
const TEMPO_OLD: u8 = 66; // u16 bpm (FL < 12)
const ARRANGEMENT_NEW: u8 = 99; // u16 iid
const CHANNEL_COLOR: u8 = 128; // u32 RGBA bytes
const PATTERN_COLOR: u8 = 150; // u32 RGBA bytes
const TEMPO: u8 = 156; // u32 bpm × 1000
const VERSION: u8 = 199; // ASCII "25.1.6.4669"
const PATTERN_NOTES: u8 = 224; // 24-byte records
const PLAYLIST: u8 = 233; // fixed-size records (size varies by version)
const TRACK_DATA: u8 = 238; // starts with u32 iid, u32 colour
const TRACK_NAME: u8 = 239; // UTF-16LE

/// Playlist item references at or above this are patterns.
const PATTERN_BASE: u16 = 20480;
/// FL's untouched clip colour (#485156): we swap in a palette colour instead,
/// otherwise every FL thumbnail is the same grey-blue.
const FL_DEFAULT: u32 = 0x485156;
const FALLBACK: [u32; 10] = [
    0x7FB0E0, 0x9ACD68, 0xE8A65A, 0xD97FB8, 0x6ED3C2, 0xE0D06A, 0xA48BE0, 0xE07B6E, 0x74C78E, 0xC8A27A,
];

enum Value<'a> {
    Int(u32),
    Data(&'a [u8]),
}

struct Events<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for Events<'a> {
    type Item = (u8, Value<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        let d = self.data;
        let id = *d.get(self.pos)?;
        self.pos += 1;
        let v = match id {
            0..=63 => {
                let v = *d.get(self.pos)? as u32;
                self.pos += 1;
                Value::Int(v)
            }
            64..=127 => {
                let v = u16::from_le_bytes(d.get(self.pos..self.pos + 2)?.try_into().ok()?) as u32;
                self.pos += 2;
                Value::Int(v)
            }
            128..=191 => {
                let v = u32::from_le_bytes(d.get(self.pos..self.pos + 4)?.try_into().ok()?);
                self.pos += 4;
                Value::Int(v)
            }
            _ => {
                let mut len = 0usize;
                let mut shift = 0;
                loop {
                    let b = *d.get(self.pos)?;
                    self.pos += 1;
                    len |= ((b & 0x7f) as usize) << shift;
                    shift += 7;
                    if b < 0x80 || shift > 28 {
                        break;
                    }
                }
                let v = d.get(self.pos..self.pos + len)?;
                self.pos += len;
                Value::Data(v)
            }
        };
        Some((id, v))
    }
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// FL stores colours as little-endian RGBA bytes.
fn rgba_to_rgb(v: u32) -> u32 {
    let [r, g, b, _] = v.to_le_bytes();
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units).trim_end_matches('\0').trim().to_string()
}

#[derive(Default)]
struct Pattern {
    color: Option<u32>,
    /// (position ticks, length ticks, key)
    notes: Vec<(u32, u32, u8)>,
}

pub fn read(path: &Path) -> Option<AlsInfo> {
    let data = fs::read(path).ok()?;
    if data.get(..4)? != b"FLhd" {
        return None;
    }
    let header_len = u32_at(&data, 4) as usize;
    let channels = u16_at(&data, 10) as u32;
    let ppq = (u16_at(&data, 12) as f32).max(1.0);
    let body = 8 + header_len;
    if data.get(body..body + 4)? != b"FLdt" {
        return None;
    }

    let mut meta = AlsMeta { channels, ..Default::default() };
    let mut patterns: HashMap<u16, Pattern> = HashMap::new();
    let mut cur_pattern: Option<u16> = None;
    let mut channel_colors: HashMap<u16, u32> = HashMap::new();
    let mut cur_channel: Option<u16> = None;
    // Per-track (index = iid - 1) colour and name.
    let mut track_info: HashMap<u32, (u32, String)> = HashMap::new();
    let mut cur_track: Option<u32> = None;
    let mut playlist: Option<&[u8]> = None;
    let mut arrangements = 0;

    for (id, v) in (Events { data: &data, pos: body + 8 }) {
        match (id, v) {
            (VERSION, Value::Data(b)) => {
                let s = String::from_utf8_lossy(b).trim_end_matches('\0').to_string();
                // "25.1.6.4669" → "FL Studio 25.1.6"
                let short: Vec<&str> = s.split('.').take(3).collect();
                meta.version = format!("FL Studio {}", short.join("."));
            }
            (TEMPO, Value::Int(t)) => meta.tempo = Some(t as f32 / 1000.0),
            (TEMPO_OLD, Value::Int(t)) if meta.tempo.is_none() => meta.tempo = Some(t as f32),
            (CHANNEL_NEW, Value::Int(i)) => cur_channel = Some(i as u16),
            (CHANNEL_COLOR, Value::Int(c)) => {
                if let Some(ch) = cur_channel {
                    channel_colors.insert(ch, rgba_to_rgb(c));
                }
            }
            (PATTERN_NEW, Value::Int(i)) => {
                cur_pattern = Some(i as u16);
                patterns.entry(i as u16).or_default();
            }
            (PATTERN_COLOR, Value::Int(c)) => {
                if let Some(p) = cur_pattern {
                    patterns.entry(p).or_default().color = Some(rgba_to_rgb(c));
                }
            }
            (PATTERN_NOTES, Value::Data(b)) => {
                if let Some(p) = cur_pattern {
                    let pat = patterns.entry(p).or_default();
                    for n in b.chunks_exact(24) {
                        let key = u16_at(n, 12).min(127) as u8;
                        pat.notes.push((u32_at(n, 0), u32_at(n, 8), key));
                    }
                }
            }
            (ARRANGEMENT_NEW, Value::Int(_)) => arrangements += 1,
            (PLAYLIST, Value::Data(b)) => {
                // Only the first arrangement is drawn.
                if playlist.is_none() && arrangements <= 1 {
                    playlist = Some(b);
                }
            }
            (TRACK_DATA, Value::Data(b)) if b.len() >= 8 => {
                // Track data belongs to the first arrangement until a second one starts.
                if arrangements <= 1 {
                    let iid = u32_at(b, 0);
                    cur_track = Some(iid);
                    track_info.entry(iid).or_insert((rgba_to_rgb(u32_at(b, 4)), String::new()));
                }
            }
            (TRACK_NAME, Value::Data(b)) => {
                if let (Some(t), true) = (cur_track, arrangements <= 1) {
                    if let Some(info) = track_info.get_mut(&t) {
                        info.1 = utf16(b);
                    }
                }
            }
            _ => {}
        }
    }
    meta.patterns = patterns.len() as u32;
    if meta.version.is_empty() {
        meta.version = "FL Studio".into();
    }

    let arrangement = playlist.map(|b| build(b, ppq, &patterns, &channel_colors, &track_info)).unwrap_or_default();
    Some(AlsInfo { meta, arrangement })
}

/// Playlist records grew over the years: 32 bytes (FL ≤ 20), 60 (FL 21), 80 (FL 24+).
fn item_size(b: &[u8]) -> Option<usize> {
    [80, 60, 32].into_iter().find(|&size| {
        if b.len() % size != 0 || b.is_empty() {
            return false;
        }
        let n = b.len() / size;
        let ok = (0..n).filter(|k| u16_at(b, k * size + 4) == PATTERN_BASE).count();
        ok * 10 >= n * 9
    })
}

fn build(
    b: &[u8],
    ppq: f32,
    patterns: &HashMap<u16, Pattern>,
    channel_colors: &HashMap<u16, u32>,
    track_info: &HashMap<u32, (u32, String)>,
) -> Arrangement {
    let Some(size) = item_size(b) else { return Arrangement::default() };
    let pick = |c: Option<u32>, seed: u16| match c {
        Some(c) if c != FL_DEFAULT => c,
        _ => FALLBACK[seed as usize % FALLBACK.len()],
    };

    // track index → clips
    let mut rows: HashMap<u16, Vec<Clip>> = HashMap::new();
    for item in b.chunks_exact(size) {
        let pos = u32_at(item, 0) as f32;
        let index = u16_at(item, 6);
        let len = u32_at(item, 8) as f32;
        let track = 499u16.saturating_sub(u16_at(item, 12));
        let start_offset = i32::from_le_bytes(item[24..28].try_into().unwrap());
        if len <= 0.0 {
            continue;
        }
        let start = pos / ppq;
        let end = (pos + len) / ppq;

        let clip = if index >= PATTERN_BASE {
            let id = index - PATTERN_BASE;
            let pat = patterns.get(&id);
            let offset = start_offset.max(0) as f32;
            let notes = pat
                .map(|p| {
                    p.notes
                        .iter()
                        .filter_map(|&(np, nl, key)| {
                            let rel = np as f32 - offset;
                            (rel >= 0.0 && rel < len).then(|| Note {
                                start: (pos + rel) / ppq,
                                len: (nl as f32).min(len - rel) / ppq,
                                key,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Clip { start, end, color: pick(pat.and_then(|p| p.color), id), disabled: false, notes }
        } else {
            Clip {
                start,
                end,
                color: pick(channel_colors.get(&index).copied(), index.wrapping_add(5)),
                disabled: false,
                notes: vec![],
            }
        };
        rows.entry(track).or_default().push(clip);
    }

    let mut order: Vec<u16> = rows.keys().copied().collect();
    order.sort();
    let tracks: Vec<ArrTrack> = order
        .into_iter()
        .map(|t| {
            let mut clips = rows.remove(&t).unwrap_or_default();
            clips.sort_by(|a, b| a.start.total_cmp(&b.start));
            let (color, name) = track_info.get(&(t as u32 + 1)).cloned().unwrap_or((FL_DEFAULT, String::new()));
            let has_notes = clips.iter().any(|c| !c.notes.is_empty());
            // Untouched tracks take the colour of their first clip, like a header strip.
            let color = if color == FL_DEFAULT { clips.first().map(|c| c.color).unwrap_or(color) } else { color };
            ArrTrack { name, color, midi: has_notes, clips }
        })
        .collect();
    let length = tracks.iter().flat_map(|t| t.clips.iter().map(|c| c.end)).fold(0.0f32, f32::max);
    Arrangement { tracks, length }
}
