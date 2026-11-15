//! Project tags ("Finished", "Abandoned", …).
//!
//! Which tags a project has is saved inside the project itself, in a hidden
//! `.librarian-tags` file, so they survive moving or renaming the project,
//! syncing it, or copying it to another computer. A folder holding several
//! loose .flp projects keeps one line per .flp in that file.
//!
//! Custom tag definitions (name + colour) live in Librarian's config folder;
//! a tag name found in a project but unknown here is added back as custom.
//!
//! Older versions kept every assignment in one file keyed by path relative
//! to the scan root; those are moved into the projects on the next scan.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct TagDef {
    pub name: String,
    /// 0xRRGGBB
    pub color: u32,
    pub builtin: bool,
}

/// Display name: built-in tags are stored in English and shown translated.
pub fn label(name: &str) -> String {
    match BUILTIN.iter().find(|(n, _)| *n == name) {
        Some((n, _)) => crate::i18n::t(n).to_string(),
        None => name.to_string(),
    }
}

/// In the order they're listed: furthest along first.
pub const BUILTIN: [(&str, u32); 6] = [
    ("Released", 0x8B5CF6),
    ("Finished", 0x22A06B),
    ("In Progress", 0x3B82F6),
    ("Idea", 0xE0A800),
    ("Abandoned", 0x8A8F98),
    ("Other", 0x14B8A6),
];

/// Colours handed out to new custom tags, in order.
const CUSTOM: [u32; 7] = [0xEC4899, 0xF97316, 0x6366F1, 0x84CC16, 0xEF4444, 0x06B6D4, 0xA855F7];

#[derive(Debug, Clone, Default)]
pub struct Tags {
    pub defs: Vec<TagDef>,
    /// project key (path relative to the root) → tag names, as read from the projects.
    pub assigned: HashMap<String, Vec<String>>,
    /// Assignments from the old single-file format still waiting to be
    /// written into their projects (say, ones on a drive not plugged in).
    legacy: HashMap<String, Vec<String>>,
    /// Where the definitions (and leftover legacy assignments) are saved.
    path: Option<PathBuf>,
}

/// Hidden per-project tag file.
pub const PROJECT_FILE: &str = ".librarian-tags";
/// The old shared tag file inside the projects folder.
const OLD_FOLDER_FILE: &str = ".librarian-tags.txt";

/// Where the definitions are stored.
pub fn location() -> Option<PathBuf> {
    Some(crate::cache::config_dir()?.join("tags.txt"))
}

/// The tag file for a project and its line in it: `.` for a project folder,
/// the file name for a loose .flp sharing its folder.
fn file_for(project: &Path) -> Option<(PathBuf, String)> {
    if project.is_dir() {
        Some((project.join(PROJECT_FILE), ".".into()))
    } else {
        let name = project.file_name()?.to_string_lossy().into_owned();
        Some((project.parent()?.join(PROJECT_FILE), name))
    }
}

fn read_entries(file: &Path) -> Vec<(String, Vec<String>)> {
    let Ok(text) = fs::read_to_string(file) else { return Vec::new() };
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut it = l.split('\t');
            let key = it.next()?.to_string();
            let names: Vec<String> = it.filter(|s| !s.trim().is_empty()).map(String::from).collect();
            (!key.is_empty() && !names.is_empty()).then_some((key, names))
        })
        .collect()
}

/// Tags saved in a project.
pub fn read(project: &Path) -> Vec<String> {
    let Some((file, key)) = file_for(project) else { return Vec::new() };
    read_entries(&file).into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_default()
}

