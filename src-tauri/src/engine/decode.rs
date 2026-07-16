use image::metadata::Orientation;
use image::{DynamicImage, GenericImageView, ImageFormat};

use super::EngineError;

pub struct Decoded {
    pub image: DynamicImage,
    pub format: ImageFormat,
    /// EXIF orientation value (1 = upright). The returned image is already
    /// rotated/flipped accordingly.
    pub orientation: u32,
}

pub fn decode(bytes: &[u8]) -> Result<Decoded, EngineError> {
    let format = image::guess_format(bytes).map_err(|e| EngineError::Decode(e.to_string()))?;
    let mut image = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| EngineError::Decode(e.to_string()))?;

    let orientation = read_exif_orientation(bytes).unwrap_or(1);
    apply_exif_orientation(&mut image, orientation);

    Ok(Decoded {
        image,
        format,
        orientation,
    })
}

/// Decode straight into an upright thumbnail: the rotate and any later pixel
/// scan run on the small image, not the full-resolution one.
pub fn decode_thumbnail(bytes: &[u8], max_dim: u32) -> Result<DynamicImage, EngineError> {
    let format = image::guess_format(bytes).map_err(|e| EngineError::Decode(e.to_string()))?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| EngineError::Decode(e.to_string()))?;
    let mut thumb = image.thumbnail(max_dim, max_dim);
    drop(image);
    apply_exif_orientation(&mut thumb, read_exif_orientation(bytes).unwrap_or(1));
    Ok(thumb)
}

fn apply_exif_orientation(image: &mut DynamicImage, orientation: u32) {
    if orientation != 1 {
        if let Some(o) = u8::try_from(orientation)
            .ok()
            .and_then(Orientation::from_exif)
        {
            image.apply_orientation(o);
        }
    }
}

fn read_exif_orientation(bytes: &[u8]) -> Option<u32> {
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

/// Multi-frame images are refused: every processing path would silently
/// flatten them to their first frame.
pub fn is_animated(bytes: &[u8], format: ImageFormat) -> bool {
    match format {
        ImageFormat::Gif => gif_has_second_frame(bytes),
        ImageFormat::Png => png_has_chunk(bytes, b"acTL"),
        ImageFormat::WebP => webp_has_chunk(bytes, b"ANIM"),
        _ => false,
    }
}

fn gif_has_second_frame(bytes: &[u8]) -> bool {
    use image::AnimationDecoder;
    image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))
        .map(|d| d.into_frames().nth(1).is_some_and(|f| f.is_ok()))
        .unwrap_or(false)
}

fn png_has_chunk(bytes: &[u8], wanted: &[u8; 4]) -> bool {
    if bytes.len() < 8 {
        return false;
    }
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let Ok(len) = <[u8; 4]>::try_from(&bytes[i..i + 4]).map(u32::from_be_bytes) else {
            return false;
        };
        let chunk_type = &bytes[i + 4..i + 8];
        if chunk_type == wanted {
            return true;
        }
        if chunk_type == b"IEND" {
            return false;
        }
        let Some(total) = (len as usize).checked_add(12) else {
            return false;
        };
        i = match i.checked_add(total) {
            Some(next) => next,
            None => return false,
        };
    }
    false
}

fn webp_has_chunk(bytes: &[u8], wanted: &[u8; 4]) -> bool {
    if bytes.len() < 12 {
        return false;
    }
    let mut i = 12;
    while i + 8 <= bytes.len() {
        if &bytes[i..i + 4] == wanted {
            return true;
        }
        let Ok(len) = <[u8; 4]>::try_from(&bytes[i + 4..i + 8]).map(u32::from_le_bytes) else {
            return false;
        };
        let padded = (len as usize) + (len as usize & 1);
        i = match i.checked_add(8 + padded) {
            Some(next) => next,
            None => return false,
        };
    }
    false
}

/// True when at least one pixel is not fully opaque.
pub fn has_transparent_pixel(image: &DynamicImage) -> bool {
    if !image.color().has_alpha() {
        return false;
    }
    match image {
        DynamicImage::ImageRgba8(img) => img.pixels().any(|p| p.0[3] != u8::MAX),
        DynamicImage::ImageLumaA8(img) => img.pixels().any(|p| p.0[1] != u8::MAX),
        DynamicImage::ImageRgba16(img) => img.pixels().any(|p| p.0[3] != u16::MAX),
        DynamicImage::ImageLumaA16(img) => img.pixels().any(|p| p.0[1] != u16::MAX),
        DynamicImage::ImageRgba32F(img) => img.pixels().any(|p| p.0[3] < 1.0),
        other => {
            let (w, h) = other.dimensions();
            (0..h).any(|y| (0..w).any(|x| other.get_pixel(x, y).0[3] != u8::MAX))
        }
    }
}
