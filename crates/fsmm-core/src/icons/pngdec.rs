//! PNG decoding: any PNG the `png` crate reads becomes 8-bit RGBA.
//!
//! Palette, grey, tRNS and 16-bit images are normalised by the decoder's transformations plus a
//! small channel expansion here. Dimensions are checked against `MAX_DIMENSION` from the header
//! before any pixel buffer is allocated, and the decoder is given an explicit memory limit.

use std::fmt::Display;
use std::io::Cursor;

use png::{ColorType, Transformations};

use super::{IconError, Rgba, MAX_DIMENSION};

/// Memory the decoder may use for its own buffers (rows and decompression state).
const DECODER_LIMIT_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn decode(bytes: &[u8]) -> Result<Rgba, IconError> {
    let limits = png::Limits {
        bytes: DECODER_LIMIT_BYTES,
    };
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(bytes), limits);
    decoder.set_transformations(Transformations::normalize_to_color8());

    let header = decoder.read_header_info().map_err(malformed)?;
    let (width, height) = (header.width, header.height);
    if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
        return Err(IconError::Malformed(format!(
            "PNG dimensions {width}x{height} are outside 1..={MAX_DIMENSION}"
        )));
    }

    let mut reader = decoder.read_info().map_err(malformed)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| IconError::Malformed("PNG is too large to decode".into()))?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(malformed)?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(IconError::Unsupported(format!(
            "PNG bit depth {:?}",
            info.bit_depth
        )));
    }

    let channels = match info.color_type {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => {
            return Err(IconError::Unsupported(
                "PNG palette was not expanded".into(),
            ))
        }
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let mut pixels = Vec::with_capacity(w * h * 4);
    for row in buf.chunks_exact(info.line_size).take(h) {
        for px in row[..w * channels].chunks_exact(channels) {
            match info.color_type {
                ColorType::Grayscale => pixels.extend_from_slice(&[px[0], px[0], px[0], 255]),
                ColorType::GrayscaleAlpha => {
                    pixels.extend_from_slice(&[px[0], px[0], px[0], px[1]])
                }
                ColorType::Rgb => pixels.extend_from_slice(&[px[0], px[1], px[2], 255]),
                _ => pixels.extend_from_slice(px),
            }
        }
    }
    Ok(Rgba {
        width: info.width,
        height: info.height,
        pixels,
    })
}

fn malformed(e: impl Display) -> IconError {
    IconError::Malformed(format!("PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::png;

    #[test]
    fn rgb_png_gets_an_opaque_alpha_channel() {
        let bytes = png(2, 1, ColorType::Rgb, &[1, 2, 3, 4, 5, 6]);
        let image = decode(&bytes).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn rgba_png_keeps_its_alpha() {
        let bytes = png(1, 2, ColorType::Rgba, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let image = decode(&bytes).unwrap();
        assert_eq!((image.width, image.height), (1, 2));
        assert_eq!(image.pixels, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    fn encode(
        width: u32,
        height: u32,
        color: ColorType,
        depth: png::BitDepth,
        configure: impl FnOnce(&mut png::Encoder<&mut Vec<u8>>),
        data: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        configure(&mut encoder);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn grey_16_bit_and_palette_pngs_are_normalised() {
        let grey16 = encode(
            1,
            1,
            ColorType::Grayscale,
            png::BitDepth::Sixteen,
            |_| {},
            &[0x80, 0xFF],
        );
        assert_eq!(decode(&grey16).unwrap().pixels, [0x80, 0x80, 0x80, 255]);

        let grey_alpha = encode(
            1,
            1,
            ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            |_| {},
            &[9, 33],
        );
        assert_eq!(decode(&grey_alpha).unwrap().pixels, [9, 9, 9, 33]);

        // Two palette entries; entry 1 is half transparent through tRNS.
        let palette = encode(
            2,
            1,
            ColorType::Indexed,
            png::BitDepth::Eight,
            |e| {
                e.set_palette(vec![10, 20, 30, 40, 50, 60]);
                e.set_trns(vec![255, 128]);
            },
            &[0, 1],
        );
        assert_eq!(
            decode(&palette).unwrap().pixels,
            [10, 20, 30, 255, 40, 50, 60, 128]
        );
    }

    #[test]
    fn png_over_the_dimension_cap_is_rejected() {
        let wide = png(
            MAX_DIMENSION + 1,
            1,
            ColorType::Rgb,
            &vec![0; (MAX_DIMENSION as usize + 1) * 3],
        );
        assert!(matches!(decode(&wide), Err(IconError::Malformed(_))));
        let at_cap = png(
            MAX_DIMENSION,
            1,
            ColorType::Rgb,
            &vec![0; MAX_DIMENSION as usize * 3],
        );
        assert_eq!(decode(&at_cap).unwrap().width, MAX_DIMENSION);
    }

    #[test]
    fn broken_png_data_is_malformed() {
        let valid = png(2, 2, ColorType::Rgb, &[0; 12]);
        assert!(matches!(decode(&[]), Err(IconError::Malformed(_))));
        assert!(matches!(
            decode(&valid[..valid.len() / 2]),
            Err(IconError::Malformed(_))
        ));
        assert!(matches!(
            decode(b"not a png at all"),
            Err(IconError::Malformed(_))
        ));
    }
}
