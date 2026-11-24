//! Which of a torrent's trackers NDM lets librqbit contact (#40). A torrent names its trackers,
//! and a tracker on this computer or the local network (`127.0.0.1`, `192.168.1.1`, `localhost`)
//! lets a torrent make NDM send requests into the network it sits in: those are dropped. With a
//! proxy, UDP trackers are dropped too: librqbit sends them around the proxy.
//!
//! The list is changed in the torrent itself (`announce`, `announce-list`, outside the info
//! dictionary, so its hash stays the same) and in a magnet link (`tr=`).

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

/// Maximum size of a remote tracker subscription list (2 MiB).
pub const MAX_SUBSCRIPTION_BYTES: usize = 2 * 1024 * 1024;

/// Timeout for fetching a remote tracker subscription list.
pub const SUBSCRIPTION_TIMEOUT: Duration = Duration::from_secs(15);

/// Whether NDM may contact `tracker`.
pub fn allowed(tracker: &str, proxied: bool) -> bool {
    let Ok(url) = url::Url::parse(tracker.trim()) else {
        return false;
    };
    match url.scheme() {
        "http" | "https" => {}
        "udp" if !proxied => {}
        _ => return false,
    }
    match url.host() {
        Some(url::Host::Domain(d)) => {
            if let Ok(ip) = d.trim_matches(['[', ']']).parse::<IpAddr>() {
                return public(ip);
            }
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            !(d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local"))
        }
        Some(url::Host::Ipv4(a)) => public(IpAddr::V4(a)),
        Some(url::Host::Ipv6(a)) => public(IpAddr::V6(a)),
        None => false,
    }
}

/// An address on the internet (not this computer, the local network or a reserved range).
pub(crate) fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_loopback()
                || a.is_private()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || a.is_multicast()
                // 0.0.0.0/8, "this network": 0.0.0.1 reaches the machine's own stack.
                || o[0] == 0
                // 100.64.0.0/10, carrier-grade NAT.
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                // 198.18.0.0/15, the benchmark ranges.
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                // 240.0.0.0/4, reserved (255.255.255.255 is the broadcast, above).
                || o[0] >= 240)
        }
        IpAddr::V6(a) => {
            let s = a.segments();
            // IPv4 inside IPv6 that isn't the mapped form: compatible (::a.b.c.d) and NAT64
            // (64:ff9b::a.b.c.d). Whichever it is, its IPv4 rules decide.
            let embedded = (s[..6] == [0; 6] && !a.is_unspecified() && !a.is_loopback())
                .then_some([s[6], s[7]])
                .or_else(|| {
                    (s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0; 4]).then_some([s[6], s[7]])
                });
            if let Some([hi, lo]) = embedded {
                let v4 = Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
                return public(IpAddr::V4(v4));
            }
            !(a.is_loopback()
                || a.is_unspecified()
                || a.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local
                || (s[0] & 0xffc0) == 0xfe80 // link local
                || a.to_ipv4_mapped().is_some_and(|v4| !public(IpAddr::V4(v4))))
        }
    }
}

/// A magnet link's trackers (`tr=`), each once, in its order.
pub fn in_magnet(magnet: &str) -> Vec<String> {
    let Some((_, query)) = magnet.split_once('?') else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        let v = v.trim();
        if k.eq_ignore_ascii_case("tr") && !v.is_empty() && !out.iter().any(|t| t == v) {
            out.push(v.to_owned());
        }
    }
    out
}

/// A magnet link with only the trackers NDM may contact.
pub fn filter_magnet(magnet: &str, proxied: bool) -> String {
    let Some((head, query)) = magnet.split_once('?') else {
        return magnet.to_owned();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|kv| match kv.split_once('=') {
            Some((k, v)) if k.eq_ignore_ascii_case("tr") => {
                let v = url::form_urlencoded::parse(format!("x={v}").as_bytes())
                    .next()
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                allowed(&v, proxied)
            }
            _ => true,
        })
        .collect();
    format!("{head}?{}", kept.join("&"))
}

