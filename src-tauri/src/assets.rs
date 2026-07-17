//! First-use download of the background-removal assets: the onnxruntime
//! dynamic library (pinned official Microsoft release) and the BiRefNet-lite
//! model (pinned Hugging Face revision, MIT). Downloads are sha256-verified
//! and installed atomically under the app data directory; after that the
//! feature is fully offline.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

const ORT_BASE: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0";
/// ISNet general-use (DIS, Apache-2.0) — the rembg workhorse. BiRefNet-lite
/// was evaluated first for quality but needs ~80 s per image on CPU;
/// ISNet lands in the couple-of-seconds range with very close results.
const MODEL_URL: &str =
    "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx";
const MODEL_SHA256: &str = "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a";

struct RemoteAsset {
    label: &'static str,
    url: String,
    /// Hash of the downloaded bytes (archive or plain file).
    sha256: &'static str,
    /// Entry to pull out of the downloaded archive (None = plain file).
    archive_entry: Option<String>,
    dest_name: &'static str,
    /// Hash of the installed artifact, re-checked at load time so a torn
    /// or tampered file in the user-writable dir is never dlopen'd/parsed.
    payload_sha256: &'static str,
    download_bytes: u64,
}

#[cfg(target_os = "windows")]
fn runtime_asset() -> RemoteAsset {
    RemoteAsset {
        label: "onnxruntime",
        url: format!("{ORT_BASE}/onnxruntime-win-x64-1.22.0.zip"),
        sha256: "174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da",
        archive_entry: Some("onnxruntime-win-x64-1.22.0/lib/onnxruntime.dll".into()),
        dest_name: "onnxruntime.dll",
        payload_sha256: "579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559",
        download_bytes: 72_368_545,
    }
}

#[cfg(target_os = "linux")]
fn runtime_asset() -> RemoteAsset {
    RemoteAsset {
        label: "onnxruntime",
        url: format!("{ORT_BASE}/onnxruntime-linux-x64-1.22.0.tgz"),
        sha256: "8344d55f93d5bc5021ce342db50f62079daf39aaafb5d311a451846228be49b3",
        archive_entry: Some("onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0".into()),
        dest_name: "libonnxruntime.so",
        payload_sha256: "3da6146e14e7b8aaec625dde11d6114c7457c87a5f93d744897da8781e35c673",
        download_bytes: 7_798_730,
    }
}

#[cfg(target_os = "macos")]
fn runtime_asset() -> RemoteAsset {
    RemoteAsset {
        label: "onnxruntime",
        url: format!("{ORT_BASE}/onnxruntime-osx-universal2-1.22.0.tgz"),
        sha256: "cfa6f6584d87555ed9f6e7e8a000d3947554d589efe3723b8bfa358cd263d03c",
        archive_entry: Some(
            "onnxruntime-osx-universal2-1.22.0/lib/libonnxruntime.1.22.0.dylib".into(),
        ),
        dest_name: "libonnxruntime.dylib",
        payload_sha256: "db045368293215c9d22aa7b8c983d688b3ae9ca1da3f64ffbe01ba7df31c3355",
        download_bytes: 54_820_264,
    }
}

