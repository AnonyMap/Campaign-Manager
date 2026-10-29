#!/usr/bin/env bash
# Build dist/Campaign_Manager-<version>-x86_64.AppImage on Linux.
# Uses cargo-zigbuild (if installed) to target glibc 2.28 so the AppImage runs on older distros.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET=x86_64-unknown-linux-gnu
if command -v cargo-zigbuild >/dev/null 2>&1; then
  cargo zigbuild --release -p campaign-manager --target "$TARGET.2.28"
else
  echo "cargo-zigbuild not found: building against this system's glibc"
  cargo build --release -p campaign-manager --target "$TARGET"
fi
BIN="target/$TARGET/release/campaign-manager"

APPDIR=target/AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/512x512/apps"
cp "$BIN" "$APPDIR/usr/bin/"
cp packaging/campaign-manager.desktop "$APPDIR/usr/share/applications/"
cp packaging/campaign-manager.desktop "$APPDIR/"
cp crates/cm_app/assets/icon.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/campaign-manager.png"
cp crates/cm_app/assets/icon.png "$APPDIR/campaign-manager.png"
cp crates/cm_app/assets/icon.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/campaign-manager" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

TOOL=target/appimagetool-x86_64.AppImage
if [ ! -x "$TOOL" ]; then
  curl -fsSL -o "$TOOL" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$TOOL"
fi
mkdir -p dist
OUT="dist/Campaign_Manager-$VERSION-x86_64.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT"
echo "Built $OUT"
