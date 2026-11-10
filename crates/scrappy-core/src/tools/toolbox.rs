//! Toolbox: where each tool is (Scrappy's copy in the tools folder first, then `PATH`), the status the window shows
//! (versions cached until a program's file changes), and installs started as jobs (ADR-0002, ADR-0012).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use serde::Serialize;
use ts_rs::TS;

use super::install::{self, program_file};
use super::manifest::{self, Tool};
use super::run;
use crate::jobs::{JobId, Jobs};

// ── Types ─────────────────────────────────
/// Where a tool was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolSource {
    /// Scrappy's own copy, in the tools folder.
    Scrappy,
    /// A copy on `PATH`.
    System,
    Missing,
}

/// One tool, for the window's tool list.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolStatus {
    pub tool: Tool,
    pub source: ToolSource,
    pub version: Option<String>,
    /// The version Scrappy's download has.
    pub pinned_version: String,
    /// Older than `pinned_version`, or Scrappy's own copy doesn't run.
    pub outdated: bool,
    /// Scrappy has a download of it for this system.
    pub downloadable: bool,
    /// The install job running for it, if any.
    pub job: Option<JobId>,
}

/// An install that was started, so the window can follow its job.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolJob {
    pub tool: Tool,
    pub id: JobId,
}

/// Each program's version with its file's change time.
type VersionCache = Arc<Mutex<HashMap<PathBuf, (SystemTime, Option<String>)>>>;

/// The tools folder, the installs running in it, and the versions read so far.
#[derive(Clone)]
pub struct Toolbox {
    dir: PathBuf,
    installing: Arc<Mutex<HashMap<Tool, JobId>>>,
    /// Starting yt-dlp or gallery-dl takes about a second, so a version is read again only when the file changes.
    versions: VersionCache,
}

// ── Private helpers ───────────────────────
/// Whether a copy needs an update: older than the pin, or Scrappy's own copy doesn't run. A copy on `PATH` whose
/// version can't be read is left alone.
fn outdated(tool: Tool, source: ToolSource, version: Option<&str>) -> bool {
    match (source, version) {
        (ToolSource::Missing, _) => false,
        (ToolSource::Scrappy, None) => true,
        (ToolSource::System, None) => false,
        (_, Some(version)) => run::older_than(version, tool.pinned_version()) == Some(true),
    }
}

fn changed_at(program: &Path) -> Option<SystemTime> {
    std::fs::metadata(program).and_then(|metadata| metadata.modified()).ok()
}

pub(super) fn on_path(program: &str) -> Option<PathBuf> {
    let file = program_file(program);
    std::env::split_paths(&std::env::var_os("PATH")?).map(|dir| dir.join(&file)).find(|path| path.is_file())
}

