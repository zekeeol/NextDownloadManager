use super::*;
use ndm_torrent::authorized_storage::{self, PreparedFile};
use std::{collections::HashSet, path::Path};

pub(super) const DEFAULT_TRACKER_SUBSCRIPTION: &str =
    "https://tracker.adysec.com/trackers_best.txt";
const TRACKER_REFRESH_AGE: chrono::TimeDelta = chrono::TimeDelta::hours(24);
const TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60);
const METADATA_TIMEOUT: Duration = Duration::from_secs(90);
const METADATA_IDLE_GRACE: Duration = Duration::from_secs(35);
const METADATA_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

impl Core {
    fn session(&mut self) -> Result<Arc<ndm_torrent::Torrents>, String> {
        if let Some(t) = &self.torrent {
            return Ok(t.clone());
        }
        let mut cfg = ndm_torrent::Config::new(self.data.join("torrents"));
        cfg.upnp = false;
        cfg.lsd = false;
        cfg.dht = self.settings.torrent_dht;
        cfg.utp = self.settings.torrent_utp;
        cfg.peer_limit = Some(self.settings.torrent_peer_limit.clamp(10, 500) as usize);
        cfg.extra_trackers = self.settings.extra_trackers.clone();
        for record in &self.records {
            if let Some(spec) = &record.torrent {
                let root = spec.folder.clone();
                let paths = spec.paths.clone();
                let prepared = prepare(&root, &paths).map_err(|e| e.to_string())?;
                let reopen = root.clone();
                authorized_storage::authorize(
                    &self.data.join("torrents/session"),
                    &spec.hash,
                    &root,
                    prepared,
                    Arc::new(move || prepare(&reopen, &paths)),
                );
            }
        }
        let torrent = tokio::runtime::Handle::current()
            .block_on(ndm_torrent::Torrents::start(cfg))
            .map_err(|e| e.to_string())?;
        torrent.set_download_limit(kb_to_bps(self.settings.global_download_limit_kb));
        torrent.set_upload_limit(kb_to_bps(self.settings.torrent_upload_limit_kb));
        let torrent = Arc::new(torrent);
        self.torrent = Some(torrent.clone());
        Ok(torrent)
    }
    pub(super) fn apply_tracker_urls(
        &mut self,
        raw_urls: Vec<String>,
        force_sync: bool,
    ) -> Result<(), String> {
        let mut normalized = Vec::new();
        let mut seen = HashSet::new();
        for raw in raw_urls {
            for parsed in ndm_torrent::trackers::parse_config_urls(&raw) {
                let valid = ndm_torrent::trackers::validate_source_url(&parsed)
                    .map_err(|e| format!("Tracker source {}: {e}", normalized.len() + 1))?;
                if seen.insert(valid.clone()) {
                    normalized.push(valid);
                }
            }
        }
        let changed = normalized != self.settings.tracker_urls;
        self.settings.tracker_urls = normalized;
        if self.settings.tracker_urls.is_empty() {
            if let Some(job) = self.tracker_sync.take() {
                job.task.abort();
            }
            self.settings.extra_trackers.clear();
            self.settings.trackers_updated = None;
            self.tracker_sync_error = None;
            if let Some(t) = &self.torrent {
                t.set_extra_trackers(Vec::new());
            }
            return Ok(());
        }
        let has_subscription = self
            .settings
            .tracker_urls
            .iter()
            .any(|u| !ndm_torrent::trackers::is_direct_tracker(u));
        if !has_subscription {
            if let Some(job) = self.tracker_sync.take() {
                job.task.abort();
            }
            self.settings.extra_trackers = self.settings.tracker_urls.clone();
            self.settings.trackers_updated = Some(chrono::Utc::now().to_rfc3339());
            self.tracker_sync_error = None;
            if let Some(t) = &self.torrent {
                t.set_extra_trackers(self.settings.extra_trackers.clone());
            }
            return Ok(());
        }
        if changed {
            self.settings.trackers_updated = None;
            self.settings.extra_trackers = self
                .settings
                .tracker_urls
                .iter()
                .filter(|u| ndm_torrent::trackers::is_direct_tracker(u))
                .cloned()
                .collect();
            if let Some(t) = &self.torrent {
                t.set_extra_trackers(self.settings.extra_trackers.clone());
            }
        }
        if changed || force_sync || self.settings.extra_trackers.is_empty() {
            self.start_tracker_sync();
        }
        Ok(())
    }

