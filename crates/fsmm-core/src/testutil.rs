//! Fixture builders for tests. Everything is generated into temp directories at test time:
//! no real mod files, icons or machine paths are stored in the repo.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, Clone, Copy)]
pub enum Method {
    Stored,
    Deflated,
}

/// Writes a zip. A name ending in `/` becomes a directory entry.
pub fn write_zip(path: &Path, entries: &[(&str, &[u8])], method: Method) {
    let method = match method {
        Method::Stored => CompressionMethod::Stored,
        Method::Deflated => CompressionMethod::Deflated,
    };
    let options = SimpleFileOptions::default().compression_method(method);
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, data) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, options).unwrap();
        } else {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
    }
    zip.finish().unwrap();
}

/// Writes an unpacked folder mod, creating subdirectories as needed.
pub fn write_folder(root: &Path, entries: &[(&str, &[u8])]) {
    for (name, data) in entries {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// How a generated DDS file declares its pixel format.
#[derive(Debug, Clone, Copy)]
pub enum DdsFormat {
    /// Legacy header with a FourCC such as `*b"DXT1"`.
    FourCc([u8; 4]),
    /// `DX10` FourCC followed by the extended header with this DXGI format (98 = BC7_UNORM).
    Dx10 { dxgi: u32 },
    /// Uncompressed: bit count and R, G, B, A masks (A = 0 means no alpha).
    Rgb { bits: u32, masks: [u32; 4] },
}

/// Builds a DDS file: magic, 124-byte header, optional DX10 header, then `data` verbatim.
pub fn dds(width: u32, height: u32, mip_count: u32, format: DdsFormat, data: &[u8]) -> Vec<u8> {
    const CAPS: u32 = 0x1;
    const HEIGHT: u32 = 0x2;
    const WIDTH: u32 = 0x4;
    const PITCH: u32 = 0x8;
    const PIXELFORMAT: u32 = 0x1000;
    const MIPMAPCOUNT: u32 = 0x2_0000;
    const LINEARSIZE: u32 = 0x8_0000;
    const PF_ALPHAPIXELS: u32 = 0x1;
    const PF_FOURCC: u32 = 0x4;
    const PF_RGB: u32 = 0x40;

    let (size_flag, pf_flags, fourcc, bits, masks) = match format {
        DdsFormat::FourCc(cc) => (LINEARSIZE, PF_FOURCC, cc, 0, [0; 4]),
        DdsFormat::Dx10 { .. } => (LINEARSIZE, PF_FOURCC, *b"DX10", 0, [0; 4]),
        DdsFormat::Rgb { bits, masks } => {
            let alpha = if masks[3] != 0 { PF_ALPHAPIXELS } else { 0 };
            (PITCH, PF_RGB | alpha, [0; 4], bits, masks)
        }
    };
    let mut flags = CAPS | HEIGHT | WIDTH | PIXELFORMAT | size_flag;
    if mip_count > 0 {
        flags |= MIPMAPCOUNT;
    }

    let mut out = Vec::with_capacity(148 + data.len());
    out.extend_from_slice(b"DDS ");
    for v in [124, flags, height, width, data.len() as u32, 0, mip_count] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0; 44]);
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&pf_flags.to_le_bytes());
    out.extend_from_slice(&fourcc);
    out.extend_from_slice(&bits.to_le_bytes());
    for m in masks {
        out.extend_from_slice(&m.to_le_bytes());
    }
    for v in [0x1000u32, 0, 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    if let DdsFormat::Dx10 { dxgi } = format {
        for v in [dxgi, 3, 0, 1, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    debug_assert_eq!(&out[4..8], &124u32.to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// Encodes 8-bit PNG pixels (`png::ColorType::Rgb` or `Rgba`).
pub fn png(width: u32, height: u32, color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    writer.finish().unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dds_header_has_the_documented_layout() {
        let legacy = dds(8, 4, 1, DdsFormat::FourCc(*b"DXT1"), &[0; 16]);
        assert_eq!(&legacy[..4], b"DDS ");
        assert_eq!(legacy.len(), 128 + 16);
        assert_eq!(&legacy[84..88], b"DXT1"); // pixel format FourCC
        assert_eq!(u32::from_le_bytes(legacy[12..16].try_into().unwrap()), 4); // height
        assert_eq!(u32::from_le_bytes(legacy[16..20].try_into().unwrap()), 8); // width

        let dx10 = dds(4, 4, 1, DdsFormat::Dx10 { dxgi: 98 }, &[0; 16]);
        assert_eq!(dx10.len(), 148 + 16);
        assert_eq!(u32::from_le_bytes(dx10[128..132].try_into().unwrap()), 98);

        let bgr = DdsFormat::Rgb {
            bits: 24,
            masks: [0xff0000, 0xff00, 0xff, 0],
        };
        let rgb = dds(1, 1, 0, bgr, &[0; 3]);
        assert_eq!(u32::from_le_bytes(rgb[80..84].try_into().unwrap()), 0x40); // RGB, no alpha
    }

    #[test]
    fn png_round_trips_through_the_decoder() {
        let bytes = png(2, 1, png::ColorType::Rgba, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, 2);
    }
}
