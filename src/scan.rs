//! Walks a root folder, finds Ableton Live projects, and notes everything
//! that doesn't follow the folder conventions ("issues").
//!
//! Convention (Live creates this layout itself):
//!   `<name> Project/`
//!       `<name>.als`            ← the set (others may sit beside it)
//!       `*.mp3 | *.wav`         ← demos bounced by hand
//!       `Backup/`               ← ignored (contains older .als copies)
//!       `Samples/`              ← ignored (contains recorded/consolidated audio)
//!       `Ableton Project Info/` ← marks the folder as a Live project
//!
//! Anything inside a folder named `legacy` is skipped (old projects kept aside).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use unicode_normalization::UnicodeNormalization;

use crate::als::{AlsMeta, Arrangement};
pub use crate::apps::Daw;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: SystemTime,
    pub size: u64,
}

/// One bounce, possibly exported in several formats (`test3.wav` + `test3.mp3`).
#[derive(Debug, Clone)]
pub struct Version {
    /// File name without extension, e.g. `test3`.
    pub label: String,
    /// Lossless first: that's the one we play.
    pub files: Vec<FileEntry>,
    pub modified: SystemTime,
}

impl Version {
    pub fn file(&self) -> &FileEntry {
        &self.files[0]
    }

    /// e.g. "WAV + MP3"
    pub fn formats(&self) -> String {
        self.files.iter().map(|f| ext(&f.name).to_uppercase()).collect::<Vec<_>>().join(" + ")
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub daw: Daw,
    /// The project folder — or, for an FL project sharing its folder with
    /// others, the .flp itself. Unique per project; used as its identity.
    pub dir: PathBuf,
    /// Display name: folder name without " Project", keeping any suffix such
    /// as a release title — `dnb Project (夢の幻覚)` → `dnb (夢の幻覚)`.
    pub name: String,
    /// The name Live gave the project (and its set) — `dnb`.
    pub base: String,
    /// Parent folder relative to the scan root, e.g. `2026/2026-10`.
    pub location: String,
    /// First path component under the root, used for the sidebar groups.
    pub group: String,
    /// Project files (.als / .flp) at the top level of the project, newest first.
    pub sets: Vec<FileEntry>,
    /// Bounced demos at the top level of the project, newest first.
    pub demos: Vec<FileEntry>,
    /// The same demos grouped by name across formats, newest first.
    pub versions: Vec<Version>,
    /// Most recent modification among sets and demos.
    pub modified: SystemTime,
    pub meta: Option<AlsMeta>,
    pub arrangement: Option<Arc<Arrangement>>,
    /// Tags saved in the project (see `tags`).
    pub tags: Vec<String>,
}

impl Project {
    /// The set Live opens by default: `<name>.als`, else the newest one.
    pub fn main_set(&self) -> Option<&FileEntry> {
        self.named_set().or_else(|| self.sets.first())
    }

    fn named_set(&self) -> Option<&FileEntry> {
        let wanted = self.base.trim().to_lowercase();
        self.sets.iter().find(|s| stem_of(&s.name).trim().to_lowercase() == wanted)
    }

