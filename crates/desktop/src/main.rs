//! GPUI is the default desktop. Existing CLI and headless entry points use the download service.
#![cfg_attr(windows, windows_subsystem = "windows")]

extern crate gpui_kit as gpui;
mod app_icon;
mod bridge;
mod shell;
mod theme;
use gpui_kit::component::TitleBar;
use gpui_kit::*;
gpui_kit::actions!(ndm, [Quit]);

#[cfg(windows)]
fn attach_parent_console() -> bool {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetConsoleCP};

    // SAFETY: These APIs take no pointers. The flag selects the existing parent console.
    #[allow(unsafe_code)]
    unsafe {
        GetConsoleCP() != 0 || AttachConsole(ATTACH_PARENT_PROCESS) != 0
    }
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|a| {
        a == "--headless"
            || a == "--cli"
            || a.to_string_lossy().starts_with("--cli=")
            || a == "--forget-sign-ins"
            || a == "--crash-monitor"
    }) {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let error = std::process::Command::new(bridge::backend())
                .args(args)
                .exec();
            eprintln!("{error}");
            std::process::exit(1);
        }
        #[cfg(not(unix))]
        {
            #[cfg(windows)]
            let mut command = if attach_parent_console() {
                std::process::Command::new(bridge::backend())
            } else {
                bridge::quiet_command(bridge::backend())
            };
            #[cfg(not(windows))]
            let mut command = std::process::Command::new(bridge::backend());
            let code = command
                .args(args)
                .status()
                .ok()
                .and_then(|s| s.code())
                .unwrap_or(1);
            std::process::exit(code);
        }
    }
    // One native window per data directory; browser handoffs reuse the existing UI.
    let data = ndm_ipc::data_dir().expect("No application data directory");
    ndm_ipc::private_dir(&data).expect("Cannot create application data directory");
    let desktop_lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data.join("desktop.lock"))
        .expect("Cannot open desktop lock");
    if desktop_lock.try_lock().is_err() {
        if !args.iter().any(|a| a == ndm_ipc::FROM_HOST) {
            let _ = ndm_ipc::request(&ndm_ipc::Command {
                desktop: Some(serde_json::json!({"op":"browser","command":{}})),
                ..Default::default()
            });
        }
        return;
    }
    application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            init(cx);
            app_icon::install();
            theme::install(cx);
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus(vec![Menu {
                name: "Next Download Manager".into(),
                disabled: false,
                items: vec![MenuItem::action("Quit Next Download Manager", Quit)],
            }]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let initial = std::env::var("NDM_WINDOW_SIZE")
                .ok()
                .and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((
                        w.parse::<f32>().ok()?.clamp(900., 2000.),
                        h.parse::<f32>().ok()?.clamp(600., 1400.),
                    ))
                })
                .unwrap_or((1280., 800.));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(initial.0), px(initial.1)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(600.))),
                window_background: WindowBackgroundAppearance::Opaque,
                titlebar: Some(TitlebarOptions {
                    title: Some("Next Download Manager".into()),
                    traffic_light_position: Some(point(px(14.), px(14.))),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            };
            let (_, view) = open_window(options, cx, |window, cx| {
                cx.new(|cx| shell::Shell::new(window, cx))
            })
            .expect("Cannot open Next Download Manager");
            cx.on_app_quit(move |cx| {
                view.update(cx, |view, _| view.bridge.stop());
                async {}
            })
            .detach();
            cx.activate(true);
        });
}