/// A .torrent with only the trackers NDM may contact; everything else byte for byte, so its
/// info hash is the same. None when it isn't a bencoded dictionary.
pub fn filter_torrent(bytes: &[u8], proxied: bool) -> Option<Vec<u8>> {
    let entries = top_level(bytes)?;
    // An `announce-list` replaces `announce` ([`listed`]): when none of its trackers is kept,
    // `announce` goes too, or the torrent would ask a tracker it never named for its peers.
    let list = entries
        .iter()
        .find(|(k, _)| *k == b"announce-list")
        .map(|e| e.1);
    let replaced = list.and_then(list_of_lists).is_some_and(|t| !t.is_empty());
    let none_kept = match list {
        Some(v) => kept(v, proxied)?.is_empty(),
        None => false,
    };
    let mut out = b"d".to_vec();
    for (key, value) in entries {
        match key {
            b"announce" => {
                let url = string_at(value)?;
                if allowed(&String::from_utf8_lossy(url), proxied) && !(replaced && none_kept) {
                    push_string(&mut out, key);
                    out.extend_from_slice(value);
                }
            }
            b"announce-list" => {
                let tiers = kept(value, proxied)?;
                if !tiers.is_empty() {
                    push_string(&mut out, key);
                    out.push(b'l');
                    for tier in tiers {
                        out.push(b'l');
                        for u in tier {
                            push_string(&mut out, u);
                        }
                        out.push(b'e');
                    }
                    out.push(b'e');
                }
            }
            _ => {
                push_string(&mut out, key);
                out.extend_from_slice(value);
            }
        }
    }
    out.push(b'e');
    Some(out)
}

/// An `announce-list`'s tiers with only the trackers NDM may contact, and only the tiers left
/// with one. None when it isn't a list of lists.
fn kept(value: &[u8], proxied: bool) -> Option<Vec<Vec<&[u8]>>> {
    Some(
        list_of_lists(value)?
            .into_iter()
            .map(|tier| {
                tier.into_iter()
                    .filter(|u| allowed(&String::from_utf8_lossy(u), proxied))
                    .collect::<Vec<_>>()
            })
            .filter(|tier| !tier.is_empty())
            .collect(),
    )
}

/// A .torrent's trackers, each once, in its order: `announce-list`'s tiers, or `announce` when
/// it has no list (BEP 12: a list replaces it).
pub fn listed(bytes: &[u8]) -> Vec<String> {
    let Some(entries) = top_level(bytes) else {
        return Vec::new();
    };
    let value = |k: &[u8]| entries.iter().find(|(key, _)| *key == k).map(|e| e.1);
    let urls: Vec<&[u8]> = match value(b"announce-list").and_then(list_of_lists) {
        Some(tiers) if !tiers.is_empty() => tiers.into_iter().flatten().collect(),
        _ => value(b"announce").and_then(string_at).into_iter().collect(),
    };
    let mut out: Vec<String> = Vec::new();
    for u in urls {
        let u = String::from_utf8_lossy(u).trim().to_owned();
        if !u.is_empty() && !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

/// A .torrent with `more` trackers (each a tier of its own, after the ones it has; those it
/// has already are skipped); everything else byte for byte, so its info hash is the same. One
/// with only `announce` gets a list holding it first; one with none gets `announce` too. None
/// when it isn't a bencoded dictionary.
pub fn add(bytes: &[u8], more: &[String]) -> Option<Vec<u8>> {
    let entries = top_level(bytes)?;
    let have = listed(bytes);
    let mut fresh: Vec<&str> = Vec::new();
    for u in more.iter().map(|u| u.trim()) {
        if !u.is_empty() && !have.iter().any(|h| h == u) && !fresh.contains(&u) {
            fresh.push(u);
        }
    }
    if fresh.is_empty() {
        return Some(bytes.to_vec());
    }
    let mut tiers: Vec<Vec<&[u8]>> = match entries
        .iter()
        .find(|(k, _)| *k == b"announce-list")
        .map(|e| list_of_lists(e.1))
    {
        Some(None) => return None,
        Some(Some(list)) if !list.is_empty() => list,
        _ => have.iter().map(|u| vec![u.as_bytes()]).collect(),
    };
    tiers.extend(fresh.iter().map(|u| vec![u.as_bytes()]));
    let mut list = b"l".to_vec();
    for tier in &tiers {
        list.push(b'l');
        for u in tier {
            push_string(&mut list, u);
        }
        list.push(b'e');
    }
    list.push(b'e');
    let mut out: Vec<(&[u8], Vec<u8>)> = entries
        .into_iter()
        .filter(|(k, _)| *k != b"announce-list")
        .map(|(k, v)| (k, v.to_vec()))
        .collect();
    out.push((b"announce-list", list));
    if !out.iter().any(|(k, _)| *k == b"announce") {
        let mut first = Vec::new();
        push_string(&mut first, fresh[0].as_bytes());
        out.push((b"announce", first));
    }
    // A dictionary's keys are sorted (bencode); the info dictionary's bytes are untouched.
    out.sort_by(|a, b| a.0.cmp(b.0));
    let mut done = b"d".to_vec();
    for (k, v) in out {
        push_string(&mut done, k);
        done.extend_from_slice(&v);
    }
    done.push(b'e');
    Some(done)
}

/// Whether `url` is a direct tracker endpoint rather than a tracker subscription list URL.
pub fn is_direct_tracker(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url.trim()) else {
        return false;
    };
    match parsed.scheme() {
        "udp" => true,
        "http" | "https" => {
            let path_lower = parsed.path().to_ascii_lowercase();
            if [".txt", ".list", ".lst", ".md", ".csv", ".json"]
                .iter()
                .any(|ext| path_lower.ends_with(ext))
            {
                return false;
            }
            parsed.path_segments().is_some_and(|mut segs| {
                segs.any(|seg| {
                    let s = seg.to_ascii_lowercase();
                    s == "announce" || s.starts_with("announce.")
                })
            })
        }
        _ => false,
    }
}

/// Validate a user-configured tracker source URL (either a subscription list URL or a direct tracker).
pub fn validate_source_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Tracker URL cannot be empty".into());
    }
    let parsed =
        url::Url::parse(trimmed).map_err(|e| format!("Invalid tracker URL '{trimmed}': {e}"))?;
    match parsed.scheme() {
        "http" | "https" | "udp" => {}
        scheme => {
            return Err(format!(
                "Unsupported tracker URL scheme '{scheme}' in '{trimmed}' (use http, https, or udp)"
            ));
        }
    }
    if parsed.host().is_none() {
        return Err(format!("Tracker URL '{trimmed}' is missing a host"));
    }
    if is_direct_tracker(trimmed) && !allowed(trimmed, false) {
        return Err(format!(
            "Direct tracker '{trimmed}' points to a local, private, or disallowed address"
        ));
    }
    Ok(trimmed.to_owned())
}

