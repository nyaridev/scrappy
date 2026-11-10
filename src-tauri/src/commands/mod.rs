//! Commands the window can call, one file per view or system. Each one parses its input, makes one scrappy-core call
//! and returns the result.

pub mod app;
pub mod dedupe;
pub mod extension;
pub mod grab;
pub mod hunt;
pub mod jobs;
pub mod library;
pub mod media;
pub mod scrape;
pub mod settings;
pub mod tools;
