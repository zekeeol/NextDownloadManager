---
name: Next Download Manager Native Desktop
description: A compact GPUI download workspace using a refined Light & Dark shadcn/ui visual language.
colors:
  primary: "#0284c7"
  primary-active: "#0369a1"
  success: "#059669"
  warning: "#d97706"
  danger: "#e11d48"
  ink: "#0f172a"
  ink-secondary: "#334155"
  muted: "#64748b"
  line: "#e2e8f0"
  subtle-line: "#f1f5f9"
  soft: "#f1f5f9"
  chrome: "#f8fafc"
  surface: "#ffffff"
  accent: "#e0f2fe"
  selected: "#f0f9ff"
  selected-hover: "#e0f2fe"
  dark-surface: "#0f172a"
  dark-chrome: "#0b1120"
  dark-sidebar: "#0d1424"
  dark-soft: "#162033"
  dark-ink: "#f1f5f9"
  dark-primary: "#38bdf8"
  dark-success: "#34d399"
  dark-danger: "#fb7185"
  segment-divider: "#fb7185"
  backdrop: "#02061766"
typography:
  headline:
    fontFamily: ".SystemUIFont"
    fontSize: "17.5px"
    fontWeight: 600
  title:
    fontFamily: ".SystemUIFont"
    fontSize: "15px"
    fontWeight: 600
  body:
    fontFamily: ".SystemUIFont"
    fontSize: "13px"
  label:
    fontFamily: ".SystemUIFont"
    fontSize: "12px"
  caption:
    fontFamily: ".SystemUIFont"
    fontSize: "11px"
  mono:
    fontFamily: "Menlo"
    fontSize: "11px"
rounded:
  progress: "9999px"
  segment-track: "5px"
  control: "7px"
  card: "10px"
  dialog: "14px"
spacing:
  tight: "4px"
  small: "8px"
  compact: "10px"
  medium: "12px"
  regular: "16px"
  large: "20px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.surface}"
    rounded: "{rounded.control}"
    height: "32px"
    typography: "{typography.label}"
  navigation-selected:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.primary}"
    rounded: "8px"
    height: "32px"
  download-row:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    height: "68px"
  download-row-selected:
    backgroundColor: "{colors.selected}"
  download-row-hover:
    backgroundColor: "{colors.chrome}"
  download-row-focus:
    backgroundColor: "{colors.accent}"
  metadata-container:
    backgroundColor: "{colors.soft}"
    rounded: "{rounded.card}"
    padding: "14px"
  progress-track:
    backgroundColor: "{colors.line}"
    rounded: "{rounded.progress}"
    height: "6px"
  dialog:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.dialog}"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
---

# Design System: Next Download Manager Native Desktop

## Overview

**Creative North Star: "shadcn/ui, rendered natively in GPUI"**

The desktop interface uses a crisp Light & Dark slate workspace with sky blue actions, category-tinted file tiles, pill-shaped segmented tabs, native switches, and emerald/sky transfer progress. All controls are native GPUI elements and `gpui-kit 0.8.1` components.

This document applies to `crates/desktop` only (`src/theme.rs`, `src/shell.rs`, `src/main.rs`).

## Colors & Themes

- **Frosted Glass Vibrancy & Palettes (`theme::palette`)**: The main window uses `WindowBackgroundAppearance::Blurred` in `src/main.rs` with a transparent root layout in `Shell::render` so macOS vibrancy shines through translucent `chrome`, `sidebar`, `toolbar`, `table_header`, `glass_card`, and `surface` layers, while `modal_surface` and `modal_chrome` remain crisp and opaque for floating dialogs.
- **System / Light / Dark Theme (`theme::ThemePref`)**: Defaults to `System` (live-synced via `cx.observe_window_appearance`) and can be switched between `System`, `Light`, and `Dark` from the titlebar or Settings → General → Appearance, persisted in `<data_dir>/ui-theme.txt` and synchronized with `gpui_kit::component::ThemeMode` via `theme::apply`.
- **Category Tints (`theme::category_tint`)**: Download rows and detail headers display 36–38px rounded category icon tiles tinted by file type (`Torrents` indigo, `Video` rose, `Music` amber, `Compressed` teal, `Documents` sky, `Programs` emerald).

## Layout & Components

- **Titlebar (`48px`)**: Translucent frosted bar with gradient app mark, title, `v<version>` pill badge, live aggregate speed pill when transfers are active, 3-state theme switcher (`Auto`/`Light`/`Dark`), and Settings trigger.
- **Toolbar (`56px`)**: Primary `Add URL` and `Add torrent` controls, transfer action cluster (`Resume`, `Pause`, `Pause all`, `Remove`), and cleanable `236px` search input.
- **Sidebar (`216px`)**: `LIBRARY` and `CATEGORIES` sections with pill count badges, plus an inset `Main queue` card at the bottom with live active status and `Start`/`Stop` actions.
- **Download Table (`68px` rows)**: Workspace header with item count pill and quick `Open folder` / `Show details` actions; rows with left selection indicator bar, category icon tile, filename + host/progress subline, size, dot+label status with pill progress bar, colored speed indicator, and remaining time.
- **Footer (`34px`)**: Layered service health dot, active count, aggregate download speed, and free disk space.
- **Modal Dialogs (`14px` radius)**: `shadow_xl()` over translucent backdrop, context-tinted `52px` header, `segmented_tabs`, inset `section_card` containers, `Switch` toggles in BitTorrent settings, and a pinned `56px` action footer bar.

## Do's and Don'ts

- **Do** preserve keyboard navigation (`Tab`, `Space`, `Enter`, `Escape`, `Cmd-N`, `Cmd-I`, `Cmd-Q`) and accessible roles/labels.
- **Do** route all colors through `theme::palette(dark)` so Light and Dark modes stay consistent across every region and modal.
- **Don't** hardcode light-only `rgb(...)` backgrounds or text colors inside `shell.rs`.