/// Parse user-configured tracker source URLs from multi-line or whitespace-separated text,
/// ignoring empty lines and `#` comments, deduplicating while preserving order.
pub fn parse_config_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for token in line.split_whitespace() {
            let token = token.trim();
            if !token.is_empty() && seen.insert(token.to_owned()) {
                out.push(token.to_owned());
            }
        }
    }
    out
}

/// Parse a plain-text tracker list (newline, blank-line, `#`-comment, or comma-separated),
/// keeping only trackers NDM may contact, deduplicated while preserving order.
pub fn parse_list(text: &str, proxied: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for token in line.split(|c: char| c.is_whitespace() || c == ',') {
            let token = token.trim();
            if !token.is_empty() && allowed(token, proxied) && seen.insert(token.to_owned()) {
                out.push(token.to_owned());
            }
        }
    }
    out
}

/// Return a magnet link with `more` trackers appended (each percent-encoded), skipping any
/// trackers already present in `magnet`.
pub fn add_to_magnet(magnet: &str, more: &[String]) -> String {
    let mut existing: HashSet<String> = in_magnet(magnet).into_iter().collect();
    let mut fresh = Vec::new();
    for t in more {
        let t = t.trim();
        if !t.is_empty() && existing.insert(t.to_owned()) {
            fresh.push(t);
        }
    }
    if fresh.is_empty() {
        return magnet.to_owned();
    }
    let mut out = magnet.to_owned();
    for t in fresh {
        if !out.contains('?') {
            out.push('?');
        } else if !out.ends_with('?') && !out.ends_with('&') {
            out.push('&');
        }
        let enc: String = url::form_urlencoded::byte_serialize(t.as_bytes()).collect();
        out.push_str("tr=");
        out.push_str(&enc);
    }
    out
}

/// Replace all `announce` and `announce-list` entries in a bencoded `.torrent` with `trackers`,
/// preserving the `info` dictionary and all other keys byte-for-byte.
pub fn replace(bytes: &[u8], trackers: &[String]) -> Option<Vec<u8>> {
    let entries = top_level(bytes)?;
    let mut stripped = b"d".to_vec();
    for (k, v) in entries {
        if k == b"announce" || k == b"announce-list" {
            continue;
        }
        push_string(&mut stripped, k);
        stripped.extend_from_slice(v);
    }
    stripped.push(b'e');
    if trackers.is_empty() {
        Some(stripped)
    } else {
        add(&stripped, trackers)
    }
}

