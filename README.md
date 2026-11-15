# Librarian

A small, native project browser for music producers. Pick your projects folder once; Librarian finds every
Ableton Live and FL Studio project inside it, reads tempo / track count / Live version from the `.als`, draws a minimal
"screenshot" of its arrangement (tracks, clips in Live's colours, MIDI notes), and lines up the newest bounce
with a waveform so you can tell what that badly-named project actually is. List and Logic-style grid views,
light/dark following the system, and the "Open" button carries your installed Live's own icon.

## Stack

Rust + [iced](https://iced.rs) (GPU-rendered, no WebView), `rodio` for playback, `symphonia` for decoding
waveforms, `quick-xml` + `flate2` to read `.als` files. One binary, ~8.5 MB `.app`, same code on macOS and Windows.

## Ableton conventions it relies on

- A project is a folder named `<name> Project` (or any folder containing `Ableton Project Info/`) with at least one `.als` at its top level.
- Bounces = `.wav/.mp3/.aif/.flac` at the project's top level; newest is the preview (ties: lossless first).
- `Backup/` and `Samples/` are never scanned.
- Anything inside a folder named `legacy` (any case, any depth) is skipped: a place for old projects you don't want listed.

## Download

Every push to `main` is built by GitHub Actions and published on the
[Releases page](https://github.com/Dyrox/librarian/releases):

- **Windows**: `…-windows-x64-setup.exe` (installer) or `…-windows-x64-portable.zip` (unzip and run;
  settings stay in the `LibrarianData` folder next to `Librarian.exe`)
- **macOS**: `…-macos-universal.dmg` (Apple Silicon + Intel)
- **Linux**: `librarian_…_amd64.deb` or `…-linux-x86_64.tar.gz`

Versions are `<major>.<minor>.<build>`: major/minor come from `Cargo.toml`, the last number is the CI run.
Builds aren't signed yet: on macOS right-click → Open the first time; on Windows, SmartScreen → More info → Run anyway.

## Build it yourself

```sh
cargo run --release                          # dev
./scripts/bundle-macos.sh                    # -> dist/Librarian.app
UNIVERSAL=1 DMG=1 ./scripts/bundle-macos.sh  # universal binary + .dmg
./scripts/package-linux.sh                   # on Linux, after cargo build --release: .deb + .tar.gz
./target/release/librarian --scan DIR        # print what the scanner sees
```

Linux needs `pkg-config libasound2-dev libxkbcommon-dev libwayland-dev` to build.

## Settings

Gear button at the bottom of the sidebar, or ⌘ , (Ctrl , on Windows):

- Language (system / English / 中文) and appearance (system / light / dark)
- Which Ableton Live / FL Studio app the Open buttons use (found automatically, or pick one)
- Output device for previews, and WAV vs MP3 when a bounce exists in both
- Folder watching and NEW marks for unheard bounces
- Cache size / clear, and a shortcut list

While a bounce plays, ← / → skip back / forward 5 seconds; ⇧ ← / ⇧ → switch between bounces.

Each project's tags are saved inside it, in a hidden `.librarian-tags` file, so they stay put when a
project is moved, renamed or copied to another computer (loose `.flp` files sharing a folder get a line
each in that folder's file). Tags from older versions are moved there on the first scan.

Preferences, custom tag names and listening history live in `~/Library/Application Support/Librarian`, caches in
`~/Library/Caches/Librarian`.

Debug aids: `LIBRARIAN_ROOT=DIR` scans another folder for one session; `LIBRARIAN_FILTER=issues|settings` opens on that page; `LIBRARIAN_SCROLL=PX` scrolls the grid there after 3 s.
