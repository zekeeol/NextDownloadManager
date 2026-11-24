# Architecture

How Next Download Manager (`v0.0.1`) is structured across workspace crates.

> See also [NDM native architecture](native-architecture.md) for the GPUI/core decoupling boundary, IPC protocol details, and macOS bundle layout.

## Crates

| Path | Crate | Role |
|---|---|---|
| `crates/desktop` | `ndm-app` | The native **GPUI (`gpui-kit`)** desktop app (`nextdownloadmanager`): frosted-glass window, `System / Light / Dark` themes, download list, filters, connection/segment details, torrent preview, and Settings |
| `crates/service` | `ndm-backend` | GUI-free background service host (`nextdownloadmanager-backend`): owns `Core`, Tokio runtime, loopback IPC listener, and periodic `core.tick()` |
| `crates/core` | `ndm-core` | GUI-free application core: download coordination, concurrency queue, settings, atomic `core.json` persistence, completion stats (`avg_speed`, `duration_secs`), and torrent/browser orchestration |
| `crates/engine` | `ndm-engine` | Multi-connection HTTP(S) / FTP(S) engine: `probe()`, segment `Planner`, `Worker × N`, EWMA streaming speed in `stream_body`, `HostLimits`, token-bucket `Limiter`, crash-safe `.mdm.json` resume state, checksums, and proxy/PAC scripts |
| `crates/torrent` | `ndm-torrent` | BitTorrent and magnet transfers via `librqbit` (with `patches/librqbit-tracker-comms` fixing `left=0` during magnet metadata lookup, plus multi-source extra tracker fetching) |
| `crates/ipc` | `ndm-ipc` | Single-instance `127.0.0.1` token-authenticated IPC, `DesktopRequest` enum, shared API DTOs (`DownloadInfo`, `ListAnswer`), capture rules, extension IDs, and data-dir helpers |
| `crates/cli` | `ndm-cli` | Headless CLI (`nextdownloadmanager-cli` / `nextdownloadmanager --cli …`): `get`, `resume`, queue control, and automation API commands |
| `crates/nmhost` | `ndm-nmhost` | Browser native messaging host (`com.nextdownloadmanager.host`): validates extension ID and forwards browser download/magnet handoffs to `ndm-backend` |
| `crates/mcp` | `ndm-mcp` | MCP server (`nextdownloadmanager-mcp`): exposes download listing, addition, pause/resume, and queue tools to AI assistants |
| `crates/remote` | `ndm-remote` | HTTPS remote control module (LAN server/pairing utilities) |
| `crates/debrid` | `ndm-debrid` | Debrid service API clients (Real-Debrid, AllDebrid, Premiumize, TorBox, Mega-Debrid) |
| `crates/cnl` | `ndm-cnl` | Click'n'Load 2 parser/listener on `127.0.0.1:9666` |
| `crates/extract` | `ndm-extract` | Archive extraction (`zip`, `tar`, `gz`, `xz` in-process; external 7-Zip/HatchZip support) |
| `crates/geo` | `ndm-geo` | Offline IP-to-country lookup table (`crates/geo/data/ipcountry.bin.xz`, DB-IP Lite) |
| `crates/tools` | `ndm-tools` | Helper integration (`yt-dlp`, `ffmpeg`, `gallery-dl`, `deno`) and scoped relay helpers |
| `crates/crash` | `ndm-crash` | Out-of-process minidump crash/freeze monitor and symbolication utilities |
| `crates/labserver` | `ndm-labserver` | Local HTTP `Range` + FTP/FTPS fault-injection server for automated tests |
| `extension/` | — | Companion Chromium (MV3), Firefox, and Safari WebExtension sources |
| `docs/` | — | Documentation, tutorials, and `automation-api.schema.json` |

Dependencies point strictly inward: `ndm-app` communicates with `ndm-backend` only over authenticated local IPC (`ndm-ipc`); `ndm-core` and `ndm-backend` have **zero GUI dependencies** (`python3 tools/dev/check-layering.py`).

## Engine (`crates/engine`)

```text
Download
 ├─ probe()              size, resumable?, validators (ETag / Last-Modified), file name
 ├─ Planner (pure)       segment table: when a connection is free, the largest remaining
 │                       part is split in half (planner.rs, tested by random simulation)
 ├─ Worker × N           one HTTP/1.1 connection each, reused across segments; slow ones
 │                       are dropped and their part re-split
 │    ├─ stream_body     single-connection fallback with 250ms EWMA rate sampling
 │    └─ writes at its offset into <name>.mdm.part (created sparse)
 ├─ HostLimits           learns how many connections each server accepts
 ├─ Limiter (shared)     per-download and global token buckets
 ├─ State                segment table saved atomically (craft-atomic) for crash-safe resume
 └─ Snapshot             live downloaded bytes, connections, segments, and events
```

FTP and FTPS (`ftp.rs`) share the same planner, workers, and resume state using `SIZE` / `MDTM` and `REST` + `RETR` on pooled control sessions.

## Desktop UI (`crates/desktop`)

Legacy `egui` (`crates/app` / `ndm-legacy`) has been completely removed; the desktop app (`ndm-app`) is native GPUI:

- **`src/main.rs`**: Single-instance lock (`desktop.lock`), runtime Dock icon setup (`app_icon.rs`), initial theme install, `WindowOptions` with `WindowBackgroundAppearance::Blurred` and transparent unified titlebar.
- **`src/bridge.rs`**: Background worker thread that starts `nextdownloadmanager-backend --headless --desktop-service` if needed, polls `list` / `events` / `detail`, and keeps disk/IPC off the GPUI UI thread.
- **`src/theme.rs`**: Semantic `Palette` with translucent frosted glass layers (`chrome`, `sidebar`, `toolbar`, `table_header`, `glass_card`, `surface`) and opaque elevated modals (`modal_surface`, `modal_chrome`). Implements `ThemePref` (`System`, `Light`, `Dark`) persisted in `<data_dir>/ui-theme.txt` and synchronized with `gpui_kit::component::Theme`.
- **`src/shell.rs`**: Subscribes to macOS appearance updates via `cx.observe_window_appearance`, renders the sidebar, toolbar, download list (with smoothed speed, ETA, unknown-size streaming state, and completion `avg_speed` / `duration_secs`), detail modal, torrent preview modal, and tabbed Settings (`General`, `Speed & Limits`, `BitTorrent`).

## Data & Persistence

State lives in the user's NDM data folder (overridable via `NDM_DATA_DIR`, e.g. `~/Library/Application Support/NextDownloadManager` on macOS):

- `core.json` / `core.json.bak`: atomic state & settings persistence (`crates/core/src/persistence.rs`).
- `ipc.json`: owner-only loopback port & auth token for `ndm-backend`.
- `ui-theme.txt`: saved `system` / `light` / `dark` appearance mode.
- `browser-rules.json`: private-permission site/file-type capture exclusions.
- `torrents/`: torrent session data and authorized pre-opened storage handles.

## Browser Integration

```text
page ──► extension ──native messaging──► ndm-nmhost ──ipc──► ndm-backend (ndm-core) ──events──► ndm-app
```

- `ndm-nmhost` only serves NDM's own extension IDs (`crates/ipc/src/extension.rs`, unpacked Chromium ID `mnnniimekbcglecjidnckfjigmmkmgmc`, host name `com.nextdownloadmanager.host`).
- `ndm-core` accepts the transfer (holding cookies/POST bodies in memory only) before confirming to the extension, or queues a UI event (`torrent`, `settings`, `download`) consumed by `Shell::pump`.
