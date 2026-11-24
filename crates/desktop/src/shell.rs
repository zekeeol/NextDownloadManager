use crate::{bridge::Bridge, theme as t};
use gpui_kit::{
    assets::IconName,
    base::FocusTrapElement,
    component::{
        Disableable, Icon,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        switch::Switch,
    },
    prelude::*,
    *,
};
use ndm_ipc::api::{DownloadInfo, ListAnswer};
use serde_json::{Value, json};
use std::{collections::HashSet, time::Duration};

gpui_kit::actions!(ndm, [ClosePanel, NewDownload, ShowDetails]);

pub struct Shell {
    focus: FocusHandle,
    row_focus: std::collections::HashMap<u64, FocusHandle>,
    pub bridge: Bridge,
    theme_pref: t::ThemePref,
    dark: bool,
    _appearance_sub: Subscription,
    list: ListAnswer,
    selected: Option<u64>,
    filter: String,
    search: Entity<InputState>,
    source: Entity<InputState>,
    folder: Entity<InputState>,
    limit: Entity<InputState>,
    panel: Option<&'static str>,
    detail: Value,
    tab: usize,
    details: bool,
    notice: Option<String>,
    ready: bool,
    torrent_token: Option<String>,
    torrent: Value,
    chosen: HashSet<usize>,
    subfolder: bool,
    connections: Entity<InputState>,
    settings_tab: usize,
    max_concurrent: Entity<InputState>,
    global_dl_limit: Entity<InputState>,
    per_dl_limit: Entity<InputState>,
    torrent_ul_limit: Entity<InputState>,
    torrent_seed: bool,
    torrent_seed_minutes: Entity<InputState>,
    torrent_seed_ratio: Entity<InputState>,
    torrent_peer_limit: Entity<InputState>,
    torrent_dht: bool,
    torrent_utp: bool,
    tracker_urls: Entity<TextareaState>,
    extra_trackers_count: usize,
    trackers_updated: Option<String>,
    trackers_syncing: bool,
    trackers_error: Option<String>,
    browser_events: std::collections::VecDeque<Value>,
    _poll: Task<()>,
}

fn row() -> Div {
    div().flex().items_center()
}

fn col() -> Div {
    div().flex().flex_col()
}

fn text(value: impl Into<SharedString>) -> Div {
    div().child(value.into())
}

fn muted(value: impl Into<SharedString>, p: &t::Palette) -> Div {
    text(value).text_color(p.muted).text_size(px(12.))
}

fn field_label(value: impl Into<SharedString>, p: &t::Palette) -> Div {
    text(value)
        .text_color(p.ink_secondary)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
}

fn helper_text(value: impl Into<SharedString>, p: &t::Palette) -> Div {
    text(value).text_color(p.muted).text_size(px(11.5))
}

fn btn(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .h(px(32.))
        .rounded(px(7.))
        .text_size(px(12.))
}

fn icon(name: IconName) -> Icon {
    Icon::new(name).size(px(16.))
}

fn input(
    value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.to_owned())
            .placeholder(placeholder.to_owned())
    })
}

fn bytes(n: u64) -> String {
    let (v, u) = if n >= 1 << 30 {
        (n as f64 / (1u64 << 30) as f64, "GB")
    } else if n >= 1 << 20 {
        (n as f64 / (1u64 << 20) as f64, "MB")
    } else if n >= 1024 {
        (n as f64 / 1024., "KB")
    } else {
        (n as f64, "B")
    };
    format!("{v:.2} {u}")
}

fn fraction(d: &DownloadInfo) -> f32 {
    if d.status == "Complete" {
        return 1.;
    }
    d.size.filter(|n| *n > 0).map_or(0., |n| {
        (d.downloaded as f64 / n as f64).clamp(0., 1.) as f32
    })
}

fn display_size(d: &DownloadInfo) -> String {
    if let Some(total) = d.size.filter(|n| *n > 0) {
        bytes(total)
    } else if d.downloaded > 0 {
        bytes(d.downloaded)
    } else {
        "Unknown".into()
    }
}

fn eta_remaining(d: &DownloadInfo) -> Option<u64> {
    if d.status == "Downloading" && d.speed > 0 {
        let total = d.size.filter(|n| *n > 0)?;
        let rem = total.saturating_sub(d.downloaded);
        Some(rem.div_ceil(d.speed).max(1))
    } else {
        None
    }
}

fn category_icon(category: &str) -> IconName {
    match category {
        "Torrents" => IconName::Magnet,
        "Video" => IconName::Video,
        "Music" => IconName::Music,
        "Compressed" => IconName::Archive,
        "Documents" => IconName::FileText,
        "Programs" => IconName::AppWindow,
        _ => IconName::File,
    }
}

fn is_error_status(status: &str) -> bool {
    let lower = status.to_lowercase();
    lower.contains("error") || lower.contains("fail")
}

fn progress_bar(
    value: f32,
    height: f32,
    active: bool,
    complete: bool,
    error: bool,
    indeterminate: bool,
    p: &t::Palette,
) -> Div {
    let clamped = if indeterminate {
        0.35
    } else {
        value.clamp(0., 1.)
    };
    let fill = div().h_full().w(relative(clamped)).rounded_full();
    let fill = if error {
        fill.bg(p.danger)
    } else if complete {
        fill.bg(p.success)
    } else if active {
        fill.bg(linear_gradient(
            90.,
            linear_color_stop(p.primary_bright, 0.),
            linear_color_stop(p.primary, 1.),
        ))
    } else {
        fill.bg(p.muted)
    };
    div()
        .h(px(height))
        .w_full()
        .rounded_full()
        .bg(p.progress_track)
        .overflow_hidden()
        .when(clamped > 0., |d| d.child(fill))
}

fn status_badge(status: &str, downloading: bool, has_size: bool, pct: f32, p: &t::Palette) -> Div {
    let seeding = status == "Seeding";
    let complete = status == "Complete";
    let error = is_error_status(status);
    let paused_or_queued = matches!(status, "Paused" | "Queued" | "Waiting");
    let (dot_color, text_color) = if downloading {
        (p.primary, p.primary)
    } else if complete || seeding {
        (p.success, p.success)
    } else if error {
        (p.danger, p.danger)
    } else if paused_or_queued {
        (p.warning, p.ink_secondary)
    } else {
        (p.muted, p.muted)
    };
    let label = if downloading {
        if has_size {
            format!("{pct:.1}%")
        } else {
            "Downloading".into()
        }
    } else {
        status.to_owned()
    };
    row()
        .gap(px(6.))
        .child(
            div()
                .size(px(6.))
                .rounded_full()
                .flex_shrink_0()
                .bg(dot_color),
        )
        .child(
            text(label)
                .text_size(px(12.))
                .font_weight(if downloading || complete || seeding {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(text_color)
                .text_ellipsis(),
        )
}

fn section_card(p: &t::Palette) -> Div {
    col()
        .p(px(14.))
        .gap(px(12.))
        .rounded(px(10.))
        .bg(p.soft)
        .border_1()
        .border_color(p.border_subtle)
}

fn icon_tile(ico: IconName, color: Rgba, bg: Rgba, tile_size: f32, icon_size: f32) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .size(px(tile_size))
        .rounded(px(if tile_size >= 34. { 9. } else { 7. }))
        .bg(bg)
        .child(icon(ico).size(px(icon_size)).text_color(color))
}

fn cell(value: impl Into<SharedString>, width: f32) -> Div {
    text(value)
        .w(px(width))
        .flex_shrink_0()
        .px(px(12.))
        .overflow_hidden()
        .text_ellipsis()
}

fn parse_tracker_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for token in line.split_whitespace() {
            let token = token.trim();
            if !token.is_empty() && seen.insert(token.to_owned()) {
                out.push(token.to_owned());
            }
        }
    }
    out
}

fn switch_row(
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    checked: bool,
    p: &t::Palette,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> Div {
    row()
        .justify_between()
        .gap(px(12.))
        .p(px(10.))
        .rounded(px(8.))
        .bg(p.modal_surface)
        .border_1()
        .border_color(p.border_subtle)
        .child(
            col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    text(title)
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ink),
                )
                .child(helper_text(subtitle, p)),
        )
        .child(
            Switch::new(id)
                .checked(checked)
                .accessibility_label(title)
                .on_click(on_change),
        )
}

