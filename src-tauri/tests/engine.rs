//! End-to-end engine tests over real files on disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use image::{DynamicImage, GenericImageView, RgbImage, Rgba, RgbaImage};
use imagesconverter_lib::engine::{process_file, Action, EngineError, Options, TargetFormat};

// ---------- helpers ----------

static DIR_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Fresh scratch directory per test.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "imagesconverter-test-{}-{}",
        std::process::id(),
        DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn opts(format: TargetFormat, max_size_kb: Option<u64>, delete_original: bool) -> Options {
    Options {
        format,
        max_size_kb,
        delete_original,
    }
}

/// Colorful gradient so JPEG/WebP have real content to chew on.
fn photo(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            ((x + y) % 256) as u8,
        ])
    })
}

fn write_png(path: &Path, image: &DynamicImage) {
    image
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
}

fn write_jpeg(path: &Path, image: &DynamicImage) {
    image
        .save_with_format(path, image::ImageFormat::Jpeg)
        .unwrap();
}

/// A JPEG with a handcrafted EXIF APP1 segment (orientation + artist).
fn jpeg_with_exif(path: &Path, image: &DynamicImage, orientation: u16) {
    let mut plain = Vec::new();
    image
        .to_rgb8()
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut plain, 90,
        ))
        .unwrap();

    // TIFF (little endian): header + IFD0 with Orientation and Artist.
    let artist = b"test author\0";
    let mut tiff: Vec<u8> = Vec::new();
    tiff.extend_from_slice(b"II\x2A\x00");
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
    tiff.extend_from_slice(&2u16.to_le_bytes()); // entry count
                                                 // Orientation: tag 0x0112, SHORT, count 1
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&(orientation as u32).to_le_bytes());
    // Artist: tag 0x013B, ASCII, stored past the IFD (offset 38)
    tiff.extend_from_slice(&0x013Bu16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&(artist.len() as u32).to_le_bytes());
    tiff.extend_from_slice(&38u32.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes()); // next IFD
    tiff.extend_from_slice(artist);

    let mut app1: Vec<u8> = Vec::new();
    app1.extend_from_slice(b"Exif\x00\x00");
    app1.extend_from_slice(&tiff);

    let mut out = Vec::new();
    out.extend_from_slice(&plain[0..2]); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&plain[2..]);
    fs::write(path, out).unwrap();
}

