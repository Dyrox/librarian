#!/bin/sh
# Builds dist/Librarian.app (version from Cargo.toml).
#
#   ./scripts/bundle-macos.sh            this Mac's architecture
#   UNIVERSAL=1 ./scripts/bundle-macos.sh Apple Silicon + Intel in one binary
#   DMG=1 ...                             also make dist/Librarian-<version>-macos-universal.dmg
set -e
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
export MACOSX_DEPLOYMENT_TARGET=11.0

if [ "$UNIVERSAL" = 1 ]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  # One cargo run builds both architectures side by side.
  cargo build --release --target aarch64-apple-darwin --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create -output target/universal/librarian \
    target/aarch64-apple-darwin/release/librarian \
    target/x86_64-apple-darwin/release/librarian
  BIN=target/universal/librarian
  ARCH=universal
else
  cargo build --release
  BIN=target/release/librarian
  ARCH=$(uname -m)
fi

APP=dist/Librarian.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/librarian"
cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Librarian</string>
  <key>CFBundleDisplayName</key><string>Librarian</string>
  <key>CFBundleIdentifier</key><string>app.librarian.projects</string>
  <key>CFBundleExecutable</key><string>librarian</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# Ad-hoc signature: required to run on Apple Silicon (not notarized).
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
du -sh "$APP"

if [ "$DMG" = 1 ]; then
  OUT="dist/Librarian-$VERSION-macos-$ARCH.dmg"
  STAGE=$(mktemp -d)
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  rm -f "$OUT"
  hdiutil create -volname "Librarian $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$OUT" >/dev/null
  rm -rf "$STAGE"
  echo "$OUT"
fi
