#!/usr/bin/env bash
# Generate crux.icns from SVG source
# Requires: rsvg-convert (brew install librsvg), iconutil (Xcode)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SVG_SOURCE="${PROJECT_ROOT}/extra/crux-icon.svg"
ICONSET_DIR="${PROJECT_ROOT}/resources/crux.iconset"
ICNS_OUTPUT="${PROJECT_ROOT}/resources/crux.icns"

# Check dependencies
if ! command -v rsvg-convert &>/dev/null; then
    echo "Error: rsvg-convert not found. Install with: brew install librsvg"
    exit 1
fi
if ! command -v iconutil &>/dev/null; then
    echo "Error: iconutil not found. Install Xcode Command Line Tools."
    exit 1
fi
if [ ! -f "$SVG_SOURCE" ]; then
    echo "Error: SVG source not found at $SVG_SOURCE"
    exit 1
fi

echo "==> Generating icon from: $SVG_SOURCE"

# Create iconset directory
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

# macOS icon sizes: name → pixel size
# Standard (1x) and Retina (2x) variants
SIZES=(
    "icon_16x16:16"
    "icon_16x16@2x:32"
    "icon_32x32:32"
    "icon_32x32@2x:64"
    "icon_128x128:128"
    "icon_128x128@2x:256"
    "icon_256x256:256"
    "icon_256x256@2x:512"
    "icon_512x512:512"
    "icon_512x512@2x:1024"
)

for entry in "${SIZES[@]}"; do
    name="${entry%%:*}"
    pixels="${entry##*:}"
    output="${ICONSET_DIR}/${name}.png"
    echo "    ${name}.png (${pixels}x${pixels})"
    rsvg-convert -w "$pixels" -h "$pixels" "$SVG_SOURCE" -o "$output"
done

echo "==> Converting iconset to icns"
iconutil --convert icns "$ICONSET_DIR" --output "$ICNS_OUTPUT"

echo "==> Cleaning up iconset directory"
rm -rf "$ICONSET_DIR"

echo "==> Done: $ICNS_OUTPUT"
ls -lh "$ICNS_OUTPUT"
