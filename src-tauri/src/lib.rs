pub mod commands;
pub mod db;
pub mod providers;

use std::sync::Mutex;

use db::{connection_path, init_connection};
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub conn: Mutex<Connection>,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let path = connection_path(&app.handle())?;
            let conn = init_connection(&path)?;
            app.manage(AppState {
                conn: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_default_soul,
            commands::create_default_setting,
            commands::load_soul_file,
            commands::load_setting_file,
            commands::save_soul_file,
            commands::save_setting_file,
            commands::list_souls,
            commands::list_settings,
            commands::upsert_soul,
            commands::upsert_setting,
            commands::get_soul,
            commands::get_setting,
            commands::delete_soul,
            commands::delete_setting,
            commands::list_conversation_messages,
            commands::delete_conversation,
            commands::send_mock_turn,
            commands::send_api_turn,
            commands::compile_context,
            commands::run_consolidation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mnemosyne");
}