fn model_asset() -> RemoteAsset {
    RemoteAsset {
        label: "model",
        url: MODEL_URL.into(),
        sha256: MODEL_SHA256,
        archive_entry: None,
        dest_name: "isnet-general-use.onnx",
        payload_sha256: MODEL_SHA256,
        download_bytes: 178_648_008,
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BgAssetsStatus {
    pub ready: bool,
    pub missing_bytes: u64,
}

/// Progress reported to the UI while installing.
#[derive(Serialize, Clone)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InstallEvent {
    Progress { received: u64, total: u64 },
    Done,
}

#[derive(Clone)]
pub struct BgAssets {
    dir: PathBuf,
}

impl BgAssets {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            dir: app_data_dir.join("bg-removal"),
        }
    }

    pub fn dylib_path(&self) -> PathBuf {
        self.dir.join(runtime_asset().dest_name)
    }

    pub fn model_path(&self) -> PathBuf {
        self.dir.join(model_asset().dest_name)
    }

    fn missing(&self) -> Vec<RemoteAsset> {
        [runtime_asset(), model_asset()]
            .into_iter()
            .filter(|a| !self.dir.join(a.dest_name).is_file())
            .collect()
    }

    pub fn status(&self) -> BgAssetsStatus {
        let missing = self.missing();
        BgAssetsStatus {
            ready: missing.is_empty(),
            missing_bytes: missing.iter().map(|a| a.download_bytes).sum(),
        }
    }

    /// Re-hash the installed artifacts against the pins embedded in the
    /// binary. A corrupted or tampered file is deleted so the UI offers the
    /// download again — the repair path for torn installs (power loss) and
    /// the guard against dlopen'ing a swapped library.
    pub fn verify_installed(&self) -> Result<(), String> {
        for asset in [runtime_asset(), model_asset()] {
            let path = self.dir.join(asset.dest_name);
            if !path.is_file() {
                return Err(format!("{} is missing", asset.label));
            }
            let actual = sha256_file(&path).map_err(|e| format!("{}: {e}", asset.label))?;
            if !actual.eq_ignore_ascii_case(asset.payload_sha256) {
                let _ = fs::remove_file(&path);
                return Err(format!(
                    "{} failed integrity check and was removed — download it again",
                    asset.label
                ));
            }
        }
        Ok(())
    }

    /// Download and install whatever is missing. `progress(received, total)`
    /// is called with byte counts across all pending downloads.
    pub fn install(&self, progress: impl Fn(u64, u64)) -> Result<(), String> {
        let missing = self.missing();
        if missing.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.dir).map_err(|e| format!("create dir: {e}"))?;
        let total: u64 = missing.iter().map(|a| a.download_bytes).sum();
        let mut done: u64 = 0;
        for asset in &missing {
            self.install_one(asset, |received| progress(done + received, total))?;
            done += asset.download_bytes;
            progress(done, total);
        }
        Ok(())
    }

    fn install_one(&self, asset: &RemoteAsset, progress: impl Fn(u64)) -> Result<(), String> {
        let staged = self.dir.join(format!("{}.download", asset.dest_name));
        let dest = self.dir.join(asset.dest_name);

        download_verified(
            &asset.url,
            asset.sha256,
            asset.download_bytes,
            &staged,
            &progress,
        )
        .map_err(|e| format!("{}: {e}", asset.label))?;

        let result = match &asset.archive_entry {
            None => fs::rename(&staged, &dest).map_err(|e| format!("install: {e}")),
            Some(entry) => {
                let extracted = self.dir.join(format!("{}.extracted", asset.dest_name));
                extract_entry(&staged, entry, &extracted)
                    .and_then(|()| {
                        fs::rename(&extracted, &dest).map_err(|e| format!("install: {e}"))
                    })
                    .inspect_err(|_| {
                        let _ = fs::remove_file(&extracted);
                    })
            }
        };
        let _ = fs::remove_file(&staged);
        result
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("read: {e}"))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Stream the URL to `staged`, hashing on the fly; fail on sha mismatch or
/// on a body larger than the pinned size.
fn download_verified(
    url: &str,
    expected_sha256: &str,
    expected_bytes: u64,
    staged: &Path,
    progress: &impl Fn(u64),
) -> Result<(), String> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(3600))
        .call()
        .map_err(|e| format!("download: {e}"))?;

    let mut reader = response.into_reader();
    let mut file = fs::File::create(staged).map_err(|e| format!("write: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    let mut received: u64 = 0;

    loop {
        let n = reader.read(&mut buffer).map_err(|e| {
            let _ = fs::remove_file(staged);
            format!("download: {e}")
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        std::io::Write::write_all(&mut file, &buffer[..n]).map_err(|e| {
            let _ = fs::remove_file(staged);
            format!("write: {e}")
        })?;
        received += n as u64;
        // The exact size is pinned along with the hash: a body that keeps
        // streaming past it can only be wrong, stop before it fills the disk.
        if received > expected_bytes {
            let _ = fs::remove_file(staged);
            return Err(format!(
                "response larger than the expected {expected_bytes} bytes"
            ));
        }
        progress(received);
    }
    // Flush to disk before the rename: a crash right after install must not
    // leave a full-length torn file behind.
    file.sync_all().map_err(|e| format!("write: {e}"))?;
    drop(file);

    let digest = hasher.finalize();
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(staged);
        return Err(format!(
            "checksum mismatch (expected {expected_sha256}, got {actual}) — download corrupted or upstream changed"
        ));
    }
    Ok(())
}

/// Pull a single entry out of a .zip or .tgz archive (detected by magic).
fn extract_entry(archive: &Path, entry: &str, dest: &Path) -> Result<(), String> {
    let mut magic = [0u8; 2];
    {
        let mut f = fs::File::open(archive).map_err(|e| format!("open archive: {e}"))?;
        f.read_exact(&mut magic)
            .map_err(|e| format!("read archive: {e}"))?;
    }

    if &magic == b"PK" {
        let file = fs::File::open(archive).map_err(|e| format!("open archive: {e}"))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
        let mut wanted = zip
            .by_name(entry)
            .map_err(|e| format!("zip entry {entry}: {e}"))?;
        let mut out = fs::File::create(dest).map_err(|e| format!("extract: {e}"))?;
        std::io::copy(&mut wanted, &mut out).map_err(|e| format!("extract: {e}"))?;
        out.sync_all().map_err(|e| format!("extract: {e}"))?;
        return Ok(());
    }

    let file = fs::File::open(archive).map_err(|e| format!("open archive: {e}"))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for maybe_entry in tar.entries().map_err(|e| format!("tar: {e}"))? {
        let mut tar_entry = maybe_entry.map_err(|e| format!("tar: {e}"))?;
        let path = tar_entry
            .path()
            .map_err(|e| format!("tar: {e}"))?
            .to_string_lossy()
            .into_owned();
        if path == entry {
            let mut out = fs::File::create(dest).map_err(|e| format!("extract: {e}"))?;
            std::io::copy(&mut tar_entry, &mut out).map_err(|e| format!("extract: {e}"))?;
            out.sync_all().map_err(|e| format!("extract: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("entry {entry} not found in archive"))
}
