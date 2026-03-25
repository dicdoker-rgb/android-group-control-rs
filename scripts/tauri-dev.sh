#!/bin/zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin web
cd "$ROOT_DIR/frontend"
npm run dev
