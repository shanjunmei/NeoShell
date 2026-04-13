#!/bin/bash
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION="0.1.0"
APP_NAME="NeoShell"

echo "=== Building ${APP_NAME} ${VERSION} for Windows ==="

# Step 1: Build release binary
echo "[1/3] Building release binary..."
cargo build --release

# Step 2: Create directory structure
echo "[2/3] Assembling package..."
DIST_DIR="dist/${APP_NAME}-${VERSION}-windows"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

cp target/release/neoshell.exe "$DIST_DIR/"
cp assets/icon.png "$DIST_DIR/"

# Step 3: Create distributable
echo "[3/3] Creating distributable..."
if command -v candle &> /dev/null; then
    candle -ext WixUIExtension scripts/neoshell.wxs -o dist/neoshell.wixobj
    light -ext WixUIExtension dist/neoshell.wixobj -o "dist/${APP_NAME}-${VERSION}-windows.msi"
    rm -f dist/neoshell.wixobj
    echo ""
    echo "=== Build Complete ==="
    echo "  MSI: dist/${APP_NAME}-${VERSION}-windows.msi"
else
    ZIP="dist/${APP_NAME}-${VERSION}-windows.zip"
    cd dist && zip -r "$(basename "$ZIP")" "$(basename "$DIST_DIR")" && cd ..
    echo ""
    echo "=== Build Complete ==="
    echo "  Archive: ${ZIP}"
fi
echo "  App dir: ${DIST_DIR}"
