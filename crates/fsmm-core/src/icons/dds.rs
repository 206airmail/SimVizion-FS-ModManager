//! DDS decoding: validates the header, then turns mip 0 into RGBA.
//!
//! Handles legacy FourCC DXT1/DXT3/DXT5, DX10 headers (BC1, BC2, BC3, BC7, B8G8R8A8,
//! R8G8B8A8) and uncompressed 24 or 32 bits per pixel RGB(A) described by channel masks.
//! The header is untrusted: every size is checked against `MAX_DIMENSION` and against the bytes
//! actually present before anything is allocated.

use super::{IconError, Rgba, MAX_DIMENSION};

const MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: u32 = 124;
/// Pixel data starts here when there is no DX10 extension header.
const DATA_START: usize = 128;
/// Pixel data starts here when the DX10 extension header (20 bytes) is present.
const DX10_DATA_START: usize = 148;

const PF_ALPHAPIXELS: u32 = 0x1;
const PF_FOURCC: u32 = 0x4;
const PF_RGB: u32 = 0x40;

/// Block-compressed formats that `bcdec_rs` decodes.
#[derive(Clone, Copy)]
enum Bc {
    Bc1,
    Bc2,
    Bc3,
    Bc7,
}

impl Bc {
    fn block_bytes(self) -> usize {
        match self {
            Bc::Bc1 => 8,
            _ => 16,
        }
    }
}

/// Uncompressed pixels: `bytes` per pixel (little-endian) and the R, G, B, A channel masks.
struct Packed {
    bytes: usize,
    masks: [u32; 4],
    has_alpha: bool,
}

enum Layout {
    Blocks(Bc),
    Packed(Packed),
}

pub(super) fn decode(bytes: &[u8]) -> Result<Rgba, IconError> {
    if bytes.len() < DATA_START {
        return Err(malformed("DDS header is truncated"));
    }
    if &bytes[..4] != MAGIC {
        return Err(malformed("missing DDS signature"));
    }
    if le_u32(bytes, 4) != HEADER_SIZE {
        return Err(malformed("DDS header size is not 124"));
    }
    let height = le_u32(bytes, 12);
    let width = le_u32(bytes, 16);
    if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
        return Err(malformed(format!(
            "DDS dimensions {width}x{height} are outside 1..={MAX_DIMENSION}"
        )));
    }

    let (layout, data_start) = read_pixel_format(bytes)?;
    let data = &bytes[data_start..];
    let (w, h) = (width as usize, height as usize);

    let required = match &layout {
        Layout::Blocks(bc) => (w.div_ceil(4) * h.div_ceil(4) * bc.block_bytes()) as u64,
        Layout::Packed(p) => (w * h * p.bytes) as u64,
    };
    if (data.len() as u64) < required {
        return Err(malformed(format!(
            "DDS pixel data is truncated: need {required} bytes, have {}",
            data.len()
        )));
    }

    let pixels = match &layout {
        Layout::Blocks(bc) => decode_blocks(*bc, w, h, data),
        Layout::Packed(p) => decode_packed(p, w, h, data)?,
    };
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// Works out the pixel layout and where the pixel data starts.
fn read_pixel_format(bytes: &[u8]) -> Result<(Layout, usize), IconError> {
    let flags = le_u32(bytes, 80);
    if flags & PF_FOURCC != 0 {
        let cc = [bytes[84], bytes[85], bytes[86], bytes[87]];
        return match &cc {
            b"DXT1" => Ok((Layout::Blocks(Bc::Bc1), DATA_START)),
            b"DXT3" => Ok((Layout::Blocks(Bc::Bc2), DATA_START)),
            b"DXT5" => Ok((Layout::Blocks(Bc::Bc3), DATA_START)),
            b"DX10" => {
                if bytes.len() < DX10_DATA_START {
                    return Err(malformed("DDS DX10 header is truncated"));
                }
                let layout = dxgi_layout(le_u32(bytes, 128))?;
                Ok((layout, DX10_DATA_START))
            }
            _ => Err(IconError::Unsupported(format!(
                "DDS FourCC {}",
                fourcc_name(cc)
            ))),
        };
    }
    if flags & PF_RGB != 0 {
        let bits = le_u32(bytes, 88);
        if bits != 24 && bits != 32 {
            return Err(IconError::Unsupported(format!(
                "uncompressed DDS with {bits} bits per pixel"
            )));
        }
        let masks = [
            le_u32(bytes, 92),
            le_u32(bytes, 96),
            le_u32(bytes, 100),
            le_u32(bytes, 104),
        ];
        let packed = Packed {
            bytes: (bits / 8) as usize,
            masks,
            has_alpha: flags & PF_ALPHAPIXELS != 0 && masks[3] != 0,
        };
        return Ok((Layout::Packed(packed), DATA_START));
    }
    Err(IconError::Unsupported(format!(
        "DDS pixel format flags {flags:#x}"
    )))
}

