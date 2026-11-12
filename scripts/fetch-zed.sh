#!/bin/sh
# Shallow copy of the Zed tag the GPUI app builds against.
set -e
TAG=$(grep -o 'tag = "v[0-9.]*"' "$(dirname "$0")/../desktop/Cargo.toml" | head -1 | cut -d'"' -f2)
DIR="$(dirname "$0")/../zed-src"
[ -d "$DIR/.git" ] && [ "$(git -C "$DIR" describe --tags 2>/dev/null)" = "$TAG" ] && exit 0
rm -rf "$DIR"
git clone -q --depth 1 --branch "$TAG" https://github.com/zed-industries/zed "$DIR"
