use image::metadata::Orientation;
use image::{DynamicImage, GenericImageView, ImageFormat};

use super::EngineError;

pub struct Decoded {
    pub image: DynamicImage,
    pub format: ImageFormat,
    /// EXIF orientation value (1 = upright). The returned image is already
    /// rotated/flipped accordingly.
    pub orientation: u32,
    /// True when at least one pixel is not fully opaque.
    pub has_transparency: bool,
}

pub fn decode(bytes: &[u8]) -> Result<Decoded, EngineError> {
    let format = image::guess_format(bytes).map_err(|e| EngineError::Decode(e.to_string()))?;
    let mut image = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| EngineError::Decode(e.to_string()))?;

    let orientation = read_exif_orientation(bytes).unwrap_or(1);
    if orientation != 1 {
        if let Some(o) = Orientation::from_exif(orientation as u8) {
            image.apply_orientation(o);
        }
    }

    let has_transparency = has_transparent_pixel(&image);

    Ok(Decoded {
        image,
        format,
        orientation,
        has_transparency,
    })
}

fn read_exif_orientation(bytes: &[u8]) -> Option<u32> {
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

fn has_transparent_pixel(image: &DynamicImage) -> bool {
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
