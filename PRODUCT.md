# Product

## Platform

Native desktop (`macOS` Apple Silicon primary via GPUI; headless/CLI/core architecture portable across Unix/Windows) + companion Chromium/Firefox WebExtension.

## Users

Users looking for a fast, free, open-source (**GPL-3.0-or-later**) desktop download manager without ads, trials, or closed-source bloat—combining multi-connection HTTP/FTP transfers, BitTorrent/magnet downloads, and browser download handoff in a clean native interface.

## Product Purpose

**Next Download Manager (`v0.0.1`)** is an independent Rust download manager built around a native **GPUI (`gpui-kit`)** desktop shell (`crates/desktop`) and a GUI-free download engine/core (`crates/core`, `crates/engine`, `crates/torrent`, `crates/service`).

## Positioning

- **Native & Lightweight**: Crisp shadcn/ui-inspired GPUI interface with macOS frosted-glass vibrancy (`WindowBackgroundAppearance::Blurred`) and automatic `System / Light / Dark` appearance syncing; stripped release macOS `.app` bundle is ~28 MB.
- **Fast Multi-Connection Engine**: Multi-segment HTTP/HTTPS/FTP/FTPS transfers with slow-connection shedding, pause/resume persistence, speed limiting (global & per-task), and completion stats (`avg_speed`, `duration_secs`).
- **Built-in BitTorrent & Magnet Support**: Metadata file preview and selective download, multi-source extra tracker subscriptions & sync, DHT/uTP/peer controls, seeding duration & share-ratio limits, and fixed tracker `left=0` reporting during magnet metadata lookup.
- **Clean-Room & Open Source**: GPL-3.0-or-later fork of Mister Download Manager with zero closed-source assets or reverse-engineered code.

## Operating Context & Distribution

- Built locally as `target/dist/Next Download Manager.app` (`tools/installer/build-local-macos.sh`, ad-hoc signed) containing `nextdownloadmanager`, `nextdownloadmanager-backend`, `nextdownloadmanager-cli`, `nextdownloadmanager-nmhost`, `nextdownloadmanager-mcp`, and version-matched browser extension packages (`target/extension/`).
- Browser integration uses the dedicated Chromium extension ID (`mnnniimekbcglecjidnckfjigmmkmgmc`) and native messaging host `com.nextdownloadmanager.host`.
- State persists atomically in `<data_dir>/core.json` (with `.bak` backup); UI theme preference persists in `<data_dir>/ui-theme.txt`.

## Product Principles & Current Scope

1. **Responsive native UI**: Transparent frosted chrome, clear status/speed/ETA visibility, full keyboard navigation, and accessible focus states in Light & Dark themes.
2. **Reliable state & safe paths**: Atomic state persistence, symlink-safe torrent storage authorization, and memory-only browser cookies.
3. **Honest boundaries**: macOS Apple Silicon is the actively verified desktop platform; Windows/Linux native GPUI testing, Safari `.appex` signing, and legacy yt-dlp/HLS/DASH/debrid workflows are not yet shipped in this GPUI release.
