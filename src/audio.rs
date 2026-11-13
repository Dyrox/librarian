//! Single-voice audio preview built on rodio.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::cpal::{BufferSize, SupportedBufferSize};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

pub struct Audio {
    // The device sink is opened lazily on first play, so the app doesn't grab
    // the audio device until it's actually needed.
    out: Option<(MixerDeviceSink, Player)>,
    /// Name of the device `out` was opened on, to follow default-device changes.
    device: Option<String>,
    /// Device the user picked; None (or unplugged) = the system default.
    pub preferred: Option<String>,
    pub current: Option<PathBuf>,
    pub duration: Option<Duration>,
}

fn device_name(d: &rodio::cpal::Device) -> Option<String> {
    d.description().ok().map(|x| x.name().to_string())
}

/// Names of every output device, for the settings page.
pub fn output_devices() -> Vec<String> {
    let mut names: Vec<String> = rodio::cpal::default_host()
        .output_devices()
        .map(|it| it.filter_map(|d| device_name(&d)).collect())
        .unwrap_or_default();
    names.dedup();
    names
}

/// The preferred device if it's plugged in, else the system default.
fn pick_device(preferred: Option<&str>) -> Option<(rodio::cpal::Device, String)> {
    let host = rodio::cpal::default_host();
    if let Some(want) = preferred {
        if let Ok(mut devices) = host.output_devices() {
            if let Some(d) = devices.find(|d| device_name(d).as_deref() == Some(want)) {
                return Some((d, want.to_string()));
            }
        }
    }
    let d = host.default_output_device()?;
    let name = device_name(&d).unwrap_or_default();
    Some((d, name))
}

/// Opens `device` with a buffer size it actually supports. rodio's default
/// (~50 ms, e.g. 2048 frames) is rejected by Bluetooth headphones such as
/// AirPods, whose range tops out around 960.
fn open_sink(device: rodio::cpal::Device) -> Result<MixerDeviceSink, String> {
    let range = device.default_output_config().ok().map(|c| *c.buffer_size());
    let size = match range {
        Some(SupportedBufferSize::Range { min, max }) => BufferSize::Fixed(2048.clamp(min, max)),
        _ => BufferSize::Default,
    };
    DeviceSinkBuilder::from_device(device)
        .and_then(|b| b.with_buffer_size(size).open_stream())
        .or_else(|_| DeviceSinkBuilder::open_default_sink())
        .map_err(|e| format!("Audio output unavailable ({e})"))
}

impl Audio {
    pub fn new() -> Self {
        Self { out: None, device: None, preferred: None, current: None, duration: None }
    }

    fn player(&mut self) -> Result<&Player, String> {
        let (device, name) = pick_device(self.preferred.as_deref()).ok_or("No audio output device")?;
        if self.out.is_none() || Some(&name) != self.device.as_ref() {
            self.out = None;
            let mut sink = open_sink(device)?;
            sink.log_on_drop(false);
            let player = Player::connect_new(sink.mixer());
            self.out = Some((sink, player));
            self.device = Some(name);
        }
        Ok(&self.out.as_ref().unwrap().1)
    }

    pub fn play(&mut self, path: &Path, start: Option<Duration>) -> Result<(), String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let decoder = Decoder::try_from(file).map_err(|e| e.to_string())?;
        let total = rodio::Source::total_duration(&decoder);
        let player = self.player()?;
        player.clear();
        player.append(decoder);
        if let Some(at) = start {
            let _ = player.try_seek(at);
        }
        player.play();
        self.current = Some(path.to_path_buf());
        self.duration = total;
        Ok(())
    }

    pub fn toggle(&mut self) {
        if let Some((_, p)) = &self.out {
            if p.is_paused() { p.play() } else { p.pause() }
        }
    }

    pub fn stop(&mut self) {
        if let Some((_, p)) = &self.out {
            p.clear();
        }
        self.current = None;
        self.duration = None;
    }

    pub fn seek(&mut self, at: Duration) {
        if let Some((_, p)) = &self.out {
            let _ = p.try_seek(at);
        }
    }

    pub fn is_playing(&self) -> bool {
        match &self.out {
            Some((_, p)) => self.current.is_some() && !p.is_paused() && !p.empty(),
            None => false,
        }
    }

    pub fn is_paused(&self) -> bool {
        matches!(&self.out, Some((_, p)) if p.is_paused())
    }

    /// True once the current file has played through.
    pub fn finished(&self) -> bool {
        match &self.out {
            Some((_, p)) => self.current.is_some() && p.empty(),
            None => false,
        }
    }

    pub fn position(&self) -> Duration {
        match &self.out {
            Some((_, p)) => p.get_pos(),
            None => Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "lists real hardware"]
    fn list_devices() {
        println!("{:?}", super::output_devices());
    }
}
