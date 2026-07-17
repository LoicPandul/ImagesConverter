//! Background removal with ISNet general-use (DIS, ONNX). The onnxruntime
//! library is loaded dynamically at runtime (`ort` load-dynamic): nothing is
//! linked at build time, both the runtime and the model are fetched on
//! first use.

use std::path::Path;
use std::sync::Mutex;

use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, Luma, RgbaImage};
use ndarray::Array4;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use super::EngineError;

/// ISNet is a fixed-size network.
const SIDE: u32 = 1024;
/// ISNet normalization (matches the reference rembg pipeline).
const MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const STD: [f32; 3] = [1.0, 1.0, 1.0];

/// Point ort at the onnxruntime dynamic library. Process-wide and
/// idempotent: only the first call performs the initialization.
pub fn init_runtime(dylib: &Path) -> Result<(), EngineError> {
    static INIT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    let path = dylib.to_string_lossy().into_owned();
    INIT.get_or_init(|| {
        ort::init_from(path)
            .commit()
            .map(|_| ())
            .map_err(|e| format!("onnxruntime init: {e}"))
    })
    .clone()
    .map_err(EngineError::Matting)
}

/// A loaded background-removal model. `matte` serializes calls: onnxruntime
/// already parallelizes internally across cores.
pub struct Matting {
    session: Mutex<Session>,
    input_name: String,
}

impl Matting {
    pub fn load(model: &Path) -> Result<Self, EngineError> {
        let session = Session::builder()
            .and_then(|b| b.with_optimization_level(GraphOptimizationLevel::Level3))
            .and_then(|b| b.with_intra_threads(num_threads()))
            .and_then(|b| b.commit_from_file(model))
            .map_err(|e| EngineError::Matting(format!("model load: {e}")))?;
        let input_name = session
            .inputs
            .first()
            .map(|i| i.name.clone())
            .ok_or_else(|| EngineError::Matting("model has no input".into()))?;
        Ok(Self {
            session: Mutex::new(session),
            input_name,
        })
    }

    /// Soft alpha matte at the image's own resolution (0 = background,
    /// 255 = subject).
    pub fn matte(&self, image: &DynamicImage) -> Result<GrayImage, EngineError> {
        let rgb = image
            .resize_exact(SIDE, SIDE, FilterType::Triangle)
            .to_rgb8();

        let side = SIDE as usize;
        let mut input = Array4::<f32>::zeros((1, 3, side, side));
        for (x, y, pixel) in rgb.enumerate_pixels() {
            for c in 0..3 {
                input[[0, c, y as usize, x as usize]] =
                    (pixel.0[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }

        let matte = {
            let session = self
                .session
                .lock()
                .map_err(|_| EngineError::Matting("model session poisoned".into()))?;
            let outputs = session
                .run(
                    ort::inputs![self.input_name.as_str() => input.view()]
                        .map_err(|e| EngineError::Matting(format!("inputs: {e}")))?,
                )
                .map_err(|e| EngineError::Matting(format!("inference: {e}")))?;
            let output = outputs
                .iter()
                .next()
                .ok_or_else(|| EngineError::Matting("model returned no output".into()))?
                .1;
            // The fp16 export yields f16 tensors, the fp32 one yields f32.
            if let Ok(view) = output.try_extract_tensor::<f32>() {
                view.iter().copied().collect::<Vec<f32>>()
            } else {
                let view = output
                    .try_extract_tensor::<half::f16>()
                    .map_err(|e| EngineError::Matting(format!("output: {e}")))?;
                view.iter().map(|v| v.to_f32()).collect::<Vec<f32>>()
            }
        };

        if matte.len() < side * side {
            return Err(EngineError::Matting(format!(
                "unexpected output size {}",
                matte.len()
            )));
        }

        // Min-max stretch, like the reference ISNet pipeline: the raw map is
        // in [0, 1] but rarely spans it fully.
        let plane = &matte[..side * side];
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for &v in plane {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let range = (hi - lo).max(f32::EPSILON);
        let small = GrayImage::from_fn(SIDE, SIDE, |x, y| {
            let v = (matte[y as usize * side + x as usize] - lo) / range;
            Luma([(v.clamp(0.0, 1.0) * 255.0).round() as u8])
        });
        Ok(image::imageops::resize(
            &small,
            image.width(),
            image.height(),
            FilterType::Triangle,
        ))
    }
}

/// Multiply the matte into the image's alpha channel (straight alpha).
/// Shoulders are clamped so near-misses become fully opaque/transparent,
/// which kills halos and helps compression.
pub fn apply_matte(image: &DynamicImage, matte: &GrayImage) -> RgbaImage {
    let mut rgba = image.to_rgba8();
    for (pixel, m) in rgba.pixels_mut().zip(matte.pixels()) {
        let a = shoulder(m.0[0]);
        pixel.0[3] = ((u16::from(pixel.0[3]) * u16::from(a)) / 255) as u8;
    }
    rgba
}

fn shoulder(a: u8) -> u8 {
    match a {
        0..=7 => 0,
        248..=255 => 255,
        other => other,
    }
}

fn num_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
