#!/usr/bin/env bash
# Build dist/Campaign_Manager-<version>-x86_64.AppImage on Linux.
# Uses cargo-zigbuild (if installed) to target glibc 2.28 so the AppImage runs on older distros.
#
# The AppImage carries "update information" pointing at this project's GitHub releases, and
# a matching .zsync file is written next to it. AppImage managers such as Gear Lever and
# AppImageUpdate use the two to find and install new versions. Set UPDATE_INFO to change
# where updates come from (a fork, say), or to an empty string to leave it out.
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
# the desktop entry, with this build's version (shown by AppImage managers)
sed "s/^X-AppImage-Version=.*/X-AppImage-Version=$VERSION/" packaging/campaign-manager.desktop > "$APPDIR/campaign-manager.desktop"
cp "$APPDIR/campaign-manager.desktop" "$APPDIR/usr/share/applications/"
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
# Updates come from the latest GitHub release of this repository (GITHUB_REPOSITORY is set
# on GitHub Actions, so a fork's builds point at the fork).
REPO="${GITHUB_REPOSITORY:-AnonyMap/Campaign-Manager}"
UPDATE_INFO="${UPDATE_INFO-gh-releases-zsync|${REPO%%/*}|${REPO##*/}|latest|Campaign_Manager-*-x86_64.AppImage.zsync}"
UPDATE_ARGS=()
if [ -n "$UPDATE_INFO" ]; then
  UPDATE_ARGS=(-u "$UPDATE_INFO")
fi
rm -f "$OUT" "$OUT.zsync"
(cd dist && ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "../$TOOL" --no-appstream "${UPDATE_ARGS[@]}" "../$APPDIR" "$(basename "$OUT")")
echo "Built $OUT"
if [ -n "$UPDATE_INFO" ]; then
  if [ -f "$OUT.zsync" ]; then
    echo "Built $OUT.zsync (update information: $UPDATE_INFO)"
  else
    echo "Warning: no .zsync was produced, so AppImage managers won't see updates" >&2
  fi
fi
