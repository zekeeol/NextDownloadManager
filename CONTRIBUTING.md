# Contributing

Thanks for helping with **Next Download Manager** (`v0.0.1`). Bug reports, ideas, UI polish, and pull requests are welcome at [zekeeol/NextDownloadManager](https://github.com/zekeeol/NextDownloadManager).

## Getting set up

Primary verified development environment is **macOS Apple Silicon** (with Rust stable `>= 1.98`, Xcode Command Line Tools, Python `3.11+`, and Node.js for browser extension tests).

```sh
# Build the GPUI desktop app and all required background/CLI/host binaries
cargo build -p ndm-app -p ndm-backend -p ndm-cli -p ndm-nmhost -p ndm-mcp
cargo run -p ndm-app
```

To test changes without touching your personal download list or settings, use an isolated data directory:

```sh
NDM_DATA_DIR=/tmp/ndm-dev-data cargo run -p ndm-app
```

To build a self-contained macOS `.app` bundle (including `AppIcon.icns` and version-matched browser extensions):

```sh
bash tools/installer/build-local-macos.sh release
```

`cargo run -p ndm-labserver -- --faults "rate=2M"` starts a local test HTTP server that simulates throttled connections, disconnects, busy responses, or missing `Range` support for offline testing.

## Before you open a pull request

Run the following checks on a clean tree:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p ndm-app
cargo test -p ndm-core
cargo test -p ndm-ipc
cargo test -p ndm-crash --test in_frame
python3 tools/dev/check-layering.py
python3 tools/extension/pack.py && node tools/extension/check-pack.mjs && node tools/extension/test.mjs
```

If you modify `crates/ipc/src/api.rs` or `crates/ipc/src/lib.rs` (command/response DTOs), regenerate the JSON schema before committing:

```sh
NDM_WRITE_SCHEMA=1 cargo test -p ndm-ipc schema
```

Keep changes focused and follow these rules:

- **Clean room.** NDM contains no code, binaries, artwork, icons, sounds, or text from Internet Download Manager or any other closed-source download manager. Never disassemble, decompile, or patch another program, and never bypass licensing.
- **No `egui` reintroduction.** The legacy `crates/app` (`ndm-legacy`) frontend has been removed; all desktop UI lives in `crates/desktop` (`gpui-kit`), while `crates/core` and `crates/service` remain strictly GUI-free (`tools/dev/check-layering.py`).
- **Safe Rust (`unsafe_code = "deny"`).** Keep workspace crates portable and free of `unsafe`.
- **Keep IPC, schema, and extensions in sync.** Changes to download fields, browser handoff commands, or extension manifests (`extension/manifest*.json`) must keep `Cargo.toml` version, `docs/automation-api.schema.json`, and `tools/extension/check-pack.mjs` aligned.

## Fuzzing

Parsers handling untrusted network/server inputs have fuzz targets in [`fuzz/`](fuzz) (`cargo-fuzz`, libFuzzer): `hls`, `ytdlp_json`, `web_page`, `file_name`, `torrent`, `signature`, `click_n_load`, and `archive`:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz list
cargo +nightly fuzz run torrent fuzz/corpus/torrent fuzz/seeds/torrent -- -max_total_time=300
```

## Reporting problems

Please open an issue at [GitHub Issues](https://github.com/zekeeol/NextDownloadManager/issues) with your version (`v0.0.1` or `nextdownloadmanager-cli --version`), macOS/OS version, and reproduction steps. For security vulnerabilities, see [SECURITY.md](SECURITY.md).

Contributions are accepted under the project's license: [GNU GPL v3 or later](LICENSE).
