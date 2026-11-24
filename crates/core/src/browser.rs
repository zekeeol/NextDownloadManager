//! Browser adapter commands. No window or extension runtime belongs in core.
use super::*;
impl Core {
    pub(super) fn browser(&mut self, mut command: Command) -> Value {
        if self.browser_events.len() >= 128 {
            return error("Open NDM and finish pending browser requests first");
        }
        if command.video_page.is_some()
            || command.video_lookup.is_some()
            || command.video_pick.is_some()
        {
            return error(
                "Stream extraction is not available in the native desktop. Download direct media files instead.",
            );
        }
        if command.links_part.is_some() {
            return error("This link selection is too large. Send a smaller selection.");
        }
        if let Some(site) = command
            .exclude_site
            .as_ref()
            .or(command.include_site.as_ref())
        {
            let site = site.trim().to_ascii_lowercase();
            if site.is_empty()
                || site.len() > 253
                || site
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
            {
                return error("Invalid site name");
            }
            let mut rules = std::fs::read(self.data.join("browser-rules.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<ndm_ipc::capture::CaptureRules>(&b).ok())
                .unwrap_or_default();
            let mut sites: Vec<_> = rules
                .excluded_sites
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            sites.retain(|s| !s.eq_ignore_ascii_case(&site));
            if command.exclude_site.is_some() {
                sites.push(site);
            }
            rules.excluded_sites = sites.join(" ");
            return match serde_json::to_vec(&rules)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    ndm_ipc::private_write(&self.data.join("browser-rules.json"), &b)
                        .map_err(|e| e.to_string())
                }) {
                Ok(()) => json!({"ok":true}),
                Err(e) => error(&e),
            };
        }
        if command
            .show
            .as_deref()
            .is_some_and(|s| s.starts_with("options"))
        {
            self.browser_events.push_back(json!({"kind":"settings"}));
            return json!({"ok":true});
        }
        if command.panel_hide_site.is_some() || command.panel_show_site.is_some() {
            return error("Video panels are not available in the native desktop");
        }
        if !command.links.is_empty() {
            if command.links.len() > 100 {
                return error("Send at most 100 links at once");
            }
            if self.browser_events.len() + command.links.len() > 128 {
                return error("Open NDM and finish pending browser requests first");
            }
            // Validate the entire selection before accepting it.
            if command.links.iter().any(|s| !browser_url(s)) {
                return error("Unsupported link in selection");
            }
            let links = std::mem::take(&mut command.links);
            let mut ids = Vec::new();
            for url in links {
                let mut item = command.clone();
                item.url = Some(url);
                // A page's Cookie header is not a cookie jar for unrelated link hosts.
                if item
                    .url
                    .as_deref()
                    .and_then(|s| Url::parse(s).ok())
                    .zip(item.referer.as_deref().and_then(|s| Url::parse(s).ok()))
                    .is_none_or(|(u, r)| u.origin() != r.origin())
                {
                    item.cookies = None;
                    item.cookie_jar.clear();
                }
                let result = self.browser(item);
                if result.get("error").is_some() {
                    return result;
                }
                ids.push(result["id"].clone());
            }
            return json!({"ok":true,"ids":ids});
        }
        if let Some(url) = command.url.clone() {
            if !browser_url(&url) {
                return error("Unsupported browser download address");
            }
            if url.to_ascii_lowercase().starts_with("magnet:?") {
                self.browser_events
                    .push_back(json!({"kind":"torrent","source":url}));
                return json!({"ok":true});
            }
            if command.proxy_login {
                return error("This proxy needs credentials. Keep this download in the browser.");
            }
            if command.browser_handoff
                && let Some(r) = self
                    .records
                    .iter()
                    .find(|r| r.info.url == url && r.info.status != "Error")
            {
                return json!({"ok":true,"id":r.info.id});
            }
            let torrent = ndm_ipc::capture::is_torrent(
                &url,
                command.name.as_deref().unwrap_or(""),
                command.mime.as_deref(),
            );
            let added = self.add(command);
            if let Some(id) = added["id"].as_u64() {
                if let Some(r) = self.records.iter_mut().find(|r| r.info.id == id) {
                    if r.info.status == "Error" {
                        return error(
                            r.info
                                .error
                                .as_deref()
                                .unwrap_or("Cannot start browser download"),
                        );
                    }
                    r.open_torrent = torrent;
                }
                self.browser_events
                    .push_back(json!({"kind":"download","id":id}));
                self.dirty = true;
                self.save();
                return json!({"ok":true,"id":id});
            }
            return added;
        }
        if let Some(address) = command.show_download {
            let id = self
                .records
                .iter()
                .find(|r| r.info.url == address)
                .map(|r| r.info.id);
            self.browser_events
                .push_back(json!({"kind":"download","id":id}));
        } else {
            self.browser_events.push_back(json!({"kind":"open"}));
        }
        json!({"ok":true})
    }
}
fn browser_url(s: &str) -> bool {
    Url::parse(s).is_ok_and(|u| ndm_engine::supported(&u) || u.scheme() == "magnet")
}
