#!/usr/bin/env bash
# Checks the Safari extension's native half without Safari: builds its relay and helper as a
# small sandboxed program (the same entitlements as the extension, a data folder of its own),
# asks the helper for Options' rules through it, and expects them back.
#   tools/safari/selftest.sh <nextdownloadmanager-nmhost>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
helper="$1"
work="$(mktemp -d)"
trap 'rm -rf "$work" "$HOME/Library/Caches/ndm-safari-selftest"' EXIT
app="$work/RelayTest.app"
mkdir -p "$app/Contents/MacOS" "$HOME/Library/Caches/ndm-safari-selftest"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.nextdownloadmanager.app.safari.selftest</string>
<key>CFBundleName</key><string>RelayTest</string>
<key>CFBundleExecutable</key><string>RelayTest</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
xcrun swiftc -parse-as-library -module-name RelayTest -framework SafariServices \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$app/Contents/Info.plist" \
  -o "$app/Contents/MacOS/RelayTest" "$here/SafariWebExtensionHandler.swift" "$here/selftest/RelayTest.swift"
cp "$helper" "$app/Contents/MacOS/nextdownloadmanager-nmhost"

# The extension's entitlements, the data folder swapped for the test's own.
sed 's|/Library/Application Support/nextdownloadmanager/|/Library/Caches/ndm-safari-selftest/|' \
  "$here/extension.entitlements" > "$work/test.entitlements"
codesign --force --sign - --entitlements "$here/helper.entitlements" "$app/Contents/MacOS/nextdownloadmanager-nmhost"
codesign --force --sign - --entitlements "$work/test.entitlements" "$app/Contents/MacOS/RelayTest"
NDM_DATA_DIR="$HOME/Library/Caches/ndm-safari-selftest" "$app/Contents/MacOS/RelayTest"
