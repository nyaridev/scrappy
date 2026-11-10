//! Fingerprint: a picture's 64-bit perceptual hash (a DCT hash: similar pictures get hashes a few bits apart) with its
//! width and height, and a JPEG's quality estimated from its quantization table (ADR-0009).

// ── Imports ───────────────────────────────
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use image_hasher::{HashAlg, HasherConfig};

// ── Constants ─────────────────────────────
/// The JPEG standard's luminance table; encoders scale it by quality (IJG's formula). Only its sum is used, so its
/// order doesn't matter.
const STANDARD_LUMINANCE: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51,
    87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];
/// The quantization tables come before the picture data; a header longer than this is unusual.
const HEADER_BYTES: u64 = 256 * 1024;

// ── Private helpers ───────────────────────
/// The first luminance quantization table (`DQT`, table 0) in a JPEG's header.
fn luminance_table(header: &[u8]) -> Option<Vec<u32>> {
    if header.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= header.len() {
        let (marker, length) = (header[at + 1], usize::from(u16::from_be_bytes([header[at + 2], header[at + 3]])));
        if header[at] != 0xFF || length < 2 {
            return None;
        }
        let segment = header.get(at + 4..at + 2 + length)?;
        if marker == 0xDB {
            let mut table = segment;
            while let Some(&info) = table.first() {
                let (wide, id) = (info >> 4 == 1, info & 0x0F);
                let size = if wide { 128 } else { 64 };
                let values = table.get(1..1 + size)?;
                if id == 0 {
                    let read = |i: usize| if wide { u32::from(u16::from_be_bytes([values[2 * i], values[2 * i + 1]])) } else { u32::from(values[i]) };
                    return Some((0..64).map(read).collect());
                }
                table = &table[1 + size..];
            }
        }
        // Start of scan: the picture data follows, and no table came.
        if marker == 0xDA {
            return None;
        }
        at += 2 + length;
    }
    None
}

// ── Public API ────────────────────────────
/// The perceptual hash, width and height of the picture at `file`. Decodes the whole picture: run it off the async
/// threads.
pub fn fingerprint(file: &Path) -> Result<(u64, u32, u32), String> {
    let picture = image::ImageReader::open(file)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|e| format!("Can't open the picture: {e}"))?
        .decode()
        .map_err(|e| format!("Can't read the picture: {e}"))?;
    let hasher = HasherConfig::new().hash_size(8, 8).hash_alg(HashAlg::Median).preproc_dct().to_hasher();
    let bytes: [u8; 8] = hasher.hash_image(&picture).as_bytes().try_into().map_err(|_| "The hash isn't 64 bits.")?;
    Ok((u64::from_be_bytes(bytes), picture.width(), picture.height()))
}

/// A JPEG's quality from 1 to 100, estimated from how much its luminance table is scaled from the standard one.
/// `None` for anything that isn't a JPEG with a table.
pub fn jpeg_quality(file: &Path) -> Option<u8> {
    let mut header = Vec::new();
    BufReader::new(File::open(file).ok()?).take(HEADER_BYTES).read_to_end(&mut header).ok()?;
    let table = luminance_table(&header)?;
    let scale = f64::from(table.iter().sum::<u32>()) * 100.0 / f64::from(STANDARD_LUMINANCE.iter().sum::<u32>());
    let quality = if scale <= 100.0 { (200.0 - scale) / 2.0 } else { 5000.0 / scale };
    Some(quality.round().clamp(1.0, 100.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, jpeg_quality};
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageFormat, Rgb, RgbImage};
    use std::fs::File;

    /// A picture with soft shapes, so hashes and JPEG artifacts behave as on a photo.
    fn picture(width: u32, height: u32) -> RgbImage {
        RgbImage::from_fn(width, height, |x, y| {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let ring = ((u - 0.6).hypot(v - 0.4) * 9.0).sin().abs();
            Rgb([(u * 255.0) as u8, (ring * 255.0) as u8, (v * 200.0) as u8])
        })
    }

    #[test]
    fn similar_pictures_get_close_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let (big, small, other) = (dir.path().join("big.png"), dir.path().join("small.jpg"), dir.path().join("other.png"));
        picture(800, 600).save_with_format(&big, ImageFormat::Png).unwrap();
        let resized = image::imageops::resize(&picture(800, 600), 400, 300, image::imageops::FilterType::Triangle);
        JpegEncoder::new_with_quality(File::create(&small).unwrap(), 70).encode_image(&resized).unwrap();
        image::imageops::flip_vertical(&picture(800, 600)).save_with_format(&other, ImageFormat::Png).unwrap();

        let (a, b, c) = (fingerprint(&big).unwrap(), fingerprint(&small).unwrap(), fingerprint(&other).unwrap());
        assert_eq!((a.1, a.2, b.1, b.2), (800, 600, 400, 300));
        // A median hash sets about half its bits; one with nearly none set would match everything.
        assert!((16..=48).contains(&a.0.count_ones()), "{:064b}", a.0);
        assert!((a.0 ^ b.0).count_ones() <= 4, "same picture, {} bits apart", (a.0 ^ b.0).count_ones());
        assert!((a.0 ^ c.0).count_ones() > 12, "different pictures, {} bits apart", (a.0 ^ c.0).count_ones());
        assert!(fingerprint(&dir.path().join("none.png")).is_err());
    }

    #[test]
    fn estimates_jpeg_quality() {
        let dir = tempfile::tempdir().unwrap();
        for quality in [40, 75, 95] {
            let file = dir.path().join(format!("q{quality}.jpg"));
            JpegEncoder::new_with_quality(File::create(&file).unwrap(), quality).encode_image(&picture(64, 64)).unwrap();
            let estimate = jpeg_quality(&file).unwrap();
            assert!(estimate.abs_diff(quality) <= 2, "{quality} read as {estimate}");
        }
        let png = dir.path().join("a.png");
        picture(8, 8).save_with_format(&png, ImageFormat::Png).unwrap();
        assert_eq!(jpeg_quality(&png), None);
    }
}
