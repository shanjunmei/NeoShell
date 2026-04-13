#!/bin/bash
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION="0.1.0"
ARCH=$(uname -m)
APP_NAME="NeoShell"

echo "=== Building ${APP_NAME} ${VERSION} for Linux (${ARCH}) ==="

# Step 1: Build release binary
echo "[1/3] Building release binary..."
cargo build --release

# Step 2: Create directory structure
echo "[2/3] Assembling package..."
APP_DIR="dist/${APP_NAME}-${VERSION}-linux-${ARCH}"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/256x256/apps"

cp target/release/neoshell "$APP_DIR/usr/bin/"
cp assets/icon.png "$APP_DIR/usr/share/icons/hicolor/256x256/apps/neoshell.png"

# Desktop entry
cat > "$APP_DIR/usr/share/applications/neoshell.desktop" << 'DESKTOP'
[Desktop Entry]
Name=NeoShell
Comment=Cross-Platform SSH Manager
Exec=neoshell
Icon=neoshell
Terminal=false
Type=Application
Categories=System;TerminalEmulator;Network;
DESKTOP

# Step 3: Create distributable
echo "[3/3] Creating distributable..."
if command -v appimagetool &> /dev/null; then
    # AppRun script
    cat > "$APP_DIR/AppRun" << 'APPRUN'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
exec "${HERE}/usr/bin/neoshell" "$@"
APPRUN
    chmod +x "$APP_DIR/AppRun"
    cp assets/icon.png "$APP_DIR/neoshell.png"
    cp "$APP_DIR/usr/share/applications/neoshell.desktop" "$APP_DIR/"

    APPIMAGE="dist/${APP_NAME}-${VERSION}-linux-${ARCH}.AppImage"
    appimagetool "$APP_DIR" "$APPIMAGE"
    echo ""
    echo "=== Build Complete ==="
    echo "  AppImage: ${APPIMAGE}"
else
    TARBALL="dist/${APP_NAME}-${VERSION}-linux-${ARCH}.tar.gz"
    tar -czf "$TARBALL" -C dist "$(basename "$APP_DIR")"
    echo ""
    echo "=== Build Complete ==="
    TARBALL_SIZE=$(du -h "$TARBALL" | cut -f1)
    echo "  Archive: ${TARBALL} (${TARBALL_SIZE})"
fi
echo "  App dir: ${APP_DIR}"
