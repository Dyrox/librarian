//! Finds installed DAWs, borrows their real app icon for "Open" buttons and
//! opens projects with a specific app when the user picked one.

#[cfg_attr(all(unix, not(target_os = "macos")), allow(unused_imports))]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Daw {
    Ableton,
    FlStudio,
}

impl Daw {
    pub const ALL: [Daw; 2] = [Daw::Ableton, Daw::FlStudio];

    pub fn name(self) -> &'static str {
        match self {
            Daw::Ableton => "Ableton Live",
            Daw::FlStudio => "FL Studio",
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn slug(self) -> &'static str {
        match self {
            Daw::Ableton => "ableton",
            Daw::FlStudio => "fl",
        }
    }

    /// Which DAW a project file belongs to.
    pub fn of_file(path: &Path) -> Option<Daw> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "als" => Some(Daw::Ableton),
            "flp" => Some(Daw::FlStudio),
            _ => None,
        }
    }
}

/// "Ableton Live 12 Suite" from `/Applications/Ableton Live 12 Suite.app`.
pub fn app_name(app: &Path) -> String {
    app.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Opens `file` with `app`, or with the system's default app for it.
pub fn open_with(app: Option<&Path>, file: &Path) {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        if let Some(app) = app {
            cmd.arg("-a").arg(app);
        }
        let _ = cmd.arg(file).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = match app {
            Some(app) => Command::new(app).arg(file).spawn(),
            None => Command::new("cmd").args(["/C", "start", ""]).arg(file).spawn(),
        };
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = match app {
            Some(app) => Command::new(app).arg(file).spawn(),
            None => Command::new("xdg-open").arg(file).spawn(),
        };
    }
}

/// Looks for the newest installed copy of `daw`.
#[cfg(target_os = "macos")]
pub fn find_app(daw: Daw) -> Option<PathBuf> {
    let prefix = daw.name();
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    let mut found: Vec<PathBuf> = roots
        .iter()
        .filter_map(|r| fs::read_dir(r).ok())
        .flat_map(|it| it.flatten())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            name.starts_with(prefix) && name.ends_with(".app")
        })
        .collect();
    // "Ableton Live 12 Suite" sorts after "Ableton Live 11 …": newest wins.
    found.sort();
    found.pop()
}

#[cfg(target_os = "windows")]
pub fn find_app(daw: Daw) -> Option<PathBuf> {
    // Live: C:\ProgramData\Ableton\Live 12 Suite\Program\Ableton Live 12 Suite.exe
    // FL:   C:\Program Files\Image-Line\FL Studio 2025\FL64.exe
    let (root, exe): (PathBuf, fn(&Path) -> Option<PathBuf>) = match daw {
        Daw::Ableton => (PathBuf::from(r"C:\ProgramData\Ableton"), |dir| {
            let name = dir.file_name()?.to_string_lossy().into_owned();
            Some(dir.join("Program").join(format!("Ableton {name}.exe")))
        }),
        Daw::FlStudio => (PathBuf::from(r"C:\Program Files\Image-Line"), |dir| Some(dir.join("FL64.exe"))),
    };
    let mut found: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|e| exe(&e.path()))
        .filter(|p| p.exists())
        .collect();
    found.sort();
    found.pop()
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn find_app(_daw: Daw) -> Option<PathBuf> {
    None
}

/// Returns a PNG of the app's icon, extracted once and cached.
#[cfg(target_os = "macos")]
pub fn icon_png(daw: Daw, app: &Path) -> Option<PathBuf> {
    let out_dir = crate::cache::cache_subdir("icons")?;
    // Key the cache on the app's own name so upgrading Live refreshes it.
    let stem = app.file_stem()?.to_string_lossy().replace(' ', "_");
    let out = out_dir.join(format!("{}-{}.png", daw.slug(), stem));
    if out.exists() {
        return Some(out);
    }

    let plist = app.join("Contents/Info.plist");
    let icon_name = Command::new("plutil")
        .args(["-extract", "CFBundleIconFile", "raw"])
        .arg(&plist)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "app".into());
    let mut icns = app.join("Contents/Resources").join(&icon_name);
    if icns.extension().is_none() {
        icns.set_extension("icns");
    }

    let ok = Command::new("sips")
        .args(["-s", "format", "png", "-Z", "96"])
        .arg(&icns)
        .arg("--out")
        .arg(&out)
        .output()
        .is_ok_and(|o| o.status.success());
    ok.then_some(out)
}

#[cfg(not(target_os = "macos"))]
pub fn icon_png(_daw: Daw, _app: &Path) -> Option<PathBuf> {
    // TODO(windows): pull the icon resource out of the DAW's .exe.
    None
}
