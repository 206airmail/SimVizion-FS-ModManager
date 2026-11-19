//! Decoding mod icons (DDS or PNG) into small PNG thumbnails.
//!
//! Real icons are DXT1, DXT3, DXT5, BC7 (DX10 header) or uncompressed 24/32-bit DDS, or PNG.
//! Only mip 0 is decoded. Input is untrusted: malformed headers must return an error, never
//! panic or allocate without bound.

use std::fmt;

use crate::container::{IconLocation, ModContainer, ReadError};

/// Longest edge of a thumbnail, in pixels.
pub const THUMBNAIL_EDGE: u32 = 256;
/// Largest width or height accepted from an icon header.
pub const MAX_DIMENSION: u32 = 8192;

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

pub fn decode_dds(_bytes: &[u8]) -> Result<Rgba, IconError> {
    todo!("WS-C")
}

pub fn decode_png(_bytes: &[u8]) -> Result<Rgba, IconError> {
    todo!("WS-C")
}

/// Area-averaged downscale so the longest edge is at most `max_edge`; never upscales.
pub fn downscale(_image: &Rgba, _max_edge: u32) -> Rgba {
    todo!("WS-C")
}

pub fn encode_png(_image: &Rgba) -> Result<Vec<u8>, IconError> {
    todo!("WS-C")
}

/// Reads the icon from its container and returns PNG bytes with the longest edge at most
/// `max_edge`.
pub fn thumbnail(
    _container: &mut ModContainer,
    _icon: &IconLocation,
    _max_edge: u32,
) -> Result<Vec<u8>, IconError> {
    todo!("WS-C")
}
