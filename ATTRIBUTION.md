# Third-party assets & attributions

Next Download Manager (`ndm-*`) is a modified GPUI-native fork of **Mister Download Manager** by **Ahmed Nefzaoui** and upstream contributors, distributed under **GPL-3.0-or-later** ([LICENSE](LICENSE)).

Legacy `crates/app` (`egui` / `ndm-legacy`) icon and theme directories were removed when standardizing on the native GPUI desktop interface; original author and upstream credits remain applicable.

## Bundled third-party assets

| Asset | Author | Licence | Where |
|---|---|---|---|
| flag-icons 7.6.0 (250 country flags sprite sheet) | Panayiotis Lipiridis and contributors, <https://github.com/lipis/flag-icons> | MIT — `crates/geo/data/LICENSE-flag-icons` | `crates/geo/data/flags.png` |
| DB-IP IP to Country Lite (local IP-to-country lookup table packed by `tools/geo/build.py`) | DB-IP, <https://db-ip.com>: IP Geolocation by DB-IP | Creative Commons Attribution 4.0 International, <https://creativecommons.org/licenses/by/4.0/>; edition in `crates/geo/data/SOURCE.txt` | `crates/geo/data/ipcountry.bin.xz` |
| Patrick Hand (website/README diagrams; not bundled in the app binary) | Patrick Wagesreiter | SIL Open Font License 1.1 — `tools/readme/node_modules/@fontsource/patrick-hand/LICENSE` | `docs/images/races-*.svg` |
| *Big Buck Bunny* (README video button screenshots; not bundled in the app binary) | Blender Foundation, <https://peach.blender.org> | Creative Commons Attribution 3.0 | `docs/images/video-button-*.png` |

## Upstream icon & README asset history

- The original Mister Download Manager application icon (`art/Mister Download Manager.png`) was created by Ahmed Nefzaoui; website/README block icons (`docs/images/icons/`) were created for upstream documentation.

## Archive extraction (`ndm-extract`)

Libraries used by archive extraction (compatible with GPL-3.0-or-later):

| Library | Licence | For |
|---|---|---|
| [zip](https://crates.io/crates/zip) 9.1 | MIT | `.zip`, with [deflate64](https://crates.io/crates/deflate64) (MIT) and WinZip AES ([RustCrypto](https://github.com/RustCrypto) aes, hmac, pbkdf2: MIT OR Apache-2.0) |
| [lzma-rust2](https://crates.io/crates/lzma-rust2) 0.23 | Apache-2.0 | `.xz` |
| [tar](https://crates.io/crates/tar) 0.4 | MIT OR Apache-2.0 | `.tar` |

7z and RAR archives can be extracted via external [7-Zip](https://www.7-zip.org/) (GNU LGPL, with the unRAR restriction for RAR code) when installed on the user's system; it is neither bundled nor linked into NDM binaries.

NDM contains no code, artwork, icons, sounds, or text from Internet Download Manager (see [README.md](README.md)).

## Native GPUI fork additions

- **[GPUI Kit](https://github.com/longbridge/gpui-kit)** (`gpui-kit`, `gpui-component`, `gpui-base`, `gpui-pre`): Apache-2.0.
- **Bundled Lucide icons** (via `gpui-kit-assets`): ISC.
- **Native desktop icon** (`crates/desktop/assets/app-icon.svg`, `app-icon.png`, and exported `AppIcon.icns`): original artwork for this fork, distributed under the project's GPL-3.0-or-later license.