fn duration(s: u64) -> String {
    if s >= 3600 {
        format!("{}h {}m", s / 3600, s / 60 % 60)
    } else if s >= 60 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.bind_keys([
            KeyBinding::new("escape", ClosePanel, None),
            KeyBinding::new("cmd-n", NewDownload, None),
            KeyBinding::new("cmd-i", ShowDetails, None),
        ]);
        let search = input("", "Search downloads…", window, cx);
        cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify())
            .detach();
        let source = input("", "https://example.com/file.zip", window, cx);
        let folder = input(
            &dirs::download_dir().unwrap_or_default().to_string_lossy(),
            "Destination folder",
            window,
            cx,
        );
        let connections = input("8", "Connections (1–128)", window, cx);
        let max_concurrent = input("4", "Concurrent downloads (1–16)", window, cx);
        let global_dl_limit = input("0", "KB/s (0 = unlimited)", window, cx);
        let per_dl_limit = input("0", "KB/s (0 = unlimited)", window, cx);
        let torrent_ul_limit = input("0", "KB/s (0 = unlimited)", window, cx);
        let torrent_seed_minutes = input("60", "Minutes (0 = unlimited)", window, cx);
        let torrent_seed_ratio = input("1.0", "Ratio (0 = unlimited)", window, cx);
        let torrent_peer_limit = input("128", "Peers (10–500)", window, cx);
        let tracker_urls = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder(
                "One URL per line (tracker subscription URLs or direct trackers)\nhttps://raw.githubusercontent.com/adysec/tracker/main/trackers_best.txt",
            )
        });
        let limit = input("1024", "KB per second", window, cx);
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.pump(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let theme_pref = t::load_theme_pref();
        let dark = theme_pref.is_dark(window.appearance());
        t::apply(dark, cx);
        let _appearance_sub = cx.observe_window_appearance(window, |this, window, cx| {
            this.sync_theme(window, cx);
        });
        Self {
            focus,
            row_focus: Default::default(),
            bridge: Bridge::start(),
            theme_pref,
            dark,
            _appearance_sub,
            list: ListAnswer::default(),
            selected: None,
            filter: "All downloads".into(),
            search,
            source,
            folder,
            limit,
            panel: None,
            detail: Value::Null,
            tab: 0,
            details: true,
            notice: None,
            ready: false,
            torrent_token: None,
            torrent: Value::Null,
            chosen: HashSet::new(),
            subfolder: true,
            connections,
            settings_tab: 0,
            max_concurrent,
            global_dl_limit,
            per_dl_limit,
            torrent_ul_limit,
            torrent_seed: false,
            torrent_seed_minutes,
            torrent_seed_ratio,
            torrent_peer_limit,
            torrent_dht: true,
            torrent_utp: true,
            tracker_urls,
            extra_trackers_count: 0,
            trackers_updated: None,
            trackers_syncing: false,
            trackers_error: None,
            browser_events: Default::default(),
            _poll: poll,
        }
    }

    fn sync_theme(&mut self, window: &Window, cx: &mut Context<Self>) {
        let dark = self.theme_pref.is_dark(window.appearance());
        if self.dark != dark {
            self.dark = dark;
            t::apply(dark, cx);
            cx.notify();
        }
    }

    fn set_theme_pref(&mut self, pref: t::ThemePref, window: &Window, cx: &mut Context<Self>) {
        self.theme_pref = pref;
        t::save_theme_pref(pref);
        let dark = pref.is_dark(window.appearance());
        if self.dark != dark {
            self.dark = dark;
            t::apply(dark, cx);
        }
        cx.notify();
    }

    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        while let Ok(m) = self.bridge.rx.try_recv() {
            changed = true;
            if let Some(error) = m.value["error"].as_str() {
                self.notice = Some(error.into());
                if matches!(m.tag.as_str(), "inspect" | "torrent") {
                    if let Some(token) = self.torrent_token.take() {
                        self.bridge
                            .send("cancel", json!({"op":"torrent_cancel","token":token}));
                    }
                    self.torrent = Value::Null;
                }
                continue;
            }
            match m.tag.as_str() {
                "list" => {
                    if let Ok(list) = serde_json::from_value::<ListAnswer>(m.value) {
                        for d in &list.downloads {
                            self.row_focus
                                .entry(d.id)
                                .or_insert_with(|| cx.focus_handle().tab_stop(true));
                        }
                        self.row_focus
                            .retain(|id, _| list.downloads.iter().any(|d| d.id == *id));
                        self.list = list;
                        self.ready = true;
                    }
                }
                "events" => {
                    if let Some(events) = m.value["events"].as_array() {
                        self.browser_events.extend(events.iter().cloned());
                    }
                }
                "settings" => {
                    if let Some(n) = m.value["connections"].as_u64() {
                        self.connections
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(folder) = m.value["folder"].as_str() {
                        self.folder
                            .update(cx, |v, cx| v.set_value(folder.to_owned(), window, cx));
                    }
                    if let Some(n) = m.value["max_concurrent"].as_u64() {
                        self.max_concurrent
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(n) = m.value["global_download_limit_kb"].as_u64() {
                        self.global_dl_limit
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(n) = m.value["per_download_limit_kb"].as_u64() {
                        self.per_dl_limit
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(n) = m.value["torrent_upload_limit_kb"].as_u64() {
                        self.torrent_ul_limit
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(b) = m.value["torrent_seed"].as_bool() {
                        self.torrent_seed = b;
                    }
                    if let Some(n) = m.value["torrent_seed_minutes"].as_u64() {
                        self.torrent_seed_minutes
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(r) = m.value["torrent_seed_ratio"].as_f64() {
                        self.torrent_seed_ratio
                            .update(cx, |v, cx| v.set_value(format!("{r}"), window, cx));
                    }
                    if let Some(n) = m.value["torrent_peer_limit"].as_u64() {
                        self.torrent_peer_limit
                            .update(cx, |v, cx| v.set_value(n.to_string(), window, cx));
                    }
                    if let Some(b) = m.value["torrent_dht"].as_bool() {
                        self.torrent_dht = b;
                    }
                    if let Some(b) = m.value["torrent_utp"].as_bool() {
                        self.torrent_utp = b;
                    }
                    if let Some(urls) = m.value["tracker_urls"].as_array() {
                        let text = urls
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.tracker_urls
                            .update(cx, |v, cx| v.set_value(text, window, cx));
                    }
                    self.extra_trackers_count =
                        m.value["extra_trackers_count"].as_u64().unwrap_or(0) as usize;
                    self.trackers_updated = m.value["trackers_updated"].as_str().map(str::to_owned);
                    self.trackers_syncing = m.value["trackers_syncing"].as_bool().unwrap_or(false);
                    self.trackers_error = m.value["trackers_error"].as_str().map(str::to_owned);
                }
                "settings-status" => {
                    self.extra_trackers_count =
                        m.value["extra_trackers_count"].as_u64().unwrap_or(0) as usize;
                    self.trackers_updated = m.value["trackers_updated"].as_str().map(str::to_owned);
                    self.trackers_syncing = m.value["trackers_syncing"].as_bool().unwrap_or(false);
                    self.trackers_error = m.value["trackers_error"].as_str().map(str::to_owned);
                }
                "trackers-sync" => {
                    self.trackers_syncing = m.value["trackers_syncing"].as_bool().unwrap_or(false);
                    self.extra_trackers_count =
                        m.value["extra_trackers_count"].as_u64().unwrap_or(0) as usize;
                    self.trackers_error = m.value["trackers_error"].as_str().map(str::to_owned);
                    self.notice = None;
                }
                "browser-register" => {
                    self.notice = Some(
                        "Browser host registered. Load the matching NDM extension in your browser."
                            .into(),
                    );
                }
                "settings-saved" => {
                    let syncing = m.value["trackers_syncing"].as_bool().unwrap_or(false);
                    self.trackers_syncing = syncing;
                    self.extra_trackers_count =
                        m.value["extra_trackers_count"].as_u64().unwrap_or(0) as usize;
                    self.close_panel();
                    self.notice = Some(if syncing {
                        "Settings saved · Syncing trackers…".into()
                    } else {
                        "Settings saved".into()
                    });
                }
                "detail" => self.detail = m.value,
                "inspect" => self.torrent_token = m.value["token"].as_str().map(str::to_owned),
                "torrent" => {
                    if m.value["pending"] == true {
                        self.torrent = m.value;
                    } else {
                        if self.torrent["files"].is_null() {
                            self.chosen = m.value["files"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|f| f["index"].as_u64().map(|n| n as usize))
                                .collect();
                        }
                        self.torrent = m.value;
                    }
                }
                "added" => {
                    self.close_panel();
                    self.notice = Some("Download added".into());
                }
                "action" => {
                    if let Some(skipped) = m.value["skipped"].as_array().and_then(|a| a.first()) {
                        self.notice =
                            Some(skipped["why"].as_str().unwrap_or("Action skipped").into());
                    }
                }
                _ => {}
            }
        }
        if self.panel.is_none()
            && let Some(event) = self.browser_events.pop_front()
        {
            cx.activate(true);
            match event["kind"].as_str() {
                Some("torrent") => {
                    self.open_add(true, window, cx);
                    if let Some(source) = event["source"].as_str() {
                        self.source
                            .update(cx, |v, cx| v.set_value(source.to_owned(), window, cx));
                        self.inspect(cx);
                    }
                }
                Some("settings") => {
                    self.panel = Some("settings");
                    self.notice = None;
                }
                Some("download") => {
                    self.filter = "All downloads".into();
                    self.selected = event["id"].as_u64();
                    self.notice = Some("Download received from browser".into());
                }
                _ => {}
            }
            changed = true;
        }
        if self.panel == Some("progress")
            && let Some(id) = self.selected
        {
            self.bridge.send("detail", json!({"op":"detail","id":id}));
        }
        if self.panel == Some("torrent")
            && self.torrent["files"].is_null()
            && let Some(token) = &self.torrent_token
        {
            self.bridge
                .send("torrent", json!({"op":"torrent_poll","token":token}));
        }
        if self.panel == Some("settings") && self.trackers_syncing {
            self.bridge
                .send("settings-status", json!({"op":"settings"}));
        }
        if changed
            || self.panel == Some("progress")
            || self.panel == Some("torrent")
            || (self.panel == Some("settings") && self.trackers_syncing)
        {
            cx.notify();
        }
    }

    fn selected(&self) -> Option<&DownloadInfo> {
        self.list
            .downloads
            .iter()
            .find(|d| Some(d.id) == self.selected)
    }

    fn active_count(&self) -> usize {
        self.list
            .downloads
            .iter()
            .filter(|d| matches!(d.status.as_str(), "Downloading" | "Seeding"))
            .count()
    }

    fn action(&mut self, action: &str, all: bool) {
        let ids: Vec<_> = if all {
            self.list
                .downloads
                .iter()
                .filter(|d| {
                    if action == "pause" {
                        matches!(d.status.as_str(), "Downloading" | "Seeding")
                    } else {
                        d.status != "Complete"
                    }
                })
                .map(|d| d.id)
                .collect()
        } else {
            self.selected.into_iter().collect()
        };
        self.bridge
            .send("action", json!({"op":"action","action":action,"ids":ids}));
    }

    fn close_panel(&mut self) {
        if let Some(token) = self.torrent_token.take() {
            self.bridge
                .send("cancel", json!({"op":"torrent_cancel","token":token}));
        }
        self.panel = None;
        self.torrent = Value::Null;
        self.chosen.clear();
    }

    fn open_add(&mut self, torrent: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_panel();
        self.source = input(
            "",
            if torrent {
                "Magnet link or .torrent file path"
            } else {
                "https://example.com/file.zip"
            },
            window,
            cx,
        );
        self.panel = Some(if torrent { "torrent" } else { "add" });
        self.notice = None;
        self.source.update(cx, |v, cx| v.focus(window, cx));
        cx.notify();
    }

    fn add(&mut self, later: bool, cx: &mut Context<Self>) {
        let source = self.source.read(cx).value().trim().to_owned();
        let dir = self.folder.read(cx).value().trim().to_owned();
        if dir.is_empty() {
            self.notice = Some("Choose a destination folder".into());
            cx.notify();
            return;
        }
        if self.panel == Some("torrent") {
            if self.chosen.is_empty() {
                self.notice = Some("Select at least one file".into());
                cx.notify();
                return;
            }
            self.bridge.send(
                "added",
                json!({"op":"torrent_add","token":self.torrent_token,"dir":dir,"files":self.chosen,"subfolder":self.subfolder,"later":later}),
            );
        } else {
            if url::Url::parse(&source).is_ok_and(|u| u.scheme() == "magnet") {
                self.panel = Some("torrent");
                self.inspect(cx);
                return;
            }
            if !url::Url::parse(&source)
                .is_ok_and(|u| matches!(u.scheme(), "http" | "https" | "ftp" | "ftps"))
            {
                self.notice = Some("Enter a valid HTTP, HTTPS, FTP or FTPS address".into());
                cx.notify();
                return;
            }
            self.bridge.send(
                "added",
                json!({"op":"add","command":{"url":source,"dir":dir,"later":later,"silent":true}}),
            );
        }
    }

    fn inspect(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx).value().trim().to_owned();
        if source.is_empty() {
            self.notice = Some("Choose a .torrent file or paste a magnet link".into());
        } else {
            if let Some(token) = self.torrent_token.take() {
                self.bridge
                    .send("cancel", json!({"op":"torrent_cancel","token":token}));
            }
            self.torrent = Value::Null;
            self.chosen.clear();
            self.notice = None;
            self.bridge
                .send("inspect", json!({"op":"torrent_inspect","source":source}));
        }
        cx.notify();
    }

    fn nav_item(
        &self,
        label: &'static str,
        ico: IconName,
        ico_color: Rgba,
        count: Option<usize>,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.filter == label;
        let mut item = row()
            .id(label)
            .role(Role::Button)
            .aria_selected(selected)
            .cursor_pointer()
            .h(px(32.))
            .w_full()
            .px(px(10.))
            .gap(px(9.))
            .rounded(px(8.))
            .border_1()
            .border_color(if selected {
                p.accent_border
            } else {
                rgba(0x00000000)
            })
            .bg(if selected {
                p.accent_soft
            } else {
                rgba(0x00000000)
            })
            .hover(|s| s.bg(if selected { p.accent_soft } else { p.nav_hover }))
            .on_click(cx.listener(move |s, _, _, cx| {
                s.filter = label.into();
                cx.notify();
            }))
            .child(
                icon(ico)
                    .size(px(15.))
                    .text_color(if selected { ico_color } else { p.muted }),
            )
            .child(
                text(label)
                    .flex_1()
                    .text_size(px(12.5))
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if selected { p.primary } else { p.ink_secondary })
                    .text_ellipsis(),
            );
        if let Some(n) = count {
            item = item.child(
                text(n.to_string())
                    .text_size(px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if selected { p.primary } else { p.muted })
                    .px(px(6.))
                    .py(px(1.))
                    .rounded_full()
                    .bg(p.surface)
                    .border_1()
                    .border_color(if selected {
                        p.accent_border
                    } else {
                        p.border_subtle
                    }),
            );
        }
        item
    }

    fn segmented_tabs<const N: usize>(
        &self,
        group_id: &'static str,
        labels: [&'static str; N],
        active: usize,
        is_settings: bool,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut bar = row()
            .p(px(3.))
            .gap(px(2.))
            .rounded(px(9.))
            .bg(p.soft)
            .border_1()
            .border_color(p.border_subtle);
        for (index, label) in labels.into_iter().enumerate() {
            let selected = active == index;
            bar = bar.child(
                row()
                    .id((group_id, index))
                    .role(Role::Tab)
                    .aria_selected(selected)
                    .cursor_pointer()
                    .h(px(28.))
                    .px(px(12.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if selected {
                        p.border_subtle
                    } else {
                        rgba(0x00000000)
                    })
                    .bg(if selected {
                        p.surface_elevated
                    } else {
                        rgba(0x00000000)
                    })
                    .when(selected, |d| d.shadow_xs())
                    .text_size(px(12.))
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if selected { p.ink } else { p.muted })
                    .hover(|s| if selected { s } else { s.text_color(p.ink) })
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if is_settings {
                            s.settings_tab = index;
                        } else {
                            s.tab = index;
                        }
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        bar
    }

    fn heading(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let active = self.active_count();
        row()
            .h(px(48.))
            .pl(px(82.))
            .pr(px(16.))
            .justify_between()
            .border_b_1()
            .border_color(p.border)
            .bg(p.chrome)
            .child(
                row()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(26.))
                            .rounded(px(7.))
                            .shadow_xs()
                            .bg(linear_gradient(
                                135.,
                                linear_color_stop(p.primary_bright, 0.),
                                linear_color_stop(p.primary, 1.),
                            ))
                            .child(
                                icon(IconName::Download)
                                    .size(px(15.))
                                    .text_color(rgb(0xffffff)),
                            ),
                    )
                    .child(
                        text("Next Download Manager")
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(
                        text(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.muted)
                            .px(px(7.))
                            .py(px(1.5))
                            .rounded_full()
                            .bg(p.soft)
                            .border_1()
                            .border_color(p.border),
                    ),
            )
            .child(
                row()
                    .gap(px(6.))
                    .when(active > 0, |r| {
                        r.child(
                            row()
                                .h(px(24.))
                                .px(px(10.))
                                .gap(px(6.))
                                .rounded_full()
                                .bg(p.success_soft)
                                .border_1()
                                .border_color(p.success_border)
                                .child(div().size(px(6.)).rounded_full().bg(p.success))
                                .child(
                                    text(format!("{}/s", bytes(self.list.speed)))
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.success),
                                ),
                        )
                    })
                    .child({
                        let (theme_ico, theme_label, next_label) = match self.theme_pref {
                            t::ThemePref::System => (IconName::Monitor, "Auto", "Light"),
                            t::ThemePref::Light => (IconName::Sun, "Light", "Dark"),
                            t::ThemePref::Dark => (IconName::Moon, "Dark", "System"),
                        };
                        let tip = format!(
                            "Theme: {} — click for {next_label}",
                            self.theme_pref.label()
                        );
                        btn("toggle-theme", theme_label)
                            .icon(theme_ico)
                            .ghost()
                            .h(px(30.))
                            .tooltip(tip)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.set_theme_pref(s.theme_pref.next(), w, cx);
                            }))
                    })
                    .child(
                        btn("settings", "Settings")
                            .icon(IconName::Settings)
                            .ghost()
                            .h(px(30.))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.notice = None;
                                s.settings_tab = 0;
                                s.panel = Some("settings");
                                s.bridge.send("settings", json!({"op":"settings"}));
                                cx.notify();
                            })),
                    ),
            )
    }

    fn toolbar(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let can_resume = self
            .selected()
            .is_some_and(|d| !matches!(d.status.as_str(), "Complete" | "Downloading" | "Seeding"));
        let can_pause = self
            .selected()
            .is_some_and(|d| matches!(d.status.as_str(), "Downloading" | "Seeding"));
        let any_active = self.active_count() > 0;
        let has_selected = self.selected.is_some();

        row()
            .h(px(56.))
            .px(px(18.))
            .gap(px(6.))
            .bg(p.toolbar)
            .border_b_1()
            .border_color(p.border)
            .child(
                btn("add-url", "Add URL")
                    .primary()
                    .icon(IconName::Plus)
                    .tooltip("Add download URL (⌘N)")
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, w, cx| s.open_add(false, w, cx))),
            )
            .child(
                btn("add-torrent", "Add torrent")
                    .icon(IconName::Magnet)
                    .tooltip("Add .torrent file or magnet link")
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, w, cx| s.open_add(true, w, cx))),
            )
            .child(div().h(px(22.)).w(px(1.)).bg(p.border).mx(px(6.)))
            .child(
                btn("resume", "Resume")
                    .ghost()
                    .icon(IconName::Play)
                    .disabled(!can_resume)
                    .on_click(cx.listener(|s, _, _, _| s.action("start", false))),
            )
            .child(
                btn("pause", "Pause")
                    .ghost()
                    .icon(IconName::Pause)
                    .disabled(!can_pause)
                    .on_click(cx.listener(|s, _, _, _| s.action("pause", false))),
            )
            .child(
                btn("pause-all", "Pause all")
                    .ghost()
                    .icon(IconName::Pause)
                    .disabled(!any_active)
                    .on_click(cx.listener(|s, _, _, _| s.action("pause", true))),
            )
            .child(
                btn("remove", "Remove")
                    .ghost()
                    .icon(IconName::Trash)
                    .when(has_selected, |b| b.text_color(p.danger))
                    .disabled(!has_selected)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.notice = None;
                        s.panel = Some("remove");
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                div().w(px(236.)).flex_shrink_0().child(
                    Input::new(&self.search)
                        .prefix(icon(IconName::Search).size(px(14.)).text_color(p.muted))
                        .cleanable(true),
                ),
            )
    }

    fn sidebar(&self, p: &t::Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = col().min_h(px(480.)).h_full().p(px(12.)).gap(px(2.)).child(
            text("LIBRARY")
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.muted)
                .px(px(10.))
                .pt(px(4.))
                .pb(px(6.)),
        );

        for (label, ico) in [
            ("All downloads", IconName::Layers),
            ("Unfinished", IconName::Clock),
            ("Finished", IconName::CircleCheck),
        ] {
            let count = self
                .list
                .downloads
                .iter()
                .filter(|d| match label {
                    "Unfinished" => d.status != "Complete",
                    "Finished" => d.status == "Complete",
                    _ => true,
                })
                .count();
            nav = nav.child(self.nav_item(label, ico, p.primary, Some(count), p, cx));
        }

        nav = nav.child(
            text("CATEGORIES")
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.muted)
                .px(px(10.))
                .pt(px(14.))
                .pb(px(6.)),
        );

        for (label, ico) in [
            ("Compressed", IconName::Archive),
            ("Documents", IconName::FileText),
            ("Music", IconName::Music),
            ("Programs", IconName::AppWindow),
            ("Video", IconName::Video),
            ("Torrents", IconName::Magnet),
        ] {
            let count = self
                .list
                .downloads
                .iter()
                .filter(|d| d.category == label)
                .count();
            let (tint, _) = t::category_tint(label, self.dark);
            nav = nav.child(self.nav_item(label, ico, tint, (count > 0).then_some(count), p, cx));
        }

        let active = self.active_count();
        let queue_card = col()
            .p(px(10.))
            .gap(px(8.))
            .rounded(px(10.))
            .bg(p.glass_card)
            .border_1()
            .border_color(p.border)
            .child(
                row()
                    .justify_between()
                    .child(
                        text("Main queue")
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(
                        text(if active > 0 {
                            format!("{active} active")
                        } else {
                            "Idle".into()
                        })
                        .text_size(px(10.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if active > 0 { p.success } else { p.muted })
                        .px(px(6.))
                        .py(px(1.))
                        .rounded_full()
                        .bg(if active > 0 {
                            p.success_soft
                        } else {
                            p.soft
                        }),
                    ),
            )
            .child(
                row()
                    .gap(px(6.))
                    .child(
                        btn("queue-start", "Start")
                            .icon(IconName::Play)
                            .ghost()
                            .h(px(28.))
                            .flex_1()
                            .tooltip("Start main queue")
                            .on_click(cx.listener(|s, _, _, _| {
                                s.bridge.send("queue", json!({"op":"queue","start":true}))
                            })),
                    )
                    .child(
                        btn("queue-stop", "Stop")
                            .icon(IconName::Pause)
                            .ghost()
                            .h(px(28.))
                            .flex_1()
                            .tooltip("Stop main queue")
                            .on_click(cx.listener(|s, _, _, _| {
                                s.bridge.send("queue", json!({"op":"queue","start":false}))
                            })),
                    ),
            );

        col()
            .id("sidebar-scroll")
            .w(px(216.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.border)
            .child(nav.child(div().flex_1().min_h(px(12.))).child(queue_card))
    }

    fn table(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let q = self.search.read(cx).value().to_lowercase();
        let rows: Vec<_> = self
            .list
            .downloads
            .iter()
            .rev()
            .filter(|d| {
                let filter = match self.filter.as_str() {
                    "All downloads" => true,
                    "Unfinished" => d.status != "Complete",
                    "Finished" => d.status == "Complete",
                    c => d.category == c,
                };
                filter
                    && (q.is_empty()
                        || d.file.to_lowercase().contains(&q)
                        || d.url.to_lowercase().contains(&q))
            })
            .collect();
        let visible_count = rows.len();

        let mut body = col()
            .id("downloads-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        if rows.is_empty() {
            let empty_title = if !self.ready {
                "Starting download service…"
            } else if !q.is_empty() {
                "No matching downloads"
            } else if self.filter != "All downloads" {
                "Nothing in this view"
            } else {
                "Your downloads, all in one place"
            };
            let empty_subtitle = if !self.ready {
                "Connecting to the background transfer engine."
            } else if !q.is_empty() {
                "Try a different filename, host, or clear the search filter."
            } else if self.filter != "All downloads" {
                "Downloads matching this filter will appear here automatically."
            } else {
                "Add a direct link or a torrent magnet to start transferring at full speed."
            };

            let mut empty = col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .px(px(24.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(68.))
                        .rounded(px(20.))
                        .bg(p.accent_soft)
                        .border_1()
                        .border_color(p.accent_border)
                        .child(icon(IconName::Download).size(px(28.)).text_color(p.primary)),
                )
                .child(
                    col()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            text(empty_title)
                                .text_size(px(15.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink),
                        )
                        .child(muted(empty_subtitle, p).text_size(px(12.5))),
                );

            if !q.is_empty() {
                empty = empty.child(btn("clear-search", "Clear search").on_click(cx.listener(
                    |s, _, w, cx| {
                        s.search
                            .update(cx, |v, cx| v.set_value(String::new(), w, cx));
                        cx.notify();
                    },
                )));
            } else if self.filter != "All downloads" {
                empty = empty.child(btn("show-all-downloads", "Show all downloads").on_click(
                    cx.listener(|s, _, _, cx| {
                        s.filter = "All downloads".into();
                        cx.notify();
                    }),
                ));
            } else if self.ready {
                empty = empty.child(
                    row()
                        .gap(px(8.))
                        .pt(px(4.))
                        .child(
                            btn("empty-add-url", "Add URL")
                                .primary()
                                .icon(IconName::Plus)
                                .on_click(cx.listener(|s, _, w, cx| s.open_add(false, w, cx))),
                        )
                        .child(
                            btn("empty-add-torrent", "Add torrent")
                                .icon(IconName::Magnet)
                                .on_click(cx.listener(|s, _, w, cx| s.open_add(true, w, cx))),
                        ),
                );
            }
            body = body.child(empty);
        }

        for d in rows {
            let id = d.id;
            let selected = self.selected == Some(id);
            let downloading = d.status == "Downloading";
            let seeding = d.status == "Seeding";
            let complete = d.status == "Complete";
            let is_err = is_error_status(&d.status);
            let has_size = d.size.is_some_and(|n| n > 0);
            let frac = fraction(d);
            let eta = eta_remaining(d);
            let (ico_color, ico_bg) = t::category_tint(&d.category, self.dark);
            let host = url::Url::parse(&d.url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_else(|| "BitTorrent".into());
            let subline = if downloading {
                match d.size {
                    Some(total) => format!("{host}  ·  {} / {}", bytes(d.downloaded), bytes(total)),
                    None => format!("{host}  ·  {}", bytes(d.downloaded)),
                }
            } else {
                format!("{host}  ·  {}", d.category)
            };

            body = body.child(
                row()
                    .id(("download", id))
                    .role(Role::Button)
                    .track_focus(&self.row_focus[&id])
                    .tab_stop(true)
                    .aria_selected(selected)
                    .focus_visible(|style| {
                        style.border_2().border_color(p.primary).bg(p.accent_soft)
                    })
                    .on_key_down(cx.listener(move |s, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "space" | " " => {
                                s.selected = Some(id);
                                cx.stop_propagation();
                                cx.notify();
                            }
                            "enter" => {
                                s.selected = Some(id);
                                s.notice = None;
                                s.panel = Some("progress");
                                s.detail = Value::Null;
                                s.tab = 0;
                                cx.stop_propagation();
                                cx.notify();
                            }
                            _ => {}
                        }
                    }))
                    .aria_label(format!("{} — {}", d.file, d.status))
                    .h(px(68.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(p.border_subtle)
                    .bg(if selected {
                        p.selected
                    } else {
                        rgba(0x00000000)
                    })
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(if selected {
                            p.selected_hover
                        } else {
                            p.row_hover
                        })
                    })
                    .on_click(cx.listener(move |s, event: &ClickEvent, _, cx| {
                        s.selected = Some(id);
                        if event.click_count() == 2 {
                            s.notice = None;
                            s.panel = Some("progress");
                            s.detail = Value::Null;
                            s.tab = 0;
                        }
                        cx.notify();
                    }))
                    .child(div().w(px(3.)).h_full().flex_shrink_0().bg(if selected {
                        p.primary
                    } else {
                        rgba(0x00000000)
                    }))
                    .child(
                        div()
                            .w(px(55.))
                            .flex_shrink_0()
                            .pl(px(11.))
                            .child(icon_tile(
                                category_icon(&d.category),
                                ico_color,
                                ico_bg,
                                36.,
                                17.,
                            )),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .pr(px(14.))
                            .child(
                                text(d.file.clone())
                                    .text_ellipsis()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.ink),
                            )
                            .child(helper_text(subline, p).text_ellipsis()),
                    )
                    .child(
                        cell(display_size(d), 98.)
                            .text_size(px(12.))
                            .text_color(p.ink_secondary),
                    )
                    .child(
                        col()
                            .w(px(172.))
                            .flex_shrink_0()
                            .px(px(12.))
                            .gap(px(7.))
                            .child(status_badge(
                                &d.status,
                                downloading,
                                has_size,
                                frac * 100.,
                                p,
                            ))
                            .child(progress_bar(
                                frac,
                                6.,
                                downloading,
                                complete || seeding,
                                is_err,
                                downloading && !has_size,
                                p,
                            )),
                    )
                    .child(
                        cell(
                            if seeding {
                                format!("↑ {}/s", bytes(d.speed))
                            } else if downloading {
                                format!("↓ {}/s", bytes(d.speed))
                            } else if complete {
                                d.avg_speed
                                    .filter(|s| *s > 0)
                                    .map(|s| format!("≈ {}/s", bytes(s)))
                                    .unwrap_or_else(|| "—".into())
                            } else {
                                "—".into()
                            },
                            114.,
                        )
                        .text_size(px(12.))
                        .font_weight(if downloading || seeding {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if seeding {
                            p.success
                        } else if downloading {
                            p.primary
                        } else if complete && d.avg_speed.is_some_and(|s| s > 0) {
                            p.ink_secondary
                        } else {
                            p.muted
                        }),
                    )
                    .child(
                        cell(
                            if let Some(secs) = eta {
                                duration(secs)
                            } else if complete {
                                d.duration_secs
                                    .map(|s| format!("Took {}", duration(s)))
                                    .unwrap_or_else(|| "—".into())
                            } else {
                                "—".into()
                            },
                            88.,
                        )
                        .text_size(px(12.))
                        .text_color(
                            if eta.is_some() || (complete && d.duration_secs.is_some()) {
                                p.ink_secondary
                            } else {
                                p.muted
                            },
                        ),
                    ),
            );
        }

        let selected_path = self.selected().map(|d| d.path.clone());
        let subtitle = if !q.is_empty() {
            format!("Filtered by \"{}\"", self.search.read(cx).value().trim())
        } else if visible_count > 0 {
            "Double-click any item to inspect connections & speed limits".into()
        } else {
            "Ready for new transfers".into()
        };

        col()
            .flex_1()
            .min_w_0()
            .bg(p.surface)
            .child(
                row()
                    .h(px(62.))
                    .px(px(22.))
                    .justify_between()
                    .child(
                        col()
                            .gap(px(2.))
                            .child(
                                row()
                                    .gap(px(8.))
                                    .child(
                                        text(self.filter.clone())
                                            .text_size(px(17.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.ink),
                                    )
                                    .child(
                                        text(format!(
                                            "{} item{}",
                                            visible_count,
                                            if visible_count == 1 { "" } else { "s" }
                                        ))
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(p.muted)
                                        .px(px(8.))
                                        .py(px(1.5))
                                        .rounded_full()
                                        .bg(p.soft)
                                        .border_1()
                                        .border_color(p.border_subtle),
                                    ),
                            )
                            .child(helper_text(subtitle, p)),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .when_some(selected_path, |r, sel_path| {
                                r.child(
                                    btn("open-selected-folder", "Open folder")
                                        .icon(IconName::FolderOpen)
                                        .ghost()
                                        .on_click(move |_, _, _| {
                                            let path = std::path::PathBuf::from(&sel_path);
                                            let folder = path.parent().unwrap_or(&path);
                                            let _ = open::that_detached(folder);
                                        }),
                                )
                            })
                            .child(
                                btn("show-details", "Show details")
                                    .icon(IconName::SlidersHorizontal)
                                    .ghost()
                                    .tooltip("Show download details (⌘I)")
                                    .disabled(self.selected.is_none())
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.panel = Some("progress");
                                        s.notice = None;
                                        s.detail = Value::Null;
                                        s.tab = 0;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                row()
                    .h(px(34.))
                    .bg(p.table_header)
                    .border_y_1()
                    .border_color(p.border)
                    .text_color(p.muted)
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().w(px(58.)).flex_shrink_0())
                    .child(text("FILE NAME").flex_1().pr(px(12.)))
                    .child(cell("SIZE", 98.))
                    .child(cell("STATUS", 172.))
                    .child(cell("SPEED", 114.))
                    .child(cell("TIME LEFT", 88.)),
            )
            .child(body)
    }

    fn footer(&self, p: &t::Palette) -> Div {
        let active = self.active_count();
        row()
            .h(px(34.))
            .px(px(18.))
            .gap(px(12.))
            .border_t_1()
            .border_color(p.border)
            .bg(p.chrome)
            .text_size(px(11.5))
            .text_color(p.muted)
            .child(
                row()
                    .gap(px(7.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(12.))
                            .rounded_full()
                            .bg(if self.ready {
                                p.success_soft
                            } else {
                                p.warning_soft
                            })
                            .child(div().size(px(6.)).rounded_full().bg(if self.ready {
                                p.success
                            } else {
                                p.warning
                            })),
                    )
                    .child(text(if self.ready {
                        "Download service connected"
                    } else {
                        "Connecting to service…"
                    })),
            )
            .child(div().flex_1())
            .child(
                row()
                    .gap(px(5.))
                    .child(icon(IconName::Activity).size(px(12.5)).text_color(p.muted))
                    .child(format!("{active} active")),
            )
            .child(div().h(px(12.)).w(px(1.)).bg(p.border))
            .child(
                row()
                    .gap(px(4.))
                    .child(icon(IconName::ArrowDown).size(px(12.5)).text_color(
                        if self.list.speed > 0 {
                            p.primary
                        } else {
                            p.muted
                        },
                    ))
                    .child(
                        text(format!("{}/s", bytes(self.list.speed)))
                            .font_weight(if self.list.speed > 0 {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if self.list.speed > 0 { p.ink } else { p.muted }),
                    ),
            )
            .when_some(self.list.free, |r, free| {
                r.child(div().h(px(12.)).w(px(1.)).bg(p.border)).child(
                    row()
                        .gap(px(5.))
                        .child(icon(IconName::HardDrive).size(px(12.5)).text_color(p.muted))
                        .child(format!("{} free", bytes(free))),
                )
            })
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t::palette(self.dark);
        let mut root = col()
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &ClosePanel, w, cx| {
                s.close_panel();
                w.focus(&s.focus, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &NewDownload, w, cx| s.open_add(false, w, cx)))
            .on_action(cx.listener(|s, _: &ShowDetails, _, cx| {
                s.notice = None;
                if s.selected.is_some() {
                    s.panel = Some("progress");
                    s.tab = 0;
                    cx.notify();
                }
            }))
            .size_full()
            .relative()
            .text_color(p.ink)
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .child(self.heading(&p, cx))
            .child(self.toolbar(&p, cx))
            .child(
                row()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(self.sidebar(&p, cx))
                    .child(self.table(&p, cx)),
            )
            .child(self.footer(&p));

        if let Some(notice) = &self.notice
            && self.panel.is_none()
        {
            root = root.child(
                row()
                    .absolute()
                    .bottom(px(46.))
                    .right(px(20.))
                    .max_w(px(460.))
                    .px(px(14.))
                    .py(px(10.))
                    .gap(px(10.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(p.notice_border)
                    .bg(p.notice_bg)
                    .shadow_md()
                    .child(icon_tile(
                        IconName::Info,
                        p.primary,
                        p.accent_soft,
                        24.,
                        13.,
                    ))
                    .child(
                        text(notice.clone())
                            .flex_1()
                            .text_size(px(12.5))
                            .text_color(p.notice_text),
                    )
                    .child(
                        Button::new("dismiss")
                            .icon(IconName::X)
                            .ghost()
                            .h(px(26.))
                            .rounded(px(6.))
                            .accessibility_label("Dismiss")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.notice = None;
                                cx.notify();
                            })),
                    ),
            );
        }

        if let Some(panel) = self.panel {
            root = root.child(self.overlay(panel, f32::from(window.bounds().size.height), &p, cx));
        }
        root
    }
}

impl Shell {
    fn overlay(
        &self,
        panel: &str,
        height: f32,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title, header_ico, header_color, header_bg) = match panel {
            "add" => ("Add download", IconName::Plus, p.primary, p.accent_soft),
            "torrent" => {
                let (c, b) = t::category_tint("Torrents", self.dark);
                ("Add torrent", IconName::Magnet, c, b)
            }
            "progress" => (
                "Download details",
                IconName::Activity,
                p.primary,
                p.accent_soft,
            ),
            "remove" => ("Remove download", IconName::Trash, p.danger, p.danger_soft),
            _ => ("Settings", IconName::Settings, p.primary, p.accent_soft),
        };
        let compact = height < 700.;
        let mut content = col()
            .gap(px(if compact { 10. } else { 14. }))
            .p(px(if compact { 16. } else { 20. }));
        let mut footer_bar = row()
            .h(px(56.))
            .flex_shrink_0()
            .px(px(20.))
            .bg(p.modal_chrome)
            .rounded_b(px(13.))
            .border_t_1()
            .border_color(p.border)
            .items_center()
            .justify_end()
            .gap(px(8.));

        match panel {
            "add" | "torrent" => {
                content = content.child(helper_text(
                    if panel == "torrent" {
                        "Choose a .torrent file or paste a magnet link to inspect its contents."
                    } else {
                        "Enter a direct file URL and choose where to save it."
                    },
                    p,
                ));
                content = content.child(
                    col()
                        .gap(px(6.))
                        .child(field_label(
                            if panel == "torrent" {
                                "Torrent source or magnet link"
                            } else {
                                "Download URL"
                            },
                            p,
                        ))
                        .child(Input::new(&self.source).cleanable(true)),
                );

                if panel == "torrent" {
                    content = content.child(
                        row()
                            .gap(px(8.))
                            .child(
                                btn("browse-torrent", "Choose .torrent file…")
                                    .icon(IconName::FolderOpen)
                                    .on_click(cx.listener(|_s, _, w, cx| {
                                        let weak = cx.entity().downgrade();
                                        let task = rfd::AsyncFileDialog::new()
                                            .add_filter("Torrent", &["torrent"])
                                            .pick_file();
                                        cx.spawn_in(w, async move |_, cx| {
                                            if let Some(file) = task.await {
                                                let path =
                                                    file.path().to_string_lossy().into_owned();
                                                let _ = weak.update_in(cx, |s, w, cx| {
                                                    s.source.update(cx, |v, cx| {
                                                        v.set_value(path, w, cx)
                                                    });
                                                    s.inspect(cx);
                                                });
                                            }
                                        })
                                        .detach();
                                    })),
                            )
                            .child(
                                btn("read-torrent", "Read files")
                                    .icon(IconName::RefreshCw)
                                    .disabled(
                                        self.torrent_token.is_some()
                                            && self.torrent["files"].is_null(),
                                    )
                                    .on_click(cx.listener(|s, _, _, cx| s.inspect(cx))),
                            ),
                    );

                    let mut tree = col()
                        .h(px(if compact { 120. } else { 215. }))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(8.));

                    let mut chosen_bytes: u64 = 0;
                    if let Some(files) = self.torrent["files"].as_array() {
                        tree = tree.child(
                            row()
                                .px(px(12.))
                                .py(px(9.))
                                .flex_shrink_0()
                                .justify_between()
                                .bg(p.table_header)
                                .rounded_t(px(7.))
                                .border_b_1()
                                .border_color(p.border)
                                .child(
                                    text(
                                        self.torrent["name"]
                                            .as_str()
                                            .unwrap_or("Torrent")
                                            .to_owned(),
                                    )
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink)
                                    .text_ellipsis(),
                                )
                                .child(
                                    text(format!("{} files", files.len()))
                                        .text_size(px(11.))
                                        .text_color(p.muted),
                                ),
                        );
                        let mut file_rows = col()
                            .id("torrent-files")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p(px(4.));
                        for file in files {
                            let index = file["index"].as_u64().unwrap_or(0) as usize;
                            let path = file["path"].as_str().unwrap_or("").to_owned();
                            let fsize = file["size"].as_u64().unwrap_or(0);
                            let checked = self.chosen.contains(&index);
                            if checked {
                                chosen_bytes = chosen_bytes.saturating_add(fsize);
                            }
                            file_rows = file_rows.child(
                                row()
                                    .h(px(34.))
                                    .flex_shrink_0()
                                    .px(px(8.))
                                    .gap(px(8.))
                                    .rounded(px(6.))
                                    .border_b_1()
                                    .border_color(p.border_subtle)
                                    .hover(|s| s.bg(p.row_hover))
                                    .child(
                                        Checkbox::new(("torrent-file", index))
                                            .checked(checked)
                                            .accessibility_label(path.clone())
                                            .on_click(cx.listener(
                                                move |s, checked: &bool, _, cx| {
                                                    if *checked {
                                                        s.chosen.insert(index);
                                                    } else {
                                                        s.chosen.remove(&index);
                                                    }
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        text(path)
                                            .flex_1()
                                            .min_w_0()
                                            .text_ellipsis()
                                            .text_size(px(12.))
                                            .text_color(p.ink),
                                    )
                                    .child(muted(bytes(fsize), p).text_size(px(11.5))),
                            );
                        }
                        tree = tree.child(file_rows);
                    } else {
                        tree = tree.child(
                            col()
                                .id("torrent-files")
                                .flex_1()
                                .min_h_0()
                                .items_center()
                                .justify_center()
                                .gap(px(8.))
                                .child(icon_tile(IconName::File, p.muted, p.soft, 36., 18.))
                                .child(muted(
                                    if self.torrent_token.is_some() {
                                        "Reading torrent metadata…"
                                    } else {
                                        "The torrent's files will appear here."
                                    },
                                    p,
                                ))
                                .when_some(self.torrent["lookup"].as_object(), |c, _| {
                                    c.child(helper_text(
                                        format!(
                                            "{} trackers · {} DHT nodes · {} peers · {} connected",
                                            self.torrent["lookup"]["trackers"],
                                            self.torrent["lookup"]["dht_nodes"],
                                            self.torrent["lookup"]["peers"],
                                            self.torrent["lookup"]["connected"]
                                        ),
                                        p,
                                    ))
                                }),
                        );
                    }

                    content = content.child(tree).child(
                        row()
                            .justify_between()
                            .child(helper_text(
                                if chosen_bytes > 0 {
                                    format!(
                                        "{} files selected ({})",
                                        self.chosen.len(),
                                        bytes(chosen_bytes)
                                    )
                                } else {
                                    format!("{} files selected", self.chosen.len())
                                },
                                p,
                            ))
                            .child(
                                row()
                                    .gap(px(4.))
                                    .child(
                                        btn("select-all", "Select all")
                                            .ghost()
                                            .h(px(26.))
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.chosen = s.torrent["files"]
                                                    .as_array()
                                                    .into_iter()
                                                    .flatten()
                                                    .filter_map(|f| {
                                                        f["index"].as_u64().map(|n| n as usize)
                                                    })
                                                    .collect();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        btn("select-none", "Select none")
                                            .ghost()
                                            .h(px(26.))
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.chosen.clear();
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    );
                }

                content = content.child(
                    col().gap(px(6.)).child(field_label("Save to", p)).child(
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().child(Input::new(&self.folder)))
                            .child(
                                btn("browse-folder", "Browse…")
                                    .icon(IconName::FolderOpen)
                                    .on_click(cx.listener(|_, _, w, cx| {
                                        let weak = cx.entity().downgrade();
                                        let dialog = rfd::AsyncFileDialog::new().pick_folder();
                                        cx.spawn_in(w, async move |_, cx| {
                                            if let Some(folder) = dialog.await {
                                                let path =
                                                    folder.path().to_string_lossy().into_owned();
                                                let _ = weak.update_in(cx, |s, w, cx| {
                                                    s.folder.update(cx, |v, cx| {
                                                        v.set_value(path, w, cx)
                                                    });
                                                });
                                            }
                                        })
                                        .detach();
                                    })),
                            ),
                    ),
                );

                if panel == "torrent" {
                    content = content.child(
                        Checkbox::new("create-subfolder")
                            .label("Create a subfolder for this torrent")
                            .checked(self.subfolder)
                            .on_click(cx.listener(|s, v: &bool, _, cx| {
                                s.subfolder = *v;
                                cx.notify();
                            })),
                    );
                }

                footer_bar = footer_bar
                    .child(
                        btn("cancel-add", "Cancel").on_click(cx.listener(|s, _, _, cx| {
                            s.close_panel();
                            cx.notify();
                        })),
                    )
                    .child(
                        btn("download-later", "Download later")
                            .disabled(panel == "torrent" && self.chosen.is_empty())
                            .on_click(cx.listener(|s, _, _, cx| s.add(true, cx))),
                    )
                    .child(
                        btn("download-now", "Download now")
                            .primary()
                            .icon(IconName::Download)
                            .disabled(panel == "torrent" && self.chosen.is_empty())
                            .on_click(cx.listener(|s, _, _, cx| s.add(false, cx))),
                    );
            }

            "progress" => {
                if let Some(d) = self.selected() {
                    let d = d.clone();
                    let frac = fraction(&d);
                    let downloading = d.status == "Downloading";
                    let seeding = d.status == "Seeding";
                    let complete = d.status == "Complete";
                    let is_err = is_error_status(&d.status);
                    let has_size = d.size.is_some_and(|n| n > 0);
                    let (ico_color, ico_bg) = t::category_tint(&d.category, self.dark);

                    content = content
                        .child(
                            row()
                                .gap(px(12.))
                                .child(icon_tile(
                                    category_icon(&d.category),
                                    ico_color,
                                    ico_bg,
                                    38.,
                                    18.,
                                ))
                                .child(
                                    col()
                                        .flex_1()
                                        .min_w_0()
                                        .gap(px(3.))
                                        .child(
                                            text(d.file.clone())
                                                .text_size(px(15.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(p.ink)
                                                .text_ellipsis(),
                                        )
                                        .child(status_badge(
                                            &d.status,
                                            downloading,
                                            has_size,
                                            frac * 100.,
                                            p,
                                        )),
                                ),
                        )
                        .child(self.segmented_tabs(
                            "detail-tab",
                            ["Download status", "Speed limiter", "On completion"],
                            self.tab,
                            false,
                            p,
                            cx,
                        ));

                    if self.tab == 0 {
                        let resume = match self.detail["resumable"].as_bool() {
                            Some(true) => "Supported",
                            Some(false) => "Not supported",
                            None => "Not known yet",
                        };
                        let (time_label, time_value) = if complete {
                            (
                                "Elapsed time",
                                d.duration_secs.map(duration).unwrap_or_else(|| "—".into()),
                            )
                        } else {
                            (
                                "Time left",
                                eta_remaining(&d)
                                    .map(duration)
                                    .unwrap_or_else(|| "—".into()),
                            )
                        };
                        let downloaded_text = if has_size || complete {
                            format!("{} ({:.1}%)", bytes(d.downloaded), frac * 100.)
                        } else {
                            bytes(d.downloaded)
                        };
                        let rate_text = if complete {
                            d.avg_speed
                                .filter(|s| *s > 0)
                                .map(|s| format!("{}/s (avg)", bytes(s)))
                                .unwrap_or_else(|| "—".into())
                        } else {
                            format!("{}/s", bytes(d.speed))
                        };

                        let mut meta = section_card(p).child(
                            div()
                                .px(px(10.))
                                .py(px(7.))
                                .rounded(px(6.))
                                .bg(p.surface)
                                .border_1()
                                .border_color(p.border_subtle)
                                .child(
                                    text(d.url.clone())
                                        .text_size(px(11.))
                                        .font_family("Menlo")
                                        .text_color(p.ink_secondary)
                                        .text_ellipsis(),
                                ),
                        );

                        let pairs = [
                            (
                                ("Status", d.status.clone()),
                                ("File size", display_size(&d)),
                            ),
                            (
                                ("Downloaded", downloaded_text),
                                ("Transfer rate", rate_text),
                            ),
                            (
                                (time_label, time_value),
                                ("Resume capability", resume.into()),
                            ),
                        ];
                        for ((l1, v1), (l2, v2)) in pairs {
                            meta = meta.child(
                                row()
                                    .gap(px(16.))
                                    .child(
                                        col()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(2.))
                                            .child(helper_text(l1, p))
                                            .child(
                                                text(v1)
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(p.ink)
                                                    .text_ellipsis(),
                                            ),
                                    )
                                    .child(
                                        col()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(2.))
                                            .child(helper_text(l2, p))
                                            .child(
                                                text(v2)
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(p.ink)
                                                    .text_ellipsis(),
                                            ),
                                    ),
                            );
                        }

                        content = content.child(meta).child(
                            col()
                                .gap(px(7.))
                                .child(
                                    row()
                                        .justify_between()
                                        .child(field_label("Overall progress", p))
                                        .child(
                                            text(if downloading && !has_size {
                                                "Streaming…".into()
                                            } else {
                                                format!("{:.2}%", frac * 100.)
                                            })
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if complete || seeding {
                                                p.success
                                            } else {
                                                p.primary
                                            }),
                                        ),
                                )
                                .child(progress_bar(
                                    frac,
                                    8.,
                                    downloading,
                                    complete || seeding,
                                    is_err,
                                    downloading && !has_size,
                                    p,
                                )),
                        );

                        if self.details {
                            content = content.child(
                                helper_text(
                                    "Start positions and download progress by connections",
                                    p,
                                )
                                .text_align(TextAlign::Center),
                            );
                            let mut track = row()
                                .h(px(20.))
                                .p(px(2.))
                                .bg(p.progress_track)
                                .border_1()
                                .border_color(p.border)
                                .rounded(px(6.));
                            if let Some(segments) = self.detail["segments"].as_array() {
                                let total = d.size.unwrap_or(1).max(1) as f32;
                                let seg_count = segments.len();
                                for (idx, s) in segments.iter().enumerate() {
                                    let start = s["start"].as_u64().unwrap_or(0);
                                    let end = s["end"].as_u64().unwrap_or(0);
                                    let len = end.saturating_sub(start).max(1);
                                    let done =
                                        s["cursor"].as_u64().unwrap_or(start).saturating_sub(start);
                                    let is_first = idx == 0;
                                    let is_last = idx + 1 == seg_count;
                                    track = track.child(
                                        div()
                                            .h_full()
                                            .w(relative(len as f32 / total))
                                            .when(is_first, |d| d.rounded_l(px(4.)))
                                            .when(is_last, |d| d.rounded_r(px(4.)))
                                            .when(!is_last, |d| {
                                                d.border_r_1().border_color(p.segment_divider)
                                            })
                                            .when(done > 0, |d| {
                                                d.child(
                                                    div()
                                                        .h_full()
                                                        .w(relative(done as f32 / len as f32))
                                                        .when(is_first, |d| d.rounded_l(px(4.)))
                                                        .when(is_last, |d| d.rounded_r(px(4.)))
                                                        .bg(p.success),
                                                )
                                            }),
                                    );
                                }
                            }
                            content = content.child(track);

                            let mut conn_rows = col()
                                .id("connection-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .pb(px(4.));
                            if let Some(conns) = self.detail["connections"].as_array() {
                                for c in conns {
                                    conn_rows = conn_rows.child(
                                        row()
                                            .h(px(28.))
                                            .flex_shrink_0()
                                            .border_b_1()
                                            .border_color(p.border_subtle)
                                            .text_size(px(11.5))
                                            .child(
                                                cell(
                                                    (c["id"].as_u64().unwrap_or(0) + 1).to_string(),
                                                    44.,
                                                )
                                                .text_color(p.muted),
                                            )
                                            .child(
                                                cell(
                                                    bytes(c["downloaded"].as_u64().unwrap_or(0)),
                                                    138.,
                                                )
                                                .text_color(p.ink),
                                            )
                                            .child(
                                                cell(
                                                    format!(
                                                        "{}/s",
                                                        bytes(c["speed"].as_u64().unwrap_or(0))
                                                    ),
                                                    110.,
                                                )
                                                .text_color(p.ink_secondary),
                                            )
                                            .child(
                                                text(c["status"].as_str().unwrap_or("").to_owned())
                                                    .text_size(px(11.5))
                                                    .text_color(p.muted)
                                                    .text_ellipsis(),
                                            ),
                                    );
                                }
                            }
                            let table = col()
                                .h(px(150.))
                                .bg(p.surface)
                                .border_1()
                                .border_color(p.border)
                                .rounded(px(8.))
                                .child(
                                    row()
                                        .h(px(30.))
                                        .flex_shrink_0()
                                        .bg(p.table_header)
                                        .rounded_t(px(7.))
                                        .border_b_1()
                                        .border_color(p.border)
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.muted)
                                        .child(cell("#", 44.))
                                        .child(cell("DOWNLOADED", 138.))
                                        .child(cell("SPEED", 110.))
                                        .child(text("CONNECTION STATUS")),
                                )
                                .child(conn_rows);
                            content = content.child(table);
                        }

                        footer_bar = footer_bar
                            .justify_between()
                            .child(
                                btn(
                                    "toggle-details",
                                    if self.details {
                                        "Hide details"
                                    } else {
                                        "Show details"
                                    },
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    |s, _, _, cx| {
                                        s.details = !s.details;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                row()
                                    .gap(px(8.))
                                    .child(
                                        btn(
                                            "detail-pause",
                                            if downloading { "Pause" } else { "Resume" },
                                        )
                                        .icon(if downloading {
                                            IconName::Pause
                                        } else {
                                            IconName::Play
                                        })
                                        .disabled(complete)
                                        .on_click(
                                            cx.listener(|s, _, _, _| {
                                                s.action(
                                                    if s.selected()
                                                        .is_some_and(|d| d.status == "Downloading")
                                                    {
                                                        "pause"
                                                    } else {
                                                        "start"
                                                    },
                                                    false,
                                                )
                                            }),
                                        ),
                                    )
                                    .child(btn("detail-close", "Close").primary().on_click(
                                        cx.listener(|s, _, _, cx| {
                                            s.close_panel();
                                            cx.notify();
                                        }),
                                    )),
                            );
                    } else if self.tab == 1 {
                        content = content.child(
                            section_card(p)
                                .child(
                                    text("Per-download speed limit")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(helper_text(
                                    "Throttle this transfer without affecting other downloads in the queue.",
                                    p,
                                ))
                                .child(
                                    col()
                                        .gap(px(6.))
                                        .child(field_label("Maximum rate (KB/s)", p))
                                        .child(Input::new(&self.limit)),
                                )
                                .child(
                                    row()
                                        .gap(px(8.))
                                        .child(
                                            btn("apply-limit", "Apply limit").primary().on_click(
                                                cx.listener(|s, _, _, cx| {
                                                    match s
                                                        .limit
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .parse::<u64>()
                                                        .ok()
                                                        .and_then(|n| n.checked_mul(1024))
                                                        .filter(|n| *n > 0)
                                                    {
                                                        Some(rate) => {
                                                            s.bridge.send(
                                                                "limit",
                                                                json!({"op":"limit","id":s.selected,"bytes":rate}),
                                                            );
                                                            s.notice =
                                                                Some("Speed limit applied".into());
                                                        }
                                                        None => {
                                                            s.notice = Some(
                                                                "Enter a positive speed in KB per second"
                                                                    .into(),
                                                            )
                                                        }
                                                    }
                                                    cx.notify();
                                                }),
                                            ),
                                        )
                                        .child(btn("unlimited", "No limit").on_click(
                                            cx.listener(|s, _, _, cx| {
                                                s.bridge.send(
                                                    "limit",
                                                    json!({"op":"limit","id":s.selected,"bytes":null}),
                                                );
                                                s.notice = Some("Speed limit removed".into());
                                                cx.notify();
                                            }),
                                        )),
                                ),
                        );
                        footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(
                            cx.listener(|s, _, _, cx| {
                                s.close_panel();
                                cx.notify();
                            }),
                        ));
                    } else {
                        let path_for_open = d.path.clone();
                        content = content.child(
                            section_card(p)
                                .child(
                                    text("Destination path")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(helper_text(
                                    "Completed files remain listed in your library for quick access.",
                                    p,
                                ))
                                .child(
                                    div()
                                        .px(px(10.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(p.surface)
                                        .border_1()
                                        .border_color(p.border_subtle)
                                        .child(
                                            text(d.path.clone())
                                                .text_size(px(11.5))
                                                .font_family("Menlo")
                                                .text_color(p.ink_secondary)
                                                .text_ellipsis(),
                                        ),
                                )
                                .child(
                                    row().child(
                                        btn("open-folder", "Open destination folder")
                                            .icon(IconName::FolderOpen)
                                            .primary()
                                            .on_click(move |_, _, _| {
                                                let path =
                                                    std::path::PathBuf::from(&path_for_open);
                                                let folder = path.parent().unwrap_or(&path);
                                                let _ = open::that_detached(folder);
                                            }),
                                    ),
                                ),
                        );
                        footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(
                            cx.listener(|s, _, _, cx| {
                                s.close_panel();
                                cx.notify();
                            }),
                        ));
                    }
                } else {
                    content = content.child(muted("Select a download to view its progress.", p));
                    footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(
                        cx.listener(|s, _, _, cx| {
                            s.close_panel();
                            cx.notify();
                        }),
                    ));
                }
            }

            "remove" => {
                if let Some(d) = self.selected() {
                    let (ico_color, ico_bg) = t::category_tint(&d.category, self.dark);
                    content = content.child(
                        row()
                            .p(px(12.))
                            .gap(px(12.))
                            .rounded(px(10.))
                            .bg(p.soft)
                            .border_1()
                            .border_color(p.border_subtle)
                            .child(icon_tile(
                                category_icon(&d.category),
                                ico_color,
                                ico_bg,
                                36.,
                                17.,
                            ))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        text(d.file.clone())
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(p.ink)
                                            .text_ellipsis(),
                                    )
                                    .child(helper_text(
                                        d.size.map(bytes).unwrap_or_else(|| "Unknown size".into()),
                                        p,
                                    )),
                            ),
                    );
                }
                content = content
                    .child(
                        text("Remove the selected download from the list?")
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.ink),
                    )
                    .child(helper_text(
                        "The downloaded file will stay on your disk.",
                        p,
                    ));
                footer_bar = footer_bar
                    .child(btn("keep", "Cancel").on_click(cx.listener(|s, _, _, cx| {
                        s.close_panel();
                        cx.notify();
                    })))
                    .child(
                        btn("confirm-remove", "Remove")
                            .danger()
                            .icon(IconName::Trash)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.action("remove", false);
                                s.close_panel();
                                s.selected = None;
                                cx.notify();
                            })),
                    );
            }

            _ => {
                content = content.child(self.segmented_tabs(
                    "settings-tab",
                    ["General", "Speed & Limits", "BitTorrent"],
                    self.settings_tab,
                    true,
                    p,
                    cx,
                ));

                if self.settings_tab == 0 {
                    let mut theme_selector = row()
                        .p(px(3.))
                        .gap(px(4.))
                        .rounded(px(9.))
                        .bg(p.modal_surface)
                        .border_1()
                        .border_color(p.border);
                    for (idx, (pref, ico)) in [
                        (t::ThemePref::System, IconName::Monitor),
                        (t::ThemePref::Light, IconName::Sun),
                        (t::ThemePref::Dark, IconName::Moon),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let active = self.theme_pref == pref;
                        theme_selector = theme_selector.child(
                            row()
                                .id(("theme-opt", idx))
                                .role(Role::Tab)
                                .aria_selected(active)
                                .flex_1()
                                .justify_center()
                                .gap(px(6.))
                                .h(px(30.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .border_1()
                                .border_color(if active { p.border } else { rgba(0x00000000) })
                                .bg(if active {
                                    p.surface_elevated
                                } else {
                                    rgba(0x00000000)
                                })
                                .when(active, |d| d.shadow_xs())
                                .text_size(px(12.))
                                .font_weight(if active {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(if active { p.ink } else { p.muted })
                                .hover(|s| if active { s } else { s.text_color(p.ink) })
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.set_theme_pref(pref, w, cx);
                                }))
                                .child(icon(ico).size(px(13.5)).text_color(if active {
                                    p.primary
                                } else {
                                    p.muted
                                }))
                                .child(pref.label()),
                        );
                    }
                    let appearance_hint = match self.theme_pref {
                        t::ThemePref::System => {
                            "Automatically follows macOS Light & Dark mode with native frosted glass."
                        }
                        t::ThemePref::Light => {
                            "Always uses Light mode with translucent frosted glass chrome."
                        }
                        t::ThemePref::Dark => {
                            "Always uses Dark mode with translucent frosted glass chrome."
                        }
                    };
                    content = content
                        .child(
                            section_card(p)
                                .child(
                                    text("Appearance")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(theme_selector)
                                .child(helper_text(appearance_hint, p)),
                        )
                        .child(
                            section_card(p)
                                .child(
                                    text("Downloads & queue")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(
                                    col()
                                        .gap(px(6.))
                                        .child(field_label("Default download folder", p))
                                        .child(
                                            row()
                                                .gap(px(8.))
                                                .child(
                                                    div().flex_1().child(Input::new(&self.folder)),
                                                )
                                                .child(
                                                    btn("settings-browse-folder", "Browse…")
                                                        .icon(IconName::FolderOpen)
                                                        .on_click(cx.listener(|_, _, w, cx| {
                                                            let weak = cx.entity().downgrade();
                                                            let dialog =
                                                                rfd::AsyncFileDialog::new()
                                                                    .pick_folder();
                                                            cx.spawn_in(w, async move |_, cx| {
                                                                if let Some(folder) = dialog.await {
                                                                    let path = folder
                                                                        .path()
                                                                        .to_string_lossy()
                                                                        .into_owned();
                                                                    let _ = weak.update_in(
                                                                        cx,
                                                                        |s, w, cx| {
                                                                            s.folder.update(
                                                                                cx,
                                                                                |v, cx| {
                                                                                    v.set_value(
                                                                                        path, w, cx,
                                                                                    )
                                                                                },
                                                                            );
                                                                        },
                                                                    );
                                                                }
                                                            })
                                                            .detach();
                                                        })),
                                                ),
                                        ),
                                )
                                .child(
                                    row()
                                        .gap(px(12.))
                                        .child(
                                            col()
                                                .flex_1()
                                                .gap(px(6.))
                                                .child(field_label(
                                                    "HTTP/FTP connections (1–128)",
                                                    p,
                                                ))
                                                .child(Input::new(&self.connections)),
                                        )
                                        .child(
                                            col()
                                                .flex_1()
                                                .gap(px(6.))
                                                .child(field_label(
                                                    "Concurrent queue tasks (1–16)",
                                                    p,
                                                ))
                                                .child(Input::new(&self.max_concurrent)),
                                        ),
                                )
                                .child(helper_text(
                                    "The desktop service listens on this computer only. Downloads resume after restart.",
                                    p,
                                )),
                        )
                        .child(
                            section_card(p)
                                .child(
                                    row()
                                        .justify_between()
                                        .child(
                                            col()
                                                .flex_1()
                                                .gap(px(3.))
                                                .child(
                                                    text("Browser extension")
                                                        .text_size(px(13.))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(p.ink),
                                                )
                                                .child(helper_text(
                                                    "Connect the matching NDM extension for direct downloads and magnet links.",
                                                    p,
                                                )),
                                        )
                                        .child(
                                            btn("browser-register", "Register host")
                                                .icon(IconName::Globe)
                                                .on_click(cx.listener(|s, _, _, _| {
                                                    s.bridge.send("browser-register", json!({}))
                                                })),
                                        ),
                                )
                                .child(helper_text(
                                    "GPL-3.0-or-later · Original author credits are preserved.",
                                    p,
                                )),
                        );
                } else if self.settings_tab == 1 {
                    content = content.child(
                        section_card(p)
                            .child(
                                text("Bandwidth & speed limits")
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink),
                            )
                            .child(
                                col()
                                    .gap(px(6.))
                                    .child(field_label(
                                        "Global download speed limit (KB/s, 0 = unlimited)",
                                        p,
                                    ))
                                    .child(Input::new(&self.global_dl_limit)),
                            )
                            .child(
                                col()
                                    .gap(px(6.))
                                    .child(field_label(
                                        "Default per-download speed limit (KB/s, 0 = unlimited)",
                                        p,
                                    ))
                                    .child(Input::new(&self.per_dl_limit)),
                            )
                            .child(
                                col()
                                    .gap(px(6.))
                                    .child(field_label(
                                        "Global BitTorrent upload limit (KB/s, 0 = unlimited)",
                                        p,
                                    ))
                                    .child(Input::new(&self.torrent_ul_limit)),
                            )
                            .child(
                                row()
                                    .p(px(10.))
                                    .gap(px(8.))
                                    .rounded(px(8.))
                                    .bg(p.surface)
                                    .border_1()
                                    .border_color(p.border_subtle)
                                    .child(
                                        icon(IconName::Info).size(px(14.)).text_color(p.primary),
                                    )
                                    .child(helper_text(
                                        "Global limits apply immediately to active transfers. BitTorrent enforces a 16 KB/s minimum non-zero rate.",
                                        p,
                                    ).flex_1()),
                            ),
                    );
                } else {
                    let (tracker_status, badge_color, badge_bg) = if self.trackers_syncing {
                        ("Syncing trackers…".to_owned(), p.primary, p.accent_soft)
                    } else if self.extra_trackers_count > 0 {
                        (
                            format!(
                                "{} extra tracker{} loaded",
                                self.extra_trackers_count,
                                if self.extra_trackers_count == 1 {
                                    ""
                                } else {
                                    "s"
                                }
                            ),
                            p.success,
                            p.success_soft,
                        )
                    } else {
                        (
                            "No extra trackers configured".to_owned(),
                            p.muted,
                            p.surface,
                        )
                    };

                    content = content
                        .child(
                            section_card(p)
                                .child(
                                    text("Seeding limits")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(switch_row(
                                    "torrent-seed",
                                    "Keep seeding after completion",
                                    "Share completed torrent pieces until time or ratio limits are reached.",
                                    self.torrent_seed,
                                    p,
                                    cx.listener(|s, v: &bool, _, cx| {
                                        s.torrent_seed = *v;
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    row()
                                        .gap(px(12.))
                                        .child(
                                            col()
                                                .flex_1()
                                                .gap(px(6.))
                                                .child(field_label(
                                                    "Max seeding time (min, 0 = unlimited)",
                                                    p,
                                                ))
                                                .child(Input::new(&self.torrent_seed_minutes)),
                                        )
                                        .child(
                                            col()
                                                .flex_1()
                                                .gap(px(6.))
                                                .child(field_label(
                                                    "Max share ratio (0 = unlimited)",
                                                    p,
                                                ))
                                                .child(Input::new(&self.torrent_seed_ratio)),
                                        ),
                                ),
                        )
                        .child(
                            section_card(p)
                                .child(
                                    text("Network & peers")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink),
                                )
                                .child(
                                    col()
                                        .gap(px(6.))
                                        .child(field_label("Max peers per torrent (10–500)", p))
                                        .child(Input::new(&self.torrent_peer_limit)),
                                )
                                .child(
                                    row()
                                        .gap(px(10.))
                                        .child(
                                            div().flex_1().child(switch_row(
                                                "torrent-dht",
                                                "Enable DHT",
                                                "Decentralized peer lookup",
                                                self.torrent_dht,
                                                p,
                                                cx.listener(|s, v: &bool, _, cx| {
                                                    s.torrent_dht = *v;
                                                    cx.notify();
                                                }),
                                            )),
                                        )
                                        .child(
                                            div().flex_1().child(switch_row(
                                                "torrent-utp",
                                                "Enable uTP",
                                                "UDP transport protocol",
                                                self.torrent_utp,
                                                p,
                                                cx.listener(|s, v: &bool, _, cx| {
                                                    s.torrent_utp = *v;
                                                    cx.notify();
                                                }),
                                            )),
                                        ),
                                )
                                .child(helper_text(
                                    "Network & peer settings apply when the next torrent session starts.",
                                    p,
                                )),
                        )
                        .child(
                            section_card(p)
                                .child(
                                    row()
                                        .justify_between()
                                        .child(
                                            text("Torrent trackers")
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(p.ink),
                                        )
                                        .child(
                                            text(tracker_status)
                                                .text_size(px(11.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(badge_color)
                                                .px(px(8.))
                                                .py(px(2.))
                                                .rounded_full()
                                                .bg(badge_bg)
                                                .border_1()
                                                .border_color(p.border_subtle),
                                        ),
                                )
                                .child(helper_text(
                                    "Extra tracker subscription URLs or direct trackers, one per line.",
                                    p,
                                ))
                                .child(Textarea::new(&self.tracker_urls).h(px(96.)))
                                .child(
                                    row().justify_end().child(
                                        btn(
                                            "sync-trackers",
                                            if self.trackers_syncing {
                                                "Syncing…"
                                            } else {
                                                "Sync trackers"
                                            },
                                        )
                                        .icon(IconName::RefreshCw)
                                        .disabled(self.trackers_syncing)
                                        .on_click(cx.listener(|s, _, _, cx| {
                                            let urls = parse_tracker_urls(
                                                &s.tracker_urls.read(cx).value(),
                                            );
                                            s.trackers_syncing = true;
                                            s.trackers_error = None;
                                            s.notice = None;
                                            s.bridge.send(
                                                "trackers-sync",
                                                json!({"op":"sync_trackers","tracker_urls":urls}),
                                            );
                                            cx.notify();
                                        })),
                                    ),
                                )
                                .when_some(self.trackers_error.as_ref(), |this, err| {
                                    this.child(
                                        row()
                                            .p(px(9.))
                                            .gap(px(8.))
                                            .rounded(px(7.))
                                            .bg(p.danger_soft)
                                            .border_1()
                                            .border_color(p.danger_border)
                                            .child(
                                                icon(IconName::CircleAlert)
                                                    .size(px(14.))
                                                    .text_color(p.danger),
                                            )
                                            .child(
                                                text(err.clone())
                                                    .flex_1()
                                                    .text_size(px(11.5))
                                                    .text_color(p.danger),
                                            ),
                                    )
                                }),
                        );
                }

                footer_bar = footer_bar
                    .child(btn("cancel-settings", "Cancel").on_click(cx.listener(
                        |s, _, _, cx| {
                            s.close_panel();
                            cx.notify();
                        },
                    )))
                    .child(
                        btn("save-settings", "Save settings")
                            .primary()
                            .icon(IconName::Check)
                            .on_click(cx.listener(|s, _, _, cx| {
                                let connections = match s
                                    .connections
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u32>()
                                {
                                    Ok(n) if (1..=128).contains(&n) => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter a connection count between 1 and 128".into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let max_concurrent = match s
                                    .max_concurrent
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u32>()
                                {
                                    Ok(n) if (1..=16).contains(&n) => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter a concurrent queue limit between 1 and 16"
                                                .into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let global_dl_kb = match s
                                    .global_dl_limit
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u64>()
                                {
                                    Ok(n) if n <= 10_485_760 => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter a valid global download limit in KB/s (0 = unlimited)"
                                                .into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let per_dl_kb = match s
                                    .per_dl_limit
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u64>()
                                {
                                    Ok(n) if n <= 10_485_760 => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter a valid per-download limit in KB/s (0 = unlimited)"
                                                .into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let torrent_ul_kb = match s
                                    .torrent_ul_limit
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u64>()
                                {
                                    Ok(n) if n <= 10_485_760 => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter a valid torrent upload limit in KB/s (0 = unlimited)"
                                                .into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let seed_minutes = match s
                                    .torrent_seed_minutes
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u32>()
                                {
                                    Ok(n) if n <= 525_600 => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter seeding minutes between 0 and 525600".into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let seed_ratio = match s
                                    .torrent_seed_ratio
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<f64>()
                                {
                                    Ok(r) if r.is_finite() && (0.0..=100.0).contains(&r) => r,
                                    _ => {
                                        s.notice =
                                            Some("Enter a share ratio between 0 and 100".into());
                                        cx.notify();
                                        return;
                                    }
                                };
                                let peer_limit = match s
                                    .torrent_peer_limit
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u32>()
                                {
                                    Ok(n) if (10..=500).contains(&n) => n,
                                    _ => {
                                        s.notice = Some(
                                            "Enter max peers per torrent between 10 and 500".into(),
                                        );
                                        cx.notify();
                                        return;
                                    }
                                };
                                let tracker_urls =
                                    parse_tracker_urls(&s.tracker_urls.read(cx).value());
                                s.bridge.send(
                                    "settings-saved",
                                    json!({
                                        "op": "save_settings",
                                        "connections": connections,
                                        "folder": s.folder.read(cx).value().to_string(),
                                        "tracker_urls": tracker_urls,
                                        "max_concurrent": max_concurrent,
                                        "global_download_limit_kb": global_dl_kb,
                                        "per_download_limit_kb": per_dl_kb,
                                        "torrent_upload_limit_kb": torrent_ul_kb,
                                        "torrent_seed": s.torrent_seed,
                                        "torrent_seed_minutes": seed_minutes,
                                        "torrent_seed_ratio": seed_ratio,
                                        "torrent_peer_limit": peer_limit,
                                        "torrent_dht": s.torrent_dht,
                                        "torrent_utp": s.torrent_utp,
                                    }),
                                );
                                cx.notify();
                            })),
                    );
            }
        }

        if let Some(notice) = &self.notice {
            content = col()
                .child(
                    row()
                        .mx(px(if compact { 16. } else { 20. }))
                        .mt(px(if compact { 12. } else { 16. }))
                        .p(px(10.))
                        .gap(px(8.))
                        .rounded(px(8.))
                        .bg(p.danger_soft)
                        .border_1()
                        .border_color(p.danger_border)
                        .child(
                            icon(IconName::CircleAlert)
                                .size(px(15.))
                                .text_color(p.danger),
                        )
                        .child(
                            text(notice.clone())
                                .flex_1()
                                .text_size(px(12.))
                                .text_color(p.danger),
                        ),
                )
                .child(content);
        }

        let dialog_height = (height - 40.).min(match panel {
            "remove" => 280.,
            "add" => 360.,
            "torrent" => 640.,
            "progress" => 710.,
            "settings" => 710.,
            _ => 470.,
        });

        let dialog = col()
            .id("modal-dialog")
            .role(Role::Dialog)
            .focus_trap("native-dialog", &self.focus)
            .aria_label(title)
            .w(px(if panel == "torrent" { 640. } else { 600. }))
            .h(px(dialog_height))
            .bg(p.modal_surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(14.))
            .overflow_hidden()
            .shadow_xl()
            .child(
                row()
                    .h(px(52.))
                    .flex_shrink_0()
                    .px(px(18.))
                    .justify_between()
                    .bg(p.modal_chrome)
                    .rounded_t(px(13.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        row()
                            .gap(px(10.))
                            .child(icon_tile(header_ico, header_color, header_bg, 28., 15.))
                            .child(
                                text(title)
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink),
                            ),
                    )
                    .child(
                        Button::new("close-dialog")
                            .icon(IconName::X)
                            .ghost()
                            .h(px(28.))
                            .rounded(px(7.))
                            .accessibility_label("Close")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.close_panel();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("dialog-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(footer_bar);

        col()
            .id("modal-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .bg(p.backdrop)
            .child(dialog)
    }
}