    pub(super) fn refresh_trackers_if_due(&mut self) {
        if self.tracker_sync.is_some()
            || self
                .last_tracker_sync
                .is_some_and(|last| last.elapsed() < TRACKER_RETRY_INTERVAL)
            || !self
                .settings
                .tracker_urls
                .iter()
                .any(|u| !ndm_torrent::trackers::is_direct_tracker(u))
        {
            return;
        }
        let fresh = self
            .settings
            .trackers_updated
            .as_deref()
            .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
            .is_some_and(|stamp| {
                chrono::Utc::now().signed_duration_since(stamp) < TRACKER_REFRESH_AGE
            });
        if !fresh {
            self.start_tracker_sync();
        }
    }

    pub(super) fn start_tracker_sync(&mut self) {
        if let Some(job) = self.tracker_sync.take() {
            job.task.abort();
        }
        if self.settings.tracker_urls.is_empty() {
            self.settings.extra_trackers.clear();
            self.settings.trackers_updated = None;
            self.tracker_sync_error = None;
            return;
        }
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let urls = self.settings.tracker_urls.clone();
        let has_subscription = urls
            .iter()
            .any(|u| !ndm_torrent::trackers::is_direct_tracker(u));
        let cached = self.settings.extra_trackers.clone();
        let (tx, rx) = tokio::sync::watch::channel(None);
        let task = rt.spawn(async move {
            let mut outcome = ndm_torrent::trackers::fetch_sources(&urls, false).await;
            if has_subscription && outcome.subscriptions_succeeded == 0 {
                outcome.trackers = cached;
            }
            let _ = tx.send(Some(outcome));
        });
        self.tracker_sync_error = None;
        self.last_tracker_sync = Some(Instant::now());
        self.tracker_sync = Some(TrackerSyncJob { rx, task });
    }

    pub(super) fn poll_tracker_sync(&mut self) {
        let ready = self
            .tracker_sync
            .as_ref()
            .and_then(|job| job.rx.borrow().clone());
        let Some(outcome) = ready else {
            return;
        };
        self.tracker_sync = None;
        if outcome.subscriptions_succeeded > 0 || outcome.failed.is_empty() {
            self.settings.trackers_updated = Some(chrono::Utc::now().to_rfc3339());
        }
        self.settings.extra_trackers = outcome.trackers;
        if let Some(t) = &self.torrent {
            t.set_extra_trackers(self.settings.extra_trackers.clone());
        }
        self.tracker_sync_error = if outcome.failed.is_empty() {
            None
        } else if outcome.failed.len() == 1 {
            let (url, err) = &outcome.failed[0];
            Some(format!("Failed to fetch {url}: {err}"))
        } else {
            let (url, err) = &outcome.failed[0];
            Some(format!(
                "{} tracker source(s) failed (first: {url}: {err})",
                outcome.failed.len()
            ))
        };
        self.dirty = true;
        self.save();
    }

