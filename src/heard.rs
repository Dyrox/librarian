//! Remembers which bounces you've listened to, so new exports get a "NEW"
//! dot until you play them.
//!
//! Stored per file (path relative to the scan root) with the modification
//! time you heard. Re-exporting over the same file name makes it new again.
//! The first time a folder is seen, everything in it counts as heard.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::scan::{FileEntry, Project, Version};

#[derive(Debug, Default)]
pub struct Heard {
    /// The root these entries belong to; a different root starts over.
    root: Option<PathBuf>,
    files: HashMap<String, u64>,
}

fn file() -> Option<PathBuf> {
    Some(crate::cache::config_dir()?.join("heard.txt"))
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Heard {
    pub fn load() -> Self {
        let mut heard = Heard::default();
        let Some(text) = file().and_then(|f| fs::read_to_string(f).ok()) else { return heard };
        for line in text.lines() {
            if let Some(root) = line.strip_prefix("root\t") {
                heard.root = Some(PathBuf::from(root));
            } else if let Some((t, key)) = line.split_once('\t') {
                if let Ok(t) = t.parse() {
                    heard.files.insert(key.to_string(), t);
                }
            }
        }
        heard
    }

    pub fn save(&self) {
        let Some(f) = file() else { return };
        let mut out = String::new();
        if let Some(r) = &self.root {
            out += &format!("root\t{}\n", r.display());
        }
        let mut keys: Vec<&String> = self.files.keys().collect();
        keys.sort();
        for k in keys {
            out += &format!("{}\t{}\n", self.files[k], k);
        }
        let _ = fs::write(f, out);
    }

    fn key(root: &Path, f: &FileEntry) -> String {
        f.path.strip_prefix(root).unwrap_or(&f.path).to_string_lossy().replace('\\', "/")
    }

    /// Called after every scan. On a folder we haven't seen, marks everything
    /// heard; otherwise forgets files that are gone. Saves if anything changed.
    pub fn sync(&mut self, root: &Path, projects: &[Project]) {
        let all = projects.iter().flat_map(|p| p.versions.iter()).flat_map(|v| v.files.iter());
        if self.root.as_deref() != Some(root) {
            self.root = Some(root.to_path_buf());
            self.files = all.map(|f| (Self::key(root, f), secs(f.modified))).collect();
            self.save();
            return;
        }
        let present: std::collections::HashSet<String> = all.map(|f| Self::key(root, f)).collect();
        let before = self.files.len();
        self.files.retain(|k, _| present.contains(k));
        if self.files.len() != before {
            self.save();
        }
    }

    fn file_is_new(&self, root: &Path, f: &FileEntry) -> bool {
        self.files.get(&Self::key(root, f)).is_none_or(|&t| secs(f.modified) > t)
    }

    pub fn is_new(&self, root: &Path, v: &Version) -> bool {
        self.root.as_deref() == Some(root) && v.files.iter().any(|f| self.file_is_new(root, f))
    }

    pub fn new_count(&self, root: &Path, p: &Project) -> usize {
        p.versions.iter().filter(|v| self.is_new(root, v)).count()
    }

    /// Marks every format of `v` heard. Returns true if anything changed.
    pub fn mark(&mut self, root: &Path, v: &Version) -> bool {
        let mut changed = false;
        for f in &v.files {
            if self.file_is_new(root, f) {
                self.files.insert(Self::key(root, f), secs(f.modified));
                changed = true;
            }
        }
        changed
    }
}
