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
/// A hash is filed by each of its four 16-bit quarters to find close ones.
const QUARTERS: u32 = 4;

// ── Types ─────────────────────────────────
/// How alike two pictures must be to count as copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// Calls `pair(i, j)`, `i < j`, at least once for every two hashes at most `limit` bits apart. Two hashes that close
/// have a quarter at most `limit / 4` bits apart, so each hash looks only at those filed under a quarter that near its
/// own instead of at every other one (multi-index hashing): the work grows with the pictures, not with their pairs.
fn each_close_pair(hashes: &[u64], limit: u32, mut pair: impl FnMut(usize, usize)) {
    let quarter = |hash: u64, part: u32| (hash >> (16 * part)) as u16;
    let flips: Vec<u16> = (0..=u16::MAX).filter(|flip| flip.count_ones() <= limit / QUARTERS).collect();
    for part in 0..QUARTERS {
        let mut filed: Vec<Vec<usize>> = vec![Vec::new(); 1 << 16];
        for (index, &hash) in hashes.iter().enumerate() {
            filed[usize::from(quarter(hash, part))].push(index);
        }
        for (i, &hash) in hashes.iter().enumerate() {
            for flip in &flips {
                for &j in &filed[usize::from(quarter(hash, part) ^ flip)] {
                    if j > i && (hash ^ hashes[j]).count_ones() <= limit {
                        pair(i, j);
                    }
                }
            }
        }
    }
}

// ── Public API ────────────────────────────
/// The groups of two or more copies among `copies`, each best first, the biggest groups first.
pub fn groups(copies: Vec<Picture>, strictness: Strictness) -> Vec<Vec<Picture>> {
    let mut parents: Vec<usize> = (0..copies.len()).collect();
    let hashes: Vec<u64> = copies.iter().map(|copy| copy.hash).collect();
    each_close_pair(&hashes, strictness.max_distance(), |i, j| {
        let (a, b) = (root(&mut parents, i), root(&mut parents, j));
        if a != b && same_shape(&copies[i], &copies[j]) {
            parents[a] = b;
        }
    });
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
    fn groups_as_comparing_every_pair_would() {
        // Clusters of hashes a few to a dozen bits from a random center, from a fixed xorshift.
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut hashes = Vec::new();
        for _ in 0..200 {
            let center = next();
            for _ in 0..10 {
                let flips = (0..next() % 13).fold(0u64, |mask, _| mask | 1 << (next() % 64));
                hashes.push(center ^ flips);
            }
        }
        for strictness in [Strictness::Strict, Strictness::Normal, Strictness::Loose] {
            let limit = strictness.max_distance();
            let mut parents: Vec<usize> = (0..hashes.len()).collect();
            for i in 0..hashes.len() {
                for j in i + 1..hashes.len() {
                    if (hashes[i] ^ hashes[j]).count_ones() <= limit {
                        let (a, b) = (super::root(&mut parents, i), super::root(&mut parents, j));
                        parents[a] = b;
                    }
                }
            }
            let copies = hashes.iter().enumerate().map(|(i, &hash)| copy(&format!("{i:04}.png"), hash, (4, 3), 1, None));
            let mut expected: Vec<Vec<String>> = (0..hashes.len())
                .fold(std::collections::BTreeMap::<usize, Vec<String>>::new(), |mut by_root, i| {
                    by_root.entry(super::root(&mut parents, i)).or_default().push(format!("{i:04}.png"));
                    by_root
                })
                .into_values()
                .filter(|group| group.len() > 1)
                .collect();
            let mut found: Vec<Vec<String>> = groups(copies.collect(), strictness)
                .into_iter()
                .map(|group| {
                    let mut paths: Vec<String> = group.into_iter().map(|copy| copy.path).collect();
                    paths.sort();
                    paths
                })
                .collect();
            expected.sort();
            found.sort();
            assert!(!found.is_empty());
            assert_eq!(found, expected, "{strictness:?}");
        }
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