    pub fn latest_demo(&self) -> Option<&FileEntry> {
        self.versions.first().map(Version::file)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IssueKind {
    /// A project folder (has `Ableton Project Info`) with no .als in it.
    EmptyProject,
    /// The .als can't be decompressed/parsed (filled in after loading).
    UnreadableSet,
    /// Audio files sitting outside any project folder.
    LooseBounces,
    /// A .als sitting outside a project folder.
    LooseSet,
    /// Audio inside a project's own subfolder (not Samples/Backup): not previewed.
    NestedBounces,
    /// No `<name>.als` matching the folder name (usually a renamed folder).
    NameMismatch,
    /// Several unrelated .flp files sharing one folder.
    LooseFlp,
}

impl IssueKind {
    pub const ALL: [IssueKind; 7] = [
        IssueKind::EmptyProject,
        IssueKind::UnreadableSet,
        IssueKind::LooseBounces,
        IssueKind::LooseSet,
        IssueKind::LooseFlp,
        IssueKind::NestedBounces,
        IssueKind::NameMismatch,
    ];

    pub fn title(self) -> &'static str {
        match self {
            IssueKind::EmptyProject => "Project folder without a Live set",
            IssueKind::UnreadableSet => "Live set can't be read",
            IssueKind::LooseBounces => "Bounces outside a project",
            IssueKind::LooseSet => "Live set outside a project folder",
            IssueKind::NestedBounces => "Bounces tucked in a subfolder",
            IssueKind::NameMismatch => "Set name doesn't match its folder",
            IssueKind::LooseFlp => "FL projects sharing a folder",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            IssueKind::EmptyProject => "The folder is marked as a Live project but has no .als at its top level.",
            IssueKind::UnreadableSet => "The file couldn't be decompressed or parsed. It may be truncated or from an unsupported version.",
            IssueKind::LooseBounces => "Audio that isn't next to any .als, so Librarian can't attach it to a project. Move it into its project folder.",
            IssueKind::LooseSet => "Live keeps samples and backups next to the set in a \"… Project\" folder. A lone .als can lose track of its files.",
            IssueKind::NestedBounces => "Librarian only previews bounces at the top level of a project. These are inside a subfolder.",
            IssueKind::NameMismatch => "There's no \"<folder name>.als\". Usually the folder was renamed after creation; Librarian uses the newest set.",
            IssueKind::LooseFlp => "Each FL project should have its own folder with its bounces next to it (like hypertrance/). Here several share one folder, so bounces can only be matched by file name.",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            IssueKind::EmptyProject => "empty",
            IssueKind::UnreadableSet => "unreadable",
            IssueKind::LooseBounces => "loose-audio",
            IssueKind::LooseSet => "loose-set",
            IssueKind::NestedBounces => "nested-audio",
            IssueKind::NameMismatch => "name",
            IssueKind::LooseFlp => "loose-flp",
        }
    }

    pub fn is_error(self) -> bool {
        matches!(self, IssueKind::EmptyProject | IssueKind::UnreadableSet)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IssueFile {
    pub name: String,
    /// For loose bounces: the project this file probably belongs to.
    pub belongs_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub kind: IssueKind,
    /// The folder (or file) to reveal.
    pub path: PathBuf,
    /// Path relative to the scan root, for display.
    pub location: String,
    pub files: Vec<IssueFile>,
    /// The project this issue is about, if any (to badge its card).
    pub project: Option<PathBuf>,
}

impl Issue {
    /// Stable identity for the ignore list.
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind.slug(), self.path.display())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub projects: Vec<Project>,
    pub issues: Vec<Issue>,
    /// The root itself couldn't be read because of permissions.
    pub denied: bool,
}

const AUDIO_EXTS: &[&str] = &["mp3", "wav", "aif", "aiff", "flac"];
const SKIP_DIRS: &[&str] = &["Samples", "Backup", "Ableton Project Info", "node_modules"];
/// Other apps' document packages: their media is theirs, not loose.
const PACKAGE_EXTS: &[&str] = &[".app", ".logicx", ".band", ".logic"];

/// A `legacy/` folder (any case, at any depth) holds old projects the user
/// has retired: neither listed nor checked for issues.
pub fn is_legacy(name: &str) -> bool {
    name.eq_ignore_ascii_case("legacy")
}

fn is_package(name: &str) -> bool {
    let lower = name.to_lowercase();
    PACKAGE_EXTS.iter().any(|e| lower.ends_with(e))
}

/// Folders that hold audio on purpose (exported stems, sample/loop stashes).
fn is_audio_stash(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["stem", "sample", "loop"].iter().any(|w| lower.contains(w))
}

pub fn scan(root: &Path) -> ScanResult {
    let mut out = ScanResult::default();
    if let Err(e) = fs::read_dir(root) {
        out.denied = e.kind() == std::io::ErrorKind::PermissionDenied;
        return out;
    }
    walk(root, root, 0, &mut out);
    for p in &mut out.projects {
        p.tags = crate::tags::read(&p.dir);
    }
    out.projects.sort_by(|a, b| b.modified.cmp(&a.modified));
    guess_owners(&mut out);
    out
}

struct Listing {
    dirs: Vec<(String, PathBuf)>,
    files: Vec<FileEntry>,
}

fn list(dir: &Path) -> Option<Listing> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let name: String = entry.file_name().to_string_lossy().nfc().collect();
        if name.starts_with('.') {
            continue;
        }
        if ft.is_dir() {
            // Symlinks are neither dirs nor files here: no cycles.
            dirs.push((name, entry.path()));
        } else if ft.is_file() {
            let Ok(md) = entry.metadata() else { continue };
            files.push(FileEntry {
                path: entry.path(),
                name,
                modified: md.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                size: md.len(),
            });
        }
    }
    Some(Listing { dirs, files })
}

fn ext(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_audio(f: &FileEntry) -> bool {
    f.size > 0 && AUDIO_EXTS.contains(&ext(&f.name).as_str())
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().nfc().collect::<String>())
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default()
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut ScanResult) {
    if depth > 12 {
        return;
    }
    let Some(listing) = list(dir) else { return };

    for (name, path) in &listing.dirs {
        if SKIP_DIRS.contains(&name.as_str()) || is_package(name) || is_legacy(name) {
            continue;
        }
        if !read_project(root, path, name, out) {
            walk(root, path, depth + 1, out);
        }
    }

    let stash = dir.file_name().is_some_and(|n| is_audio_stash(&n.to_string_lossy()));
    if stash {
        return;
    }
    let mut loose_audio: Vec<&FileEntry> = listing.files.iter().filter(|f| is_audio(f)).collect();

    // FL Studio projects: .flp files in this folder.
    let flps: Vec<&FileEntry> = listing.files.iter().filter(|f| ext(&f.name) == "flp").collect();
    if !flps.is_empty() {
        let folder = dir.file_name().map(|n| norm(&n.to_string_lossy())).unwrap_or_default();
        let named_after_folder = flps.iter().any(|f| norm(stem_of(&f.name)) == folder);
        if dir != root && (flps.len() == 1 || named_after_folder) {
            // `hypertrance/hypertrance.flp`: the folder is the project, every bounce in it belongs to it.
            out.projects.push(fl_project(root, dir, dir, &flps, &loose_audio));
            loose_audio.clear();
        } else {
            // Several unrelated .flp files in one folder: one project each; claim bounces by name.
            for f in &flps {
                let key = norm(stem_of(&f.name));
                let (mine, rest): (Vec<&FileEntry>, Vec<&FileEntry>) = loose_audio
                    .into_iter()
                    .partition(|a| key.chars().count() >= 2 && norm(stem_of(&a.name)).starts_with(&key));
                loose_audio = rest;
                out.projects.push(fl_project(root, dir, &f.path, &[*f], &mine));
            }
            out.issues.push(Issue {
                kind: IssueKind::LooseFlp,
                path: dir.to_path_buf(),
                location: rel(root, dir),
                files: flps.iter().map(|f| IssueFile { name: f.name.clone(), belongs_to: None }).collect(),
                project: None,
            });
        }
    }

    if !loose_audio.is_empty() {
        out.issues.push(Issue {
            kind: IssueKind::LooseBounces,
            path: dir.to_path_buf(),
            location: rel(root, dir),
            files: loose_audio.iter().map(|f| IssueFile { name: f.name.clone(), belongs_to: None }).collect(),
            project: None,
        });
    }
    for f in listing.files.iter().filter(|f| ext(&f.name) == "als") {
        out.issues.push(Issue {
            kind: IssueKind::LooseSet,
            path: f.path.clone(),
            location: rel(root, &f.path),
            files: vec![],
            project: None,
        });
    }
}

fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// Builds an FL Studio project. `id` is the folder for a folder-per-project
/// layout, or the .flp itself when several share a folder.
fn fl_project(root: &Path, folder: &Path, id: &Path, flps: &[&FileEntry], audio: &[&FileEntry]) -> Project {
    let mut sets: Vec<FileEntry> = flps.iter().map(|f| (*f).clone()).collect();
    sets.sort_by(|a, b| b.modified.cmp(&a.modified));
    let mut demos: Vec<FileEntry> = audio.iter().map(|f| (*f).clone()).collect();
    demos.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| lossless_rank(&a.name).cmp(&lossless_rank(&b.name)))
    });
    let folder_name = folder.file_name().map(|n| n.to_string_lossy().nfc().collect::<String>()).unwrap_or_default();
    // Prefer the .flp named after the folder, else the newest.
    let main = sets
        .iter()
        .find(|s| norm(stem_of(&s.name)) == norm(&folder_name))
        .or_else(|| sets.first())
        .map(|s| stem_of(&s.name).trim().to_string())
        .unwrap_or_default();
    let modified = sets.iter().chain(demos.iter()).map(|f| f.modified).max().unwrap_or(SystemTime::UNIX_EPOCH);
    // Show where it lives: the parent of a project folder, or the folder of a loose .flp.
    let place = if id == folder { folder.parent().unwrap_or(folder) } else { folder };
    let location = rel(root, place);
    let group = place
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.components().next())
        .map(|c| c.as_os_str().to_string_lossy().nfc().collect())
        .unwrap_or_default();
    Project {
        daw: Daw::FlStudio,
        dir: id.to_path_buf(),
        name: main.clone(),
        base: main,
        location,
        group,
        sets,
        versions: group_versions(&demos),
        demos,
        modified,
        meta: None,
        arrangement: None,
        tags: Vec::new(),
    }
}

