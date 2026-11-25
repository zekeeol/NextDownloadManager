use super::*;

fn dialog_content(compact: bool) -> Div {
    col()
        .gap(px(if compact { 10. } else { 14. }))
        .p(px(if compact { 16. } else { 20. }))
}

fn dialog_footer(p: &t::Palette) -> Div {
    row()
        .h(px(49.))
        .flex_shrink_0()
        .px(px(20.))
        .bg(p.modal_chrome)
        .rounded_b(px(t::PANE_RADIUS))
        .border_t_1()
        .border_color(p.border)
        .items_center()
        .justify_end()
        .gap(px(8.))
}

fn settings_description(value: &'static str, p: &t::Palette) -> Div {
    muted(value, p)
        .min_w_0()
        .whitespace_normal()
        .line_height(px(18.))
}

impl Shell {
    fn detail_tabs(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let mut bar = row()
            .gap(px(4.))
            .pb(px(5.))
            .border_b_1()
            .border_color(p.border);
        for (index, label) in ["Download status", "Speed limiter", "On completion"]
            .into_iter()
            .enumerate()
        {
            let selected = self.tab == index;
            bar = bar.child(
                div()
                    .rounded(px(t::CONTROL_RADIUS))
                    .when(selected, |d| d.bg(p.selected))
                    .child(
                        btn(("detail-tab", index), label)
                            .ghost()
                            .toggled(selected)
                            .text_color(if selected { p.primary } else { p.ink_secondary })
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.tab = index;
                                cx.notify();
                            })),
                    ),
            );
        }
        bar
    }

    fn settings_navigation(&self, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let mut nav = col()
            .w(px(152.))
            .p(px(8.))
            .gap(px(2.))
            .flex_shrink_0()
            .bg(p.modal_chrome)
            .border_r_1()
            .border_color(p.border_subtle);
        for (index, (label, glyph)) in [
            ("General", IconName::Settings),
            ("Speed & Limits", IconName::Activity),
            ("BitTorrent", IconName::Magnet),
        ]
        .into_iter()
        .enumerate()
        {
            let selected = self.settings_tab == index;
            let label_content = row()
                .w_full()
                .min_w_0()
                .gap(px(8.))
                .child(div().flex_shrink_0().child(icon(glyph).size(px(15.))))
                .child(
                    text(label)
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.))
                        .text_ellipsis(),
                );
            nav = nav.child(
                div()
                    .rounded(px(t::CONTROL_RADIUS))
                    .when(selected, |d| d.bg(p.selected))
                    .child(
                        Button::new(("settings-tab", index))
                            .w_full()
                            .h(px(32.))
                            .px(px(10.))
                            .rounded(px(t::CONTROL_RADIUS))
                            .ghost()
                            .accessibility_label(label)
                            .toggled(selected)
                            .text_color(if selected { p.primary } else { p.ink_secondary })
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.settings_tab = index;
                                cx.notify();
                            }))
                            .child(label_content),
                    ),
            );
        }
        nav
    }

    pub(super) fn overlay(
        &self,
        panel: &str,
        height: f32,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (title, header_ico, header_color, header_bg) = match panel {
            "add" => ("Add download", IconName::Plus, p.primary, p.accent_soft),
            "torrent" => {
                let (c, b) = t::category_tint("Torrents", self.dark);
                ("Add torrent", IconName::Magnet, c, b)
            }
            "progress" => (
                "Download details",
                IconName::Activity,
                p.primary,
                p.accent_soft,
            ),
            "remove" => ("Remove download", IconName::Trash, p.danger, p.danger_soft),
            _ => ("Settings", IconName::Settings, p.primary, p.accent_soft),
        };
        let compact = height < 700.;
        // Keeping each body in its own function limits the stack used by GPUI
        // builder temporaries in unoptimized Windows builds.
        let (mut content, footer_bar) = match panel {
            "add" => self.add_dialog(false, compact, p, cx),
            "torrent" => self.add_dialog(true, compact, p, cx),
            "progress" => self.progress_dialog(compact, p, cx),
            "remove" => self.remove_dialog(compact, p, cx),
            _ => self.settings_dialog(compact, p, cx),
        };

        if let Some(notice) = &self.notice {
            content = col()
                .child(
                    row()
                        .mx(px(if compact { 16. } else { 20. }))
                        .mt(px(if compact { 12. } else { 16. }))
                        .p(px(10.))
                        .gap(px(8.))
                        .rounded(px(8.))
                        .bg(p.danger_soft)
                        .border_1()
                        .border_color(p.danger_border)
                        .child(
                            icon(IconName::CircleAlert)
                                .size(px(15.))
                                .text_color(p.danger),
                        )
                        .child(
                            text(notice.clone())
                                .flex_1()
                                .text_size(px(12.))
                                .text_color(p.danger),
                        ),
                )
                .child(content);
        }

        let dialog_height = (height - 40.).min(match panel {
            "remove" => 280.,
            "add" => 360.,
            "torrent" => 640.,
            "progress" => 710.,
            "settings" => 710.,
            _ => 470.,
        });

        let dialog = col()
            .id("modal-dialog")
            .role(Role::Dialog)
            .focus_trap("native-dialog", &self.focus)
            .aria_label(title)
            .w(px(match panel {
                "settings" => 728.,
                "progress" | "torrent" => 640.,
                "remove" => 496.,
                _ => 568.,
            }))
            .h(px(dialog_height))
            .bg(p.modal_surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(t::PANE_RADIUS))
            .overflow_hidden()
            .shadow_xl()
            .child(
                row()
                    .h(px(46.))
                    .flex_shrink_0()
                    .px(px(18.))
                    .justify_between()
                    .bg(p.modal_chrome)
                    .rounded_t(px(t::PANE_RADIUS))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        row()
                            .gap(px(10.))
                            .child(icon_tile(header_ico, header_color, header_bg, 28., 15.))
                            .child(
                                text(title)
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink),
                            ),
                    )
                    .child(
                        Button::new("close-dialog")
                            .icon(IconName::X)
                            .ghost()
                            .h(px(28.))
                            .rounded(px(7.))
                            .accessibility_label("Close")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.close_panel();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                row()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .when(panel == "settings", |d| {
                        d.child(self.settings_navigation(p, cx))
                    })
                    .child(
                        div()
                            .id((
                                "dialog-scroll",
                                if panel == "settings" {
                                    self.settings_tab
                                } else {
                                    0
                                },
                            ))
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(content),
                    ),
            )
            .child(footer_bar);

        col()
            .id("modal-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .bg(p.backdrop)
            .child(dialog)
    }

    fn add_dialog(
        &self,
        torrent: bool,
        compact: bool,
        p: &t::Palette,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let mut content = dialog_content(compact);
        let mut footer_bar = dialog_footer(p);
        content = content.child(helper_text(
            if torrent {
                "Choose a .torrent file or paste a magnet link to inspect its contents."
            } else {
                "Enter a direct file URL and choose where to save it."
            },
            p,
        ));
        content = content.child(
            col()
                .gap(px(6.))
                .child(field_label(
                    if torrent {
                        "Torrent source or magnet link"
                    } else {
                        "Download URL"
                    },
                    p,
                ))
                .child(Input::new(&self.source).cleanable(true)),
        );

        if torrent {
            content = content.child(
                row()
                    .gap(px(8.))
                    .child(
                        btn("browse-torrent", "Choose .torrent file…")
                            .icon(IconName::FolderOpen)
                            .on_click(cx.listener(|_s, _, w, cx| {
                                let weak = cx.entity().downgrade();
                                let task = rfd::AsyncFileDialog::new()
                                    .add_filter("Torrent", &["torrent"])
                                    .pick_file();
                                cx.spawn_in(w, async move |_, cx| {
                                    if let Some(file) = task.await {
                                        let path = file.path().to_string_lossy().into_owned();
                                        let _ = weak.update_in(cx, |s, w, cx| {
                                            s.source.update(cx, |v, cx| v.set_value(path, w, cx));
                                            s.inspect(cx);
                                        });
                                    }
                                })
                                .detach();
                            })),
                    )
                    .child(
                        btn("read-torrent", "Read files")
                            .icon(IconName::RefreshCw)
                            .disabled(
                                self.torrent_token.is_some() && self.torrent["files"].is_null(),
                            )
                            .on_click(cx.listener(|s, _, _, cx| s.inspect(cx))),
                    ),
            );

            let mut tree = col()
                .h(px(if compact { 120. } else { 215. }))
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .rounded(px(8.));

            let mut chosen_bytes: u64 = 0;
            if let Some(files) = self.torrent["files"].as_array() {
                tree = tree.child(
                    row()
                        .px(px(12.))
                        .py(px(9.))
                        .flex_shrink_0()
                        .justify_between()
                        .bg(p.table_header)
                        .rounded_t(px(7.))
                        .border_b_1()
                        .border_color(p.border)
                        .child(
                            text(
                                self.torrent["name"]
                                    .as_str()
                                    .unwrap_or("Torrent")
                                    .to_owned(),
                            )
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink)
                            .text_ellipsis(),
                        )
                        .child(
                            text(format!("{} files", files.len()))
                                .text_size(px(11.))
                                .text_color(p.muted),
                        ),
                );
                let mut file_rows = col()
                    .id("torrent-files")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(4.));
                for file in files {
                    let index = file["index"].as_u64().unwrap_or(0) as usize;
                    let path = file["path"].as_str().unwrap_or("").to_owned();
                    let fsize = file["size"].as_u64().unwrap_or(0);
                    let checked = self.chosen.contains(&index);
                    if checked {
                        chosen_bytes = chosen_bytes.saturating_add(fsize);
                    }
                    file_rows = file_rows.child(
                        row()
                            .h(px(34.))
                            .flex_shrink_0()
                            .px(px(8.))
                            .gap(px(8.))
                            .rounded(px(6.))
                            .border_b_1()
                            .border_color(p.border_subtle)
                            .hover(|s| s.bg(p.row_hover))
                            .child(
                                Checkbox::new(("torrent-file", index))
                                    .checked(checked)
                                    .accessibility_label(path.clone())
                                    .on_click(cx.listener(move |s, checked: &bool, _, cx| {
                                        if *checked {
                                            s.chosen.insert(index);
                                        } else {
                                            s.chosen.remove(&index);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                text(path)
                                    .flex_1()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .text_size(px(12.))
                                    .text_color(p.ink),
                            )
                            .child(muted(bytes(fsize), p).text_size(px(11.5))),
                    );
                }
                tree = tree.child(file_rows);
            } else {
                tree = tree.child(
                    col()
                        .id("torrent-files")
                        .flex_1()
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .gap(px(8.))
                        .child(icon_tile(IconName::File, p.muted, p.soft, 36., 18.))
                        .child(muted(
                            if self.torrent_token.is_some() {
                                "Reading torrent metadata…"
                            } else {
                                "The torrent's files will appear here."
                            },
                            p,
                        ))
                        .when_some(self.torrent["lookup"].as_object(), |c, _| {
                            c.child(helper_text(
                                format!(
                                    "{} trackers · {} DHT nodes · {} peers · {} connected",
                                    self.torrent["lookup"]["trackers"],
                                    self.torrent["lookup"]["dht_nodes"],
                                    self.torrent["lookup"]["peers"],
                                    self.torrent["lookup"]["connected"]
                                ),
                                p,
                            ))
                        }),
                );
            }

            content = content.child(tree).child(
                row()
                    .justify_between()
                    .child(helper_text(
                        if chosen_bytes > 0 {
                            format!(
                                "{} files selected ({})",
                                self.chosen.len(),
                                bytes(chosen_bytes)
                            )
                        } else {
                            format!("{} files selected", self.chosen.len())
                        },
                        p,
                    ))
                    .child(
                        row()
                            .gap(px(4.))
                            .child(btn("select-all", "Select all").ghost().h(px(26.)).on_click(
                                cx.listener(|s, _, _, cx| {
                                    s.chosen = s.torrent["files"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .filter_map(|f| f["index"].as_u64().map(|n| n as usize))
                                        .collect();
                                    cx.notify();
                                }),
                            ))
                            .child(
                                btn("select-none", "Select none")
                                    .ghost()
                                    .h(px(26.))
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.chosen.clear();
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }

        content = content.child(
            col().gap(px(6.)).child(field_label("Save to", p)).child(
                row()
                    .gap(px(8.))
                    .child(div().flex_1().child(Input::new(&self.folder)))
                    .child(
                        btn("browse-folder", "Browse…")
                            .icon(IconName::FolderOpen)
                            .on_click(cx.listener(|_, _, w, cx| {
                                let weak = cx.entity().downgrade();
                                let dialog = rfd::AsyncFileDialog::new().pick_folder();
                                cx.spawn_in(w, async move |_, cx| {
                                    if let Some(folder) = dialog.await {
                                        let path = folder.path().to_string_lossy().into_owned();
                                        let _ = weak.update_in(cx, |s, w, cx| {
                                            s.folder.update(cx, |v, cx| v.set_value(path, w, cx));
                                        });
                                    }
                                })
                                .detach();
                            })),
                    ),
            ),
        );

        if torrent {
            content = content.child(
                Checkbox::new("create-subfolder")
                    .label("Create a subfolder for this torrent")
                    .checked(self.subfolder)
                    .on_click(cx.listener(|s, v: &bool, _, cx| {
                        s.subfolder = *v;
                        cx.notify();
                    })),
            );
        }

        footer_bar = footer_bar
            .child(
                btn("cancel-add", "Cancel").on_click(cx.listener(|s, _, _, cx| {
                    s.close_panel();
                    cx.notify();
                })),
            )
            .child(
                btn("download-later", "Download later")
                    .disabled(torrent && self.chosen.is_empty())
                    .on_click(cx.listener(|s, _, _, cx| s.add(true, cx))),
            )
            .child(
                btn("download-now", "Download now")
                    .primary()
                    .icon(IconName::Download)
                    .disabled(torrent && self.chosen.is_empty())
                    .on_click(cx.listener(|s, _, _, cx| s.add(false, cx))),
            );
        (content, footer_bar)
    }

    fn progress_dialog(&self, compact: bool, p: &t::Palette, cx: &mut Context<Self>) -> (Div, Div) {
        let mut content = dialog_content(compact);
        let mut footer_bar = dialog_footer(p);
        if let Some(d) = self.selected() {
            let d = d.clone();
            let frac = fraction(&d);
            let downloading = d.status == "Downloading";
            let seeding = d.status == "Seeding";
            let complete = d.status == "Complete";
            let is_err = is_error_status(&d.status);
            let has_size = d.size.is_some_and(|n| n > 0);
            let (ico_color, ico_bg) = t::category_tint(&d.category, self.dark);

            content = content
                .child(
                    row()
                        .gap(px(12.))
                        .child(icon_tile(
                            category_icon(&d.category),
                            ico_color,
                            ico_bg,
                            38.,
                            18.,
                        ))
                        .child(
                            col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(3.))
                                .child(
                                    text(d.file.clone())
                                        .text_size(px(15.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink)
                                        .text_ellipsis(),
                                )
                                .child(status_badge(
                                    &d.status,
                                    downloading,
                                    has_size,
                                    frac * 100.,
                                    p,
                                )),
                        ),
                )
                .child(self.detail_tabs(p, cx));

            if self.tab == 0 {
                let resume = match self.detail["resumable"].as_bool() {
                    Some(true) => "Supported",
                    Some(false) => "Not supported",
                    None => "Not known yet",
                };
                let (time_label, time_value) = if complete {
                    (
                        "Elapsed time",
                        d.duration_secs.map(duration).unwrap_or_else(|| "—".into()),
                    )
                } else {
                    (
                        "Time left",
                        eta_remaining(&d)
                            .map(duration)
                            .unwrap_or_else(|| "—".into()),
                    )
                };
                let downloaded_text = if has_size || complete {
                    format!("{} ({:.1}%)", bytes(d.downloaded), frac * 100.)
                } else {
                    bytes(d.downloaded)
                };
                let rate_text = if complete {
                    d.avg_speed
                        .filter(|s| *s > 0)
                        .map(|s| format!("{}/s (avg)", bytes(s)))
                        .unwrap_or_else(|| "—".into())
                } else {
                    format!("{}/s", bytes(d.speed))
                };

                let mut meta = section_card(p).child(
                    div()
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border_subtle)
                        .child(
                            text(d.url.clone())
                                .text_size(px(11.))
                                .font_family("Menlo")
                                .text_color(p.ink_secondary)
                                .text_ellipsis(),
                        ),
                );

                let pairs = [
                    (
                        ("Status", d.status.clone()),
                        ("File size", display_size(&d)),
                    ),
                    (
                        ("Downloaded", downloaded_text),
                        ("Transfer rate", rate_text),
                    ),
                    (
                        (time_label, time_value),
                        ("Resume capability", resume.into()),
                    ),
                ];
                for ((l1, v1), (l2, v2)) in pairs {
                    meta = meta.child(
                        row()
                            .gap(px(16.))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(helper_text(l1, p))
                                    .child(
                                        text(v1)
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(p.ink)
                                            .text_ellipsis(),
                                    ),
                            )
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(helper_text(l2, p))
                                    .child(
                                        text(v2)
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(p.ink)
                                            .text_ellipsis(),
                                    ),
                            ),
                    );
                }

                content = content.child(meta).child(
                    col()
                        .gap(px(7.))
                        .child(
                            row()
                                .justify_between()
                                .child(field_label("Overall progress", p))
                                .child(
                                    text(if downloading && !has_size {
                                        "Streaming…".into()
                                    } else {
                                        format!("{:.2}%", frac * 100.)
                                    })
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(
                                        if complete || seeding {
                                            p.success
                                        } else {
                                            p.primary
                                        },
                                    ),
                                ),
                        )
                        .child(progress_bar(
                            frac,
                            8.,
                            downloading,
                            complete || seeding,
                            is_err,
                            downloading && !has_size,
                            p,
                        )),
                );

                if self.details {
                    content = content.child(
                        helper_text("Start positions and download progress by connections", p)
                            .text_align(TextAlign::Center),
                    );
                    let mut track = row()
                        .h(px(20.))
                        .p(px(2.))
                        .bg(p.progress_track)
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(6.));
                    if let Some(segments) = self.detail["segments"].as_array() {
                        let total = d.size.unwrap_or(1).max(1) as f32;
                        let seg_count = segments.len();
                        for (idx, s) in segments.iter().enumerate() {
                            let start = s["start"].as_u64().unwrap_or(0);
                            let end = s["end"].as_u64().unwrap_or(0);
                            let len = end.saturating_sub(start).max(1);
                            let done = s["cursor"].as_u64().unwrap_or(start).saturating_sub(start);
                            let is_first = idx == 0;
                            let is_last = idx + 1 == seg_count;
                            track = track.child(
                                div()
                                    .h_full()
                                    .w(relative(len as f32 / total))
                                    .when(is_first, |d| d.rounded_l(px(4.)))
                                    .when(is_last, |d| d.rounded_r(px(4.)))
                                    .when(!is_last, |d| {
                                        d.border_r_1().border_color(p.segment_divider)
                                    })
                                    .when(done > 0, |d| {
                                        d.child(
                                            div()
                                                .h_full()
                                                .w(relative(done as f32 / len as f32))
                                                .when(is_first, |d| d.rounded_l(px(4.)))
                                                .when(is_last, |d| d.rounded_r(px(4.)))
                                                .bg(p.success),
                                        )
                                    }),
                            );
                        }
                    }
                    content = content.child(track);

                    let mut conn_rows = col()
                        .id("connection-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .pb(px(4.));
                    if let Some(conns) = self.detail["connections"].as_array() {
                        for c in conns {
                            conn_rows = conn_rows.child(
                                row()
                                    .h(px(28.))
                                    .flex_shrink_0()
                                    .border_b_1()
                                    .border_color(p.border_subtle)
                                    .text_size(px(11.5))
                                    .child(
                                        cell((c["id"].as_u64().unwrap_or(0) + 1).to_string(), 44.)
                                            .text_color(p.muted),
                                    )
                                    .child(
                                        cell(bytes(c["downloaded"].as_u64().unwrap_or(0)), 138.)
                                            .text_color(p.ink),
                                    )
                                    .child(
                                        cell(
                                            format!(
                                                "{}/s",
                                                bytes(c["speed"].as_u64().unwrap_or(0))
                                            ),
                                            110.,
                                        )
                                        .text_color(p.ink_secondary),
                                    )
                                    .child(
                                        text(c["status"].as_str().unwrap_or("").to_owned())
                                            .text_size(px(11.5))
                                            .text_color(p.muted)
                                            .text_ellipsis(),
                                    ),
                            );
                        }
                    }
                    let table = col()
                        .h(px(150.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(8.))
                        .child(
                            row()
                                .h(px(30.))
                                .flex_shrink_0()
                                .bg(p.table_header)
                                .rounded_t(px(7.))
                                .border_b_1()
                                .border_color(p.border)
                                .text_size(px(10.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.muted)
                                .child(cell("#", 44.))
                                .child(cell("DOWNLOADED", 138.))
                                .child(cell("SPEED", 110.))
                                .child(text("CONNECTION STATUS")),
                        )
                        .child(conn_rows);
                    content = content.child(table);
                }

                footer_bar = footer_bar
                    .justify_between()
                    .child(
                        btn(
                            "toggle-details",
                            if self.details {
                                "Hide details"
                            } else {
                                "Show details"
                            },
                        )
                        .ghost()
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.details = !s.details;
                            cx.notify();
                        })),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .child(
                                btn("detail-pause", if downloading { "Pause" } else { "Resume" })
                                    .icon(if downloading {
                                        IconName::Pause
                                    } else {
                                        IconName::Play
                                    })
                                    .disabled(complete)
                                    .on_click(cx.listener(|s, _, _, _| {
                                        s.action(
                                            if s.selected()
                                                .is_some_and(|d| d.status == "Downloading")
                                            {
                                                "pause"
                                            } else {
                                                "start"
                                            },
                                            false,
                                        )
                                    })),
                            )
                            .child(btn("detail-close", "Close").primary().on_click(cx.listener(
                                |s, _, _, cx| {
                                    s.close_panel();
                                    cx.notify();
                                },
                            ))),
                    );
            } else if self.tab == 1 {
                content = content.child(
                    section_card(p)
                        .child(
                            text("Per-download speed limit")
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink),
                        )
                        .child(helper_text(
                            "Throttle this transfer without affecting other downloads in the queue.",
                            p,
                        ))
                        .child(
                            col()
                                .gap(px(6.))
                                .child(field_label("Maximum rate (KB/s)", p))
                                .child(Input::new(&self.limit)),
                        )
                        .child(
                            row()
                                .gap(px(8.))
                                .child(
                                    btn("apply-limit", "Apply limit").primary().on_click(
                                        cx.listener(|s, _, _, cx| {
                                            match s
                                                .limit
                                                .read(cx)
                                                .value()
                                                .trim()
                                                .parse::<u64>()
                                                .ok()
                                                .and_then(|n| n.checked_mul(1024))
                                                .filter(|n| *n > 0)
                                            {
                                                Some(rate) => {
                                                    s.bridge.send(
                                                        "limit",
                                                        json!({"op":"limit","id":s.selected,"bytes":rate}),
                                                    );
                                                    s.notice =
                                                        Some("Speed limit applied".into());
                                                }
                                                None => {
                                                    s.notice = Some(
                                                        "Enter a positive speed in KB per second"
                                                            .into(),
                                                    )
                                                }
                                            }
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .child(btn("unlimited", "No limit").on_click(
                                    cx.listener(|s, _, _, cx| {
                                        s.bridge.send(
                                            "limit",
                                            json!({"op":"limit","id":s.selected,"bytes":null}),
                                        );
                                        s.notice = Some("Speed limit removed".into());
                                        cx.notify();
                                    }),
                                )),
                        ),
                );
                footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(cx.listener(
                    |s, _, _, cx| {
                        s.close_panel();
                        cx.notify();
                    },
                )));
            } else {
                let path_for_open = d.path.clone();
                content = content.child(
                    section_card(p)
                        .child(
                            text("Destination path")
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink),
                        )
                        .child(helper_text(
                            "Completed files remain listed in your library for quick access.",
                            p,
                        ))
                        .child(
                            div()
                                .px(px(10.))
                                .py(px(8.))
                                .rounded(px(6.))
                                .bg(p.surface)
                                .border_1()
                                .border_color(p.border_subtle)
                                .child(
                                    text(d.path.clone())
                                        .text_size(px(11.5))
                                        .font_family("Menlo")
                                        .text_color(p.ink_secondary)
                                        .text_ellipsis(),
                                ),
                        )
                        .child(
                            row().child(
                                btn("open-folder", "Open destination folder")
                                    .icon(IconName::FolderOpen)
                                    .primary()
                                    .on_click(move |_, _, _| {
                                        let path = std::path::PathBuf::from(&path_for_open);
                                        let folder = path.parent().unwrap_or(&path);
                                        let _ = open::that_detached(folder);
                                    }),
                            ),
                        ),
                );
                footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(cx.listener(
                    |s, _, _, cx| {
                        s.close_panel();
                        cx.notify();
                    },
                )));
            }
        } else {
            content = content.child(muted("Select a download to view its progress.", p));
            footer_bar = footer_bar.child(btn("detail-close", "Close").on_click(cx.listener(
                |s, _, _, cx| {
                    s.close_panel();
                    cx.notify();
                },
            )));
        }
        (content, footer_bar)
    }

    fn remove_dialog(&self, compact: bool, p: &t::Palette, cx: &mut Context<Self>) -> (Div, Div) {
        let mut content = dialog_content(compact);
        let mut footer_bar = dialog_footer(p);
        if let Some(d) = self.selected() {
            let (ico_color, ico_bg) = t::category_tint(&d.category, self.dark);
            content = content.child(
                row()
                    .p(px(12.))
                    .gap(px(12.))
                    .rounded(px(10.))
                    .bg(p.soft)
                    .border_1()
                    .border_color(p.border_subtle)
                    .child(icon_tile(
                        category_icon(&d.category),
                        ico_color,
                        ico_bg,
                        36.,
                        17.,
                    ))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                text(d.file.clone())
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.ink)
                                    .text_ellipsis(),
                            )
                            .child(helper_text(
                                d.size.map(bytes).unwrap_or_else(|| "Unknown size".into()),
                                p,
                            )),
                    ),
            );
        }
        content = content
            .child(
                text("Remove the selected download from the list?")
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.ink),
            )
            .child(helper_text(
                "The downloaded file will stay on your disk.",
                p,
            ));
        footer_bar = footer_bar
            .child(btn("keep", "Cancel").on_click(cx.listener(|s, _, _, cx| {
                s.close_panel();
                cx.notify();
            })))
            .child(
                btn("confirm-remove", "Remove")
                    .danger()
                    .icon(IconName::Trash)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.action("remove", false);
                        s.close_panel();
                        s.selected = None;
                        cx.notify();
                    })),
            );
        (content, footer_bar)
    }

    fn settings_dialog(&self, compact: bool, p: &t::Palette, cx: &mut Context<Self>) -> (Div, Div) {
        let content = dialog_content(compact);
        let content = match self.settings_tab {
            0 => self.general_settings(content, p, cx),
            1 => self.speed_settings(content, p),
            _ => self.torrent_settings(content, p, cx),
        };
        let mut footer_bar = dialog_footer(p);
        footer_bar = footer_bar
            .child(
                btn("cancel-settings", "Cancel").on_click(cx.listener(|s, _, _, cx| {
                    s.close_panel();
                    cx.notify();
                })),
            )
            .child(
                btn("save-settings", "Save settings")
                    .primary()
                    .icon(IconName::Check)
                    .on_click(cx.listener(|s, _, _, cx| {
                        let connections = match s.connections.read(cx).value().trim().parse::<u32>()
                        {
                            Ok(n) if (1..=128).contains(&n) => n,
                            _ => {
                                s.notice =
                                    Some("Enter a connection count between 1 and 128".into());
                                cx.notify();
                                return;
                            }
                        };
                        let max_concurrent =
                            match s.max_concurrent.read(cx).value().trim().parse::<u32>() {
                                Ok(n) if (1..=16).contains(&n) => n,
                                _ => {
                                    s.notice = Some(
                                        "Enter a concurrent queue limit between 1 and 16".into(),
                                    );
                                    cx.notify();
                                    return;
                                }
                            };
                        let global_dl_kb =
                            match s.global_dl_limit.read(cx).value().trim().parse::<u64>() {
                                Ok(n) if n <= 10_485_760 => n,
                                _ => {
                                    s.notice = Some(
                                    "Enter a valid global download limit in KB/s (0 = unlimited)"
                                        .into(),
                                );
                                    cx.notify();
                                    return;
                                }
                            };
                        let per_dl_kb = match s.per_dl_limit.read(cx).value().trim().parse::<u64>()
                        {
                            Ok(n) if n <= 10_485_760 => n,
                            _ => {
                                s.notice = Some(
                                    "Enter a valid per-download limit in KB/s (0 = unlimited)"
                                        .into(),
                                );
                                cx.notify();
                                return;
                            }
                        };
                        let torrent_ul_kb =
                            match s.torrent_ul_limit.read(cx).value().trim().parse::<u64>() {
                                Ok(n) if n <= 10_485_760 => n,
                                _ => {
                                    s.notice = Some(
                                    "Enter a valid torrent upload limit in KB/s (0 = unlimited)"
                                        .into(),
                                );
                                    cx.notify();
                                    return;
                                }
                            };
                        let seed_minutes = match s
                            .torrent_seed_minutes
                            .read(cx)
                            .value()
                            .trim()
                            .parse::<u32>()
                        {
                            Ok(n) if n <= 525_600 => n,
                            _ => {
                                s.notice =
                                    Some("Enter seeding minutes between 0 and 525600".into());
                                cx.notify();
                                return;
                            }
                        };
                        let seed_ratio =
                            match s.torrent_seed_ratio.read(cx).value().trim().parse::<f64>() {
                                Ok(r) if r.is_finite() && (0.0..=100.0).contains(&r) => r,
                                _ => {
                                    s.notice = Some("Enter a share ratio between 0 and 100".into());
                                    cx.notify();
                                    return;
                                }
                            };
                        let peer_limit =
                            match s.torrent_peer_limit.read(cx).value().trim().parse::<u32>() {
                                Ok(n) if (10..=500).contains(&n) => n,
                                _ => {
                                    s.notice = Some(
                                        "Enter max peers per torrent between 10 and 500".into(),
                                    );
                                    cx.notify();
                                    return;
                                }
                            };
                        let tracker_urls = parse_tracker_urls(&s.tracker_urls.read(cx).value());
                        s.bridge.send(
                            "settings-saved",
                            json!({
                                "op": "save_settings",
                                "connections": connections,
                                "folder": s.folder.read(cx).value().to_string(),
                                "tracker_urls": tracker_urls,
                                "max_concurrent": max_concurrent,
                                "global_download_limit_kb": global_dl_kb,
                                "per_download_limit_kb": per_dl_kb,
                                "torrent_upload_limit_kb": torrent_ul_kb,
                                "torrent_seed": s.torrent_seed,
                                "torrent_seed_minutes": seed_minutes,
                                "torrent_seed_ratio": seed_ratio,
                                "torrent_peer_limit": peer_limit,
                                "torrent_dht": s.torrent_dht,
                                "torrent_utp": s.torrent_utp,
                            }),
                        );
                        cx.notify();
                    })),
            );
        (content, footer_bar)
    }

    fn general_settings(&self, mut content: Div, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let mut theme_selector = row().gap(px(6.));
        for (pref, ico) in [
            (t::ThemePref::System, IconName::Monitor),
            (t::ThemePref::Light, IconName::Sun),
            (t::ThemePref::Dark, IconName::Moon),
        ] {
            let active = self.theme_pref == pref;
            theme_selector = theme_selector.child(
                div()
                    .flex_1()
                    .rounded(px(t::CONTROL_RADIUS))
                    .border_1()
                    .border_color(if active { p.accent_border } else { p.border })
                    .bg(if active { p.selected } else { p.modal_surface })
                    .child(
                        btn(pref.as_str(), pref.label())
                            .w_full()
                            .ghost()
                            .icon(ico)
                            .toggled(active)
                            .text_color(if active { p.primary } else { p.ink_secondary })
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.set_theme_pref(pref, w, cx);
                            })),
                    ),
            );
        }
        content = content
            .child(
                section_card(p)
                    .child(
                        text("Appearance")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(theme_selector),
            )
            .child(
                section_card(p)
                    .child(
                        text("Downloads & queue")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(
                        col()
                            .gap(px(6.))
                            .child(field_label("Default download folder", p))
                            .child(
                                row()
                                    .gap(px(8.))
                                    .child(
                                        div().flex_1().child(Input::new(&self.folder)),
                                    )
                                    .child(
                                        btn("settings-browse-folder", "Browse…")
                                            .icon(IconName::FolderOpen)
                                            .on_click(cx.listener(|_, _, w, cx| {
                                                let weak = cx.entity().downgrade();
                                                let dialog =
                                                    rfd::AsyncFileDialog::new()
                                                        .pick_folder();
                                                cx.spawn_in(w, async move |_, cx| {
                                                    if let Some(folder) = dialog.await {
                                                        let path = folder
                                                            .path()
                                                            .to_string_lossy()
                                                            .into_owned();
                                                        let _ = weak.update_in(
                                                            cx,
                                                            |s, w, cx| {
                                                                s.folder.update(
                                                                    cx,
                                                                    |v, cx| {
                                                                        v.set_value(
                                                                            path, w, cx,
                                                                        )
                                                                    },
                                                                );
                                                            },
                                                        );
                                                    }
                                                })
                                                .detach();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        row()
                            .gap(px(12.))
                            .child(
                                col()
                                    .flex_1()
                                    .gap(px(6.))
                                    .child(field_label(
                                        "HTTP/FTP connections (1–128)",
                                        p,
                                    ))
                                    .child(Input::new(&self.connections)),
                            )
                            .child(
                                col()
                                    .flex_1()
                                    .gap(px(6.))
                                    .child(field_label(
                                        "Concurrent queue tasks (1–16)",
                                        p,
                                    ))
                                    .child(Input::new(&self.max_concurrent)),
                            ),
                    )
                    .child(settings_description(
                        "The desktop service listens on this computer only. Downloads resume after restart.",
                        p,
                    )),
            )
            .child(
                section_card(p)
                    .child(
                        row()
                            .gap(px(12.))
                            .child(
                                text("Browser extension")
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink),
                            )
                            .child(
                                btn("browser-register", "Register host")
                                    .flex_shrink_0()
                                    .icon(IconName::Globe)
                                    .on_click(cx.listener(|s, _, _, _| {
                                        s.bridge.send("browser-register", json!({}))
                                    })),
                            ),
                    )
                    .child(settings_description(
                        "Connect the matching NDM extension for direct downloads and magnet links.",
                        p,
                    ))
                    .child(settings_description(
                        "GPL-3.0-or-later · Original author credits are preserved.",
                        p,
                    )),
            );
        content
    }

    fn speed_settings(&self, mut content: Div, p: &t::Palette) -> Div {
        content = content.child(
            section_card(p)
                .child(
                    text("Bandwidth & speed limits")
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.ink),
                )
                .child(
                    col()
                        .gap(px(6.))
                        .child(field_label(
                            "Global download speed limit (KB/s, 0 = unlimited)",
                            p,
                        ))
                        .child(Input::new(&self.global_dl_limit)),
                )
                .child(
                    col()
                        .gap(px(6.))
                        .child(field_label(
                            "Default per-download speed limit (KB/s, 0 = unlimited)",
                            p,
                        ))
                        .child(Input::new(&self.per_dl_limit)),
                )
                .child(
                    col()
                        .gap(px(6.))
                        .child(field_label(
                            "Global BitTorrent upload limit (KB/s, 0 = unlimited)",
                            p,
                        ))
                        .child(Input::new(&self.torrent_ul_limit)),
                )
                .child(
                    row()
                        .items_start()
                        .p(px(10.))
                        .gap(px(8.))
                        .rounded(px(8.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border_subtle)
                        .child(
                            row()
                                .h(px(18.))
                                .flex_shrink_0()
                                .child(icon(IconName::Info).size(px(14.)).text_color(p.primary)),
                        )
                        .child(
                            settings_description(
                                "Global limits apply immediately to active transfers.\nBitTorrent enforces a 16 KB/s minimum non-zero rate.",
                                p,
                            )
                            .flex_1(),
                        ),
                ),
        );
        content
    }

    fn torrent_settings(&self, mut content: Div, p: &t::Palette, cx: &mut Context<Self>) -> Div {
        let (tracker_status, badge_color, badge_bg) = if self.trackers_syncing {
            ("Syncing trackers…".to_owned(), p.primary, p.accent_soft)
        } else if self.extra_trackers_count > 0 {
            (
                format!(
                    "{} extra tracker{} loaded",
                    self.extra_trackers_count,
                    if self.extra_trackers_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
                p.success,
                p.success_soft,
            )
        } else {
            (
                "No extra trackers configured".to_owned(),
                p.muted,
                p.surface,
            )
        };

        content = content
            .child(
                section_card(p)
                    .child(
                        text("Seeding limits")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(switch_row(
                        "torrent-seed",
                        "Keep seeding after completion",
                        "Share completed torrent pieces until time or ratio limits are reached.",
                        self.torrent_seed,
                        p,
                        cx.listener(|s, v: &bool, _, cx| {
                            s.torrent_seed = *v;
                            cx.notify();
                        }),
                    ))
                    .child(
                        row()
                            .gap(px(12.))
                            .child(
                                col()
                                    .flex_1()
                                    .gap(px(6.))
                                    .child(field_label("Max seeding time (min, 0 = unlimited)", p))
                                    .child(Input::new(&self.torrent_seed_minutes)),
                            )
                            .child(
                                col()
                                    .flex_1()
                                    .gap(px(6.))
                                    .child(field_label("Max share ratio (0 = unlimited)", p))
                                    .child(Input::new(&self.torrent_seed_ratio)),
                            ),
                    ),
            )
            .child(
                section_card(p)
                    .child(
                        text("Network & peers")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink),
                    )
                    .child(
                        col()
                            .gap(px(6.))
                            .child(field_label("Max peers per torrent (10–500)", p))
                            .child(Input::new(&self.torrent_peer_limit)),
                    )
                    .child(
                        row()
                            .gap(px(10.))
                            .child(div().flex_1().child(switch_row(
                                "torrent-dht",
                                "Enable DHT",
                                "Decentralized peer lookup",
                                self.torrent_dht,
                                p,
                                cx.listener(|s, v: &bool, _, cx| {
                                    s.torrent_dht = *v;
                                    cx.notify();
                                }),
                            )))
                            .child(div().flex_1().child(switch_row(
                                "torrent-utp",
                                "Enable uTP",
                                "UDP transport protocol",
                                self.torrent_utp,
                                p,
                                cx.listener(|s, v: &bool, _, cx| {
                                    s.torrent_utp = *v;
                                    cx.notify();
                                }),
                            ))),
                    )
                    .child(helper_text(
                        "Network & peer settings apply when the next torrent session starts.",
                        p,
                    )),
            )
            .child(
                section_card(p)
                    .child(
                        row()
                            .justify_between()
                            .child(
                                text("Torrent trackers")
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink),
                            )
                            .child(
                                text(tracker_status)
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(badge_color)
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded_full()
                                    .bg(badge_bg)
                                    .border_1()
                                    .border_color(p.border_subtle),
                            ),
                    )
                    .child(helper_text(
                        "Extra tracker subscription URLs or direct trackers, one per line.",
                        p,
                    ))
                    .child(Textarea::new(&self.tracker_urls).h(px(96.)))
                    .child(
                        row().justify_end().child(
                            btn(
                                "sync-trackers",
                                if self.trackers_syncing {
                                    "Syncing…"
                                } else {
                                    "Sync trackers"
                                },
                            )
                            .icon(IconName::RefreshCw)
                            .disabled(self.trackers_syncing)
                            .on_click(cx.listener(|s, _, _, cx| {
                                let urls = parse_tracker_urls(&s.tracker_urls.read(cx).value());
                                s.trackers_syncing = true;
                                s.trackers_error = None;
                                s.notice = None;
                                s.bridge.send(
                                    "trackers-sync",
                                    json!({"op":"sync_trackers","tracker_urls":urls}),
                                );
                                cx.notify();
                            })),
                        ),
                    )
                    .when_some(self.trackers_error.as_ref(), |this, err| {
                        this.child(
                            row()
                                .p(px(9.))
                                .gap(px(8.))
                                .rounded(px(7.))
                                .bg(p.danger_soft)
                                .border_1()
                                .border_color(p.danger_border)
                                .child(
                                    icon(IconName::CircleAlert)
                                        .size(px(14.))
                                        .text_color(p.danger),
                                )
                                .child(
                                    text(err.clone())
                                        .flex_1()
                                        .text_size(px(11.5))
                                        .text_color(p.danger),
                                ),
                        )
                    }),
            );
        content
    }
}
