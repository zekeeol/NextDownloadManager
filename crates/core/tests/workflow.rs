//! Black-box core tests: no GPUI, no IPC server, real loopback Range downloads.
use ndm_core::Core;
use ndm_ipc::Command;
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn ask(core: &mut Core, request: Value) -> Value {
    let reply = core.handle(Command {
        desktop: Some(request),
        ..Default::default()
    });
    assert!(reply.get("error").is_none(), "{reply}");
    reply
}
fn wait(core: &mut Core, predicate: impl Fn(&Value) -> bool) {
    let start = Instant::now();
    loop {
        core.tick();
        let list = ask(core, json!({"op":"list"}));
        if predicate(&list) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Timed out: {list}"
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}
#[test]
fn download_pause_restart_resume_queue_and_verify() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _enter = rt.enter();
    let server = rt
        .block_on(ndm_labserver::LabServer::start(ndm_labserver::Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        }))
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let downloads = temp.path().join("files");
    let mut core = Core::open(data.clone()).unwrap();
    let source = format!("http://{}/gen/4M/test.bin?rate=32K", server.addr());
    let added = ask(
        &mut core,
        json!({"op":"add","command":{"url":source,"dir":downloads,"silent":true}}),
    );
    let id = added["id"].as_u64().unwrap();
    wait(&mut core, |v| {
        v["downloads"][0]["downloaded"].as_u64().unwrap_or(0) > 100_000
    });
    let detail = ask(&mut core, json!({"op":"detail","id":id}));
    assert!(detail["connections"].as_array().unwrap().len() > 1);
    ask(
        &mut core,
        json!({"op":"action","action":"pause","ids":[id]}),
    );
    core.shutdown();
    drop(core);
    let mut core = Core::open(data).unwrap();
    let list = ask(&mut core, json!({"op":"list"}));
    assert_eq!(list["downloads"][0]["status"], "Stopped");
    assert!(list["downloads"][0]["downloaded"].as_u64().unwrap() > 0);
    ask(&mut core, json!({"op":"limit","id":id,"bytes":8_388_608}));
    ask(
        &mut core,
        json!({"op":"action","action":"start","ids":[id]}),
    );
    wait(&mut core, |v| v["downloads"][0]["status"] == "Complete");
    let list = ask(&mut core, json!({"op":"list"}));
    let path = Path::new(list["downloads"][0]["path"].as_str().unwrap());
    let actual = std::fs::read(path).unwrap();
    // Same deterministic content as a separate, unthrottled request to the lab.
    let expected = rt.block_on(async {
        reqwest::get(source.replace("?rate=32K", ""))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
    });
    assert_eq!(actual.as_slice(), expected.as_ref());
    let queued = ask(
        &mut core,
        json!({"op":"add","command":{"url":format!("http://{}/gen/128K/queued.bin",server.addr()),"dir":downloads,"later":true}}),
    );
    let qid = queued["id"].as_u64().unwrap();
    assert_eq!(
        ask(&mut core, json!({"op":"list"}))["downloads"][1]["status"],
        "Queued"
    );
    ask(&mut core, json!({"op":"queue","start":true}));
    wait(&mut core, |v| v["downloads"][1]["status"] == "Complete");
    ask(
        &mut core,
        json!({"op":"action","action":"remove","ids":[qid]}),
    );
    assert!(downloads.join("queued.bin").exists());
    core.shutdown();
    drop(server);
}
#[test]
fn corrupt_state_is_preserved_and_invalid_commands_fail() {
    let temp = tempfile::tempdir().unwrap();
    let bad = temp.path().join("core.json");
    std::fs::write(&bad, "broken").unwrap();
    assert!(Core::open(temp.path().to_owned()).is_err());
    assert_eq!(std::fs::read_to_string(bad).unwrap(), "broken");
    let temp = tempfile::tempdir().unwrap();
    let mut core = Core::open(temp.path().to_owned()).unwrap();
    assert!(
        core.handle(Command {
            desktop: Some(json!({"op":"invented"})),
            ..Default::default()
        })
        .get("error")
        .is_some()
    );
    assert!(
        core.handle(Command {
            desktop: Some(json!({"op":"add","command":{"url":"file:///etc/passwd"}})),
            ..Default::default()
        })
        .get("error")
        .is_some()
    );
}
#[test]
fn torrent_file_selection_is_validated_and_persisted() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _enter = rt.enter();
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("one.txt"), b"first file").unwrap();
    std::fs::write(source.join("two.txt"), b"second file").unwrap();
    let spawner = ndm_torrent::librqbit::spawn_utils::BlockingSpawner::new(1);
    let made = rt
        .block_on(ndm_torrent::librqbit::create_torrent(
            &source,
            ndm_torrent::librqbit::CreateTorrentOptions {
                name: Some("selection"),
                trackers: Vec::new(),
                piece_length: Some(16384),
            },
            &spawner,
        ))
        .unwrap();
    let torrent = temp.path().join("sample.torrent");
    std::fs::write(&torrent, made.as_bytes().unwrap()).unwrap();
    let data = temp.path().join("data");
    let mut core = Core::open(data.clone()).unwrap();
    let preview = ask(&mut core, json!({"op":"torrent_inspect","source":torrent}));
    let token = preview["token"].as_str().unwrap();
    let start = Instant::now();
    loop {
        let polled = ask(&mut core, json!({"op":"torrent_poll","token":token}));
        if polled["pending"] != true {
            assert_eq!(polled["files"].as_array().unwrap().len(), 2);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    let bad=core.handle(Command{desktop:Some(json!({"op":"torrent_add","token":token,"dir":temp.path().join("output"),"files":[99],"subfolder":true,"later":true})),..Default::default()});
    assert!(bad.get("error").is_some());
    ask(
        &mut core,
        json!({"op":"torrent_add","token":token,"dir":temp.path().join("output"),"files":[0],"subfolder":true,"later":true}),
    );
    let list = ask(&mut core, json!({"op":"list"}));
    assert_eq!(list["downloads"][0]["category"], "Torrents");
    assert_eq!(list["downloads"][0]["status"], "Queued");
    core.shutdown();
    drop(core);
    let mut reopened = Core::open(data).unwrap();
    assert_eq!(
        ask(&mut reopened, json!({"op":"list"}))["downloads"][0]["status"],
        "Queued"
    );
    reopened.shutdown();
}

#[test]
fn browser_commands_are_confirmed_and_events_reach_the_ui() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _enter = rt.enter();
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mut core = Core::open(data.clone()).unwrap();
    let browser = |command| json!({"op":"browser","command":command});
    let bad = core.handle(Command {
        desktop: Some(browser(json!({"url":"file:///etc/passwd"}))),
        ..Default::default()
    });
    assert!(bad.get("error").is_some());
    assert!(
        ask(&mut core, json!({"op":"list"}))["downloads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let magnet = "magnet:?xt=urn:btih:5b64c35d01e23f3e50bb4507bba3f2fcae1187b2";
    ask(&mut core, browser(json!({"url":magnet})));
    let events = ask(&mut core, json!({"op":"events"}));
    assert_eq!(events["events"][0]["kind"], "torrent");
    assert_eq!(events["events"][0]["source"], magnet);
    assert!(
        ask(&mut core, json!({"op":"events"}))["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    ask(&mut core, browser(json!({"show":"options:general"})));
    assert_eq!(
        ask(&mut core, json!({"op":"events"}))["events"][0]["kind"],
        "settings"
    );
    ask(&mut core, browser(json!({"exclude_site":"example.com"})));
    let read_rules = || {
        serde_json::from_slice::<ndm_ipc::capture::CaptureRules>(
            &std::fs::read(data.join("browser-rules.json")).unwrap(),
        )
        .unwrap()
    };
    assert!(read_rules().site_excluded("https://example.com/file.zip"));
    ask(&mut core, browser(json!({"include_site":"example.com"})));
    assert!(!read_rules().site_excluded("https://example.com/file.zip"));
    let added = ask(
        &mut core,
        browser(
            json!({"links":["https://a.test/first.zip","https://b.test/second.zip"],"dir":temp.path().join("files"),"later":true,"cookies":"secret=private","referer":"https://a.test/"}),
        ),
    );
    assert_eq!(added["ids"].as_array().unwrap().len(), 2);
    let saved = std::fs::read_to_string(data.join("core.json")).unwrap();
    assert!(!saved.contains("secret=private"));
    let invalid_stream = core.handle(Command {
        desktop: Some(browser(json!({"video_page":"https://example.com/watch"}))),
        ..Default::default()
    });
    assert!(invalid_stream.get("error").is_some());
    core.shutdown();
}

#[test]
fn extra_tracker_subscriptions_sync_persist_and_apply_to_torrents() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _enter = rt.enter();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hits_srv = hits.clone();
    rt.spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let hits_srv = hits_srv.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                hits_srv.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let body = if req.contains("GET /list1.txt") {
                    "http://extra-a.example/announce\nhttp://127.0.0.1/local\n"
                } else if req.contains("GET /list2.txt") {
                    "udp://extra-b.example:6969/announce\nhttp://extra-a.example/announce\n"
                } else {
                    ""
                };
                let resp = if body.is_empty() {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_owned()
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                };
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });

    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let downloads = temp.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut core = Core::open(data.clone()).unwrap();

    // Invalid tracker URL is rejected without corrupting settings.
    let bad = core.handle(Command {
        desktop: Some(json!({
            "op": "save_settings",
            "connections": 8,
            "folder": downloads,
            "tracker_urls": ["file:///etc/passwd"]
        })),
        ..Default::default()
    });
    assert!(bad.get("error").is_some(), "{bad}");

    let urls = vec![
        format!("http://{addr}/list1.txt"),
        format!("http://{addr}/list2.txt"),
        "https://direct.example/announce".to_owned(),
    ];
    let saved = ask(
        &mut core,
        json!({
            "op": "save_settings",
            "connections": 8,
            "folder": downloads,
            "tracker_urls": urls,
        }),
    );
    assert_eq!(saved["trackers_syncing"], true);

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        core.tick();
        let st = ask(&mut core, json!({"op":"settings"}));
        if st["trackers_syncing"] == false {
            assert_eq!(st["extra_trackers_count"], 3, "{st}");
            assert!(st["trackers_error"].is_null(), "{st}");
            break;
        }
        assert!(Instant::now() < deadline, "tracker sync timed out: {st}");
        std::thread::sleep(Duration::from_millis(20));
    }

    // Persistence across restart.
    core.shutdown();
    drop(core);
    let mut reopened = Core::open(data).unwrap();
    let st = ask(&mut reopened, json!({"op":"settings"}));
    assert_eq!(st["extra_trackers_count"], 3, "{st}");
    assert_eq!(st["tracker_urls"].as_array().unwrap().len(), 3);

    // Explicit sync_trackers triggers a fresh fetch.
    let before = hits.load(std::sync::atomic::Ordering::Relaxed);
    ask(&mut reopened, json!({"op":"sync_trackers"}));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        reopened.tick();
        let st = ask(&mut reopened, json!({"op":"settings"}));
        if st["trackers_syncing"] == false {
            assert!(
                hits.load(std::sync::atomic::Ordering::Relaxed) > before,
                "expected new HTTP hits"
            );
            assert_eq!(st["extra_trackers_count"], 3, "{st}");
            break;
        }
        assert!(Instant::now() < deadline, "re-sync timed out: {st}");
        std::thread::sleep(Duration::from_millis(20));
    }

    reopened.shutdown();
}

