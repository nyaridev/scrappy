//! Tools: finds, downloads, checks (SHA-256) and updates the external engines: yt-dlp, Deno, gallery-dl, ffmpeg and
//! ffprobe (ADR-0002, ADR-0012), and finds the installed browser Grab's deep scan drives (ADR-0007).

mod browser;
mod install;
mod manifest;
mod run;
mod toolbox;

pub use browser::browser;
pub use manifest::Tool;
pub use run::command;
pub use toolbox::{ToolJob, ToolSource, ToolStatus, Toolbox};