fn read_orientation(path: &Path) -> Option<u32> {
    let bytes = fs::read(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

fn has_any_exif(path: &Path) -> bool {
    let bytes = fs::read(path).unwrap();
    exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .map(|e| e.fields().count() > 0)
        .unwrap_or(false)
}

fn size_of(path: &Path) -> u64 {
    fs::metadata(path).unwrap().len()
}

// ---------- conversion ----------

#[test]
fn png_to_webp_keeps_original() {
    let dir = scratch();
    let src = dir.join("photo.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(320, 200)));

    let outcome = process_file(&src, &opts(TargetFormat::Webp, None, false)).unwrap();

    assert_eq!(outcome.action, Action::Converted);
    assert_eq!(outcome.out_path, dir.join("photo-converted.webp"));
    assert!(src.exists(), "original must be kept");
    let converted = image::open(&outcome.out_path).unwrap();
    assert_eq!(converted.dimensions(), (320, 200));
}

#[test]
fn png_to_jpeg_delete_original() {
    let dir = scratch();
    let src = dir.join("photo.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(64, 48)));

    let outcome = process_file(&src, &opts(TargetFormat::Jpeg, None, true)).unwrap();

    assert_eq!(outcome.out_path, dir.join("photo.jpeg"));
    assert!(!src.exists(), "original must be deleted");
    assert_eq!(
        image::guess_format(&fs::read(&outcome.out_path).unwrap()).unwrap(),
        image::ImageFormat::Jpeg
    );
}

#[test]
fn transparent_image_to_jpeg_is_refused() {
    let dir = scratch();
    let src = dir.join("alpha.png");
    let mut img = RgbaImage::from_pixel(32, 32, Rgba([10, 200, 50, 255]));
    img.put_pixel(3, 3, Rgba([0, 0, 0, 0]));
    write_png(&src, &DynamicImage::ImageRgba8(img));

    let err = process_file(&src, &opts(TargetFormat::Jpeg, None, false)).unwrap_err();
    assert!(matches!(err, EngineError::JpegTransparency));
    assert!(src.exists());
}

#[test]
fn opaque_rgba_to_jpeg_is_allowed() {
    let dir = scratch();
    let src = dir.join("opaque.png");
    let img = RgbaImage::from_pixel(32, 32, Rgba([10, 200, 50, 255]));
    write_png(&src, &DynamicImage::ImageRgba8(img));

    let outcome = process_file(&src, &opts(TargetFormat::Jpeg, None, false)).unwrap();
    assert_eq!(outcome.action, Action::Converted);
}

#[test]
fn exif_orientation_is_applied_on_convert() {
    let dir = scratch();
    let src = dir.join("rotated.jpg");
    // 100x40 landscape, orientation 6 (must display as 40x100 portrait).
    jpeg_with_exif(&src, &DynamicImage::ImageRgb8(photo(100, 40)), 6);
    assert_eq!(read_orientation(&src), Some(6), "fixture sanity check");

    let outcome = process_file(&src, &opts(TargetFormat::Png, None, false)).unwrap();

    let converted = image::open(&outcome.out_path).unwrap();
    assert_eq!(
        converted.dimensions(),
        (40, 100),
        "rotation must be applied"
    );
    assert!(!has_any_exif(&outcome.out_path));
}

#[test]
fn unsupported_extension_is_rejected() {
    let dir = scratch();
    let src = dir.join("document.txt");
    fs::write(&src, b"hello").unwrap();
    let err = process_file(&src, &opts(TargetFormat::Webp, None, false)).unwrap_err();
    assert!(matches!(err, EngineError::Unsupported(_)));
}

// ---------- metadata cleaning ----------

#[test]
fn clean_jpeg_lossless_preserves_pixels() {
    let dir = scratch();
    let src = dir.join("meta.jpg");
    jpeg_with_exif(&src, &DynamicImage::ImageRgb8(photo(120, 80)), 1);
    assert!(has_any_exif(&src));
    let before = image::open(&src).unwrap().to_rgb8();

    let outcome = process_file(&src, &opts(TargetFormat::Jpeg, None, false)).unwrap();

    assert_eq!(outcome.action, Action::Cleaned);
    assert!(
        outcome.lossless,
        "orientation=1 must take the lossless path"
    );
    assert_eq!(outcome.out_path, dir.join("meta-clean.jpg"));
    assert!(!has_any_exif(&outcome.out_path));
    let after = image::open(&outcome.out_path).unwrap().to_rgb8();
    assert_eq!(before.as_raw(), after.as_raw(), "pixels must be untouched");
}

#[test]
fn clean_rotated_jpeg_reencodes_upright() {
    let dir = scratch();
    let src = dir.join("meta6.jpg");
    jpeg_with_exif(&src, &DynamicImage::ImageRgb8(photo(100, 40)), 6);

    let outcome = process_file(&src, &opts(TargetFormat::Jpeg, None, true)).unwrap();

    assert_eq!(outcome.action, Action::Cleaned);
    assert!(!outcome.lossless);
    assert_eq!(outcome.out_path, src, "delete mode cleans in place");
    assert!(!has_any_exif(&src));
    assert_eq!(image::open(&src).unwrap().dimensions(), (40, 100));
}

#[test]
fn clean_png_drops_text_chunks_losslessly() {
    let dir = scratch();
    let src = dir.join("texty.png");
    let img = DynamicImage::ImageRgb8(photo(50, 50));
    // Encode, then inject a tEXt chunk right after IHDR (33 bytes in).
    let mut bytes = Vec::new();
    img.write_with_encoder(image::codecs::png::PngEncoder::new(&mut bytes))
        .unwrap();
    let mut text_chunk: Vec<u8> = Vec::new();
    let payload = b"Comment\0leaked secret";
    text_chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    text_chunk.extend_from_slice(b"tEXt");
    text_chunk.extend_from_slice(payload);
    let crc = {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(b"tEXt");
        hasher.update(payload);
        hasher.finalize()
    };
    text_chunk.extend_from_slice(&crc.to_be_bytes());
    let mut with_meta = bytes[..33].to_vec();
    with_meta.extend_from_slice(&text_chunk);
    with_meta.extend_from_slice(&bytes[33..]);
    fs::write(&src, &with_meta).unwrap();

    let before = image::open(&src).unwrap().to_rgb8();
    let outcome = process_file(&src, &opts(TargetFormat::Png, None, true)).unwrap();

    assert!(outcome.lossless);
    let cleaned = fs::read(&src).unwrap();
    assert!(
        !cleaned.windows(4).any(|w| w == b"tEXt"),
        "tEXt chunk must be gone"
    );
    assert_eq!(
        before.as_raw(),
        image::open(&src).unwrap().to_rgb8().as_raw()
    );
}

// ---------- compression ----------

#[test]
fn compress_jpeg_to_target_size() {
    let dir = scratch();
    let src = dir.join("big.jpg");
    write_jpeg(&src, &DynamicImage::ImageRgb8(photo(1600, 1200)));
    let original_size = size_of(&src);
    let target_kb = 40;
    assert!(original_size > target_kb * 1024, "fixture sanity check");

    let outcome = process_file(&src, &opts(TargetFormat::Jpeg, Some(target_kb), false)).unwrap();

    assert_eq!(outcome.action, Action::Compressed);
    assert_eq!(outcome.out_path, dir.join("big-compressed.jpg"));
    assert!(size_of(&outcome.out_path) <= target_kb * 1024);
    image::open(&outcome.out_path).unwrap();
}

#[test]
fn compress_png_to_target_size() {
    let dir = scratch();
    let src = dir.join("big.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(800, 600)));
    let target_kb = size_of(&src) / 1024 / 3; // ask for a third of the size

    let outcome = process_file(&src, &opts(TargetFormat::Png, Some(target_kb), true)).unwrap();

    assert_eq!(outcome.out_path, src);
    assert!(size_of(&src) <= target_kb * 1024);
    image::open(&src).unwrap();
}

#[test]
fn convert_with_budget_compresses_too() {
    let dir = scratch();
    let src = dir.join("photo.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(1200, 900)));

    let outcome = process_file(&src, &opts(TargetFormat::Webp, Some(30), false)).unwrap();

    assert_eq!(outcome.action, Action::Converted);
    assert_eq!(outcome.out_path, dir.join("photo-converted.webp"));
    assert!(size_of(&outcome.out_path) <= 30 * 1024);
}

#[test]
fn unreachable_target_reports_warning() {
    let dir = scratch();
    let src = dir.join("noise.png");
    // True noise (LCG) compresses terribly; 1 KB is unreachable even at 10%.
    let mut state: u32 = 0x1234_5678;
    let mut noise = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 24) as u8
    };
    let mut raw = Vec::with_capacity(600 * 600 * 4);
    for _ in 0..600 * 600 {
        raw.extend_from_slice(&[noise(), noise(), noise(), 255]);
    }
    let img = RgbaImage::from_raw(600, 600, raw).unwrap();
    write_png(&src, &DynamicImage::ImageRgba8(img));

    let outcome = process_file(&src, &opts(TargetFormat::Png, Some(1), false)).unwrap();
    assert!(outcome.warning.is_some());
    assert!(outcome.out_path.exists());
}

// ---------- naming & safety ----------

#[test]
fn collisions_get_numbered_suffix() {
    let dir = scratch();
    let src = dir.join("photo.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(64, 64)));
    fs::write(dir.join("photo.webp"), b"pre-existing, do not overwrite").unwrap();

    let outcome = process_file(&src, &opts(TargetFormat::Webp, None, true)).unwrap();

    assert_eq!(outcome.out_path, dir.join("photo (1).webp"));
    assert_eq!(
        fs::read(dir.join("photo.webp")).unwrap(),
        b"pre-existing, do not overwrite",
        "existing files must never be overwritten"
    );
    assert!(!src.exists());
}

#[test]
fn no_temp_files_left_behind() {
    let dir = scratch();
    let src = dir.join("photo.png");
    write_png(&src, &DynamicImage::ImageRgb8(photo(64, 64)));
    process_file(&src, &opts(TargetFormat::Webp, None, false)).unwrap();

    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn webp_roundtrip_same_format_clean() {
    let dir = scratch();
    let src = dir.join("photo.webp");
    let outcome0 = {
        // Produce a webp via the engine itself.
        let tmp = dir.join("seed.png");
        write_png(&tmp, &DynamicImage::ImageRgb8(photo(128, 96)));
        process_file(&tmp, &opts(TargetFormat::Webp, None, false)).unwrap()
    };
    fs::rename(&outcome0.out_path, &src).unwrap();

    let outcome = process_file(&src, &opts(TargetFormat::Webp, None, false)).unwrap();
    assert_eq!(outcome.action, Action::Cleaned);
    assert!(outcome.lossless, "plain webp must take the lossless path");
    image::open(&outcome.out_path).unwrap();
}
