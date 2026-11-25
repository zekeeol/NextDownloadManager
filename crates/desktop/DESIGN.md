# Next Download Manager desktop

## Workspace

- The title bar contains search, the sidebar toggle and the appearance switch alongside the platform window controls.
- The collapsible sidebar groups library filters, download categories, the main queue and Settings.
- The command bar keeps adding links and torrents, transfer actions, view controls and the details toggle with the download pane.
- List view shows size and status at every supported width. Speed and time columns appear when the list has room. Grid view shows the same filtered downloads and selection with category icons and progress.
- The optional details pane shows the selected task's metadata and opens the full transfer dialog. Service connection, active transfers, throughput and free space appear in the bottom status bar.

## Appearance

System, Light and Dark share the same semantic tokens in `src/theme.rs`. The system mode updates with native window appearance. Explicit preferences persist in `ui-theme.txt`. Both the workspace and GPUI Kit controls use the active palette. Dialogs use opaque surfaces, scrollable content and fixed headers and footers. Settings uses a separate navigation column.

`src/shell/workspace.rs` renders the workspace. `src/shell/dialogs.rs` renders download, torrent, transfer, removal and settings dialogs. `src/shell.rs` handles the shared state and service messages.
