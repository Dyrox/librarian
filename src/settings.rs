//! The Settings page: language, appearance, DAW apps, playback, library
//! behaviour and storage. Opened from the gear in the sidebar or ⌘ ,.

use std::fmt;
use std::path::{Path, PathBuf};

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, button, column, container, image, pick_list, row, scrollable, text, toggler};
use iced::{Alignment, Element, Length, Task};

use crate::apps::{self, Daw};
use crate::i18n::{self, Lang, t};
use crate::icons::{self, icon};
use crate::style::{self, *};
use crate::{App, Filter, Message, audio, blocking, cache, fmt_size, reveal_path, scan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemePref::System => "system",
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "light" => ThemePref::Light,
            "dark" => ThemePref::Dark,
            _ => ThemePref::System,
        }
    }
}

/// An entry in the output-device drop-down.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceChoice {
    Default,
    Named(String),
}

impl fmt::Display for DeviceChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceChoice::Default => f.write_str(t("System default")),
            DeviceChoice::Named(n) => f.write_str(n),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Set {
    Open,
    Close,
    Lang(Lang),
    Theme(ThemePref),
    ChooseApp(Daw),
    AppChosen(Daw, Option<PathBuf>),
    AutoApp(Daw),
    Device(DeviceChoice),
    Devices(Vec<String>),
    PreferMp3(bool),
    Watch(bool),
    NewMarks(bool),
    CacheSize(u64),
    ClearCache,
    RevealSettings,
    RevealRoot,
    OpenUrl(&'static str),
}

const AUTHOR_URL: &str = "https://github.com/Dyrox";
const REPO_URL: &str = "https://github.com/Dyrox/librarian";

/// Made once: a fresh `Handle` every frame gets a new id, so iced re-uploads
/// the image on each redraw and it flickers.
static AVATAR: std::sync::LazyLock<image::Handle> =
    std::sync::LazyLock::new(|| image::Handle::from_bytes(&include_bytes!("../assets/avatar.png")[..]));

fn msg(s: Set) -> Message {
    Message::Set(s)
}

impl App {
    pub(crate) fn update_settings(&mut self, s: Set) -> Task<Message> {
        match s {
            Set::Open => {
                self.settings_open = true;
                self.menu = None;
                Task::batch([
                    Task::perform(blocking(audio::output_devices), |d| msg(Set::Devices(d))),
                    Task::perform(blocking(cache::cache_size), |n| msg(Set::CacheSize(n))),
                ])
            }
            Set::Close => {
                self.settings_open = false;
                Task::none()
            }
            Set::Lang(lang) => {
                self.prefs.lang = lang;
                i18n::set(lang);
                self.save_prefs();
                Task::none()
            }
            Set::Theme(theme) => {
                self.prefs.theme = theme;
                self.apply_theme();
                self.save_prefs();
                Task::none()
            }
            Set::ChooseApp(daw) => Task::perform(
                async move {
                    let dialog = rfd::AsyncFileDialog::new().set_title(i18n::choose_app(daw.name()));
                    #[cfg(target_os = "macos")]
                    let dialog = dialog.add_filter("Application", &["app"]).set_directory("/Applications");
                    #[cfg(target_os = "windows")]
                    let dialog = dialog.add_filter("Program", &["exe"]);
                    dialog.pick_file().await.map(|h| h.path().to_path_buf())
                },
                move |path| msg(Set::AppChosen(daw, path)),
            ),
            Set::AppChosen(daw, Some(path)) => {
                self.prefs.set_app(daw, Some(path));
                self.save_prefs();
                self.resolve_app(daw)
            }
            Set::AppChosen(_, None) => Task::none(),
            Set::AutoApp(daw) => {
                self.prefs.set_app(daw, None);
                self.save_prefs();
                self.resolve_app(daw)
            }
            Set::Device(choice) => {
                self.prefs.device = match choice {
                    DeviceChoice::Default => None,
                    DeviceChoice::Named(n) => Some(n),
                };
                self.audio.preferred = self.prefs.device.clone();
                self.save_prefs();
                // Move whatever is playing over to the new output, same spot.
                if let (Some(now), Some(_)) = (self.now.clone(), self.audio.current.clone()) {
                    let at = self.audio.position();
                    let paused = self.audio.is_paused();
                    let task = self.play(now.project, now.file, Some(at));
                    if paused {
                        self.audio.toggle();
                    }
                    return task;
                }
                Task::none()
            }
            Set::Devices(list) => {
                self.devices = list;
                Task::none()
            }
            Set::PreferMp3(on) => {
                self.prefs.prefer_mp3 = on;
                self.save_prefs();
                scan::order_formats(&mut self.projects, on);
                let files: Vec<_> = self
                    .projects
                    .iter()
                    .filter_map(|p| self.current_demo(p).map(|d| (d.path.clone(), d.modified)))
                    .collect();
                Task::batch(files.into_iter().map(|(f, m)| self.request_peaks(&f, m)).collect::<Vec<_>>())
            }
            Set::Watch(on) => {
                self.prefs.watch = on;
                self.save_prefs();
                if on { self.rescan() } else { Task::none() }
            }
            Set::NewMarks(on) => {
                self.prefs.new_marks = on;
                self.save_prefs();
                if !on && self.filter == Filter::Unheard {
                    self.filter = Filter::All;
                }
                Task::none()
            }
            Set::CacheSize(n) => {
                self.cache_size = Some(n);
                Task::none()
            }
            Set::ClearCache => {
                self.cache_size = None;
                Task::perform(
                    blocking(|| {
                        cache::clear_cache();
                        cache::cache_size()
                    }),
                    |n| msg(Set::CacheSize(n)),
                )
            }
            Set::RevealSettings => {
                if let Some(dir) = cache::settings_dir() {
                    reveal_path(&dir.join("prefs.txt"));
                }
                Task::none()
            }
            Set::OpenUrl(url) => {
                apps::open_with(None, Path::new(url));
                Task::none()
            }
            Set::RevealRoot => {
                if let Some(root) = self.root() {
                    reveal_path(root);
                }
                Task::none()
            }
        }
    }

    /// Finds the app for `daw` (the user's pick, else the newest installed)
    /// and its icon, off the UI thread.
    pub(crate) fn resolve_app(&self, daw: Daw) -> Task<Message> {
        let custom = self.prefs.app(daw).cloned();
        Task::perform(
            blocking(move || {
                let app = custom.or_else(|| apps::find_app(daw));
                let icon = app.as_deref().and_then(|a| apps::icon_png(daw, a));
                (app, icon)
            }),
            move |(app, icon)| Message::AppResolved(daw, app, icon),
        )
    }

    pub(crate) fn apply_theme(&mut self) {
        let dark = match self.prefs.theme {
            ThemePref::System => self.system_dark,
            ThemePref::Light => false,
            ThemePref::Dark => true,
        };
        if dark != self.dark {
            self.dark = dark;
            style::set_dark(dark);
        }
    }

    // ------------------------------------------------------------------ view

    pub(crate) fn settings_view(&self) -> Element<'_, Message> {
        let p = p();
        let header = row![
            column![
                text(t("Settings")).size(26).font(BOLD),
                text(format!("Librarian {}", env!("CARGO_PKG_VERSION"))).size(13).color(p.text_2),
            ]
            .spacing(2)
            .width(Length::Fill),
            button(text(t("Done")).size(13).font(MEDIUM)).padding([7, 16]).style(secondary).on_press(msg(Set::Close)),
        ]
        .align_y(Alignment::Center)
        .padding([22, 28]);

        let general = section(
            t("General"),
            None,
            vec![
                item(
                    None,
                    t("Language"),
                    None,
                    segmented(&[(Lang::System, t("System")), (Lang::En, "English"), (Lang::Zh, "中文")], self.prefs.lang, Set::Lang),
                ),
                item(
                    None,
                    t("Appearance"),
                    None,
                    segmented(
                        &[(ThemePref::System, t("System")), (ThemePref::Light, t("Light")), (ThemePref::Dark, t("Dark"))],
                        self.prefs.theme,
                        Set::Theme,
                    ),
                ),
                item(
                    None,
                    t("Projects folder"),
                    self.root().map(tilde),
                    row![
                        small_button(t("Change…"), Message::PickFolder),
                        button(icon(icons::FINDER, 14, p.text))
                            .padding([7, 9])
                            .style(secondary)
                            .on_press_maybe(self.root().map(|_| msg(Set::RevealRoot))),
                    ]
                    .spacing(6)
                    .into(),
                ),
            ],
        );

        let daws = section(
            t("DAW apps"),
            Some(t("The Open buttons launch projects with these apps.")),
            Daw::ALL.iter().map(|&d| self.daw_item(d)).collect(),
        );

        let mut choices = vec![DeviceChoice::Default];
        choices.extend(self.devices.iter().cloned().map(DeviceChoice::Named));
        let selected = match &self.prefs.device {
            Some(n) => {
                if !self.devices.contains(n) {
                    // Remembered device that isn't plugged in right now.
                    choices.push(DeviceChoice::Named(n.clone()));
                }
                DeviceChoice::Named(n.clone())
            }
            None => DeviceChoice::Default,
        };
        let device_list = pick_list(choices, Some(selected), |c| msg(Set::Device(c)))
            .text_size(13)
            .padding([7, 12])
            .width(240)
            .style(dropdown)
            .menu_style(dropdown_menu);

        let playback = section(
            t("Playback"),
            None,
            vec![
                item(
                    None,
                    t("Output device"),
                    Some(t("Librarian previews on this device, independent of your DAW.").into()),
                    device_list.into(),
                ),
                item(
                    None,
                    t("Preferred format"),
                    Some(t("When a bounce exists as both WAV and MP3, play this one.").into()),
                    segmented(&[(false, "WAV"), (true, "MP3")], self.prefs.prefer_mp3, Set::PreferMp3),
                ),
            ],
        );

        let ignored = self.prefs.ignored.len();
        let ignored_ctl: Element<_> = if ignored == 0 {
            text(t("None")).size(13).color(p.text_3).into()
        } else {
            row![
                text(i18n::n_ignored(ignored)).size(13).color(p.text_2),
                small_button(t("Show again"), Message::ResetIgnored),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        };
        let library = section(
            t("Library"),
            None,
            vec![
                item(
                    None,
                    t("Watch the folder for changes"),
                    Some(t("New bounces and saved projects show up on their own.").into()),
                    switch_ctl(self.prefs.watch, Set::Watch),
                ),
                item(
                    None,
                    t("Mark unheard bounces as NEW"),
                    Some(t("Shows a NEW chip until you play the bounce.").into()),
                    switch_ctl(self.prefs.new_marks, Set::NewMarks),
                ),
                item(None, t("Ignored issues"), None, ignored_ctl),
            ],
        );

        let size_label = match self.cache_size {
            Some(n) => fmt_size(n),
            None => t("Calculating…").to_string(),
        };
        let storage = section(
            t("Storage"),
            None,
            vec![
                item(
                    None,
                    t("Cache"),
                    Some(t("Thumbnails and waveforms. Rebuilt automatically when needed.").into()),
                    row![
                        text(size_label).size(13).color(p.text_2),
                        button(text(t("Clear")).size(12).font(MEDIUM))
                            .padding([6, 12])
                            .style(secondary)
                            .on_press_maybe(self.cache_size.filter(|&n| n > 0).map(|_| msg(Set::ClearCache))),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into(),
                ),
                item(
                    None,
                    t("Settings files"),
                    Some(t("Preferences, tags and listening history.").into()),
                    button(
                        row![icon(icons::FINDER, 13, p.text), text(crate::reveal_label()).size(12).font(MEDIUM)]
                            .spacing(6)
                            .align_y(Alignment::Center),
                    )
                    .padding([6, 12])
                    .style(secondary)
                    .on_press(msg(Set::RevealSettings))
                    .into(),
                ),
            ],
        );

        let cmd = if cfg!(target_os = "macos") { "⌘ ," } else { "Ctrl ," };
        let keys = section(
            t("Keyboard shortcuts"),
            None,
            vec![
                shortcut(t("Play / pause"), &[t("Space")]),
                shortcut(t("Skip back / forward 5 seconds"), &["←", "→"]),
                shortcut(t("Previous / next bounce of the playing project"), &["⇧ ←", "⇧ →"]),
                shortcut(t("Open the project in its DAW"), &[t("Double-click")]),
                shortcut(
                    t("Drag the bounce out as a file"),
                    &[if cfg!(target_os = "macos") { t("⌥ + drag") } else { t("Alt + drag") }],
                ),
                shortcut(t("Open settings"), &[cmd]),
                shortcut(t("Close a menu or the settings"), &["Esc"]),
            ],
        );

        let version = if i18n::zh() {
            format!("版本 {}", env!("CARGO_PKG_VERSION"))
        } else {
            format!("Version {}", env!("CARGO_PKG_VERSION"))
        };
        let avatar = image(AVATAR.clone())
            .width(40)
            .height(40)
            .border_radius(20);
        let about = section(
            t("About"),
            None,
            vec![
                item(
                    Some(container(icon(icons::LOGO, 22, p.text)).width(40).center_x(40).into()),
                    "Librarian",
                    Some(format!("{version}  ·  {}", t("A project browser for Ableton Live and FL Studio."))),
                    small_button(t("Source code"), msg(Set::OpenUrl(REPO_URL))),
                ),
                item(
                    Some(avatar.into()),
                    t("Made by Dyrox"),
                    Some("github.com/Dyrox".into()),
                    small_button("GitHub", msg(Set::OpenUrl(AUTHOR_URL))),
                ),
            ],
        );

        let body = column![general, daws, playback, library, storage, keys, about].spacing(24).max_width(760);
        column![
            header,
            scrollable(
                container(body)
                    .width(Length::Fill)
                    .center_x(Length::Fill)
                    .padding(iced::Padding { top: 2.0, right: 28.0, bottom: 28.0, left: 28.0 }),
            )
            .height(Length::Fill)
            .direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(3)))
            .style(scroll),
        ]
        .into()
    }

    fn daw_item(&self, daw: Daw) -> Element<'_, Message> {
        let p = p();
        let app = self.daw_apps.get(&daw);
        let custom = self.prefs.app(daw).is_some();
        let glyph: Element<_> = match self.daw_icons.get(&daw) {
            Some(h) => image(h.clone()).width(28).height(28).into(),
            None => container(icon(icons::FILE, 16, p.text_2)).width(28).center_x(28).into(),
        };
        let desc = match app {
            Some(a) => format!("{}  ·  {}", apps::app_name(a), t(if custom { "Chosen by you" } else { "Found automatically" })),
            None => t("Not found. Projects open with the system default.").to_string(),
        };
        let mut ctl = Row::new().spacing(6).align_y(Alignment::Center);
        if custom {
            ctl = ctl.push(
                button(text(t("Use automatic")).size(12).font(MEDIUM))
                    .padding([6, 10])
                    .style(ghost)
                    .on_press(msg(Set::AutoApp(daw))),
            );
        }
        ctl = ctl.push(small_button(t("Choose…"), msg(Set::ChooseApp(daw))));
        item(Some(glyph), daw.name(), Some(desc), ctl.into())
    }
}

