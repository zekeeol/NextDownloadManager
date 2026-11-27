//! Keep IPC, process startup and disk waits off the native window thread.
use serde_json::{Value, json};
use std::{
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Message {
    pub tag: String,
    pub value: Value,
}
enum Job {
    Request(String, Value),
    Stop,
}
pub struct Bridge {
    tx: mpsc::Sender<Job>,
    pub rx: mpsc::Receiver<Message>,
    worker: Option<std::thread::JoinHandle<()>>,
}

pub fn backend() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name(format!(
            "nextdownloadmanager-backend{}",
            std::env::consts::EXE_SUFFIX
        ))
}

pub fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}

fn request(value: Value) -> Value {
    service_reply(ndm_ipc::request_checked(&ndm_ipc::Command {
        desktop: Some(value),
        ..Default::default()
    }))
}

fn service_reply(reply: Result<String, ndm_ipc::SendError>) -> Value {
    let message = match reply {
        Ok(text) => match text.trim() {
            "timeout" => "The download service is busy. Waiting for it to respond.",
            "gone" | "" => "The download service disconnected. Trying to reconnect.",
            _ => {
                return serde_json::from_str(&text).unwrap_or_else(|_| {
                    json!({
                        "error": "The download service returned an invalid reply.",
                        "service_error": true,
                    })
                });
            }
        },
        Err(ndm_ipc::SendError::NotRunning(_)) => {
            "Cannot connect to the download service. Trying to reconnect."
        }
        Err(ndm_ipc::SendError::Refused(_)) => {
            "The download service did not respond. Trying to reconnect."
        }
    };
    json!({"error":message,"service_error":true})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_service_reply_is_received_and_the_next_poll_recovers() {
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
        };

        let data = std::env::temp_dir().join(format!("ndm-bridge-{}", ndm_ipc::random_token()));
        ndm_ipc::private_dir(&data).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        ndm_ipc::private_write(
            &data.join("ipc.json"),
            json!({
                "port": listener.local_addr().unwrap().port(),
                "token": "test",
                "pid": std::process::id(),
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        ndm_ipc::set_data_dir(data.clone());
        let server = std::thread::spawn(move || {
            for reply in ["timeout\n", "{\"downloads\":[]}\n"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&line).unwrap()["desktop"]["op"],
                    "list"
                );
                if reply == "timeout\n" {
                    std::thread::sleep(Duration::from_millis(2100));
                }
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        let busy = request(json!({"op":"list"}));
        assert_eq!(busy["service_error"], true);
        assert_eq!(
            busy["error"],
            "The download service is busy. Waiting for it to respond."
        );
        assert_eq!(request(json!({"op":"list"})), json!({"downloads":[]}));
        server.join().unwrap();
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn service_protocol_failures_are_distinguished_from_download_errors() {
        for reply in ["timeout", "gone", "", "invalid json"] {
            let value = service_reply(Ok(reply.into()));
            assert_eq!(value["service_error"], true, "{reply}");
            assert!(value["error"].is_string());
        }
        let refused = service_reply(Err(ndm_ipc::SendError::Refused(
            "Windows localized socket error (os error 10060)".into(),
        )));
        assert_eq!(refused["service_error"], true);
        assert!(!refused["error"].as_str().unwrap().contains("10060"));
        let download = json!({"error":"The destination folder is not writable."});
        assert_eq!(service_reply(Ok(download.to_string())), download);
        let recovered = json!({"downloads":[]});
        assert_eq!(service_reply(Ok(recovered.to_string())), recovered);
    }
}
impl Bridge {
    pub fn start() -> Self {
        let (tx, jobs) = mpsc::channel();
        let (updates, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut child: Option<Child> = None;
            let first = request(json!({"op":"list"}));
            if first.get("error").is_some() {
                match quiet_command(backend())
                    .args(["--headless", "--desktop-service"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                {
                    Ok(c) => child = Some(c),
                    Err(e) => {
                        let _ = updates.send(Message { tag:"error".into(), value:json!({"error":format!("Cannot start download service: {e}. Build ndm-backend alongside ndm-app.")}) });
                        return;
                    }
                }
                let start = Instant::now();
                loop {
                    let list = request(json!({"op":"list"}));
                    if list.get("error").is_none() {
                        break;
                    }
                    if start.elapsed() > Duration::from_secs(30) {
                        let _ = updates.send(Message {
                            tag: "error".into(),
                            value: list,
                        });
                        if let Some(c) = &mut child {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(150));
                }
            }
            let _ = updates.send(Message {
                tag: "settings".into(),
                value: request(json!({"op":"settings"})),
            });
            loop {
                match jobs.recv_timeout(Duration::from_millis(350)) {
                    Ok(Job::Request(tag, value)) => {
                        if tag == "browser-register" {
                            let host = backend().with_file_name(format!(
                                "nextdownloadmanager-nmhost{}",
                                std::env::consts::EXE_SUFFIX
                            ));
                            let value = match quiet_command(host).arg("--register").output() {
                                Ok(out) if out.status.success() => json!({"ok":true}),
                                Ok(out) => {
                                    json!({"error":String::from_utf8_lossy(&out.stderr).trim()})
                                }
                                Err(e) => json!({"error":e.to_string()}),
                            };
                            let _ = updates.send(Message { tag, value });
                            continue;
                        }
                        let _ = updates.send(Message {
                            tag,
                            value: request(value),
                        });
                        let _ = updates.send(Message {
                            tag: "list".into(),
                            value: request(json!({"op":"list"})),
                        });
                    }
                    Ok(Job::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let _ = updates.send(Message {
                            tag: "events".into(),
                            value: request(json!({"op":"events"})),
                        });
                        let _ = updates.send(Message {
                            tag: "list".into(),
                            value: request(json!({"op":"list"})),
                        });
                    }
                }
            }
            if let Some(mut c) = child {
                let _ = ndm_ipc::send(&ndm_ipc::Command {
                    exit: true,
                    ..Default::default()
                });
                let start = Instant::now();
                while c.try_wait().ok().flatten().is_none()
                    && start.elapsed() < Duration::from_secs(8)
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                if c.try_wait().ok().flatten().is_none() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        });
        Self {
            tx,
            rx,
            worker: Some(worker),
        }
    }
    pub fn send(&self, tag: &str, value: Value) {
        let _ = self.tx.send(Job::Request(tag.into(), value));
    }
    pub fn stop(&mut self) {
        let _ = self.tx.send(Job::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}
