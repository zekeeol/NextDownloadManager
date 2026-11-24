# Keyboard shortcuts

[← Back to docs index](README.md)

The GPUI desktop interface (`v0.0.1`) supports keyboard shortcuts, `Tab` focus traversal, focus rings (`focus_visible`), and ARIA roles/labels (`Role::Button`, `Role::Tab`, `Role::Dialog`, focus-trapped modals).

## Global shortcuts (macOS)

| Keys | What they do |
| --- | --- |
| **⌘N** | Open **Add download** (`Add URL`) dialog |
| **⌘I** | Open **Download details** for the selected transfer |
| **⌘Q** | Quit Next Download Manager cleanly (stops owned background service & saves state) |
| **Esc** | Close the active modal dialog (`Add download`, `Add torrent`, `Download details`, `Remove`, or `Settings`) |

## Navigation & download list

| Keys | What they do |
| --- | --- |
| **Tab** / **Shift+Tab** | Move keyboard focus across toolbar controls, sidebar filters, and download rows |
| **Space** | Select the focused download row |
| **Enter** | Open **Download details** for the focused download row (same as double-clicking a row) |

## In dialogs

- Modal dialogs trap focus (`focus_trap`) while open and restore focus to the main window when closed with **Esc** or **Close**.
- Standard text inputs (`Search`, URL, folder path, speed/torrent settings, and tracker URLs textarea) support macOS text editing shortcuts (**⌘A**, **⌘C**, **⌘V**, **⌘X**, **⌘Z**).
