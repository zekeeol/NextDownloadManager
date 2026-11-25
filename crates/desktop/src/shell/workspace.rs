use super::*;
use gpui_kit::component::TitleBar;

const INSPECTOR_WIDTH: f32 = 272.;
const WORKSPACE_GUTTER: f32 = 8.;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Filter {
    All,
    Unfinished,
    Finished,
    Category(String),
}

impl Filter {
    pub(super) fn matches(&self, download: &DownloadInfo) -> bool {
        match self {
            Self::All => true,
            Self::Unfinished => download.status != "Complete",
            Self::Finished => download.status == "Complete",
            Self::Category(key) => {
                download.category == *key || (key == "General" && download.category.is_empty())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    List,
    Grid,
}

struct ListColumns {
    speed: bool,
    time: bool,
}

impl ListColumns {
    const SIZE_WIDTH: f32 = 96.;
    const STATUS_WIDTH: f32 = 140.;
    const SPEED_WIDTH: f32 = 106.;
    const TIME_WIDTH: f32 = 92.;
    const NAME_INSET: f32 = 40.;

    fn for_width(width: f32) -> Self {
        Self {
            speed: width >= 570.,
            time: width >= 730.,
        }
    }
}

pub(super) fn matches_search(download: &DownloadInfo, query: &str) -> bool {
    query.is_empty()
        || download.file.to_lowercase().contains(query)
        || download.url.to_lowercase().contains(query)
}

fn title(value: impl Into<SharedString>) -> Div {
    text(value).font_weight(FontWeight::SEMIBOLD)
}

fn rule(p: &t::Palette) -> Div {
    div()
        .w(px(1.))
        .h(px(18.))
        .mx(px(3.))
        .flex_shrink_0()
        .bg(p.border)
}

fn tool(id: &'static str, glyph: IconName, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .icon(glyph)
        .compact()
        .w(px(28.))
        .h(px(28.))
        .rounded(px(t::CONTROL_RADIUS))
        .accessibility_label(label)
        .tooltip(label)
}

fn active_tool(button: Button, active: bool, p: &t::Palette) -> Div {
    div()
        .rounded(px(t::CONTROL_RADIUS))
        .when(active, |d| d.bg(p.accent_soft))
        .child(
            button
                .toggled(active)
                .when(active, |b| b.text_color(p.primary)),
        )
}

fn speed_label(d: &DownloadInfo) -> String {
    match d.status.as_str() {
        "Downloading" => format!("↓ {}/s", bytes(d.speed)),
        "Seeding" => format!("↑ {}/s", bytes(d.speed)),
        "Complete" => d
            .avg_speed
            .filter(|speed| *speed > 0)
            .map_or_else(|| "—".into(), |speed| format!("≈ {}/s", bytes(speed))),
        _ => "—".into(),
    }
}

fn time_label(d: &DownloadInfo) -> String {
    if let Some(seconds) = eta_remaining(d) {
        return duration(seconds);
    }
    if d.status == "Complete"
        && let Some(seconds) = d.duration_secs
    {
        return format!("Took {}", duration(seconds));
    }
    "—".into()
}

fn transfer_progress(d: &DownloadInfo, height: f32, p: &t::Palette) -> Div {
    let downloading = d.status == "Downloading";
    progress_bar(
        fraction(d),
        height,
        downloading,
        matches!(d.status.as_str(), "Complete" | "Seeding"),
        is_error_status(&d.status),
        downloading && !d.size.is_some_and(|n| n > 0),
        p,
    )
}

fn transfer_status(d: &DownloadInfo, p: &t::Palette) -> Div {
    status_badge(
        &d.status,
        d.status == "Downloading",
        d.size.is_some_and(|n| n > 0),
        fraction(d) * 100.,
        p,
    )
}

fn file_icon(d: &DownloadInfo, dark: bool, tile: f32, glyph: f32) -> Div {
    let (color, bg) = t::category_tint(&d.category, dark);
    icon_tile(category_icon(&d.category), color, bg, tile, glyph)
}

fn info_row(label: &'static str, value: impl Into<SharedString>, p: &t::Palette) -> Div {
    row()
        .items_start()
        .gap(px(8.))
        .child(muted(label, p).w(px(69.)).flex_shrink_0())
        .child(
            text(value)
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .text_size(px(12.)),
        )
}

impl Shell {
    fn filter_label(&self) -> &str {
        match &self.filter {
            Filter::All => "All downloads",
            Filter::Unfinished => "Unfinished",
            Filter::Finished => "Finished",
            Filter::Category(key) => self
                .list
                .categories
                .iter()
                .find(|category| category.key == *key)
                .map_or(key.as_str(), |category| category.name.as_str()),
        }
    }

    fn show_progress(&mut self, cx: &mut Context<Self>) {
        if self.selected().is_some() {
            self.notice = None;
            self.detail = Value::Null;
            self.tab = 0;
            self.panel = Some("progress");
            cx.notify();
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        self.settings_tab = 0;
        self.panel = Some("settings");
        self.bridge.send("settings", json!({"op":"settings"}));
        cx.notify();
    }

    fn topbar(&self, width: f32, p: &t::Palette, cx: &mut Context<Self>) -> TitleBar {
        let (theme_icon, next) = match self.theme_pref {
            t::ThemePref::System => (IconName::Monitor, "Switch to light theme"),
            t::ThemePref::Light => (IconName::Sun, "Switch to dark theme"),
            t::ThemePref::Dark => (IconName::Moon, "Follow system theme"),
        };
        let sidebar_tip = if self.sidebar_collapsed {
            "Show sidebar"
        } else {
            "Hide sidebar"
        };
        TitleBar::new()
            .h(px(t::BAR_HEIGHT))
            .bg(p.chrome)
            .border_color(p.chrome)
            .child(
                row()
                    .w(px(212.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .child(icon_tile(
                        IconName::Download,
                        p.on_action,
                        p.action,
                        23.,
                        14.,
                    ))
                    .child(title("Next Download Manager").text_size(px(13.)))
                    .child(
                        tool("toggle-sidebar", IconName::PanelLeft, sidebar_tip)
                            .toggled(!self.sidebar_collapsed)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sidebar_collapsed = !s.sidebar_collapsed;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex_1())
            .child(
                div().w(px((width * 0.36).clamp(280., 560.))).child(
                    Input::new(&self.search)
                        .aria_label("Search downloads")
                        .bordered(false)
                        .rounded(px(t::CONTROL_RADIUS))
                        .bg(p.soft)
                        .prefix(icon(IconName::Search).size(px(14.)).text_color(p.muted))
                        .cleanable(true),
                ),
            )
            .child(div().flex_1())
            .child(row().flex_shrink_0().px(px(6.)).child(
                tool("toggle-theme", theme_icon, next).on_click(cx.listener(|s, _, w, cx| {
                    s.set_theme_pref(s.theme_pref.next(), w, cx);
                })),
            ))
    }

    fn nav_item(
        &self,
        label: &str,
        glyph: IconName,
        color: Rgba,
        filter: Filter,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let active = self.filter == filter;
        let count = self
            .list
            .downloads
            .iter()
            .filter(|d| filter.matches(d))
            .count();
        let id: SharedString = match &filter {
            Filter::Category(key) => format!("category-{key}").into(),
            _ => format!("filter-{label}").into(),
        };
        let button = Button::new(id)
            .h(px(30.))
            .ghost()
            .compact()
            .rounded(px(t::CONTROL_RADIUS))
            .toggled(active)
            .accessibility_label(label.to_owned())
            .tooltip(label.to_owned())
            .on_click(cx.listener(move |s, _, _, cx| {
                if s.filter != filter {
                    if s.selected().is_some_and(|d| !filter.matches(d)) {
                        s.selected = None;
                    }
                    s.filter = filter.clone();
                    cx.notify();
                }
            }));

        let base = row()
            .rounded(px(t::CONTROL_RADIUS))
            .when(active, |d| d.bg(p.selected));

        if self.sidebar_collapsed {
            return base
                .justify_center()
                .child(button.icon(glyph).w(px(32.)).text_color(color));
        }

        base.child(
            button.flex_1().min_w_0().child(
                row()
                    .gap(px(9.))
                    .w_full()
                    .min_w_0()
                    .child(icon(glyph).text_color(color))
                    .child(text(label.to_owned()).flex_1().min_w_0().text_ellipsis())
                    .when(count > 0 || active, |d| {
                        d.child(muted(count.to_string(), p).flex_shrink_0())
                    }),
            ),
        )
    }

    fn sidebar(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let collapsed = self.sidebar_collapsed;
        let mut nav = col()
            .id("sidebar-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(px(2.))
            .pr(px(1.))
            .when(!collapsed, |d| {
                d.child(title("Library").px(px(9.)).pb(px(5.)))
            });

        for (label, glyph, kind, color) in [
            ("All downloads", IconName::Download, Filter::All, p.primary),
            ("Unfinished", IconName::Clock, Filter::Unfinished, p.warning),
            (
                "Finished",
                IconName::CircleCheck,
                Filter::Finished,
                p.success,
            ),
        ] {
            nav = nav.child(self.nav_item(label, glyph, color, kind, p, cx));
        }
        nav = nav.child(if collapsed {
            div().h(px(1.)).mx(px(8.)).my(px(8.)).bg(p.border)
        } else {
            title("Categories").px(px(9.)).pt(px(14.)).pb(px(5.))
        });

        if self.list.categories.is_empty() {
            for key in [
                "General",
                "Compressed",
                "Documents",
                "Music",
                "Programs",
                "Video",
                "Torrents",
            ] {
                nav = nav.child(self.nav_item(
                    key,
                    category_icon(key),
                    t::category_tint(key, self.dark).0,
                    Filter::Category(key.to_owned()),
                    p,
                    cx,
                ));
            }
        } else {
            for category in &self.list.categories {
                let key = &category.key;
                nav = nav.child(self.nav_item(
                    &category.name,
                    category_icon(key),
                    t::category_tint(key, self.dark).0,
                    Filter::Category(key.clone()),
                    p,
                    cx,
                ));
            }
        }

        let queue = self.list.queues.first();
        let queue_status = queue.map_or("Idle", |q| if q.running { "Running" } else { "Stopped" });
        let queue_controls = row()
            .gap(px(2.))
            .when(collapsed, |d| d.flex_col())
            .child(
                tool("queue-start", IconName::Play, "Start main queue")
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, _, _| {
                        s.bridge.send("queue", json!({"op":"queue","start":true}));
                    })),
            )
            .child(
                tool("queue-stop", IconName::Pause, "Stop main queue")
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, _, _| {
                        s.bridge.send("queue", json!({"op":"queue","start":false}));
                    })),
            );
        let queue_item = row()
            .border_t_1()
            .border_color(p.border)
            .pt(px(7.))
            .pb(px(4.))
            .when(collapsed, |d| d.justify_center())
            .when(!collapsed, |d| {
                d.child(
                    col()
                        .flex_1()
                        .min_w_0()
                        .pl(px(9.))
                        .child(
                            title(queue.map_or("Main queue", |q| q.name.as_str()).to_owned())
                                .text_ellipsis(),
                        )
                        .child(muted(queue_status, p)),
                )
            })
            .child(queue_controls);
        let settings = if collapsed {
            tool("settings", IconName::Settings, "Settings")
        } else {
            btn("settings", "Settings").ghost().icon(IconName::Settings)
        };

        col()
            .w(px(if collapsed {
                t::RAIL_WIDTH
            } else {
                t::SIDEBAR_WIDTH
            }))
            .px(px(8.))
            .pt(px(5.))
            .pb(px(WORKSPACE_GUTTER))
            .gap(px(4.))
            .flex_shrink_0()
            .min_h_0()
            .child(nav)
            .child(queue_item)
            .child(
                row()
                    .when(collapsed, |d| d.justify_center())
                    .child(settings.on_click(cx.listener(|s, _, _, cx| s.open_settings(cx)))),
            )
    }

    fn commands(&self, visible: usize, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let selected = self.selected();
        let transferring =
            selected.is_some_and(|d| matches!(d.status.as_str(), "Downloading" | "Seeding"));
        let can_resume = selected.is_some_and(|d| d.status != "Complete") && !transferring;
        let path = selected
            .map(|d| d.path.clone())
            .filter(|path| !path.is_empty());
        let summary = if let Some(d) = selected {
            format!("1 selected · {}", display_size(d))
        } else {
            format!("{visible} {}", if visible == 1 { "item" } else { "items" })
        };

        row()
            .h(px(42.))
            .px(px(10.))
            .gap(px(3.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border_subtle)
            .child(
                row()
                    .flex_1()
                    .min_w_0()
                    .gap(px(7.))
                    .child(icon(IconName::Download).text_color(p.primary))
                    .child(
                        title(self.filter_label().to_owned())
                            .min_w_0()
                            .text_ellipsis(),
                    ),
            )
            .child(
                muted(summary, p)
                    .flex_shrink_0()
                    .pr(px(5.))
                    .when(selected.is_some(), |d| d.text_color(p.primary)),
            )
            .child(
                btn("add-url", "Add URL")
                    .primary()
                    .icon(IconName::Plus)
                    .h(px(28.))
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, w, cx| s.open_add(false, w, cx))),
            )
            .child(
                tool("add-torrent", IconName::Magnet, "Add torrent")
                    .disabled(!self.ready)
                    .on_click(cx.listener(|s, _, w, cx| s.open_add(true, w, cx))),
            )
            .child(rule(p))
            .child(
                tool("resume", IconName::Play, "Resume download")
                    .disabled(!can_resume)
                    .on_click(cx.listener(|s, _, _, _| s.action("start", false))),
            )
            .child(
                tool("pause", IconName::Pause, "Pause download")
                    .disabled(!transferring)
                    .on_click(cx.listener(|s, _, _, _| s.action("pause", false))),
            )
            .child(
                tool("pause-all", IconName::CirclePause, "Pause all downloads")
                    .disabled(self.active_count() == 0)
                    .on_click(cx.listener(|s, _, _, _| s.action("pause", true))),
            )
            .child(
                tool(
                    "open-folder",
                    IconName::FolderOpen,
                    "Open containing folder",
                )
                .disabled(path.is_none())
                .on_click(move |_, _, _| {
                    if let Some(path) = &path {
                        let path = std::path::Path::new(path);
                        let _ = open::that_detached(
                            path.parent()
                                .filter(|p| !p.as_os_str().is_empty())
                                .unwrap_or(path),
                        );
                    }
                }),
            )
            .child(
                tool("remove", IconName::Trash, "Remove download")
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.notice = None;
                        s.panel = Some("remove");
                        cx.notify();
                    })),
            )
            .child(
                tool("show-transfer", IconName::Activity, "Transfer details")
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|s, _, _, cx| s.show_progress(cx))),
            )
            .child(rule(p))
            .child(active_tool(
                tool("view-list", IconName::List, "List view").on_click(cx.listener(
                    |s, _, _, cx| {
                        s.view = View::List;
                        cx.notify();
                    },
                )),
                self.view == View::List,
                p,
            ))
            .child(active_tool(
                tool("view-grid", IconName::Grid2x2, "Grid view").on_click(cx.listener(
                    |s, _, _, cx| {
                        s.view = View::Grid;
                        cx.notify();
                    },
                )),
                self.view == View::Grid,
                p,
            ))
            .child(active_tool(
                tool(
                    "toggle-inspector",
                    IconName::PanelRight,
                    "Toggle details pane",
                )
                .on_click(cx.listener(|s, _, _, cx| {
                    s.inspector_open = !s.inspector_open;
                    cx.notify();
                })),
                self.inspector_open,
                p,
            ))
    }

    fn download_item(
        &self,
        download: &DownloadInfo,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = download.id;
        let selected = self.selected == Some(id);
        div()
            .id(("download", id))
            .role(Role::Button)
            .track_focus(&self.row_focus[&id])
            .tab_stop(true)
            .aria_selected(selected)
            .aria_label(format!("{} — {}", download.file, download.status))
            .cursor_pointer()
            .rounded(px(t::CONTROL_RADIUS))
            .when(selected, |d| d.bg(p.selected))
            .focus_visible(|style| style.bg(p.accent_soft).border_color(p.primary))
            .hover(|style| {
                style.bg(if selected {
                    p.selected_hover
                } else {
                    p.row_hover
                })
            })
            .on_key_down(cx.listener(move |s, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "space" | " " => {
                        s.selected = Some(id);
                        cx.notify();
                    }
                    "enter" => {
                        s.selected = Some(id);
                        s.show_progress(cx);
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_click(cx.listener(move |s, event: &ClickEvent, _, cx| {
                s.selected = Some(id);
                if event.click_count() == 2 {
                    s.show_progress(cx);
                }
                cx.notify();
            }))
    }

    fn download_list(
        &self,
        downloads: &[&DownloadInfo],
        width: f32,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let columns = ListColumns::for_width(width);
        let header = row()
            .h(px(29.))
            .mx(px(7.))
            .pl(px(ListColumns::NAME_INSET))
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border_subtle)
            .text_size(px(12.))
            .text_color(p.muted)
            .child(text("Name").flex_1().min_w_0())
            .child(cell("Size", ListColumns::SIZE_WIDTH))
            .child(cell("Status", ListColumns::STATUS_WIDTH))
            .when(columns.speed, |d| {
                d.child(cell("Speed", ListColumns::SPEED_WIDTH))
            })
            .when(columns.time, |d| {
                d.child(cell("Time", ListColumns::TIME_WIDTH))
            });

        let mut list = col()
            .id("downloads-list-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(7.))
            .pt(px(3.));
        for download in downloads {
            let name = row()
                .flex_1()
                .min_w_0()
                .gap(px(8.))
                .pr(px(8.))
                .child(file_icon(download, self.dark, 24., 14.))
                .child(text(download.file.clone()).min_w_0().text_ellipsis());
            let status = col()
                .w(px(ListColumns::STATUS_WIDTH))
                .flex_shrink_0()
                .px(px(12.))
                .gap(px(2.))
                .child(transfer_status(download, p))
                .when(download.status == "Downloading", |d| {
                    d.child(transfer_progress(download, 3., p))
                });
            list = list.child(
                self.download_item(download, p, cx)
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .h(px(36.))
                    .border_1()
                    .border_color(rgba(0x00000000))
                    .child(name.pl(px(7.)))
                    .child(
                        cell(display_size(download), ListColumns::SIZE_WIDTH)
                            .text_size(px(12.))
                            .text_color(p.ink_secondary),
                    )
                    .child(status)
                    .when(columns.speed, |d| {
                        d.child(
                            cell(speed_label(download), ListColumns::SPEED_WIDTH)
                                .text_size(px(12.))
                                .text_color(p.ink_secondary),
                        )
                    })
                    .when(columns.time, |d| {
                        d.child(
                            cell(time_label(download), ListColumns::TIME_WIDTH)
                                .text_size(px(12.))
                                .text_color(p.ink_secondary),
                        )
                    }),
            );
        }
        col().flex_1().min_h_0().child(header).child(list)
    }

    fn download_grid(
        &self,
        downloads: &[&DownloadInfo],
        width: f32,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let columns = ((width - 28.) / 145.).floor().max(1.) as usize;
        let mut grid = col()
            .id("downloads-grid-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(12.))
            .gap(px(8.));
        for group in downloads.chunks(columns) {
            let mut line = row().gap(px(8.)).flex_shrink_0();
            for download in group {
                line = line.child(
                    self.download_item(download, p, cx)
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .h(px(116.))
                        .px(px(9.))
                        .pt(px(10.))
                        .gap(px(5.))
                        .border_1()
                        .border_color(p.border_subtle)
                        .child(file_icon(download, self.dark, 38., 21.))
                        .child(text(download.file.clone()).text_ellipsis())
                        .child(transfer_status(download, p))
                        .when(download.status == "Downloading", |d| {
                            d.child(transfer_progress(download, 3., p))
                        }),
                );
            }
            line =
                line.children((group.len()..columns).map(|_| div().flex_1().min_w_0().px(px(10.))));
            grid = grid.child(line);
        }
        col().flex_1().min_h_0().child(grid)
    }

    fn empty(&self, searching: bool, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let (glyph, message) = if !self.ready {
            (IconName::Activity, "Connecting to download service…")
        } else if searching {
            (IconName::Search, "No downloads match your search")
        } else if self.filter != Filter::All {
            (IconName::File, "No downloads in this view")
        } else {
            (IconName::Download, "No downloads yet")
        };
        col()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .child(icon_tile(glyph, p.primary, p.accent_soft, 44., 21.))
            .child(title(message).text_size(px(14.)))
            .when(self.ready, |d| {
                let action = if searching {
                    btn("clear-search", "Clear search").on_click(cx.listener(|s, _, w, cx| {
                        s.search
                            .update(cx, |v, cx| v.set_value(String::new(), w, cx));
                        cx.notify();
                    }))
                } else if self.filter != Filter::All {
                    btn("show-all-downloads", "Show all downloads").on_click(cx.listener(
                        |s, _, _, cx| {
                            s.filter = Filter::All;
                            cx.notify();
                        },
                    ))
                } else {
                    btn("empty-add-url", "Add a download")
                        .icon(IconName::Plus)
                        .on_click(cx.listener(|s, _, w, cx| s.open_add(false, w, cx)))
                };
                d.child(action)
            })
    }

    fn inspector(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let header = row()
            .h(px(35.))
            .flex_shrink_0()
            .px(px(12.))
            .justify_between()
            .child(title("Details"))
            .child(
                tool("close-inspector", IconName::X, "Close details pane").on_click(cx.listener(
                    |s, _, _, cx| {
                        s.inspector_open = false;
                        cx.notify();
                    },
                )),
            );
        let body = if let Some(d) = self.selected() {
            col()
                .gap(px(12.))
                .p(px(12.))
                .child(
                    col()
                        .gap(px(9.))
                        .p(px(12.))
                        .rounded(px(t::CONTROL_RADIUS))
                        .bg(p.soft)
                        .child(file_icon(d, self.dark, 45., 23.))
                        .child(title(d.file.clone()).text_ellipsis().text_size(px(14.)))
                        .child(transfer_status(d, p))
                        .child(transfer_progress(d, 4., p))
                        .child(muted(format!("{} downloaded", bytes(d.downloaded)), p)),
                )
                .child(
                    col()
                        .gap(px(6.))
                        .px(px(2.))
                        .child(info_row("Size", display_size(d), p))
                        .child(info_row(
                            "Category",
                            if d.category.is_empty() {
                                "General".into()
                            } else {
                                d.category.clone()
                            },
                            p,
                        ))
                        .child(info_row("Speed", speed_label(d), p))
                        .child(info_row("Time", time_label(d), p))
                        .when_some(d.added.clone(), |el, value| {
                            el.child(info_row("Added", value, p))
                        })
                        .when(!d.path.is_empty(), |el| {
                            el.child(info_row("Location", d.path.clone(), p))
                        })
                        .when_some(d.error.clone(), |el, error| {
                            el.child(text(error).text_size(px(12.)).text_color(p.danger))
                        }),
                )
                .child(
                    btn("show-details", "Transfer details")
                        .icon(IconName::Activity)
                        .on_click(cx.listener(|s, _, _, cx| s.show_progress(cx))),
                )
        } else {
            col()
                .px(px(14.))
                .pt(px(16.))
                .child(muted("Select a download to see its details.", p))
        };

        col()
            .w(px(INSPECTOR_WIDTH))
            .flex_shrink_0()
            .min_h_0()
            .border_l_1()
            .border_color(if self.selected().is_some() {
                p.accent_border
            } else {
                p.border_subtle
            })
            .child(header)
            .child(
                div()
                    .id("inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
    }

    fn footer(&self, p: &t::Palette) -> Div {
        row()
            .h(px(29.))
            .px(px(10.))
            .gap(px(12.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.border_subtle)
            .text_size(px(12.))
            .text_color(p.muted)
            .child(
                row()
                    .gap(px(6.))
                    .child(div().size(px(6.)).rounded_full().bg(if self.ready {
                        p.success
                    } else {
                        p.warning
                    }))
                    .child(if self.ready {
                        "Service connected"
                    } else {
                        "Connecting…"
                    }),
            )
            .child(div().flex_1())
            .child(text(format!("{} active", self.active_count())))
            .child(
                row()
                    .gap(px(4.))
                    .child(
                        icon(IconName::ArrowDown)
                            .size(px(13.))
                            .text_color(p.primary),
                    )
                    .child(format!("{}/s", bytes(self.list.speed))),
            )
            .when_some(self.list.free, |d, free| {
                d.child(
                    row()
                        .gap(px(5.))
                        .child(icon(IconName::HardDrive).size(px(13.)))
                        .child(format!("{} free", bytes(free))),
                )
            })
    }

    fn workspace(&self, width: f32, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let downloads: Vec<_> = self
            .list
            .downloads
            .iter()
            .rev()
            .filter(|d| self.filter.matches(d) && matches_search(d, &query))
            .collect();
        let sidebar_width = if self.sidebar_collapsed {
            t::RAIL_WIDTH
        } else {
            t::SIDEBAR_WIDTH
        };
        let pane_width = width
            - sidebar_width
            - WORKSPACE_GUTTER
            - if self.inspector_open {
                INSPECTOR_WIDTH
            } else {
                0.
            };
        let items = if downloads.is_empty() {
            self.empty(!query.is_empty(), p, cx)
        } else {
            match self.view {
                View::List => self.download_list(&downloads, pane_width, p, cx),
                View::Grid => self.download_grid(&downloads, pane_width, p, cx),
            }
        };

        col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .mr(px(WORKSPACE_GUTTER))
            .mb(px(WORKSPACE_GUTTER))
            .bg(p.surface)
            .rounded(px(t::PANE_RADIUS))
            .border_1()
            .border_color(p.border)
            .overflow_hidden()
            .child(self.commands(downloads.len(), p, cx))
            .child(
                row()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(col().flex_1().min_w_0().child(items))
                    .when(self.inspector_open, |d| d.child(self.inspector(p, cx))),
            )
            .child(self.footer(p))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t::palette(self.dark);
        let width = f32::from(window.viewport_size().width);
        col()
            .track_focus(&self.focus)
            .on_action(cx.listener(|s: &mut Self, _: &ClosePanel, w, cx| {
                if s.panel.is_some() {
                    s.close_panel();
                    w.focus(&s.focus, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|s: &mut Self, _: &NewDownload, w, cx| {
                if s.ready && s.panel.is_none() {
                    s.open_add(false, w, cx);
                }
            }))
            .on_action(cx.listener(|s: &mut Self, _: &ShowDetails, _, cx| {
                if s.panel.is_none() {
                    s.show_progress(cx);
                }
            }))
            .on_action(cx.listener(|s: &mut Self, _: &FocusSearch, w, cx| {
                if s.panel.is_none() {
                    s.search.update(cx, |state, cx| state.focus(w, cx));
                }
            }))
            .size_full()
            .relative()
            .bg(p.chrome)
            .text_color(p.ink)
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .child(self.topbar(width, &p, cx))
            .child(
                row()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(self.sidebar(&p, cx))
                    .child(self.workspace(width, &p, cx)),
            )
            .children(
                self.notice
                    .as_ref()
                    .filter(|_| self.panel.is_none())
                    .map(|notice| {
                        row()
                            .absolute()
                            .bottom(px(46.))
                            .right(px(20.))
                            .max_w(px(460.))
                            .p(px(9.))
                            .gap(px(8.))
                            .rounded(px(t::CONTROL_RADIUS))
                            .border_1()
                            .border_color(p.border)
                            .bg(p.modal_surface)
                            .shadow_md()
                            .child(icon(IconName::Info).text_color(p.primary))
                            .child(text(notice.clone()).flex_1())
                            .child(
                                tool("dismiss", IconName::X, "Dismiss notification").on_click(
                                    cx.listener(|s, _, _, cx| {
                                        s.notice = None;
                                        cx.notify();
                                    }),
                                ),
                            )
                    }),
            )
            .children(
                self.panel.map(|panel| {
                    self.overlay(panel, f32::from(window.viewport_size().height), &p, cx)
                }),
            )
    }
}
