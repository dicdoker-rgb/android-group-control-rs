#!/bin/zsh
set -euo pipefail

MODE="${1:-release}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_TRIPLE="$(rustc -vV | awk '/host:/ {print $2}')"
BINARY_NAME="web"
OUTPUT_DIR="$ROOT_DIR/src-tauri/binaries"

if [[ "$MODE" == "release" ]]; then
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin "$BINARY_NAME" --release
  SOURCE_BIN="$ROOT_DIR/target/release/$BINARY_NAME"
else
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin "$BINARY_NAME"
  SOURCE_BIN="$ROOT_DIR/target/debug/$BINARY_NAME"
fi

mkdir -p "$OUTPUT_DIR"
cp "$SOURCE_BIN" "$OUTPUT_DIR/${BINARY_NAME}-${TARGET_TRIPLE}"
chmod +x "$OUTPUT_DIR/${BINARY_NAME}-${TARGET_TRIPLE}"
echo "Prepared sidecar: $OUTPUT_DIR/${BINARY_NAME}-${TARGET_TRIPLE}"