#[test]
fn settings_speed_limits_concurrency_and_seeding_persist_and_validate() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let downloads = temp.path().join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut core = Core::open(data.clone()).unwrap();

    for bad_payload in [
        json!({"op":"save_settings","connections":8,"folder":downloads,"max_concurrent":0}),
        json!({"op":"save_settings","connections":8,"folder":downloads,"max_concurrent":99}),
        json!({"op":"save_settings","connections":8,"folder":downloads,"torrent_seed_ratio":-0.5}),
        json!({"op":"save_settings","connections":8,"folder":downloads,"torrent_peer_limit":5}),
        json!({"op":"save_settings","connections":8,"folder":downloads,"torrent_peer_limit":999}),
    ] {
        let reply = core.handle(Command {
            desktop: Some(bad_payload),
            ..Default::default()
        });
        assert!(reply.get("error").is_some(), "{reply}");
    }

    ask(
        &mut core,
        json!({
            "op": "save_settings",
            "connections": 12,
            "folder": downloads,
            "max_concurrent": 6,
            "global_download_limit_kb": 4096,
            "per_download_limit_kb": 512,
            "torrent_upload_limit_kb": 256,
            "torrent_seed": true,
            "torrent_seed_minutes": 45,
            "torrent_seed_ratio": 2.5,
            "torrent_peer_limit": 250,
            "torrent_dht": false,
            "torrent_utp": false,
        }),
    );

    let added = ask(
        &mut core,
        json!({"op":"add","command":{"url":"http://127.0.0.1:9/test.bin","dir":downloads,"later":true}}),
    );
    let detail = ask(&mut core, json!({"op":"detail","id":added["id"]}));
    assert_eq!(detail["limit"], 512 * 1024);

    core.shutdown();
    drop(core);

    let mut reopened = Core::open(data).unwrap();
    let st = ask(&mut reopened, json!({"op":"settings"}));
    assert_eq!(st["connections"], 12);
    assert_eq!(st["max_concurrent"], 6);
    assert_eq!(st["global_download_limit_kb"], 4096);
    assert_eq!(st["per_download_limit_kb"], 512);
    assert_eq!(st["torrent_upload_limit_kb"], 256);
    assert_eq!(st["torrent_seed"], true);
    assert_eq!(st["torrent_seed_minutes"], 45);
    assert_eq!(st["torrent_seed_ratio"], 2.5);
    assert_eq!(st["torrent_peer_limit"], 250);
    assert_eq!(st["torrent_dht"], false);
    assert_eq!(st["torrent_utp"], false);
    reopened.shutdown();
}