// ---------------------------------------------------------------- pieces

fn section<'a>(title: &'static str, desc: Option<&'static str>, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let p = p();
    let mut rows = Column::new();
    let n = items.len();
    for (i, it) in items.into_iter().enumerate() {
        rows = rows.push(it);
        if i + 1 < n {
            rows = rows.push(container(Space::new().height(1)).style(divider).width(Length::Fill));
        }
    }
    let mut head = column![text(title).size(13).font(SEMIBOLD)].spacing(2);
    if let Some(d) = desc {
        head = head.push(text(d).size(12).color(p.text_2));
    }
    column![container(head).padding([0, 4]), container(rows).style(issue_card).width(Length::Fill)].spacing(8).into()
}

/// One settings row: [glyph] title / description … control.
fn item<'a>(
    glyph: Option<Element<'a, Message>>,
    title: &'static str,
    desc: Option<String>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let p = p();
    let mut label = column![text(title).size(13).font(MEDIUM)].spacing(2);
    if let Some(d) = desc {
        label = label.push(text(d).size(12).color(p.text_2).wrapping(Wrapping::WordOrGlyph));
    }
    let mut r = Row::new().spacing(12).align_y(Alignment::Center);
    if let Some(g) = glyph {
        r = r.push(g);
    }
    r.push(label.width(Length::Fill)).push(control).padding([12, 16]).into()
}

