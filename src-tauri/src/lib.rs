mod assets;
mod commands;
pub mod engine;

use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            app.manage(commands::BgState::new(app_data));
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::ConversionState::default())
        .invoke_handler(tauri::generate_handler![
            commands::inspect_files,
            commands::file_thumbnail,
            commands::convert_files,
            commands::cancel_conversion,
            commands::bg_status,
            commands::bg_install
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