/// Returns true when `dir` is a project folder (healthy or not), so the
/// caller doesn't descend into it.
fn read_project(root: &Path, dir: &Path, folder_name: &str, out: &mut ScanResult) -> bool {
    let has_info = dir.join("Ableton Project Info").is_dir();
    if !folder_name.ends_with(" Project") && !has_info {
        return false;
    }
    // `dnb Project (夢の幻覚)`: base `dnb`, display `dnb (夢の幻覚)`.
    let (base, suffix) = match folder_name.find(" Project") {
        Some(i) => (&folder_name[..i], folder_name[i + " Project".len()..].trim()),
        None => (folder_name, ""),
    };
    let base: String = base.trim().nfc().collect();
    let name: String = if suffix.is_empty() { base.clone() } else { format!("{base} {suffix}").nfc().collect() };
    let Some(listing) = list(dir) else { return true };

    let mut sets: Vec<FileEntry> = listing.files.iter().filter(|f| ext(&f.name) == "als").cloned().collect();
    let mut demos: Vec<FileEntry> = listing.files.iter().filter(|f| is_audio(f)).cloned().collect();

    if sets.is_empty() {
        if listing.files.iter().any(|f| ext(&f.name) == "flp") {
            return true; // an FL project that happens to be called "… Project"
        }
        if has_info {
            out.issues.push(Issue {
                kind: IssueKind::EmptyProject,
                path: dir.to_path_buf(),
                location: rel(root, dir),
                files: demos.iter().map(|f| IssueFile { name: f.name.clone(), belongs_to: None }).collect(),
                project: None,
            });
            return true;
        }
        // Just a folder whose name ends in " Project": keep looking inside.
        return false;
    }

    sets.sort_by(|a, b| b.modified.cmp(&a.modified));
    // Newest first; when a .wav and .mp3 of the same bounce share a timestamp,
    // put the lossless one first.
    demos.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| lossless_rank(&a.name).cmp(&lossless_rank(&b.name)))
    });

    let modified = sets
        .iter()
        .chain(demos.iter())
        .map(|f| f.modified)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let parent_rel = dir.parent().and_then(|p| p.strip_prefix(root).ok());
    let location = dir.parent().map(|p| rel(root, p)).unwrap_or_default();
    let group = parent_rel
        .and_then(|p| p.components().next())
        .map(|c| c.as_os_str().to_string_lossy().nfc().collect())
        .unwrap_or_default();

    let project = Project {
        daw: Daw::Ableton,
        dir: dir.to_path_buf(),
        name,
        base,
        location,
        group,
        sets,
        versions: group_versions(&demos),
        demos,
        modified,
        meta: None,
        arrangement: None,
        tags: Vec::new(),
    };

    if project.named_set().is_none() {
        out.issues.push(Issue {
            kind: IssueKind::NameMismatch,
            path: dir.to_path_buf(),
            location: rel(root, dir),
            files: project.sets.iter().map(|s| IssueFile { name: s.name.clone(), belongs_to: None }).collect(),
            project: Some(dir.to_path_buf()),
        });
    }

    // Audio hiding in the project's own subfolders (Samples/Backup excluded).
    for (sub_name, sub) in &listing.dirs {
        if SKIP_DIRS.contains(&sub_name.as_str()) || is_package(sub_name) {
            continue;
        }
        let mut found = Vec::new();
        collect_audio(sub, 0, &mut found);
        if !found.is_empty() {
            out.issues.push(Issue {
                kind: IssueKind::NestedBounces,
                path: sub.clone(),
                location: rel(root, sub),
                files: found.into_iter().map(|name| IssueFile { name, belongs_to: None }).collect(),
                project: Some(dir.to_path_buf()),
            });
        }
    }

    out.projects.push(project);
    true
}

