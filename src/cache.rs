//! Tiny on-disk cache keyed by (path, mtime, size), so relaunches are instant.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::als::AlsInfo;
use crate::apps::Daw;
use crate::i18n::Lang;
use crate::settings::ThemePref;
use crate::peaks::Peaks;

/// Folder name under Application Support / Caches (or %APPDATA% etc.).
pub const APP: &str = "Librarian";

/// Librarian used to be called Lumen. Moves its settings and caches over
/// once, if the old folders are really ours (another app is called Lumen).
pub fn migrate_from_lumen() {
    if portable_dir().is_some() {
        return;
    }
    let moves = [
        (dirs::config_dir(), &["prefs.txt", "root.txt", "tags.txt", "heard.txt"][..]),
        (dirs::cache_dir(), &["project", "peaks", "icons"][..]),
    ];
    for (base, ours) in moves {
        let Some(base) = base else { continue };
        let (old, new) = (base.join("Lumen"), base.join(APP));
        if !new.exists() && ours.iter().any(|f| old.join(f).exists()) {
            let _ = fs::rename(&old, &new);
        }
    }
}

/// Portable mode: a `LibrarianData` folder next to the executable (the
/// Windows portable zip ships with one) keeps everything there instead of
/// in the user's profile, so the app can live on a USB stick.
fn portable_dir() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("LibrarianData");
    dir.is_dir().then_some(dir)
}

fn base() -> Option<PathBuf> {
    match portable_dir() {
        Some(p) => Some(p.join("cache")),
        None => Some(dirs::cache_dir()?.join(APP)),
    }
}

/// Cache subfolder, created on demand.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn cache_subdir(name: &str) -> Option<PathBuf> {
    let dir = base()?.join(name);
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn key(path: &Path) -> Option<String> {
    let md = fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    mtime.hash(&mut h);
    md.len().hash(&mut h);
    Some(format!("{:016x}", h.finish()))
}

fn slot(kind: &str, path: &Path) -> Option<PathBuf> {
    let dir = base()?.join(kind);
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join(key(path)?))
}

/// Parsed project info for a `.als` or `.flp`, cached by file identity.
pub fn als(path: &Path) -> Option<AlsInfo> {
    let slot = slot("project", path);
    if let Some(s) = &slot {
        if let Ok(bytes) = fs::read(s) {
            if let Some(info) = AlsInfo::from_bytes(&bytes) {
                return Some(info);
            }
        }
    }
    let is_flp = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("flp"));
    let info = if is_flp { crate::flp::read(path)? } else { crate::als::read(path)? };
    if let Some(s) = slot {
        let _ = fs::write(s, info.to_bytes());
    }
    Some(info)
}

pub fn peaks(path: &Path) -> Option<Peaks> {
    let slot = slot("peaks", path);
    if let Some(s) = &slot {
        if let Ok(bytes) = fs::read(s) {
            if let Some(p) = Peaks::from_bytes(&bytes) {
                return Some(p);
            }
        }
    }
    let p = crate::peaks::compute(path)?;
    if let Some(s) = slot {
        let _ = fs::write(s, p.to_bytes());
    }
    Some(p)
}

// ---- preferences ----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Prefs {
    pub root: Option<PathBuf>,
    pub grid: bool,
    pub sort_by_name: bool,
    pub window: Option<(f32, f32)>,
    /// Issue keys the user chose to ignore.
    pub ignored: Vec<String>,
    pub lang: Lang,
    pub theme: ThemePref,
    /// DAW apps picked by hand (otherwise found automatically).
    pub app_ableton: Option<PathBuf>,
    pub app_fl: Option<PathBuf>,
    /// Output device name; None = follow the system default.
    pub device: Option<String>,
    /// Play the MP3 rather than the WAV when a bounce has both.
    pub prefer_mp3: bool,
    pub watch: bool,
    pub new_marks: bool,
    /// Keep tags in `<root>/.librarian-tags.txt` instead of the config folder.
    pub tags_in_folder: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            root: None,
            grid: true,
            sort_by_name: false,
            window: None,
            ignored: Vec::new(),
            lang: Lang::System,
            theme: ThemePref::System,
            app_ableton: None,
            app_fl: None,
            device: None,
            prefer_mp3: false,
            watch: true,
            new_marks: true,
            tags_in_folder: false,
        }
    }
}

