//! Project tags ("Finished", "Abandoned", …): definitions + assignments,
//! stored in Librarian's config folder as plain text.
//!
//! Assignments are keyed by the project's path *relative to the scan root*,
//! so moving the whole music folder keeps them. When a project is moved
//! inside the folder, its tags follow it by name (see `follow_moves`).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    /// relative project path → tag names
    pub assigned: HashMap<String, Vec<String>>,
    /// Where these tags are saved.
    path: Option<PathBuf>,
    /// Modification time of `path` when we last read or wrote it.
    mtime: Option<SystemTime>,
}

/// Name of the tag file when it lives inside the projects folder.
pub const FOLDER_FILE: &str = ".librarian-tags.txt";

/// Where tags are stored: Librarian's config folder, or the projects folder itself.
pub fn location(root: Option<&Path>, in_folder: bool) -> Option<PathBuf> {
    match root {
        Some(root) if in_folder => {
            let file = root.join(FOLDER_FILE);
            // Written by the app's old name (Lumen)? Take it over.
            let old = root.join(".lumen-tags.txt");
            if !file.exists() && old.exists() {
                let _ = fs::rename(&old, &file);
            }
            Some(file)
        }
        _ => {
            Some(crate::cache::config_dir()?.join("tags.txt"))
        }
    }
}

fn mtime(path: Option<&Path>) -> Option<SystemTime> {
    fs::metadata(path?).and_then(|m| m.modified()).ok()
}

impl Tags {
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut tags = Tags {
            defs: BUILTIN
                .iter()
                .map(|(n, c)| TagDef { name: n.to_string(), color: *c, builtin: true })
                .collect(),
            assigned: HashMap::new(),
            mtime: mtime(path.as_deref()),
            path,
        };
        let Some(text) = tags.path.as_ref().and_then(|f| fs::read_to_string(f).ok()) else { return tags };
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("def\t") {
                let mut it = rest.splitn(2, '\t');
                let (Some(color), Some(name)) = (it.next(), it.next()) else { continue };
                let Ok(color) = u32::from_str_radix(color.trim_start_matches('#'), 16) else { continue };
                if !tags.defs.iter().any(|d| d.name == name) {
                    tags.defs.push(TagDef { name: name.to_string(), color, builtin: false });
                }
            } else if let Some(rest) = line.strip_prefix("tag\t") {
                let mut it = rest.splitn(2, '\t');
                let (Some(key), Some(names)) = (it.next(), it.next()) else { continue };
                let names: Vec<String> = names.split('\t').filter(|s| !s.is_empty()).map(String::from).collect();
                if !names.is_empty() {
                    tags.assigned.insert(key.to_string(), names);
                }
            }
        }
        tags
    }

    /// True if another computer (or app) changed the file since we read it.
    pub fn changed_on_disk(&self) -> bool {
        mtime(self.path.as_deref()) != self.mtime
    }

    /// Saves to `path` from now on. With `merge`, tags already saved there
    /// (say, by another computer) are combined with ours; otherwise ours
    /// replace them.
    pub fn relocate(&mut self, path: Option<PathBuf>, merge: bool) {
        if path == self.path {
            return;
        }
        let there = if merge { Tags::load(path.clone()) } else { Tags::default() };
        for d in there.defs.into_iter().filter(|d| !d.builtin) {
            if self.def(&d.name).is_none() {
                self.defs.push(d);
            }
        }
        for (key, names) in there.assigned {
            let list = self.assigned.entry(key).or_default();
            for n in names {
                if !list.contains(&n) {
                    list.push(n);
                }
            }
        }
        self.path = path;
        self.save();
    }

    pub fn save(&mut self) {
        let Some(f) = self.path.clone() else { return };
        let mut out = String::new();
        for d in self.defs.iter().filter(|d| !d.builtin) {
            out += &format!("def\t{:06X}\t{}\n", d.color, d.name);
        }
        let mut keys: Vec<&String> = self.assigned.keys().collect();
        keys.sort();
        for k in keys {
            let names = &self.assigned[k];
            if !names.is_empty() {
                out += &format!("tag\t{}\t{}\n", k, names.join("\t"));
            }
        }
        let _ = fs::write(&f, out);
        self.mtime = mtime(Some(&f));
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

    pub fn toggle(&mut self, key: &str, tag: &str) {
        let list = self.assigned.entry(key.to_string()).or_default();
        if let Some(i) = list.iter().position(|t| t == tag) {
            list.remove(i);
        } else {
            list.push(tag.to_string());
        }
        if list.is_empty() {
            self.assigned.remove(key);
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
        if !self.defs.iter().any(|d| d.name.eq_ignore_ascii_case(&name)) {
            let n = self.defs.iter().filter(|d| !d.builtin).count();
            self.defs.push(TagDef { name: name.clone(), color: CUSTOM[n % CUSTOM.len()], builtin: false });
        }
        Some(name)
    }

    pub fn delete(&mut self, name: &str) {
        self.defs.retain(|d| d.builtin || d.name != name);
        for list in self.assigned.values_mut() {
            list.retain(|t| t != name);
        }
        self.assigned.retain(|_, v| !v.is_empty());
    }

    /// Re-attach tags whose project path no longer exists to a project with
    /// the same file/folder name that has no tags yet (i.e. it was moved).
    /// Returns true if anything changed.
    pub fn follow_moves(&mut self, existing: &[String]) -> bool {
        let present: HashSet<&String> = existing.iter().collect();
        let orphans: Vec<String> = self.assigned.keys().filter(|k| !present.contains(k)).cloned().collect();
        let mut changed = false;
        for old in orphans {
            let name = old.rsplit('/').next().unwrap_or(&old).to_string();
            let candidates: Vec<&String> = existing
                .iter()
                .filter(|k| k.rsplit('/').next() == Some(name.as_str()) && !self.assigned.contains_key(*k))
                .collect();
            if candidates.len() == 1 {
                let new = candidates[0].clone();
                if let Some(v) = self.assigned.remove(&old) {
                    self.assigned.insert(new, v);
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocate_merges_into_folder_and_replaces_on_the_way_back() {
        let dir = std::env::temp_dir().join(format!("librarian-tags-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (local, shared) = (dir.join("local.txt"), dir.join(FOLDER_FILE));

        // Another computer already tagged "b" in the shared folder.
        let mut other = Tags::load(Some(shared.clone()));
        other.create("Mix Later");
        other.toggle("2026/b Project", "Mix Later");
        other.save();

        let mut mine = Tags::load(Some(local.clone()));
        mine.toggle("2026/a Project", "Finished");
        mine.save();

        mine.relocate(Some(shared.clone()), true);
        let merged = Tags::load(Some(shared.clone()));
        assert!(merged.has("2026/a Project", "Finished"));
        assert!(merged.has("2026/b Project", "Mix Later"));
        assert!(merged.def("Mix Later").is_some());

        // Back to this computer only: current state replaces the old local file.
        mine.toggle("2026/a Project", "Finished");
        mine.relocate(Some(local.clone()), false);
        let back = Tags::load(Some(local));
        assert!(!back.has("2026/a Project", "Finished"));
        assert!(back.has("2026/b Project", "Mix Later"));
        let _ = fs::remove_dir_all(&dir);
    }
}
