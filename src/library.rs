//! STPRO library core: discovers recently-used files, flags those that match the
//! `S00E00` episode pattern (case-insensitive), and aggregates them into series
//! so we can tell the user what they watched last and what to watch next.

use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

/// A single episode file the scanner flagged.
#[derive(Clone, Debug)]
pub struct Episode {
    pub season: u32,
    pub number: u32,
    /// ISO-8601 timestamp string. These sort lexicographically, so plain string
    /// comparison gives correct chronological ordering — no date parsing needed.
    pub visited: String,
    pub path: String,
    pub filename: String,
}

impl Episode {
    /// `S01E05` style tag.
    pub fn tag(&self) -> String {
        format!("S{:02}E{:02}", self.season, self.number)
    }
}

/// An aggregated show, built from one or more flagged episode files.
#[derive(Clone, Debug)]
pub struct Series {
    pub name: String,
    /// Deduplicated episodes, sorted by season then number.
    pub episodes: Vec<Episode>,
    /// The episode with the most recent `visited` timestamp.
    pub last_watched: Episode,
}

impl Series {
    /// Count of distinct episodes seen.
    pub fn watched_count(&self) -> usize {
        self.episodes.len()
    }

    /// Highest season number present.
    pub fn seasons(&self) -> u32 {
        self.episodes.iter().map(|e| e.season).max().unwrap_or(1)
    }

    /// The next episode the user should watch: the one numerically after the last
    /// watched episode within the same season. Returns `(season, number)`.
    pub fn next_up(&self) -> (u32, u32) {
        (self.last_watched.season, self.last_watched.number + 1)
    }

    /// `S01E06` tag for the next episode to watch.
    pub fn next_up_tag(&self) -> String {
        let (s, e) = self.next_up();
        format!("S{:02}E{:02}", s, e)
    }

    /// Whether the suggested next episode already exists in the user's library.
    pub fn next_available(&self) -> bool {
        let (s, e) = self.next_up();
        self.episodes
            .iter()
            .any(|ep| ep.season == s && ep.number == e)
    }

    /// How far through the last-watched season the user is (0.0..=1.0), based on
    /// episodes seen in that season vs. the highest episode number seen there.
    pub fn season_progress(&self) -> f32 {
        let season = self.last_watched.season;
        let in_season: Vec<&Episode> =
            self.episodes.iter().filter(|e| e.season == season).collect();
        let max_ep = in_season.iter().map(|e| e.number).max().unwrap_or(1).max(1);
        (in_season.len() as f32 / max_ep as f32).clamp(0.0, 1.0)
    }

    /// Date portion (`YYYY-MM-DD`) of the last watch, for display.
    pub fn last_watched_date(&self) -> String {
        self.last_watched
            .visited
            .split('T')
            .next()
            .unwrap_or(&self.last_watched.visited)
            .to_string()
    }
}

/// Result of a full scan.
pub struct Scan {
    pub series: Vec<Series>,
    pub source: String,
    pub total_files_seen: usize,
}

/// Run the default scan: parse the freedesktop recently-used database.
pub fn scan_recent() -> Scan {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/.local/share/recently-used.xbel");
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    let hrefs = extract_bookmarks(&raw);
    let total = hrefs.len();
    let series = build_series(hrefs);
    Scan {
        series,
        source: path,
        total_files_seen: total,
    }
}

/// Build a `Scan` from a directory walk so it can be merged with the recent scan.
pub fn scan_dir_full(dir: &str) -> Scan {
    let mut hrefs: Vec<(String, String)> = Vec::new();
    walk_full(Path::new(dir), 0, &mut hrefs);
    let total = hrefs.len();
    Scan {
        series: build_series(hrefs),
        source: dir.to_string(),
        total_files_seen: total,
    }
}

fn walk_full(dir: &Path, depth: usize, out: &mut Vec<(String, String)>) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_full(&path, depth + 1, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let ts = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(format_systime)
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
            // Pack "path\x00timestamp" so build_series can recover the visit time.
            out.push((format!("{}\x00{}", path.to_string_lossy(), ts), name.to_string()));
        }
    }
}

