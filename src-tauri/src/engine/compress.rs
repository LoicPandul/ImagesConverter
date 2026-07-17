//! Fit an image into a byte budget.
//!
//! JPEG / WebP: binary search on quality (1..=95), then progressive downscale
//! as a last resort — same strategy as v1.
//! PNG: max lossless compression first, then lossy palette quantization
//! (imagequant, the pngquant library — built in, no external binary needed),
//! then downscale.

use image::imageops::FilterType;
use image::DynamicImage;
use rgb::FromSlice;

use super::encode;
use super::{EngineError, TargetFormat};

const RESIZE_SCALES: [f32; 9] = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1];
/// Same quality tiers pngquant is driven with in v1.
const PNG_QUALITY_TIERS: [(u8, u8); 4] = [(60, 80), (40, 60), (20, 40), (0, 20)];

pub struct CompressResult {
    pub data: Vec<u8>,
    pub resized_to: Option<(u32, u32)>,
    pub warning: Option<String>,
}

pub fn to_target_size(
    image: &DynamicImage,
    format: TargetFormat,
    target_bytes: u64,
) -> Result<CompressResult, EngineError> {
    match format {
        TargetFormat::Png => png_to_target(image, target_bytes),
        _ => lossy_to_target(image, format, target_bytes),
    }
}

fn fits(data: &[u8], target_bytes: u64) -> bool {
    data.len() as u64 <= target_bytes
}

fn resize(image: &DynamicImage, scale: f32) -> DynamicImage {
    let w = ((image.width() as f32 * scale) as u32).max(1);
    let h = ((image.height() as f32 * scale) as u32).max(1);
    if !image.color().has_alpha() {
        return image.resize_exact(w, h, FilterType::Lanczos3);
    }
    // Straight-alpha resampling bleeds the RGB hidden under transparent
    // pixels into the visible edges (background-colored halos on cutouts):
    // premultiply, resize, unpremultiply.
    let mut rgba = image.to_rgba8();
    for p in rgba.pixels_mut() {
        let a = u16::from(p.0[3]);
        for c in 0..3 {
            p.0[c] = ((u16::from(p.0[c]) * a) / 255) as u8;
        }
    }
    let mut resized = image::imageops::resize(&rgba, w, h, FilterType::Lanczos3);
    for p in resized.pixels_mut() {
        let a = u16::from(p.0[3]);
        for c in 0..3 {
            let unpremultiplied = (u16::from(p.0[c]) * 255).checked_div(a).unwrap_or(0);
            p.0[c] = unpremultiplied.min(255) as u8;
        }
    }
    DynamicImage::ImageRgba8(resized)
}

fn lossy_to_target(
    image: &DynamicImage,
    format: TargetFormat,
    target_bytes: u64,
) -> Result<CompressResult, EngineError> {
    // Already small enough at top quality?
    let at_best = encode::encode_lossy(image, format, 95)?;
    if fits(&at_best, target_bytes) {
        return Ok(CompressResult {
            data: at_best,
            resized_to: None,
            warning: None,
        });
    }

    // 95 just failed, so the full-size search starts at 94.
    if let Some(data) = quality_search(image, format, target_bytes, 94)? {
        return Ok(CompressResult {
            data,
            resized_to: None,
            warning: None,
        });
    }

    for scale in RESIZE_SCALES {
        let smaller = resize(image, scale);
        if let Some(data) = quality_search(&smaller, format, target_bytes, 95)? {
            return Ok(CompressResult {
                data,
                resized_to: Some((smaller.width(), smaller.height())),
                warning: None,
            });
        }
    }

    // Nothing fit: return the smallest possible output with a warning.
    let tiny = resize(image, 0.1);
    let data = encode::encode_lossy(&tiny, format, 1)?;
    Ok(CompressResult {
        resized_to: Some((tiny.width(), tiny.height())),
        warning: Some("could not reach the requested size".into()),
        data,
    })
}

