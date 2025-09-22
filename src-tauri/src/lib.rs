mod commands;
mod db;
mod gauge;
mod licensing;
mod models;
mod ops;
mod scanner;
mod selector;

use db::{init_pool, Database, DbPool};
use licensing::LicenseStorage;
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn app_db_path() -> std::path::PathBuf {
    let app_data_dir = dirs::data_dir().expect("Failed to get app data directory");
    let app_dir = app_data_dir.join("white-space");
    std::fs::create_dir_all(&app_dir).expect("Failed to create app directory");
    app_dir.join("database.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Initialize database pool
            let db_path = app_db_path();
            let pool = init_pool(&db_path);
            app.manage::<DbPool>(pool);

            // Initialize licensing storage (Send+Sync)
            app.manage(LicenseStorage {
                cache: tokio::sync::RwLock::new(Default::default()),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::add_folder,
            commands::list_folders,
            commands::pick_directory,
            commands::list_dir,
            commands::open_in_system,
            commands::get_platform_info,
            commands::scan_roots,
            commands::start_scan,
            commands::scan_status,
            commands::get_candidates,
            commands::daily_candidates,
            commands::gauge_state,
            commands::archive_files,
            commands::delete_files,
            commands::undo_last,
            commands::list_undoable_batches,
            commands::undo_batch,
            commands::get_review_items,
            commands::get_thumbnail,
            commands::get_prefs,
            commands::set_prefs,
            licensing::ls_activate,
            licensing::ls_validate,
            licensing::ls_deactivate,
            licensing::ls_get_status,
            licensing::ls_check_validation_needed,
            licensing::ls_auto_validate,
            licensing::ls_clear_license
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
