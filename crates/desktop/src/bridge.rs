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
    match ndm_ipc::request(&ndm_ipc::Command {
        desktop: Some(value),
        ..Default::default()
    }) {
        Ok(text) => serde_json::from_str(&text)
            .unwrap_or_else(|_| json!({"error":"The download service returned an invalid reply"})),
        Err(e) => json!({"error":e.to_string()}),
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
