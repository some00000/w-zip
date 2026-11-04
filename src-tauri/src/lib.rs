mod archive;
mod commands;
mod error;
mod format;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::pick_files,
            commands::pick_save_path,
            commands::pick_wzip,
            commands::pick_folder,
            commands::pack_wzip,
            commands::unpack_wzip,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("w+zip");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running w+zip");
}
