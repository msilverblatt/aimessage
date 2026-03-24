#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUNDLE_DIR="$PROJECT_DIR/bundle/AiMessage.app"
CONTENTS_DIR="$BUNDLE_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"

echo "Building aimessage (release)..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"

echo "Creating app bundle..."
mkdir -p "$MACOS_DIR"

# Write Info.plist
cat > "$CONTENTS_DIR/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>aimessage</string>
    <key>CFBundleIdentifier</key>
    <string>com.aimessage.server</string>
    <key>CFBundleName</key>
    <string>AiMessage</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSAppleEventsUsageDescription</key>
    <string>AiMessage needs to control Messages.app to send iMessages.</string>
</dict>
</plist>
PLIST

# Copy binary
cp "$PROJECT_DIR/target/release/aimessage" "$MACOS_DIR/aimessage"

echo ""
echo "Done! App bundle at: $BUNDLE_DIR"
echo ""
echo "To grant permissions:"
echo "  1. Open System Settings → Privacy & Security → Full Disk Access"
echo "     Click '+' and select: $BUNDLE_DIR"
echo "  2. Automation permission will be prompted on first run"
echo ""
echo "To run:"
echo "  open $BUNDLE_DIR"
