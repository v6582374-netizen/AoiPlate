#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUNDLE_DIR="$ROOT_DIR/target/release/bundle/osx"
APP_NAME="AoiPlate"
CLI_NAME="AoiPlate-macos-arm64"

mkdir -p "$DIST_DIR"

echo "[1/4] Building release binary..."
(
  cd "$ROOT_DIR"
  cargo build --release
)

echo "[2/4] Building macOS .app bundle..."
(
  cd "$ROOT_DIR"
  cargo bundle --release
)

APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
if [[ ! -d "$APP_PATH" ]]; then
  echo "error: app bundle not found at $APP_PATH" >&2
  exit 1
fi

echo "[3/4] Packaging artifacts..."
cp "$ROOT_DIR/target/release/$APP_NAME" "$DIST_DIR/$CLI_NAME"
tar -czf "$DIST_DIR/$CLI_NAME.tar.gz" -C "$DIST_DIR" "$CLI_NAME"
ditto -c -k --sequesterRsrc --keepParent "$APP_PATH" "$DIST_DIR/$APP_NAME.app.zip"

echo "[4/4] Writing checksums..."
(
  cd "$DIST_DIR"
  shasum -a 256 "$CLI_NAME" "$CLI_NAME.tar.gz" "$APP_NAME.app.zip" > SHA256SUMS.txt
)

echo "Done. Artifacts in: $DIST_DIR"
ls -lh "$DIST_DIR"
