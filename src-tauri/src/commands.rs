use std::{fs, path::PathBuf};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State, Window};

use state_engine::{
    consolidation::consolidate_soul,
    context_compiler::{
        compile_context_for_messages, compile_context_for_separate_user_message, estimate_tokens,
        ContextMessage, ContextPreview,
    },
    hidden_state::{parse_hidden_state, HiddenState},
    patch::EnginePatch,
    setting::{new_default_setting, SettingSoul},
    soul::{fresh_scenario_soul, new_default_soul, Soul},
};

use crate::{
    db::{
        self, AssistantMessageVariant, ChatMessage, LlmPayloadLog, ProviderProfile, SettingSummary,
        SoulSummary,
    },
    providers::{
        api::{build_system_prompt, ApiProvider, ApiProviderSettings},
        mock::MockProvider,
    },
    AppState,
};

const CONSOLIDATION_INTERVAL_TURNS: u64 = 10;
const NO_LLM_PAYLOAD_LOGS_MESSAGE: &str = "No LLM payload logs found for this conversation.";

#[derive(Debug, serde::Serialize)]
pub struct TurnResult {
    pub conversation_id: String,
    pub soul: Soul,
    pub visible_response: String,
    pub context_preview: ContextPreview,
    pub messages: Vec<ChatMessage>,
    pub consolidation_ran: bool,
    pub debug: TurnDebug,
}

