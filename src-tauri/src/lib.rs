pub mod chat_commands;
pub mod commands;
pub mod db;
pub mod pipeline_trace;
pub mod providers;

use std::sync::Mutex;

use db::{connection_path, init_connection};
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub conn: Mutex<Connection>,
    /// The embedded local repair model process, if one was started this session.
    pub local_model: Mutex<Option<commands::EmbeddedModel>>,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let path = connection_path(&app.handle())?;
            let conn = init_connection(&path)?;
            if let Err(err) = db::recover_incomplete_sessions_on_startup(&conn) {
                eprintln!("Mnemosyne startup recovery warning: {err}");
            }
            app.manage(AppState {
                conn: Mutex::new(conn),
                local_model: Mutex::new(None),
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
            commands::validate_mne_bundle,
            commands::preview_mne_import,
            commands::import_mne_as_new,
            commands::list_souls,
            commands::list_souls_debug,
            commands::list_conversations,
            commands::list_player_personas,
            commands::get_active_player_persona,
            commands::set_active_player_persona,
            commands::upsert_player_persona,
            commands::archive_player_persona,
            commands::restore_player_persona,
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
            commands::archive_soul,
            commands::restore_soul,
            commands::list_archived_souls,
            commands::archive_savepoint,
            commands::restore_savepoint,
            commands::list_archived_savepoints,
            commands::delete_setting,
            commands::archive_setting,
            commands::restore_setting,
            commands::list_archived_settings,
            commands::list_conversation_messages,
            commands::delete_conversation,
            commands::delete_message,
            commands::restore_inactive_messages,
            commands::open_session_data_location,
            commands::create_backup,
            commands::archive_session,
            commands::restore_session,
            commands::list_archived_sessions,
            commands::hide_turn_range,
            commands::hide_latest_benchmark_failed_user_message,
            commands::restore_turn_range,
            commands::list_hidden_turns,
            commands::dedupe_active_adjacent_user_messages,
            commands::update_user_message,
            commands::list_assistant_message_variants,
            commands::select_assistant_message_variant,
            commands::delete_assistant_message_variant,
            commands::inspect_turn_branch_integrity,
            commands::repair_accidental_normal_send_variants,
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
            commands::archive_provider_profile,
            commands::restore_provider_profile,
            commands::list_archived_provider_profiles,
            commands::get_latest_evaluator_job,
            commands::cancel_evaluator_job,
            commands::retry_evaluator_job,
            commands::repair_evaluator_ops,
            commands::run_evaluator_contract_test,
            commands::run_structured_evaluator_diagnostic,
            commands::run_benchmark,
            commands::prepare_benchmark_session,
            commands::generate_benchmark_player_message,
            commands::generate_traditional_rp_message,
            commands::benchmark_turn_summary,
            commands::finalize_benchmark,
            commands::set_active_evaluator_profile,
            commands::curate_memory,
            commands::send_mock_turn,
            commands::send_api_turn,
            commands::compile_context,
            commands::preview_api_payload,
            commands::run_consolidation,
            commands::start_embedded_repair_model,
            commands::stop_embedded_repair_model,
            commands::embedded_repair_model_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Mnemosyne")
        .run(|app_handle, event| {
            // Kill the embedded local repair model when the app exits so it
            // doesn't linger as an orphaned process.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(mut guard) = state.local_model.lock() {
                        if let Some(mut model) = guard.take() {
                            let _ = model.child.kill();
                        }
                    }
                }
            }
        });
}
