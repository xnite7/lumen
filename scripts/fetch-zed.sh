#!/bin/sh
# Shallow copy of the Zed tag the GPUI app builds against, plus couch's one
# tweak: COUCH_TEXT_CONTRAST overrides DirectWrite's enhanced contrast, so
# couch can draw softer text edges than Windows' default.
set -e
TAG=$(grep -o 'tag = "v[0-9.]*"' "$(dirname "$0")/../desktop/Cargo.toml" | head -1 | cut -d'"' -f2)
DIR="$(dirname "$0")/../zed-src"
if ! { [ -d "$DIR/.git" ] && [ "$(git -C "$DIR" describe --tags 2>/dev/null)" = "$TAG" ]; }; then
  rm -rf "$DIR"
  git clone -q --depth 1 --branch "$TAG" https://github.com/zed-industries/zed "$DIR"
fi
R="$DIR/crates/gpui_windows/src/directx_renderer.rs"
if ! grep -q COUCH_TEXT_CONTRAST "$R"; then
  sed -i \
    -e 's|grayscale_enhanced_contrast: render_params.GetGrayscaleEnhancedContrast(),|grayscale_enhanced_contrast: std::env::var("COUCH_TEXT_CONTRAST").ok().and_then(\|v\| v.parse().ok()).unwrap_or(render_params.GetGrayscaleEnhancedContrast()),|' \
    -e 's|subpixel_enhanced_contrast: render_params.GetEnhancedContrast(),|subpixel_enhanced_contrast: std::env::var("COUCH_TEXT_CONTRAST").ok().and_then(\|v\| v.parse().ok()).unwrap_or(render_params.GetEnhancedContrast()),|' \
    "$R"
  grep -q COUCH_TEXT_CONTRAST "$R" || { echo "couch text patch did not apply" >&2; exit 1; }
fi