#[derive(Debug, serde::Serialize)]
pub struct TurnDebug {
    pub provider: String,
    pub hidden_state_found: bool,
    pub fallback_hidden_state_generated: bool,
    pub tag: Option<String>,
    pub trust_delta: Option<f32>,
    pub affection_delta: Option<f32>,
    pub new_location: Option<String>,
    pub present_characters: Vec<String>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct StreamChunk {
    pub conversation_id: String,
    pub chunk: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LlmPayloadTokenEstimate {
    pub system: usize,
    pub context: usize,
    pub user: usize,
    pub total: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct LlmPayloadPreview {
    pub provider: String,
    pub mode: String,
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context: String,
    pub estimated_tokens: LlmPayloadTokenEstimate,
}

#[derive(Debug, serde::Serialize)]
pub struct VariantSelectionResult {
    pub variants: Vec<AssistantMessageVariant>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub message: String,
}

#[tauri::command]
pub fn create_default_soul(character_name: String) -> Soul {
    new_default_soul(&character_name)
}

#[tauri::command]
pub fn create_fresh_scenario_soul(
    state: State<'_, AppState>,
    soul_id: String,
    setting_id: Option<String>,
) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let base = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let scenario_world = setting_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map(|id| db::get_setting(&conn, id).map(|setting| setting.world))
        .transpose()
        .map_err(|err| err.to_string())?;
    let fresh = fresh_scenario_soul(&base, scenario_world);
    db::upsert_soul(&conn, &fresh).map_err(|err| err.to_string())?;
    Ok(fresh)
}

#[tauri::command]
pub fn create_default_setting(setting_name: String) -> SettingSoul {
    new_default_setting(&setting_name)
}

#[tauri::command]
pub fn load_soul_file(path: String) -> Result<Soul, String> {
    let content = fs::read_to_string(PathBuf::from(path)).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn load_setting_file(path: String) -> Result<SettingSoul, String> {
    let content = fs::read_to_string(PathBuf::from(path)).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_soul_file(path: String, soul: Soul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&soul).map_err(|err| err.to_string())?;
    fs::write(PathBuf::from(path), content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_setting_file(path: String, setting: SettingSoul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&setting).map_err(|err| err.to_string())?;
    fs::write(PathBuf::from(path), content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_settings(state: State<'_, AppState>) -> Result<Vec<SettingSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_settings(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_soul(state: State<'_, AppState>, soul: Soul) -> Result<SoulSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_setting(
    state: State<'_, AppState>,
    setting: SettingSoul,
) -> Result<SettingSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_setting(&conn, &setting).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_soul(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, setting_id: String) -> Result<SettingSoul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_setting(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_soul(state: State<'_, AppState>, soul_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_setting(state: State<'_, AppState>, setting_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_setting(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_conversation(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_message(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_message(&conn, &conversation_id, message_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_assistant_message_variants(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
) -> Result<Vec<AssistantMessageVariant>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_assistant_message_variants(&conn, &conversation_id, message_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn select_assistant_message_variant(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    variant_id: i64,
) -> Result<VariantSelectionResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::select_assistant_message_variant(&conn, &conversation_id, message_id, variant_id)
        .map_err(|err| err.to_string())?;
    Ok(VariantSelectionResult {
        variants: db::list_assistant_message_variants(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?,
        messages: db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?,
    })
}

#[tauri::command]
pub fn delete_assistant_message_variant(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    variant_id: i64,
) -> Result<VariantSelectionResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_assistant_message_variant(&conn, &conversation_id, message_id, variant_id)
        .map_err(|err| err.to_string())?;
    Ok(VariantSelectionResult {
        variants: db::list_assistant_message_variants(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?,
        messages: db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?,
    })
}

#[tauri::command]
pub fn list_llm_payload_logs(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<LlmPayloadLog>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_llm_payload_log(
    state: State<'_, AppState>,
    log_id: i64,
) -> Result<LlmPayloadLog, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_llm_payload_log(&conn, log_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_visible_chat_log(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ExportResult, String> {
    let messages = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::list_messages(&conn, &conversation_id, 10_000).map_err(|err| err.to_string())?
    };
    let markdown = render_visible_chat_log(&messages);
    let path = write_export_file(&app, &conversation_id, "visible-chat-log", &markdown)?;
    Ok(ExportResult {
        path: path.display().to_string(),
        message: "Visible chat log exported.".into(),
    })
}

#[tauri::command]
pub fn export_llm_payload_history(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ExportResult, String> {
    let logs = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())?
    };
    let markdown = render_llm_payload_history(&logs);
    let path = write_export_file(&app, &conversation_id, "llm-payload-history", &markdown)?;
    Ok(ExportResult {
        path: path.display().to_string(),
        message: if logs.is_empty() {
            NO_LLM_PAYLOAD_LOGS_MESSAGE.into()
        } else {
            format!("Exported {} LLM payload log(s).", logs.len())
        },
    })
}

#[tauri::command]
pub fn list_provider_profiles(state: State<'_, AppState>) -> Result<Vec<ProviderProfile>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_provider_profiles(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProviderProfile, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_provider_profile(
    state: State<'_, AppState>,
    profile: ProviderProfile,
) -> Result<ProviderProfile, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_provider_profile(&conn, &profile).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn compile_context(
    state: State<'_, AppState>,
    soul_id: String,
    conversation_id: String,
) -> Result<ContextPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let messages = db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?;
    Ok(compile_context_for_messages(
        &soul,
        &messages_to_context(messages),
    ))
}

#[tauri::command]
pub fn preview_api_payload(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    settings: ApiProviderSettings,
    provider: String,
) -> Result<LlmPayloadPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let messages = messages_to_context(
        db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?,
    );

    Ok(build_llm_payload_preview(
        &soul, &messages, &user_text, &mode, &settings, &provider,
    ))
}

#[tauri::command]
pub fn run_consolidation(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    consolidate_soul(&mut soul);
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    Ok(soul)
}

#[tauri::command]
pub fn send_mock_turn(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
) -> Result<TurnResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    send_mock_turn_with_conn(
        &conn,
        conversation_id,
        soul_id,
        user_text,
        mode,
        replacement_assistant_id,
        correction_instruction,
    )
}

fn send_mock_turn_with_conn(
    conn: &Connection,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
) -> Result<TurnResult, String> {
    let (mut soul, snapshot_user_text, pre_turn_soul_json) =
        if let Some(message_id) = replacement_assistant_id {
            let snapshot = db::get_turn_snapshot(&conn, &conversation_id, message_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "No turn snapshot found for assistant response".to_string())?;
            let soul: Soul =
                serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
            (soul, snapshot.user_text, snapshot.soul_json)
        } else {
            let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
            let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
            (soul, user_text.clone(), pre_turn_soul_json)
        };

    db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
        .map_err(|err| err.to_string())?;
    if replacement_assistant_id.is_none() {
        db::insert_message(&conn, &conversation_id, "user", &user_text)
            .map_err(|err| err.to_string())?;
    }

    let before_messages = match replacement_assistant_id {
        Some(message_id) => db::list_messages_before_id(&conn, &conversation_id, message_id, 5),
        None => db::list_messages(&conn, &conversation_id, 5),
    }
    .map_err(|err| err.to_string())?;
    let context_preview = compile_context_with_correction(
        &soul,
        &messages_to_context(before_messages),
        correction_instruction.as_deref(),
    );
    let provider = MockProvider::default();
    let raw_response = provider.complete(&soul, &context_preview.text, &snapshot_user_text, &mode);
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let debug = debug_from_hidden_state("Mock", &parsed.hidden_state, true, false);
    let debug_json = serde_json::to_string(&debug).map_err(|err| err.to_string())?;

    parsed.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    let assistant_message_id = if let Some(message_id) = replacement_assistant_id {
        message_id
    } else {
        db::insert_message_and_get_id(&conn, &conversation_id, "assistant", &parsed.visible_text)
            .map_err(|err| err.to_string())?
    };

    if replacement_assistant_id.is_some() {
        db::create_assistant_message_variant(
            &conn,
            &conversation_id,
            assistant_message_id,
            &parsed.visible_text,
            None,
            Some(
                if correction_instruction
                    .as_deref()
                    .map(str::trim)
                    .filter(|instruction| !instruction.is_empty())
                    .is_some()
                {
                    "fix"
                } else {
                    "regenerate"
                },
            ),
            true,
            Some(&pre_turn_soul_json),
            Some(&debug_json),
        )
        .map_err(|err| err.to_string())?;
    } else {
        db::seed_initial_assistant_message_variant(
            &conn,
            &conversation_id,
            assistant_message_id,
            &parsed.visible_text,
            Some("original"),
            Some(&pre_turn_soul_json),
            Some(&debug_json),
        )
        .map_err(|err| err.to_string())?;
    }

    if replacement_assistant_id.is_none() {
        db::upsert_turn_snapshot(
            &conn,
            &db::TurnSnapshot {
                conversation_id: conversation_id.clone(),
                assistant_message_id,
                user_text: snapshot_user_text.clone(),
                soul_json: pre_turn_soul_json.clone(),
            },
        )
        .map_err(|err| err.to_string())?;
    }

    let consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
    if consolidation_ran {
        consolidate_soul(&mut soul);
    }

    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    let context_preview =
        compile_context_for_messages(&soul, &messages_to_context(messages.clone()));

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response: parsed.visible_text,
        context_preview,
        messages,
        consolidation_ran,
        debug,
    })
}

#[tauri::command]
pub async fn send_api_turn(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    settings: ApiProviderSettings,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
) -> Result<TurnResult, String> {
    let (mut soul, context_preview, snapshot_user_text, pre_turn_soul_json) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let (soul, snapshot_user_text) = if let Some(message_id) = replacement_assistant_id {
            let snapshot = db::get_turn_snapshot(&conn, &conversation_id, message_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "No turn snapshot found for assistant response".to_string())?;
            let soul: Soul =
                serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
            (soul, snapshot.user_text)
        } else {
            (
                db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?,
                user_text.clone(),
            )
        };
        db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
            .map_err(|err| err.to_string())?;
        if replacement_assistant_id.is_none() {
            db::insert_message(&conn, &conversation_id, "user", &user_text)
                .map_err(|err| err.to_string())?;
        }

        let before_messages = match replacement_assistant_id {
            Some(message_id) => db::list_messages_before_id(&conn, &conversation_id, message_id, 5),
            None => db::list_messages(&conn, &conversation_id, 5),
        }
        .map_err(|err| err.to_string())?;
        let context_preview = compile_context_with_correction(
            &soul,
            &messages_to_context(before_messages),
            correction_instruction.as_deref(),
        );
        let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
        (
            soul,
            context_preview,
            snapshot_user_text,
            pre_turn_soul_json,
        )
    };

    let system_prompt = build_system_prompt(&settings, &soul, &context_preview.text, &mode);
    let provider = ApiProvider::default();
    let stream_conversation_id = conversation_id.clone();
    let effective_user_text =
        build_user_text_with_correction(&snapshot_user_text, correction_instruction.as_deref());
    let payload_log_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                id: 0,
                conversation_id: conversation_id.clone(),
                message_id: replacement_assistant_id,
                provider: "API".into(),
                mode: mode.trim().to_string(),
                model: settings.model.trim().to_string(),
                base_url: settings.base_url.trim().to_string(),
                system_message: system_prompt.clone(),
                user_message: effective_user_text.trim().to_string(),
                context_text: context_preview.text.clone(),
                estimated_system_tokens: estimate_tokens(&system_prompt),
                estimated_user_tokens: estimate_tokens(&effective_user_text),
                estimated_total_tokens: estimate_tokens(&system_prompt)
                    + estimate_tokens(&effective_user_text),
                created_at: db::now_ts(),
            },
        )
        .map_err(|err| err.to_string())?
    };
    let raw_response = provider
        .complete_streaming(&settings, &system_prompt, &effective_user_text, |chunk| {
            window
                .emit(
                    "api-chunk",
                    StreamChunk {
                        conversation_id: stream_conversation_id.clone(),
                        chunk: chunk.to_string(),
                    },
                )
                .map_err(|err| err.to_string())
        })
        .await?;
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let hidden_state_found = parsed.has_patch();
    let fallback_hidden_state_generated = !hidden_state_found;
    let (hidden_state, engine_patch) = if fallback_hidden_state_generated {
        let hidden_state =
            generated_api_hidden_state(&soul, &snapshot_user_text, &parsed.visible_text);
        let engine_patch = EnginePatch::from(&hidden_state);
        (hidden_state, engine_patch)
    } else {
        (parsed.hidden_state.clone(), parsed.engine_patch.clone())
    };
    let debug = debug_from_hidden_state(
        "API",
        &hidden_state,
        hidden_state_found,
        fallback_hidden_state_generated,
    );
    let debug_json = serde_json::to_string(&debug).map_err(|err| err.to_string())?;

    let _ = engine_patch.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    let visible_response = parsed.visible_text;

    let (messages, context_preview, consolidation_ran) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let assistant_message_id = if let Some(message_id) = replacement_assistant_id {
            message_id
        } else {
            db::insert_message_and_get_id(&conn, &conversation_id, "assistant", &visible_response)
                .map_err(|err| err.to_string())?
        };
        db::set_llm_payload_log_message_id(&conn, payload_log_id, assistant_message_id)
            .map_err(|err| err.to_string())?;

        if replacement_assistant_id.is_some() {
            db::create_assistant_message_variant(
                &conn,
                &conversation_id,
                assistant_message_id,
                &visible_response,
                None,
                Some(
                    if correction_instruction
                        .as_deref()
                        .map(str::trim)
                        .filter(|instruction| !instruction.is_empty())
                        .is_some()
                    {
                        "fix"
                    } else {
                        "regenerate"
                    },
                ),
                true,
                Some(&pre_turn_soul_json),
                Some(&debug_json),
            )
            .map_err(|err| err.to_string())?;
        } else {
            db::seed_initial_assistant_message_variant(
                &conn,
                &conversation_id,
                assistant_message_id,
                &visible_response,
                Some("original"),
                Some(&pre_turn_soul_json),
                Some(&debug_json),
            )
            .map_err(|err| err.to_string())?;
        }

        if replacement_assistant_id.is_none() {
            db::upsert_turn_snapshot(
                &conn,
                &db::TurnSnapshot {
                    conversation_id: conversation_id.clone(),
                    assistant_message_id,
                    user_text: snapshot_user_text.clone(),
                    soul_json: pre_turn_soul_json.clone(),
                },
            )
            .map_err(|err| err.to_string())?;
        }

        let consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
        if consolidation_ran {
            consolidate_soul(&mut soul);
        }

        db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
        let messages =
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
        let context_preview =
            compile_context_for_messages(&soul, &messages_to_context(messages.clone()));

        (messages, context_preview, consolidation_ran)
    };

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response,
        context_preview,
        messages,
        consolidation_ran,
        debug,
    })
}

fn messages_to_context(messages: Vec<ChatMessage>) -> Vec<ContextMessage> {
    messages
        .into_iter()
        .map(|message| ContextMessage {
            role: message.role,
            content: message.content,
        })
        .collect()
}

fn compile_context_with_correction(
    soul: &Soul,
    messages: &[ContextMessage],
    correction_instruction: Option<&str>,
) -> ContextPreview {
    let mut preview = compile_context_for_separate_user_message(soul, messages);
    let instruction = correction_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty());
    if let Some(instruction) = instruction {
        preview.text = format!(
            "[FIX INSTRUCTION, TEMPORARY HIGH PRIORITY]\nApply this only while generating the next narrator response. Do not store it as memory.\nInstruction: {instruction}\n\n{}",
            preview.text
        );
        preview.estimated_tokens = estimate_tokens(&preview.text);
        preview.truncated = true;
    }
    preview
}

fn build_user_text_with_correction(
    user_text: &str,
    correction_instruction: Option<&str>,
) -> String {
    let instruction = correction_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty());
    if let Some(instruction) = instruction {
        format!("{user_text}\n\n[FIX INSTRUCTION - APPLY TO THIS RESPONSE ONLY]\n{instruction}")
    } else {
        user_text.to_string()
    }
}

