use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::engine::{self, Options};

#[derive(Default)]
pub struct ConversionState {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InspectedFile {
    path: String,
    name: String,
    size: u64,
    extension: String,
    supported: bool,
}

#[tauri::command]
pub fn inspect_files(paths: Vec<String>) -> Vec<InspectedFile> {
    paths
        .into_iter()
        .map(|path| {
            let p = Path::new(&path);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let extension = p
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let metadata = std::fs::metadata(p).ok();
            let is_file = metadata.as_ref().is_some_and(|m| m.is_file());
            let size = metadata.map(|m| m.len()).unwrap_or(0);
            let supported = is_file && engine::is_supported_extension(&extension);
            InspectedFile {
                path,
                name,
                size,
                extension,
                supported,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn file_thumbnail(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::thumbnail_data_uri(Path::new(&path), 144).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProgressEvent {
    Start {
        path: String,
    },
    File {
        path: String,
        ok: bool,
        action: Option<engine::Action>,
        message: Option<String>,
        out_path: Option<String>,
        in_bytes: Option<u64>,
        out_bytes: Option<u64>,
        resized_to: Option<(u32, u32)>,
        lossless: bool,
        warning: Option<String>,
    },
    Done {
        total: usize,
        succeeded: usize,
        failed: usize,
        cancelled: bool,
    },
}

#[tauri::command]
pub async fn convert_files(
    paths: Vec<String>,
    options: Options,
    on_event: Channel<ProgressEvent>,
    state: State<'_, ConversionState>,
) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("a conversion is already running".into());
    }
    state.cancel.store(false, Ordering::SeqCst);

    let cancel = state.cancel.clone();
    let running = state.running.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let total = paths.len();
        let succeeded = AtomicUsize::new(0);
        let failed = AtomicUsize::new(0);

        paths.par_iter().for_each(|path| {
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            let _ = on_event.send(ProgressEvent::Start { path: path.clone() });
            let event = match engine::process_file(Path::new(path), &options) {
                Ok(outcome) => {
                    succeeded.fetch_add(1, Ordering::SeqCst);
                    ProgressEvent::File {
                        path: path.clone(),
                        ok: true,
                        action: Some(outcome.action),
                        message: None,
                        out_path: Some(outcome.out_path.to_string_lossy().into_owned()),
                        in_bytes: Some(outcome.in_bytes),
                        out_bytes: Some(outcome.out_bytes),
                        resized_to: outcome.resized_to,
                        lossless: outcome.lossless,
                        warning: outcome.warning,
                    }
                }
                Err(error) => {
                    failed.fetch_add(1, Ordering::SeqCst);
                    ProgressEvent::File {
                        path: path.clone(),
                        ok: false,
                        action: None,
                        message: Some(error.to_string()),
                        out_path: None,
                        in_bytes: None,
                        out_bytes: None,
                        resized_to: None,
                        lossless: false,
                        warning: None,
                    }
                }
            };
            let _ = on_event.send(event);
        });

        let done = succeeded.load(Ordering::SeqCst) + failed.load(Ordering::SeqCst);
        let _ = on_event.send(ProgressEvent::Done {
            total,
            succeeded: succeeded.load(Ordering::SeqCst),
            failed: failed.load(Ordering::SeqCst),
            cancelled: cancel.load(Ordering::SeqCst) && done < total,
        });
    })
    .await;

    running.store(false, Ordering::SeqCst);
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_conversion(state: State<'_, ConversionState>) {
    state.cancel.store(true, Ordering::SeqCst);
}
