//! Colour-independent monochrome Facing Brackets tray icon generator and PNG/ICO encoder.
//!
//! Three distinct geometric silhouettes ensure state is never conveyed by colour alone:
//! - `Idle`: Open facing brackets with an empty center.
//! - `InSession`: Facing brackets enclosing a solid filled center dot.
//! - `Alert`: Facing brackets with a center exclamation mark (`!`) and top-right diamond.

use serde::{Deserialize, Serialize};

/// Monochrome ink colour (`#F5F6F4`) used for all opaque tray pixels so state is 100%
/// colour-independent and legible against dark taskbars.
pub const MONOCHROME_TRAY_INK: [u8; 3] = [0xF5, 0xF6, 0xF4];

/// Monochrome ink colour (`#14181B`) used for app installer/bundle icon variants.
pub const MONOCHROME_BUNDLE_INK: [u8; 3] = [0x14, 0x18, 0x1B];

/// Host tray icon states matching the Oudos Facing Brackets specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayVisualState {
    /// No active or blocked session; open facing brackets with empty center.
    Idle,
    /// Active `SessionGrant` live; facing brackets enclosing a solid center dot.
    InSession,
    /// Security halt or blocked attempt; facing brackets with `!` and top-right diamond.
    Alert,
}

impl TrayVisualState {
    /// Human-readable status text for tray tooltips and screen readers.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::InSession => "In Session",
            Self::Alert => "Alert",
        }
    }
}

/// Renders a `32x32` RGBA8 monochrome Facing Brackets tray icon for `state`.
#[must_use]
pub fn render_tray_rgba(state: TrayVisualState) -> Vec<u8> {
    render_monochrome_brackets_rgba(state, 32, MONOCHROME_TRAY_INK)
}

/// Renders a square `size x size` RGBA8 monochrome Facing Brackets icon using a single `ink` RGB.
#[must_use]
pub fn render_monochrome_brackets_rgba(state: TrayVisualState, size: u32, ink: [u8; 3]) -> Vec<u8> {
    let dim_u32 = size.max(16);
    let dim = usize::try_from(dim_u32).unwrap_or(32);
    let mut buf = vec![0u8; dim * dim * 4];
    let dim_f = f64::from(dim_u32);

    for y in 0..dim_u32 {
        for x in 0..dim_u32 {
            // Normalize pixel center onto the canonical 32x32 grid.
            let gx = ((f64::from(x) + 0.5) * 32.0) / dim_f;
            let gy = ((f64::from(y) + 0.5) * 32.0) / dim_f;

            let in_left_bracket = is_in_left_bracket(gx, gy);
            let in_right_bracket = is_in_left_bracket(32.0 - gx, gy);

            let in_state_glyph = match state {
                TrayVisualState::Idle => false,
                TrayVisualState::InSession => {
                    let dx = gx - 16.0;
                    let dy = gy - 16.0;
                    (dx * dx + dy * dy) <= 3.2 * 3.2
                }
                TrayVisualState::Alert => {
                    let in_exclaim_stem =
                        (14.8..=17.2).contains(&gx) && (10.5..=17.2).contains(&gy);
                    let dot_offset_x = gx - 16.0;
                    let dot_offset_y = gy - 20.8;
                    let in_exclaim_dot =
                        (dot_offset_x * dot_offset_x + dot_offset_y * dot_offset_y) <= 1.5 * 1.5;
                    let in_diamond = ((gx - 27.0).abs() + (gy - 5.0).abs()) <= 3.2;
                    in_exclaim_stem || in_exclaim_dot || in_diamond
                }
            };

            if in_left_bracket || in_right_bracket || in_state_glyph {
                let x_idx = usize::try_from(x).unwrap_or(0);
                let y_idx = usize::try_from(y).unwrap_or(0);
                let idx = (y_idx * dim + x_idx) * 4;
                buf[idx] = ink[0];
                buf[idx + 1] = ink[1];
                buf[idx + 2] = ink[2];
                buf[idx + 3] = 0xFF;
            }
        }
    }

    buf
}

