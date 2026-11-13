#!/bin/sh
# Builds the Linux packages (run on Linux, after `cargo build --release`):
#   dist/librarian_<version>_amd64.deb
#   dist/Librarian-<version>-linux-x86_64.tar.gz
set -e
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
BIN=target/release/librarian
mkdir -p dist

# ---- .deb -------------------------------------------------------------------
DEB=$(mktemp -d)/librarian_${VERSION}_amd64
install -Dm755 "$BIN" "$DEB/usr/bin/librarian"
install -Dm644 packaging/linux/librarian.desktop "$DEB/usr/share/applications/librarian.desktop"
install -Dm644 assets/icon-256.png "$DEB/usr/share/icons/hicolor/256x256/apps/librarian.png"
install -Dm644 assets/icon.svg "$DEB/usr/share/icons/hicolor/scalable/apps/librarian.svg"
mkdir -p "$DEB/DEBIAN"
cat > "$DEB/DEBIAN/control" <<CONTROL
Package: librarian
Version: $VERSION
Section: sound
Priority: optional
Architecture: amd64
Maintainer: Dyrox <noreply@github.com>
Homepage: https://github.com/Dyrox/librarian
Depends: libasound2 | libasound2t64, libxkbcommon0
Recommends: xdg-desktop-portal, xdg-utils
Installed-Size: $(du -sk "$DEB/usr" | cut -f1)
Description: Browse and preview Ableton Live and FL Studio projects
 Librarian finds every Ableton Live and FL Studio project in a folder, draws a
 picture of its arrangement and lets you listen to its latest bounce.
CONTROL
dpkg-deb --build --root-owner-group "$DEB" "dist/librarian_${VERSION}_amd64.deb"

# ---- portable tarball -------------------------------------------------------
TAR=$(mktemp -d)/Librarian-$VERSION-linux-x86_64
mkdir -p "$TAR"
cp "$BIN" "$TAR/librarian"
cp packaging/linux/librarian.desktop assets/icon-256.png README.md "$TAR/"
tar -C "$(dirname "$TAR")" -czf "dist/Librarian-$VERSION-linux-x86_64.tar.gz" "$(basename "$TAR")"

ls -la dist