    pub(super) fn inspect_torrent(&mut self, source: String) -> Value {
        self.poll_tracker_sync();
        if self.previews.len() >= 8 {
            return error("Close another torrent preview first");
        }
        let slot: Preview = Default::default();
        let out = slot.clone();
        let lookup = Arc::new(ndm_torrent::Lookup::default());
        let progress = lookup.clone();
        let task = if let Some(source) = ndm_torrent::Source::magnet(&source) {
            self.refresh_trackers_if_due();
            let sync_rx = self.tracker_sync.as_ref().map(|j| j.rx.clone());
            let session = match self.session() {
                Ok(t) => t,
                Err(e) => return error(&e),
            };
            let extra = self.settings.extra_trackers.clone();
            tokio::spawn(async move {
                let result = resolve_magnet(&session, &source, &progress, &extra, sync_rx).await;
                *out.lock().unwrap() = Some(result);
            })
        } else {
            let bytes = match ndm_torrent::read_file(Path::new(&source)) {
                Ok(bytes) => bytes,
                Err(e) => return error(&e.to_string()),
            };
            tokio::spawn(async move {
                *out.lock().unwrap() =
                    Some(ndm_torrent::meta_from_bytes(&bytes).map_err(|e| e.to_string()));
            })
        };
        let token = ndm_ipc::random_token();
        self.previews.insert(
            token.clone(),
            PreviewJob {
                slot,
                lookup,
                task,
                created: Instant::now(),
            },
        );
        json!({"token":token,"pending":true})
    }
    pub(super) fn poll_torrent(&self, token: &str) -> Value {
        let Some(job) = self.previews.get(token) else {
            return error("Torrent preview expired");
        };
        let result = job.slot.lock().unwrap();
        match result.as_ref() {
            None => {
                let s = job.lookup.status();
                json!({"pending":true,"lookup":{"trackers":s.trackers,"trackers_syncing":self.tracker_sync.is_some(),"dht":s.dht,"dht_nodes":s.dht_nodes,"peers":s.peers,"connected":s.connected,"failed":s.failed}})
            }
            Some(Err(e)) => error(e),
            Some(Ok(meta)) => {
                json!({"name":meta.name,"size":meta.total(),"files":meta.files.iter().enumerate().filter(|(_,f)|!f.padding).map(|(index,f)|json!({"index":index,"path":f.path,"size":f.size})).collect::<Vec<_>>()})
            }
        }
    }
    pub(super) fn add_torrent(
        &mut self,
        token: String,
        dir: String,
        files: Vec<usize>,
        subfolder: bool,
        later: bool,
    ) -> Value {
        let Some(job) = self.previews.get(&token) else {
            return error("Read the torrent files first");
        };
        let meta = match job.slot.lock().unwrap().as_ref() {
            Some(Ok(m)) => m.clone(),
            Some(Err(e)) => return error(e),
            None => return error("Torrent metadata is not ready"),
        };
        let selected: HashSet<_> = files.iter().copied().collect();
        if files.is_empty()
            || selected.len() != files.len()
            || files
                .iter()
                .any(|n| *n >= meta.files.len() || meta.files[*n].padding)
        {
            return error("Select at least one valid file");
        }
        if self
            .records
            .iter()
            .any(|r| r.torrent.as_ref().is_some_and(|t| t.hash == meta.info_hash))
        {
            return error("This torrent is already in the download list");
        }
        let dir = PathBuf::from(dir);
        if !dir.is_absolute() {
            return error("Choose an absolute destination folder");
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return error(&e.to_string());
        }
        if !meta.existing_in_with_subfolder(&dir, subfolder).is_empty() {
            return error("Files already exist here. Choose another destination folder.");
        }
        let folder = meta.folder_in_with_subfolder(&dir, subfolder);
        let paths = meta
            .files
            .iter()
            .map(|f| ndm_torrent_file::File {
                path: f.path.clone(),
                size: f.size,
                padding: f.padding,
            })
            .collect();
        let id = self.next_id;
        self.next_id += 1;
        self.records.push(Record {
            info: DownloadInfo {
                id,
                file: meta.name.clone(),
                url: ndm_torrent::magnet_link(&meta.torrent).unwrap_or_default(),
                path: folder.to_string_lossy().into_owned(),
                status: if later { "Queued" } else { "Stopped" }.into(),
                size: Some(files.iter().map(|n| meta.files[*n].size).sum()),
                downloaded: 0,
                speed: 0,
                avg_speed: None,
                duration_secs: None,
                added: Some(chrono::Utc::now().to_rfc3339()),
                category: "Torrents".into(),
                error: None,
            },
            limit: kb_to_bps(self.settings.per_download_limit_kb),
            detail: Value::Null,
            open_torrent: false,
            active_secs: 0.0,
            torrent: Some(TorrentRecord {
                hash: meta.info_hash,
                bytes: meta.torrent,
                folder,
                files,
                paths,
                peers: meta.peers,
                seeded_secs: 0,
                uploaded: 0,
            }),
        });
        self.previews.remove(&token);
        self.dirty = true;
        if !later && let Err(e) = self.start_torrent(id) {
            self.fail(id, e);
        }
        self.save();
        json!({"id":id,"ok":true})
    }
    pub(super) fn start_torrent(&mut self, id: u64) -> Result<(), String> {
        self.poll_tracker_sync();
        let session = self.session()?;
        session.set_extra_trackers(self.settings.extra_trackers.clone());
        let record = self
            .records
            .iter()
            .find(|r| r.info.id == id)
            .ok_or("Download not found")?;
        let limit = record.limit;
        let spec = record.torrent.clone().ok_or("Not a torrent")?;
        if session.has(&spec.hash) {
            let needs_readd = !ndm_torrent::is_private(&spec.bytes)
                && !self.settings.extra_trackers.is_empty()
                && {
                    let existing: HashSet<String> =
                        session.trackers(&spec.hash).into_iter().collect();
                    self.settings.extra_trackers.iter().any(|t| {
                        ndm_torrent::trackers::allowed(t, false)
                            && url::Url::parse(t)
                                .ok()
                                .is_some_and(|u| !existing.contains(u.as_str()))
                    })
                };
            if needs_readd {
                let root = spec.folder.clone();
                let paths = spec.paths.clone();
                let prepared = prepare(&root, &paths).map_err(|e| e.to_string())?;
                let session_path = self.data.join("torrents/session");
                let reopen_root = root.clone();
                authorized_storage::authorize(
                    &session_path,
                    &spec.hash,
                    &root,
                    prepared,
                    Arc::new(move || prepare(&reopen_root, &paths)),
                );
                tokio::runtime::Handle::current()
                    .block_on(session.readd(&spec.hash, &spec.bytes, false))
                    .map_err(|e| e.to_string())?;
            }
            tokio::runtime::Handle::current()
                .block_on(session.resume(&spec.hash))
                .map_err(|e| e.to_string())?;
        } else {
            let root = spec.folder.clone();
            let paths = spec.paths.clone();
            // Open without following symlinks; hand the checked descriptors to torrent storage.
            let prepared = prepare(&root, &paths).map_err(|e| e.to_string())?;
            let session_path = self.data.join("torrents/session");
            let reopen_root = root.clone();
            authorized_storage::authorize(
                &session_path,
                &spec.hash,
                &root,
                prepared,
                Arc::new(move || prepare(&reopen_root, &paths)),
            );
            tokio::runtime::Handle::current()
                .block_on(session.add(
                    &spec.bytes,
                    &spec.folder,
                    Some(spec.files.clone()),
                    spec.peers.clone(),
                    false,
                ))
                .map_err(|e| e.to_string())?;
        }
        if limit.is_some() {
            let _ = session.set_torrent_limits(&spec.hash, limit, None);
        }
        let r = self.records.iter_mut().find(|r| r.info.id == id).unwrap();
        r.info.status = "Downloading".into();
        r.info.error = None;
        self.torrent_started.insert(id, Instant::now());
        self.dirty = true;
        Ok(())
    }
    pub(super) fn tick_torrents(&mut self) {
        let Some(session) = &self.torrent else { return };
        let seed_enabled = self.settings.torrent_seed;
        let seed_limit_secs = (self.settings.torrent_seed_minutes > 0)
            .then(|| u64::from(self.settings.torrent_seed_minutes) * 60);
        let seed_limit_ratio =
            (self.settings.torrent_seed_ratio > 0.0).then_some(self.settings.torrent_seed_ratio);
        for record in &mut self.records {
            let Some(spec) = record.torrent.as_mut() else {
                continue;
            };
            let Some(stats) = session.seen(&spec.hash) else {
                continue;
            };
            let id = record.info.id;
            match record.info.status.as_str() {
                "Downloading" => {
                    record.info.downloaded = stats.done;
                    record.info.size = Some(stats.total.max(stats.done))
                        .filter(|n| *n > 0)
                        .or(record.info.size);
                    record.info.speed = stats.down_speed.max(0.) as u64;
                    spec.uploaded = stats.uploaded;
                    match stats.phase {
                        ndm_torrent::Phase::Finished => {
                            if let Some(started) = self.torrent_started.remove(&id) {
                                record.active_secs += started.elapsed().as_secs_f64();
                            }
                            record.info.duration_secs =
                                Some(record.active_secs.round().max(1.0) as u64);
                            record.info.avg_speed = (record.info.downloaded > 0).then(|| {
                                (record.info.downloaded as f64 / record.active_secs.max(0.05))
                                    as u64
                            });
                            let ratio_reached = seed_limit_ratio.is_some_and(|r| {
                                stats.total > 0 && (stats.uploaded as f64 / stats.total as f64) >= r
                            });
                            let time_reached =
                                seed_limit_secs.is_some_and(|limit| spec.seeded_secs >= limit);
                            if seed_enabled && !ratio_reached && !time_reached {
                                record.info.status = "Seeding".into();
                                record.info.speed = stats.up_speed.max(0.) as u64;
                                self.seeding_tick.insert(id, Instant::now());
                                self.dirty = true;
                            } else {
                                record.info.status = "Complete".into();
                                record.info.speed = 0;
                                self.seeding_tick.remove(&id);
                                let _ = tokio::runtime::Handle::current()
                                    .block_on(session.pause(&spec.hash));
                                self.dirty = true;
                            }
                        }
                        ndm_torrent::Phase::Error => {
                            if let Some(started) = self.torrent_started.remove(&id) {
                                record.active_secs += started.elapsed().as_secs_f64();
                            }
                            record.info.status = "Error".into();
                            record.info.error = stats.error;
                            record.info.speed = 0;
                            self.seeding_tick.remove(&id);
                            self.dirty = true;
                        }
                        _ => {}
                    }
                }
                "Seeding" => {
                    let now = Instant::now();
                    let last = self.seeding_tick.entry(id).or_insert(now);
                    let elapsed = now.saturating_duration_since(*last);
                    let whole_secs = elapsed.as_secs();
                    if whole_secs > 0 {
                        spec.seeded_secs = spec.seeded_secs.saturating_add(whole_secs);
                        *last += Duration::from_secs(whole_secs);
                    }
                    spec.uploaded = stats.uploaded;
                    record.info.downloaded = stats.done;
                    record.info.size = Some(stats.total);
                    record.info.speed = stats.up_speed.max(0.) as u64;
                    if stats.phase == ndm_torrent::Phase::Error {
                        record.info.status = "Error".into();
                        record.info.error = stats.error;
                        record.info.speed = 0;
                        self.seeding_tick.remove(&id);
                        self.dirty = true;
                        continue;
                    }
                    let ratio_reached = seed_limit_ratio.is_some_and(|r| {
                        stats.total > 0 && (stats.uploaded as f64 / stats.total as f64) >= r
                    });
                    let time_reached =
                        seed_limit_secs.is_some_and(|limit| spec.seeded_secs >= limit);
                    if !seed_enabled
                        || ratio_reached
                        || time_reached
                        || stats.phase == ndm_torrent::Phase::Paused
                    {
                        if stats.phase != ndm_torrent::Phase::Paused {
                            let _ = tokio::runtime::Handle::current()
                                .block_on(session.pause(&spec.hash));
                        }
                        record.info.status = "Complete".into();
                        record.info.speed = 0;
                        self.seeding_tick.remove(&id);
                        self.dirty = true;
                    }
                }
                _ => {}
            }
        }
    }
}
async fn resolve_magnet(
    session: &ndm_torrent::Torrents,
    source: &ndm_torrent::Source,
    progress: &ndm_torrent::Lookup,
    cached: &[String],
    sync: Option<tokio::sync::watch::Receiver<Option<ndm_torrent::trackers::FetchOutcome>>>,
) -> Result<ndm_torrent::Meta, String> {
    use ndm_torrent::TorrentError;
    use tokio::time::{Instant, interval, timeout};

    let started = Instant::now();
    let deadline = started + METADATA_TIMEOUT;
    let lookup = async {
        let initial =
            session.metadata_with_trackers(source, METADATA_TIMEOUT, Some(progress), cached);
        let Some(mut sync) = sync else {
            return initial.await;
        };
        let fresh = {
            let refreshed = async {
                let result = sync.wait_for(|result| result.is_some()).await.ok()?;
                let trackers = &result.as_ref()?.trackers;
                (!trackers.is_empty() && trackers != cached).then(|| trackers.clone())
            };
            tokio::pin!(initial, refreshed);
            tokio::select! {
                biased;
                result = &mut initial => match result {
                    Ok(_) | Err(TorrentError::NotATorrent | TorrentError::UnsafeName(_)) => return result,
                    Err(_) => match refreshed.await {
                        Some(trackers) => trackers,
                        None => return result,
                    },
                },
                fresh = &mut refreshed => match fresh {
                    Some(trackers) => trackers,
                    None => return initial.await,
                },
            }
        };
        session
            .metadata_with_trackers(
                source,
                deadline.saturating_duration_since(Instant::now()),
                Some(progress),
                &fresh,
            )
            .await
    };
    let idle = async {
        let mut tick = interval(Duration::from_secs(1));
        let mut exhausted_since = None;
        loop {
            tick.tick().await;
            let s = progress.status();
            if s.peers > 0 && s.failed >= s.peers {
                let since = exhausted_since
                    .filter(|(peers, _)| *peers == s.peers)
                    .map_or_else(Instant::now, |(_, since)| since);
                exhausted_since = Some((s.peers, since));
                if started.elapsed() >= METADATA_IDLE_GRACE
                    && since.elapsed() >= METADATA_IDLE_TIMEOUT
                {
                    return Err(TorrentError::MetadataTimeout);
                }
            } else {
                exhausted_since = None;
            }
        }
    };
    let result = timeout(METADATA_TIMEOUT, async {
        tokio::select! {
            result = lookup => result,
            result = idle => result,
        }
    })
    .await
    .unwrap_or(Err(TorrentError::MetadataTimeout));
    result.map_err(|e| {
        let s = progress.status();
        match e {
            TorrentError::Engine(_) | TorrentError::MetadataTimeout
                if !s.dht && s.trackers == 0 =>
            {
                "No discovery sources are available. Enable DHT or sync trackers, then retry."
                    .into()
            }
            TorrentError::MetadataTimeout if s.peers == 0 => {
                "No peers were found for this magnet. Check your connection or try again later."
                    .into()
            }
            TorrentError::MetadataTimeout => {
                "No peer provided the file list. Try again later or open a .torrent file.".into()
            }
            TorrentError::NotATorrent => {
                "Enter a valid magnet link or choose a .torrent file.".into()
            }
            _ => e.to_string(),
        }
    })
}

