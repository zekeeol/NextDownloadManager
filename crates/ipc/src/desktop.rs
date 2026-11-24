//! Versioned native desktop contract. This module contains data only, with no UI types.
use crate::Command;
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesktopRequest {
    List,
    Browser {
        command: Box<Command>,
    },
    Events,
    Add {
        command: Box<Command>,
    },
    Action {
        action: String,
        ids: Vec<u64>,
    },
    Detail {
        id: u64,
    },
    Limit {
        id: u64,
        bytes: Option<u64>,
    },
    Queue {
        start: bool,
    },
    Settings,
    SaveSettings {
        connections: u32,
        folder: String,
        #[serde(default)]
        tracker_urls: Option<Vec<String>>,
        #[serde(default)]
        max_concurrent: Option<u32>,
        #[serde(default)]
        global_download_limit_kb: Option<u64>,
        #[serde(default)]
        per_download_limit_kb: Option<u64>,
        #[serde(default)]
        torrent_upload_limit_kb: Option<u64>,
        #[serde(default)]
        torrent_seed: Option<bool>,
        #[serde(default)]
        torrent_seed_minutes: Option<u32>,
        #[serde(default)]
        torrent_seed_ratio: Option<f64>,
        #[serde(default)]
        torrent_peer_limit: Option<u32>,
        #[serde(default)]
        torrent_dht: Option<bool>,
        #[serde(default)]
        torrent_utp: Option<bool>,
    },
    SyncTrackers {
        #[serde(default)]
        tracker_urls: Option<Vec<String>>,
    },
    TorrentInspect {
        source: String,
    },
    TorrentPoll {
        token: String,
    },
    TorrentCancel {
        token: String,
    },
    TorrentAdd {
        token: String,
        dir: String,
        files: Vec<usize>,
        subfolder: bool,
        later: bool,
    },
}
