use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::DynamicImage;

use super::{EngineError, TargetFormat};

/// Defaults used when no size budget is requested.
pub const JPEG_QUALITY: u8 = 85;
pub const WEBP_QUALITY: u8 = 82;
/// Near-lossless quality for metadata cleaning when a re-encode is required
/// (rotated image). v1 silently re-saved at quality 75.
pub const CLEAN_QUALITY: u8 = 95;

pub fn encode_default(image: &DynamicImage, format: TargetFormat) -> Result<Vec<u8>, EngineError> {
    match format {
        TargetFormat::Jpeg => encode_jpeg(image, JPEG_QUALITY),
        TargetFormat::Webp => encode_webp(image, WEBP_QUALITY),
        TargetFormat::Png => encode_png_best(image),
    }
}

pub fn encode_clean(image: &DynamicImage, format: TargetFormat) -> Result<Vec<u8>, EngineError> {
    match format {
        TargetFormat::Jpeg => encode_jpeg(image, CLEAN_QUALITY),
        TargetFormat::Webp => encode_webp(image, CLEAN_QUALITY),
        TargetFormat::Png => encode_png_best(image),
    }
}

/// Encoder used by the target-size search (PNG has no quality knob).
pub fn encode_lossy(
    image: &DynamicImage,
    format: TargetFormat,
    quality: u8,
) -> Result<Vec<u8>, EngineError> {
    match format {
        TargetFormat::Jpeg => encode_jpeg(image, quality),
        TargetFormat::Webp => encode_webp(image, quality),
        TargetFormat::Png => encode_png_best(image),
    }
}

pub fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, EngineError> {
    let rgb = image.to_rgb8();
    let mut out = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut out, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|e| EngineError::Encode(e.to_string()))?;
    Ok(out)
}

pub fn encode_webp(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, EngineError> {
    let rgba = image.to_rgba8();
    let encoder = webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height());
    let memory = encoder
        .encode_simple(false, f32::from(quality))
        .map_err(|e| EngineError::Encode(format!("webp: {e:?}")))?;
    Ok(memory.to_vec())
}

pub fn encode_png_best(image: &DynamicImage) -> Result<Vec<u8>, EngineError> {
    encode_png(image, CompressionType::Best)
}

pub fn encode_png_fast(image: &DynamicImage) -> Result<Vec<u8>, EngineError> {
    encode_png(image, CompressionType::Fast)
}

fn encode_png(image: &DynamicImage, compression: CompressionType) -> Result<Vec<u8>, EngineError> {
    // PNG supports 8/16-bit gray/rgb with or without alpha; normalize other
    // layouts to rgba8/rgb8 first.
    let normalized: DynamicImage = match image {
        DynamicImage::ImageLuma8(_)
        | DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgba8(_)
        | DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_) => image.clone(),
        other if other.color().has_alpha() => DynamicImage::ImageRgba8(other.to_rgba8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    };
    let mut out = Vec::new();
    let encoder = PngEncoder::new_with_quality(&mut out, compression, FilterType::Adaptive);
    normalized
        .write_with_encoder(encoder)
        .map_err(|e| EngineError::Encode(e.to_string()))?;
    Ok(out)
}