fn build_llm_payload_preview(
    soul: &Soul,
    messages: &[ContextMessage],
    user_text: &str,
    mode: &str,
    settings: &ApiProviderSettings,
    provider: &str,
) -> LlmPayloadPreview {
    let context_preview = if user_text.trim().is_empty() {
        compile_context_for_messages(soul, messages)
    } else {
        compile_context_for_separate_user_message(soul, messages)
    };
    let system_message = build_system_prompt(settings, soul, &context_preview.text, mode);
    let user_message = user_text.trim().to_string();
    let system_tokens = estimate_tokens(&system_message);
    let context_tokens = estimate_tokens(&context_preview.text);
    let user_tokens = estimate_tokens(&user_message);

    LlmPayloadPreview {
        provider: provider.trim().to_string(),
        mode: mode.trim().to_string(),
        model: settings.model.trim().to_string(),
        base_url: settings.base_url.trim().to_string(),
        system_message,
        user_message,
        context: context_preview.text,
        estimated_tokens: LlmPayloadTokenEstimate {
            system: system_tokens,
            context: context_tokens,
            user: user_tokens,
            total: system_tokens + user_tokens,
        },
    }
}

fn render_visible_chat_log(messages: &[ChatMessage]) -> String {
    let mut lines = vec!["# Mnemosyne Chat Log".to_string()];
    for message in messages.iter().filter(|message| message.role != "system") {
        let role = match message.role.as_str() {
            "assistant" => "Narrator",
            "user" => "User",
            other => other,
        };
        let content = if message.role == "assistant" {
            strip_hidden_state_blocks(&message.content)
        } else {
            message.content.trim_end().to_string()
        };
        lines.push(String::new());
        lines.push(format!("## {role}"));
        lines.push(format!("Created: {}", message.created_at));
        lines.push(String::new());
        lines.push(content);
    }
    lines.push(String::new());
    lines.join("\n")
}