fn segmented<'a, T: Copy + PartialEq + 'static>(
    options: &[(T, &'static str)],
    current: T,
    on: fn(T) -> Set,
) -> Element<'a, Message> {
    let buttons = options.iter().map(|&(value, label)| {
        button(text(label).size(12).font(MEDIUM))
            .padding([5, 12])
            .style(segment(value == current))
            .on_press(msg(on(value)))
            .into()
    });
    container(Row::with_children(buttons).spacing(2)).padding(3).style(segment_track).into()
}

fn switch_ctl<'a>(on: bool, f: fn(bool) -> Set) -> Element<'a, Message> {
    toggler(on).on_toggle(move |b| msg(f(b))).size(20).style(style::switch).into()
}

fn small_button<'a>(label: &'static str, on: Message) -> Element<'a, Message> {
    button(text(label).size(12).font(MEDIUM)).padding([6, 12]).style(secondary).on_press(on).into()
}

fn shortcut<'a>(what: &'static str, keys: &[&'static str]) -> Element<'a, Message> {
    let caps = Row::with_children(
        keys.iter().map(|k| container(text(*k).size(11).font(MEDIUM)).padding([2, 8]).style(keycap).into()),
    )
    .spacing(4);
    item(None, what, None, caps.into())
}

/// `/Users/me/Music` → `~/Music`.
fn tilde(path: &Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}
