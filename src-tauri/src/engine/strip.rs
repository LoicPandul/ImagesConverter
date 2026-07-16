//! Lossless metadata removal: rewrite the container, drop metadata segments,
//! never touch encoded pixel data. Used when the file already has the target
//! format and needs no rotation. Any parsing surprise returns None and the
//! caller falls back to a re-encode.

use image::ImageFormat;

pub fn strip_metadata(bytes: &[u8], format: ImageFormat) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => strip_jpeg(bytes),
        ImageFormat::Png => strip_png(bytes),
        ImageFormat::WebP => strip_webp(bytes),
        _ => None,
    }
}

/// Drop APP1..APP13, APP15 and COM segments (EXIF, XMP, ICC, IPTC, comments).
/// APP0 (JFIF) and APP14 (Adobe color transform) stay: decoders rely on them.
fn strip_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[0..2]);
    let mut i = 2;
    loop {
        if i + 2 > bytes.len() {
            return None;
        }
        if bytes[i] != 0xFF {
            return None;
        }
        // Fill bytes are allowed between segments.
        while i + 2 <= bytes.len() && bytes[i + 1] == 0xFF {
            i += 1;
        }
        let marker = bytes[i + 1];
        match marker {
            // SOS: entropy-coded data follows — copy the rest verbatim.
            0xDA => {
                out.extend_from_slice(&bytes[i..]);
                return Some(out);
            }
            // Standalone markers (no length field).
            0x01 | 0xD0..=0xD7 => {
                out.extend_from_slice(&bytes[i..i + 2]);
                i += 2;
            }
            _ => {
                if i + 4 > bytes.len() {
                    return None;
                }
                let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
                if len < 2 || i + 2 + len > bytes.len() {
                    return None;
                }
                let keep = !matches!(marker, 0xE1..=0xED | 0xEF | 0xFE);
                if keep {
                    out.extend_from_slice(&bytes[i..i + 2 + len]);
                }
                i += 2 + len;
            }
        }
    }
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Chunks required for correct rendering. Everything else (tEXt, eXIf, tIME,
/// pHYs, iCCP, ...) is metadata and gets dropped.
const PNG_KEEP: [&[u8; 4]; 9] = [
    b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"IEND", b"sRGB", b"gAMA", b"cHRM", b"sBIT",
];

fn strip_png(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 8 || bytes[0..8] != PNG_SIGNATURE {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&PNG_SIGNATURE);
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        let chunk_type: &[u8; 4] = bytes[i + 4..i + 8].try_into().ok()?;
        let total = len.checked_add(12)?;
        if i + total > bytes.len() {
            return None;
        }
        if PNG_KEEP.contains(&chunk_type) {
            out.extend_from_slice(&bytes[i..i + total]);
        }
        if chunk_type == b"IEND" {
            return Some(out);
        }
        i += total;
    }
    None
}

/// WebP RIFF container: drop EXIF, XMP and ICCP chunks; clear the matching
/// feature bits in the VP8X header when present.
fn strip_webp(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let mut chunks: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let fourcc: [u8; 4] = bytes[i..i + 4].try_into().ok()?;
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().ok()?) as usize;
        let padded = len + (len & 1);
        if i + 8 + padded > bytes.len() {
            return None;
        }
        let data = bytes[i + 8..i + 8 + len].to_vec();
        chunks.push((fourcc, data));
        i += 8 + padded;
    }

    let is_metadata = |cc: &[u8; 4]| matches!(cc, b"EXIF" | b"XMP " | b"ICCP");
    let had_metadata = chunks.iter().any(|(cc, _)| is_metadata(cc));
    chunks.retain(|(cc, _)| !is_metadata(cc));

    // Nothing to strip: hand back the original bytes untouched.
    if !had_metadata {
        return Some(bytes.to_vec());
    }

    let mut body: Vec<u8> = Vec::with_capacity(bytes.len());
    body.extend_from_slice(b"WEBP");
    for (fourcc, data) in &mut chunks {
        if *fourcc == *b"VP8X" && !data.is_empty() {
            // Clear ICC (bit 5), EXIF (bit 3) and XMP (bit 2) feature flags.
            data[0] &= !0b0010_1100;
        }
        body.extend_from_slice(&fourcc[..]);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() & 1 == 1 {
            body.push(0);
        }
    }

    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_rejects_garbage() {
        assert!(strip_jpeg(b"not a jpeg").is_none());
    }

    #[test]
    fn png_rejects_garbage() {
        assert!(strip_png(b"not a png").is_none());
    }

    #[test]
    fn webp_rejects_garbage() {
        assert!(strip_webp(b"RIFFxxxxNOPE").is_none());
    }
}