/// Largest quality in 1..=high that fits the budget.
fn quality_search(
    image: &DynamicImage,
    format: TargetFormat,
    target_bytes: u64,
    high: u8,
) -> Result<Option<Vec<u8>>, EngineError> {
    let (mut low, mut high) = (1u8, high);
    let mut best = None;
    while low <= high {
        let mid = low + (high - low) / 2;
        let data = encode::encode_lossy(image, format, mid)?;
        if fits(&data, target_bytes) {
            best = Some(data);
            low = mid + 1;
        } else {
            if mid == 1 {
                break;
            }
            high = mid - 1;
        }
    }
    Ok(best)
}

fn png_to_target(image: &DynamicImage, target_bytes: u64) -> Result<CompressResult, EngineError> {
    let lossless = encode::encode_png_best(image)?;
    if fits(&lossless, target_bytes) {
        return Ok(CompressResult {
            data: lossless,
            resized_to: None,
            warning: None,
        });
    }

    // The RGBA copy is invariant across tiers — materialize it once.
    let full_rgba = image.to_rgba8();
    for (min_q, max_q) in PNG_QUALITY_TIERS {
        if let Some(data) = quantize(&full_rgba, min_q, max_q)? {
            if fits(&data, target_bytes) {
                return Ok(CompressResult {
                    data,
                    resized_to: None,
                    warning: None,
                });
            }
        }
    }

    drop(full_rgba);
    for scale in RESIZE_SCALES {
        let smaller = resize(image, scale).to_rgba8();
        if let Some(data) = quantize(&smaller, 0, 80)? {
            if fits(&data, target_bytes) {
                return Ok(CompressResult {
                    resized_to: Some((smaller.width(), smaller.height())),
                    data,
                    warning: None,
                });
            }
        }
    }

    // Best effort, like v1: hand back the best lossless encode with a warning.
    Ok(CompressResult {
        data: lossless,
        resized_to: None,
        warning: Some("could not reach the requested size".into()),
    })
}

/// Lossy PNG via palette quantization. Returns Ok(None) when the requested
/// quality floor cannot be met (same semantics as a failing pngquant tier).
fn quantize(
    rgba: &image::RgbaImage,
    min_quality: u8,
    max_quality: u8,
) -> Result<Option<Vec<u8>>, EngineError> {
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);

    let mut attributes = imagequant::new();
    attributes
        .set_quality(min_quality, max_quality)
        .map_err(|e| EngineError::Encode(format!("quantize: {e}")))?;

    let mut liq_image = attributes
        .new_image_borrowed(rgba.as_raw().as_rgba(), width, height, 0.0)
        .map_err(|e| EngineError::Encode(format!("quantize: {e}")))?;

    let mut result = match attributes.quantize(&mut liq_image) {
        Ok(result) => result,
        Err(imagequant::Error::QualityTooLow) => return Ok(None),
        Err(e) => return Err(EngineError::Encode(format!("quantize: {e}"))),
    };
    result
        .set_dithering_level(1.0)
        .map_err(|e| EngineError::Encode(format!("quantize: {e}")))?;

    let (palette, pixels) = result
        .remapped(&mut liq_image)
        .map_err(|e| EngineError::Encode(format!("quantize: {e}")))?;

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);
        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut trns = Vec::with_capacity(palette.len());
        for color in &palette {
            plte.extend_from_slice(&[color.r, color.g, color.b]);
            trns.push(color.a);
        }
        encoder.set_palette(plte);
        if trns.iter().any(|&a| a != u8::MAX) {
            encoder.set_trns(trns);
        }
        let mut writer = encoder
            .write_header()
            .map_err(|e| EngineError::Encode(format!("png: {e}")))?;
        writer
            .write_image_data(&pixels)
            .map_err(|e| EngineError::Encode(format!("png: {e}")))?;
    }
    Ok(Some(out))
}
