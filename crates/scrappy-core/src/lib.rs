//! scrappy-core: everything Scrappy does. It never uses Tauri: the shell in `src-tauri/` calls it and forwards results
//! and progress to the window (ADR-0001).

pub mod app;
pub mod bridge;
pub mod crawl;
pub mod dedupe;
pub mod grab;
pub mod hunt;
pub mod jobs;
pub mod library;
pub mod logging;
pub mod media;
pub mod settings;
pub mod tools;
pub mod vault;
