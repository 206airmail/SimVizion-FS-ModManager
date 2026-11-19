//! Decoding mod icons (DDS or PNG) into small PNG thumbnails.
//!
//! Real icons are DXT1, DXT3, DXT5, BC7 (DX10 header) or uncompressed 24/32-bit DDS, or PNG.
//! Only mip 0 is decoded. Input is untrusted: malformed headers must return an error, never
//! panic or allocate without bound.

use std::fmt;

use crate::container::{IconLocation, ModContainer, ReadError};

mod dds;
mod pngdec;
mod resize;

/// Largest icon file read from a container (compressed or not), in bytes.
const MAX_ICON_FILE_BYTES: u64 = 32 * 1024 * 1024;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Longest edge of a thumbnail, in pixels.
pub const THUMBNAIL_EDGE: u32 = 256;
/// Largest width or height accepted from an icon header.
pub const MAX_DIMENSION: u32 = 8192;
/// Largest `width * height` decoded (64 MiB of RGBA). Real icons are at most 1024×1024.
pub const MAX_PIXELS: u64 = 4096 * 4096;

/// Rejects images whose decoded size would exceed [`MAX_PIXELS`].
fn check_pixel_budget(width: u32, height: u32) -> Result<(), IconError> {
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(IconError::Malformed(format!(
            "{width}x{height} is more than {MAX_PIXELS} pixels"
        )));
    }
    Ok(())
}

/// 8-bit RGBA pixels, row-major, no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub enum IconError {
    Read(ReadError),
    /// A format the decoder does not handle; the detail names it.
    Unsupported(String),
    /// Truncated or inconsistent data; the detail says what.
    Malformed(String),
    Encode(String),
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::Read(e) => write!(f, "could not read icon: {e}"),
            IconError::Unsupported(d) => write!(f, "unsupported icon format: {d}"),
            IconError::Malformed(d) => write!(f, "malformed icon: {d}"),
            IconError::Encode(d) => write!(f, "could not encode thumbnail: {d}"),
        }
    }
}

impl std::error::Error for IconError {}

/// Decodes mip 0 of a DDS file. See `dds.rs` for the supported formats.
pub fn decode_dds(bytes: &[u8]) -> Result<Rgba, IconError> {
    dds::decode(bytes)
}

/// Decodes any PNG the `png` crate reads into 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<Rgba, IconError> {
    pngdec::decode(bytes)
}

/// Area-averaged downscale so the longest edge is at most `max_edge`; never upscales.
pub fn downscale(image: &Rgba, max_edge: u32) -> Rgba {
    resize::downscale(image, max_edge)
}

pub fn encode_png(image: &Rgba) -> Result<Vec<u8>, IconError> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| IconError::Encode(e.to_string()))?;
    writer
        .write_image_data(&image.pixels)
        .map_err(|e| IconError::Encode(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| IconError::Encode(e.to_string()))?;
    Ok(out)
}

