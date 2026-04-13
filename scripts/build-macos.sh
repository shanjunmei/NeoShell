#!/bin/bash
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION="0.1.0"
ARCH=$(uname -m)
APP_NAME="NeoShell"
BUNDLE_ID="com.neoshell.app"
DMG_NAME="${APP_NAME}-${VERSION}-macos-${ARCH}.dmg"
APP_DIR="dist/${APP_NAME}.app"

echo "=== Building ${APP_NAME} ${VERSION} for macOS (${ARCH}) ==="

# Step 1: Build release binary
echo "[1/5] Building release binary..."
RUSTFLAGS="-C linker=/usr/bin/cc" CC=/usr/bin/cc CXX=/usr/bin/c++ cargo build --release

# Step 2: Convert icon.png to .icns
echo "[2/5] Creating app icon..."
ICON_SRC="assets/icon.png"
if [ ! -f "$ICON_SRC" ]; then
    echo "ERROR: $ICON_SRC not found"
    exit 1
fi

ICONSET_DIR=$(mktemp -d)/icon.iconset
mkdir -p "$ICONSET_DIR"

sips -z 16 16     "$ICON_SRC" --out "$ICONSET_DIR/icon_16x16.png"      > /dev/null 2>&1
sips -z 32 32     "$ICON_SRC" --out "$ICONSET_DIR/icon_16x16@2x.png"   > /dev/null 2>&1
sips -z 32 32     "$ICON_SRC" --out "$ICONSET_DIR/icon_32x32.png"      > /dev/null 2>&1
sips -z 64 64     "$ICON_SRC" --out "$ICONSET_DIR/icon_32x32@2x.png"   > /dev/null 2>&1
sips -z 128 128   "$ICON_SRC" --out "$ICONSET_DIR/icon_128x128.png"    > /dev/null 2>&1
sips -z 256 256   "$ICON_SRC" --out "$ICONSET_DIR/icon_128x128@2x.png" > /dev/null 2>&1
sips -z 256 256   "$ICON_SRC" --out "$ICONSET_DIR/icon_256x256.png"    > /dev/null 2>&1
sips -z 512 512   "$ICON_SRC" --out "$ICONSET_DIR/icon_256x256@2x.png" > /dev/null 2>&1
sips -z 512 512   "$ICON_SRC" --out "$ICONSET_DIR/icon_512x512.png"    > /dev/null 2>&1
sips -z 1024 1024 "$ICON_SRC" --out "$ICONSET_DIR/icon_512x512@2x.png" > /dev/null 2>&1

iconutil -c icns "$ICONSET_DIR" -o dist/icon.icns
rm -rf "$(dirname "$ICONSET_DIR")"

# Step 3: Create .app bundle structure
echo "[3/5] Assembling .app bundle..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

cp target/release/neoshell "$APP_DIR/Contents/MacOS/"
cp dist/icon.icns "$APP_DIR/Contents/Resources/"

# Step 4: Create Info.plist
echo "[4/5] Writing Info.plist..."
cat > "$APP_DIR/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>neoshell</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

# Step 5: Create .dmg
echo "[5/5] Creating DMG installer..."
rm -rf dist/dmg
mkdir -p dist/dmg
cp -r "$APP_DIR" dist/dmg/
ln -s /Applications dist/dmg/Applications

rm -f "dist/${DMG_NAME}"
hdiutil create -volname "${APP_NAME}" -srcfolder dist/dmg -ov -format UDZO "dist/${DMG_NAME}"
rm -rf dist/dmg

# Cleanup intermediate files
rm -f dist/icon.icns

echo ""
echo "=== Build Complete ==="
DMG_PATH="dist/${DMG_NAME}"
DMG_SIZE=$(du -h "$DMG_PATH" | cut -f1)
echo "  DMG: ${DMG_PATH} (${DMG_SIZE})"
echo "  App: ${APP_DIR}"
