#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_DIR/bundle/AiMessage.app/Contents/MacOS"

echo "Building aimessage..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"

echo "Copying binary to app bundle..."
cp "$PROJECT_DIR/target/release/aimessage" "$APP_DIR/aimessage"

echo ""
echo "Done! App bundle at: $PROJECT_DIR/bundle/AiMessage.app"
echo ""
echo "To grant permissions:"
echo "  1. Open System Settings → Privacy & Security → Full Disk Access"
echo "     Click '+' and select: $PROJECT_DIR/bundle/AiMessage.app"
echo "  2. Open System Settings → Privacy & Security → Automation"
echo "     (will be prompted on first run)"
echo ""
echo "To run:"
echo "  open $PROJECT_DIR/bundle/AiMessage.app"
echo "  # or directly:"
echo "  $APP_DIR/aimessage"
