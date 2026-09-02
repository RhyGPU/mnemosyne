pub mod benchmark;
pub mod chat_commands;
pub mod commands;
pub mod db;
pub mod embedded_model;
pub mod job_progress;

#[cfg(test)]
mod evaluator_bakeoff;

#[cfg(test)]
mod live_harness;
pub mod mne;
pub mod pipeline_trace;
pub mod providers;

use std::sync::Mutex;

use db::{connection_path, init_connection};
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub conn: Mutex<Connection>,
    /// The embedded local repair model process, if one was started this session.
    pub local_model: Mutex<Option<embedded_model::EmbeddedModel>>,
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
            commands::session::create_default_soul,
            commands::session::create_fresh_scenario_soul,
            commands::session::create_session_soul_from_savepoint,
            commands::session::save_session_as_new_soul,
            commands::session::create_default_setting,
            commands::session::load_soul_file,
            commands::session::load_setting_file,
            commands::session::save_soul_file,
            commands::session::save_setting_file,
            mne::service::export_character_soul_mne,
            mne::service::export_world_setting_mne,
            mne::service::export_scenario_bundle_mne,
            mne::service::export_current_session_checkpoint_mne,
            mne::service::import_mne_bundle,
            mne::service::validate_mne_bundle,
            mne::service::preview_mne_import,
            mne::service::import_mne_as_new,
            commands::session::list_souls,
            commands::session::list_souls_debug,
            commands::session::list_conversations,
            commands::session::touch_conversation_access,
            commands::session::list_session_state_hub,
            commands::session::list_session_state_map,
            commands::session::list_player_personas,
            commands::session::list_archived_player_personas,
            commands::session::get_active_player_persona,
            commands::session::set_active_player_persona,
            commands::session::upsert_player_persona,
            commands::session::archive_player_persona,
            commands::session::restore_player_persona,
            commands::session::rename_conversation,
            commands::session::import_image_asset,
            commands::session::import_image_asset_bytes,
            commands::session::create_user_image_message,
            commands::session::create_user_image_message_bytes,
            commands::session::get_image_asset,
            commands::session::get_image_asset_data_url,
            commands::session::list_settings,
            commands::session::upsert_soul,
            commands::session::upsert_setting,
            commands::session::get_soul,
            commands::session::clear_soul_world_state,
            commands::session::clear_soul_profile_scenario,
            commands::session::clear_soul_recent_events,
            commands::session::clear_soul_memories,
            commands::session::get_setting,
            commands::session::delete_soul,
            commands::session::archive_soul,
            commands::session::purge_soul,
            commands::session::restore_soul,
            commands::session::list_archived_souls,
            commands::session::archive_savepoint,
            commands::session::restore_savepoint,
            commands::session::list_archived_savepoints,
            commands::session::delete_setting,
            commands::session::archive_setting,
            commands::session::purge_setting,
            commands::session::restore_setting,
            commands::session::list_archived_settings,
            commands::session::list_conversation_messages,
            commands::session::delete_conversation,
            commands::session::delete_message,
            commands::session::restore_inactive_messages,
            commands::session::open_session_data_location,
            commands::session::create_backup,
            commands::session::archive_session,
            commands::session::restore_session,
            commands::session::list_archived_sessions,
            commands::session::hide_turn_range,
            commands::session::hide_latest_benchmark_failed_user_message,
            commands::session::restore_turn_range,
            commands::session::list_hidden_turns,
            commands::session::dedupe_active_adjacent_user_messages,
            commands::session::update_user_message,
            commands::session::list_assistant_message_variants,
            commands::session::select_assistant_message_variant,
            commands::session::delete_assistant_message_variant,
            commands::session::inspect_turn_branch_integrity,
            commands::session::repair_accidental_normal_send_variants,
            commands::session::list_llm_payload_logs,
            commands::session::get_llm_payload_log,
            commands::session::get_branch_patch_debug,
            commands::session::rebuild_session_from_ledger,
            commands::session::seed_observable_knowledge,
            commands::session::export_visible_chat_log,
            commands::session::export_llm_payload_history,
            commands::evaluator::list_provider_profiles,
            commands::evaluator::get_provider_profile,
            commands::evaluator::upsert_provider_profile,
            commands::evaluator::delete_provider_profile,
            commands::evaluator::archive_provider_profile,
            commands::evaluator::restore_provider_profile,
            commands::evaluator::list_archived_provider_profiles,
            commands::evaluator::get_latest_evaluator_job,
            commands::evaluator::cancel_evaluator_job,
            commands::evaluator::retry_evaluator_job,
            commands::evaluator::repair_evaluator_ops,
            commands::evaluator::run_evaluator_contract_test,
            commands::evaluator::run_session_form_eval_benchmark,
            commands::evaluator::run_structured_evaluator_diagnostic,
            benchmark::run_benchmark,
            benchmark::prepare_benchmark_session,
            benchmark::generate_benchmark_player_message,
            benchmark::generate_traditional_rp_message,
            benchmark::benchmark_turn_summary,
            benchmark::finalize_benchmark,
            commands::evaluator::set_active_evaluator_profile,
            commands::evaluator::curate_memory,
            commands::send_mock_turn,
            commands::send_api_turn,
            commands::compile_context,
            commands::preview_api_payload,
            commands::run_consolidation,
            embedded_model::start_embedded_repair_model,
            embedded_model::stop_embedded_repair_model,
            embedded_model::embedded_repair_model_status,
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
