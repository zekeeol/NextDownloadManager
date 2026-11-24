#!/usr/bin/env bash
# Native desktop icon exports. The SVG is the editable source; PNG is committed so
# packaging only needs macOS built-in tools, not an SVG renderer.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:?Usage: build-native-icon.sh output.icns}"
iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset"
trap 'rm -rf "$(dirname "$iconset")"' EXIT
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$root/crates/desktop/assets/app-icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -z "$retina" "$retina" "$root/crates/desktop/assets/app-icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$out"
