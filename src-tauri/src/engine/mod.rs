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

    if opts.format == TargetFormat::Jpeg && decoded.has_transparency {
        return Err(EngineError::JpegTransparency);
    }

    let same_format = opts.format.matches(decoded.format);
    let target_bytes = opts.max_size_kb.map(|kb| kb.saturating_mul(1024));

    let (data, action, resized_to, lossless, warning) = match (same_format, target_bytes) {
        // Same format, no size budget: metadata clean only. When the image
        // needs no rotation, strip metadata without touching the pixels.
        (true, None) => {
            let stripped = (decoded.orientation == 1)
                .then(|| strip::strip_metadata(&bytes, decoded.format))
                .flatten();
            match stripped {
                Some(data) => (data, Action::Cleaned, None, true, None),
                None => (
                    encode::encode_clean(&decoded.image, opts.format)?,
                    Action::Cleaned,
                    None,
                    false,
                    None,
                ),
            }
        }
        (same, Some(budget)) => {
            let result = compress::to_target_size(&decoded.image, opts.format, budget)?;
            let action = if same {
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
        (false, None) => (
            encode::encode_default(&decoded.image, opts.format)?,
            Action::Converted,
            None,
            false,
            None,
        ),
    };

    let out_path = plan_output_path(path, opts, action);
    let out_bytes = data.len() as u64;
    let in_place = out_path == path;

    write_atomic(&out_path, &data, in_place)?;

    if opts.delete_original && !in_place {
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
    let decoded = decode::decode(&bytes)?;
    let thumb = decoded.image.thumbnail(max_dim, max_dim);
    if decoded.has_transparency {
        let data = encode::encode_png_fast(&thumb)?;
        Ok(format!("data:image/png;base64,{}", BASE64.encode(data)))
    } else {
        let data = encode::encode_jpeg(&thumb, 80)?;
        Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(data)))
    }
}

/// Output naming, matching v1 conventions, but collision-safe:
/// - keep original: `photo-converted.webp`, `photo-compressed.png`, `photo-clean.jpg`
/// - delete original: same-format work replaces the file in place; a format
///   change writes `photo.webp` next to it and removes the original.
fn plan_output_path(path: &Path, opts: &Options, action: Action) -> PathBuf {
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("image")
        .to_string();
    let orig_ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if opts.delete_original {
        match action {
            Action::Converted => unique_path(dir, &stem, opts.format.extension(), Some(path)),
            // In-place replacement.
            _ => path.to_path_buf(),
        }
    } else {
        let (suffix, ext) = match action {
            Action::Converted => ("-converted", opts.format.extension().to_string()),
            Action::Compressed => ("-compressed", orig_ext),
            Action::Cleaned => ("-clean", orig_ext),
        };
        unique_path(dir, &format!("{stem}{suffix}"), &ext, Some(path))
    }
}

/// First free path `dir/stem.ext`, `dir/stem (1).ext`, ... never returning
/// `avoid` (the source file itself).
fn unique_path(dir: &Path, stem: &str, ext: &str, avoid: Option<&Path>) -> PathBuf {
    let mut n = 0u32;
    loop {
        let name = if n == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let candidate = dir.join(name);
        let clashes_source = avoid.is_some_and(|a| a == candidate);
        if !clashes_source && !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `data` to a temp file in the destination directory, then move it
/// into place. The original is only ever replaced by fully written data.
fn write_atomic(dest: &Path, data: &[u8], replace_existing: bool) -> io::Result<()> {
    let dir = dest.parent().unwrap_or_else(|| Path::new(""));
    let tmp = dir.join(format!(
        ".imagesconverter-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp, data)?;
    if replace_existing && dest.exists() {
        if let Err(e) = fs::remove_file(dest) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    }
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}