#[test]
fn completion_records_stats_and_backfills_unknown_size() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _enter = rt.enter();
    let server = rt
        .block_on(ndm_labserver::LabServer::start(ndm_labserver::Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        }))
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let downloads = temp.path().join("files");
    std::fs::create_dir_all(&downloads).unwrap();
    let mut core = Core::open(data.clone()).unwrap();
    let source = format!("http://{}/gen/256K/archive.zip", server.addr());
    ask(
        &mut core,
        json!({"op":"add","command":{"url":source,"dir":downloads,"silent":true}}),
    );
    wait(&mut core, |v| v["downloads"][0]["status"] == "Complete");
    let list = ask(&mut core, json!({"op":"list"}));
    assert_eq!(list["downloads"][0]["size"], 256 * 1024);
    assert!(list["downloads"][0]["avg_speed"].as_u64().unwrap_or(0) > 0);
    assert!(list["downloads"][0]["duration_secs"].as_u64().unwrap_or(0) >= 1);
    core.shutdown();
    drop(core);

    // Simulate legacy core.json record missing `size` on a completed unknown-length download.
    let legacy_data = temp.path().join("legacy-data");
    std::fs::create_dir_all(&legacy_data).unwrap();
    let saved = json!({
        "version": 1,
        "settings": {},
        "records": [{
            "info": {
                "id": 1,
                "file": "legacy.zip",
                "url": "https://example.com/legacy.zip",
                "path": downloads.join("legacy.zip"),
                "status": "Complete",
                "size": null,
                "downloaded": 12345,
                "speed": 0,
                "added": null,
                "category": "Compressed"
            },
            "limit": null
        }],
        "next_id": 2
    });
    std::fs::write(
        legacy_data.join("core.json"),
        serde_json::to_vec_pretty(&saved).unwrap(),
    )
    .unwrap();
    let mut reopened = Core::open(legacy_data).unwrap();
    let legacy_list = ask(&mut reopened, json!({"op":"list"}));
    assert_eq!(legacy_list["downloads"][0]["size"], 12345);
    reopened.shutdown();
}