/// Saves a project's tags into it; no tags removes its line (and the file
/// once it's empty).
pub fn write(project: &Path, names: &[String]) -> std::io::Result<()> {
    let Some((file, key)) = file_for(project) else { return Ok(()) };
    let mut entries = read_entries(&file);
    entries.retain(|(k, _)| *k != key);
    if !names.is_empty() {
        entries.insert(0, (key, names.to_vec()));
    }
    if entries.is_empty() {
        return match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let mut out = String::from("# Librarian tags\n");
    for (k, v) in &entries {
        out += &format!("{k}\t{}\n", v.join("\t"));
    }
    write_hidden(&file, out.as_bytes())
}

#[cfg(windows)]
fn write_hidden(file: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    // A dot doesn't hide a file on Windows; the attribute does. It must be
    // passed on every write too: replacing a hidden file without it fails.
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .attributes(FILE_ATTRIBUTE_HIDDEN)
        .open(file)?
        .write_all(data)
}

#[cfg(not(windows))]
fn write_hidden(file: &Path, data: &[u8]) -> std::io::Result<()> {
    fs::write(file, data)
}

impl Tags {
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut tags = Tags {
            defs: BUILTIN
                .iter()
                .map(|(n, c)| TagDef { name: n.to_string(), color: *c, builtin: true })
                .collect(),
            path,
            ..Default::default()
        };
        if let Some(f) = tags.path.clone() {
            tags.merge_file(&f);
        }
        tags
    }

    /// Reads definitions and old-style assignments from a tag file.
    fn merge_file(&mut self, file: &Path) {
        let Ok(text) = fs::read_to_string(file) else { return };
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("def\t") {
                let mut it = rest.splitn(2, '\t');
                let (Some(color), Some(name)) = (it.next(), it.next()) else { continue };
                let Ok(color) = u32::from_str_radix(color.trim_start_matches('#'), 16) else { continue };
                if self.def(name).is_none() {
                    self.defs.push(TagDef { name: name.to_string(), color, builtin: false });
                }
            } else if let Some(rest) = line.strip_prefix("tag\t") {
                let mut it = rest.splitn(2, '\t');
                let (Some(key), Some(names)) = (it.next(), it.next()) else { continue };
                let list = self.legacy.entry(key.to_string()).or_default();
                for n in names.split('\t').filter(|s| !s.is_empty()) {
                    if !list.iter().any(|x| x == n) {
                        list.push(n.to_string());
                    }
                }
                if list.is_empty() {
                    self.legacy.remove(key);
                }
            }
        }
    }

    pub fn save(&self) {
        let Some(f) = &self.path else { return };
        let mut out = String::new();
        for d in self.defs.iter().filter(|d| !d.builtin) {
            out += &format!("def\t{:06X}\t{}\n", d.color, d.name);
        }
        let mut keys: Vec<&String> = self.legacy.keys().collect();
        keys.sort();
        for k in keys {
            out += &format!("tag\t{}\t{}\n", k, self.legacy[k].join("\t"));
        }
        let _ = fs::write(f, out);
    }

    /// Takes in what a fresh scan found: each project's own tags, plus any
    /// old-style assignments for it, which are written into the project.
    /// `projects` is (key, project path, tags read from the project).
    pub fn sync(&mut self, root: &Path, projects: &[(String, PathBuf, Vec<String>)]) {
        let mut dirty = false;
        // The old shared file in the projects folder (or from when the app was called Lumen).
        for old in [OLD_FOLDER_FILE, ".lumen-tags.txt"] {
            let f = root.join(old);
            if f.is_file() {
                self.merge_file(&f);
                let _ = fs::rename(&f, root.join(format!("{old}.migrated")));
                dirty = true;
            }
        }
        if !self.legacy.is_empty() {
            let keys: Vec<String> = projects.iter().map(|(k, _, _)| k.clone()).collect();
            self.follow_moves(&keys);
        }

        self.assigned.clear();
        for (key, dir, found) in projects {
            let mut names = found.clone();
            if let Some(old) = self.legacy.remove(key) {
                dirty = true;
                for n in old {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
                if names != *found && write(dir, &names).is_err() {
                    // Couldn't write into the project: try again next time.
                    self.legacy.insert(key.clone(), names.clone());
                }
            }
            for n in &names {
                if self.def(n).is_none() {
                    self.create(n);
                    dirty = true;
                }
            }
            if !names.is_empty() {
                self.assigned.insert(key.clone(), names);
            }
        }
        if dirty {
            self.save();
        }
    }

    pub fn key(root: &Path, project: &Path) -> String {
        project.strip_prefix(root).unwrap_or(project).to_string_lossy().replace('\\', "/")
    }

    pub fn of(&self, key: &str) -> &[String] {
        self.assigned.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn has(&self, key: &str, tag: &str) -> bool {
        self.of(key).iter().any(|t| t == tag)
    }

    /// Adds or removes a tag and saves it into the project.
    pub fn toggle(&mut self, key: &str, project: &Path, tag: &str) {
        let list = self.assigned.entry(key.to_string()).or_default();
        if let Some(i) = list.iter().position(|t| t == tag) {
            list.remove(i);
        } else {
            list.push(tag.to_string());
        }
        let list = list.clone();
        if list.is_empty() {
            self.assigned.remove(key);
        }
        if let Err(e) = write(project, &list) {
            eprintln!("librarian: couldn't save tags in {}: {e}", project.display());
        }
    }

    pub fn def(&self, name: &str) -> Option<&TagDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    /// Adds a custom tag (no-op if it exists). Returns its name, trimmed.
    pub fn create(&mut self, name: &str) -> Option<String> {
        let name = name.trim().replace('\t', " ");
        if name.is_empty() {
            return None;
        }
        if let Some(d) = self.defs.iter().find(|d| d.name.eq_ignore_ascii_case(&name)) {
            return Some(d.name.clone());
        }
        let n = self.defs.iter().filter(|d| !d.builtin).count();
        self.defs.push(TagDef { name: name.clone(), color: CUSTOM[n % CUSTOM.len()], builtin: false });
        Some(name)
    }

    /// Removes a custom tag everywhere. `projects` maps keys to project paths.
    pub fn delete(&mut self, name: &str, projects: &[(String, PathBuf)]) {
        self.defs.retain(|d| d.builtin || d.name != name);
        for (key, dir) in projects {
            if self.has(key, name) {
                self.toggle(key, dir, name);
            }
        }
        for list in self.legacy.values_mut() {
            list.retain(|t| t != name);
        }
        self.legacy.retain(|_, v| !v.is_empty());
    }

    /// Old-style assignments whose project path no longer exists go to a
    /// project with the same file/folder name, if exactly one has none yet.
    fn follow_moves(&mut self, existing: &[String]) {
        let present: HashSet<&String> = existing.iter().collect();
        let orphans: Vec<String> = self.legacy.keys().filter(|k| !present.contains(k)).cloned().collect();
        for old in orphans {
            let name = old.rsplit('/').next().unwrap_or(&old).to_string();
            let candidates: Vec<&String> = existing
                .iter()
                .filter(|k| k.rsplit('/').next() == Some(name.as_str()) && !self.legacy.contains_key(*k))
                .collect();
            if candidates.len() == 1 {
                let new = candidates[0].clone();
                if let Some(v) = self.legacy.remove(&old) {
                    self.legacy.insert(new, v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("librarian-tags-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tags_live_in_the_project_and_follow_it() {
        let root = tmp("move");
        let a = root.join("a Project");
        fs::create_dir_all(&a).unwrap();
        let loose = root.join("beats");
        fs::create_dir_all(&loose).unwrap();
        let (x, y) = (loose.join("x.flp"), loose.join("y.flp"));
        fs::write(&x, b"").unwrap();
        fs::write(&y, b"").unwrap();

        let mut tags = Tags::load(None);
        tags.toggle("a Project", &a, "Released");
        tags.toggle("beats/x.flp", &x, "Idea");
        tags.toggle("beats/y.flp", &y, "Finished");
        assert_eq!(read(&a), ["Released"]);
        assert_eq!(read(&x), ["Idea"]);
        assert_eq!(read(&y), ["Finished"]);

        // Renamed and moved: the tags come along.
        let moved = root.join("2026").join("renamed Project");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::rename(&a, &moved).unwrap();
        assert_eq!(read(&moved), ["Released"]);

        // Untagging the last one removes the line, then the file.
        tags.toggle("beats/x.flp", &x, "Idea");
        assert!(read(&x).is_empty());
        assert_eq!(read(&y), ["Finished"]);
        tags.toggle("beats/y.flp", &y, "Finished");
        assert!(!loose.join(PROJECT_FILE).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn old_assignments_move_into_projects() {
        let root = tmp("migrate");
        let conf = root.join("tags.txt");
        fs::write(&conf, "def\tEC4899\tMix Later\ntag\tsong Project\tFinished\ntag\tgone Project\tIdea\n").unwrap();
        fs::write(root.join(OLD_FOLDER_FILE), "tag\tother Project\tMix Later\n").unwrap();
        let (song, other) = (root.join("song Project"), root.join("other Project"));
        fs::create_dir_all(&song).unwrap();
        fs::create_dir_all(&other).unwrap();
        // A tag written on another computer that this one hasn't heard of.
        write(&other, &["Vocals".to_string()]).unwrap();

        let mut tags = Tags::load(Some(conf.clone()));
        let found = |p: &Path| read(p);
        tags.sync(&root, &[
            ("song Project".into(), song.clone(), found(&song)),
            ("other Project".into(), other.clone(), found(&other)),
        ]);
        assert_eq!(read(&song), ["Finished"]);
        assert_eq!(read(&other), ["Vocals", "Mix Later"]);
        assert!(tags.def("Vocals").is_some());
        assert!(tags.def("Mix Later").is_some());
        assert!(!root.join(OLD_FOLDER_FILE).exists());
        // The project that isn't here keeps its old entry for later.
        let saved = fs::read_to_string(&conf).unwrap();
        assert!(!saved.contains("song Project"));
        assert!(saved.contains("tag\tgone Project\tIdea"));
        let _ = fs::remove_dir_all(&root);
    }
}
