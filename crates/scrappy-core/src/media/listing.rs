//! Listing: a link checked as a job (ADR-0030). yt-dlp lists lazily, one JSON line per entry as its page of the list
//! arrives; the lines are grouped by their list's link into the listing `-J` gives (one list, or a channel's lists as
//! sections), so the window shows the list while it grows. A link that's one video prints one line, its whole JSON.

// ── Imports ───────────────────────────────
use serde_json::{Value, json};
use tokio::process::Command;

use super::engine;
use super::probe::{Info, LIST_LIMIT, Probe, names_playlist};
use crate::jobs::JobId;
use crate::tools::Toolbox;
use crate::vault::CookieFile;

// ── Types ─────────────────────────────────
/// Where a check is: still listing, done, or stopped (then what was listed is the list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Listing,
    Done,
    Stopped,
}

/// One list's entries as yt-dlp printed them; each carries its list's `playlist_*` fields.
struct List {
    url: String,
    entries: Vec<Value>,
}

/// The lines read so far.
#[derive(Default)]
pub(super) struct Listing {
    /// A link that's one video: its JSON.
    single: Option<String>,
    lists: Vec<List>,
}

/// The latest link check: its job, link and lines, the list's thumbnail once fetched, and the probe once it ended.
pub(super) struct Check {
    pub id: JobId,
    pub url: String,
    pub listing: Listing,
    pub state: State,
    pub thumbnail: Option<String>,
    pub probe: Option<Probe>,
}

// ── Private helpers ───────────────────────
fn text<'a>(line: &'a Value, key: &str) -> Option<&'a str> {
    line.get(key).and_then(Value::as_str)
}

/// A list in `-J`'s shape, its fields from its first entry. Stopped, its count is what was listed.
fn list_json(list: &List, state: State) -> Value {
    let first = &list.entries[0];
    let field = |key: &str| first.get(format!("playlist_{key}")).cloned().unwrap_or(Value::Null);
    let count = if state == State::Stopped { json!(list.entries.len()) } else { field("count") };
    json!({
        "id": text(first, "playlist_id").unwrap_or(&list.url),
        "_type": "playlist",
        "title": field("title"),
        "channel": field("channel"),
        "uploader": field("uploader"),
        "extractor_key": first.get("extractor_key"),
        "webpage_url": list.url,
        "playlist_count": count,
        "entries": list.entries,
    })
}

// ── Public API ────────────────────────────
impl Listing {
    /// Adds one line yt-dlp printed; a line that isn't JSON is skipped.
    pub(super) fn add(&mut self, line: &str) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { return };
        if entry.get("playlist_index").is_none_or(Value::is_null) {
            self.single = Some(line.to_string());
            return;
        }
        let url = text(&entry, "playlist_webpage_url").or_else(|| text(&entry, "playlist_id")).unwrap_or_default();
        match self.lists.iter_mut().find(|list| list.url == url) {
            Some(list) => list.entries.push(entry),
            None => self.lists.push(List { url: url.to_string(), entries: vec![entry] }),
        }
    }

    /// How many entries are listed.
    pub(super) fn listed(&self) -> usize {
        self.lists.iter().map(|list| list.entries.len()).sum()
    }

    /// The listing as `-J` gives it: the video, one list, or a channel's lists as sections under the channel's name.
    pub(super) fn json(&self, state: State) -> Option<String> {
        let listing = match self.lists.as_slice() {
            [] => return self.single.clone(),
            [list] => list_json(list, state),
            lists => {
                let first = &lists[0].entries[0];
                let name = text(first, "playlist_channel").or_else(|| text(first, "playlist_uploader"));
                let id = text(first, "playlist_channel_id").or_else(|| text(first, "playlist_id"));
                let id = id.unwrap_or(&lists[0].url);
                let sections: Vec<Value> = lists.iter().map(|list| list_json(list, state)).collect();
                json!({
                    "id": id,
                    "_type": "playlist",
                    "title": name,
                    "channel": name,
                    "extractor_key": first.get("extractor_key"),
                    "entries": sections,
                })
            }
        };
        Some(listing.to_string())
    }
}

