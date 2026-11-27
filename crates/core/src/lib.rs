//! Download application core. No window, renderer, GPUI, egui, or platform event loop.
//! Call `tick` periodically from any host and `handle` with the shared IPC DTOs.
mod browser;
mod persistence;
mod torrents;
use ndm_engine::{Destination, Download, Options, Request, download::State};
use ndm_ipc::{
    Command,
    api::{CategoryInfo, DownloadInfo, ListAnswer, QueueInfo},
    desktop::DesktopRequest,
};
use reqwest::header::{CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub connections: u32,
    pub folder: PathBuf,
    pub tracker_urls: Vec<String>,
    #[serde(default)]
    tracker_subscription_initialized: bool,
    pub extra_trackers: Vec<String>,
    pub trackers_updated: Option<String>,
    pub max_concurrent: u32,
    pub global_download_limit_kb: u64,
    pub per_download_limit_kb: u64,
    pub torrent_upload_limit_kb: u64,
    pub torrent_seed: bool,
    pub torrent_seed_minutes: u32,
    pub torrent_seed_ratio: f64,
    pub torrent_peer_limit: u32,
    pub torrent_dht: bool,
    pub torrent_utp: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            connections: 8,
            folder: dirs::download_dir().unwrap_or_else(|| PathBuf::from("downloads")),
            tracker_urls: vec![torrents::DEFAULT_TRACKER_SUBSCRIPTION.into()],
            tracker_subscription_initialized: true,
            extra_trackers: Vec::new(),
            trackers_updated: None,
            max_concurrent: 4,
            global_download_limit_kb: 0,
            per_download_limit_kb: 0,
            torrent_upload_limit_kb: 0,
            torrent_seed: false,
            torrent_seed_minutes: 60,
            torrent_seed_ratio: 1.0,
            torrent_peer_limit: 128,
            torrent_dht: true,
            torrent_utp: true,
        }
    }
}
fn kb_to_bps(kb: u64) -> Option<u64> {
    (kb > 0).then(|| kb.saturating_mul(1024))
}
#[derive(Clone, Serialize, Deserialize)]
struct TorrentRecord {
    hash: String,
    bytes: Vec<u8>,
    folder: PathBuf,
    files: Vec<usize>,
    paths: Vec<ndm_torrent_file::File>,
    #[serde(default)]
    peers: Vec<std::net::SocketAddr>,
    #[serde(default)]
    seeded_secs: u64,
    #[serde(default)]
    uploaded: u64,
}
// Paths and sizes persisted as DTOs, independent of the torrent library's runtime types.
mod ndm_torrent_file {
    use super::*;
    #[derive(Clone, Serialize, Deserialize)]
    pub struct File {
        pub path: PathBuf,
        pub size: u64,
        pub padding: bool,
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    info: DownloadInfo,
    limit: Option<u64>,
    #[serde(default)]
    torrent: Option<TorrentRecord>,
    #[serde(default)]
    detail: Value,
    #[serde(default)]
    open_torrent: bool,
    #[serde(default)]
    active_secs: f64,
}
struct Running {
    download: Download,
    last: u64,
    at: Instant,
    started: Instant,
    stopping: bool,
}
type Preview = Arc<Mutex<Option<Result<ndm_torrent::Meta, String>>>>;
struct PreviewJob {
    lookup: Arc<ndm_torrent::Lookup>,
    slot: Preview,
    task: tokio::task::JoinHandle<()>,
    created: Instant,
}
struct TrackerSyncJob {
    rx: tokio::sync::watch::Receiver<Option<ndm_torrent::trackers::FetchOutcome>>,
    task: tokio::task::JoinHandle<()>,
}
pub struct Core {
    data: PathBuf,
    settings: Settings,
    records: Vec<Record>,
    next_id: u64,
    running: HashMap<u64, Running>,
    contexts: HashMap<u64, Command>,
    torrent: Option<Arc<ndm_torrent::Torrents>>,
    previews: HashMap<String, PreviewJob>,
    tracker_sync: Option<TrackerSyncJob>,
    tracker_sync_error: Option<String>,
    last_tracker_sync: Option<Instant>,
    global_limiter: Arc<ndm_engine::limiter::Limiter>,
    seeding_tick: HashMap<u64, Instant>,
    torrent_started: HashMap<u64, Instant>,
    queue_running: bool,
    dirty: bool,
    last_save: Instant,
    storage_error: Option<String>,
    browser_events: std::collections::VecDeque<Value>,
}
impl Core {
    pub fn open(data: PathBuf) -> Result<Self, String> {
        ndm_ipc::private_dir(&data).map_err(|e| e.to_string())?;
        let (mut settings, mut records, next_id) = persistence::load(&data)?;
        let mut dirty = false;
        if !settings.tracker_subscription_initialized {
            if settings.tracker_urls.is_empty() {
                settings
                    .tracker_urls
                    .push(torrents::DEFAULT_TRACKER_SUBSCRIPTION.into());
            }
            settings.tracker_subscription_initialized = true;
            dirty = true;
        }
        for r in &mut records {
            if matches!(r.info.status.as_str(), "Downloading" | "Seeding") {
                r.info.status = "Stopped".into();
                dirty = true;
            }
            if r.info.status == "Complete" && r.info.size.is_none_or(|n| n == 0) {
                let recovered = (r.info.downloaded > 0)
                    .then_some(r.info.downloaded)
                    .or_else(|| {
                        std::fs::metadata(&r.info.path)
                            .ok()
                            .map(|m| m.len())
                            .filter(|n| *n > 0)
                    });
                if recovered.is_some() {
                    r.info.size = recovered;
                    dirty = true;
                }
            }
            r.info.speed = 0;
        }
        let global_limiter = Arc::new(ndm_engine::limiter::Limiter::new(kb_to_bps(
            settings.global_download_limit_kb,
        )));
        Ok(Self {
            data,
            settings,
            records,
            next_id,
            running: HashMap::new(),
            contexts: HashMap::new(),
            torrent: None,
            previews: HashMap::new(),
            tracker_sync: None,
            tracker_sync_error: None,
            last_tracker_sync: None,
            global_limiter,
            seeding_tick: HashMap::new(),
            torrent_started: HashMap::new(),
            queue_running: false,
            dirty,
            last_save: Instant::now(),
            storage_error: None,
            browser_events: Default::default(),
        })
    }
    pub fn handle(&mut self, command: Command) -> Value {
        if let Some(value) = command.desktop {
            return match serde_json::from_value::<DesktopRequest>(value) {
                Ok(request) => self.desktop(request),
                Err(e) => json!({"error":format!("Invalid desktop request: {e}")}),
            };
        }
        if command.query.as_deref() == Some("list") {
            return self.list();
        }
        if let Some(action) = command.action.as_deref() {
            return self.act(action, &command.ids);
        }
        if command.api.is_some() {
            return json!({"api":true});
        }
        if command.url.is_some() {
            return self.add(command);
        }
        json!({"ok":true})
    }
    fn desktop(&mut self, request: DesktopRequest) -> Value {
        match request {
            DesktopRequest::List => self.list(),
            DesktopRequest::Browser { command } => self.browser(*command),
            DesktopRequest::Events => {
                json!({"events":self.browser_events.drain(..).collect::<Vec<_>>()})
            }
            DesktopRequest::Add { command } => self.add(*command),
            DesktopRequest::Action { action, ids } => self.act(&action, &ids),
            DesktopRequest::Detail { id } => self.detail(id),
            DesktopRequest::Limit { id, bytes } => {
                let Some(r) = self.records.iter_mut().find(|r| r.info.id == id) else {
                    return error("Download not found");
                };
                r.limit = bytes.filter(|n| *n > 0);
                if let Some(l) = self.running.get(&id) {
                    l.download.set_speed_limit(r.limit);
                }
                if let (Some(t), Some(spec)) = (&self.torrent, &r.torrent) {
                    let _ = t.set_torrent_limits(&spec.hash, r.limit, None);
                }
                self.dirty = true;
                json!({"ok":true})
            }
            DesktopRequest::Queue { start } => {
                self.queue_running = start;
                if !start {
                    let ids: Vec<_> = self
                        .records
                        .iter()
                        .filter(|r| r.info.status == "Downloading")
                        .map(|r| r.info.id)
                        .collect();
                    self.act("pause", &ids);
                }
                json!({"ok":true})
            }
            DesktopRequest::Settings => {
                self.poll_tracker_sync();
                json!({
                    "connections": self.settings.connections,
                    "folder": self.settings.folder,
                    "tracker_urls": self.settings.tracker_urls,
                    "extra_trackers_count": self.settings.extra_trackers.len(),
                    "trackers_updated": self.settings.trackers_updated,
                    "trackers_syncing": self.tracker_sync.is_some(),
                    "trackers_error": self.tracker_sync_error,
                    "max_concurrent": self.settings.max_concurrent,
                    "global_download_limit_kb": self.settings.global_download_limit_kb,
                    "per_download_limit_kb": self.settings.per_download_limit_kb,
                    "torrent_upload_limit_kb": self.settings.torrent_upload_limit_kb,
                    "torrent_seed": self.settings.torrent_seed,
                    "torrent_seed_minutes": self.settings.torrent_seed_minutes,
                    "torrent_seed_ratio": self.settings.torrent_seed_ratio,
                    "torrent_peer_limit": self.settings.torrent_peer_limit,
                    "torrent_dht": self.settings.torrent_dht,
                    "torrent_utp": self.settings.torrent_utp,
                    "remote": false,
                    "protocol": ndm_ipc::desktop::VERSION,
                })
            }
            DesktopRequest::SaveSettings {
                connections,
                folder,
                tracker_urls,
                max_concurrent,
                global_download_limit_kb,
                per_download_limit_kb,
                torrent_upload_limit_kb,
                torrent_seed,
                torrent_seed_minutes,
                torrent_seed_ratio,
                torrent_peer_limit,
                torrent_dht,
                torrent_utp,
            } => {
                if !(1..=128).contains(&connections) {
                    return error("Connections must be between 1 and 128");
                }
                if let Some(mc) = max_concurrent
                    && !(1..=16).contains(&mc)
                {
                    return error("Maximum concurrent downloads must be between 1 and 16");
                }
                for (label, v) in [
                    ("Global download limit", global_download_limit_kb),
                    ("Per-download limit", per_download_limit_kb),
                    ("Torrent upload limit", torrent_upload_limit_kb),
                ] {
                    if let Some(kb) = v
                        && kb > 10_485_760
                    {
                        return error(&format!("{label} must be at most 10485760 KB/s"));
                    }
                }
                if let Some(mins) = torrent_seed_minutes
                    && mins > 525_600
                {
                    return error("Seeding time limit must be at most 525600 minutes");
                }
                if let Some(ratio) = torrent_seed_ratio
                    && (!ratio.is_finite() || !(0.0..=100.0).contains(&ratio))
                {
                    return error("Share ratio limit must be between 0 and 100");
                }
                if let Some(peers) = torrent_peer_limit
                    && !(10..=500).contains(&peers)
                {
                    return error("Max peers per torrent must be between 10 and 500");
                }
                let folder = PathBuf::from(folder);
                if !folder.is_absolute() {
                    return error("Choose an absolute download folder");
                }
                if let Err(e) = std::fs::create_dir_all(&folder) {
                    return error(&e.to_string());
                }
                if let Some(urls) = tracker_urls
                    && let Err(e) = self.apply_tracker_urls(urls, false)
                {
                    return error(&e);
                }
                self.settings.connections = connections;
                self.settings.folder = folder;
                if let Some(mc) = max_concurrent {
                    self.settings.max_concurrent = mc;
                }
                if let Some(kb) = global_download_limit_kb {
                    self.settings.global_download_limit_kb = kb;
                }
                if let Some(kb) = per_download_limit_kb {
                    self.settings.per_download_limit_kb = kb;
                }
                if let Some(kb) = torrent_upload_limit_kb {
                    self.settings.torrent_upload_limit_kb = kb;
                }
                if let Some(seed) = torrent_seed {
                    self.settings.torrent_seed = seed;
                }
                if let Some(mins) = torrent_seed_minutes {
                    self.settings.torrent_seed_minutes = mins;
                }
                if let Some(ratio) = torrent_seed_ratio {
                    self.settings.torrent_seed_ratio = ratio;
                }
                if let Some(peers) = torrent_peer_limit {
                    self.settings.torrent_peer_limit = peers;
                }
                if let Some(dht) = torrent_dht {
                    self.settings.torrent_dht = dht;
                }
                if let Some(utp) = torrent_utp {
                    self.settings.torrent_utp = utp;
                }
                let global_dl = kb_to_bps(self.settings.global_download_limit_kb);
                self.global_limiter.set(global_dl);
                if let Some(t) = &self.torrent {
                    t.set_download_limit(global_dl);
                    t.set_upload_limit(kb_to_bps(self.settings.torrent_upload_limit_kb));
                }
                self.dirty = true;
                self.save();
                json!({
                    "ok": true,
                    "trackers_syncing": self.tracker_sync.is_some(),
                    "extra_trackers_count": self.settings.extra_trackers.len(),
                })
            }
            DesktopRequest::SyncTrackers { tracker_urls } => {
                if let Some(urls) = tracker_urls {
                    if let Err(e) = self.apply_tracker_urls(urls, true) {
                        return error(&e);
                    }
                } else {
                    self.start_tracker_sync();
                }
                self.dirty = true;
                self.save();
                json!({
                    "ok": true,
                    "trackers_syncing": self.tracker_sync.is_some(),
                    "extra_trackers_count": self.settings.extra_trackers.len(),
                    "trackers_error": self.tracker_sync_error,
                })
            }
            DesktopRequest::TorrentInspect { source } => self.inspect_torrent(source),
            DesktopRequest::TorrentPoll { token } => self.poll_torrent(&token),
            DesktopRequest::TorrentCancel { token } => {
                if let Some(p) = self.previews.remove(&token) {
                    p.task.abort();
                }
                json!({"ok":true})
            }
            DesktopRequest::TorrentAdd {
                token,
                dir,
                files,
                subfolder,
                later,
            } => self.add_torrent(token, dir, files, subfolder, later),
        }
    }
    fn list(&self) -> Value {
        if let Some(e) = &self.storage_error {
            return error(e);
        }
        let list = ListAnswer {
            downloads: self.records.iter().map(|r| r.info.clone()).collect(),
            speed: self.records.iter().map(|r| r.info.speed).sum(),
            queues: vec![QueueInfo {
                name: "Main queue".into(),
                running: self.queue_running,
                ids: self
                    .records
                    .iter()
                    .filter(|r| r.info.status == "Queued")
                    .map(|r| r.info.id)
                    .collect(),
            }],
            categories: [
                "General",
                "Compressed",
                "Documents",
                "Music",
                "Programs",
                "Video",
                "Torrents",
            ]
            .iter()
            .map(|s| CategoryInfo {
                key: (*s).into(),
                name: (*s).into(),
            })
            .collect(),
            free: ndm_engine::disk::volume_of(&self.settings.folder).map(|v| v.free),
        };
        serde_json::to_value(list).unwrap_or_default()
    }
    fn add(&mut self, cmd: Command) -> Value {
        let Some(url) = cmd.url.as_deref().and_then(|s| Url::parse(s.trim()).ok()) else {
            return error("Enter a valid download address");
        };
        if url.scheme() == "magnet" {
            return self.inspect_torrent(url.to_string());
        }
        if !ndm_engine::supported(&url) {
            return error("Unsupported download scheme");
        }
        let folder = cmd
            .dir
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.settings.folder.clone());
        if let Err(e) = std::fs::create_dir_all(&folder) {
            return error(&e.to_string());
        }
        let name = ndm_engine::naming::sanitize(
            &cmd.name
                .clone()
                .unwrap_or_else(|| ndm_engine::naming::file_name(None, &url)),
        );
        let mut path = ndm_engine::naming::unique_path(&folder, &name);
        if self
            .records
            .iter()
            .any(|r| std::path::Path::new(&r.info.path) == path)
        {
            let stem = std::path::Path::new(&name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = std::path::Path::new(&name)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            for index in 1u64.. {
                let candidate =
                    ndm_engine::naming::unique_path(&folder, &format!("{stem} ({index}){ext}"));
                if !self
                    .records
                    .iter()
                    .any(|r| std::path::Path::new(&r.info.path) == candidate)
                {
                    path = candidate;
                    break;
                }
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.records.push(Record {
            info: DownloadInfo {
                id,
                file: name.clone(),
                url: url.to_string(),
                path: path.to_string_lossy().into_owned(),
                status: if cmd.later { "Queued" } else { "Stopped" }.into(),
                size: cmd.size,
                downloaded: 0,
                speed: 0,
                avg_speed: None,
                duration_secs: None,
                added: Some(chrono::Utc::now().to_rfc3339()),
                category: cmd
                    .category
                    .clone()
                    .unwrap_or_else(|| category(&name).into()),
                error: None,
            },
            limit: kb_to_bps(self.settings.per_download_limit_kb),
            torrent: None,
            detail: Value::Null,
            open_torrent: false,
            active_secs: 0.0,
        });
        self.contexts.insert(id, cmd.clone());
        self.dirty = true;
        if !cmd.later
            && let Err(e) = self.start_file(id, Some(cmd))
        {
            self.fail(id, e);
        }
        self.save();
        json!({"id":id,"ok":true})
    }
    fn start_file(&mut self, id: u64, context: Option<Command>) -> Result<(), String> {
        if self.running.contains_key(&id) {
            return Err("The download is still stopping; wait a moment".into());
        }
        let r = self
            .records
            .iter_mut()
            .find(|r| r.info.id == id)
            .ok_or("Download not found")?;
        let cmd = context
            .or_else(|| self.contexts.get(&id).cloned())
            .unwrap_or_default();
        let url = Url::parse(&r.info.url).map_err(|e| e.to_string())?;
        let mut headers = HeaderMap::new();
        for (key, value) in [
            (COOKIE, cmd.cookies),
            (REFERER, cmd.referer),
            (USER_AGENT, cmd.user_agent),
            (ORIGIN, cmd.origin),
            (CONTENT_TYPE, cmd.post_type),
        ] {
            if let Some(value) = value {
                headers.insert(
                    key,
                    HeaderValue::from_str(&value).map_err(|e| e.to_string())?,
                );
            }
        }
        let opts = Options {
            max_connections: self.settings.connections,
            speed_limit: r.limit,
            global: Some(self.global_limiter.clone()),
            proxy: if let Some(script) = cmd.proxy_pac {
                ndm_engine::Proxy::Pac {
                    script: script.into(),
                }
            } else if let Some(url) = cmd.proxy {
                ndm_engine::Proxy::Manual {
                    url,
                    login: None,
                    http: true,
                    https: true,
                }
            } else {
                ndm_engine::Proxy::None
            },
            post: cmd.post.map(Into::into),
            checksum: cmd
                .checksum
                .as_deref()
                .map(|s| {
                    ndm_engine::verify::Hash::parse(s).ok_or_else(|| "Invalid checksum".to_owned())
                })
                .transpose()?,
            ..Default::default()
        };
        let download = Download::resume_or_start(
            ndm_engine::state::state_path(std::path::Path::new(&r.info.path)),
            Request {
                url,
                headers,
                dest: Destination::Free(PathBuf::from(&r.info.path)),
            },
            opts,
            None,
            r.info.downloaded,
        );
        r.info.status = "Downloading".into();
        r.info.error = None;
        self.running.insert(
            id,
            Running {
                download,
                last: r.info.downloaded,
                at: Instant::now(),
                started: Instant::now(),
                stopping: false,
            },
        );
        self.dirty = true;
        Ok(())
    }
    fn fail(&mut self, id: u64, message: String) {
        if let Some(r) = self.records.iter_mut().find(|r| r.info.id == id) {
            r.info.status = "Error".into();
            r.info.error = Some(message);
            r.info.speed = 0;
        }
        self.dirty = true;
    }
    fn act(&mut self, action: &str, ids: &[u64]) -> Value {
        if action == "start_queue" || action == "stop_queue" {
            self.queue_running = action == "start_queue";
            return json!({"done":[],"skipped":[]});
        }
        let mut done = Vec::new();
        let mut skipped = Vec::new();
        for id in ids {
            let Some(at) = self.records.iter().position(|r| r.info.id == *id) else {
                skipped.push(json!({"id":id,"why":"Download not found"}));
                continue;
            };
            let result: Result<(), String> = match action {
                "pause" => self.pause(*id),
                "start" => {
                    if self.records[at].info.status == "Complete" {
                        Err("Already complete".into())
                    } else if self.records[at].torrent.is_some() {
                        self.start_torrent(*id)
                    } else {
                        self.start_file(*id, None)
                    }
                }
                "remove" | "delete" => {
                    if matches!(
                        self.records[at].info.status.as_str(),
                        "Downloading" | "Seeding"
                    ) || self.running.contains_key(id)
                    {
                        Err("Pause the download before removing it".into())
                    } else if action == "delete" {
                        Err("Delete files through the file manager; remove keeps your files".into())
                    } else {
                        self.contexts.remove(id);
                        self.seeding_tick.remove(id);
                        self.torrent_started.remove(id);
                        let r = self.records.remove(at);
                        if let (Some(t), Some(spec)) = (&self.torrent, &r.torrent) {
                            let _ = tokio::runtime::Handle::current()
                                .block_on(t.remove(&spec.hash, false));
                        }
                        Ok(())
                    }
                }
                _ => Err("Unknown action".into()),
            };
            match result {
                Ok(()) => done.push(*id),
                Err(why) => skipped.push(json!({"id":id,"why":why})),
            }
        }
        self.dirty = true;
        self.save();
        json!({"done":done,"skipped":skipped})
    }
    fn pause(&mut self, id: u64) -> Result<(), String> {
        self.seeding_tick.remove(&id);
        if let Some(l) = self.running.get_mut(&id) {
            l.stopping = true;
            l.download.cancel();
        }
        let torrent_elapsed = self.torrent_started.remove(&id);
        let r = self
            .records
            .iter_mut()
            .find(|r| r.info.id == id)
            .ok_or("Download not found")?;
        if let Some(started) = torrent_elapsed {
            r.active_secs += started.elapsed().as_secs_f64();
        }
        if let (Some(t), Some(spec)) = (&self.torrent, &r.torrent) {
            tokio::runtime::Handle::current()
                .block_on(t.pause(&spec.hash))
                .map_err(|e| e.to_string())?;
        }
        r.info.status = "Stopped".into();
        r.info.speed = 0;
        Ok(())
    }
    pub fn tick(&mut self) {
        let mut finished = Vec::new();
        for (id, live) in &mut self.running {
            let snapshot = live.download.snapshot();
            let Some(r) = self.records.iter_mut().find(|r| r.info.id == *id) else {
                continue;
            };
            r.detail = snapshot_detail(*id, r.limit, &snapshot);
            r.info.downloaded = snapshot.downloaded;
            r.info.size = snapshot.size.or(r.info.size);
            if let Some(path) = &snapshot.file {
                r.info.path = path.to_string_lossy().into_owned();
                r.info.file = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
            }
            let conn_speed: u64 = snapshot.connections.iter().map(|c| c.speed).sum();
            let elapsed = live.at.elapsed().as_secs_f64();
            if elapsed >= 0.25 {
                if live.stopping {
                    r.info.speed = 0;
                } else {
                    let delta = snapshot.downloaded.saturating_sub(live.last);
                    let inst = (delta as f64 / elapsed) as u64;
                    let sample = if inst > 0 { inst } else { conn_speed };
                    let smoothed = if r.info.speed == 0 {
                        sample
                    } else {
                        ((r.info.speed as f64 * 0.55) + (sample as f64 * 0.45)).round() as u64
                    };
                    r.info.speed = if sample == 0 && smoothed < 64 {
                        0
                    } else {
                        smoothed
                    };
                }
                live.last = snapshot.downloaded;
                live.at = Instant::now();
            } else if !live.stopping && r.info.speed == 0 && conn_speed > 0 {
                r.info.speed = conn_speed;
            }
            match snapshot.state {
                State::Done => {
                    if r.info.size.is_none_or(|n| n == 0) {
                        r.info.size = (r.info.downloaded > 0)
                            .then_some(r.info.downloaded)
                            .or_else(|| {
                                std::fs::metadata(&r.info.path)
                                    .ok()
                                    .map(|m| m.len())
                                    .filter(|n| *n > 0)
                            });
                    }
                    r.active_secs += live.started.elapsed().as_secs_f64();
                    r.info.duration_secs = Some(r.active_secs.round().max(1.0) as u64);
                    r.info.avg_speed = (r.info.downloaded > 0)
                        .then(|| (r.info.downloaded as f64 / r.active_secs.max(0.05)) as u64);
                    r.info.status = "Complete".into();
                    r.info.speed = 0;
                    finished.push(*id);
                }
                State::Cancelled => {
                    r.active_secs += live.started.elapsed().as_secs_f64();
                    r.info.status = "Stopped".into();
                    r.info.speed = 0;
                    finished.push(*id);
                }
                State::Failed => {
                    r.active_secs += live.started.elapsed().as_secs_f64();
                    r.info.status = "Error".into();
                    r.info.error = snapshot.error;
                    r.info.speed = 0;
                    finished.push(*id);
                }
                _ => {}
            }
        }
        let ready: Vec<_> = self
            .records
            .iter_mut()
            .filter(|r| r.open_torrent && r.info.status == "Complete")
            .map(|r| {
                r.open_torrent = false;
                r.info.path.clone()
            })
            .collect();
        for source in ready {
            self.browser_events
                .push_back(json!({"kind":"torrent","source":source}));
        }
        for id in finished {
            self.running.remove(&id);
            self.dirty = true;
        }
        self.tick_torrents();
        self.poll_tracker_sync();
        self.refresh_trackers_if_due();
        if self.queue_running {
            let active = self
                .records
                .iter()
                .filter(|r| r.info.status == "Downloading")
                .count();
            let max_concurrent = self.settings.max_concurrent.clamp(1, 16) as usize;
            let ids: Vec<_> = self
                .records
                .iter()
                .filter(|r| r.info.status == "Queued")
                .take(max_concurrent.saturating_sub(active))
                .map(|r| r.info.id)
                .collect();
            for id in ids {
                let result = if self
                    .records
                    .iter()
                    .any(|r| r.info.id == id && r.torrent.is_some())
                {
                    self.start_torrent(id)
                } else {
                    self.start_file(id, None)
                };
                if let Err(e) = result {
                    self.fail(id, e);
                }
            }
            if !self
                .records
                .iter()
                .any(|r| matches!(r.info.status.as_str(), "Queued" | "Downloading"))
            {
                self.queue_running = false;
            }
        }
        self.previews.retain(|_, job| {
            if job.created.elapsed() > Duration::from_secs(600) {
                job.task.abort();
                false
            } else {
                true
            }
        });
        if self.last_save.elapsed() > Duration::from_secs(2) {
            if !self.running.is_empty() {
                self.dirty = true;
            }
            self.save();
        }
    }
    fn detail(&self, id: u64) -> Value {
        let Some(r) = self.records.iter().find(|r| r.info.id == id) else {
            return error("Download not found");
        };
        let Some(l) = self.running.get(&id) else {
            return if r.detail.is_null() {
                json!({"id":id,"limit":r.limit,"connections":[],"segments":[],"resumable":null})
            } else {
                r.detail.clone()
            };
        };
        let s = l.download.snapshot();
        json!({"id":id,"limit":r.limit,"resumable":s.resumable,"connections":s.connections.iter().map(|c|json!({"id":c.id,"downloaded":c.downloaded,"speed":c.speed,"status":format!("{:?}",c.status)})).collect::<Vec<_>>(),"segments":s.segments.iter().map(|p|json!({"start":p.start,"cursor":p.cursor,"end":p.end})).collect::<Vec<_>>(),"events":s.events.iter().map(|e|e.text.clone()).collect::<Vec<_>>()})
    }
    pub fn shutdown(&mut self) {
        let ids: Vec<_> = self
            .records
            .iter()
            .filter(|r| matches!(r.info.status.as_str(), "Downloading" | "Seeding"))
            .map(|r| r.info.id)
            .collect();
        for id in ids {
            let _ = self.pause(id);
        }
        let start = Instant::now();
        while !self.running.is_empty() && start.elapsed() < Duration::from_secs(10) {
            self.tick();
            std::thread::sleep(Duration::from_millis(30));
        }
        for job in self.previews.values() {
            job.task.abort();
        }
        if let Some(job) = self.tracker_sync.take() {
            job.task.abort();
        }
        if let Some(t) = &self.torrent {
            tokio::runtime::Handle::current().block_on(t.stop());
        }
        self.dirty = true;
        self.save();
    }
    fn save(&mut self) {
        if !self.dirty {
            return;
        }
        match persistence::save(self) {
            Ok(()) => {
                self.dirty = false;
                self.storage_error = None;
            }
            Err(e) => self.storage_error = Some(format!("Cannot save download state: {e}")),
        }
        self.last_save = Instant::now();
    }
}
fn error(s: &str) -> Value {
    json!({"error":s})
}
fn category(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "zip" | "rar" | "gz" | "7z" | "xz" => "Compressed",
        "pdf" | "doc" | "docx" | "txt" => "Documents",
        "mp3" | "flac" | "wav" => "Music",
        "mp4" | "mkv" | "webm" => "Video",
        "exe" | "dmg" | "msi" | "deb" | "appimage" => "Programs",
        _ => "General",
    }
}

fn snapshot_detail(id: u64, limit: Option<u64>, s: &ndm_engine::Snapshot) -> Value {
    json!({"id":id,"limit":limit,"resumable":s.resumable,"connections":s.connections.iter().map(|c|json!({"id":c.id,"downloaded":c.downloaded,"speed":c.speed,"status":format!("{:?}",c.status)})).collect::<Vec<_>>(),"segments":s.segments.iter().map(|p|json!({"start":p.start,"cursor":p.cursor,"end":p.end})).collect::<Vec<_>>(),"events":s.events.iter().map(|e|e.text.clone()).collect::<Vec<_>>()})
}
