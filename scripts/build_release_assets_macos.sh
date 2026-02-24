#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUNDLE_DIR="$ROOT_DIR/target/release/bundle/osx"
APP_NAME="AoiPlate"
CLI_NAME="AoiPlate-macos-arm64"
DMG_NAME="$APP_NAME.dmg"

ICON_SCRIPT="$ROOT_DIR/scripts/generate_app_icon_macos.sh"
ICON_FILE="$ROOT_DIR/assets/icons/AoiPlate.icns"
TMP_DMG_DIR=""

cleanup() {
  if [[ -n "$TMP_DMG_DIR" && -d "$TMP_DMG_DIR" ]]; then
    rm -rf "$TMP_DMG_DIR"
  fi
}
trap cleanup EXIT

mkdir -p "$DIST_DIR"

echo "[1/6] Generating app icon..."
if [[ ! -x "$ICON_SCRIPT" ]]; then
  chmod +x "$ICON_SCRIPT"
fi
"$ICON_SCRIPT"
if [[ ! -f "$ICON_FILE" ]]; then
  echo "error: expected icon file not found: $ICON_FILE" >&2
  exit 1
fi

echo "[2/6] Building release binary..."
(
  cd "$ROOT_DIR"
  cargo build --release
)

echo "[3/6] Building macOS .app bundle..."
(
  cd "$ROOT_DIR"
  cargo bundle --release
)

APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
if [[ ! -d "$APP_PATH" ]]; then
  echo "error: app bundle not found at $APP_PATH" >&2
  exit 1
fi

echo "[4/6] Packaging binary + zip artifacts..."
cp "$ROOT_DIR/target/release/$APP_NAME" "$DIST_DIR/$CLI_NAME"
tar -czf "$DIST_DIR/$CLI_NAME.tar.gz" -C "$DIST_DIR" "$CLI_NAME"
ditto -c -k --sequesterRsrc --keepParent "$APP_PATH" "$DIST_DIR/$APP_NAME.app.zip"

echo "[5/6] Building DMG installer..."
TMP_DMG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/aoiplate-dmg.XXXXXX")"
cp -R "$APP_PATH" "$TMP_DMG_DIR/"
ln -s /Applications "$TMP_DMG_DIR/Applications"
rm -f "$DIST_DIR/$DMG_NAME"
hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$TMP_DMG_DIR" \
  -ov \
  -format UDZO \
  "$DIST_DIR/$DMG_NAME" >/dev/null

echo "[6/6] Writing checksums..."
(
  cd "$DIST_DIR"
  shasum -a 256 "$CLI_NAME" "$CLI_NAME.tar.gz" "$APP_NAME.app.zip" "$DMG_NAME" > SHA256SUMS.txt
)

echo "Done. Artifacts in: $DIST_DIR"
ls -lh "$DIST_DIR"