impl Check {
    /// What's listed so far: its JSON, read, and the probe the window shows.
    pub(super) fn build(&self) -> Option<(String, Info, Probe)> {
        let json = self.listing.json(self.state)?;
        let info = Info::parse(&json).ok()?;
        let probe = info.probe(&self.url, self.thumbnail.clone());
        Some((json, info, probe))
    }
}

/// yt-dlp's lazy listing of `url` (already checked), one JSON line per entry, with a sign-in when there is one.
pub(super) fn command(toolbox: &Toolbox, url: &str, cookies: Option<&CookieFile>) -> Result<Command, String> {
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_cookies(&mut command, cookies);
    if names_playlist(url) {
        // After the `--no-playlist` every yt-dlp run starts with; the later one wins.
        command.arg("--yes-playlist");
    }
    let items = format!("1-{LIST_LIMIT}");
    command.args(["--flat-playlist", "--lazy-playlist", "--playlist-items", &items, "-j", "--", url]);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::{Check, Listing, State};
    use crate::media::probe::{Info, section_listings};

    /// yt-dlp's lazy lines for `youtube.com/@name`: each entry with its list's fields.
    fn entry(id: &str, list: &str, count: Option<u32>) -> String {
        let count = count.map_or("null".into(), |count| count.to_string());
        format!(
            r#"{{"_type":"url","id":"{id}","title":"Video {id}","url":"https://www.youtube.com/watch?v={id}",
            "extractor_key":"Youtube","playlist_index":1,"playlist_id":"UC1","playlist_title":"GDULA - {list}",
            "playlist_channel":"GDULA","playlist_channel_id":"UC1","playlist_count":{count},
            "playlist_webpage_url":"https://www.youtube.com/@GDULA7/{list}"}}"#
        )
        .replace('\n', "")
    }

    fn check(listing: Listing, state: State) -> Check {
        let url = "https://www.youtube.com/@GDULA7".into();
        Check { id: 1, url, listing, state, thumbnail: None, probe: None }
    }

    #[test]
    fn reads_one_video() {
        let mut listing = Listing::default();
        listing.add(r#"{"id":"a","title":"Me at the zoo","_type":"video","playlist_index":null,"formats":[]}"#);
        listing.add("not json");
        let json = listing.json(State::Done).unwrap();
        assert_eq!(Info::parse(&json).unwrap().title(), "Me at the zoo");
        assert_eq!(listing.listed(), 0);
    }

    #[test]
    fn groups_a_channels_lists_into_sections() {
        let mut listing = Listing::default();
        for line in [entry("a", "videos", None), entry("b", "videos", None), entry("c", "shorts", None)] {
            listing.add(&line);
        }
        assert_eq!(listing.listed(), 3);
        let (json, _, probe) = check(listing, State::Listing).build().unwrap();
        assert_eq!(probe.title, "GDULA");
        let sections: Vec<_> = probe.sections.iter().map(|s| (s.title.as_str(), s.count)).collect();
        assert_eq!(sections, [("videos", 2), ("shorts", 1)]);
        assert_eq!(probe.sections[0].entries[1].url, "https://www.youtube.com/watch?v=b");
        assert_eq!(section_listings(&json)[1].0, "https://www.youtube.com/@GDULA7/shorts");
    }

    #[test]
    fn a_stopped_list_is_what_was_listed() {
        let mut listing = Listing::default();
        listing.add(&entry("a", "videos", Some(369)));
        listing.add(&entry("b", "videos", Some(369)));
        let mut check = check(listing, State::Listing);
        assert_eq!(check.build().unwrap().2.count, Some(369));
        check.state = State::Stopped;
        let probe = check.build().unwrap().2;
        assert_eq!((probe.count, probe.more, probe.entries.len()), (Some(2), false, 2));
        assert_eq!(probe.title, "GDULA - videos");
    }
}