fn prepare(root: &Path, paths: &[ndm_torrent_file::File]) -> std::io::Result<Vec<PreparedFile>> {
    checked_dir(root)?;
    paths
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.padding)
        .map(|(index, f)| {
            ndm_torrent::check_relative_path(&f.path)
                .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
            let path = root.join(&f.path);
            if let Some(parent) = path.parent() {
                checked_dir(parent)?;
            }
            if std::fs::symlink_metadata(&path)
                .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
            {
                return Err(std::io::Error::other(
                    "Torrent destination is not a regular file",
                ));
            }
            let file = safe_open(root, &f.path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if file.metadata()?.nlink() > 1 {
                    return Err(std::io::Error::other(
                        "Torrent destination is a hard-linked file",
                    ));
                }
            }
            Ok(PreparedFile { index, path, file })
        })
        .collect()
}
fn checked_dir(path: &Path) -> std::io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(std::io::Error::other(
                    "Torrent destination contains a symbolic link",
                ));
            }
            Ok(m) if !m.is_dir() => {
                return Err(std::io::Error::other(
                    "Torrent destination contains a non-directory",
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(&current)?,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn safe_open(root: &Path, path: &Path) -> std::io::Result<std::fs::File> {
    use rustix::fs::{CWD, Mode, OFlags, openat};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut dir = openat(CWD, "/", flags, Mode::empty())?;
    for part in root.components() {
        if let std::path::Component::Normal(name) = part {
            dir = openat(&dir, name, flags, Mode::empty())?;
        }
    }
    if let Some(parent) = path.parent() {
        for part in parent.components() {
            if let std::path::Component::Normal(name) = part {
                dir = openat(&dir, name, flags, Mode::empty())?;
            }
        }
    }
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("Missing torrent file name"))?;
    let file = openat(
        &dir,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?;
    Ok(file.into())
}
#[cfg(not(unix))]
fn safe_open(root: &Path, path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(root.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_empty_tracker_settings_get_the_default_once() {
        let data = tempfile::tempdir().unwrap();
        let path = data.path().join("core.json");
        let old = json!({
            "version": 1,
            "settings": {"tracker_urls": []},
            "records": [],
            "next_id": 1
        });
        std::fs::write(&path, old.to_string()).unwrap();

        let mut core = Core::open(data.path().to_path_buf()).unwrap();
        assert_eq!(core.settings.tracker_urls, [DEFAULT_TRACKER_SUBSCRIPTION]);
        core.apply_tracker_urls(Vec::new(), false).unwrap();
        core.save();

        let reopened = Core::open(data.path().to_path_buf()).unwrap();
        assert!(reopened.settings.tracker_urls.is_empty());
    }

    #[tokio::test]
    async fn pending_subscription_is_used_by_the_open_preview() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = ndm_torrent::Config::new(temp.path().to_path_buf());
        config.offline = true;
        config.dht = false;
        let session = ndm_torrent::Torrents::start(config).await.unwrap();
        let source = ndm_torrent::Source::magnet(
            "magnet:?xt=urn:btih:6384d53c65bc94efeffc2447db33d25d4a8e139b",
        )
        .unwrap();
        let progress = ndm_torrent::Lookup::default();
        let (tx, rx) = tokio::sync::watch::channel(None);
        let mut preview =
            std::pin::pin!(resolve_magnet(&session, &source, &progress, &[], Some(rx)));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut preview)
                .await
                .is_err()
        );
        tx.send(Some(ndm_torrent::trackers::FetchOutcome {
            trackers: vec!["https://test.example/announce".into()],
            ..Default::default()
        }))
        .unwrap();
        assert!(preview.await.is_err());
        assert_eq!(progress.status().trackers, 1);
        assert!(session.extra_trackers().is_empty());
        session.stop().await;
    }

    // Metadata-only magnet lookups discover peers which must be passed to the transfer.
    // No tracker or DHT can rediscover the loopback seeder in this test.
    #[test]
    fn preview_peers_survive_selection_and_start_transfer() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _enter = rt.enter();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("payload.bin");
        let expected = vec![7u8; 100_000];
        std::fs::write(&source, &expected).unwrap();
        let spawner = ndm_torrent::librqbit::spawn_utils::BlockingSpawner::new(1);
        let made = rt
            .block_on(ndm_torrent::librqbit::create_torrent(
                &source,
                ndm_torrent::librqbit::CreateTorrentOptions {
                    name: Some("payload.bin"),
                    trackers: Vec::new(),
                    piece_length: Some(16384),
                },
                &spawner,
            ))
            .unwrap();
        let bytes = made.as_bytes().unwrap().to_vec();
        let offline = |path| {
            let mut cfg = ndm_torrent::Config::new(path);
            cfg.offline = true;
            cfg.dht = false;
            cfg.upnp = false;
            cfg.lsd = false;
            cfg
        };
        let mut seed_config = offline(root.join("seed"));
        seed_config.require_authorization = false;
        let seeder = rt
            .block_on(ndm_torrent::Torrents::start(seed_config))
            .unwrap();
        let hash = rt
            .block_on(seeder.add(&bytes, &root, None, Vec::new(), false))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while seeder.stats(&hash).unwrap().phase != ndm_torrent::Phase::Finished {
            assert!(Instant::now() < deadline, "seeder did not finish checking");
            std::thread::sleep(Duration::from_millis(30));
        }
        let peer = seeder.listen_addr().unwrap();
        let mut meta = ndm_torrent::meta_from_bytes(&bytes).unwrap();
        meta.peers = vec![peer];
        let data = root.join("data");
        let mut core = Core::open(data.clone()).unwrap();
        core.torrent = Some(Arc::new(
            rt.block_on(ndm_torrent::Torrents::start(offline(data.join("torrents"))))
                .unwrap(),
        ));
        core.previews.insert(
            "preview".into(),
            PreviewJob {
                lookup: Default::default(),
                slot: Arc::new(Mutex::new(Some(Ok(meta)))),
                task: tokio::spawn(async {}),
                created: Instant::now(),
            },
        );
        let result = core.add_torrent(
            "preview".into(),
            root.join("downloads").to_string_lossy().into(),
            vec![0],
            false,
            true,
        );
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(core.records[0].torrent.as_ref().unwrap().peers, vec![peer]);
        // Persistence must retain the peers for a queued task as well.
        let saved = std::fs::read_to_string(data.join("core.json")).unwrap();
        assert!(saved.contains(&peer.to_string()));
        core.start_torrent(result["id"].as_u64().unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            core.tick();
            if core.records[0].info.status == "Complete" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "transfer stalled: {:?}",
                core.records[0].info
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            std::fs::read(root.join("downloads/payload.bin")).unwrap(),
            expected
        );
        core.shutdown();
        rt.block_on(seeder.stop());
    }

    #[test]
    fn torrent_seeding_enters_seeding_and_stops_when_limit_or_toggle_triggers() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _enter = rt.enter();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("payload.bin");
        let expected = vec![9u8; 65_536];
        std::fs::write(&source, &expected).unwrap();
        let spawner = ndm_torrent::librqbit::spawn_utils::BlockingSpawner::new(1);
        let made = rt
            .block_on(ndm_torrent::librqbit::create_torrent(
                &source,
                ndm_torrent::librqbit::CreateTorrentOptions {
                    name: Some("payload.bin"),
                    trackers: Vec::new(),
                    piece_length: Some(16384),
                },
                &spawner,
            ))
            .unwrap();
        let bytes = made.as_bytes().unwrap().to_vec();
        let offline = |path| {
            let mut cfg = ndm_torrent::Config::new(path);
            cfg.offline = true;
            cfg.dht = false;
            cfg.upnp = false;
            cfg.lsd = false;
            cfg
        };
        let mut seed_config = offline(root.join("seed"));
        seed_config.require_authorization = false;
        let seeder = rt
            .block_on(ndm_torrent::Torrents::start(seed_config))
            .unwrap();
        let hash = rt
            .block_on(seeder.add(&bytes, &root, None, Vec::new(), false))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while seeder.stats(&hash).unwrap().phase != ndm_torrent::Phase::Finished {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(30));
        }
        let peer = seeder.listen_addr().unwrap();
        let mut meta = ndm_torrent::meta_from_bytes(&bytes).unwrap();
        meta.peers = vec![peer];

        let data = root.join("data");
        let mut core = Core::open(data.clone()).unwrap();
        core.settings.torrent_seed = true;
        core.settings.torrent_seed_minutes = 10;
        core.settings.torrent_seed_ratio = 2.0;
        core.settings.global_download_limit_kb = 1024;
        core.settings.torrent_upload_limit_kb = 256;
        core.global_limiter.set(kb_to_bps(1024));
        let session = Arc::new(
            rt.block_on(ndm_torrent::Torrents::start(offline(data.join("torrents"))))
                .unwrap(),
        );
        session.set_download_limit(kb_to_bps(1024));
        session.set_upload_limit(kb_to_bps(256));
        assert_eq!(core.global_limiter.get(), Some(1024 * 1024));
        assert_eq!(session.download_limit(), Some(1024 * 1024));
        assert_eq!(session.upload_limit(), Some(256 * 1024));
        core.torrent = Some(session);

        core.previews.insert(
            "preview".into(),
            PreviewJob {
                lookup: Default::default(),
                slot: Arc::new(Mutex::new(Some(Ok(meta)))),
                task: tokio::spawn(async {}),
                created: Instant::now(),
            },
        );
        let result = core.add_torrent(
            "preview".into(),
            root.join("downloads").to_string_lossy().into(),
            vec![0],
            false,
            false,
        );
        assert_eq!(result["ok"], true, "{result}");

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            core.tick();
            if core.records[0].info.status == "Seeding" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "did not enter Seeding: {:?}",
                core.records[0].info
            );
            std::thread::sleep(Duration::from_millis(50));
        }

        // Exceeding seeding time limit transitions Seeding -> Complete and pauses.
        core.records[0].torrent.as_mut().unwrap().seeded_secs = 600;
        core.tick();
        assert_eq!(core.records[0].info.status, "Complete");
        assert_eq!(core.records[0].info.speed, 0);

        core.shutdown();
        rt.block_on(seeder.stop());
    }
}
