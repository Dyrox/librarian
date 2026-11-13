//! Watches the projects folder and emits one "something changed" signal
//! once the folder has been quiet for a moment — so a bounce that's still
//! being exported only triggers a rescan when the DAW has finished writing.

use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc as async_mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify::{EventKind, RecursiveMode, Watcher};

/// How long the folder must stay quiet before we rescan.
const QUIET: Duration = Duration::from_millis(700);
/// …but never hold a change back for longer than this (very long exports).
const MAX_WAIT: Duration = Duration::from_secs(5);

const WATCHED_EXTS: &[&str] = &["als", "flp", "mp3", "wav", "aif", "aiff", "flac"];
const IGNORED_DIRS: &[&str] = &["Samples", "Backup", "Ableton Project Info", "node_modules"];

/// Use with `Subscription::run_with(root, watch::changes)`.
pub fn changes(root: &PathBuf) -> impl Stream<Item = ()> + use<> {
    let root = root.clone();
    iced::stream::channel(4, async move |mut out: async_mpsc::Sender<()>| {
        let (tx, mut rx) = async_mpsc::unbounded::<()>();
        std::thread::Builder::new()
            .name("librarian-watch".into())
            .spawn(move || run(root, tx))
            .ok();
        while rx.next().await.is_some() {
            if out.send(()).await.is_err() {
                break;
            }
        }
    })
}

fn run(root: PathBuf, out: async_mpsc::UnboundedSender<()>) {
    let (tx, rx) = mpsc::channel::<()>();
    let filter_root = root.clone();
    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if is_relevant(&filter_root, &event) {
                let _ = tx.send(());
            }
        }
    });
    let Ok(mut watcher) = watcher else { return };
    if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
        return;
    }

    // `rx` ends when the watcher (which owns `tx`) is dropped; we exit when
    // the app side goes away (send fails), dropping the watcher with us.
    while rx.recv().is_ok() {
        let first = Instant::now();
        loop {
            match rx.recv_timeout(QUIET) {
                Ok(()) if first.elapsed() < MAX_WAIT => continue,
                Ok(()) | Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        if out.unbounded_send(()).is_err() {
            return;
        }
    }
}

fn is_relevant(root: &Path, event: &notify::Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let ignored = rel.components().any(|c| match c {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                name.starts_with('.') || IGNORED_DIRS.contains(&name.as_ref())
            }
            _ => false,
        });
        if ignored {
            return false;
        }
        match path.extension() {
            Some(ext) => WATCHED_EXTS.contains(&ext.to_string_lossy().to_lowercase().as_str()),
            // No extension: most likely a folder being added, moved or removed.
            None => matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))),
        }
    })
}