fn is_in_left_bracket(gx: f64, gy: f64) -> bool {
    let in_spine = (4.5..=7.3).contains(&gx) && (7.8..=24.2).contains(&gy);
    let in_top_arm = (5.8..=11.4).contains(&gx) && (6.2..=9.0).contains(&gy);
    let in_bot_arm = (5.8..=11.4).contains(&gx) && (23.0..=25.8).contains(&gy);
    in_spine || in_top_arm || in_bot_arm
}

/// Encodes raw `RGBA8` pixels as a valid uncompressed-deflate PNG byte stream.
#[must_use]
pub fn encode_rgba_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row_bytes = (width as usize) * 4;
    let mut scanlines = Vec::with_capacity((row_bytes + 1) * (height as usize));
    for row in 0..(height as usize) {
        scanlines.push(0u8); // Filter type 0 (None)
        let start = row * row_bytes;
        scanlines.extend_from_slice(&rgba[start..start + row_bytes]);
    }

    let zlib_stream = encode_zlib_stored(&scanlines);

    let mut out = Vec::with_capacity(zlib_stream.len() + 64);
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    push_png_chunk(&mut out, *b"IHDR", &ihdr);
    push_png_chunk(&mut out, *b"IDAT", &zlib_stream);
    push_png_chunk(&mut out, *b"IEND", &[]);
    out
}

/// Encodes a `32x32` `RGBA8` buffer as a valid Windows `.ico` file using 32-bit DIB BMP data.
#[must_use]
pub fn encode_rgba_ico_32(rgba: &[u8]) -> Vec<u8> {
    let width = 32usize;
    let height = 32usize;
    let dib_pixel_bytes = width * height * 4;
    let and_mask_bytes = (width / 8) * height; // 4 bytes per row * 32 = 128 bytes
    let image_size = 40 + dib_pixel_bytes + and_mask_bytes;

    let mut out = Vec::with_capacity(22 + image_size);
    // ICONDIR
    out.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // Icon type
    out.extend_from_slice(&1u16.to_le_bytes()); // Image count = 1

    // ICONDIRENTRY
    out.push(32); // Width
    out.push(32); // Height
    out.push(0); // Color count
    out.push(0); // Reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // Planes
    out.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel
    out.extend_from_slice(&u32::try_from(image_size).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes()); // Offset

    // BITMAPINFOHEADER (40 bytes)
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&32i32.to_le_bytes()); // biWidth
    out.extend_from_slice(&64i32.to_le_bytes()); // biHeight (doubled for XOR + AND)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    out.extend_from_slice(&u32::try_from(dib_pixel_bytes).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    // Bottom-up BGRA scanlines
    for y in (0..height).rev() {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            out.push(rgba[idx + 2]); // B
            out.push(rgba[idx + 1]); // G
            out.push(rgba[idx]); // R
            out.push(rgba[idx + 3]); // A
        }
    }

    // All-zero AND mask
    out.resize(out.len() + and_mask_bytes, 0);
    out
}

fn encode_zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + 32);
    out.extend_from_slice(&[0x78, 0x01]); // Zlib CMF/FLG (no compression)

    let mut offset = 0usize;
    while offset < raw.len() || raw.is_empty() {
        let remaining = raw.len().saturating_sub(offset);
        let chunk_len = remaining.min(65_535);
        let is_final = offset + chunk_len >= raw.len();
        out.push(u8::from(is_final));
        let len_u16 = u16::try_from(chunk_len).unwrap_or(u16::MAX);
        out.extend_from_slice(&len_u16.to_le_bytes());
        out.extend_from_slice(&(!len_u16).to_le_bytes());
        out.extend_from_slice(&raw[offset..offset + chunk_len]);
        offset += chunk_len;
        if raw.is_empty() {
            break;
        }
    }

    let adler = adler32(raw);
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

fn push_png_chunk(out: &mut Vec<u8>, tag: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(&tag);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(&tag);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
