// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod als;
mod apps;
mod audio;
mod cache;
mod flp;
mod icons;
mod ctx;
mod heard;
mod i18n;
mod settings;
mod platform;
mod peaks;
mod scan;
mod style;
mod tags;
mod thumb;
mod waveform;
mod watch;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Datelike, Local};
use iced::futures::channel::oneshot;
use iced::theme::Mode;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Row, Space, button, canvas, center, column, container, image, mouse_area, responsive, row,
    scrollable, stack, text, text_input,
};
use iced::{
    Alignment, Color, Element, Event, Length, Size, Subscription, Task, Theme, event, keyboard, system,
    time, window,
};

use als::{AlsMeta, Arrangement};
use audio::Audio;
use i18n::t;
use icons::icon;
use peaks::Peaks;
use apps::Daw;
use scan::{Issue, IssueKind, Project, ScanResult};
use style::*;
use thumb::Thumb;
use waveform::Waveform;

pub fn main() -> iced::Result {
    cache::migrate_from_lumen();
    // `librarian --scan <folder>`: print what the scanner sees and exit (debug aid).
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--scan" {
        let t = Instant::now();
        let projects = scan::scan(Path::new(&args[2])).projects;
        let scanned = t.elapsed();
        for p in &projects {
            let info = p.main_set().and_then(|s| cache::als(&s.path));
            println!(
                "{:<40} {:<28} sets={} demos={} latest={:?} meta={:?} arrangement={:?}",
                p.name,
                p.location,
                p.sets.len(),
                p.demos.len(),
                p.latest_demo().map(|d| &d.name),
                info.as_ref().map(|i| (&i.meta.version, i.meta.tempo, i.meta.audio_tracks, i.meta.midi_tracks)),
                info.as_ref().map(|i| (
                    i.arrangement.tracks.len(),
                    i.arrangement.tracks.iter().map(|t| t.clips.len()).sum::<usize>(),
                    i.arrangement.tracks.iter().flat_map(|t| &t.clips).map(|c| c.notes.len()).sum::<usize>(),
                    i.arrangement.length,
                )),
            );
        }
        println!("{} projects, scan {:?}, total {:?}", projects.len(), scanned, t.elapsed());
        return Ok(());
    }

    // `librarian --issues <folder>`: print convention issues and exit.
    if args.len() == 3 && args[1] == "--issues" {
        let result = scan::scan(Path::new(&args[2]));
        for i in &result.issues {
            println!("[{}] {}", i.kind.slug(), if i.location.is_empty() { "(root)" } else { &i.location });
            for f in i.files.iter().take(8) {
                match &f.belongs_to {
                    Some(owner) => println!("      {}  ->  {}", f.name, owner),
                    None => println!("      {}", f.name),
                }
            }
            if i.files.len() > 8 {
                println!("      … {} more", i.files.len() - 8);
            }
        }
        println!("{} issues", result.issues.len());
        return Ok(());
    }

    let prefs = cache::load_prefs();
    let (w, h) = prefs.window.unwrap_or((1180.0, 800.0));

    iced::application(App::boot, App::update, App::view)
        .title("Librarian")
        .subscription(App::subscription)
        .theme(|app: &App| if app.dark { Theme::Dark } else { Theme::Light })
        .style(|_, _| iced::theme::Style { background_color: p().chrome, text_color: p().text })
        .exit_on_close_request(false)
        .window(window::Settings {
            size: Size::new(w.max(820.0), h.max(520.0)),
            min_size: Some(Size::new(820.0, 520.0)),
            #[cfg(target_os = "macos")]
            platform_specific: window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: true,
            },
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    All,
    Recent,
    WithDemo,
    Unheard,
    Daw(Daw),
    Tag(String),
    /// Projects with no tag at all.
    Untagged,
    Issues,
    Group(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    Recent,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    List,
    Grid,
}

#[derive(Debug, Clone)]
struct NowPlaying {
    project: PathBuf,
    file: PathBuf,
}

type Info = (AlsMeta, Arc<Arrangement>);

#[derive(Debug, Clone)]
pub enum Message {
    PickFolder,
    FolderPicked(Option<PathBuf>),
    Rescan,
    /// The watched folder changed on disk.
    FsChanged,
    MarkHeard(PathBuf),
    Scanned(ScanResult),
    InfoLoaded(PathBuf, Option<Info>),
    PeaksLoaded(PathBuf, SystemTime, Option<Peaks>),
    /// The DAW's app (picked or found) and a PNG of its icon.
    AppResolved(Daw, Option<PathBuf>, Option<PathBuf>),
    Set(settings::Set),
    SetFilter(Filter),
    Query(String),
    SetSort(Sort),
    SetView(View),
    ToggleExpand(PathBuf),
    Select(PathBuf),
    Hover(Option<PathBuf>),
    PlayProject(PathBuf),
    /// Pick which bounce version a project shows/plays.
    SelectVersion(PathBuf, usize),
    /// ← / → : step the playing project's version.
    StepVersion(i32),
    /// ←/→: skip back/forward this many seconds in what's playing.
    SeekBy(i32),
    PlayFile(PathBuf, PathBuf),
    SeekTo(PathBuf, PathBuf, f32),
    TogglePlay,
    Stop,
    Tick,
    Open(PathBuf),
    Reveal(PathBuf),
    Focused,
    Ignore(String),
    ContextMenu(PathBuf),
    /// The grid scrolled: (offset from the top, visible height).
    GridScrolled(f32, f32),
    /// Scroll distance in pixels (lines are counted as 20 px).
    Wheel(f32),
    /// A right click nothing else handled: close the menu (only the menu).
    DismissMenu,
    CloseMenu,
    OpenSub(Option<ctx::Sub>),
    /// A menu command: close the menu, then run it.
    FromMenu(Box<Message>),
    CopyPath(PathBuf),
    ToggleTag(PathBuf, String),
    TagDraft(String),
    CreateTag(PathBuf),
    DeleteTag(String),
    Noop,
    PollTheme,
    ResetIgnored,
    ThemeChanged(Mode),
    Resized(Size),
    CloseRequested(window::Id),
}

struct App {
    prefs: cache::Prefs,
    projects: Vec<Project>,
    issues: Vec<Issue>,
    /// Projects whose main set failed to parse.
    unreadable: HashSet<PathBuf>,
    scanning: bool,
    /// A change arrived mid-scan: scan again when this one lands.
    rescan_pending: bool,
    /// The OS refused to let us read the root folder.
    root_denied: bool,
    last_scan: Option<Instant>,
    heard: heard::Heard,
    peaks: HashMap<PathBuf, (SystemTime, Peaks)>,
    pending_peaks: HashSet<PathBuf>,
    filter: Filter,
    query: String,
    sort: Sort,
    view: View,
    expanded: Option<PathBuf>,
    selected: Option<PathBuf>,
    hovered: Option<PathBuf>,
    /// Selected bounce version per project (index into `versions`; default 0 = newest).
    version_sel: HashMap<PathBuf, usize>,
    audio: Audio,
    now: Option<NowPlaying>,
    position: Duration,
    error: Option<String>,
    dark: bool,
    daw_icons: HashMap<Daw, image::Handle>,
    daw_apps: HashMap<Daw, PathBuf>,
    settings_open: bool,
    /// Output devices, listed when the settings page opens.
    devices: Vec<String>,
    cache_size: Option<u64>,
    /// The OS appearance (used when the theme setting is "System").
    system_dark: bool,
    tags: tags::Tags,
    tag_draft: String,
    menu: Option<ctx::MenuState>,
    window_size: Size,
    /// Where the grid is scrolled to and how tall it is, so only the rows on
    /// screen are built (see `grid`).
    grid_offset: f32,
    grid_height: f32,
}

/// Runs blocking work on the rayon pool and resolves when it's done.
fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> impl std::future::Future<Output = T> {
    let (tx, rx) = oneshot::channel();
    rayon::spawn(move || {
        let _ = tx.send(f());
    });
    async move { rx.await.expect("worker dropped") }
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let mut prefs = cache::load_prefs();
        // Debug aid: LIBRARIAN_ROOT=/some/dir scans that folder this session only.
        if let Some(r) = std::env::var_os("LIBRARIAN_ROOT") {
            prefs.root = Some(PathBuf::from(r));
        }
        i18n::set(prefs.lang);
        let system_dark = platform::system_dark().unwrap_or(false);
        let tags = tags::Tags::load(tags::location());
        let mut audio = Audio::new();
        audio.preferred = prefs.device.clone();
        let mut app = Self {
            sort: if prefs.sort_by_name { Sort::Name } else { Sort::Recent },
            view: if prefs.grid { View::Grid } else { View::List },
            prefs,
            projects: Vec::new(),
            issues: Vec::new(),
            unreadable: HashSet::new(),
            scanning: false,
            rescan_pending: false,
            root_denied: false,
            last_scan: None,
            heard: heard::Heard::load(),
            peaks: HashMap::new(),
            pending_peaks: HashSet::new(),
            // Debug aid: LIBRARIAN_FILTER=issues opens straight on the Issues page.
            filter: if std::env::var("LIBRARIAN_FILTER").as_deref() == Ok("issues") { Filter::Issues } else { Filter::All },
            query: String::new(),
            expanded: None,
            selected: None,
            hovered: None,
            version_sel: HashMap::new(),
            audio,
            now: None,
            position: Duration::ZERO,
            error: None,
            dark: false,
            daw_icons: HashMap::new(),
            daw_apps: HashMap::new(),
            // Debug aid: LIBRARIAN_FILTER=settings opens straight on Settings.
            settings_open: std::env::var("LIBRARIAN_FILTER").as_deref() == Ok("settings"),
            devices: Vec::new(),
            cache_size: None,
            system_dark,
            tags,
            tag_draft: String::new(),
            menu: None,
            window_size: Size::ZERO,
            grid_offset: 0.0,
            grid_height: 1200.0,
        };
        app.apply_theme();
        let scan = app.rescan();
        let icon = Task::batch(Daw::ALL.map(|d| app.resolve_app(d)));
        let settings = if app.settings_open { app.update_settings(settings::Set::Open) } else { Task::none() };
        let theme = system::theme().map(Message::ThemeChanged);
        // Debug aid: LIBRARIAN_SCROLL=<px> scrolls the grid there after 3 s.
        let scroll = match std::env::var("LIBRARIAN_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
            Some(y) => Task::perform(blocking(|| std::thread::sleep(Duration::from_secs(3))), |_| ()).then(move |()| {
                iced::widget::operation::scroll_to(GRID_ID, scrollable::AbsoluteOffset { x: None, y: Some(y) })
            }),
            None => Task::none(),
        };
        (app, Task::batch([theme, scan, icon, settings, scroll]))
    }

    fn root(&self) -> Option<&Path> {
        self.prefs.root.as_deref()
    }

    /// Bounce versions of `p` you haven't played yet.
    fn new_count(&self, p: &Project) -> usize {
        if !self.prefs.new_marks {
            return 0;
        }
        self.root().map_or(0, |r| self.heard.new_count(r, p))
    }

    fn version_is_new(&self, v: &scan::Version) -> bool {
        self.prefs.new_marks && self.root().is_some_and(|r| self.heard.is_new(r, v))
    }

    fn save_prefs(&mut self) {
        self.prefs.grid = self.view == View::Grid;
        self.prefs.sort_by_name = self.sort == Sort::Name;
        if std::env::var_os("LIBRARIAN_ROOT").is_none() {
            cache::save_prefs(&self.prefs);
        }
    }

    fn rescan(&mut self) -> Task<Message> {
        let Some(root) = self.prefs.root.clone() else { return Task::none() };
        if self.scanning {
            self.rescan_pending = true;
            return Task::none();
        }
        self.scanning = true;
        self.last_scan = Some(Instant::now());
        Task::perform(blocking(move || scan::scan(&root)), Message::Scanned)
    }

    fn request_peaks(&mut self, file: &Path, modified: SystemTime) -> Task<Message> {
        let fresh = self.peaks.get(file).is_some_and(|(m, _)| *m == modified);
        if fresh || self.pending_peaks.contains(file) {
            return Task::none();
        }
        self.pending_peaks.insert(file.to_path_buf());
        let f = file.to_path_buf();
        Task::perform(blocking(move || cache::peaks(&f)), {
            let f = file.to_path_buf();
            move |p| Message::PeaksLoaded(f.clone(), modified, p)
        })
    }

    /// Scanner issues plus unreadable sets, minus ignored ones.
    fn all_issues(&self) -> Vec<Issue> {
        let root = self.root().map(Path::to_path_buf).unwrap_or_default();
        let mut out: Vec<Issue> = self.issues.clone();
        for p in self.projects.iter().filter(|p| self.unreadable.contains(&p.dir)) {
            if let Some(set) = p.main_set() {
                out.push(Issue {
                    kind: IssueKind::UnreadableSet,
                    path: set.path.clone(),
                    location: set.path.strip_prefix(&root).map(|r| r.display().to_string()).unwrap_or_default(),
                    files: vec![],
                    project: Some(p.dir.clone()),
                });
            }
        }
        out.retain(|i| !self.prefs.ignored.contains(&i.key()));
        out
    }

    fn version_index(&self, pr: &Project) -> usize {
        self.version_sel.get(&pr.dir).copied().unwrap_or(0).min(pr.versions.len().saturating_sub(1))
    }

    /// The bounce a project currently shows and plays.
    fn current_demo<'a>(&self, pr: &'a Project) -> Option<&'a scan::FileEntry> {
        pr.versions.get(self.version_index(pr)).map(scan::Version::file)
    }

    fn select_version(&mut self, dir: PathBuf, index: usize) -> Task<Message> {
        let Some(pr) = self.project(&dir) else { return Task::none() };
        let Some(v) = pr.versions.get(index) else { return Task::none() };
        let (file, modified) = (v.file().path.clone(), v.file().modified);
        self.version_sel.insert(dir.clone(), index);
        let peaks = self.request_peaks(&file, modified);

        // A/B: if this project is playing, carry on from the same spot in the new version.
        let switch = match &self.now {
            Some(n) if n.project == dir && n.file != file => {
                let was_paused = self.audio.is_paused();
                let at = self.position;
                let t = self.play(dir, file, Some(at));
                if was_paused {
                    self.audio.toggle();
                }
                t
            }
            _ => Task::none(),
        };
        Task::batch([peaks, switch])
    }

    fn project(&self, dir: &Path) -> Option<&Project> {
        self.projects.iter().find(|p| p.dir == dir)
    }

    fn duration_of(&self, file: &Path) -> Option<Duration> {
        self.peaks
            .get(file)
            .map(|(_, p)| Duration::from_secs_f32(p.duration_secs))
            .or_else(|| {
                (self.audio.current.as_deref() == Some(file)).then_some(self.audio.duration).flatten()
            })
    }

    fn play(&mut self, project: PathBuf, file: PathBuf, start: Option<Duration>) -> Task<Message> {
        match self.audio.play(&file, start) {
            Ok(()) => {
                self.error = None;
                self.position = start.unwrap_or_default();
                if let Some(root) = self.prefs.root.clone() {
                    let version = self.projects.iter().find(|p| p.dir == project).and_then(|p| p.versions.iter().find(|v| v.files.iter().any(|f| f.path == file)));
                    if let Some(v) = version {
                        if self.heard.mark(&root, v) {
                            self.heard.save();
                        }
                    }
                }
                self.now = Some(NowPlaying { project, file: file.clone() });
                let modified = std::fs::metadata(&file)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                self.request_peaks(&file, modified)
            }
            Err(e) => {
                self.error = Some(i18n::couldnt_play(&file_name(&file), &e));
                Task::none()
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickFolder => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title(t("Choose your music projects folder"))
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::FolderPicked,
            ),
            Message::FolderPicked(Some(path)) => {
                self.prefs.root = Some(path);
                self.save_prefs();
                self.projects.clear();
                self.filter = Filter::All;
                self.expanded = None;
                self.selected = None;
                self.scanning = false;
                self.rescan()
            }
            Message::FolderPicked(None) => Task::none(),
            Message::Rescan | Message::FsChanged => self.rescan(),
            Message::MarkHeard(dir) => {
                self.menu = None;
                if let (Some(root), Some(i)) = (self.prefs.root.clone(), self.projects.iter().position(|p| p.dir == dir)) {
                    let mut changed = false;
                    for v in &self.projects[i].versions {
                        changed |= self.heard.mark(&root, v);
                    }
                    if changed {
                        self.heard.save();
                    }
                }
                Task::none()
            }
            Message::Focused => {
                if self.last_scan.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
                    self.rescan()
                } else {
                    Task::none()
                }
            }
            Message::Scanned(result) => {
                self.scanning = false;
                let ScanResult { mut projects, issues, denied } = result;
                self.root_denied = denied;
                scan::order_formats(&mut projects, self.prefs.prefer_mp3);
                self.issues = issues;
                // Debug aid: LIBRARIAN_DEBUG_MENU=bounces|tags opens the context menu on the first project.
                if let (Ok(which), None, Some(first)) = (std::env::var("LIBRARIAN_DEBUG_MENU"), &self.menu, projects.first()) {
                    let sub = match which.as_str() {
                        "tags" => Some(ctx::Sub::Tags),
                        "bounces" => Some(ctx::Sub::Bounces),
                        _ => None,
                    };
                    self.menu = Some(ctx::MenuState { project: first.dir.clone(), at: iced::Point::new(560.0, 150.0), sub, scrolled: 0.0 });
                }
                if let Some(root) = self.prefs.root.clone() {
                    let found: Vec<_> = projects
                        .iter()
                        .map(|p| (tags::Tags::key(&root, &p.dir), p.dir.clone(), p.tags.clone()))
                        .collect();
                    self.tags.sync(&root, &found);
                    self.heard.sync(&root, &projects);
                }
                let old: HashMap<_, _> = self
                    .projects
                    .drain(..)
                    .map(|p| (p.dir.clone(), (p.meta, p.arrangement)))
                    .collect();
                let mut tasks = Vec::new();
                for p in &mut projects {
                    if let Some((meta, arr)) = old.get(&p.dir) {
                        p.meta = meta.clone();
                        p.arrangement = arr.clone();
                    }
                    if let Some(set) = p.main_set() {
                        let (dir, path) = (p.dir.clone(), set.path.clone());
                        tasks.push(Task::perform(
                            blocking(move || cache::als(&path).map(|i| (i.meta, Arc::new(i.arrangement)))),
                            move |info| Message::InfoLoaded(dir.clone(), info),
                        ));
                    }
                }
                let demos: Vec<_> = projects
                    .iter()
                    .filter_map(|p| p.latest_demo().map(|d| (d.path.clone(), d.modified)))
                    .collect();
                self.projects = projects;
                for (path, modified) in demos {
                    tasks.push(self.request_peaks(&path, modified));
                }
                if let Filter::Group(g) = &self.filter {
                    if !self.projects.iter().any(|p| &p.group == g) {
                        self.filter = Filter::All;
                    }
                }
                if std::mem::take(&mut self.rescan_pending) {
                    tasks.push(self.rescan());
                }
                Task::batch(tasks)
            }
            Message::InfoLoaded(dir, info) => {
                if info.is_none() {
                    self.unreadable.insert(dir.clone());
                } else {
                    self.unreadable.remove(&dir);
                }
                if let (Some(p), Some((meta, arr))) = (self.projects.iter_mut().find(|p| p.dir == dir), info) {
                    if p.meta.as_ref() != Some(&meta) {
                        p.meta = Some(meta);
                    }
                    if p.arrangement.as_deref() != Some(&*arr) {
                        p.arrangement = Some(arr);
                    }
                }
                Task::none()
            }
            Message::PeaksLoaded(path, modified, peaks) => {
                self.pending_peaks.remove(&path);
                if let Some(p) = peaks {
                    self.peaks.insert(path, (modified, p));
                }
                Task::none()
            }
            Message::AppResolved(daw, app, icon) => {
                match app {
                    Some(a) => self.daw_apps.insert(daw, a),
                    None => self.daw_apps.remove(&daw),
                };
                match icon {
                    Some(i) => self.daw_icons.insert(daw, image::Handle::from_path(i)),
                    None => self.daw_icons.remove(&daw),
                };
                Task::none()
            }
            Message::Set(s) => self.update_settings(s),
            Message::SetFilter(f) => {
                self.filter = f;
                self.settings_open = false;
                Task::none()
            }
            Message::Query(q) => {
                self.query = q;
                Task::none()
            }
            Message::SetSort(s) => {
                self.sort = s;
                self.save_prefs();
                Task::none()
            }
            Message::SetView(v) => {
                self.view = v;
                self.save_prefs();
                Task::none()
            }
            Message::ToggleExpand(dir) => {
                if self.expanded.as_ref() == Some(&dir) {
                    self.expanded = None;
                    return Task::none();
                }
                let reqs: Vec<_> = self
                    .project(&dir)
                    .map(|p| p.demos.iter().map(|d| (d.path.clone(), d.modified)).collect())
                    .unwrap_or_default();
                self.expanded = Some(dir);
                Task::batch(reqs.into_iter().map(|(f, m)| self.request_peaks(&f, m)).collect::<Vec<_>>())
            }
            Message::Select(dir) => {
                self.selected = Some(dir);
                Task::none()
            }
            Message::Hover(dir) => {
                self.hovered = dir;
                Task::none()
            }
            Message::PlayProject(dir) => {
                if self.now.as_ref().is_some_and(|n| n.project == dir) {
                    self.audio.toggle();
                    return Task::none();
                }
                match self.project(&dir).and_then(|p| self.current_demo(p)).map(|d| d.path.clone()) {
                    Some(file) => self.play(dir, file, None),
                    None => Task::none(),
                }
            }
            Message::SelectVersion(dir, index) => self.select_version(dir, index),
            Message::SeekBy(secs) => {
                let Some(file) = self.audio.current.clone() else { return Task::none() };
                let pos = self.audio.position().as_secs_f32();
                let end = self.duration_of(&file).map(|d| d.as_secs_f32() - 0.2).unwrap_or(f32::MAX);
                let at = Duration::from_secs_f32((pos + secs as f32).clamp(0.0, end.max(0.0)));
                self.audio.seek(at);
                self.position = at;
                Task::none()
            }
            Message::StepVersion(delta) => {
                let Some(dir) = self.now.as_ref().map(|n| n.project.clone()) else { return Task::none() };
                let Some(pr) = self.project(&dir) else { return Task::none() };
                let n = pr.versions.len() as i32;
                if n < 2 {
                    return Task::none();
                }
                // Versions are newest-first: → goes to the next older one.
                let next = (self.version_index(pr) as i32 + delta).clamp(0, n - 1) as usize;
                self.select_version(dir, next)
            }
            Message::PlayFile(dir, file) => {
                if self.audio.current.as_deref() == Some(file.as_path()) {
                    self.audio.toggle();
                    Task::none()
                } else {
                    if let Some(i) = self
                        .project(&dir)
                        .and_then(|p| p.versions.iter().position(|v| v.files.iter().any(|f| f.path == file)))
                    {
                        self.version_sel.insert(dir.clone(), i);
                    }
                    self.play(dir, file, None)
                }
            }
            Message::SeekTo(dir, file, frac) => {
                let at = self.duration_of(&file).map(|d| d.mul_f32(frac));
                if self.audio.current.as_deref() == Some(file.as_path()) {
                    if let Some(at) = at {
                        self.audio.seek(at);
                        self.position = at;
                        if self.audio.is_paused() {
                            self.audio.toggle();
                        }
                    }
                    Task::none()
                } else {
                    self.play(dir, file, at)
                }
            }
            Message::TogglePlay => {
                if self.now.is_some() {
                    self.audio.toggle();
                }
                Task::none()
            }
            Message::Stop => {
                self.audio.stop();
                self.now = None;
                self.position = Duration::ZERO;
                Task::none()
            }
            Message::Tick => {
                if self.audio.finished() {
                    self.audio.stop();
                    self.now = None;
                    self.position = Duration::ZERO;
                } else {
                    self.position = self.audio.position();
                }
                Task::none()
            }
            Message::Open(path) => {
                // A hand-picked app wins; otherwise the system's default for the file.
                let app = Daw::of_file(&path).and_then(|d| self.prefs.app(d)).cloned();
                apps::open_with(app.as_deref(), &path);
                Task::none()
            }
            Message::Reveal(path) => {
                reveal_path(&path);
                Task::none()
            }
            Message::ContextMenu(dir) => {
                self.selected = Some(dir.clone());
                self.tag_draft.clear();
                self.menu = Some(ctx::MenuState { project: dir, at: cursor(), sub: None, scrolled: 0.0 });
                Task::none()
            }
            Message::GridScrolled(offset, height) => {
                self.grid_offset = offset;
                self.grid_height = height;
                Task::none()
            }
            Message::Wheel(d) => {
                // Close on a deliberate scroll only: trackpad two-finger
                // clicks emit small scroll events as the fingers settle.
                let Some(m) = self.menu.as_mut() else { return Task::none() };
                m.scrolled += d;
                if std::env::var_os("LIBRARIAN_DEBUG_MENU").is_some() {
                    eprintln!("menu: wheel {d:.1}px, total {:.1}px", m.scrolled);
                }
                if m.scrolled > 24.0 {
                    self.menu = None;
                    self.tag_draft.clear();
                }
                Task::none()
            }
            Message::DismissMenu => {
                if self.menu.take().is_some() {
                    self.tag_draft.clear();
                }
                Task::none()
            }
            Message::CloseMenu => {
                if self.menu.take().is_some() {
                    self.tag_draft.clear();
                } else {
                    self.settings_open = false;
                }
                Task::none()
            }
            Message::OpenSub(sub) => {
                let Some(m) = self.menu.as_mut() else { return Task::none() };
                if m.sub == sub {
                    return Task::none();
                }
                m.sub = sub;
                if sub == Some(ctx::Sub::Tags) {
                    return iced::widget::operation::focus(ctx::NEW_TAG_INPUT);
                }
                Task::none()
            }
            Message::FromMenu(inner) => {
                self.menu = None;
                self.tag_draft.clear();
                self.update(*inner)
            }
            Message::CopyPath(path) => iced::clipboard::write(path.display().to_string()),
            Message::ToggleTag(dir, tag) => {
                if let Some(pr) = self.project(&dir) {
                    let key = self.tag_key(pr);
                    self.tags.toggle(&key, &dir, &tag);
                }
                Task::none()
            }
            Message::TagDraft(s) => {
                self.tag_draft = s;
                Task::none()
            }
            Message::CreateTag(dir) => {
                if let Some(name) = self.tags.create(&self.tag_draft) {
                    if let Some(pr) = self.project(&dir) {
                        let key = self.tag_key(pr);
                        if !self.tags.has(&key, &name) {
                            self.tags.toggle(&key, &dir, &name);
                        }
                    }
                    self.tags.save();
                }
                self.tag_draft.clear();
                Task::none()
            }
            Message::DeleteTag(name) => {
                let projects: Vec<_> = self.projects.iter().map(|p| (self.tag_key(p), p.dir.clone())).collect();
                self.tags.delete(&name, &projects);
                self.tags.save();
                if self.filter == Filter::Tag(name) {
                    self.filter = Filter::All;
                }
                Task::none()
            }
            Message::Noop => Task::none(),
            Message::PollTheme => {
                if let Some(dark) = platform::system_dark() {
                    self.system_dark = dark;
                    self.apply_theme();
                }
                Task::none()
            }
            Message::Ignore(key) => {
                if !self.prefs.ignored.contains(&key) {
                    self.prefs.ignored.push(key);
                    self.save_prefs();
                }
                Task::none()
            }
            Message::ResetIgnored => {
                self.prefs.ignored.clear();
                self.save_prefs();
                Task::none()
            }
            Message::ThemeChanged(mode) => {
                // macOS is polled instead (see PollTheme).
                if platform::system_dark().is_none() {
                    self.system_dark = mode == Mode::Dark;
                    self.apply_theme();
                }
                Task::none()
            }
            Message::Resized(size) => {
                self.window_size = size;
                self.prefs.window = Some((size.width, size.height));
                Task::none()
            }
            Message::CloseRequested(_) => {
                self.save_prefs();
                iced::exit()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = event::listen_with(|event, status, id| match (event, status) {
            (Event::Window(window::Event::Focused), _) => Some(Message::Focused),
            (Event::Window(window::Event::Resized(size)), _) => Some(Message::Resized(size)),
            (Event::Window(window::Event::CloseRequested), _) => Some(Message::CloseRequested(id)),
            // Remembered for placing the context menu, without a message:
            // every message rebuilds and lays out the whole window.
            (Event::Mouse(iced::mouse::Event::CursorMoved { position }), _) => {
                set_cursor(position);
                None
            }
            (Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)), event::Status::Ignored) => {
                Some(Message::DismissMenu)
            }
            (
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }),
                _,
            ) => Some(Message::CloseMenu),
            // Match the physical key too: with a Chinese input method the
            // character is a full-width "，".
            (Event::Keyboard(keyboard::Event::KeyPressed { key, physical_key, modifiers, .. }), _)
                if modifiers.command()
                    && (physical_key == keyboard::key::Physical::Code(keyboard::key::Code::Comma)
                        || matches!(&key, keyboard::Key::Character(c) if c.as_str() == "," || c.as_str() == "，")) =>
            {
                Some(Message::Set(settings::Set::Open))
            }
            (
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Space),
                    ..
                }),
                event::Status::Ignored,
            ) => Some(Message::TogglePlay),
            (
                Event::Keyboard(keyboard::Event::KeyPressed { key: keyboard::Key::Named(named), modifiers, .. }),
                event::Status::Ignored,
            ) => {
                let dir = match named {
                    keyboard::key::Named::ArrowLeft => -1,
                    keyboard::key::Named::ArrowRight => 1,
                    _ => return None,
                };
                // ←/→ skip 5 s, ⇧←/⇧→ switch bounce.
                Some(if modifiers.shift() { Message::StepVersion(dir) } else { Message::SeekBy(dir * 5) })
            }
            _ => None,
        });
        let theme = if cfg!(target_os = "macos") {
            time::every(Duration::from_secs(1)).map(|_| Message::PollTheme)
        } else {
            system::theme_changes().map(Message::ThemeChanged)
        };
        let watcher = match self.prefs.root.clone().filter(|_| self.prefs.watch) {
            Some(root) => Subscription::run_with(root, watch::changes).map(|()| Message::FsChanged),
            None => Subscription::none(),
        };
        // Scrolling only matters while the menu is open (it closes it); a
        // message per scroll tick would rebuild the window every frame.
        let wheel = if self.menu.is_some() {
            event::listen_with(|event, _, _| match event {
                Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => Some(Message::Wheel(match delta {
                    iced::mouse::ScrollDelta::Lines { x, y } => (x.abs() + y.abs()) * 20.0,
                    iced::mouse::ScrollDelta::Pixels { x, y } => x.abs() + y.abs(),
                })),
                _ => None,
            })
        } else {
            Subscription::none()
        };
        let events = Subscription::batch([events, watcher, wheel]);
        if self.audio.is_playing() {
            Subscription::batch([
                events,
                theme,
                time::every(Duration::from_millis(40)).map(|_| Message::Tick),
            ])
        } else {
            Subscription::batch([events, theme])
        }
    }

    // ---------------------------------------------------------------- views

    fn view(&self) -> Element<'_, Message> {
        let main: Element<_> = if self.settings_open {
            self.settings_view()
        } else if self.root().is_none() {
            self.welcome()
        } else {
            self.main_panel()
        };
        let base: Element<_> = row![
            self.sidebar(),
            container(main).style(panel).width(Length::Fill).height(Length::Fill).padding(1)
        ]
        .padding(iced::Padding { top: 8.0, right: 8.0, bottom: 8.0, left: 0.0 })
        .into();
        // Always a stack, menu or not: switching between `base` and
        // `stack![base, menu]` changes the widget tree, and iced then throws
        // away the scroll positions inside `base` (lists jump to the top).
        let menu: Element<_> = self.context_menu().unwrap_or_else(|| Space::new().into());
        stack![base, menu].into()
    }

    fn welcome(&self) -> Element<'_, Message> {
        let p = p();
        center(
            column![
                icon(icons::LOGO, 44, p.text),
                text(t("Your projects, at a glance")).size(26).font(BOLD),
                text(t("Choose the folder where you keep your music projects.\nLibrarian finds every Ableton Live and FL Studio project inside it, draws its arrangement and lines up your latest bounce for a quick listen."))
                    .size(14)
                    .color(p.text_2)
                    .align_x(Alignment::Center),
                Space::new().height(8),
                button(
                    row![icon(icons::FOLDER, 16, p.on_accent), text(t("Choose Folder")).size(14).font(MEDIUM)]
                        .spacing(8)
                        .align_y(Alignment::Center)
                )
                .padding([11, 22])
                .style(primary)
                .on_press(Message::PickFolder),
            ]
            .spacing(12)
            .align_x(Alignment::Center)
            .max_width(460),
        )
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let p = p();
        let total = self.projects.len();
        let recent = self.projects.iter().filter(|p| is_recent(p)).count();
        let with_demo = self.projects.iter().filter(|p| !p.demos.is_empty()).count();
        let unheard = self.projects.iter().filter(|p| self.new_count(p) > 0).count();

        let nav_item = |ic: &'static str, label: String, count: usize, f: Filter| {
            let active = !self.settings_open && self.filter == f;
            button(
                row![
                    icon(ic, 16, if active { p.text } else { p.text_2 }),
                    text(label)
                        .size(14)
                        .font(if active { MEDIUM } else { iced::Font::DEFAULT })
                        .wrapping(Wrapping::None)
                        .width(Length::Fill),
                    text(count.to_string()).size(12).color(p.text_3),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 10])
            .style(nav(active))
            .on_press(Message::SetFilter(f))
        };

        // Groups: top-level folders, most recently active first.
        let mut groups: Vec<(String, usize, SystemTime)> = Vec::new();
        for pr in &self.projects {
            match groups.iter_mut().find(|g| g.0 == pr.group) {
                Some(g) => {
                    g.1 += 1;
                    g.2 = g.2.max(pr.modified);
                }
                None => groups.push((pr.group.clone(), 1, pr.modified)),
            }
        }
        groups.sort_by(|a, b| b.2.cmp(&a.2));
        let root_name = self.root().map(file_name).unwrap_or_default();

        let folder_list = Column::with_children(groups.into_iter().map(|(g, n, _)| {
            let label = if g.is_empty() { root_name.clone() } else { g.clone() };
            nav_item(icons::FOLDER, label, n, Filter::Group(g)).into()
        }))
        .spacing(2);

        let root_card = button(
            row![
                icon(icons::FOLDER, 16, p.text_2),
                column![
                    text(if root_name.is_empty() { t("No folder").into() } else { root_name.clone() })
                        .size(13)
                        .font(MEDIUM)
                        .wrapping(Wrapping::None),
                    text(t("Change folder…")).size(11).color(p.text_2),
                ]
                .spacing(1)
                .clip(true),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 10])
        .style(nav(false))
        .on_press(Message::PickFolder);

        let content = column![
            row![icon(icons::LOGO, 20, p.text), text("Librarian").size(17).font(BOLD)]
                .spacing(9)
                .align_y(Alignment::Center)
                .padding([0, 10]),
            Space::new().height(14),
            nav_item(icons::GRID, t("All Projects").into(), total, Filter::All),
            nav_item(icons::CLOCK, t("Last 30 Days").into(), recent, Filter::Recent),
            nav_item(icons::WAVE, t("With Demos").into(), with_demo, Filter::WithDemo),
            if self.prefs.new_marks {
                Element::from(nav_item(icons::SPARKLE, t("New Bounces").into(), unheard, Filter::Unheard))
            } else {
                Space::new().into()
            },
            self.issues_nav(),
            Space::new().height(14),
            container(text(t("DAWS")).size(11).font(SEMIBOLD).color(p.text_3)).padding([0, 10]),
            Space::new().height(4),
            self.daw_nav(Daw::Ableton),
            self.daw_nav(Daw::FlStudio),
            Space::new().height(14),
            container(text(t("TAGS")).size(11).font(SEMIBOLD).color(p.text_3)).padding([0, 10]),
            Space::new().height(4),
            self.tags_nav(),
            Space::new().height(14),
            container(text(t("FOLDERS")).size(11).font(SEMIBOLD).color(p.text_3)).padding([0, 10]),
            Space::new().height(4),
            scrollable(folder_list)
                .height(Length::Fill)
                .direction(Direction::Vertical(Scrollbar::new().width(4).scroller_width(4)))
                .style(scroll),
            container(Space::new().height(1)).style(divider).width(Length::Fill),
            Space::new().height(6),
            row![
                root_card,
                button(center(icon(icons::GEAR, 16, if self.settings_open { p.text } else { p.text_2 })))
                    .width(36)
                    .height(36)
                    .padding(0)
                    .style(nav(self.settings_open))
                    .on_press(Message::Set(if self.settings_open { settings::Set::Close } else { settings::Set::Open })),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        ]
        .spacing(2);

        container(content)
            .width(236)
            .height(Length::Fill)
            .padding(iced::Padding { top: 46.0, right: 12.0, bottom: 4.0, left: 12.0 })
            .into()
    }

    fn visible(&self) -> Vec<&Project> {
        let q = self.query.trim().to_lowercase();
        let mut list: Vec<&Project> = self
            .projects
            .iter()
            .filter(|p| match &self.filter {
                Filter::All => true,
                Filter::Recent => is_recent(p),
                Filter::WithDemo => !p.demos.is_empty(),
                // Keep the one you're listening to, so it doesn't vanish mid-play.
                Filter::Unheard => self.new_count(p) > 0 || self.now.as_ref().is_some_and(|n| n.project == p.dir),
                Filter::Daw(d) => p.daw == *d,
                Filter::Tag(t) => self.tags.has(&self.tag_key(p), t),
                Filter::Untagged => self.tags.of(&self.tag_key(p)).is_empty(),
                Filter::Issues => false,
                Filter::Group(g) => &p.group == g,
            })
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.location.to_lowercase().contains(&q)
                    || p.demos.iter().any(|d| d.name.to_lowercase().contains(&q))
            })
            .collect();
        if self.sort == Sort::Name {
            list.sort_by_key(|p| p.name.to_lowercase());
        }
        list
    }

    fn main_panel(&self) -> Element<'_, Message> {
        let p = p();
        let list = self.visible();
        let title = match &self.filter {
            Filter::All => t("All Projects").to_string(),
            Filter::Recent => t("Last 30 Days").to_string(),
            Filter::WithDemo => t("With Demos").to_string(),
            Filter::Unheard => t("New Bounces").to_string(),
            Filter::Daw(d) => d.name().to_string(),
            Filter::Tag(name) => tags::label(name),
            Filter::Untagged => t("Untagged").to_string(),
            Filter::Issues => t("Issues").to_string(),
            Filter::Group(g) if g.is_empty() => self.root().map(file_name).unwrap_or_default(),
            Filter::Group(g) => g.clone(),
        };
        let subtitle = if self.scanning && self.projects.is_empty() {
            t("Scanning…").to_string()
        } else if self.filter == Filter::Issues {
            let n = self.all_issues().len();
            if n == 0 { t("Everything follows your conventions").to_string() } else { i18n::tidy(n) }
        } else {
            i18n::n_projects(list.len())
        };

        let seg = |label: &'static str, s: Sort| {
            button(text(t(label)).size(12).font(MEDIUM))
                .padding([5, 12])
                .style(segment(self.sort == s))
                .on_press(Message::SetSort(s))
        };
        let view_seg = |ic: &'static str, v: View| {
            let active = self.view == v;
            button(icon(ic, 14, if active { p.text } else { p.text_2 }))
                .padding([5, 10])
                .style(segment(active))
                .on_press(Message::SetView(v))
        };

        let header = row![
            column![
                text(title).size(26).font(BOLD).wrapping(Wrapping::None),
                text(subtitle).size(13).color(p.text_2)
            ]
            .spacing(2)
            .width(Length::Fill)
            .clip(true),
            container(
                row![
                    icon(icons::SEARCH, 14, p.text_3),
                    text_input(t("Search projects"), &self.query)
                        .on_input(Message::Query)
                        .padding([7, 0])
                        .size(13)
                        .style(search),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([0, 14])
            .width(220)
            .style(search_box),
            container(row![seg("Recent", Sort::Recent), seg("Name", Sort::Name)].spacing(2))
                .padding(3)
                .style(segment_track),
            container(row![view_seg(icons::LIST, View::List), view_seg(icons::TILES, View::Grid)].spacing(2))
                .padding(3)
                .style(segment_track),
            button(icon(icons::REFRESH, 16, if self.scanning { p.text_3 } else { p.text }))
                .padding(9)
                .style(secondary)
                .on_press_maybe((!self.scanning).then_some(Message::Rescan)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([22, 28]);

        let body: Element<_> = if self.filter == Filter::Issues {
            self.issues_view()
        } else if list.is_empty() {
            // A first scan that hangs is almost always macOS holding the folder
            // behind a privacy prompt ("…access files in your Desktop folder").
            let waiting = self.scanning
                && self.projects.is_empty()
                && self.last_scan.is_some_and(|t| t.elapsed() > Duration::from_secs(2));
            let (msg, hint) = if waiting {
                (
                    "Waiting for permission to read this folder…",
                    Some("macOS may be asking whether Librarian can access this folder. Look for that dialog and click Allow."),
                )
            } else if self.scanning {
                ("Looking for projects…", None)
            } else if self.root_denied {
                (
                    "macOS is blocking access to this folder.",
                    Some("Allow Librarian in System Settings › Privacy & Security › Files and Folders, then press Refresh."),
                )
            } else if self.query.is_empty() {
                ("No Ableton Live or FL Studio projects found here.", None)
            } else {
                ("Nothing matches your search.", None)
            };
            let mut col = column![text(t(msg)).size(14).color(p.text_2)].spacing(6).align_x(Alignment::Center);
            if let Some(h) = hint {
                col = col.push(text(t(h)).size(12).color(p.text_3).align_x(Alignment::Center).width(Length::Fixed(380.0)));
            }
            center(col).into()
        } else if self.view == View::Grid {
            self.grid(list)
        } else {
            scrollable(
                Column::with_children(list.into_iter().map(|pr| self.card(pr)))
                    .spacing(10)
                    .padding(iced::Padding { top: 2.0, right: 28.0, bottom: 24.0, left: 28.0 }),
            )
            .height(Length::Fill)
            .direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(3)))
            .style(scroll)
            .into()
        };

        let mut col = column![header, body];
        if let Some(err) = &self.error {
            col = col.push(
                container(text(err.clone()).size(12).color(p.danger)).padding([8, 28]).width(Length::Fill),
            );
        }
        if let Some(bar) = self.player_bar() {
            col = col.push(container(Space::new().height(1)).style(divider).width(Length::Fill));
            col = col.push(bar);
        }
        col.into()
    }

    fn progress_for(&self, file: &Path) -> Option<f32> {
        let now = self.now.as_ref()?;
        if now.file != file {
            return None;
        }
        let d = self.duration_of(file)?.as_secs_f32();
        Some(if d > 0.0 { (self.position.as_secs_f32() / d).clamp(0.0, 1.0) } else { 0.0 })
    }

    fn thumb<'a>(&self, pr: &'a Project) -> canvas::Canvas<Thumb, Message> {
        canvas(Thumb { arrangement: pr.arrangement.clone(), key: pr.dir.clone() })
    }

    /// "Open" button carrying the DAW's own icon when we found it.
    fn open_button<'a>(&self, daw: Daw, path: &Path, label: Option<&'static str>) -> button::Button<'a, Message> {
        let p = p();
        let glyph: Element<'a, Message> = match self.daw_icons.get(&daw) {
            Some(h) => image(h.clone()).width(18).height(18).into(),
            None => icon(icons::EXTERNAL, 14, p.text).into(),
        };
        let content: Element<'a, Message> = match label {
            Some(l) => row![glyph, text(l).size(13).font(MEDIUM)].spacing(7).align_y(Alignment::Center).into(),
            None => glyph,
        };
        button(content)
            .padding(if label.is_some() { [7, 12] } else { [6, 6] })
            .style(secondary)
            .on_press(Message::Open(path.to_path_buf()))
    }

    fn card<'a>(&'a self, pr: &'a Project) -> Element<'a, Message> {
        let p = p();
        let expanded = self.expanded.as_ref() == Some(&pr.dir);
        let is_current = self.now.as_ref().is_some_and(|n| n.project == pr.dir);
        let playing = is_current && !self.audio.is_paused();
        let demo = self.current_demo(pr);

        let play_btn = button(center(icon(if playing { icons::PAUSE } else { icons::PLAY }, 15, p.on_accent)))
            .width(36)
            .height(36)
            .padding(0)
            .style(primary)
            .on_press_maybe(demo.map(|_| Message::PlayProject(pr.dir.clone())));

        let mut title = row![text(&pr.name).size(15).font(SEMIBOLD).wrapping(Wrapping::None)]
            .spacing(8)
            .align_y(Alignment::Center);
        if let Some(m) = &pr.meta {
            if !m.version.is_empty() {
                let (bg, fg) = if pr.daw == Daw::FlStudio { (p.fl_bg, p.fl) } else { (p.blue_bg, p.blue) };
                title = title.push(chip_text(m.version.clone(), bg, fg));
            }
        }
        if let Some(chips) = self.tag_chips(pr, 11.0) {
            title = title.push(chips);
        }
        if let Some(chip) = new_chip(self.new_count(pr)) {
            title = title.push(chip);
        }
        if is_current {
            title = title.push(chip_text(t(if playing { "Playing" } else { "Paused" }).into(), p.green_bg, p.green));
        }
        let n_issues = self.all_issues().iter().filter(|i| i.project.as_ref() == Some(&pr.dir)).count();
        if n_issues > 0 {
            title = title.push(chip_text(
                i18n::n_issues(n_issues),
                p.warn_bg,
                p.warn,
            ));
        }

        let mut sub = vec![];
        if !pr.location.is_empty() {
            sub.push(pr.location.clone());
        }
        sub.push(fmt_time(pr.modified));
        let subtitle = text(sub.join("  ·  ")).size(12).color(p.text_2).wrapping(Wrapping::None);

        let mut stats = row![].spacing(6).align_y(Alignment::Center);
        if let Some(m) = &pr.meta {
            if let Some(t) = m.tempo {
                stats = stats.push(stat(format!("{} BPM", fmt_bpm(t))));
            }
            let (n, fl) = if pr.daw == Daw::FlStudio { (m.channels, true) } else { (m.track_count(), false) };
            if n > 0 {
                stats = stats.push(stat(if fl { i18n::n_channels(n) } else { i18n::n_tracks(n) }));
            }
        }

        let mut head = row![
            play_btn,
            column![title, subtitle].spacing(3).width(Length::Fill).clip(true),
            stats,
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if let Some(s) = pr.main_set() {
            head = head.push(self.open_button(pr.daw, &s.path, Some(t("Open"))));
        }
        head = head.push(icon(if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, 16, p.text_3));

        let wave_row: Element<_> = match demo {
            Some(d) => {
                let progress = self.progress_for(&d.path);
                let dur = self.duration_of(&d.path);
                let time_label = match (progress, dur) {
                    (Some(_), Some(total)) => format!("{} / {}", fmt_dur(self.position), fmt_dur(total)),
                    (_, Some(total)) => fmt_dur(total),
                    _ => String::new(),
                };
                let v = &pr.versions[self.version_index(pr)];
                column![
                    canvas(Waveform {
                        peaks: self.peaks.get(&d.path).map(|(_, pk)| pk),
                        progress,
                        file: d.path.clone(),
                        project: pr.dir.clone(),
                        bar: 2.0,
                        gap: 2.0,
                    })
                    .width(Length::Fill)
                    .height(36),
                    row![
                        self.version_pills(pr),
                        text(format!("{}  ·  {}", fmt_time(v.modified), v.formats()))
                            .size(11)
                            .color(p.text_3)
                            .wrapping(Wrapping::None)
                            .width(Length::Fill),
                        text(time_label)
                            .size(11)
                            .color(if progress.is_some() { p.text } else { p.text_3 })
                            .font(MEDIUM),
                    ],
                ]
                .spacing(6)
                .into()
            }
            None => container(text(t("No bounce yet. Export a demo into the project folder to preview it here.")).size(12))
                .style(empty_wave)
                .padding([9, 12])
                .width(Length::Fill)
                .into(),
        };

        let right = column![head, wave_row].spacing(12).width(Length::Fill);
        let thumb_h = if demo.is_some() { 106.0 } else { 86.0 };
        let top = row![self.thumb(pr).width(196).height(thumb_h), right].spacing(16).align_y(Alignment::Center);

        let mut body = column![top].spacing(12);
        if expanded {
            body = body.push(self.details(pr));
        }

        mouse_area(
            button(body)
                .width(Length::Fill)
                .padding(10)
                .style(card(expanded))
                .on_press(Message::ToggleExpand(pr.dir.clone())),
        )
        .on_right_press(Message::ContextMenu(pr.dir.clone()))
        .into()
    }

    /// List view: one pill per version (newest first), the selected one filled.
    fn version_pills<'a>(&'a self, pr: &'a Project) -> Element<'a, Message> {
        const MAX: usize = 5;
        let p = p();
        let sel = self.version_index(pr);
        let playing_file = self.now.as_ref().filter(|n| n.project == pr.dir).map(|n| n.file.clone());
        let mut r = row![].spacing(4).align_y(Alignment::Center);
        // Always keep the selected one visible, even if it's old.
        let mut shown: Vec<usize> = (0..pr.versions.len().min(MAX)).collect();
        if sel >= MAX {
            shown.pop();
            shown.push(sel);
        }
        for i in shown {
            let v = &pr.versions[i];
            let active = i == sel;
            let is_playing = playing_file.as_ref().is_some_and(|f| v.files.iter().any(|x| &x.path == f));
            let mut label = row![].spacing(5).align_y(Alignment::Center);
            if is_playing {
                label = label.push(container(Space::new().width(6).height(6)).style(pill(p.green, p.green)));
            } else if self.version_is_new(v) {
                label = label.push(container(Space::new().width(6).height(6)).style(pill(p.blue, p.blue)));
            }
            label = label.push(text(v.label.clone()).size(11).font(if active { SEMIBOLD } else { MEDIUM }).wrapping(Wrapping::None));
            r = r.push(
                button(label)
                    .padding([3, 9])
                    .style(version_pill(active))
                    .on_press(Message::SelectVersion(pr.dir.clone(), i)),
            );
        }
        if pr.versions.len() > MAX {
            r = r.push(
                button(text(format!("+{}", pr.versions.len() - MAX)).size(11).font(MEDIUM))
                    .padding([3, 8])
                    .style(version_pill(false))
                    .on_press(Message::ToggleExpand(pr.dir.clone())),
            );
        }
        container(r).max_width(560.0).clip(true).into()
    }

    /// Grid view: `‹  test3  ›  1/3`.
    /// `width` is the tile's width, to decide whether the name fits centred.
    fn version_stepper<'a>(&'a self, pr: &'a Project, width: f32) -> Element<'a, Message> {
        let p = p();
        let sel = self.version_index(pr);
        let n = pr.versions.len();
        let arrow = |ic: &'static str, target: Option<usize>| {
            button(icon(ic, 12, if target.is_some() { p.text } else { p.text_3 }))
                .padding(3)
                .style(ghost)
                .on_press_maybe(target.map(|i| Message::SelectVersion(pr.dir.clone(), i)))
        };
        row![
            arrow(icons::CHEVRON_LEFT, (sel > 0).then(|| sel - 1)),
            // Centred when it fits; otherwise start-aligned and cut off rather
            // than running over the arrows.
            {
                let label = &pr.versions[sel].label;
                let est: f32 = label.chars().map(|c| if c.is_ascii() { 6.3 } else { 11.0 }).sum();
                let fits = est < width - 110.0;
                let c = container(text(label.clone()).size(11).font(MEDIUM).wrapping(Wrapping::None))
                    .width(Length::Fill)
                    .clip(true);
                if fits { c.center_x(Length::Fill) } else { c.align_x(Alignment::Start) }
            },
            arrow(icons::CHEVRON_RIGHT, (sel + 1 < n).then(|| sel + 1)),
            text(format!("{}/{}", sel + 1, n)).size(11).color(p.text_3),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
    }

    fn grid<'a>(&'a self, list: Vec<&'a Project>) -> Element<'a, Message> {
        responsive(move |size| {
            const PAD: f32 = 28.0;
            const GAP: f32 = 14.0;
            const MIN_TILE: f32 = 232.0;
            const TOP: f32 = 2.0;
            let avail = (size.width - 2.0 * PAD - 12.0).max(MIN_TILE);
            let cols = (((avail + GAP) / (MIN_TILE + GAP)).floor() as usize).max(1);
            let tile_w = (avail - GAP * (cols - 1) as f32) / cols as f32;

            // Only the rows on screen (plus one either side) are built; the
            // rest is empty space of the same height. With hundreds of
            // projects, building and laying out every tile on each message
            // (playback ticks, hovering, scrolling) is what made it stutter.
            let row_h = tile_height(tile_w);
            let pitch = row_h + GAP;
            let n_rows = list.len().div_ceil(cols);
            let on_screen = (self.grid_height / pitch).ceil() as usize + 1;
            let first = (((self.grid_offset - TOP) / pitch).floor().max(0.0) as usize)
                .saturating_sub(1)
                .min(n_rows.saturating_sub(on_screen + 1));
            let last = (first + on_screen + 2).min(n_rows);

            let mut col = Column::new().spacing(GAP);
            if first > 0 {
                col = col.push(Space::new().height(first as f32 * pitch - GAP));
            }
            for chunk in list.chunks(cols).skip(first).take(last - first) {
                let mut r = Row::with_children(chunk.iter().map(|pr| self.tile(pr, tile_w))).spacing(GAP).height(row_h);
                for _ in chunk.len()..cols {
                    r = r.push(Space::new().width(Length::Fill));
                }
                col = col.push(r);
            }
            if last < n_rows {
                col = col.push(Space::new().height((n_rows - last) as f32 * pitch - GAP));
            }
            scrollable(col.padding(iced::Padding { top: TOP, right: PAD, bottom: 24.0, left: PAD }))
                .height(Length::Fill)
                .direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(3)))
                .style(scroll)
                .id(GRID_ID)
                .on_scroll(|v| Message::GridScrolled(v.absolute_offset().y, v.bounds().height))
                .into()
        })
        .into()
    }

    fn tile<'a>(&'a self, pr: &'a Project, width: f32) -> Element<'a, Message> {
        let p = p();
        let selected = self.selected.as_ref() == Some(&pr.dir);
        let hovered = self.hovered.as_ref() == Some(&pr.dir);
        let is_current = self.now.as_ref().is_some_and(|n| n.project == pr.dir);
        let playing = is_current && !self.audio.is_paused();
        let demo = self.current_demo(pr);
        let thumb_h = thumb_height(width);

        let mut overlay = row![].align_y(Alignment::End);
        if demo.is_some() {
            overlay = overlay.push(
                button(center(icon(if playing { icons::PAUSE } else { icons::PLAY }, 14, rgb(0x111317))))
                    .width(34)
                    .height(34)
                    .padding(0)
                    .style(on_thumb)
                    .on_press(Message::PlayProject(pr.dir.clone())),
            );
        }
        overlay = overlay.push(Space::new().width(Length::Fill));
        if let Some(s) = pr.main_set() {
            if hovered || selected {
                overlay = overlay.push(self.open_button(pr.daw, &s.path, None));
            }
        }
        let mut top = stack![
            self.thumb(pr).width(Length::Fill).height(thumb_h),
            container(overlay).padding(10).width(Length::Fill).height(thumb_h).align_y(Alignment::End),
        ];
        if let Some(chips) = self.tag_chips_on_thumb(pr) {
            top = top.push(container(chips).padding(8).width(Length::Fill).clip(true));
        }

        let bpm = pr.meta.as_ref().and_then(|m| m.tempo).map(|t| format!("{} BPM", fmt_bpm(t))).unwrap_or_default();
        let mut name_row = row![text(&pr.name).size(14).font(SEMIBOLD).wrapping(Wrapping::None).width(Length::Fill)]
            .spacing(6)
            .align_y(Alignment::Center);
        if let Some(chip) = new_chip(self.new_count(pr)) {
            name_row = name_row.push(chip);
        }
        if is_current {
            name_row = name_row.push(chip_text(t(if playing { "Playing" } else { "Paused" }).into(), p.green_bg, p.green));
        }
        let sub = text(format!(
            "{}{}",
            fmt_time(pr.modified),
            if bpm.is_empty() { String::new() } else { format!("  ·  {bpm}") }
        ))
        .size(11)
        .color(p.text_2)
        .wrapping(Wrapping::None);

        let wave: Element<_> = match demo {
            Some(d) => canvas(Waveform {
                peaks: self.peaks.get(&d.path).map(|(_, pk)| pk),
                progress: self.progress_for(&d.path),
                file: d.path.clone(),
                project: pr.dir.clone(),
                bar: 2.0,
                gap: 1.5,
            })
            .width(Length::Fill)
            .height(22)
            .into(),
            None => container(text(t("No bounce yet")).size(11).color(p.text_3)).height(22).center_y(22).into(),
        };

        let mut info = column![name_row.clip(true), sub, Space::new().height(2), wave].spacing(3);
        if pr.versions.len() > 1 {
            info = info.push(self.version_stepper(pr, width));
        }
        // Fixed height so every row is the same height (see `grid`).
        let body = column![top, info.padding([8, 6]).height(INFO_H)].spacing(4);

        let mut area = mouse_area(
            container(body).padding(6).width(Length::Fill).height(Length::Fill).style(tile(selected, hovered)),
        )
            .on_press(Message::Select(pr.dir.clone()))
            .on_right_press(Message::ContextMenu(pr.dir.clone()))
            .on_enter(Message::Hover(Some(pr.dir.clone())))
            .on_exit(Message::Hover(None))
            .interaction(iced::mouse::Interaction::Pointer);
        if let Some(s) = pr.main_set() {
            area = area.on_double_click(Message::Open(s.path.clone()));
        }
        container(area).width(Length::Fill).height(Length::Fill).into()
    }

    fn daw_nav(&self, daw: Daw) -> Element<'_, Message> {
        let p = p();
        let active = !self.settings_open && self.filter == Filter::Daw(daw);
        let count = self.projects.iter().filter(|pr| pr.daw == daw).count();
        let glyph: Element<_> = match self.daw_icons.get(&daw) {
            Some(h) => image(h.clone()).width(17).height(17).into(),
            None => icon(icons::FILE, 16, p.text_2).into(),
        };
        button(
            row![
                glyph,
                text(daw.name())
                    .size(14)
                    .font(if active { MEDIUM } else { iced::Font::DEFAULT })
                    .width(Length::Fill),
                text(count.to_string()).size(12).color(p.text_3),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 10])
        .style(nav(active))
        .on_press(Message::SetFilter(Filter::Daw(daw)))
        .into()
    }

    fn issues_nav(&self) -> Element<'_, Message> {
        let p = p();
        let n = self.all_issues().len();
        let active = !self.settings_open && self.filter == Filter::Issues;
        let badge: Element<_> = if n > 0 {
            container(text(n.to_string()).size(11).font(SEMIBOLD))
                .padding([1, 7])
                .style(pill(p.warn_bg, p.warn))
                .into()
        } else {
            text("0").size(12).color(p.text_3).into()
        };
        button(
            row![
                icon(icons::ALERT, 16, if n > 0 { p.warn } else if active { p.text } else { p.text_2 }),
                text(t("Issues"))
                    .size(14)
                    .font(if active { MEDIUM } else { iced::Font::DEFAULT })
                    .width(Length::Fill),
                badge,
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 10])
        .style(nav(active))
        .on_press(Message::SetFilter(Filter::Issues))
        .into()
    }

    fn issues_view(&self) -> Element<'_, Message> {
        let p = p();
        let q = self.query.trim().to_lowercase();
        let issues: Vec<Issue> = self
            .all_issues()
            .into_iter()
            .filter(|i| {
                q.is_empty()
                    || i.location.to_lowercase().contains(&q)
                    || i.files.iter().any(|f| f.name.to_lowercase().contains(&q))
            })
            .collect();
        let ignored = self.prefs.ignored.len();

        if issues.is_empty() {
            let mut col = column![
                icon(icons::LOGO, 36, p.text_3),
                text(t(if q.is_empty() { "No issues. Everything is where it should be." } else { "No issues match your search." }))
                    .size(14)
                    .color(p.text_2),
            ]
            .spacing(10)
            .align_x(Alignment::Center);
            if ignored > 0 {
                col = col.push(
                    button(text(i18n::show_ignored(ignored)).size(12).font(MEDIUM))
                        .padding([6, 12])
                        .style(secondary)
                        .on_press(Message::ResetIgnored),
                );
            }
            return center(col).into();
        }

        let mut sections = Column::new().spacing(22);
        for kind in IssueKind::ALL {
            let group: Vec<&Issue> = issues.iter().filter(|i| i.kind == kind).collect();
            if group.is_empty() {
                continue;
            }
            let (fg, bg) = if kind.is_error() { (p.danger, p.danger_bg) } else { (p.warn, p.warn_bg) };
            let header = column![
                row![
                    container(Space::new().width(8).height(8)).style(pill(fg, fg)),
                    text(t(kind.title())).size(15).font(SEMIBOLD),
                    container(text(group.len().to_string()).size(11).font(SEMIBOLD))
                        .padding([1, 7])
                        .style(pill(bg, fg)),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
                text(t(kind.explain())).size(12).color(p.text_2),
            ]
            .spacing(4);

            let cards = Column::with_children(group.into_iter().map(|i| self.issue_card(i))).spacing(8);
            sections = sections.push(column![header, cards].spacing(10));
        }
        if ignored > 0 {
            sections = sections.push(
                row![
                    text(i18n::n_ignored(ignored)).size(12).color(p.text_3),
                    button(text(t("Show again")).size(12).font(MEDIUM))
                        .padding([4, 10])
                        .style(ghost)
                        .on_press(Message::ResetIgnored),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }

        scrollable(sections.padding(iced::Padding { top: 2.0, right: 28.0, bottom: 24.0, left: 28.0 }))
            .height(Length::Fill)
            .direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(3)))
            .style(scroll)
            .into()
    }

    fn issue_card<'a>(&'a self, i: &Issue) -> Element<'a, Message> {
        let p = p();
        const SHOW: usize = 6;
        let is_file = matches!(i.kind, IssueKind::LooseSet | IssueKind::UnreadableSet);
        let head = row![
            icon(if is_file { icons::FILE } else { icons::FOLDER }, 15, p.text_2),
            text(if i.location.is_empty() { t("(top of your folder)").to_string() } else { i.location.clone() })
                .size(13)
                .font(MEDIUM)
                .wrapping(Wrapping::None)
                .width(Length::Fill),
            button(text(t("Ignore")).size(12).font(MEDIUM))
                .padding([5, 10])
                .style(ghost)
                .on_press(Message::Ignore(i.key())),
            button(
                row![icon(icons::FINDER, 13, p.text), text(reveal_label()).size(12).font(MEDIUM)]
                    .spacing(6)
                    .align_y(Alignment::Center)
            )
            .padding([5, 10])
            .style(secondary)
            .on_press(Message::Reveal(i.path.clone())),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let mut col = column![head].spacing(6);
        if !i.files.is_empty() {
            let mut files = Column::new().spacing(3);
            for f in i.files.iter().take(SHOW) {
                let mut r = row![text(f.name.clone()).size(12).color(p.text_2).wrapping(Wrapping::None)]
                    .spacing(8)
                    .align_y(Alignment::Center);
                if let Some(owner) = &f.belongs_to {
                    r = r
                        .push(icon(icons::ARROW_RIGHT, 12, p.text_3))
                        .push(text(i18n::probably(owner)).size(12).color(p.blue).wrapping(Wrapping::None));
                }
                files = files.push(r);
            }
            if i.files.len() > SHOW {
                files = files.push(text(i18n::more(i.files.len() - SHOW)).size(12).color(p.text_3));
            }
            col = col.push(container(files).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 25.0 }));
        }
        container(col.clip(true)).padding([10, 14]).width(Length::Fill).style(issue_card).into()
    }

    fn details<'a>(&'a self, pr: &'a Project) -> Element<'a, Message> {
        let p = p();
        let section = |label: &'static str| text(t(label)).size(11).font(SEMIBOLD).color(p.text_3);

        let demos = Column::with_children(pr.demos.iter().map(|d| {
            let current = self.audio.current.as_deref() == Some(d.path.as_path());
            let playing = current && !self.audio.is_paused();
            let dur = self.duration_of(&d.path).map(fmt_dur).unwrap_or_default();
            button(
                row![
                    icon(if playing { icons::PAUSE } else { icons::PLAY }, 12, if current { p.text } else { p.text_2 }),
                    text(&d.name)
                        .size(13)
                        .font(if current { MEDIUM } else { iced::Font::DEFAULT })
                        .wrapping(Wrapping::None)
                        .width(Length::Fill),
                    text(dur).size(12).color(p.text_2).width(48),
                    text(fmt_size(d.size)).size(12).color(p.text_3).width(70),
                    text(fmt_time(d.modified)).size(12).color(p.text_3).width(130),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([6, 8])
            .style(ghost)
            .on_press(Message::PlayFile(pr.dir.clone(), d.path.clone()))
            .into()
        }));

        let sets = Column::with_children(pr.sets.iter().map(|s| {
            row![
                icon(icons::FILE, 13, p.text_2),
                text(&s.name).size(13).wrapping(Wrapping::None).width(Length::Fill),
                text(fmt_size(s.size)).size(12).color(p.text_3).width(70),
                text(fmt_time(s.modified)).size(12).color(p.text_3).width(130),
                self.open_button(pr.daw, &s.path, Some(t("Open"))),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding([2, 8])
            .into()
        }));

        let mut col = column![container(Space::new().height(1)).style(divider).width(Length::Fill)].spacing(10);
        col = col.push(section("TAGS")).push(self.tag_toggles(pr));
        if !pr.demos.is_empty() {
            col = col.push(section("BOUNCES")).push(demos);
        }
        col = col.push(section(if pr.daw == Daw::FlStudio { "FL PROJECT FILES" } else { "LIVE SETS" })).push(sets);
        if let Some(m) = &pr.meta {
            let mut facts = vec![];
            if m.audio_tracks > 0 {
                facts.push(i18n::fact(m.audio_tracks, "audio"));
            }
            if m.midi_tracks > 0 {
                facts.push(i18n::fact(m.midi_tracks, "MIDI"));
            }
            if m.group_tracks > 0 {
                facts.push(i18n::fact(m.group_tracks, "group"));
            }
            if m.return_tracks > 0 {
                facts.push(i18n::fact(m.return_tracks, "return"));
            }
            if m.patterns > 0 {
                facts.push(i18n::fact(m.patterns, "patterns"));
            }
            if pr.daw == Daw::FlStudio && m.channels > 0 {
                facts.push(i18n::fact(m.channels, "channels"));
            }
            if !facts.is_empty() {
                col = col.push(text(facts.join("  ·  ")).size(12).color(p.text_2));
            }
        }
        col = col.push(row![
            Space::new().width(Length::Fill),
            button(
                row![icon(icons::FINDER, 14, p.text), text(reveal_label()).size(12).font(MEDIUM)]
                    .spacing(6)
                    .align_y(Alignment::Center)
            )
            .padding([6, 12])
            .style(secondary)
            .on_press(Message::Reveal(pr.dir.clone())),
        ]);
        container(col).padding([0, 6]).into()
    }

    fn player_bar(&self) -> Option<Element<'_, Message>> {
        let p = p();
        let now = self.now.as_ref()?;
        let pr = self.project(&now.project);
        let playing = !self.audio.is_paused();
        let total = self.duration_of(&now.file);
        let progress = self.progress_for(&now.file);

        Some(
            row![
                button(center(icon(if playing { icons::PAUSE } else { icons::PLAY }, 14, p.on_accent)))
                    .width(34)
                    .height(34)
                    .padding(0)
                    .style(primary)
                    .on_press(Message::TogglePlay),
                column![
                    text(pr.map(|p| p.name.clone()).unwrap_or_default())
                        .size(13)
                        .font(SEMIBOLD)
                        .wrapping(Wrapping::None),
                    text(file_name(&now.file)).size(11).color(p.text_2).wrapping(Wrapping::None),
                ]
                .spacing(1)
                .width(200)
                .clip(true),
                text(fmt_dur(self.position)).size(11).color(p.text_2).font(MEDIUM),
                canvas(Waveform {
                    peaks: self.peaks.get(&now.file).map(|(_, pk)| pk),
                    progress,
                    file: now.file.clone(),
                    project: now.project.clone(),
                    bar: 2.0,
                    gap: 1.5,
                })
                .width(Length::Fill)
                .height(28),
                text(total.map(fmt_dur).unwrap_or_default()).size(11).color(p.text_2).font(MEDIUM),
                button(icon(icons::X, 14, p.text_2)).padding(8).style(ghost).on_press(Message::Stop),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
            .padding([12, 22])
            .into(),
        )
    }
}

// ------------------------------------------------------------------ helpers

/// "NEW" / "3 NEW" for projects with unheard bounces.
fn new_chip<'a>(n: usize) -> Option<Element<'a, Message>> {
    let p = p();
    (n > 0).then(|| chip_text(i18n::n_new(n), p.blue, Color::WHITE))
}

fn chip_text<'a>(label: String, bg: Color, fg: Color) -> Element<'a, Message> {
    container(text(label).size(11).font(MEDIUM)).padding([2, 8]).style(pill(bg, fg)).into()
}

fn stat<'a>(label: String) -> Element<'a, Message> {
    container(text(label).size(12).font(MEDIUM).wrapping(Wrapping::None))
        .padding([4, 10])
        .style(outline_pill)
        .into()
}

fn is_recent(p: &Project) -> bool {
    SystemTime::now()
        .duration_since(p.modified)
        .map(|d| d < Duration::from_secs(30 * 24 * 3600))
        .unwrap_or(true)
}

fn file_name(p: &Path) -> String {
    use unicode_normalization::UnicodeNormalization;
    p.file_name().map(|s| s.to_string_lossy().nfc().collect()).unwrap_or_default()
}

fn fmt_time(t: SystemTime) -> String {
    let dt: DateTime<Local> = t.into();
    let now = Local::now();
    let days = (now.date_naive() - dt.date_naive()).num_days();
    if i18n::zh() {
        return match days {
            0 => format!("今天 {}", dt.format("%H:%M")),
            1 => format!("昨天 {}", dt.format("%H:%M")),
            _ if dt.year() == now.year() => dt.format("%-m月%-d日 %H:%M").to_string(),
            _ => dt.format("%Y年%-m月%-d日").to_string(),
        };
    }
    match days {
        0 => format!("Today {}", dt.format("%H:%M")),
        1 => format!("Yesterday {}", dt.format("%H:%M")),
        _ if dt.year() == now.year() => dt.format("%b %-d, %H:%M").to_string(),
        _ => dt.format("%b %-d, %Y").to_string(),
    }
}

fn fmt_dur(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

fn fmt_bpm(t: f32) -> String {
    if (t - t.round()).abs() < 0.01 { format!("{}", t.round() as i32) } else { format!("{t:.2}") }
}

fn fmt_size(b: u64) -> String {
    let mb = b as f64 / 1_000_000.0;
    if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{} KB", (b / 1000).max(1)) }
}

fn reveal_label() -> &'static str {
    t(if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in Explorer" })
}

fn reveal_path(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(target_os = "windows")]
    {
        // Explorer parses its own command line: `/select,"C:\a b\c"` works,
        // but Rust's quoting of the whole argument (`"/select,C:\a b\c"`)
        // isn't understood and Explorer falls back to opening Documents.
        // It also rejects `\\?\` verbatim paths and forward slashes.
        use std::os::windows::process::CommandExt;
        let s = p.to_string_lossy().replace('/', "\\");
        let s = s.strip_prefix(r"\\?\UNC\").map(|r| format!(r"\\{r}")).unwrap_or_else(|| s.strip_prefix(r"\\?\").unwrap_or(&s).to_string());
        let _ = Command::new("explorer").raw_arg(format!("/select,\"{s}\"")).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(p.parent().unwrap_or(p)).spawn();
}

/// Height of the info block under a grid thumbnail: name, date, waveform and
/// the bounce stepper (reserved even when a project has a single bounce).
const INFO_H: f32 = 106.0;

fn thumb_height(tile_w: f32) -> f32 {
    (tile_w * 0.56).clamp(120.0, 210.0)
}

/// Height of a grid tile: padding, thumbnail, gap, info.
fn tile_height(tile_w: f32) -> f32 {
    6.0 + thumb_height(tile_w) + 4.0 + INFO_H + 6.0
}

static CURSOR: std::sync::Mutex<iced::Point> = std::sync::Mutex::new(iced::Point::ORIGIN);

fn set_cursor(p: iced::Point) {
    *CURSOR.lock().unwrap() = p;
}

fn cursor() -> iced::Point {
    *CURSOR.lock().unwrap()
}

const GRID_ID: &str = "grid";