fn collect_audio(dir: &Path, depth: usize, out: &mut Vec<String>) {
    if depth > 3 || out.len() > 500 {
        return;
    }
    let Some(listing) = list(dir) else { return };
    out.extend(listing.files.iter().filter(|f| is_audio(f)).map(|f| f.name.clone()));
    for (name, path) in &listing.dirs {
        if !SKIP_DIRS.contains(&name.as_str()) {
            collect_audio(path, depth + 1, out);
        }
    }
}

/// Lowercase alphanumerics only (keeps CJK), for fuzzy name matching.
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// For each loose bounce, guess the project it was bounced from:
/// 1. a file named exactly like a project, or starting with its name
///    (`wahaha.mp3`, `piano 8.21(ver.2).wav`), closest project first;
/// 2. otherwise, if the folder holds exactly one project folder, that one.
fn guess_owners(out: &mut ScanResult) {
    let names: Vec<(String, &Project)> = out
        .projects
        .iter()
        .map(|p| (norm(&p.base), p))
        .filter(|(n, _)| n.chars().count() >= 3)
        .collect();

    for issue in out.issues.iter_mut().filter(|i| matches!(i.kind, IssueKind::LooseBounces | IssueKind::EmptyProject)) {
        let neighbours: Vec<&Project> =
            out.projects.iter().filter(|p| p.dir.parent() == Some(issue.path.as_path())).collect();
        let neighbour = (neighbours.len() == 1).then(|| neighbours[0].name.clone());

        for f in &mut issue.files {
            let stem = norm(Path::new(&f.name).file_stem().map(|s| s.to_string_lossy()).unwrap_or_default().as_ref());
            let best = names
                .iter()
                .filter(|(n, _)| !stem.is_empty() && stem.starts_with(n.as_str()))
                .max_by_key(|(n, p)| {
                    let shared = issue
                        .path
                        .components()
                        .zip(p.dir.components())
                        .take_while(|(a, b)| a == b)
                        .count();
                    (stem == *n, shared, n.len())
                });
            f.belongs_to = best.map(|(_, p)| p.name.clone()).or_else(|| neighbour.clone());
        }
    }
}

