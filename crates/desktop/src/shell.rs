mod dialogs;
mod workspace;

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

gpui_kit::actions!(ndm, [ClosePanel, NewDownload, ShowDetails, FocusSearch]);

pub struct Shell {
    focus: FocusHandle,
    row_focus: std::collections::HashMap<u64, FocusHandle>,
    pub bridge: Bridge,
    theme_pref: t::ThemePref,
    dark: bool,
    _appearance_sub: Subscription,
    list: ListAnswer,
    selected: Option<u64>,
    filter: workspace::Filter,
    view: workspace::View,
    sidebar_collapsed: bool,
    inspector_open: bool,
    search: Entity<InputState>,
    source: Entity<InputState>,
    folder: Entity<InputState>,
    limit: Entity<InputState>,
    panel: Option<&'static str>,
    detail: Value,
    tab: usize,
    details: bool,
    notice: Option<String>,
    service_error: Option<String>,
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

fn message_text(value: impl Into<SharedString>) -> Div {
    text(value)
        .min_w_0()
        .whitespace_normal()
        .line_height(px(18.))
        .text_size(px(12.))
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
        .rounded(px(t::CONTROL_RADIUS))
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
        fill.bg(p.primary)
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
    let paused_or_queued = matches!(status, "Stopped" | "Paused" | "Queued" | "Waiting");
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
        .rounded(px(t::CONTROL_RADIUS))
        .bg(p.modal_chrome)
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
        #[cfg(target_os = "macos")]
        let (add_key, detail_key, search_key) = ("cmd-n", "cmd-i", "cmd-f");
        #[cfg(not(target_os = "macos"))]
        let (add_key, detail_key, search_key) = ("ctrl-n", "ctrl-i", "ctrl-f");
        cx.bind_keys([
            KeyBinding::new("escape", ClosePanel, None),
            KeyBinding::new(add_key, NewDownload, None),
            KeyBinding::new(detail_key, ShowDetails, None),
            KeyBinding::new(search_key, FocusSearch, None),
        ]);
        let search = input("", "Search downloads…", window, cx);
        cx.subscribe(&search, |this, state, _: &InputEvent, cx| {
            let query = state.read(cx).value().trim().to_lowercase();
            if this
                .selected()
                .is_some_and(|download| !workspace::matches_search(download, &query))
            {
                this.selected = None;
            }
            cx.notify();
        })
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
            TextareaState::new(window, cx)
                .placeholder("Tracker subscription or announce URL, one per line")
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
            filter: workspace::Filter::All,
            view: workspace::View::List,
            sidebar_collapsed: false,
            inspector_open: false,
            search,
            source,
            folder,
            limit,
            panel: None,
            detail: Value::Null,
            tab: 0,
            details: true,
            notice: None,
            service_error: None,
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
            if m.value["service_error"] == true {
                let error = m.value["error"]
                    .as_str()
                    .unwrap_or("The download service is unavailable.");
                let updating_notice = self.service_error.is_some()
                    && self.notice.as_deref() == self.service_error.as_deref();
                self.service_error = Some(error.into());
                if updating_notice
                    || !matches!(
                        m.tag.as_str(),
                        "list" | "events" | "detail" | "torrent" | "settings-status" | "cancel"
                    )
                {
                    self.notice = Some(error.into());
                }
                if m.tag == "trackers-sync" {
                    self.trackers_syncing = false;
                }
                continue;
            }
            if m.tag != "browser-register"
                && let Some(previous) = self.service_error.take()
                && self.notice.as_deref() == Some(previous.as_str())
            {
                self.notice = None;
            }
            if let Some(error) = m.value["error"].as_str() {
                self.notice = Some(error.into());
                if m.tag == "trackers-sync" {
                    self.trackers_syncing = false;
                }
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
                        if self.panel.is_none()
                            && let Some(id) = self.selected
                        {
                            let updated = list.downloads.iter().find(|d| d.id == id);
                            let hidden = updated.is_some_and(|d| !self.filter.matches(d));
                            let removed = updated.is_none() && self.selected().is_some();
                            if hidden || removed {
                                self.selected = None;
                            }
                        }
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
                    self.filter = workspace::Filter::All;
                    self.search
                        .update(cx, |v, cx| v.set_value(String::new(), window, cx));
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
            self.selected().map(|d| d.id).into_iter().collect()
        };
        if ids.is_empty() {
            return;
        }
        self.bridge
            .send("action", json!({"op":"action","action":action,"ids":ids}));
    }

    fn close_panel(&mut self) {
        if let Some(token) = self.torrent_token.take() {
            self.bridge
                .send("cancel", json!({"op":"torrent_cancel","token":token}));
        }
        if self.panel.take().is_some() {
            self.notice = None;
        }
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
}
