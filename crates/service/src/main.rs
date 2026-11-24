//! Transport host only. All state and behavior live in ndm-core.
use std::{path::PathBuf, sync::mpsc, time::Duration};
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    if let Some(cli) = ndm_cli::launched(args) {
        return ndm_cli::main(cli.args);
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    for pair in args.windows(2) {
        if pair[0] == "--data" {
            ndm_ipc::set_data_dir(PathBuf::from(&pair[1]));
        }
    }
    let data = ndm_ipc::data_dir().expect("No application data folder");
    if let Err(e) = ndm_ipc::private_dir(&data) {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    let Some(_lock) = ndm_ipc::instance_lock() else {
        eprintln!("A download service is already using this data folder");
        return std::process::ExitCode::FAILURE;
    };
    let rt = tokio::runtime::Runtime::new().expect("Cannot start runtime");
    let _enter = rt.enter();
    let mut core = match ndm_core::Core::open(data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let (tx, rx) = mpsc::channel();
    if let Err(e) = ndm_ipc::serve(tx, || {}) {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    let (stop, signal) = mpsc::channel();
    rt.spawn(async move {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Cannot listen for termination");
            tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        let _ = stop.send(());
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(incoming) => {
                if incoming.cmd.exit {
                    if let Some(reply) = incoming.reply {
                        let _ = reply.send("{\"ok\":true}".into());
                    }
                    break;
                }
                let answer = core.handle(incoming.cmd);
                if let Some(reply) = incoming.reply {
                    let _ = reply.send(answer.to_string());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        core.tick();
        if signal.try_recv().is_ok() {
            break;
        }
    }
    core.shutdown();
    std::process::ExitCode::SUCCESS
}
