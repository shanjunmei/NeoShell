#!/bin/bash
# macOS app signing script
# Usage: ./scripts/sign-macos.sh <app-path> [identity]
# If identity is "-", uses ad-hoc signing (no certificate needed)

set -e

APP_PATH="${1:?Usage: sign-macos.sh <app-path> [identity]}"
IDENTITY="${2:--}"  # Default: ad-hoc
ENTITLEMENTS="$(dirname "$0")/entitlements.plist"

# Create entitlements if not exists
if [ ! -f "$ENTITLEMENTS" ]; then
  cat > "$ENTITLEMENTS" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/>
  <key>com.apple.security.cs.allow-jit</key><true/>
  <key>com.apple.security.cs.disable-library-validation</key><true/>
  <key>com.apple.security.network.client</key><true/>
  <key>com.apple.security.files.user-selected.read-write</key><true/>
</dict></plist>
EOF
fi

echo "Signing $APP_PATH with identity: $IDENTITY"

# Sign the binary first
codesign --force --options runtime \
  --entitlements "$ENTITLEMENTS" \
  --sign "$IDENTITY" \
  "$APP_PATH/Contents/MacOS/neoshell"

# Sign the entire app bundle
codesign --force --deep --options runtime \
  --entitlements "$ENTITLEMENTS" \
  --sign "$IDENTITY" \
  "$APP_PATH"

# Verify
codesign --verify --deep --strict --verbose=2 "$APP_PATH"
echo "Signing complete and verified."
