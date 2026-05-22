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
            commands::create_fresh_scenario_soul,
            commands::create_session_soul_from_savepoint,
            commands::save_session_as_new_soul,
            commands::create_default_setting,
            commands::load_soul_file,
            commands::load_setting_file,
            commands::save_soul_file,
            commands::save_setting_file,
            commands::export_character_soul_mne,
            commands::export_world_setting_mne,
            commands::export_scenario_bundle_mne,
            commands::export_current_session_checkpoint_mne,
            commands::import_mne_bundle,
            commands::list_souls,
            commands::list_souls_debug,
            commands::list_conversations,
            commands::rename_conversation,
            commands::import_image_asset,
            commands::import_image_asset_bytes,
            commands::create_user_image_message,
            commands::create_user_image_message_bytes,
            commands::get_image_asset,
            commands::get_image_asset_data_url,
            commands::list_settings,
            commands::upsert_soul,
            commands::upsert_setting,
            commands::get_soul,
            commands::clear_soul_world_state,
            commands::clear_soul_profile_scenario,
            commands::clear_soul_recent_events,
            commands::clear_soul_memories,
            commands::get_setting,
            commands::delete_soul,
            commands::delete_setting,
            commands::list_conversation_messages,
            commands::delete_conversation,
            commands::delete_message,
            commands::restore_inactive_messages,
            commands::dedupe_active_adjacent_user_messages,
            commands::update_user_message,
            commands::list_assistant_message_variants,
            commands::select_assistant_message_variant,
            commands::delete_assistant_message_variant,
            commands::list_llm_payload_logs,
            commands::get_llm_payload_log,
            commands::get_branch_patch_debug,
            commands::rebuild_session_from_ledger,
            commands::export_visible_chat_log,
            commands::export_llm_payload_history,
            commands::list_provider_profiles,
            commands::get_provider_profile,
            commands::upsert_provider_profile,
            commands::delete_provider_profile,
            commands::send_mock_turn,
            commands::send_api_turn,
            commands::compile_context,
            commands::preview_api_payload,
            commands::run_consolidation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mnemosyne");
}
