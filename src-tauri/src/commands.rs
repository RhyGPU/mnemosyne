use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

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
        api::{
            build_narrator_system_prompt, build_state_updater_prompt, ApiMessage, ApiProvider,
            ApiProviderSettings, PreparedApiPayload,
        },
        mock::MockProvider,
    },
    AppState,
};

const CONSOLIDATION_INTERVAL_TURNS: u64 = 10;
const NO_LLM_PAYLOAD_LOGS_MESSAGE: &str = "No LLM payload logs found for this conversation.";
const FULL_CHAT_TOKEN_BUDGET: usize = 6_000;
static DEV_LOG_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    pub narrator_response_saved: bool,
    pub assistant_message_id: Option<i64>,
    pub selected_variant_id: Option<i64>,
    pub state_updater_status: String,
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

#[derive(Debug, serde::Serialize, Clone)]
pub struct SavedChatMessageEvent {
    pub conversation_id: String,
    pub message: ChatMessage,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct DevLogEvent {
    pub id: String,
    pub timestamp: i64,
    pub level: String,
    pub category: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
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
    pub context_mode: String,
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context: String,
    pub messages: Vec<ApiMessage>,
    pub truncated: bool,
    pub estimated_tokens: LlmPayloadTokenEstimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextMode {
    Brief,
    FullChat,
}

impl ContextMode {
    fn from_label(value: Option<&str>) -> Self {
        match value
            .map(str::trim)
            .unwrap_or("brief")
            .to_ascii_lowercase()
            .as_str()
        {
            "full_chat" | "full chat" | "full-chat" => Self::FullChat,
            _ => Self::Brief,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::FullChat => "full_chat",
        }
    }
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
pub fn update_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    content: String,
) -> Result<Vec<ChatMessage>, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("User message cannot be empty".into());
    }
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let updated = db::update_user_message_content(&conn, &conversation_id, message_id, trimmed)
        .map_err(|err| err.to_string())?;
    if !updated {
        return Err("User message not found".into());
    }
    db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
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
    context_mode: Option<String>,
) -> Result<LlmPayloadPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let messages = messages_to_context(
        db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?,
    );

    Ok(build_llm_payload_preview(
        &soul,
        &messages,
        &user_text,
        &mode,
        &settings,
        &provider,
        ContextMode::from_label(context_mode.as_deref()),
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
    narrator_settings: ApiProviderSettings,
    state_updater_settings: ApiProviderSettings,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
    context_mode: Option<String>,
) -> Result<TurnResult, String> {
    let context_mode = ContextMode::from_label(context_mode.as_deref());
    emit_dev_log(
        &window,
        "info",
        "app",
        "User message submitted",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "context_mode": context_mode.label(),
            "mode": mode.as_str(),
            "replacement_assistant_id": replacement_assistant_id,
            "user_message_chars": user_text.chars().count()
        })),
    );
    let (mut soul, context_messages, context_preview, snapshot_user_text, pre_turn_soul_json) = {
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

        let history_limit = if context_mode == ContextMode::FullChat {
            100
        } else {
            5
        };
        let before_messages = match replacement_assistant_id {
            Some(message_id) => {
                db::list_messages_before_id(&conn, &conversation_id, message_id, history_limit)
            }
            None => db::list_messages(&conn, &conversation_id, history_limit),
        }
        .map_err(|err| err.to_string())?;
        let context_messages = messages_to_context(before_messages);
        let context_preview = compile_context_with_correction(
            &soul,
            &context_messages,
            correction_instruction.as_deref(),
        );
        let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
        (
            soul,
            context_messages,
            context_preview,
            snapshot_user_text,
            pre_turn_soul_json,
        )
    };
    emit_dev_log(
        &window,
        "info",
        "context",
        "Narrator context compiled",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "context_mode": context_mode.label(),
            "context_tokens": context_preview.estimated_tokens,
            "context_truncated": context_preview.truncated,
            "history_messages": context_messages.len()
        })),
    );

    let effective_user_text =
        build_user_text_with_correction(&snapshot_user_text, correction_instruction.as_deref());
    let narrator_payload = prepare_narrator_payload(
        &narrator_settings,
        &soul,
        &context_messages,
        &context_preview,
        &effective_user_text,
        &mode,
        context_mode,
    );
    let provider = ApiProvider::default();
    let stream_conversation_id = conversation_id.clone();
    let narrator_token_estimate =
        estimate_tokens(&serialize_api_messages(&narrator_payload.messages));
    emit_dev_log(
        &window,
        "info",
        "narrator",
        "Narrator call started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "provider": format!("narrator_{}", context_mode.label()),
            "model": narrator_settings.model.trim(),
            "base_url": narrator_settings.base_url.trim(),
            "context_mode": context_mode.label(),
            "estimated_total_tokens": narrator_token_estimate,
            "truncated": narrator_payload.truncated
        })),
    );
    let payload_log_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                id: 0,
                conversation_id: conversation_id.clone(),
                message_id: replacement_assistant_id,
                provider: format!("narrator_{}", context_mode.label()),
                mode: mode.trim().to_string(),
                context_mode: context_mode.label().into(),
                model: narrator_settings.model.trim().to_string(),
                base_url: narrator_settings.base_url.trim().to_string(),
                system_message: narrator_payload
                    .messages
                    .first()
                    .map(|message| message.content.clone())
                    .unwrap_or_default(),
                user_message: narrator_payload.user_message.clone(),
                context_text: narrator_payload.context_text.clone(),
                estimated_system_tokens: estimate_tokens(
                    narrator_payload
                        .messages
                        .first()
                        .map(|message| message.content.as_str())
                        .unwrap_or_default(),
                ),
                estimated_user_tokens: estimate_tokens(&effective_user_text),
                estimated_total_tokens: estimate_tokens(&serialize_api_messages(
                    &narrator_payload.messages,
                )),
                truncated: narrator_payload.truncated,
                created_at: db::now_ts(),
            },
        )
        .map_err(|err| err.to_string())?
    };
    emit_dev_log(
        &window,
        "debug",
        "db",
        "Narrator payload log stored",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "payload_log_id": payload_log_id
        })),
    );
    let stream_chunk_count = Arc::new(AtomicU64::new(0));
    let stream_byte_count = Arc::new(AtomicU64::new(0));
    let stream_chunk_count_for_callback = Arc::clone(&stream_chunk_count);
    let stream_byte_count_for_callback = Arc::clone(&stream_byte_count);
    emit_dev_log(
        &window,
        "info",
        "stream",
        "Narrator streaming started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str()
        })),
    );
    let raw_response = match provider
        .complete_streaming_messages(
            &narrator_settings,
            narrator_payload.messages.clone(),
            |chunk| {
                stream_chunk_count_for_callback.fetch_add(1, Ordering::Relaxed);
                stream_byte_count_for_callback
                    .fetch_add(chunk.as_bytes().len() as u64, Ordering::Relaxed);
                window
                    .emit(
                        "api-chunk",
                        StreamChunk {
                            conversation_id: stream_conversation_id.clone(),
                            chunk: chunk.to_string(),
                        },
                    )
                    .map_err(|err| err.to_string())
            },
        )
        .await
    {
        Ok(response) => {
            emit_dev_log(
                &window,
                "success",
                "stream",
                "Narrator streaming finished",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "chunks": stream_chunk_count.load(Ordering::Relaxed),
                    "bytes": stream_byte_count.load(Ordering::Relaxed)
                })),
            );
            response
        }
        Err(err) => {
            emit_dev_log(
                &window,
                "error",
                "narrator",
                "Narrator provider failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "error": err.clone()
                })),
            );
            return Err(err);
        }
    };
    let parsed = match parse_hidden_state(&raw_response) {
        Ok(parsed) => parsed,
        Err(err) => {
            emit_dev_log(
                &window,
                "error",
                "narrator",
                "Narrator response parse failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "error": err.to_string()
                })),
            );
            return Err(err.to_string());
        }
    };
    let visible_response = parsed.visible_text.clone();
    if visible_response.trim().is_empty() {
        emit_dev_log(
            &window,
            "error",
            "narrator",
            "Narrator provider returned empty visible response",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str()
            })),
        );
        return Err("Narrator provider returned an empty visible response".into());
    }

    let (assistant_message_id, selected_variant_id) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        save_visible_narrator_response(
            &conn,
            &conversation_id,
            &visible_response,
            replacement_assistant_id,
            correction_instruction.as_deref(),
            &pre_turn_soul_json,
            &snapshot_user_text,
            payload_log_id,
        )?
    };
    {
        let saved_message = state
            .conn
            .lock()
            .map_err(|err| err.to_string())
            .and_then(|conn| {
                db::get_message(&conn, &conversation_id, assistant_message_id)
                    .map_err(|err| err.to_string())
            });
        match saved_message {
            Ok(message) => {
                if let Err(err) = window.emit(
                    "chat-message-saved",
                    SavedChatMessageEvent {
                        conversation_id: conversation_id.clone(),
                        message,
                    },
                ) {
                    eprintln!("Saved narrator message event failed: {err}");
                }
            }
            Err(err) => eprintln!("Saved narrator message reload failed: {err}"),
        }
    }
    emit_dev_log(
        &window,
        "success",
        "narrator",
        "Narrator response saved",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "selected_variant_id": selected_variant_id,
            "visible_chars": visible_response.chars().count()
        })),
    );

    let updater_system_prompt = build_state_updater_prompt(&soul);
    let updater_user_message =
        build_state_updater_user_message(&snapshot_user_text, &visible_response);
    emit_dev_log(
        &window,
        "info",
        "state_updater",
        "State updater started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "model": state_updater_settings.model.trim(),
            "base_url": state_updater_settings.base_url.trim(),
            "estimated_total_tokens": estimate_tokens(&updater_system_prompt) + estimate_tokens(&updater_user_message)
        })),
    );
    let updater_log_id = match state
        .conn
        .lock()
        .map_err(|err| err.to_string())
        .and_then(|conn| {
            db::insert_llm_payload_log(
                &conn,
                &LlmPayloadLog {
                    id: 0,
                    conversation_id: conversation_id.clone(),
                    message_id: Some(assistant_message_id),
                    provider: "state_updater".into(),
                    mode: "state_updater".into(),
                    context_mode: context_mode.label().into(),
                    model: state_updater_settings.model.trim().to_string(),
                    base_url: state_updater_settings.base_url.trim().to_string(),
                    system_message: updater_system_prompt.clone(),
                    user_message: updater_user_message.clone(),
                    context_text: updater_system_prompt.clone(),
                    estimated_system_tokens: estimate_tokens(&updater_system_prompt),
                    estimated_user_tokens: estimate_tokens(&updater_user_message),
                    estimated_total_tokens: estimate_tokens(&updater_system_prompt)
                        + estimate_tokens(&updater_user_message),
                    truncated: false,
                    created_at: db::now_ts(),
                },
            )
            .map_err(|err| err.to_string())
        }) {
        Ok(log_id) => Some(log_id),
        Err(err) => {
            eprintln!(
                "State updater payload logging failed; narration saved without updater log: {err}"
            );
            emit_dev_log(
                &window,
                "warn",
                "db",
                "State updater payload log failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "assistant_message_id": assistant_message_id,
                    "error": err.clone()
                })),
            );
            None
        }
    };
    let updater_result = provider
        .complete_prompt(
            &state_updater_settings,
            &updater_system_prompt,
            &updater_user_message,
            0.0,
        )
        .await
        .and_then(|updater_response| parse_engine_patch_json(&updater_response));
    let (hidden_state, engine_patch, state_updater_status, hidden_state_found) =
        match updater_result {
            Ok(patch) => {
                let engine_patch = sanitize_state_updater_patch(
                    patch,
                    &soul,
                    &snapshot_user_text,
                    &visible_response,
                );
                let hidden_state = hidden_state_from_engine_patch(&engine_patch);
                (hidden_state, engine_patch, "success".to_string(), true)
            }
            Err(err) => {
                eprintln!("State updater failed; narration saved without state update: {err}");
                emit_dev_log(
                    &window,
                    "error",
                    "state_updater",
                    "State updater failed; narration saved without state update",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.clone()
                    })),
                );
                (
                    HiddenState::default(),
                    EnginePatch::default(),
                    format!("failed: {err}"),
                    false,
                )
            }
        };
    let fallback_hidden_state_generated = false;
    let mut debug = debug_from_hidden_state(
        "API",
        &hidden_state,
        hidden_state_found,
        fallback_hidden_state_generated,
    );
    debug.narrator_response_saved = true;
    debug.assistant_message_id = Some(assistant_message_id);
    debug.selected_variant_id = selected_variant_id;
    debug.state_updater_status = state_updater_status;

    match engine_patch.apply_to_soul(&mut soul) {
        Ok(report) => emit_dev_log(
            &window,
            "success",
            "state_updater",
            "EnginePatch applied",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "relationship_updated": report.relationship_updated,
                "memories_added": report.memories_added,
                "world_updated": report.world_updated,
                "body_updated": report.body_updated
            })),
        ),
        Err(err) => emit_dev_log(
            &window,
            "error",
            "state_updater",
            "EnginePatch skipped by validation",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "error": format!("{err:?}")
            })),
        ),
    }
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;

    let (messages, context_preview, consolidation_ran) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        if let Some(updater_log_id) = updater_log_id {
            if let Err(err) =
                db::set_llm_payload_log_message_id(&conn, updater_log_id, assistant_message_id)
            {
                eprintln!("State updater payload log link failed; narration remains saved: {err}");
                emit_dev_log(
                    &window,
                    "warn",
                    "db",
                    "State updater payload log link failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.to_string()
                    })),
                );
            }
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
    emit_dev_log(
        &window,
        "success",
        "success",
        "Turn complete",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "state_updater_status": debug.state_updater_status,
            "consolidation_ran": consolidation_ran,
            "messages": messages.len()
        })),
    );

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

