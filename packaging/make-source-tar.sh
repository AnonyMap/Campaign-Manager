#!/usr/bin/env bash
# Create dist/campaign-manager-<version>-src.tar.gz (vendored deps not included).
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
NAME="campaign-manager-$VERSION"
mkdir -p dist
tar --exclude=./target --exclude=./dist --exclude=./.git \
    --transform "s,^\.,$NAME," -czf "dist/$NAME-src.tar.gz" .
echo "Built dist/$NAME-src.tar.gz"
