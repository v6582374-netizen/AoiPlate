#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_IMAGE="${1:-$ROOT_DIR/plate.png}"
ICON_DIR="$ROOT_DIR/assets/icons"
ICONSET_DIR="$ICON_DIR/AoiPlate.iconset"
OUTPUT_ICNS="$ICON_DIR/AoiPlate.icns"

if [[ ! -f "$SOURCE_IMAGE" ]]; then
  echo "error: source image not found: $SOURCE_IMAGE" >&2
  exit 1
fi

mkdir -p "$ICON_DIR"
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

resize_icon() {
  local size="$1"
  local name="$2"
  sips -s format png -z "$size" "$size" "$SOURCE_IMAGE" --out "$ICONSET_DIR/$name" >/dev/null
}

resize_icon 16 "icon_16x16.png"
resize_icon 32 "icon_16x16@2x.png"
resize_icon 32 "icon_32x32.png"
resize_icon 64 "icon_32x32@2x.png"
resize_icon 128 "icon_128x128.png"
resize_icon 256 "icon_128x128@2x.png"
resize_icon 256 "icon_256x256.png"
resize_icon 512 "icon_256x256@2x.png"
resize_icon 512 "icon_512x512.png"
resize_icon 1024 "icon_512x512@2x.png"

iconutil -c icns "$ICONSET_DIR" -o "$OUTPUT_ICNS"
rm -rf "$ICONSET_DIR"

echo "generated: $OUTPUT_ICNS"