fn dxgi_layout(dxgi: u32) -> Result<Layout, IconError> {
    const BGRA: [u32; 4] = [0xff_0000, 0xff00, 0xff, 0xff00_0000];
    const RGBA: [u32; 4] = [0xff, 0xff00, 0xff_0000, 0xff00_0000];
    match dxgi {
        71 | 72 => Ok(Layout::Blocks(Bc::Bc1)),
        74 | 75 => Ok(Layout::Blocks(Bc::Bc2)),
        77 | 78 => Ok(Layout::Blocks(Bc::Bc3)),
        98 | 99 => Ok(Layout::Blocks(Bc::Bc7)),
        87 => Ok(Layout::Packed(Packed {
            bytes: 4,
            masks: BGRA,
            has_alpha: true,
        })),
        28 => Ok(Layout::Packed(Packed {
            bytes: 4,
            masks: RGBA,
            has_alpha: true,
        })),
        _ => Err(IconError::Unsupported(format!("DDS DXGI format {dxgi}"))),
    }
}

/// Decodes the padded block grid, copying only the visible part of edge blocks.
/// The caller has checked that `data` holds every block.
fn decode_blocks(bc: Bc, w: usize, h: usize, data: &[u8]) -> Vec<u8> {
    let (blocks_x, blocks_y) = (w.div_ceil(4), h.div_ceil(4));
    let block_bytes = bc.block_bytes();
    let mut pixels = vec![0u8; w * h * 4];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let offset = (by * blocks_x + bx) * block_bytes;
            let block = &data[offset..offset + block_bytes];
            // 4x4 RGBA, 16 bytes per row. Cleared per block so output never depends on the
            // previous block.
            let mut tmp = [0u8; 64];
            match bc {
                Bc::Bc1 => bcdec_rs::bc1(block, &mut tmp, 16),
                Bc::Bc2 => bcdec_rs::bc2(block, &mut tmp, 16),
                Bc::Bc3 => bcdec_rs::bc3(block, &mut tmp, 16),
                Bc::Bc7 => bcdec_rs::bc7(block, &mut tmp, 16),
            }
            let x0 = bx * 4;
            let columns = (w - x0).min(4);
            for row in 0..(h - by * 4).min(4) {
                let dst = ((by * 4 + row) * w + x0) * 4;
                pixels[dst..dst + columns * 4]
                    .copy_from_slice(&tmp[row * 16..row * 16 + columns * 4]);
            }
        }
    }
    pixels
}

/// One colour channel described by a bit mask.
struct Channel {
    shift: u32,
    max: u64,
}

impl Channel {
    /// The mask must be one contiguous run of set bits.
    fn new(mask: u32) -> Option<Channel> {
        if mask == 0 {
            return None;
        }
        let shift = mask.trailing_zeros();
        let max = u64::from(mask >> shift);
        (max & (max + 1) == 0).then_some(Channel { shift, max })
    }

    /// The channel's value in `pixel`, scaled to 0..=255.
    fn extract(&self, pixel: u32) -> u8 {
        let value = u64::from(pixel >> self.shift) & self.max;
        ((value * 255 + self.max / 2) / self.max) as u8
    }
}