fn render_llm_payload_history(logs: &[LlmPayloadLog]) -> String {
    let mut lines = vec!["# Mnemosyne LLM Payload History".to_string()];
    if logs.is_empty() {
        lines.push(String::new());
        lines.push(NO_LLM_PAYLOAD_LOGS_MESSAGE.into());
        lines.push("Payload history is recorded for API provider turns. Mock conversations do not send LLM payloads.".into());
        lines.push(String::new());
        return lines.join("\n");
    }
    for (index, log) in logs.iter().enumerate() {
        lines.push(String::new());
        lines.push(format!("## Payload {}", index + 1));
        lines.push(format!("Created: {}", log.created_at));
        lines.push(format!("Provider: {}", log.provider));
        lines.push(format!("Model: {}", log.model));
        lines.push(format!("Mode: {}", log.mode));
        lines.push(format!("Base URL: {}", log.base_url));
        lines.push(format!(
            "Estimated tokens: system {}, user {}, total {}",
            log.estimated_system_tokens, log.estimated_user_tokens, log.estimated_total_tokens
        ));
        lines.push(String::new());
        lines.push("### SYSTEM MESSAGE".into());
        lines.push(log.system_message.clone());
        lines.push(String::new());
        lines.push("### USER MESSAGE".into());
        lines.push(log.user_message.clone());
        lines.push(String::new());
        lines.push("### CONTEXT".into());
        lines.push(log.context_text.clone());
    }
    lines.push(String::new());
    lines.join("\n")
}

