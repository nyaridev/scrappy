//! Hunt: images on a topic from across the web. A web search finds pages on each source (a pack's sites, or any site),
//! one finder takes the images in each page's content, and they're saved into the library (ADR-0013). Sources run in
//! parallel, each with its own limit and progress (ADR-0024); image search engines are sources too (ADR-0025).

mod content;
mod engines;
mod pack;
mod run;
mod search;
mod status;

pub use content::ImageKind;
pub use engines::ImageEngine;
pub use pack::{Pack, Packs, create as create_pack, folder as packs_folder, packs, remove as remove_pack, save as save_pack};
pub use run::{HuntRequest, HuntStarted, hunt};
pub use status::{HuntSource, HuntStatus, Hunts, SourceState};