fn decode_packed(p: &Packed, w: usize, h: usize, data: &[u8]) -> Result<Vec<u8>, IconError> {
    let channel = |mask: u32| {
        Channel::new(mask)
            .ok_or_else(|| IconError::Unsupported(format!("DDS channel mask {mask:#x}")))
    };
    let (red, green, blue) = (
        channel(p.masks[0])?,
        channel(p.masks[1])?,
        channel(p.masks[2])?,
    );
    let alpha = if p.has_alpha {
        Some(channel(p.masks[3])?)
    } else {
        None
    };

    let mut pixels = Vec::with_capacity(w * h * 4);
    for chunk in data.chunks_exact(p.bytes).take(w * h) {
        let value = chunk
            .iter()
            .rev()
            .fold(0u32, |acc, &byte| (acc << 8) | u32::from(byte));
        pixels.extend_from_slice(&[
            red.extract(value),
            green.extract(value),
            blue.extract(value),
            alpha.as_ref().map_or(255, |a| a.extract(value)),
        ]);
    }
    Ok(pixels)
}

/// Reads a little-endian `u32`; callers have already checked that `offset + 4` is in range.
fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn fourcc_name(cc: [u8; 4]) -> String {
    if cc.iter().all(|b| b.is_ascii_graphic()) {
        String::from_utf8_lossy(&cc).into_owned()
    } else {
        format!("{cc:02x?}")
    }
}

