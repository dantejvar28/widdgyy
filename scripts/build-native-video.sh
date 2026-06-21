#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
VIDEO_DIR="$ROOT_DIR/native/video"

cd "$VIDEO_DIR"

echo "[1/2] Building Rust extension (release)..."
cargo build --release

echo "[2/2] Copying module for Python import..."
cp "$VIDEO_DIR/target/release/libwiddgyy_video.so" "$VIDEO_DIR/widdgyy_video.so"

echo "Done: $VIDEO_DIR/widdgyy_video.so"
