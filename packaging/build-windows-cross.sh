#!/usr/bin/env bash
# Cross-compile the Windows build from Linux: dist/CampaignManager-<version>-windows-x64.zip
#
# Needs (one-time):
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install mingw-w64 zip        # (Fedora: mingw64-gcc zip)
#
# On a Windows PC use packaging/build-windows.ps1 instead.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET=x86_64-pc-windows-gnu
cargo build --release -p campaign-manager --target "$TARGET"

STAGE="target/windows/Campaign Manager"
rm -rf target/windows
mkdir -p "$STAGE" dist
cp "target/$TARGET/release/campaign-manager.exe" "$STAGE/Campaign Manager.exe"
cp README.md LICENSE docs/statblock-import-spec.md docs/item-import-spec.md "$STAGE/"
ZIP="dist/CampaignManager-$VERSION-windows-x64.zip"
rm -f "$ZIP"
(cd target/windows && zip -qr "../../$ZIP" "Campaign Manager")
echo "Built $ZIP"