fn malformed(detail: impl Into<String>) -> IconError {
    IconError::Malformed(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{dds, DdsFormat};

    const BGR24: DdsFormat = DdsFormat::Rgb {
        bits: 24,
        masks: [0xff_0000, 0xff00, 0xff, 0],
    };
    const BGRA32: DdsFormat = DdsFormat::Rgb {
        bits: 32,
        masks: [0xff_0000, 0xff00, 0xff, 0xff00_0000],
    };

    const RED: u16 = 0xF800;
    const GREEN: u16 = 0x07E0;
    const BLUE: u16 = 0x001F;
    const WHITE: u16 = 0xFFFF;

    /// An 8-byte BC1 colour block: two RGB565 endpoints and 16 two-bit indices (pixel 0 lowest).
    fn bc1_block(c0: u16, c1: u16, indices: u32) -> Vec<u8> {
        let mut block = c0.to_le_bytes().to_vec();
        block.extend_from_slice(&c1.to_le_bytes());
        block.extend_from_slice(&indices.to_le_bytes());
        block
    }

    fn solid(c: u16) -> Vec<u8> {
        bc1_block(c, c, 0)
    }

    fn px(image: &Rgba, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * image.width + x) * 4) as usize;
        image.pixels[i..i + 4].try_into().unwrap()
    }

    fn dxt1(width: u32, height: u32, data: &[u8]) -> Vec<u8> {
        dds(width, height, 1, DdsFormat::FourCc(*b"DXT1"), data)
    }

    #[test]
    fn dxt1_solid_blocks_decode_to_their_colour() {
        let mut data = solid(RED);
        data.extend(solid(GREEN));
        let image = decode(&dxt1(8, 4, &data)).unwrap();
        assert_eq!((image.width, image.height), (8, 4));
        assert_eq!(px(&image, 0, 0), [255, 0, 0, 255]);
        assert_eq!(px(&image, 3, 3), [255, 0, 0, 255]);
        assert_eq!(px(&image, 4, 0), [0, 255, 0, 255]);
        assert_eq!(px(&image, 7, 3), [0, 255, 0, 255]);
    }

    #[test]
    fn dxt1_four_colour_mode_interpolates_when_color0_is_greater() {
        // c0 > c1: indices 0..3 are c0, c1, 2/3 c0 + 1/3 c1, 1/3 c0 + 2/3 c1.
        let indices = 0b11_10_01_00;
        let image = decode(&dxt1(4, 4, &bc1_block(WHITE, 0x0000, indices))).unwrap();
        assert_eq!(px(&image, 0, 0), [255, 255, 255, 255]);
        assert_eq!(px(&image, 1, 0), [0, 0, 0, 255]);
        assert_eq!(px(&image, 2, 0), [170, 170, 170, 255]);
        assert_eq!(px(&image, 3, 0), [85, 85, 85, 255]);
    }

    #[test]
    fn dxt1_one_bit_alpha_mode_makes_index_3_transparent() {
        // c0 <= c1: indices 0..3 are c0, c1, the midpoint, and transparent black.
        let indices = 0b11_10_01_00;
        let image = decode(&dxt1(4, 4, &bc1_block(BLUE, RED, indices))).unwrap();
        assert_eq!(px(&image, 0, 0), [0, 0, 255, 255]);
        assert_eq!(px(&image, 1, 0), [255, 0, 0, 255]);
        assert_eq!(px(&image, 2, 0), [128, 0, 128, 255]);
        assert_eq!(px(&image, 3, 0), [0, 0, 0, 0]);
        // Remaining pixels use index 0.
        assert_eq!(px(&image, 0, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn dxt3_uses_explicit_four_bit_alpha() {
        // Alpha: four 16-bit rows, one nibble per pixel (pixel 0 lowest), n * 17.
        let mut block = Vec::new();
        block.extend_from_slice(&0xFA50u16.to_le_bytes()); // row 0: 0, 5, 10, 15
        for _ in 0..3 {
            block.extend_from_slice(&0xFFFFu16.to_le_bytes());
        }
        block.extend(solid(RED));
        let image = decode(&dds(4, 4, 1, DdsFormat::FourCc(*b"DXT3"), &block)).unwrap();
        let row0: Vec<u8> = (0..4).map(|x| px(&image, x, 0)[3]).collect();
        assert_eq!(row0, [0, 85, 170, 255]);
        assert_eq!(px(&image, 2, 0)[..3], [255, 0, 0]);
        assert_eq!(px(&image, 0, 3), [255, 0, 0, 255]);
    }

    #[test]
    fn dxt5_interpolates_alpha_between_two_endpoints() {
        // alpha0 > alpha1: eight levels. alpha0 = 255, alpha1 = 0, then six interpolated:
        // (k * 0 + (7 - k) * 255 + 3) / 7 style values as bcdec computes them.
        let levels = [255u8, 0, 218, 182, 145, 109, 73, 36];
        // 3-bit indices, pixel 0 lowest: pixels 0..8 use levels 0..8, the rest use level 0.
        let mut bits = 0u64;
        for (p, level) in (0..8u64).enumerate() {
            bits |= level << (3 * p);
        }
        let mut block = vec![255, 0];
        block.extend_from_slice(&bits.to_le_bytes()[..6]);
        block.extend(solid(GREEN));
        let image = decode(&dds(4, 4, 1, DdsFormat::FourCc(*b"DXT5"), &block)).unwrap();
        for p in 0..16u32 {
            let expected = if p < 8 { levels[p as usize] } else { 255 };
            assert_eq!(px(&image, p % 4, p / 4), [0, 255, 0, expected], "pixel {p}");
        }
    }

    #[test]
    fn dxt5_with_alpha0_not_greater_uses_six_levels_plus_zero_and_opaque() {
        // alpha0 = 0, alpha1 = 255: four interpolated values, then 0 and 255.
        let levels = [0u8, 255, 51, 102, 153, 204, 0, 255];
        let mut bits = 0u64;
        for (p, level) in (0..8u64).enumerate() {
            bits |= level << (3 * p);
        }
        let mut block = vec![0, 255];
        block.extend_from_slice(&bits.to_le_bytes()[..6]);
        block.extend(solid(GREEN));
        let image = decode(&dds(4, 4, 1, DdsFormat::FourCc(*b"DXT5"), &block)).unwrap();
        for p in 0..8u32 {
            assert_eq!(px(&image, p % 4, p / 4)[3], levels[p as usize], "pixel {p}");
        }
    }

    /// Writes bit fields least-significant-bit first, as the BC7 bit stream is laid out.
    struct BitWriter {
        bytes: [u8; 16],
        position: usize,
    }

    impl BitWriter {
        fn new() -> Self {
            Self {
                bytes: [0; 16],
                position: 0,
            }
        }

        fn push(&mut self, value: u32, bits: usize) {
            for i in 0..bits {
                if (value >> i) & 1 == 1 {
                    self.bytes[self.position / 8] |= 1 << (self.position % 8);
                }
                self.position += 1;
            }
        }
    }

    /// A BC7 mode 6 block (one subset, 7-bit RGBA endpoints plus one P bit each, 4-bit indices).
    ///
    /// Bit layout, 128 bits, LSB first:
    /// - mode: six 0 bits then a 1 (value 0b1000000 in 7 bits);
    /// - R0, R1, G0, G1, B0, B1, A0, A1, 7 bits each (56 bits), in that order;
    /// - P0 and P1, one bit each, which become the lowest bit of every channel of the endpoint;
    /// - 16 pixel indices, row-major: the anchor pixel 0 has 3 bits (its top bit is implied 0),
    ///   the other 15 have 4 bits. 7 + 56 + 2 + 3 + 60 = 128.
    ///
    /// An endpoint channel is `(value7 << 1) | P`. Index `i` blends the endpoints with the 4-bit
    /// weight table, whose entries 0 and 15 are 0 and 64, so index 0 is exactly endpoint 0 and
    /// index 15 is exactly endpoint 1. Here pixel 0 has index 0 and every other pixel index 15.
    fn bc7_mode6_block(e0: [u32; 4], p0: u32, e1: [u32; 4], p1: u32) -> [u8; 16] {
        let mut w = BitWriter::new();
        w.push(0b100_0000, 7);
        for c in 0..4 {
            w.push(e0[c], 7);
            w.push(e1[c], 7);
        }
        w.push(p0, 1);
        w.push(p1, 1);
        w.push(0, 3);
        for _ in 0..15 {
            w.push(15, 4);
        }
        assert_eq!(w.position, 128);
        w.bytes
    }

    #[test]
    fn bc7_mode6_block_decodes_to_the_spec_values() {
        // Endpoint 0: (100, 50, 20, 127) with P = 1 -> (201, 101, 41, 255).
        // Endpoint 1: (10, 20, 30, 40) with P = 0 -> (20, 40, 60, 80).
        let block = bc7_mode6_block([100, 50, 20, 127], 1, [10, 20, 30, 40], 0);
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 98 }, &block)).unwrap();
        assert_eq!(px(&image, 0, 0), [201, 101, 41, 255]);
        for p in 1..16u32 {
            assert_eq!(px(&image, p % 4, p / 4), [20, 40, 60, 80], "pixel {p}");
        }
    }

    #[test]
    fn dx10_bc1_and_bc7_srgb_variants_decode() {
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 71 }, &solid(RED))).unwrap();
        assert_eq!(px(&image, 2, 2), [255, 0, 0, 255]);
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 72 }, &solid(RED))).unwrap();
        assert_eq!(px(&image, 2, 2), [255, 0, 0, 255]);
        let block = bc7_mode6_block([1, 2, 3, 4], 0, [5, 6, 7, 8], 0);
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 99 }, &block)).unwrap();
        assert_eq!(px(&image, 0, 0), [2, 4, 6, 8]);
    }

    #[test]
    fn dx10_bc2_and_bc3_decode() {
        let mut block = vec![0xFF; 8]; // opaque explicit alpha
        block.extend(solid(BLUE));
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 74 }, &block)).unwrap();
        assert_eq!(px(&image, 1, 1), [0, 0, 255, 255]);
        let mut block = vec![255, 255, 0, 0, 0, 0, 0, 0]; // alpha 255 everywhere
        block.extend(solid(BLUE));
        let image = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 77 }, &block)).unwrap();
        assert_eq!(px(&image, 1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn dx10_uncompressed_formats_decode() {
        // B8G8R8A8: bytes are B, G, R, A.
        let image = decode(&dds(
            1,
            1,
            1,
            DdsFormat::Dx10 { dxgi: 87 },
            &[10, 20, 30, 40],
        ))
        .unwrap();
        assert_eq!(image.pixels, [30, 20, 10, 40]);
        // R8G8B8A8: bytes are R, G, B, A.
        let image = decode(&dds(
            1,
            1,
            1,
            DdsFormat::Dx10 { dxgi: 28 },
            &[10, 20, 30, 40],
        ))
        .unwrap();
        assert_eq!(image.pixels, [10, 20, 30, 40]);
    }

    #[test]
    fn uncompressed_bgr24_and_bgra32_use_the_masks() {
        let image = decode(&dds(2, 1, 0, BGR24, &[10, 20, 30, 1, 2, 3])).unwrap();
        assert_eq!(image.pixels, [30, 20, 10, 255, 3, 2, 1, 255]);
        let image = decode(&dds(1, 1, 0, BGRA32, &[10, 20, 30, 40])).unwrap();
        assert_eq!(image.pixels, [30, 20, 10, 40]);
    }

    #[test]
    fn uncompressed_32_bit_without_an_alpha_mask_is_opaque() {
        let format = DdsFormat::Rgb {
            bits: 32,
            masks: [0xff_0000, 0xff00, 0xff, 0],
        };
        let image = decode(&dds(1, 1, 1, format, &[10, 20, 30, 99])).unwrap();
        assert_eq!(image.pixels, [30, 20, 10, 255]);
    }

    #[test]
    fn uncompressed_masks_other_than_8_bits_are_scaled() {
        // 32 bits with a 5-6-5 style R, G, B in the low bits: 0x1F, 0x3F, 0x1F scale to 255.
        let format = DdsFormat::Rgb {
            bits: 32,
            masks: [0x1F, 0x7E0, 0xF800, 0],
        };
        let value = 0x1Fu32 | (0x3F << 5);
        let image = decode(&dds(1, 1, 1, format, &value.to_le_bytes())).unwrap();
        assert_eq!(image.pixels, [255, 255, 0, 255]);
    }

    #[test]
    fn mip_count_of_zero_or_many_decodes_mip_zero_only() {
        let mut data = solid(RED);
        data.extend_from_slice(&[0xAB; 100]); // pretend mip chain
        for mips in [0, 1, 10] {
            let image = decode(&dds(4, 4, mips, DdsFormat::FourCc(*b"DXT1"), &data)).unwrap();
            assert_eq!((image.width, image.height), (4, 4));
            assert_eq!(px(&image, 3, 3), [255, 0, 0, 255], "mips {mips}");
        }
    }

    #[test]
    fn sizes_that_are_not_multiples_of_four_crop_the_block_grid() {
        // 6x5 needs 2x2 blocks: top row red, green; bottom row blue, white.
        let mut data = solid(RED);
        data.extend(solid(GREEN));
        data.extend(solid(BLUE));
        data.extend(solid(WHITE));
        let image = decode(&dxt1(6, 5, &data)).unwrap();
        assert_eq!((image.width, image.height), (6, 5));
        assert_eq!(image.pixels.len(), 6 * 5 * 4);
        assert_eq!(px(&image, 3, 3), [255, 0, 0, 255]);
        assert_eq!(px(&image, 5, 0), [0, 255, 0, 255]);
        assert_eq!(px(&image, 5, 3), [0, 255, 0, 255]);
        assert_eq!(px(&image, 0, 4), [0, 0, 255, 255]);
        assert_eq!(px(&image, 5, 4), [255, 255, 255, 255]);
    }

    #[test]
    fn tiny_sizes_still_need_one_block() {
        let image = decode(&dxt1(1, 1, &solid(RED))).unwrap();
        assert_eq!(image.pixels, [255, 0, 0, 255]);
    }

    fn base() -> Vec<u8> {
        dxt1(4, 4, &solid(RED))
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn truncated_and_inconsistent_headers_are_malformed() {
        let valid = base();
        assert!(decode(&valid).is_ok());

        assert!(matches!(decode(&[]), Err(IconError::Malformed(_))));
        assert!(matches!(
            decode(&valid[..100]),
            Err(IconError::Malformed(_))
        ));

        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert!(matches!(decode(&bad_magic), Err(IconError::Malformed(_))));

        let mut bad_size = valid.clone();
        put_u32(&mut bad_size, 4, 123);
        assert!(matches!(decode(&bad_size), Err(IconError::Malformed(_))));

        for (offset, value) in [(16, 0), (12, 0), (16, MAX_DIMENSION + 1), (12, u32::MAX)] {
            let mut bad = valid.clone();
            put_u32(&mut bad, offset, value);
            assert!(
                matches!(decode(&bad), Err(IconError::Malformed(_))),
                "offset {offset} value {value}"
            );
        }
    }

    #[test]
    fn data_shorter_than_mip_zero_is_malformed() {
        // 8x8 DXT1 needs 4 blocks (32 bytes); only one is present.
        assert!(matches!(
            decode(&dxt1(8, 8, &solid(RED))),
            Err(IconError::Malformed(_))
        ));
        // Largest allowed size with almost no data must not allocate or panic.
        assert!(matches!(
            decode(&dxt1(MAX_DIMENSION, MAX_DIMENSION, &solid(RED))),
            Err(IconError::Malformed(_))
        ));
        assert!(matches!(
            decode(&dds(2, 2, 1, BGR24, &[0; 11])),
            Err(IconError::Malformed(_))
        ));
        // DX10 header cut short.
        let dx10 = dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 98 }, &[0; 16]);
        assert!(matches!(decode(&dx10[..140]), Err(IconError::Malformed(_))));
    }

    #[test]
    fn unsupported_formats_name_what_they_are() {
        let err = decode(&dds(4, 4, 1, DdsFormat::FourCc(*b"ATI2"), &[0; 16])).unwrap_err();
        assert!(
            matches!(&err, IconError::Unsupported(d) if d.contains("ATI2")),
            "{err}"
        );

        let err = decode(&dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 83 }, &[0; 16])).unwrap_err();
        assert!(
            matches!(&err, IconError::Unsupported(d) if d.contains("83")),
            "{err}"
        );

        let sixteen = DdsFormat::Rgb {
            bits: 16,
            masks: [0xF800, 0x7E0, 0x1F, 0],
        };
        let err = decode(&dds(2, 2, 1, sixteen, &[0; 8])).unwrap_err();
        assert!(
            matches!(&err, IconError::Unsupported(d) if d.contains("16")),
            "{err}"
        );

        let no_mask = DdsFormat::Rgb {
            bits: 32,
            masks: [0xff, 0, 0xff00, 0],
        };
        assert!(matches!(
            decode(&dds(1, 1, 1, no_mask, &[0; 4])),
            Err(IconError::Unsupported(_))
        ));

        // Neither FourCC nor RGB flag (for example a luminance format).
        let mut other = base();
        put_u32(&mut other, 80, 0x20000);
        assert!(matches!(decode(&other), Err(IconError::Unsupported(_))));
    }

    #[test]
    fn non_text_fourcc_is_reported_in_hex() {
        let err = decode(&dds(4, 4, 1, DdsFormat::FourCc([1, 2, 3, 4]), &[0; 16])).unwrap_err();
        assert!(
            matches!(&err, IconError::Unsupported(d) if d.contains("01")),
            "{err}"
        );
    }

    /// Deterministic pseudo-random bytes (xorshift), so failures reproduce.
    fn noise(seed: u64, len: usize) -> Vec<u8> {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn random_payloads_decode_without_panicking() {
        let formats = [
            DdsFormat::FourCc(*b"DXT1"),
            DdsFormat::FourCc(*b"DXT3"),
            DdsFormat::FourCc(*b"DXT5"),
            DdsFormat::Dx10 { dxgi: 98 },
            DdsFormat::Dx10 { dxgi: 28 },
            BGR24,
            BGRA32,
        ];
        for (n, format) in formats.into_iter().enumerate() {
            for seed in 0..40u64 {
                let data = noise(seed * 7 + n as u64, 16 * 12);
                let image = decode(&dds(7, 9, 1, format, &data));
                if let Ok(image) = image {
                    assert_eq!(image.pixels.len(), 7 * 9 * 4);
                }
            }
        }
    }

    #[test]
    fn random_bytes_and_mutated_headers_never_panic() {
        let valid = base();
        for seed in 0..200u64 {
            let _ = decode(&noise(seed, 300));
            let mut mutated = valid.clone();
            let noise = noise(seed, 8);
            let at = (noise[0] as usize) % mutated.len();
            mutated[at] = noise[1];
            let at = (noise[2] as usize) % 128;
            mutated[at] ^= noise[3];
            let _ = decode(&mutated);
        }
    }
}
