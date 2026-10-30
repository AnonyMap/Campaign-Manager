#!/usr/bin/env bash
# Build dist/CampaignManager-<version>-macos-universal.dmg. Must run on macOS (Xcode CLT).
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --release -p campaign-manager --target aarch64-apple-darwin
cargo build --release -p campaign-manager --target x86_64-apple-darwin

APP="target/Campaign Manager.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/campaign-manager" \
  target/aarch64-apple-darwin/release/campaign-manager \
  target/x86_64-apple-darwin/release/campaign-manager

# Icon
ICONSET=target/icon.iconset
rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for s in 16 32 64 128 256 512; do
  sips -z $s $s crates/cm_app/assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s*2)); [ $d -le 1024 ] && sips -z $d $d crates/cm_app/assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Campaign Manager</string>
  <key>CFBundleDisplayName</key><string>Campaign Manager</string>
  <key>CFBundleIdentifier</key><string>org.campaignmanager.app</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>campaign-manager</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

# Ad-hoc signature so Apple Silicon will run it (not notarized: right-click > Open the first time).
codesign --force --deep --sign - "$APP"

mkdir -p dist
STAGE=target/dmg; rm -rf "$STAGE"; mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Campaign Manager" -srcfolder "$STAGE" -ov -format UDZO "dist/CampaignManager-$VERSION-macos-universal.dmg"
echo "Built dist/CampaignManager-$VERSION-macos-universal.dmg"