fn format_systime(t: std::time::SystemTime) -> String {
    // Crude but dependency-free: seconds since epoch is enough to order entries,
    // and we render only the day, so format an approximate UTC date.
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let tod = secs % 86_400;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Howard Hinnant's days-from-civil inverse, for epoch-day -> (y, m, d).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Pull `(href, visited)` pairs out of a recently-used.xbel document.
/// Returns `(decoded_local_path_or_packed, basename)` tuples; we pack the visit
/// timestamp into the first field as `path\x00timestamp` to reuse `build_series`.
fn extract_bookmarks(xml: &str) -> Vec<(String, String)> {
    let re = Regex::new(
        r#"href="([^"]*)"(?:[^>]*?\bvisited="([^"]*)")?(?:[^>]*?\bmodified="([^"]*)")?"#,
    )
    .unwrap();
    let mut out = Vec::new();
    for cap in re.captures_iter(xml) {
        let href = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let visited = cap
            .get(2)
            .or_else(|| cap.get(3))
            .map(|m| m.as_str())
            .unwrap_or("1970-01-01T00:00:00Z");
        let Some(local) = file_uri_to_path(href) else {
            continue;
        };
        let name = Path::new(&local)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        out.push((format!("{local}\x00{visited}"), name));
    }
    out
}

/// Convert a `file://` URI to a decoded local path; returns `None` for non-file.
fn file_uri_to_path(href: &str) -> Option<String> {
    let rest = href.strip_prefix("file://")?;
    // After file:// there may be a host before the path; the path starts at the
    // first '/'. For local files it's `file:///...` so rest already begins '/'.
    let path_part = match rest.find('/') {
        Some(0) => rest,
        Some(idx) => &rest[idx..],
        None => rest,
    };
    Some(percent_decode(path_part))
}

/// Minimal percent-decoder (UTF-8 aware) — avoids pulling an extra dependency.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Grouping accumulator: case-folded key -> (display name, episodes by S/E).
type Grouped = HashMap<String, (String, HashMap<(u32, u32), Episode>)>;

/// Insert an episode under its series, deduping case-insensitively and keeping
/// the most recently visited copy of any given (season, episode).
fn insert_ep(grouped: &mut Grouped, name: String, ep: Episode) {
    let key = name.to_lowercase();
    let entry = grouped
        .entry(key)
        .or_insert_with(|| (name.clone(), HashMap::new()));
    entry
        .1
        .entry((ep.season, ep.number))
        .and_modify(|existing| {
            if ep.visited > existing.visited {
                *existing = ep.clone();
            }
        })
        .or_insert(ep);
}

/// Collapse the accumulator into sorted `Series`, newest activity first.
fn finalize(grouped: Grouped) -> Vec<Series> {
    let mut series: Vec<Series> = grouped
        .into_values()
        .map(|(name, map)| {
            let mut episodes: Vec<Episode> = map.into_values().collect();
            episodes.sort_by(|a, b| a.season.cmp(&b.season).then(a.number.cmp(&b.number)));
            let last_watched = episodes
                .iter()
                .max_by(|a, b| a.visited.cmp(&b.visited))
                .cloned()
                .unwrap_or_else(|| episodes[0].clone());
            Series {
                name,
                episodes,
                last_watched,
            }
        })
        .collect();
    series.sort_by(|a, b| b.last_watched.visited.cmp(&a.last_watched.visited));
    series
}

/// Turn `(packed_path, basename)` tuples into deduplicated, sorted series.
fn build_series(items: Vec<(String, String)>) -> Vec<Series> {
    // Case-insensitive SxxExx. We match anywhere in the basename; everything
    // before the match is treated as the series title.
    let re = Regex::new(r"(?i)s(\d{1,3})[\s._-]*e(\d{1,3})").unwrap();
    let mut grouped: Grouped = HashMap::new();

    for (packed, filename) in items {
        let (path, visited) = match packed.split_once('\x00') {
            Some((p, v)) => (p.to_string(), v.to_string()),
            None => (packed.clone(), "1970-01-01T00:00:00Z".to_string()),
        };

        // Strip extension before matching so we work on the stem.
        let stem = strip_ext(&filename);
        let Some(caps) = re.captures(stem) else {
            continue;
        };
        let m = caps.get(0).unwrap();
        let season: u32 = caps[1].parse().unwrap_or(0);
        let number: u32 = caps[2].parse().unwrap_or(0);

        let name = clean_series_name(&stem[..m.start()]);
        if name.is_empty() {
            continue;
        }

        insert_ep(
            &mut grouped,
            name,
            Episode {
                season,
                number,
                visited,
                path,
                filename,
            },
        );
    }

    finalize(grouped)
}

/// Merge two scans (e.g. recent + an added folder), deduping by series/episode.
pub fn merge(a: Vec<Series>, b: Vec<Series>) -> Vec<Series> {
    let mut grouped: Grouped = HashMap::new();
    for s in a.into_iter().chain(b.into_iter()) {
        for ep in s.episodes {
            insert_ep(&mut grouped, s.name.clone(), ep);
        }
    }
    finalize(grouped)
}

fn strip_ext(name: &str) -> &str {
    match name.rfind('.') {
        // Only treat as an extension if it's short and alphanumeric-ish.
        Some(idx) if name.len() - idx <= 5 && idx > 0 => &name[..idx],
        _ => name,
    }
}

/// Turn `Modern.Family.` into `Modern Family`.
fn clean_series_name(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| match c {
            '.' | '_' | '-' => ' ',
            other => other,
        })
        .collect();
    let words: Vec<String> = replaced
        .split_whitespace()
        .map(title_word)
        .filter(|w| !w.is_empty())
        .collect();
    words.join(" ").trim().to_string()
}

fn title_word(w: &str) -> String {
    // Leave all-caps acronyms (e.g. "SNL") as-is; otherwise Title Case.
    if w.chars().all(|c| c.is_ascii_uppercase()) && w.len() <= 4 {
        return w.to_string();
    }
    let mut chars = w.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}