impl Prefs {
    pub fn app(&self, daw: Daw) -> Option<&PathBuf> {
        match daw {
            Daw::Ableton => self.app_ableton.as_ref(),
            Daw::FlStudio => self.app_fl.as_ref(),
        }
    }

    pub fn set_app(&mut self, daw: Daw, path: Option<PathBuf>) {
        match daw {
            Daw::Ableton => self.app_ableton = path,
            Daw::FlStudio => self.app_fl = path,
        }
    }
}

/// Total size of the thumbnail/waveform caches.
pub fn cache_size() -> u64 {
    fn walk(dir: &Path) -> u64 {
        fs::read_dir(dir)
            .map(|it| {
                it.flatten()
                    .map(|e| match e.metadata() {
                        Ok(m) if m.is_dir() => walk(&e.path()),
                        Ok(m) => m.len(),
                        Err(_) => 0,
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    base().map(|b| walk(&b.join("project")) + walk(&b.join("peaks"))).unwrap_or(0)
}

pub fn clear_cache() {
    if let Some(b) = base() {
        let _ = fs::remove_dir_all(b.join("project"));
        let _ = fs::remove_dir_all(b.join("peaks"));
    }
}

pub fn settings_dir() -> Option<PathBuf> {
    config_dir()
}

/// Where preferences, tags and listening history live (created on demand).
pub fn config_dir() -> Option<PathBuf> {
    let dir = match portable_dir() {
        Some(p) => p,
        None => dirs::config_dir()?.join(APP),
    };
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn load_prefs() -> Prefs {
    let mut prefs = Prefs::default();
    let Some(dir) = config_dir() else { return prefs };
    if let Ok(text) = fs::read_to_string(dir.join("prefs.txt")) {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "root" => prefs.root = Some(PathBuf::from(v)).filter(|p| p.is_dir()),
                "view" => prefs.grid = v == "grid",
                "sort" => prefs.sort_by_name = v == "name",
                "ignore" => prefs.ignored.push(v.to_string()),
                "lang" => prefs.lang = Lang::parse(v),
                "theme" => prefs.theme = ThemePref::parse(v),
                "app.ableton" => prefs.app_ableton = Some(PathBuf::from(v)).filter(|p| p.exists()),
                "app.fl" => prefs.app_fl = Some(PathBuf::from(v)).filter(|p| p.exists()),
                "device" => prefs.device = Some(v.to_string()).filter(|v| !v.is_empty()),
                "prefer" => prefs.prefer_mp3 = v == "mp3",
                "watch" => prefs.watch = v != "0",
                "new_marks" => prefs.new_marks = v != "0",
                "tags_in_folder" => prefs.tags_in_folder = v == "1",
                "window" => {
                    prefs.window = v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                }
                _ => {}
            }
        }
    } else if let Ok(old) = fs::read_to_string(dir.join("root.txt")) {
        // v0.1 stored only the root folder.
        prefs.root = Some(PathBuf::from(old.trim())).filter(|p| p.is_dir());
    }
    prefs
}

pub fn save_prefs(prefs: &Prefs) {
    let Some(dir) = config_dir() else { return };
    let mut out = String::new();
    if let Some(r) = &prefs.root {
        out += &format!("root={}\n", r.display());
    }
    out += &format!("view={}\n", if prefs.grid { "grid" } else { "list" });
    out += &format!("sort={}\n", if prefs.sort_by_name { "name" } else { "recent" });
    if let Some((w, h)) = prefs.window {
        out += &format!("window={}x{}\n", w.round(), h.round());
    }
    out += &format!("lang={}\n", prefs.lang.as_str());
    out += &format!("theme={}\n", prefs.theme.as_str());
    if let Some(a) = &prefs.app_ableton {
        out += &format!("app.ableton={}\n", a.display());
    }
    if let Some(a) = &prefs.app_fl {
        out += &format!("app.fl={}\n", a.display());
    }
    if let Some(d) = &prefs.device {
        out += &format!("device={d}\n");
    }
    out += &format!("prefer={}\n", if prefs.prefer_mp3 { "mp3" } else { "wav" });
    out += &format!("watch={}\n", prefs.watch as u8);
    out += &format!("new_marks={}\n", prefs.new_marks as u8);
    out += &format!("tags_in_folder={}\n", prefs.tags_in_folder as u8);
    for key in &prefs.ignored {
        out += &format!("ignore={key}\n");
    }
    let _ = fs::write(dir.join("prefs.txt"), out);
}