fn write_export_file(
    app: &AppHandle,
    conversation_id: &str,
    label: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .download_dir()
        .or_else(|_| std::env::current_dir())
        .map_err(|err| err.to_string())?;
    dir.push("mnemosyne-exports");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let safe_conversation = safe_filename(conversation_id);
    let filename = format!("mnemosyne-{safe_conversation}-{label}-{}.md", db::now_ts());
    dir.push(filename);
    fs::write(&dir, content).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn safe_filename(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    safe.trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>()
        .if_empty("conversation")
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn strip_hidden_state_blocks(content: &str) -> String {
    let mut cleaned = content.to_string();
    loop {
        let Some(start) = cleaned.find("[HIDDEN STATE]") else {
            break;
        };
        if let Some(relative_end) = cleaned[start..].find("[/HIDDEN STATE]") {
            let end = start + relative_end + "[/HIDDEN STATE]".len();
            cleaned.replace_range(start..end, "");
        } else {
            cleaned.truncate(start);
            break;
        }
    }
    for marker in [
        "[HIDDEN_STATE]",
        "[HIDDEN STATE",
        "[/HIDDEN_STATE",
        "[/HIDDEN STATE",
    ] {
        if let Some(start) = cleaned.find(marker) {
            cleaned.truncate(start);
        }
    }
    cleaned.trim_end().to_string()
}

fn debug_from_hidden_state(
    provider: &str,
    hidden_state: &HiddenState,
    hidden_state_found: bool,
    fallback_hidden_state_generated: bool,
) -> TurnDebug {
    TurnDebug {
        provider: provider.into(),
        hidden_state_found,
        fallback_hidden_state_generated,
        tag: hidden_state.tag.clone(),
        trust_delta: hidden_state.trust_delta,
        affection_delta: hidden_state.affection_delta,
        new_location: hidden_state.new_location.clone(),
        present_characters: hidden_state.present_characters.clone().unwrap_or_default(),
    }
}

fn generated_api_hidden_state(soul: &Soul, user_text: &str, visible_text: &str) -> HiddenState {
    let tag = classify_turn_tag(user_text);
    let assistant_excerpt = visible_text.chars().take(180).collect::<String>();
    HiddenState {
        memory: Some(format!(
            "{} responded through the API provider after the user said: {} Assistant cue: {}",
            soul.character_name,
            user_text.trim(),
            assistant_excerpt.trim()
        )),
        tag: Some(tag.into()),
        trust_delta: Some(if tag == "trust_building" { 3.0 } else { 1.0 }),
        affection_delta: Some(if tag == "bonding" { 3.0 } else { 1.0 }),
        world_event: Some(format!(
            "Completed API turn: user said {}; narrator response cue: {}",
            user_text.trim(),
            assistant_excerpt.trim()
        )),
        new_location: None,
        present_characters: Some(vec![soul.character_name.clone()]),
        arousal_delta: None,
        arousal_denied: None,
        orgasm_allowed: None,
        forced_orgasm: None,
    }
}

fn classify_turn_tag(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    if lower.contains("trust") || lower.contains("promise") || lower.contains("safe") {
        "trust_building"
    } else if lower.contains("hurt") || lower.contains("blood") || lower.contains("danger") {
        "threat"
    } else if lower.contains("remember")
        || lower.contains("childhood")
        || lower.contains("together")
    {
        "bonding"
    } else if lower.contains("where") || lower.contains("look") || lower.contains("room") {
        "orientation"
    } else {
        "observation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state_engine::{context_compiler::estimate_tokens, hidden_state::HiddenState};

    #[test]
    fn hidden_state_application_updates_soul() {
        let mut soul = new_default_soul("Aurora");
        let state = HiddenState {
            memory: Some("Aurora notices a safer rhythm in the exchange.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(4.0),
            affection_delta: Some(2.0),
            world_event: Some("A small trust-building exchange changed the mood.".into()),
            new_location: None,
            present_characters: None,
            arousal_delta: None,
            arousal_denied: None,
            orgasm_allowed: None,
            forced_orgasm: None,
        };

        state.apply_to_soul(&mut soul);

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.recent_events.len(), 1);
    }

    #[test]
    fn ten_mock_turns_trigger_consolidation_and_keep_context_lean() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        let soul_id = soul.character_id.clone();
        db::upsert_soul(&conn, &soul).expect("upsert soul");

        let turns = [
            "I promise this is safe.",
            "Look at the wall and the room.",
            "We remember childhood rain together.",
            "There is danger near the door.",
            "The light flickers without changing much.",
            "A neutral breath passes in the silence.",
            "Another quiet observation settles over the silence.",
            "One more observation keeps the scene grounded.",
            "Trust the route I found.",
            "Where are we now?",
        ];

        let mut final_result = None;
        for turn in turns {
            final_result = Some(
                send_mock_turn_with_conn(
                    &conn,
                    "acceptance".into(),
                    soul_id.clone(),
                    turn.into(),
                    "Reader".into(),
                    None,
                    None,
                )
                .expect("mock turn"),
            );
        }

        let result = final_result.expect("result");
        assert!(result.consolidation_ran);
        assert_eq!(result.soul.turn_counter, 10);
        assert_eq!(result.soul.turns_since_consolidation, 0);
        assert!(result.soul.memory.recent.len() <= 4);
        assert!(result.soul.memory.core.len() > soul.memory.core.len());
        assert!(result
            .soul
            .memory
            .schemas
            .iter()
            .any(|schema| schema.schema_type == "observation"));
        assert!(!result
            .soul
            .memory
            .recent
            .iter()
            .any(|memory| memory.tag == "observation"));
        assert!(result.context_preview.estimated_tokens <= 2_000);
        assert!(estimate_tokens(&result.context_preview.text) <= 2_000);
    }

    #[test]
    fn payload_preview_excludes_api_key_and_includes_messages() {
        let soul = new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "secret-key-that-must-not-appear".into(),
            model: "debug-model".into(),
            system_prompt: String::new(),
        };
        let messages = vec![ContextMessage {
            role: "user".into(),
            content: "Hello from the preview.".into(),
        }];

        let preview = build_llm_payload_preview(
            &soul,
            &messages,
            "Current user turn",
            "Reader",
            &settings,
            "API",
        );
        let serialized = serde_json::to_string(&preview).expect("serialize preview");

        assert!(!serialized.contains("secret-key-that-must-not-appear"));
        assert!(preview.system_message.contains("You are a narrator AI"));
        assert!(preview.user_message.contains("Current user turn"));
        assert!(preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
        assert!(preview
            .context
            .contains("The current user message follows as the next user message."));
        assert!(!preview.context.contains("Current user turn"));
    }

    #[test]
    fn payload_preview_token_estimates_are_nonzero() {
        let soul = new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "secret".into(),
            model: "debug-model".into(),
            system_prompt: String::new(),
        };

        let preview =
            build_llm_payload_preview(&soul, &[], "Current user turn", "Reader", &settings, "API");

        assert!(preview.estimated_tokens.system > 0);
        assert!(preview.estimated_tokens.context > 0);
        assert!(preview.estimated_tokens.user > 0);
        assert!(preview.estimated_tokens.total > 0);
    }

    #[test]
    fn compiled_context_orders_world_before_character() {
        let soul = new_default_soul("Aurora");
        let preview = compile_context_for_messages(&soul, &[]);

        assert_order(&preview.text, "[WORLD SNAPSHOT]", "[CHARACTER SNAPSHOT]");
    }

    #[test]
    fn latest_exchange_follows_recent_chat_and_contains_override() {
        let soul = new_default_soul("Aurora");
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "Earlier beat in the thread.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Aurora set the phone on the couch and moved toward the kitchen.".into(),
            },
            ContextMessage {
                role: "user".into(),
                content: "I want pad thai too.".into(),
            },
        ];
        let preview = compile_context_for_messages(&soul, &messages);

        assert_order(
            &preview.text,
            "[RECENT CHAT, LOWER PRIORITY]",
            "[LATEST EXCHANGE, HIGH PRIORITY]",
        );
        assert!(preview
            .text
            .contains("If older context conflicts with this section, ignore older context."));
    }

    #[test]
    fn regenerate_reuses_user_message_without_double_applying_state() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        let soul_id = soul.character_id.clone();
        db::upsert_soul(&conn, &soul).expect("upsert soul");

        let first = send_mock_turn_with_conn(
            &conn,
            "regen".into(),
            soul_id.clone(),
            "I promise this is safe.".into(),
            "Reader".into(),
            None,
            None,
        )
        .expect("first turn");
        let first_assistant = first
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "assistant")
            .expect("assistant")
            .id;

        let second = send_mock_turn_with_conn(
            &conn,
            "regen".into(),
            soul_id,
            "I promise this is safe.".into(),
            "Reader".into(),
            Some(first_assistant),
            None,
        )
        .expect("regenerated turn");

        let user_count = second
            .messages
            .iter()
            .filter(|message| message.role == "user")
            .count();
        assert_eq!(
            user_count, 1,
            "regenerate must not add another user message"
        );
        assert_eq!(
            second.soul.relationships["user"].trust, first.soul.relationships["user"].trust,
            "regenerate should restore snapshot and apply once"
        );
        let variants =
            db::list_assistant_message_variants(&conn, "regen", first_assistant).unwrap();
        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants
                .iter()
                .filter(|variant| variant.is_selected)
                .count(),
            1
        );
        assert_eq!(
            variants
                .iter()
                .position(|variant| variant.is_selected)
                .map(|index| index + 1),
            Some(2)
        );
    }

    #[test]
    fn correction_instruction_is_temporary_context_not_memory() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        let soul_id = soul.character_id.clone();
        db::upsert_soul(&conn, &soul).expect("upsert soul");

        let first = send_mock_turn_with_conn(
            &conn,
            "fix".into(),
            soul_id.clone(),
            "I show her the phone.".into(),
            "Reader".into(),
            None,
            None,
        )
        .expect("first turn");
        let assistant_id = first
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "assistant")
            .expect("assistant")
            .id;

        let corrected = send_mock_turn_with_conn(
            &conn,
            "fix".into(),
            soul_id,
            "I show her the phone.".into(),
            "Reader".into(),
            Some(assistant_id),
            Some("Continue from the kitchen. Do not replay the phone reveal.".into()),
        )
        .expect("corrected turn");

        let context = compile_context_with_correction(
            &corrected.soul,
            &messages_to_context(corrected.messages.clone()),
            Some("Continue from the kitchen. Do not replay the phone reveal."),
        );
        assert!(context
            .text
            .contains("[FIX INSTRUCTION, TEMPORARY HIGH PRIORITY]"));
        assert!(!corrected
            .soul
            .memory
            .recent
            .iter()
            .any(|memory| memory.content.contains("Do not replay the phone reveal")));
    }

    #[test]
    fn visible_chat_export_strips_hidden_state() {
        let messages = vec![
            ChatMessage {
                id: 1,
                conversation_id: "export".into(),
                role: "user".into(),
                content: "Hello.".into(),
                created_at: 10,
            },
            ChatMessage {
                id: 2,
                conversation_id: "export".into(),
                role: "assistant".into(),
                content:
                    "Visible narrator text.\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]"
                        .into(),
                created_at: 11,
            },
        ];

        let exported = render_visible_chat_log(&messages);

        assert!(exported.contains("# Mnemosyne Chat Log"));
        assert!(exported.contains("## User"));
        assert!(exported.contains("## Narrator"));
        assert!(exported.contains("Visible narrator text."));
        assert!(!exported.contains("[HIDDEN STATE]"));
        assert!(!exported.contains("observation"));
    }

    #[test]
    fn payload_history_export_includes_prior_payloads_without_api_key() {
        let logs = vec![
            LlmPayloadLog {
                id: 1,
                conversation_id: "history".into(),
                message_id: Some(10),
                provider: "API".into(),
                mode: "Reader".into(),
                model: "model-a".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System A with clothing context".into(),
                user_message: "User A".into(),
                context_text: "Context A".into(),
                estimated_system_tokens: 10,
                estimated_user_tokens: 2,
                estimated_total_tokens: 12,
                created_at: 100,
            },
            LlmPayloadLog {
                id: 2,
                conversation_id: "history".into(),
                message_id: Some(11),
                provider: "API".into(),
                mode: "God".into(),
                model: "model-b".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System B".into(),
                user_message: "User B".into(),
                context_text: "Context B".into(),
                estimated_system_tokens: 11,
                estimated_user_tokens: 3,
                estimated_total_tokens: 14,
                created_at: 101,
            },
        ];

        let exported = render_llm_payload_history(&logs);

        assert!(exported.contains("## Payload 1"));
        assert!(exported.contains("## Payload 2"));
        assert!(exported.contains("Model: model-a"));
        assert!(exported.contains("Mode: God"));
        assert!(exported.contains("Base URL: https://api.example/v1"));
        assert!(exported.contains("System A with clothing context"));
        assert!(exported.contains("Context B"));
        assert!(!exported.contains("api_key"));
        assert!(!exported.contains("secret"));
    }

    #[test]
    fn empty_payload_history_export_explains_no_logs() {
        let exported = render_llm_payload_history(&[]);

        assert!(exported.contains("# Mnemosyne LLM Payload History"));
        assert!(exported.contains(NO_LLM_PAYLOAD_LOGS_MESSAGE));
        assert!(exported.contains("Mock conversations do not send LLM payloads."));
        assert!(!exported.contains("## Payload 1"));
    }

    fn assert_order(text: &str, first: &str, second: &str) {
        let first_index = text.find(first).expect("first section");
        let second_index = text.find(second).expect("second section");
        assert!(first_index < second_index);
    }
}
