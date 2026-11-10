//! Group: near-duplicate pictures put together by how many bits their hashes differ (pictures of a different shape
//! never), each group ordered best copy first: most pixels, then the better format, then the higher JPEG quality, then
//! the bigger file (ADR-0009).

// ── Imports ───────────────────────────────
use std::cmp::Reverse;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// How far two shapes may differ (width over height) and still be one picture: 3%.
const SHAPE_TOLERANCE: f64 = 0.03;
/// Formats from best to worst for keeping: lossless first.
const FORMATS: &[&[&str]] = &[&["png", "tif", "tiff", "bmp"], &["webp"], &["jpg", "jpeg"], &["gif"]];

// ── Types ─────────────────────────────────
/// How alike two pictures must be to count as copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Strictness {
    /// Only resized or re-saved copies.
    Strict,
    Normal,
    /// Also lightly edited copies; more false matches.
    Loose,
}

/// A hashed picture.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Picture {
    /// Relative to the library root.
    pub path: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Its extension in capitals: `JPG`, `PNG`.
    pub format: String,
    /// A JPEG's estimated quality, 1 to 100.
    pub quality: Option<u8>,
    #[serde(skip)]
    #[ts(skip)]
    pub hash: u64,
}

// ── Private helpers ───────────────────────
impl Strictness {
    /// The most bits two hashes of copies may differ.
    fn max_distance(self) -> u32 {
        match self {
            Self::Strict => 3,
            Self::Normal => 6,
            Self::Loose => 10,
        }
    }
}

fn same_shape(a: &Picture, b: &Picture) -> bool {
    let shape = |copy: &Picture| f64::from(copy.width) / f64::from(copy.height.max(1));
    let (a, b) = (shape(a), shape(b));
    (a - b).abs() / a.max(b) <= SHAPE_TOLERANCE
}

/// Lower is better.
fn format_rank(format: &str) -> usize {
    let format = format.to_ascii_lowercase();
    FORMATS.iter().position(|names| names.contains(&format.as_str())).unwrap_or(FORMATS.len())
}

fn root(parents: &mut [usize], mut at: usize) -> usize {
    while parents[at] != at {
        parents[at] = parents[parents[at]];
        at = parents[at];
    }
    at
}

// ── Public API ────────────────────────────
/// The groups of two or more copies among `copies`, each best first, the biggest groups first.
pub fn groups(copies: Vec<Picture>, strictness: Strictness) -> Vec<Vec<Picture>> {
    let limit = strictness.max_distance();
    let mut parents: Vec<usize> = (0..copies.len()).collect();
    // shortcut: compares every pair (fine for tens of thousands of pictures); use a BK-tree past that.
    for i in 0..copies.len() {
        for j in i + 1..copies.len() {
            if (copies[i].hash ^ copies[j].hash).count_ones() <= limit && same_shape(&copies[i], &copies[j]) {
                let (a, b) = (root(&mut parents, i), root(&mut parents, j));
                parents[a] = b;
            }
        }
    }
    let mut by_root: HashMap<usize, Vec<Picture>> = HashMap::new();
    for (index, copy) in copies.into_iter().enumerate() {
        by_root.entry(root(&mut parents, index)).or_default().push(copy);
    }
    let mut groups: Vec<Vec<Picture>> = by_root.into_values().filter(|group| group.len() > 1).collect();
    for group in &mut groups {
        group.sort_by_key(|copy| {
            let pixels = u64::from(copy.width) * u64::from(copy.height);
            (Reverse(pixels), format_rank(&copy.format), Reverse(copy.quality.unwrap_or(0)), Reverse(copy.bytes), copy.path.clone())
        });
    }
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].path.cmp(&b[0].path)));
    groups
}

#[cfg(test)]
mod tests {
    use super::{Picture, Strictness, groups};

    fn copy(path: &str, hash: u64, (width, height): (u32, u32), bytes: u64, quality: Option<u8>) -> Picture {
        let format = path.rsplit('.').next().unwrap().to_uppercase();
        Picture { path: path.into(), name: path.into(), width, height, bytes, format, quality, hash }
    }

    fn paths(groups: &[Vec<Picture>]) -> Vec<Vec<&str>> {
        groups.iter().map(|group| group.iter().map(|copy| copy.path.as_str()).collect()).collect()
    }

    #[test]
    fn groups_close_hashes_of_the_same_shape() {
        let found = groups(
            vec![
                copy("a.jpg", 0b0000, (800, 600), 90, Some(80)),
                copy("b.png", 0b0011, (400, 300), 300, None),
                copy("c.jpg", 0b0001, (800, 600), 120, Some(95)),
                // Close hash, other shape: a crop, not a copy.
                copy("d.jpg", 0b0001, (600, 600), 100, None),
                copy("e.jpg", u64::MAX, (800, 600), 100, None),
            ],
            Strictness::Strict,
        );
        assert_eq!(paths(&found), [["c.jpg", "a.jpg", "b.png"]]);
    }

    #[test]
    fn strictness_widens_the_match() {
        let pair = || vec![copy("a.png", 0, (10, 10), 1, None), copy("b.png", 0b1_1111, (10, 10), 1, None)];
        assert!(groups(pair(), Strictness::Strict).is_empty());
        assert_eq!(groups(pair(), Strictness::Normal).len(), 1);
    }

    #[test]
    fn prefers_pixels_then_format_then_quality_then_size() {
        let best = |copies| groups(copies, Strictness::Loose).remove(0).remove(0).path;
        let same = (100, 100);
        assert_eq!(best(vec![copy("a.jpg", 0, (200, 200), 1, Some(50)), copy("b.png", 0, same, 9, None)]), "a.jpg");
        assert_eq!(best(vec![copy("a.jpg", 0, same, 9, Some(99)), copy("b.png", 0, same, 1, None)]), "b.png");
        assert_eq!(best(vec![copy("a.jpg", 0, same, 9, Some(70)), copy("b.jpg", 0, same, 1, Some(90))]), "b.jpg");
        assert_eq!(best(vec![copy("a.gif", 0, same, 1, None), copy("b.gif", 0, same, 2, None)]), "b.gif");
    }
}