// ── Public API ────────────────────────────
impl Toolbox {
    /// A toolbox using `dir` (in the app data folder) for Scrappy's copies.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, installing: Arc::default(), versions: Arc::default() }
    }

    /// `program`'s version, from the cache while its file hasn't changed.
    async fn version(&self, tool: Tool, program: PathBuf) -> Option<String> {
        let changed = changed_at(&program);
        let cache = || self.versions.lock().unwrap_or_else(PoisonError::into_inner);
        if let (Some(changed), Some((at, version))) = (changed, cache().get(&program))
            && changed == *at
        {
            return version.clone();
        }
        let version = run::version(tool, &program).await;
        if let Some(changed) = changed {
            cache().insert(program, (changed, version.clone()));
        }
        version
    }

    /// Where `tool` is and its main program: Scrappy's copy when it has all the tool's programs, otherwise a copy on
    /// `PATH` that has them all.
    pub fn locate(&self, tool: Tool) -> (ToolSource, Option<PathBuf>) {
        let ours: Vec<PathBuf> = tool.programs().iter().map(|program| self.dir.join(program_file(program))).collect();
        if ours.iter().all(|path| path.is_file()) {
            return (ToolSource::Scrappy, ours.into_iter().next());
        }
        match tool.programs().iter().map(|program| on_path(program)).collect::<Option<Vec<_>>>() {
            Some(paths) => (ToolSource::System, paths.into_iter().next()),
            None => (ToolSource::Missing, None),
        }
    }

    /// One of `tool`'s programs (ffprobe for FFmpeg): beside the main program first, then on `PATH`.
    pub fn program(&self, tool: Tool, program: &str) -> Option<PathBuf> {
        let main = self.locate(tool).1?;
        let beside = main.with_file_name(program_file(program));
        if beside.is_file() { Some(beside) } else { on_path(program) }
    }

    /// Every tool, where it is, its version and any install running. Versions not cached yet are read all at once.
    pub async fn status(&self) -> Vec<ToolStatus> {
        let located = Tool::ALL.map(|tool| (tool, self.locate(tool)));
        let mut reads = tokio::task::JoinSet::new();
        for (tool, (_, program)) in &located {
            if let Some(program) = program.clone() {
                let (toolbox, tool) = (self.clone(), *tool);
                reads.spawn(async move { (tool, toolbox.version(tool, program).await) });
            }
        }
        let mut versions = HashMap::new();
        while let Some(read) = reads.join_next().await {
            if let Ok((tool, version)) = read {
                versions.insert(tool, version);
            }
        }

        let mut statuses = Vec::new();
        for (tool, (source, _)) in located {
            let version = versions.remove(&tool).flatten();
            let job = self.installing.lock().unwrap_or_else(PoisonError::into_inner).get(&tool).copied();
            statuses.push(ToolStatus {
                tool,
                source,
                outdated: outdated(tool, source, version.as_deref()),
                version,
                pinned_version: tool.pinned_version().to_string(),
                downloadable: manifest::download_for(tool).is_some(),
                job,
            });
        }
        statuses
    }

    /// Starts an install job for each tool that's missing or outdated and has a download here, and returns them all,
    /// including ones already running. An outdated copy on `PATH` gets Scrappy's own copy beside it, which then wins.
    /// Must be called inside a Tokio runtime.
    pub async fn install_needed(&self, jobs: &Arc<Jobs>) -> Vec<ToolJob> {
        let needed: Vec<Tool> = self
            .status()
            .await
            .into_iter()
            .filter(|s| s.job.is_some() || (s.downloadable && (s.source == ToolSource::Missing || s.outdated)))
            .map(|s| s.tool)
            .collect();
        let mut installing = self.installing.lock().unwrap_or_else(PoisonError::into_inner);
        let started: Vec<ToolJob> = needed
            .into_iter()
            .map(|tool| {
                if let Some(&id) = installing.get(&tool) {
                    return ToolJob { tool, id };
                }
                let toolbox = self.clone();
                let id = jobs.start(move |job| async move {
                    let result = install::install(&toolbox.dir, tool, &job).await;
                    toolbox.installing.lock().unwrap_or_else(PoisonError::into_inner).remove(&tool);
                    if result.is_ok() && tool == Tool::YtDlp {
                        tokio::spawn(async move { toolbox.update_yt_dlp().await });
                    }
                    result
                });
                ToolJob { tool, id }
            })
            .collect();
        for started in &started {
            installing.insert(started.tool, started.id);
        }
        started
    }

    /// Lets Scrappy's own yt-dlp update itself in the background. A copy on `PATH` is left alone: it's the user's.
    pub async fn update_yt_dlp(&self) {
        if let (ToolSource::Scrappy, Some(program)) = self.locate(Tool::YtDlp) {
            run::update_yt_dlp(&program).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Tool, ToolSource, Toolbox};
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::tools::install::{self, program_file};
    use crate::tools::manifest::download_for;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Proves the pins: run with `cargo test -p scrappy-core -- --ignored` after changing the manifest.
    #[tokio::test]
    #[ignore = "downloads every tool for this system from GitHub (about 300 MB)"]
    async fn installs_every_tool_for_this_system() {
        let dir = tempfile::tempdir().unwrap();
        let toolbox = Toolbox::new(dir.path().to_path_buf());
        let results = Arc::new(Mutex::new(Vec::new()));
        let sink = results.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                sink.lock().unwrap().push(end.result);
            }
        }));
        let tools: Vec<Tool> = Tool::ALL.into_iter().filter(|&tool| download_for(tool).is_some()).collect();
        for &tool in &tools {
            let dir = dir.path().to_path_buf();
            jobs.start(move |job| async move { install::install(&dir, tool, &job).await });
        }
        while results.lock().unwrap().len() < tools.len() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(results.lock().unwrap().iter().all(|result| *result == JobResult::Done), "{results:?}");
        for status in toolbox.status().await.into_iter().filter(|status| tools.contains(&status.tool)) {
            assert_eq!(status.source, ToolSource::Scrappy, "{status:?}");
            assert!(status.version.is_some() && !status.outdated, "pinned_version is wrong: {status:?}");
        }
    }

    #[tokio::test]
    async fn reuses_a_version_until_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let toolbox = Toolbox::new(dir.path().to_path_buf());
        let program = dir.path().join(program_file("yt-dlp"));
        fs::write(&program, "not a program").unwrap();
        let changed = super::changed_at(&program).unwrap();
        toolbox.versions.lock().unwrap().insert(program.clone(), (changed, Some("cached".into())));
        assert_eq!(toolbox.version(Tool::YtDlp, program.clone()).await.as_deref(), Some("cached"));

        let later = changed + Duration::from_secs(5);
        fs::File::options().write(true).open(&program).unwrap().set_modified(later).unwrap();
        assert_eq!(toolbox.version(Tool::YtDlp, program).await, None);
    }

    #[test]
    fn prefers_scrappys_copy_and_needs_every_program() {
        let dir = tempfile::tempdir().unwrap();
        let toolbox = Toolbox::new(dir.path().to_path_buf());
        fs::write(dir.path().join(program_file("ffmpeg")), "").unwrap();
        assert_ne!(toolbox.locate(Tool::Ffmpeg).0, ToolSource::Scrappy);
        fs::write(dir.path().join(program_file("ffprobe")), "").unwrap();
        let (source, program) = toolbox.locate(Tool::Ffmpeg);
        assert_eq!(source, ToolSource::Scrappy);
        assert_eq!(program, Some(dir.path().join(program_file("ffmpeg"))));
    }
}
