#!/usr/bin/env bash
# A self-contained native app bundle for local validation; always ad hoc signed.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
profile="${1:-debug}"
if [ "$profile" = release ]; then
  cargo build --release -p ndm-app -p ndm-backend -p ndm-cli -p ndm-nmhost -p ndm-mcp
elif [ "$profile" = debug ]; then
  cargo build -p ndm-app -p ndm-backend -p ndm-cli -p ndm-nmhost -p ndm-mcp
else
  echo "Usage: $0 [debug|release]" >&2
  exit 2
fi
app="$root/target/dist/Next Download Manager.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for program in nextdownloadmanager nextdownloadmanager-backend nextdownloadmanager-cli nextdownloadmanager-nmhost nextdownloadmanager-mcp; do
  cp "$root/target/$profile/$program" "$app/Contents/MacOS/$program"
  codesign --force --sign - "$app/Contents/MacOS/$program"
done
bash tools/installer/build-native-icon.sh "$app/Contents/Resources/AppIcon.icns"
python3 tools/extension/pack.py
mkdir -p "$app/Contents/Resources/BrowserExtension"
cp -R target/extension/chromium target/extension/firefox "$app/Contents/Resources/BrowserExtension/"
cp LICENSE ATTRIBUTION.md "$app/Contents/Resources/"
python3 - "$app" <<'PY'
import pathlib, plistlib, sys, tomllib
app=pathlib.Path(sys.argv[1])
version=tomllib.loads(pathlib.Path('Cargo.toml').read_text())['workspace']['package']['version']
info=dict(CFBundleIdentifier='com.nextdownloadmanager.app', CFBundleName='Next Download Manager',
          CFBundleDisplayName='Next Download Manager', CFBundleExecutable='nextdownloadmanager',
          CFBundlePackageType='APPL', CFBundleIconFile='AppIcon.icns', CFBundleVersion=version, CFBundleShortVersionString=version,
          NSHighResolutionCapable=True, LSMinimumSystemVersion='11.0')
(app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
PY
codesign --force --sign - "$app"
codesign --verify --strict --deep "$app"
codesign -dv "$app" 2>&1
# Retire only the old generated symlink-based dev bundle, which never had an icon.
# Keep its files for recovery and make the old entry point launch the current app.
legacy="$root/target/NDM.app"
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [ -d "$legacy" ] && [ ! -L "$legacy" ] && [ -L "$legacy/Contents/MacOS/nextdownloadmanager" ] &&
   [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$legacy/Contents/Info.plist" 2>/dev/null)" = com.nextdownloadmanager.dev ]; then
  "$lsregister" -u "$legacy" || true
  backup="$(mktemp -d "$root/target/legacy-dev-bundle.XXXXXX")"
  mv "$legacy" "$backup/contents"
  ln -s "dist/Next Download Manager.app" "$legacy"
fi
# Refresh only this application's registration; no global Dock/Finder cache resets.
touch "$app"
"$lsregister" -f "$app"
printf '%s\n' "$app"
