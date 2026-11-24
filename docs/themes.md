# Themes & Frosted Glass

[← Back to docs index](README.md)

Next Download Manager (`v0.0.1`) uses a unified native GPUI theme system with **macOS frosted-glass vibrancy** and automatic **System / Light / Dark** appearance synchronization.

## Switch theme mode

You can switch appearance at any time without restarting:

- **Titlebar quick toggle**: click the theme button (`Auto` / `Light` / `Dark`) in the top-right titlebar to cycle through **System → Light → Dark**.
- **Settings**: open **Settings → General → Appearance** and pick **System**, **Light**, or **Dark**.

The chosen mode is saved immediately to `<data_dir>/ui-theme.txt` (`system`, `light`, or `dark`; default is `system`).

## Follow system Light & Dark mode

When set to **System** (default), NDM reads `window.appearance()` on launch and listens to macOS appearance notifications via `cx.observe_window_appearance`. Switching macOS between Light and Dark mode updates the entire NDM window—including toolbar, sidebar, rows, badges, and open dialogs—immediately.

## Native macOS Frosted Glass (Vibrancy)

The main GPUI window enables `WindowBackgroundAppearance::Blurred` in `crates/desktop/src/main.rs` with a transparent root layout in `Shell::render`.

- **Frosted translucent chrome**: the titlebar, footer (`chrome`), `sidebar`, `toolbar`, `table_header`, and `Main queue` card (`glass_card`) use calibrated translucent Slate tints (`rgba`) so desktop wallpaper colors subtly diffuse through.
- **Crisp legible surfaces**: the download workspace (`surface`) uses high-alpha translucency, while floating dialogs and Settings cards (`modal_surface`, `modal_chrome`) stay opaque with clean hairline borders for sharp text contrast.

For exact color tokens and design rules, see [crates/desktop/DESIGN.md](../crates/desktop/DESIGN.md) and [crates/desktop/src/theme.rs](../crates/desktop/src/theme.rs).