/// Reads the icon from its container and returns PNG bytes with the longest edge at most
/// `max_edge`.
///
/// The format is taken from the file's magic bytes, not from `icon.kind`, because modders
/// routinely name a DDS file `.png` and the other way round.
pub fn thumbnail(
    container: &mut ModContainer,
    icon: &IconLocation,
    max_edge: u32,
) -> Result<Vec<u8>, IconError> {
    let bytes = container
        .read(&icon.rel, MAX_ICON_FILE_BYTES)
        .map_err(IconError::Read)?;
    let image = if bytes.starts_with(b"DDS ") {
        decode_dds(&bytes)?
    } else if bytes.starts_with(&PNG_SIGNATURE) {
        decode_png(&bytes)?
    } else {
        return Err(IconError::Unsupported(
            "icon is neither DDS nor PNG".to_string(),
        ));
    };
    encode_png(&downscale(&image, max_edge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::IconKind;
    use crate::testutil::{dds, png as png_bytes, write_folder, write_zip, DdsFormat, Method};

    const RED_BLOCK: [u8; 8] = [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0]; // c0 = c1 = red, index 0

    /// A solid red DXT1 image of the given size.
    fn red_dxt1(width: u32, height: u32) -> Vec<u8> {
        let blocks = width.div_ceil(4) * height.div_ceil(4);
        let data: Vec<u8> = RED_BLOCK.repeat(blocks as usize);
        dds(width, height, 1, DdsFormat::FourCc(*b"DXT1"), &data)
    }

    fn icon(rel: &str, kind: IconKind) -> IconLocation {
        IconLocation {
            rel: rel.to_string(),
            kind,
        }
    }

    #[test]
    fn pixel_budget_caps_decoded_size_below_the_dimension_cap() {
        assert!(check_pixel_budget(4096, 4096).is_ok());
        assert!(check_pixel_budget(8192, 1).is_ok());
        assert!(matches!(
            check_pixel_budget(4097, 4096),
            Err(IconError::Malformed(_))
        ));
        // Each edge is within MAX_DIMENSION, but the area is over budget.
        let big = dds(8192, 4096, 1, DdsFormat::FourCc(*b"DXT1"), &[]);
        assert!(matches!(decode_dds(&big), Err(IconError::Malformed(m)) if m.contains("pixels")));
    }

    #[test]
    fn encode_png_round_trips_through_decode_png() {
        let image = Rgba {
            width: 2,
            height: 2,
            pixels: (0..16).collect(),
        };
        let bytes = encode_png(&image).unwrap();
        assert_eq!(decode_png(&bytes).unwrap(), image);
    }

    #[test]
    fn encode_png_rejects_inconsistent_pixel_data() {
        let image = Rgba {
            width: 2,
            height: 2,
            pixels: vec![0; 3],
        };
        assert!(matches!(encode_png(&image), Err(IconError::Encode(_))));
    }

    #[test]
    fn thumbnail_from_a_zip_downscales_a_dds_icon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Test.zip");
        let icon_bytes = red_dxt1(30, 20);
        write_zip(
            &path,
            &[
                ("modDesc.xml", b"<modDesc/>"),
                ("icons/Icon.dds", &icon_bytes),
            ],
            Method::Deflated,
        );
        let mut container = ModContainer::open(&path).unwrap();
        // Different case from the stored name: container lookup is case-insensitive.
        let png = thumbnail(&mut container, &icon("icons/icon.DDS", IconKind::Dds), 10).unwrap();
        let image = decode_png(&png).unwrap();
        assert_eq!((image.width, image.height), (10, 7));
        assert!(image.pixels.chunks(4).all(|p| p == [255, 0, 0, 255]));
    }

    #[test]
    fn thumbnail_from_a_folder_decodes_a_png_icon_and_never_upscales() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("FS25_Folder");
        let icon_bytes = png_bytes(2, 2, png::ColorType::Rgba, &[7; 16]);
        write_folder(&root, &[("icon.png", &icon_bytes)]);
        let mut container = ModContainer::open(&root).unwrap();
        let png = thumbnail(
            &mut container,
            &icon("icon.png", IconKind::Png),
            THUMBNAIL_EDGE,
        )
        .unwrap();
        let image = decode_png(&png).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.pixels, [7; 16]);
    }

    #[test]
    fn thumbnail_trusts_the_magic_bytes_over_the_declared_kind() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("FS25_Folder");
        write_folder(&root, &[("icon.png", &red_dxt1(8, 8))]);
        let mut container = ModContainer::open(&root).unwrap();
        let png = thumbnail(&mut container, &icon("icon.png", IconKind::Png), 4).unwrap();
        let image = decode_png(&png).unwrap();
        assert_eq!((image.width, image.height), (4, 4));
    }

    #[test]
    fn thumbnail_rejects_unknown_content_and_missing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Odd.zip");
        write_zip(
            &path,
            &[("icon.dds", b"this is not an image"), ("empty.dds", b"")],
            Method::Stored,
        );
        let mut container = ModContainer::open(&path).unwrap();
        for name in ["icon.dds", "empty.dds"] {
            assert!(matches!(
                thumbnail(&mut container, &icon(name, IconKind::Dds), 256),
                Err(IconError::Unsupported(_))
            ));
        }
        assert!(matches!(
            thumbnail(&mut container, &icon("missing.dds", IconKind::Dds), 256),
            Err(IconError::Read(ReadError::NotFound))
        ));
    }

    #[test]
    fn thumbnail_reports_corrupt_icons_as_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("FS25_Corrupt");
        let truncated = red_dxt1(16, 16);
        write_folder(&root, &[("icon.dds", &truncated[..140])]);
        let mut container = ModContainer::open(&root).unwrap();
        assert!(matches!(
            thumbnail(&mut container, &icon("icon.dds", IconKind::Dds), 256),
            Err(IconError::Malformed(_))
        ));
    }

    /// Rough performance figure. Run with:
    /// `cargo test -p fsmm-core --release icons -- --ignored --nocapture`
    #[test]
    #[ignore = "timing measurement, run by hand"]
    fn timing_of_a_512_dxt1_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("FS25_Timing");
        write_folder(&root, &[("icon.dds", &red_dxt1(512, 512))]);
        let mut container = ModContainer::open(&root).unwrap();
        let location = icon("icon.dds", IconKind::Dds);
        let runs = 50;
        let start = std::time::Instant::now();
        for _ in 0..runs {
            thumbnail(&mut container, &location, THUMBNAIL_EDGE).unwrap();
        }
        let per_run = start.elapsed() / runs;
        println!("512x512 DXT1 -> 256 px thumbnail: {per_run:?} per icon (mean of {runs})");
    }
}
