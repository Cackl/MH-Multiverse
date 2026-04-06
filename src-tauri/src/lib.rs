mod calligraphy;
mod config;
mod ini;
mod launcher;
mod server;
mod store;
mod tuning;
mod updater;
mod patches;

pub use config::*;

use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};
use server::{ServerProcess, ServerState};

/// Only persist/restore size and position — not decoration or other flags
/// that would conflict with our custom titlebar (decorations: false).
const WINDOW_STATE_FLAGS: StateFlags = StateFlags::from_bits_truncate(
    StateFlags::SIZE.bits() | StateFlags::POSITION.bits()
);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ServerState(Arc::new(Mutex::new(ServerProcess::empty()))))
        .manage(calligraphy::CatalogueState(Mutex::new(None)))
        .manage(store::DisplayNameState::new())
        .setup(|app| {
            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_opener::init())?;
            app.handle().plugin(tauri_plugin_http::init())?;
            app.handle().plugin(tauri_plugin_window_state::Builder::default().build())?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.restore_state(WINDOW_STATE_FLAGS);
            }
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<ServerState>();
                    if let Ok(mut proc) = state.0.lock() {
                        server::kill_child(&mut proc);
                    }
                    let _ = app.save_window_state(WINDOW_STATE_FLAGS);
                    app.exit(0);
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::cmd_save_config,
            config::upsert_server,
            config::delete_server,
            config::set_active_server,
            config::set_game_exe,
            config::set_server_exe,
            config::set_theme,
            config::set_launch_options,
            config::set_shutdown_config,
            config::set_tuning_tags,
            config::set_tuning_favourites,
            config::set_backup_targets,
            config::set_store_html_output_dir,
            launcher::launch_game,
            launcher::game_is_running,
            server::start_server,
            server::stop_server,
            server::start_apache,
            server::stop_apache,
            server::send_command,
            server::server_is_running,
            server::apache_is_running,
            ini::read_config,
            ini::write_config,
            ini::reset_config_section,
            ini::get_config_dir,
            tuning::scan_tuning_files,
            tuning::read_tuning_file,
            tuning::write_tuning_file,
            tuning::create_tuning_file,
            tuning::get_live_tuning_dir,
            tuning::toggle_tuning_file,
            updater::check_update_available,
            updater::run_update,
            updater::create_backup,
            updater::list_backups,
            updater::restore_backup,
            updater::delete_backup,
            updater::get_backups_dir,
            calligraphy::search_prototypes,
            calligraphy::lookup_prototype_id,
            store::get_mtxstore_dir,
            store::list_catalog_files,
            store::load_catalog_entries,
            store::save_catalog_entry,
            store::delete_catalog_entry,
            store::get_next_sku_id,
            store::resolve_display_name,
            store::generate_bundle_html,
            patches::scan_patch_files,
            patches::load_patch_file,
            patches::save_patch_file,
            patches::create_patch_file,
            patches::set_patch_file_enabled,
            patches::get_patches_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}