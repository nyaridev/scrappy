//! Hunt: images on a topic from across the web. A web search finds pages (the whole web, or a source pack's sites),
//! one finder takes the images in each page's content, and they're saved into the library (ADR-0013).

mod content;
mod pack;
mod run;
mod search;

pub use content::ImageKind;
pub use pack::{Pack, Packs, folder as packs_folder, packs};
pub use run::{HuntRequest, HuntStarted, hunt};
