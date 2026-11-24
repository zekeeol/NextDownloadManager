# NDM native architecture (`v0.0.1`)

The desktop application is **Next Download Manager (`ndm-app`)**, built natively with **GPUI Kit (`gpui-kit`)**. It uses a crisp Slate/Sky/Emerald interface with macOS frosted-glass vibrancy (`WindowBackgroundAppearance::Blurred`) and automatic `System / Light / Dark` appearance tracking. It contains no webview, HTML renderer, Electron/React runtime, or inline download engine.

```text
ndm-app (crates/desktop, GPUI views + frosted theme + UI bridge)
    │ authenticated loopback IPC (127.0.0.1 + token), JSON DTOs
ndm-backend (crates/service, process lifecycle + transport)
    │ ordinary Rust library calls
ndm-core (crates/core, tasks + queues + settings + persistence)
    ├── ndm-engine (HTTP/HTTPS/FTP/FTPS, ranges, resume, checksums, limiters)
    └── ndm-torrent (metadata, selected files, tracker sync, seeding & transfers)

ndm-ipc (crates/ipc) owns the protocol DTOs shared by the adapters.
ndm-cli, ndm-nmhost and ndm-mcp remain separate adapters.
```

`ndm-core` depends on **no GUI framework** (`gpui`, `winit`, `wgpu`, etc.). It can be driven in tests, `ndm-cli`, or headless `ndm-backend` without a window. `Core::open`, `Core::handle`, `Core::tick`, and `Core::shutdown` are the host API.

Desktop commands use the tagged `DesktopRequest` enum in `crates/ipc/src/desktop.rs`. The service binds an ephemeral port on `127.0.0.1` and requires the random token from owner-only `ipc.json`. The UI bridge serializes requests on a worker thread, polls `list` / `events` / `detail`, and forwards snapshots through a channel without opening downloaded files or mutating task records directly.

Task summaries are `DownloadInfo` / `ListAnswer` (`crates/ipc/src/api.rs`). Connection and segment details are serializable snapshots that survive pause and restart. Transient browser cookies, headers, and POST bodies stay in memory only. Completed downloads record `avg_speed` and `duration_secs`, and legacy completed entries missing `size` automatically backfill from `downloaded` or file metadata on startup.

Torrents are previewed before adding, and selection indices are validated by `ndm-core`. Magnet links pasted into Add URL automatically enter torrent preview. Metadata lookup reports tracker, DHT-node, and peer counts; `patches/librqbit-tracker-comms` fixes `left=0` / `is_completed=true` reporting during magnet metadata lookup so strict trackers return seeders properly. Multi-source extra tracker subscriptions sync into `extra_trackers`, and discovered preview peers are passed into transfers. Symbolic-link destinations and unsafe paths are rejected before authorizing storage handles. Completed torrents honor configurable seeding toggle, duration, and share-ratio limits.

## Build and run

```sh
cargo build -p ndm-app -p ndm-backend -p ndm-cli -p ndm-nmhost -p ndm-mcp
cargo run -p ndm-app
```

The desktop locates `nextdownloadmanager-backend` beside its own executable. It starts an owned child only if no service answers; quitting saves and stops that child. An independently started service remains alive when the desktop closes.

```sh
# Headless host; no GUI libraries are needed to run this process.
cargo run -p ndm-backend -- --headless --data /absolute/path/to/data

# An isolated desktop instance for local QA.
NDM_DATA_DIR=/absolute/path/to/qa-data cargo run -p ndm-app

cargo test -p ndm-app
cargo test -p ndm-core
cargo test -p ndm-ipc
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/dev/check-layering.py
```

## Compatibility & Legacy Removal

The legacy `egui` frontend (`crates/app` / `ndm-legacy`) and all `egui` / `eframe` / `winit` / `wgpu` patches have been completely removed from the workspace. GPUI (`crates/desktop`) is the sole desktop UI.

NDM uses its own `NextDownloadManager` application-data directory (or `NDM_DATA_DIR`) and atomic `core.json` (with `.bak` backup). Engine `.mdm.json` / `.mdm.part` resume files retain their on-disk format for crash-safe resumption. Original upstream URLs, third-party signing keys, and GPL-3.0-or-later license terms remain respected.

## Local macOS bundle

```sh
bash tools/installer/build-local-macos.sh release
open "target/dist/Next Download Manager.app"
```

Both debug and release local bundles use **ad hoc signing** (`codesign --sign -`), with bundle identifier `com.nextdownloadmanager.app` (`v0.0.1`). After stripping debug symbols (`strip -S -x`) in release mode, the full `.app` bundle (all 5 binaries + icon + bundled Chromium/Firefox extensions) is ~28 MB.

## Licensing

The project remains **GPL-3.0-or-later** ([LICENSE](../LICENSE)). Workspace `authors` lists `["zekeeol <zekeeol@proton.me>", "Ahmed Nefzaoui"]` while preserving original author attribution in [ATTRIBUTION.md](../ATTRIBUTION.md). GPUI Kit is Apache-2.0 and its Lucide icons are ISC; original author credits do not imply endorsement of this fork.

## Browser extension adapter

`ndm-nmhost` confirms `DesktopRequest::Browser` with `ndm-core` before acknowledging the browser. UI-only requests enter a bounded event queue consumed by the desktop worker; `ndm-core` holds no GPUI types. Matching unpacked extension files (`v0.0.1`) are packaged in `target/extension/` and inside the macOS bundle (`Contents/Resources/BrowserExtension/`). See [native browser extension setup and validation](native-browser-extension.md).