#[allow(clippy::too_many_arguments)]
fn save_visible_narrator_response(
    conn: &Connection,
    conversation_id: &str,
    visible_response: &str,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<&str>,
    pre_turn_soul_json: &str,
    snapshot_user_text: &str,
    payload_log_id: i64,
) -> Result<(i64, Option<i64>), String> {
    let assistant_message_id = if let Some(message_id) = replacement_assistant_id {
        message_id
    } else {
        db::insert_message_and_get_id(conn, conversation_id, "assistant", visible_response)
            .map_err(|err| err.to_string())?
    };
    db::set_llm_payload_log_message_id(conn, payload_log_id, assistant_message_id)
        .map_err(|err| err.to_string())?;

    let pending_debug = TurnDebug {
        provider: "API".into(),
        hidden_state_found: false,
        fallback_hidden_state_generated: false,
        narrator_response_saved: true,
        assistant_message_id: Some(assistant_message_id),
        selected_variant_id: None,
        state_updater_status: "pending".into(),
        tag: None,
        trust_delta: None,
        affection_delta: None,
        new_location: None,
        present_characters: Vec::new(),
    };
    let pending_debug_json =
        serde_json::to_string(&pending_debug).map_err(|err| err.to_string())?;
    let variant = if replacement_assistant_id.is_some() {
        db::create_assistant_message_variant(
            conn,
            conversation_id,
            assistant_message_id,
            visible_response,
            None,
            Some(
                if correction_instruction
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
            Some(pre_turn_soul_json),
            Some(&pending_debug_json),
        )
        .map_err(|err| err.to_string())?
    } else {
        db::seed_initial_assistant_message_variant(
            conn,
            conversation_id,
            assistant_message_id,
            visible_response,
            Some("original"),
            Some(pre_turn_soul_json),
            Some(&pending_debug_json),
        )
        .map_err(|err| err.to_string())?
    };

    if replacement_assistant_id.is_none() {
        db::upsert_turn_snapshot(
            conn,
            &db::TurnSnapshot {
                conversation_id: conversation_id.to_string(),
                assistant_message_id,
                user_text: snapshot_user_text.to_string(),
                soul_json: pre_turn_soul_json.to_string(),
            },
        )
        .map_err(|err| err.to_string())?;
    }

    Ok((assistant_message_id, variant.id))
}

fn emit_dev_log(
    window: &Window,
    level: &str,
    category: &str,
    message: &str,
    details: Option<serde_json::Value>,
) {
    let sequence = DEV_LOG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let event = DevLogEvent {
        id: format!("{}-{sequence}", db::now_ts()),
        timestamp: db::now_ts(),
        level: level.to_string(),
        category: category.to_string(),
        message: message.to_string(),
        details: details.map(redact_dev_log_details),
    };
    if let Err(err) = window.emit("dev-log", event) {
        eprintln!("Dev log emit failed: {err}");
    }
}

fn redact_dev_log_details(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lowered = key.to_ascii_lowercase();
                    let redacted = lowered.contains("api_key")
                        || lowered == "authorization"
                        || lowered.contains("secret")
                        || lowered == "token"
                        || lowered.ends_with("_token")
                        || lowered.contains("bearer");
                    if redacted {
                        (key, serde_json::Value::String("[redacted]".into()))
                    } else {
                        (key, redact_dev_log_details(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(redact_dev_log_details).collect())
        }
        other => other,
    }
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

fn build_state_updater_user_message(user_text: &str, narrator_response: &str) -> String {
    format!(
        "[LATEST USER MESSAGE]\n{}\n\n[NARRATOR RESPONSE]\n{}",
        user_text.trim(),
        strip_hidden_state_blocks(narrator_response).trim()
    )
}

#[cfg(test)]
fn build_compact_updater_payload_for_test(
    soul: &Soul,
    user_text: &str,
    narrator_response: &str,
) -> String {
    format!(
        "{}\n\n{}",
        build_state_updater_prompt(soul),
        build_state_updater_user_message(user_text, narrator_response)
    )
}

fn parse_engine_patch_json(raw: &str) -> Result<EnginePatch, String> {
    let trimmed = raw.trim();
    let json = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.trim_end_matches("```").trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    serde_json::from_str::<EnginePatch>(json)
        .map_err(|err| format!("State updater returned invalid EnginePatch JSON: {err}"))
}

fn hidden_state_from_engine_patch(patch: &EnginePatch) -> HiddenState {
    let relationship = patch
        .soul_patch
        .as_ref()
        .and_then(|patch| patch.relationship_delta.as_ref());
    let memory = patch.soul_patch.as_ref().and_then(|patch| {
        patch
            .new_memories
            .iter()
            .find(|memory| !memory.content.trim().is_empty())
    });
    let world = patch.world_patch.as_ref();
    let body = patch.body_patch.as_ref();
    HiddenState {
        memory: memory.map(|memory| memory.content.clone()),
        tag: memory.and_then(|memory| memory.tag.clone()),
        trust_delta: relationship.and_then(|delta| delta.trust),
        affection_delta: relationship.and_then(|delta| delta.affection),
        world_event: world.and_then(|patch| {
            patch.recent_event.clone().or_else(|| {
                patch
                    .recent_events
                    .iter()
                    .find(|event| !event.trim().is_empty())
                    .cloned()
            })
        }),
        new_location: world.and_then(|patch| patch.location.clone()),
        present_characters: None,
        arousal_delta: body.and_then(|patch| patch.activation_delta),
        arousal_denied: body.and_then(|patch| patch.activation_blocked),
        orgasm_allowed: body.and_then(|patch| patch.peak_allowed),
        forced_orgasm: body.and_then(|patch| patch.forced_peak),
    }
}

fn sanitize_state_updater_patch(
    mut patch: EnginePatch,
    soul: &Soul,
    user_text: &str,
    narrator_response: &str,
) -> EnginePatch {
    let turn_text = format!("{user_text}\n{narrator_response}");
    let threat_scene = is_threat_or_emergency(&turn_text) || latest_world_event_is_threat(soul);
    let explicit_intimacy = explicitly_intimate(user_text);
    if let Some(body_patch) = patch.body_patch.as_mut() {
        if threat_scene && !explicit_intimacy {
            if body_patch.activation_delta.unwrap_or(0.0) > 0.0 {
                body_patch.activation_delta = Some(0.0);
            }
            body_patch.peak_allowed = Some(false);
            body_patch.forced_peak = Some(false);
        }
    }
    if patch
        .body_patch
        .as_ref()
        .map_or(false, |body| body.is_empty_for_commands())
    {
        patch.body_patch = None;
    }

    if let Some(world_patch) = patch.world_patch.as_mut() {
        if let Some(time) = world_patch.time_elapsed.as_mut() {
            *time = normalize_time_for_updater(time);
        }
        if world_patch.time_elapsed.is_some() && !user_text_has_explicit_time(user_text) {
            world_patch.time_elapsed = None;
        }
        if should_replace_default_plot(soul, world_patch, &turn_text) {
            if let Some(plot) = infer_active_plot(&turn_text) {
                world_patch
                    .active_plot_resolve
                    .push("Establish the first scene".into());
                world_patch
                    .active_plot_resolve
                    .push("Establish the first scene ??Aurora is alone, expecting company, or has just let someone in.".into());
                world_patch.active_plot_add.push(plot.into());
            }
        }
        if world_patch.is_empty_for_commands() {
            patch.world_patch = None;
        }
    }

    patch
}

fn normalize_time_for_updater(raw: &str) -> String {
    let trimmed = raw.trim();
    const PREFIX: &str = "Session start";
    let Some(found) = trimmed.find(PREFIX) else {
        return trimmed.to_string();
    };
    let suffix = trimmed[found + PREFIX.len()..]
        .trim_start_matches(['.', ' ', '-', ':'])
        .trim();
    if suffix.is_empty() {
        PREFIX.into()
    } else {
        suffix.into()
    }
}

fn latest_world_event_is_threat(soul: &Soul) -> bool {
    soul.world
        .recent_events
        .last()
        .map(|event| is_threat_or_emergency(event))
        .unwrap_or(false)
}

fn is_threat_or_emergency(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "threat",
        "trauma",
        "injury",
        "emergency",
        "armed raid",
        "raid",
        "restraint",
        "restrained",
        "explosion",
        "evacuation",
        "fear",
        "danger",
        "gun",
        "warrant",
        "police",
        "federal",
        "hazard",
        "blood",
        "shot",
        "shooter",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn explicitly_intimate(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "consensual",
        "intimate",
        "kiss",
        "romantic",
        "erotic",
        "aroused",
        "desire",
        "make love",
        "touch her gently",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn should_replace_default_plot(
    soul: &Soul,
    world_patch: &state_engine::patch::WorldPatch,
    turn_text: &str,
) -> bool {
    let has_default_plot = soul.world.active_plots.iter().any(|plot| {
        plot.to_ascii_lowercase()
            .contains("establish the first scene")
    });
    has_default_plot
        && (world_patch
            .recent_event
            .as_deref()
            .map(is_major_plot_shift)
            .unwrap_or(false)
            || is_major_plot_shift(turn_text))
}

fn is_major_plot_shift(text: &str) -> bool {
    infer_active_plot(text).is_some()
}

fn infer_active_plot(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if [
        "forced entry",
        "cop",
        "police",
        "warrant",
        "hazard suit",
        "hazmat",
        "raid",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("Forced-entry police operation at Aurora's apartment")
    } else if [
        "explosion",
        "federal",
        "evacuation",
        "multi-agency",
        "agency",
        "evacuate",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("Emergency evacuation during multi-agency crisis")
    } else if ["confession", "romantic", "intimacy", "kiss", "late-night"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Some("Late-night intimacy and emotional negotiation")
    } else {
        None
    }
}

fn user_text_has_explicit_time(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    let time_words = [
        "minute",
        "minutes",
        "hour",
        "hours",
        "day",
        "days",
        "week",
        "weeks",
        "month",
        "months",
        "year",
        "years",
        "tonight",
        "tomorrow",
        "yesterday",
        "morning",
        "afternoon",
        "evening",
        "midnight",
        "noon",
        "wait",
        "later",
    ];
    lower.chars().any(|ch| ch.is_ascii_digit())
        || time_words.iter().any(|word| lower.contains(word))
}

fn build_llm_payload_preview(
    soul: &Soul,
    messages: &[ContextMessage],
    user_text: &str,
    mode: &str,
    settings: &ApiProviderSettings,
    provider: &str,
    context_mode: ContextMode,
) -> LlmPayloadPreview {
    let context_preview = if user_text.trim().is_empty() {
        compile_context_for_messages(soul, messages)
    } else {
        compile_context_for_separate_user_message(soul, messages)
    };
    let prepared = prepare_narrator_payload(
        settings,
        soul,
        messages,
        &context_preview,
        user_text,
        mode,
        context_mode,
    );
    let system_message = prepared
        .messages
        .first()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let user_message = user_text.trim().to_string();
    let system_tokens = estimate_tokens(&system_message);
    let context_tokens = estimate_tokens(&prepared.context_text);
    let user_tokens = estimate_tokens(&user_message);

    LlmPayloadPreview {
        provider: provider.trim().to_string(),
        mode: mode.trim().to_string(),
        context_mode: context_mode.label().into(),
        model: settings.model.trim().to_string(),
        base_url: settings.base_url.trim().to_string(),
        system_message,
        user_message,
        context: prepared.context_text,
        messages: prepared.messages,
        truncated: prepared.truncated,
        estimated_tokens: LlmPayloadTokenEstimate {
            system: system_tokens,
            context: context_tokens,
            user: user_tokens,
            total: system_tokens + user_tokens + context_tokens,
        },
    }
}

fn prepare_narrator_payload(
    settings: &ApiProviderSettings,
    soul: &Soul,
    messages: &[ContextMessage],
    context_preview: &ContextPreview,
    user_text: &str,
    mode: &str,
    context_mode: ContextMode,
) -> PreparedApiPayload {
    match context_mode {
        ContextMode::Brief => {
            let system_message =
                build_narrator_system_prompt(settings, soul, &context_preview.text, mode, false);
            PreparedApiPayload {
                messages: vec![
                    ApiMessage::system(system_message),
                    ApiMessage::user(user_text.trim().to_string()),
                ],
                context_text: context_preview.text.clone(),
                user_message: user_text.trim().to_string(),
                truncated: context_preview.truncated,
            }
        }
        ContextMode::FullChat => {
            prepare_full_chat_payload(settings, soul, messages, user_text, mode)
        }
    }
}

fn prepare_full_chat_payload(
    settings: &ApiProviderSettings,
    soul: &Soul,
    messages: &[ContextMessage],
    user_text: &str,
    mode: &str,
) -> PreparedApiPayload {
    let system_message =
        build_narrator_system_prompt(settings, soul, &full_chat_setup(soul), mode, false);
    let mut api_messages = vec![ApiMessage::system(system_message)];
    for message in messages {
        match message.role.as_str() {
            "assistant" => api_messages.push(ApiMessage::assistant(sanitize_visible_chat_content(
                &message.content,
            ))),
            "user" => api_messages.push(ApiMessage::user(message.content.trim().to_string())),
            _ => {}
        }
    }
    if user_text.trim().len() > 0
        && !api_messages
            .last()
            .map(|message| message.role == "user" && message.content.trim() == user_text.trim())
            .unwrap_or(false)
    {
        api_messages.push(ApiMessage::user(user_text.trim().to_string()));
    }

    let mut truncated = false;
    while estimate_tokens(&serialize_api_messages(&api_messages)) > FULL_CHAT_TOKEN_BUDGET
        && api_messages.len() > 3
    {
        api_messages.remove(1);
        truncated = true;
    }
    let context_text = format!(
        "Context mode: full_chat\nTruncated: {}\n\n{}",
        truncated,
        serialize_api_messages(&api_messages)
    );
    PreparedApiPayload {
        user_message: user_text.trim().to_string(),
        messages: api_messages,
        context_text,
        truncated,
    }
}

fn full_chat_setup(soul: &Soul) -> String {
    format!(
        "[CHARACTER SETUP]\nName: {}\nDescription: {}\nAppearance: {}\nPersonality: {}\nScenario: {}\nWorld location: {}\nWorld time: {}",
        soul.character_name,
        empty_as_unspecified(&soul.profile.description),
        empty_as_unspecified(&soul.profile.appearance),
        empty_as_unspecified(&soul.profile.personality),
        empty_as_unspecified(&soul.profile.scenario),
        empty_as_unspecified(&soul.world.location),
        empty_as_unspecified(&soul.world.time_elapsed)
    )
}

fn empty_as_unspecified(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() {
        "Unspecified"
    } else {
        value
    }
}

fn sanitize_visible_chat_content(content: &str) -> String {
    strip_status_blocks_for_export(&strip_hidden_state_blocks(content))
        .trim()
        .to_string()
}

fn serialize_api_messages(messages: &[ApiMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n")
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
        lines.push(format!("Context mode: {}", log.context_mode));
        lines.push(format!("Base URL: {}", log.base_url));
        lines.push(format!("Truncated: {}", log.truncated));
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

fn strip_status_blocks_for_export(content: &str) -> String {
    let mut cleaned = String::new();
    let mut in_status = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("```status") {
            in_status = true;
            continue;
        }
        if in_status {
            if trimmed == "```" {
                in_status = false;
            }
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
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
        narrator_response_saved: false,
        assistant_message_id: None,
        selected_variant_id: None,
        state_updater_status: "legacy_hidden_state".into(),
        tag: hidden_state.tag.clone(),
        trust_delta: hidden_state.trust_delta,
        affection_delta: hidden_state.affection_delta,
        new_location: hidden_state.new_location.clone(),
        present_characters: hidden_state.present_characters.clone().unwrap_or_default(),
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
            ContextMode::Brief,
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

        let preview = build_llm_payload_preview(
            &soul,
            &[],
            "Current user turn",
            "Reader",
            &settings,
            "API",
            ContextMode::Brief,
        );

        assert!(preview.estimated_tokens.system > 0);
        assert!(preview.estimated_tokens.context > 0);
        assert!(preview.estimated_tokens.user > 0);
        assert!(preview.estimated_tokens.total > 0);
    }

    #[test]
    fn brief_context_mode_compiles_existing_sections() {
        let soul = new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "secret".into(),
            model: "debug-model".into(),
            system_prompt: String::new(),
        };
        let preview = build_llm_payload_preview(
            &soul,
            &[],
            "Current user turn",
            "Reader",
            &settings,
            "API",
            ContextMode::Brief,
        );

        assert_eq!(preview.context_mode, "brief");
        assert!(preview.context.contains("[WORLD SNAPSHOT]"));
        assert!(preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
    }

    #[test]
    fn full_chat_mode_sends_visible_history_instead_of_brief_sections() {
        let soul = new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "secret".into(),
            model: "debug-model".into(),
            system_prompt: String::new(),
        };
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "Hello.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Visible text.\n```status\nAurora | Skin: calm | Zones: room | Atmosphere: still\n```\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]".into(),
            },
        ];

        let preview = build_llm_payload_preview(
            &soul,
            &messages,
            "Current user turn",
            "Reader",
            &settings,
            "API",
            ContextMode::FullChat,
        );

        assert_eq!(preview.context_mode, "full_chat");
        assert!(!preview.context.contains("[WORLD SNAPSHOT]"));
        assert!(!preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
        assert!(preview.context.contains("user: Hello."));
        assert!(preview.context.contains("assistant: Visible text."));
        assert!(!preview.context.contains("[HIDDEN STATE]"));
        assert!(!preview.messages[2].content.contains("```status"));
        assert_eq!(preview.messages[1].role, "user");
        assert_eq!(preview.messages[2].role, "assistant");
    }

    #[test]
    fn full_chat_mode_trims_oldest_messages_when_over_budget() {
        let soul = new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "secret".into(),
            model: "debug-model".into(),
            system_prompt: String::new(),
        };
        let huge = "old ".repeat(8_000);
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: huge,
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Latest narrator tail.".into(),
            },
        ];

        let preview = build_llm_payload_preview(
            &soul,
            &messages,
            "Current user turn",
            "Reader",
            &settings,
            "API",
            ContextMode::FullChat,
        );

        assert!(preview.truncated);
        assert!(preview.context.contains("Latest narrator tail."));
        assert!(preview.context.contains("Current user turn"));
    }

    #[test]
    fn state_updater_patch_applies_through_engine_validation() {
        let mut soul = new_default_soul("Aurora");
        let raw = r#"{"schema_version":1,"soul_patch":{"relationship_delta":{"target":"user","trust":2.0},"new_memories":[{"content":"Aurora noticed the user's steady answer.","tag":"observation"}]},"world_patch":{"recent_event":"Aurora challenged the user and waited for an answer."}}"#;

        let patch = parse_engine_patch_json(raw).expect("valid patch");
        let report = patch.apply_to_soul(&mut soul).expect("engine validation");

        assert!(report.relationship_updated);
        assert_eq!(report.memories_added, 1);
        assert!(report.world_updated);
        assert_eq!(soul.relationships["user"].trust, 12.0);
    }

    #[test]
    fn unsupported_state_updater_time_jump_is_ignored() {
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"world_patch":{"time_elapsed":"Three days later","recent_event":"Aurora spoke."}}"#,
        )
        .expect("valid patch");

        let soul = new_default_soul("Aurora");
        let filtered =
            sanitize_state_updater_patch(patch, &soul, "I tell her the truth.", "Aurora spoke.");

        assert_eq!(
            filtered
                .world_patch
                .as_ref()
                .and_then(|patch| patch.time_elapsed.as_deref()),
            None
        );
        assert_eq!(
            filtered
                .world_patch
                .as_ref()
                .and_then(|patch| patch.recent_event.as_deref()),
            Some("Aurora spoke.")
        );
    }

    #[test]
    fn explicit_user_time_update_is_accepted() {
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"world_patch":{"time_elapsed":"Ten minutes later"}}"#,
        )
        .expect("valid patch");

        let soul = new_default_soul("Aurora");
        let filtered =
            sanitize_state_updater_patch(patch, &soul, "I wait ten minutes.", "Aurora waits.");

        assert_eq!(
            filtered
                .world_patch
                .as_ref()
                .and_then(|patch| patch.time_elapsed.as_deref()),
            Some("Ten minutes later")
        );
    }

    #[test]
    fn state_updater_payload_is_compact_and_excludes_compiled_context() {
        let mut soul = new_default_soul("Aurora");
        soul.world.location = "Apartment hallway".into();
        soul.world.time_elapsed = "Session startLate evening, just after midnight.".into();
        soul.world.active_plots = vec!["Establish the first scene".into()];
        soul.world.recent_events = vec![
            "Old unrelated cohabitation discussion from another session.".into(),
            "Forced entry began at Aurora's apartment door.".into(),
        ];
        let payload = build_compact_updater_payload_for_test(
            &soul,
            "Police force the door with a warrant.",
            "Aurora backs away from the forced entry.",
        );

        assert!(payload.contains("[CURRENT STATE]"));
        assert!(payload.contains("[LATEST USER MESSAGE]"));
        assert!(payload.contains("[NARRATOR RESPONSE]"));
        assert!(payload.contains("Patch schema"));
        assert!(!payload.contains("[COMPILED CONTEXT]"));
        assert!(!payload.contains("[WORLD SNAPSHOT]"));
        assert!(!payload.contains("Old unrelated cohabitation"));
        assert!(estimate_tokens(&payload) < 1_200);
        assert!(payload.contains("Time: Late evening, just after midnight."));
    }

    #[test]
    fn threat_emergency_scene_suppresses_arousal_increase() {
        let soul = new_default_soul("Aurora");
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"body_patch":{"activation_delta":25.0,"peak_allowed":true}}"#,
        )
        .expect("valid patch");

        let filtered = sanitize_state_updater_patch(
            patch,
            &soul,
            "An armed raid hits the apartment.",
            "Aurora is restrained while an explosion shakes the hallway.",
        );

        let body = filtered.body_patch.expect("body patch remains");
        assert_eq!(body.activation_delta, Some(0.0));
        assert_eq!(body.peak_allowed, Some(false));
    }

    #[test]
    fn explicit_non_threat_intimacy_allows_arousal_update() {
        let soul = new_default_soul("Aurora");
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"body_patch":{"activation_delta":12.0}}"#,
        )
        .expect("valid patch");

        let filtered = sanitize_state_updater_patch(
            patch,
            &soul,
            "In a consensual intimate moment, I kiss her gently.",
            "Aurora leans into the kiss.",
        );

        assert_eq!(
            filtered
                .body_patch
                .as_ref()
                .and_then(|body| body.activation_delta),
            Some(12.0)
        );
    }

    #[test]
    fn active_plot_replaces_default_after_major_shift() {
        let mut soul = new_default_soul("Aurora");
        soul.world.active_plots = vec!["Establish the first scene".into()];
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"world_patch":{"recent_event":"Police forced entry with a warrant."}}"#,
        )
        .expect("valid patch");

        let filtered = sanitize_state_updater_patch(
            patch,
            &soul,
            "Police force the door with a warrant.",
            "Aurora retreats from the raid.",
        );

        let world = filtered.world_patch.expect("world patch");
        assert!(world
            .active_plot_add
            .contains(&"Forced-entry police operation at Aurora's apartment".into()));
        assert!(world
            .active_plot_resolve
            .iter()
            .any(|plot| plot.contains("Establish the first scene")));
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
    fn narrator_response_is_persisted_before_state_updater_result() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        db::upsert_soul(&conn, &soul).expect("upsert soul");
        db::ensure_conversation(&conn, "dual-pass", &soul.character_id).expect("conversation");
        db::insert_message_and_get_id(&conn, "dual-pass", "user", "The siren starts.")
            .expect("user message");
        let payload_log_id = db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                id: 0,
                conversation_id: "dual-pass".into(),
                message_id: None,
                provider: "narrator_brief".into(),
                mode: "Reader".into(),
                context_mode: "brief".into(),
                model: "narrator-model".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "Narrator system".into(),
                user_message: "The siren starts.".into(),
                context_text: "Brief context".into(),
                estimated_system_tokens: 3,
                estimated_user_tokens: 3,
                estimated_total_tokens: 6,
                truncated: false,
                created_at: 100,
            },
        )
        .expect("payload log");

        let (assistant_message_id, selected_variant_id) = save_visible_narrator_response(
            &conn,
            "dual-pass",
            "Aurora snaps toward the window as the siren climbs.",
            None,
            None,
            &serde_json::to_string(&soul).expect("soul json"),
            "The siren starts.",
            payload_log_id,
        )
        .expect("save narrator");
        assert!(parse_engine_patch_json("not json").is_err());

        let messages = db::list_messages(&conn, "dual-pass", 100).expect("messages");
        let assistant = messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .expect("assistant persisted");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(
            assistant.content,
            "Aurora snaps toward the window as the siren climbs."
        );
        let exported = render_visible_chat_log(&messages);
        assert!(exported.contains("## Narrator"));
        assert!(exported.contains("Aurora snaps toward the window"));

        let variants =
            db::list_assistant_message_variants(&conn, "dual-pass", assistant_message_id)
                .expect("variants");
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
                .find(|variant| variant.is_selected)
                .unwrap()
                .id,
            selected_variant_id
        );

        let logs = db::list_llm_payload_logs(&conn, "dual-pass").expect("logs");
        assert_eq!(logs[0].message_id, Some(assistant_message_id));
    }

    #[test]
    fn dev_log_details_redact_secrets_but_keep_token_estimates() {
        let details = serde_json::json!({
            "api_key": "secret-key",
            "authorization": "Bearer secret-token",
            "estimated_total_tokens": 123,
            "nested": {
                "refresh_token": "hidden",
                "model": "safe-model"
            }
        });

        let redacted = redact_dev_log_details(details);
        let serialized = redacted.to_string();

        assert!(!serialized.contains("secret-key"));
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains("hidden"));
        assert!(serialized.contains("estimated_total_tokens"));
        assert!(serialized.contains("123"));
        assert!(serialized.contains("safe-model"));
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
                context_mode: "brief".into(),
                model: "model-a".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System A with clothing context".into(),
                user_message: "User A".into(),
                context_text: "Context A".into(),
                estimated_system_tokens: 10,
                estimated_user_tokens: 2,
                estimated_total_tokens: 12,
                truncated: false,
                created_at: 100,
            },
            LlmPayloadLog {
                id: 2,
                conversation_id: "history".into(),
                message_id: Some(11),
                provider: "API".into(),
                mode: "God".into(),
                context_mode: "brief".into(),
                model: "model-b".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System B".into(),
                user_message: "User B".into(),
                context_text: "Context B".into(),
                estimated_system_tokens: 11,
                estimated_user_tokens: 3,
                estimated_total_tokens: 14,
                truncated: false,
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
    fn payload_history_labels_narrator_and_state_updater_sources() {
        let logs = vec![
            LlmPayloadLog {
                id: 1,
                conversation_id: "history".into(),
                message_id: Some(10),
                provider: "narrator_brief".into(),
                mode: "Reader".into(),
                context_mode: "brief".into(),
                model: "model".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "Narrator system".into(),
                user_message: "User".into(),
                context_text: "Context".into(),
                estimated_system_tokens: 1,
                estimated_user_tokens: 1,
                estimated_total_tokens: 2,
                truncated: false,
                created_at: 100,
            },
            LlmPayloadLog {
                id: 2,
                conversation_id: "history".into(),
                message_id: Some(10),
                provider: "state_updater".into(),
                mode: "state_updater".into(),
                context_mode: "brief".into(),
                model: "model".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "Updater system".into(),
                user_message: "Latest turn".into(),
                context_text: "Context".into(),
                estimated_system_tokens: 1,
                estimated_user_tokens: 1,
                estimated_total_tokens: 2,
                truncated: false,
                created_at: 101,
            },
        ];

        let exported = render_llm_payload_history(&logs);

        assert!(exported.contains("Provider: narrator_brief"));
        assert!(exported.contains("Provider: state_updater"));
        assert!(exported.contains("Context mode: brief"));
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
