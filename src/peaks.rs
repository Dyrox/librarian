//! Decodes an audio file once and reduces it to a small waveform overview.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const BUCKETS: usize = 240;

#[derive(Debug, Clone, PartialEq)]
pub struct Peaks {
    /// Peak amplitude (0..=1) per bucket.
    pub values: Vec<f32>,
    pub duration_secs: f32,
}

impl Peaks {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.values.len());
        out.extend_from_slice(&self.duration_secs.to_le_bytes());
        out.extend(self.values.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 4 + 8 {
            return None;
        }
        let duration_secs = f32::from_le_bytes(b[..4].try_into().ok()?);
        let values = b[4..].iter().map(|&v| v as f32 / 255.0).collect();
        Some(Self { values, duration_secs })
    }
}

pub fn compute(path: &Path) -> Option<Peaks> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?
        .clone();
    let sample_rate = track.codec_params.sample_rate? as f64;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .ok()?;

    // Collect per-frame peak (max |sample| across channels) at a coarse resolution
    // first, then fold into BUCKETS once the true length is known.
    const CHUNK: usize = 512;
    let mut coarse: Vec<f32> = Vec::with_capacity(16_384);
    let mut acc = 0f32;
    let mut acc_n = 0usize;
    let mut frames: u64 = 0;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let cap = decoded.capacity() as u64;
        let sb = match &mut sbuf {
            Some(sb) if sb.capacity() >= decoded.capacity() * channels => sb,
            _ => {
                sbuf = Some(SampleBuffer::new(cap, spec));
                sbuf.as_mut().unwrap()
            }
        };
        sb.copy_interleaved_ref(decoded);
        for frame in sb.samples().chunks_exact(channels) {
            let mut m = 0f32;
            for s in frame {
                m = m.max(s.abs());
            }
            acc = acc.max(m);
            acc_n += 1;
            if acc_n == CHUNK {
                coarse.push(acc);
                acc = 0.0;
                acc_n = 0;
            }
        }
        frames += (sb.samples().len() / channels) as u64;
    }
    if acc_n > 0 {
        coarse.push(acc);
    }
    if coarse.is_empty() {
        return None;
    }

    let mut values = vec![0f32; BUCKETS];
    for (i, v) in coarse.iter().enumerate() {
        let b = i * BUCKETS / coarse.len();
        values[b] = values[b].max(*v);
    }
    // Normalise so quiet demos still read well.
    let max = values.iter().cloned().fold(0f32, f32::max);
    if max > 0.0 {
        for v in &mut values {
            *v = (*v / max).min(1.0);
        }
    }

    Some(Peaks {
        values,
        duration_secs: (frames as f64 / sample_rate) as f32,
    })
}
