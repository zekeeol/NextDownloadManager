# Review checklist

What to check in a pull request, based on what its diff touches.

## Every PR

- [ ] Verified in the real app/environment as a user runs it (`cargo clippy --workspace --all-targets -- -D warnings`, crate tests, and `build-local-macos.sh` when touching desktop/packaging), with explicit notes on what was verified and what was not.
- [ ] `CHANGELOG.md` and relevant docs (`README.md`, `crates/desktop/DESIGN.md`) reflect user-visible or architectural changes.
- [ ] Visible UI changes were checked in both **Light** and **Dark** modes (including `System` frosted-glass appearance, contrast, and 900×600 compact window layout).

## Desktop UI & Themes (`crates/desktop`)

- [ ] All colors flow through `theme::palette(dark)` (no hardcoded light-only colors in `shell.rs`).
- [ ] Translucent frosted layers (`WindowBackgroundAppearance::Blurred`) keep modal dialogs (`modal_surface` / `modal_chrome`) opaque and legible.
- [ ] `crates/core` and `crates/service` remain free of GUI dependencies (`python3 tools/dev/check-layering.py`).

## IPC, Schema & Browser Extensions

- **Triggered by:** changes to `crates/ipc` DTOs or `extension/`.
  - [ ] `docs/automation-api.schema.json` is up to date (`NDM_WRITE_SCHEMA=1 cargo test -p ndm-ipc schema` + `cargo test -p ndm-ipc`).
  - [ ] Extension manifests (`extension/manifest*.json`) match `Cargo.toml` version (`python3 tools/extension/pack.py && node tools/extension/check-pack.mjs && node tools/extension/test.mjs`).
  - [ ] Cross-origin batch links do not leak page `Cookie` headers; browser handoff only confirms after backend acceptance.

## Files, Paths & Torrents

- **Triggered by:** paths built from URLs, server headers, filenames, or torrent metadata.
  - [ ] Only sanitized plain relative names/paths pass (`crates/engine/src/naming.rs`, `ndm_torrent::check_relative_path`); no `/`, `\`, `..`, symlinks, or escape outside the destination folder.
  - [ ] File modifications/deletions only operate on files authorized/created for that download.

## Tests & Safety

- [ ] `unsafe_code = "deny"` remains unbroken across workspace crates.
- [ ] Time/network/process tests avoid flaky fixed sleeps without polling/timeouts and pass consistently.