/// Groups `demos` (newest first) by file stem, case-insensitively.
fn group_versions(demos: &[FileEntry]) -> Vec<Version> {
    let mut out: Vec<Version> = Vec::new();
    for d in demos {
        let label = Path::new(&d.name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let key = label.to_lowercase();
        match out.iter_mut().find(|v| v.label.to_lowercase() == key) {
            Some(v) => {
                v.files.push(d.clone());
                v.modified = v.modified.max(d.modified);
            }
            None => out.push(Version { label, files: vec![d.clone()], modified: d.modified }),
        }
    }
    for v in &mut out {
        v.files.sort_by_key(|f| lossless_rank(&f.name));
    }
    out
}

fn lossless_rank(name: &str) -> u8 {
    match ext(name).as_str() {
        "wav" | "aif" | "aiff" | "flac" => 0,
        _ => 1,
    }
}

/// Puts the format the user prefers first in each version (that's the file
/// that plays). Default: lossless first.
pub fn order_formats(projects: &mut [Project], prefer_mp3: bool) {
    for p in projects {
        for v in &mut p.versions {
            v.files.sort_by_key(|f| {
                let lossless = lossless_rank(&f.name) == 0;
                if prefer_mp3 { lossless as u8 } else { !lossless as u8 }
            });
        }
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::*;

    #[test]
    fn legacy_folders_are_skipped() {
        let root = std::env::temp_dir().join(format!("librarian-legacy-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["a Project/Ableton Project Info", "Legacy/old Project/Ableton Project Info", "2023/legacy/x", "2023/b"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join("a Project/a.als"), b"x").unwrap();
        fs::write(root.join("Legacy/old Project/old.als"), b"x").unwrap();
        fs::write(root.join("2023/legacy/x/azimuth.flp"), b"x").unwrap();
        fs::write(root.join("2023/legacy/x/old.wav"), b"x").unwrap();
        fs::write(root.join("2023/b/b.flp"), b"x").unwrap();

        let out = scan(&root);
        let mut names: Vec<&str> = out.projects.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["a", "b"]);
        assert!(out.issues.iter().all(|i| !i.path.to_string_lossy().to_lowercase().contains("legacy")));
        let _ = fs::remove_dir_all(&root);
    }
}
