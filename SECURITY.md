# Security

Please report security problems privately, not in a public issue: use GitHub's
**[Report a vulnerability](https://github.com/zekeeol/NextDownloadManager/security/advisories/new)**
form on this repository (for issues inherited from upstream Mister Download Manager, upstream advisories may also apply). Include what you found, reproduction steps, and your NDM version (shown in the titlebar badge, e.g. `v0.0.1`, or via `nextdownloadmanager-cli --version`).

You will receive a response as soon as possible, and a fix will be prepared before details are disclosed publicly.

## Security Boundaries

- **Browser native messaging host & local IPC (`crates/nmhost`, `crates/ipc`):** Browser handoffs go through the native messaging host (`com.nextdownloadmanager.host`), which only responds to NDM's dedicated extension ID. The desktop backend (`ndm-backend`) binds only to `127.0.0.1` and requires the random per-instance token stored in private-permission `ipc.json`.
- **Browser context & rules (`crates/core/src/browser.rs`):** Cookies and POST bodies passed from browser extensions are kept in memory only and never written to `core.json`; cross-origin batch links strip page `Cookie` headers; site exclusions are stored in private-permission `browser-rules.json`.
- **Safe file & torrent paths (`crates/engine/src/naming.rs`, `crates/core/src/torrents.rs`):** Downloaded filenames are sanitized before forming filesystem paths, and torrent file descriptors are pre-opened with relative-path and symlink validation before being passed to `librqbit` storage.
- **Memory & code safety (`Cargo.toml`):** `unsafe_code = "deny"` is enforced across the workspace.
- **Helper downloads (`crates/tools`):** When helper tools (`yt-dlp`, `ffmpeg`, `deno`) are used or verified, signature/checksum checks are enforced before execution.

Only the latest version (`0.x`) receives security fixes.
