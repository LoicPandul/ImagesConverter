//! Image processing engine: format conversion, target-size compression and
//! metadata removal. Pure Rust, no Tauri types — fully testable on its own.

mod compress;
mod decode;
mod encode;
mod strip;

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use image::ImageFormat;

pub const SUPPORTED_EXTENSIONS: [&str; 8] =
    ["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff"];

pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    Jpeg,
    Webp,
    Png,
}

impl TargetFormat {
    /// Extension used for converted files (matches v1 output naming).
    pub fn extension(self) -> &'static str {
        match self {
            TargetFormat::Jpeg => "jpeg",
            TargetFormat::Webp => "webp",
            TargetFormat::Png => "png",
        }
    }

    fn matches(self, format: ImageFormat) -> bool {
        matches!(
            (self, format),
            (TargetFormat::Jpeg, ImageFormat::Jpeg)
                | (TargetFormat::Webp, ImageFormat::WebP)
                | (TargetFormat::Png, ImageFormat::Png)
        )
    }

    /// Is `ext` (lowercase) a correct extension for this format? Used to
    /// re-name mislabeled files (JPEG bytes in a `.png`) instead of cleaning
    /// them in place under the wrong extension.
    fn owns_extension(self, ext: &str) -> bool {
        match self {
            TargetFormat::Jpeg => matches!(ext, "jpg" | "jpeg"),
            TargetFormat::Webp => ext == "webp",
            TargetFormat::Png => ext == "png",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub format: TargetFormat,
    #[serde(default)]
    pub max_size_kb: Option<u64>,
    pub delete_original: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Re-encoded into a different format.
    Converted,
    /// Same format, reduced to fit the requested size budget.
    Compressed,
    /// Same format, metadata removed.
    Cleaned,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub action: Action,
    pub out_path: PathBuf,
    pub in_bytes: u64,
    pub out_bytes: u64,
    /// Set when the image had to be downscaled to reach the size budget.
    pub resized_to: Option<(u32, u32)>,
    /// True when metadata was removed without re-encoding pixels.
    pub lossless: bool,
    pub warning: Option<String>,
}

#[derive(Debug)]
pub enum EngineError {
    Unsupported(String),
    Animated,
    JpegTransparency,
    Io(io::Error),
    Decode(String),
    Encode(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Unsupported(ext) => {
                if ext.is_empty() {
                    write!(f, "unsupported file type")
                } else {
                    write!(f, "unsupported format .{ext}")
                }
            }
            EngineError::Animated => write!(f, "animated images are not supported"),
            EngineError::JpegTransparency => {
                write!(f, "image contains transparency, JPEG does not support it")
            }
            EngineError::Io(e) => write!(f, "file error: {e}"),
            EngineError::Decode(e) => write!(f, "could not read image: {e}"),
            EngineError::Encode(e) => write!(f, "could not encode image: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<io::Error> for EngineError {
    fn from(e: io::Error) -> Self {
        EngineError::Io(e)
    }
}

/// Convert / compress / clean a single file according to `opts`.
pub fn process_file(path: &Path, opts: &Options) -> Result<Outcome, EngineError> {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !is_supported_extension(&ext) {
        return Err(EngineError::Unsupported(ext));
    }

    let bytes = fs::read(path)?;
    let in_bytes = bytes.len() as u64;
    let decoded = decode::decode(&bytes)?;

    // Every processing path would flatten an animation to its first frame.
    if decode::is_animated(&bytes, decoded.format) {
        return Err(EngineError::Animated);
    }

    if opts.format == TargetFormat::Jpeg && decode::has_transparent_pixel(&decoded.image) {
        return Err(EngineError::JpegTransparency);
    }

    let same_format = opts.format.matches(decoded.format);
    let target_bytes = opts.max_size_kb.map(|kb| kb.saturating_mul(1024));
    let upright = decoded.orientation == 1;
    let strip_lossless = || strip::strip_metadata(&bytes, decoded.format);

    let (data, action, resized_to, lossless, warning) = match target_bytes {
        // Metadata clean only. When the image needs no rotation, strip
        // metadata without touching the pixels.
        None if same_format => match upright.then(strip_lossless).flatten() {
            Some(data) => (data, Action::Cleaned, None, true, None),
            None => (
                encode::encode_clean(&decoded.image, opts.format)?,
                Action::Cleaned,
                None,
                false,
                None,
            ),
        },
        // Plain conversion.
        None => (
            encode::encode_default(&decoded.image, opts.format)?,
            Action::Converted,
            None,
            false,
            None,
        ),
        // Size budget. A same-format file already under budget must not be
        // re-encoded (that could grow it and degrade pixels): a lossless
        // metadata strip — which can only shrink the file — is all it needs.
        Some(budget) => {
            let shortcut = (same_format && in_bytes <= budget && upright)
                .then(strip_lossless)
                .flatten();
            match shortcut {
                Some(data) => (data, Action::Cleaned, None, true, None),
                None => {
                    let result = compress::to_target_size(&decoded.image, opts.format, budget)?;
                    let action = if same_format {
                        Action::Compressed
                    } else {
                        Action::Converted
                    };
                    (
                        result.data,
                        action,
                        result.resized_to,
                        false,
                        result.warning,
                    )
                }
            }
        }
    };

    // A best-effort result that missed the size budget must never replace
    // the original.
    let delete_original = opts.delete_original && warning.is_none();

    let out_bytes = data.len() as u64;
    let plan = plan_output(path, &ext, opts.format, action, delete_original)?;
    let (out_path, in_place) = match plan {
        OutputPlan::InPlace => (path.to_path_buf(), true),
        OutputPlan::Reserved(p) => (p, false),
    };

    write_into_place(&out_path, &data, !in_place)?;

    if delete_original && !in_place {
        fs::remove_file(path)?;
    }

    Ok(Outcome {
        action,
        out_path,
        in_bytes,
        out_bytes,
        resized_to,
        lossless,
        warning,
    })
}

/// Small preview used by the UI file cards, returned as a data URI.
pub fn thumbnail_data_uri(path: &Path, max_dim: u32) -> Result<String, EngineError> {
    let bytes = fs::read(path)?;
    let thumb = decode::decode_thumbnail(&bytes, max_dim)?;
    if decode::has_transparent_pixel(&thumb) {
        let data = encode::encode_png_fast(&thumb)?;
        Ok(format!("data:image/png;base64,{}", BASE64.encode(data)))
    } else {
        let data = encode::encode_jpeg(&thumb, 80)?;
        Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(data)))
    }
}

enum OutputPlan {
    /// Atomically replace the source file itself.
    InPlace,
    /// A new path, already reserved on disk with an empty placeholder so
    /// concurrent workers can never plan the same output file.
    Reserved(PathBuf),
}

/// Output naming, matching v1 conventions, but collision-safe:
/// - keep original: `photo-converted.webp`, `photo-compressed.png`, `photo-clean.jpg`
/// - delete original: same-format work replaces the file in place; a format
///   change (or a mislabeled extension) writes `photo.webp` and removes the
///   original.
fn plan_output(
    path: &Path,
    orig_ext: &str,
    format: TargetFormat,
    action: Action,
    delete_original: bool,
) -> io::Result<OutputPlan> {
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("image");
    let extension_is_right = format.owns_extension(orig_ext);

    if delete_original && action != Action::Converted && extension_is_right {
        return Ok(OutputPlan::InPlace);
    }

    let (stem, ext) = if delete_original {
        (stem.to_string(), format.extension())
    } else {
        let suffix = match action {
            Action::Converted => "-converted",
            Action::Compressed => "-compressed",
            Action::Cleaned => "-clean",
        };
        let ext = if action != Action::Converted && extension_is_right {
            orig_ext
        } else {
            format.extension()
        };
        (format!("{stem}{suffix}"), ext)
    };

    reserve_unique(dir, &stem, ext).map(OutputPlan::Reserved)
}

/// Atomically claim the first free `dir/stem.ext`, `dir/stem (1).ext`, ... by
/// creating it with `create_new` — two parallel workers can never get the
/// same path.
fn reserve_unique(dir: &Path, stem: &str, ext: &str) -> io::Result<PathBuf> {
    let mut n = 0u32;
    loop {
        let name = if n == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let candidate = dir.join(name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `data` to a temp file in the destination directory, then move it
/// into place with `fs::rename`, which atomically replaces the destination on
/// both Unix and Windows (MOVEFILE_REPLACE_EXISTING). On any failure the
/// destination is left as it was — an original replaced in place can never be
/// lost.
fn write_into_place(dest: &Path, data: &[u8], dest_is_reservation: bool) -> io::Result<()> {
    let dir = dest.parent().unwrap_or_else(|| Path::new(""));
    let tmp = dir.join(format!(
        ".imagesconverter-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let cleanup = |e: io::Error| {
        let _ = fs::remove_file(&tmp);
        if dest_is_reservation {
            let _ = fs::remove_file(dest);
        }
        e
    };

    fs::write(&tmp, data).map_err(cleanup)?;
    fs::rename(&tmp, dest).map_err(cleanup)?;
    Ok(())
}