/// Result of resolving one or more tracker source URLs (subscription lists and/or direct trackers).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FetchOutcome {
    pub trackers: Vec<String>,
    pub succeeded: usize,
    pub failed: Vec<(String, String)>,
}

async fn fetch_one_source(
    client: &reqwest::Client,
    src: &str,
    proxied: bool,
) -> Result<Vec<String>, String> {
    if is_direct_tracker(src) {
        if allowed(src, proxied) {
            return Ok(vec![src.to_owned()]);
        }
        return Err("tracker address is not allowed".to_owned());
    }
    let Ok(parsed) = url::Url::parse(src) else {
        return Err("invalid tracker URL".to_owned());
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("unsupported tracker URL scheme".to_owned());
    }
    let mut resp = client.get(src).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if buf.len() + chunk.len() > MAX_SUBSCRIPTION_BYTES {
            return Err("tracker list exceeds 2 MiB limit".to_owned());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(parse_list(&String::from_utf8_lossy(&buf), proxied))
}

/// Fetch and merge trackers from multiple subscription list URLs or direct tracker URLs.
pub async fn fetch_sources(sources: &[String], proxied: bool) -> FetchOutcome {
    let mut unique = Vec::new();
    let mut seen_src = HashSet::new();
    for s in sources {
        let s = s.trim();
        if !s.is_empty() && seen_src.insert(s.to_owned()) {
            unique.push(s.to_owned());
        }
    }
    if unique.is_empty() {
        return FetchOutcome::default();
    }
    let client = match ndm_engine::tls::configure(reqwest::Client::builder())
        .timeout(SUBSCRIPTION_TIMEOUT)
        .user_agent(format!("NextDownloadManager/{}", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return FetchOutcome {
                trackers: Vec::new(),
                succeeded: 0,
                failed: unique
                    .into_iter()
                    .map(|u| (u, format!("HTTP client error: {e}")))
                    .collect(),
            };
        }
    };

    let results = futures::future::join_all(
        unique
            .iter()
            .map(|src| fetch_one_source(&client, src, proxied)),
    )
    .await;

    let mut outcome = FetchOutcome::default();
    let mut seen_trackers = HashSet::new();
    for (src, res) in unique.into_iter().zip(results) {
        match res {
            Ok(list) => {
                outcome.succeeded += 1;
                for t in list {
                    if seen_trackers.insert(t.clone()) {
                        outcome.trackers.push(t);
                    }
                }
            }
            Err(err) => outcome.failed.push((src, err)),
        }
    }
    outcome
}

fn push_string(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(s.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(s);
}

/// The top-level dictionary's entries: each key and its value's bytes.
fn top_level(b: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    if b.first() != Some(&b'd') {
        return None;
    }
    let mut at = 1;
    let mut out = Vec::new();
    while *b.get(at)? != b'e' {
        let (key, next) = string(b, at)?;
        let end = skip(b, next, 0)?;
        out.push((key, &b[next..end]));
        at = end;
    }
    (at + 1 == b.len()).then_some(out)
}

/// A string at `at`: its bytes, and where it ends.
fn string(b: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let colon = at + b.get(at..)?.iter().position(|&c| c == b':')?;
    let len: usize = std::str::from_utf8(&b[at..colon]).ok()?.parse().ok()?;
    let start = colon + 1;
    let end = start.checked_add(len)?;
    Some((b.get(start..end)?, end))
}

fn string_at(value: &[u8]) -> Option<&[u8]> {
    let (s, end) = string(value, 0)?;
    (end == value.len()).then_some(s)
}

fn list_of_lists(value: &[u8]) -> Option<Vec<Vec<&[u8]>>> {
    if value.first() != Some(&b'l') {
        return None;
    }
    let mut at = 1;
    let mut tiers = Vec::new();
    while *value.get(at)? != b'e' {
        if value[at] != b'l' {
            return None;
        }
        at += 1;
        let mut tier = Vec::new();
        while *value.get(at)? != b'e' {
            let (s, next) = string(value, at)?;
            tier.push(s);
            at = next;
        }
        at += 1;
        tiers.push(tier);
    }
    Some(tiers)
}

/// Where the value at `at` ends; nesting is limited (librqbit 9.0.1's crash was unbounded depth).
fn skip(b: &[u8], at: usize, depth: usize) -> Option<usize> {
    if depth > 64 {
        return None;
    }
    match *b.get(at)? {
        b'i' => Some(at + b.get(at..)?.iter().position(|&c| c == b'e')? + 1),
        b'l' | b'd' => {
            let mut at = at + 1;
            while *b.get(at)? != b'e' {
                at = skip(b, at, depth + 1)?;
            }
            Some(at + 1)
        }
        b'0'..=b'9' => string(b, at).map(|(_, end)| end),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_and_odd_trackers_are_dropped() {
        for ok in [
            "http://tracker.example.org:80/announce",
            "https://t.example/announce",
            "udp://tracker.opentrackr.org:1337/announce",
            "http://8.8.8.8/announce",
        ] {
            assert!(allowed(ok, false), "{ok}");
        }
        for bad in [
            "http://127.0.0.1:8080/announce",
            "http://localhost/announce",
            "http://router.local/announce",
            "http://192.168.1.1/announce",
            "http://10.0.0.5/a",
            "http://169.254.1.1/a",
            "http://100.64.0.1/a",
            "http://[::1]/a",
            "http://[fd00::1]/a",
            "http://[::ffff:192.168.0.1]/a",
            "file:///etc/passwd",
            "wss://t.example/a",
            "not a url",
        ] {
            assert!(!allowed(bad, false), "{bad}");
        }
        assert!(
            !allowed("udp://tracker.example:1337/a", true),
            "UDP goes around the proxy"
        );
        assert!(allowed("https://t.example/a", true));
    }

    /// "This" network, the reserved ranges and IPv4 addresses written as IPv6 reach this
    /// machine or go nowhere: no tracker there gets a request.
    #[test]
    fn this_networks_and_reserved_addresses_are_dropped() {
        for bad in [
            "http://0.0.0.1/announce",
            "http://0.255.255.254/announce",
            "http://240.0.0.1/announce",
            "http://192.0.0.170/announce",
            "http://198.18.0.1/announce",
            "http://198.19.255.254/announce",
            "http://[::7f00:1]/announce",
            "http://[64:ff9b::a00:1]/announce",
        ] {
            assert!(!allowed(bad, false), "{bad}");
        }
        // Their public neighbours still pass.
        assert!(allowed("http://192.0.1.1/announce", false));
        assert!(allowed("http://198.20.0.1/announce", false));
        assert!(allowed("http://[2001:4860:4860::8888]/announce", false));
    }

    #[test]
    fn magnets_lose_only_the_trackers_dropped() {
        let m = "magnet:?xt=urn:btih:abc&dn=x&tr=udp%3A%2F%2Ft.example%3A1337&tr=http%3A%2F%2F127.0.0.1%2Fa";
        assert_eq!(
            filter_magnet(m, false),
            "magnet:?xt=urn:btih:abc&dn=x&tr=udp%3A%2F%2Ft.example%3A1337"
        );
        assert_eq!(filter_magnet(m, true), "magnet:?xt=urn:btih:abc&dn=x");
        assert_eq!(
            in_magnet(&format!("{m}&tr=udp%3A%2F%2Ft.example%3A1337&tr=")),
            ["udp://t.example:1337", "http://127.0.0.1/a"]
        );
        assert!(in_magnet("magnet:?xt=urn:btih:abc").is_empty());
    }

    /// Add trackers (#39): new ones after the ones there, each once, keys still sorted, the info
    /// dictionary byte for byte.
    #[test]
    fn trackers_are_added_after_the_ones_there() {
        let info = b"d6:lengthi5e4:name1:a12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaae";
        let s = |v: &str| format!("{}:{v}", v.len());
        let with = |head: &str| {
            let mut t = format!("d{head}4:info").into_bytes();
            t.extend(info);
            t.push(b'e');
            t
        };
        let (a, b, c) = (
            "http://a.example/announce",
            "udp://b.example:1337/announce",
            "https://c.example/announce",
        );
        let more = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        // Only `announce`: a list with it first, then the new ones, each a tier.
        let one = with(&format!("8:announce{}", s(a)));
        let out = add(&one, &more(&[b, a, b, " ", c])).unwrap();
        assert_eq!(
            out,
            with(&format!(
                "8:announce{}13:announce-listll{}el{}el{}ee",
                s(a),
                s(a),
                s(b),
                s(c)
            ))
        );
        assert_eq!(listed(&out), [a, b, c]);

        // A list already: kept as it is, the new one after; a comment key stays in its place.
        let listed_one = with(&format!(
            "8:announce{}13:announce-listll{}{}ee7:comment2:hi",
            s(a),
            s(a),
            s(b)
        ));
        let out = add(&listed_one, &more(&[c])).unwrap();
        assert_eq!(
            out,
            with(&format!(
                "8:announce{}13:announce-listll{}{}el{}ee7:comment2:hi",
                s(a),
                s(a),
                s(b),
                s(c)
            ))
        );

        // None at all (a magnet's): `announce` and a list.
        let bare = with("");
        let out = add(&bare, &more(&[b])).unwrap();
        assert_eq!(
            out,
            with(&format!("8:announce{}13:announce-listll{}ee", s(b), s(b)))
        );
        // Nothing new: the same bytes.
        assert_eq!(add(&out, &more(&[b])).unwrap(), out);
        assert_eq!(add(b"junk", &more(&[a])), None);

        // The hash is the same: the info bytes are untouched.
        let at = |t: &[u8]| t.windows(info.len()).position(|w| w == info).is_some();
        assert!(at(&add(&one, &more(&[c])).unwrap()));
    }

    /// The trackers change; the info dictionary, and so the hash, doesn't.
    #[test]
    fn torrents_keep_their_info_byte_for_byte() {
        let info = b"d6:lengthi5e4:name1:a12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaae";
        let s = |v: &str| format!("{}:{v}", v.len());
        let (local, udp) = ("http://127.0.0.1/announce", "udp://t.example:1337/a");
        let mut t = format!(
            "d8:announce{}13:announce-listll{}el{}ee4:info",
            s(local),
            s(local),
            s(udp)
        )
        .into_bytes();
        t.extend(info);
        t.push(b'e');
        let out = filter_torrent(&t, false).unwrap();
        let mut want = format!("d13:announce-listll{}ee4:info", s(udp)).into_bytes();
        want.extend(info);
        want.push(b'e');
        assert_eq!(out, want);
        // With a proxy nothing is left: no tracker keys at all.
        let mut bare = b"d4:info".to_vec();
        bare.extend(info);
        bare.push(b'e');
        assert_eq!(filter_torrent(&t, true).unwrap(), bare);
        assert_eq!(filter_torrent(b"not bencode", false), None);
        assert_eq!(filter_torrent(b"d4:infoi1e", false), None, "unterminated");
    }

    /// Filtering never brings a tracker in: an `announce` that a list replaced stays unused.
    #[test]
    fn filtering_only_takes_trackers_away() {
        let s = |v: &str| format!("{}:{v}", v.len());
        let (public, local) = ("http://t.example/announce", "http://127.0.0.1/announce");
        // The list names only trackers NDM won't ask: `announce` (not used) doesn't come back.
        let t = format!("d8:announce{}13:announce-listll{}eee", s(public), s(local));
        let out = filter_torrent(t.as_bytes(), false).unwrap();
        assert_eq!(listed(t.as_bytes()), [local]);
        assert!(listed(&out).is_empty(), "{}", String::from_utf8_lossy(&out));
        assert_eq!(out, b"de");
        // An empty tier is a list all the same.
        let t = format!("d8:announce{}13:announce-listlleee", s(public));
        assert!(listed(&filter_torrent(t.as_bytes(), false).unwrap()).is_empty());
        // With a tracker of the list kept, or no list at all, `announce` stays as it was.
        let t = format!(
            "d8:announce{}13:announce-listll{}{}eee",
            s(public),
            s(local),
            s(public)
        );
        assert_eq!(
            listed(&filter_torrent(t.as_bytes(), false).unwrap()),
            [public]
        );
        let t = format!("d8:announce{}e", s(public));
        assert_eq!(filter_torrent(t.as_bytes(), false).unwrap(), t.as_bytes());
    }

    #[test]
    fn direct_and_subscription_urls_are_distinguished_and_validated() {
        assert!(!is_direct_tracker(
            "https://raw.githubusercontent.com/adysec/tracker/main/trackers_best.txt"
        ));
        assert!(!is_direct_tracker("http://127.0.0.1:8080/trackers.txt"));
        assert!(!is_direct_tracker("https://example.com/list"));
        assert!(is_direct_tracker("udp://tracker.ducks.party:1984/announce"));
        assert!(is_direct_tracker("http://tracker.example.org:80/announce"));
        assert!(is_direct_tracker("https://t.example/announce.php"));

        assert!(
            validate_source_url(
                "https://raw.githubusercontent.com/adysec/tracker/main/trackers_best.txt"
            )
            .is_ok()
        );
        assert!(validate_source_url("http://127.0.0.1:8080/list.txt").is_ok());
        assert!(validate_source_url("udp://tracker.ducks.party:1984/announce").is_ok());
        assert!(validate_source_url("").is_err());
        assert!(validate_source_url("file:///etc/passwd").is_err());
        assert!(validate_source_url("udp://127.0.0.1:1337/announce").is_err());
        assert!(validate_source_url("not-a-url").is_err());
    }

    #[test]
    fn tracker_lists_are_parsed_filtered_and_deduplicated() {
        let raw = "\
# comment line
http://tracker.one/announce

udp://tracker.two:6969/announce, https://tracker.three/announce
http://127.0.0.1/announce
wss://ws.example/announce
udp://tracker.two:6969/announce
";
        assert_eq!(
            parse_list(raw, false),
            [
                "http://tracker.one/announce",
                "udp://tracker.two:6969/announce",
                "https://tracker.three/announce",
            ]
        );
        assert_eq!(
            parse_list(raw, true),
            [
                "http://tracker.one/announce",
                "https://tracker.three/announce",
            ]
        );
    }

    #[test]
    fn magnet_and_torrent_replace_preserve_identity_and_dedup() {
        let m = "magnet:?xt=urn:btih:6384d53c65bc94efeffc2447db33d25d4a8e139b&dn=test&tr=udp%3A%2F%2Ftracker.ducks.party%3A1984%2Fannounce";
        let extra = vec![
            "udp://tracker.ducks.party:1984/announce".into(),
            "https://t1.example/announce".into(),
            "http://t2.example:80/announce".into(),
        ];
        let out = add_to_magnet(m, &extra);
        assert_eq!(
            in_magnet(&out),
            [
                "udp://tracker.ducks.party:1984/announce",
                "https://t1.example/announce",
                "http://t2.example:80/announce",
            ]
        );

        let info = b"d6:lengthi5e4:name1:a12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaae";
        let s = |v: &str| format!("{}:{v}", v.len());
        let orig_t = "http://old.example/announce";
        let mut t = format!("d8:announce{}4:info", s(orig_t)).into_bytes();
        t.extend(info);
        t.push(b'e');
        let replaced = replace(&t, &["https://new.example/announce".into()]).unwrap();
        assert_eq!(listed(&replaced), ["https://new.example/announce"]);
        assert!(replaced.windows(info.len()).any(|w| w == info));
        let cleared = replace(&t, &[]).unwrap();
        assert!(listed(&cleared).is_empty());
        assert!(cleared.windows(info.len()).any(|w| w == info));
    }

    #[test]
    fn fetch_sources_merges_multiple_lists_and_tolerates_partial_failure() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                for _ in 0..3 {
                    let (mut sock, _) = listener.accept().await.unwrap();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 1024];
                        let n = sock.read(&mut buf).await.unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let resp = if req.contains("GET /a.txt") {
                            let body = "http://t1.example/announce\nhttp://127.0.0.1/bad\n";
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            )
                        } else if req.contains("GET /b.txt") {
                            let body = "http://t1.example/announce\nudp://t2.example:1337/announce\nwss://skip.example/a\n";
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            )
                        } else {
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                .to_owned()
                        };
                        let _ = sock.write_all(resp.as_bytes()).await;
                    });
                }
            });

            let sources = vec![
                format!("http://{addr}/a.txt"),
                format!("http://{addr}/b.txt"),
                format!("http://{addr}/missing.txt"),
                "https://direct.example/announce".to_owned(),
            ];
            let outcome = fetch_sources(&sources, false).await;
            assert_eq!(outcome.succeeded, 3);
            assert_eq!(outcome.failed.len(), 1);
            assert_eq!(
                outcome.trackers,
                [
                    "http://t1.example/announce",
                    "udp://t2.example:1337/announce",
                    "https://direct.example/announce",
                ]
            );
            server.await.unwrap();
        });
    }
}
