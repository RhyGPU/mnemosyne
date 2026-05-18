use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State, Window};

use state_engine::{
    consolidation::consolidate_soul,
    context_compiler::{
        compile_context_for_messages, compile_context_for_separate_user_message, estimate_tokens,
        ContextMessage, ContextPreview,
    },
    hidden_state::{parse_hidden_state, HiddenState},
    patch::{EnginePatch, MemoryApplyAction},
    setting::{new_default_setting, SettingSoul},
    soul::{
        new_default_soul, session_soul_from_savepoint, soul_savepoint_from_session,
        MemorySourceType, Soul,
    },
};

use crate::{
    db::{
        self, AssistantMessageVariant, ChatMessage, ConversationSummary, EntityRecord, ImageAsset,
        LlmPayloadLog, ProviderProfile, SettingSummary, SoulSummary,
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
const NARRATOR_BRIEF_TARGET_TOKENS: usize = 2_500;
const STATE_UPDATER_TARGET_TOKENS: usize = 1_200;
const STATE_UPDATER_TIMEOUT_SECONDS: u64 = 12;
static DEV_LOG_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy)]
struct ImageFileInfo {
    extension: &'static str,
    mime_type: &'static str,
    width: Option<i64>,
    height: Option<i64>,
}

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
    pub replay_detected: bool,
    pub replay_score: f32,
    pub replay_reason: Option<String>,
    pub replay_compared_against_message_id: Option<i64>,
    pub output_contract_warning: Option<String>,
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

#[derive(Debug, serde::Serialize)]
pub struct SessionStartResult {
    pub soul: Soul,
    pub conversation: ConversationSummary,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone)]
struct EntityTurnContext {
    entities: Vec<EntityRecord>,
    speaker: SpeakerResolution,
}

#[derive(Debug, Clone)]
struct SpeakerResolution {
    label: Option<String>,
    entity_id: String,
    display_name: String,
    status: SpeakerResolutionStatus,
    candidates: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeakerResolutionStatus {
    NoLabel,
    Exact,
    FuzzyTypo,
    Ambiguous,
    Created,
    Unknown,
}

#[derive(Debug, Clone)]
struct ReplaySource {
    message_id: i64,
    content: String,
}

#[derive(Debug, Clone, Default)]
struct ReplayGuardResult {
    replay_detected: bool,
    replay_score: f32,
    replay_reason: Option<String>,
    compared_against_message_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct OutputContractResult {
    text: String,
    warning: Option<String>,
}

#[tauri::command]
pub fn create_default_soul(character_name: String) -> Soul {
    new_default_soul(&character_name)
}

#[tauri::command]
pub fn create_fresh_scenario_soul(
    state: State<'_, AppState>,
    soul_id: String,
    _setting_id: Option<String>,
) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let base = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let fresh = session_soul_from_savepoint(&base);
    db::upsert_soul(&conn, &fresh).map_err(|err| err.to_string())?;
    Ok(fresh)
}

#[tauri::command]
pub fn create_session_soul_from_savepoint(
    state: State<'_, AppState>,
    source_soul_id: String,
    setting_id: Option<String>,
    title: Option<String>,
) -> Result<SessionStartResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let source = db::get_soul(&conn, &source_soul_id).map_err(|err| err.to_string())?;
    let session = session_soul_from_savepoint(&source);
    db::upsert_soul(&conn, &session).map_err(|err| err.to_string())?;
    let conversation_id = conversation_id_for_session(setting_id.as_deref(), &session.character_id);
    let default_title = format!("{} Session", source.character_name.trim());
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or(&default_title);
    db::ensure_conversation_with_title(&conn, &conversation_id, &session.character_id, Some(title))
        .map_err(|err| err.to_string())?;
    let opening = session.profile.opening_narrator_message.trim();
    seed_opening_narrator_message(&conn, &conversation_id, opening)
        .map_err(|err| err.to_string())?;
    let conversation =
        db::get_conversation_summary(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    Ok(SessionStartResult {
        soul: session,
        conversation,
        messages,
    })
}

fn seed_opening_narrator_message(
    conn: &Connection,
    conversation_id: &str,
    opening: &str,
) -> rusqlite::Result<Option<i64>> {
    let opening = opening.trim();
    if opening.is_empty() || !db::list_messages(conn, conversation_id, 1)?.is_empty() {
        return Ok(None);
    }
    let message_id = db::insert_message_and_get_id(conn, conversation_id, "assistant", opening)?;
    db::create_assistant_message_variant(
        conn,
        conversation_id,
        message_id,
        opening,
        Some("opening"),
        Some("opening_seed"),
        true,
        None,
        None,
    )?;
    Ok(Some(message_id))
}

fn conversation_id_for_session(setting_id: Option<&str>, session_soul_id: &str) -> String {
    match setting_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(setting_id) => format!("local-mock-{setting_id}-{session_soul_id}"),
        None => format!("local-mock-{session_soul_id}"),
    }
}

#[tauri::command]
pub fn save_session_as_new_soul(
    state: State<'_, AppState>,
    session_soul_id: String,
    name: String,
    soul_kind: Option<String>,
) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let session = db::get_soul(&conn, &session_soul_id).map_err(|err| err.to_string())?;
    let kind = soul_kind.as_deref().unwrap_or("checkpoint");
    let name = name.trim();
    let name = if name.is_empty() {
        format!("{} Checkpoint", session.character_name)
    } else {
        name.to_string()
    };
    let savepoint = soul_savepoint_from_session(&session, &name, kind);
    db::upsert_soul(&conn, &savepoint).map_err(|err| err.to_string())?;
    Ok(savepoint)
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
pub fn save_soul_file(app: AppHandle, path: String, soul: Soul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&soul).map_err(|err| err.to_string())?;
    let path = resolve_export_path(&app, &path, "soul.json")?;
    fs::write(path, content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_setting_file(app: AppHandle, path: String, setting: SettingSoul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&setting).map_err(|err| err.to_string())?;
    let path = resolve_export_path(&app, &path, "setting.json")?;
    fs::write(path, content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls_debug(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls_including_session_clones(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Result<Vec<ConversationSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_conversations(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn rename_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
    title: String,
) -> Result<ConversationSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::rename_conversation(&conn, &conversation_id, &title).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn import_image_asset(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    path: String,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: Option<String>,
) -> Result<ImageAsset, String> {
    let source = normalize_image_source(source.as_deref())?;
    let source_path = PathBuf::from(path);
    let source_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "source": source, "file": source_name })),
    );

    let result = import_image_asset_inner(
        &app,
        &state,
        &source_path,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        &source,
    );
    match &result {
        Ok(asset) => emit_dev_log(
            &window,
            "success",
            "db",
            "image_import_success",
            Some(serde_json::json!({
                "image_asset_id": asset.id,
                "source": asset.source,
                "stored_file": Path::new(&asset.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image")
            })),
        ),
        Err(err) => emit_dev_log(
            &window,
            "error",
            "db",
            "image_import_failed",
            Some(serde_json::json!({ "file": source_name, "error": err })),
        ),
    }
    result
}

fn import_image_asset_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    source_path: &Path,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: &str,
) -> Result<ImageAsset, String> {
    let bytes = fs::read(source_path).map_err(|err| format!("Image read failed: {err}"))?;
    import_image_asset_bytes_inner(
        app,
        state,
        &bytes,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        source,
    )
}

fn import_image_asset_bytes_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    bytes: &[u8],
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: &str,
) -> Result<ImageAsset, String> {
    let info = inspect_image_bytes(bytes)?;
    let mut images_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    images_dir.push("images");
    fs::create_dir_all(images_dir.join("thumbnails")).map_err(|err| err.to_string())?;
    let id = uuid_like_id();
    let file_name = format!("{id}.{}", info.extension);
    let target_path = images_dir.join(file_name);
    fs::write(&target_path, bytes).map_err(|err| format!("Image copy failed: {err}"))?;

    let asset = ImageAsset {
        id,
        file_path: target_path.display().to_string(),
        thumbnail_path: None,
        source: source.to_string(),
        mime_type: Some(info.mime_type.to_string()),
        width: info.width,
        height: info.height,
        prompt: None,
        provider: None,
        model: None,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        created_at: db::now_ts(),
    };
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let saved = db::upsert_image_asset(&conn, &asset).map_err(|err| err.to_string())?;
    if let Some(message_id) = saved.linked_message_id {
        db::attach_image_to_message(&conn, message_id, &saved.id).map_err(|err| err.to_string())?;
    }
    Ok(saved)
}

#[tauri::command]
pub fn import_image_asset_bytes(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    file_name: String,
    data_base64: String,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: Option<String>,
) -> Result<ImageAsset, String> {
    let source = normalize_image_source(source.as_deref())?;
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "source": source, "file": safe_image_log_name(&file_name) })),
    );
    let decoded = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("Image decode failed: {err}"));
    let result = decoded.and_then(|bytes| {
        import_image_asset_bytes_inner(
            &app,
            &state,
            &bytes,
            linked_soul_id,
            linked_conversation_id,
            linked_message_id,
            &source,
        )
    });
    match &result {
        Ok(asset) => emit_dev_log(
            &window,
            "success",
            "db",
            "image_import_success",
            Some(serde_json::json!({
                "image_asset_id": asset.id,
                "source": asset.source,
                "stored_file": Path::new(&asset.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image")
            })),
        ),
        Err(err) => emit_dev_log(
            &window,
            "error",
            "db",
            "image_import_failed",
            Some(serde_json::json!({ "file": safe_image_log_name(&file_name), "error": err })),
        ),
    }
    result
}

#[tauri::command]
pub fn create_user_image_message(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    path: String,
    content: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "conversation_id": conversation_id })),
    );
    let message_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let content = content
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("[Image]");
        db::insert_message_and_get_id(&conn, &conversation_id, "user", content)
            .map_err(|err| err.to_string())?
    };
    let asset = import_image_asset_inner(
        &app,
        &state,
        &PathBuf::from(path),
        None,
        Some(conversation_id.clone()),
        Some(message_id),
        "uploaded",
    );
    match asset {
        Ok(asset) => {
            emit_dev_log(
                &window,
                "success",
                "db",
                "chat_image_attached",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "message_id": message_id,
                    "image_asset_id": asset.id
                })),
            );
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
        }
        Err(err) => {
            let conn = state.conn.lock().map_err(|lock_err| lock_err.to_string())?;
            let _ = db::delete_message(&conn, &conversation_id, message_id);
            emit_dev_log(
                &window,
                "error",
                "db",
                "image_import_failed",
                Some(serde_json::json!({ "conversation_id": conversation_id, "error": err })),
            );
            Err(err)
        }
    }
}

#[tauri::command]
pub fn create_user_image_message_bytes(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    data_base64: String,
    content: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "file": safe_image_log_name(&file_name)
        })),
    );
    let message_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let content = content
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("[Image]");
        db::insert_message_and_get_id(&conn, &conversation_id, "user", content)
            .map_err(|err| err.to_string())?
    };
    let result = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("Image decode failed: {err}"))
        .and_then(|bytes| {
            import_image_asset_bytes_inner(
                &app,
                &state,
                &bytes,
                None,
                Some(conversation_id.clone()),
                Some(message_id),
                "uploaded",
            )
        });
    match result {
        Ok(asset) => {
            emit_dev_log(
                &window,
                "success",
                "db",
                "chat_image_attached",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "message_id": message_id,
                    "image_asset_id": asset.id
                })),
            );
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
        }
        Err(err) => {
            let conn = state.conn.lock().map_err(|lock_err| lock_err.to_string())?;
            let _ = db::delete_message(&conn, &conversation_id, message_id);
            emit_dev_log(
                &window,
                "error",
                "db",
                "image_import_failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "file": safe_image_log_name(&file_name),
                    "error": err
                })),
            );
            Err(err)
        }
    }
}

#[tauri::command]
pub fn get_image_asset(
    state: State<'_, AppState>,
    image_asset_id: String,
) -> Result<ImageAsset, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_image_asset(&conn, &image_asset_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_image_asset_data_url(
    state: State<'_, AppState>,
    image_asset_id: String,
) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let asset = db::get_image_asset(&conn, &image_asset_id).map_err(|err| err.to_string())?;
    let bytes = fs::read(&asset.file_path).map_err(|err| format!("Image read failed: {err}"))?;
    let info = inspect_image_bytes(&bytes)?;
    let mime_type = asset.mime_type.as_deref().unwrap_or(info.mime_type);
    Ok(format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
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
    let (visible_response, replay_guard, output_contract_warning) =
        guard_narrator_visible_response(&parsed.visible_text, &snapshot_user_text, &[]);
    let mut debug = debug_from_hidden_state("Mock", &parsed.hidden_state, true, false);
    debug.replay_detected = replay_guard.replay_detected;
    debug.replay_score = replay_guard.replay_score;
    debug.replay_reason = replay_guard.replay_reason;
    debug.replay_compared_against_message_id = replay_guard.compared_against_message_id;
    debug.output_contract_warning = output_contract_warning;
    let debug_json = serde_json::to_string(&debug).map_err(|err| err.to_string())?;

    parsed.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    let assistant_message_id = if let Some(message_id) = replacement_assistant_id {
        message_id
    } else {
        db::insert_message_and_get_id(&conn, &conversation_id, "assistant", &visible_response)
            .map_err(|err| err.to_string())?
    };

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
    let turn_started = Instant::now();
    let mut stage_started = Instant::now();
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
    let (
        mut soul,
        context_messages,
        context_preview,
        snapshot_user_text,
        pre_turn_soul_json,
        entity_context,
        replay_sources,
    ) = {
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
            emit_perf_log(
                &window,
                &conversation_id,
                "save user message",
                stage_started.elapsed(),
            );
            stage_started = Instant::now();
        }
        let entity_context =
            resolve_speaker_for_turn(&conn, &conversation_id, &soul, &snapshot_user_text)
                .map_err(|err| err.to_string())?;

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
        let mut replay_sources = recent_assistant_replay_sources(&before_messages, 2);
        if let Some(message_id) = replacement_assistant_id {
            if let Ok(message) = db::get_message(&conn, &conversation_id, message_id) {
                if message.role == "assistant" {
                    replay_sources.insert(
                        0,
                        ReplaySource {
                            message_id: message.id,
                            content: message.content,
                        },
                    );
                    replay_sources.truncate(2);
                }
            }
        }
        let context_messages = messages_to_context(before_messages);
        let context_preview = compile_context_with_correction(
            &soul,
            &context_messages,
            correction_instruction.as_deref(),
        );
        emit_perf_log(
            &window,
            &conversation_id,
            "compile narrator context",
            stage_started.elapsed(),
        );
        if context_mode == ContextMode::Brief
            && context_preview.estimated_tokens > NARRATOR_BRIEF_TARGET_TOKENS
        {
            emit_dev_log(
                &window,
                "warn",
                "performance",
                "narrator payload exceeds brief budget",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "estimated_tokens": context_preview.estimated_tokens,
                    "target_tokens": NARRATOR_BRIEF_TARGET_TOKENS
                })),
            );
        }
        let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
        (
            soul,
            context_messages,
            context_preview,
            snapshot_user_text,
            pre_turn_soul_json,
            entity_context,
            replay_sources,
        )
    };
    emit_entity_resolution_log(&window, &conversation_id, &entity_context.speaker);
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
    let mut payload_log_id = {
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
    let narrator_call_started = Instant::now();
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
            emit_perf_log(
                &window,
                &conversation_id,
                "narrator API call",
                narrator_call_started.elapsed(),
            );
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
    emit_dev_log(
        &window,
        "debug",
        "narrator",
        "anti_replay_check_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "compared_sources": replay_sources.len(),
            "compared_message_ids": replay_sources
                .iter()
                .map(|source| source.message_id)
                .collect::<Vec<_>>()
        })),
    );
    let (mut visible_response, replay_guard, mut output_contract_warning) =
        guard_narrator_visible_response(&parsed.visible_text, &snapshot_user_text, &replay_sources);
    let debug_replay_detected = replay_guard.replay_detected;
    let mut debug_replay_score = replay_guard.replay_score;
    let mut debug_replay_reason = replay_guard.replay_reason.clone();
    let mut debug_replay_compared_against_message_id = replay_guard.compared_against_message_id;

    if let Some(warning) = output_contract_warning.as_ref() {
        emit_dev_log(
            &window,
            "warn",
            "narrator",
            "Output contract guard normalized narrator response",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "warning": warning
            })),
        );
    }

    if replay_guard.replay_detected {
        emit_dev_log(
            &window,
            "warn",
            "narrator",
            "anti_replay_detected",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "score": replay_guard.replay_score,
                "reason": replay_guard.replay_reason.as_deref(),
                "compared_against_message_id": replay_guard.compared_against_message_id
            })),
        );
        emit_dev_log(
            &window,
            "info",
            "narrator",
            "anti_replay_regenerate_started",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str()
            })),
        );

        let retry_messages = messages_with_repair_instruction(&narrator_payload.messages);
        let retry_payload_log_id =
            match state
                .conn
                .lock()
                .map_err(|err| err.to_string())
                .and_then(|conn| {
                    db::insert_llm_payload_log(
                        &conn,
                        &LlmPayloadLog {
                            id: 0,
                            conversation_id: conversation_id.clone(),
                            message_id: replacement_assistant_id,
                            provider: format!(
                                "narrator_{}_anti_replay_retry",
                                context_mode.label()
                            ),
                            mode: mode.trim().to_string(),
                            context_mode: context_mode.label().into(),
                            model: narrator_settings.model.trim().to_string(),
                            base_url: narrator_settings.base_url.trim().to_string(),
                            system_message: retry_messages
                                .first()
                                .map(|message| message.content.clone())
                                .unwrap_or_default(),
                            user_message: last_user_message_content(&retry_messages),
                            context_text: serialize_api_messages(&retry_messages),
                            estimated_system_tokens: estimate_tokens(
                                retry_messages
                                    .first()
                                    .map(|message| message.content.as_str())
                                    .unwrap_or_default(),
                            ),
                            estimated_user_tokens: estimate_tokens(&last_user_message_content(
                                &retry_messages,
                            )),
                            estimated_total_tokens: estimate_tokens(&serialize_api_messages(
                                &retry_messages,
                            )),
                            truncated: narrator_payload.truncated,
                            created_at: db::now_ts(),
                        },
                    )
                    .map_err(|err| err.to_string())
                }) {
                Ok(log_id) => Some(log_id),
                Err(err) => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "db",
                        "Anti-replay retry payload log failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "error": err
                        })),
                    );
                    None
                }
            };

        match provider
            .complete_streaming_messages(&narrator_settings, retry_messages, |_| Ok(()))
            .await
        {
            Ok(retry_raw_response) => match parse_hidden_state(&retry_raw_response) {
                Ok(retry_parsed) => {
                    let (retry_visible_response, retry_guard, retry_output_warning) =
                        guard_narrator_visible_response(
                            &retry_parsed.visible_text,
                            &snapshot_user_text,
                            &replay_sources,
                        );
                    if retry_visible_response.trim().is_empty() {
                        emit_dev_log(
                            &window,
                            "warn",
                            "narrator",
                            "anti_replay_regenerate_failed",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "reason": "Retry returned empty visible response"
                            })),
                        );
                        emit_dev_log(
                            &window,
                            "warn",
                            "warning",
                            "anti_replay_final_warning",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "score": replay_guard.replay_score,
                                "reason": "Original repeated earlier narration; retry returned empty response"
                            })),
                        );
                        debug_replay_reason = Some(
                            "Initial draft repeated earlier narration; retry returned empty response"
                                .into(),
                        );
                    } else {
                        visible_response = retry_visible_response;
                        if let Some(log_id) = retry_payload_log_id {
                            payload_log_id = log_id;
                        }
                        if let Some(warning) = retry_output_warning.as_ref() {
                            emit_dev_log(
                                &window,
                                "warn",
                                "narrator",
                                "Output contract guard normalized anti-replay retry",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "warning": warning
                                })),
                            );
                        }
                        output_contract_warning = retry_output_warning;
                        if retry_guard.replay_detected {
                            emit_dev_log(
                                &window,
                                "warn",
                                "narrator",
                                "anti_replay_regenerate_failed",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "score": retry_guard.replay_score,
                                    "reason": retry_guard.replay_reason.as_deref(),
                                    "compared_against_message_id": retry_guard.compared_against_message_id
                                })),
                            );
                            emit_dev_log(
                                &window,
                                "warn",
                                "warning",
                                "anti_replay_final_warning",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "score": retry_guard.replay_score,
                                    "reason": retry_guard.replay_reason.as_deref()
                                })),
                            );
                            debug_replay_score = retry_guard.replay_score;
                            debug_replay_reason = retry_guard
                                .replay_reason
                                .clone()
                                .map(|reason| format!("{reason}; saved after one retry"));
                            debug_replay_compared_against_message_id =
                                retry_guard.compared_against_message_id;
                        } else {
                            emit_dev_log(
                                &window,
                                "success",
                                "narrator",
                                "anti_replay_passed",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "score": retry_guard.replay_score,
                                    "retry": true
                                })),
                            );
                            debug_replay_reason = Some(
                                "Initial draft repeated earlier narration; regenerated before save"
                                    .into(),
                            );
                        }
                    }
                }
                Err(err) => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "narrator",
                        "anti_replay_regenerate_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "error": err.to_string()
                        })),
                    );
                    emit_dev_log(
                        &window,
                        "warn",
                        "warning",
                        "anti_replay_final_warning",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "score": replay_guard.replay_score,
                            "reason": "Retry parse failed; saving original guarded response"
                        })),
                    );
                    debug_replay_reason =
                        Some("Initial draft repeated earlier narration; retry parse failed".into());
                }
            },
            Err(err) => {
                emit_dev_log(
                    &window,
                    "warn",
                    "narrator",
                    "anti_replay_regenerate_failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "error": err
                    })),
                );
                emit_dev_log(
                    &window,
                    "warn",
                    "warning",
                    "anti_replay_final_warning",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "score": replay_guard.replay_score,
                        "reason": "Retry provider failed; saving original guarded response"
                    })),
                );
                debug_replay_reason =
                    Some("Initial draft repeated earlier narration; retry provider failed".into());
            }
        }
    } else {
        emit_dev_log(
            &window,
            "success",
            "narrator",
            "anti_replay_passed",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "score": replay_guard.replay_score,
                "compared_against_message_id": replay_guard.compared_against_message_id
            })),
        );
    }

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

    let save_narrator_started = Instant::now();
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
    emit_perf_log(
        &window,
        &conversation_id,
        "save narrator response",
        save_narrator_started.elapsed(),
    );
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

    let updater_payload_started = Instant::now();
    let updater_system_prompt = build_state_updater_prompt(&soul);
    let entity_updater_context = build_entity_updater_context(&soul, &entity_context);
    let updater_user_message = build_state_updater_user_message(
        &snapshot_user_text,
        &visible_response,
        Some(&entity_updater_context),
    );
    let updater_token_estimate =
        estimate_tokens(&updater_system_prompt) + estimate_tokens(&updater_user_message);
    emit_perf_log(
        &window,
        &conversation_id,
        "compile updater payload",
        updater_payload_started.elapsed(),
    );
    if updater_token_estimate > STATE_UPDATER_TARGET_TOKENS {
        emit_dev_log(
            &window,
            "warn",
            "performance",
            "state updater payload exceeds budget",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "estimated_tokens": updater_token_estimate,
                "target_tokens": STATE_UPDATER_TARGET_TOKENS
            })),
        );
    }
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
            "estimated_total_tokens": updater_token_estimate
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
                    estimated_total_tokens: updater_token_estimate,
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
    let updater_call_started = Instant::now();
    let updater_response_result = provider
        .complete_prompt_with_timeout(
            &state_updater_settings,
            &updater_system_prompt,
            &updater_user_message,
            0.0,
            Duration::from_secs(STATE_UPDATER_TIMEOUT_SECONDS),
        )
        .await;
    let updater_call_elapsed = updater_call_started.elapsed();
    if updater_call_elapsed >= Duration::from_secs(STATE_UPDATER_TIMEOUT_SECONDS) {
        emit_dev_log(
            &window,
            "warn",
            "state_updater",
            "State updater timed out; narration saved without state update",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "timeout_seconds": STATE_UPDATER_TIMEOUT_SECONDS,
                "elapsed_ms": updater_call_elapsed.as_millis()
            })),
        );
    }
    emit_perf_log(
        &window,
        &conversation_id,
        "state updater API call",
        updater_call_elapsed,
    );
    let parse_started = Instant::now();
    let updater_result = match updater_response_result {
        Ok(updater_response) => parse_engine_patch_json(&updater_response),
        Err(err) => {
            if updater_call_elapsed >= Duration::from_secs(STATE_UPDATER_TIMEOUT_SECONDS)
                || err.to_lowercase().contains("timed out")
            {
                Err(format!(
                    "State updater timed out after {}s; narration saved without state update",
                    STATE_UPDATER_TIMEOUT_SECONDS
                ))
            } else {
                Err(err)
            }
        }
    };
    emit_perf_log(
        &window,
        &conversation_id,
        "parse EnginePatch",
        parse_started.elapsed(),
    );
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
    debug.replay_detected = debug_replay_detected;
    debug.replay_score = debug_replay_score;
    debug.replay_reason = debug_replay_reason;
    debug.replay_compared_against_message_id = debug_replay_compared_against_message_id;
    debug.output_contract_warning = output_contract_warning;

    let apply_started = Instant::now();
    match engine_patch.apply_to_soul(&mut soul) {
        Ok(report) => {
            emit_perf_log(
                &window,
                &conversation_id,
                "apply EnginePatch",
                apply_started.elapsed(),
            );
            emit_perf_log(
                &window,
                &conversation_id,
                "memory hygiene",
                apply_started.elapsed(),
            );
            emit_dev_log(
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
            );
            emit_relationship_delta_logs(&window, &conversation_id, &engine_patch);
            emit_memory_apply_logs(&window, &conversation_id, &report.memory_events);
        }
        Err(err) => {
            emit_perf_log(
                &window,
                &conversation_id,
                "apply EnginePatch",
                apply_started.elapsed(),
            );
            emit_dev_log(
                &window,
                "error",
                "state_updater",
                "EnginePatch skipped by validation",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "assistant_message_id": assistant_message_id,
                    "error": format!("{err:?}")
                })),
            )
        }
    }
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;

    let refresh_started = Instant::now();
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
    emit_perf_log(
        &window,
        &conversation_id,
        "refresh frontend state",
        refresh_started.elapsed(),
    );
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
    emit_perf_log(
        &window,
        &conversation_id,
        "total turn time",
        turn_started.elapsed(),
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
        replay_detected: false,
        replay_score: 0.0,
        replay_reason: None,
        replay_compared_against_message_id: None,
        output_contract_warning: None,
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

fn emit_perf_log(window: &Window, conversation_id: &str, stage: &str, elapsed: Duration) {
    emit_dev_log(
        window,
        if elapsed.as_millis() > 2_000 {
            "info"
        } else {
            "debug"
        },
        "performance",
        &format!("{stage}: {} ms", elapsed.as_millis()),
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "stage": stage,
            "elapsed_ms": elapsed.as_millis()
        })),
    );
}

fn emit_entity_resolution_log(window: &Window, conversation_id: &str, speaker: &SpeakerResolution) {
    match speaker.status {
        SpeakerResolutionStatus::NoLabel => {}
        SpeakerResolutionStatus::Exact => emit_dev_log(
            window,
            "info",
            "context",
            "Speaker entity resolved",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::FuzzyTypo => emit_dev_log(
            window,
            "warn",
            "warning",
            "Speaker label resolved to active entity",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::Ambiguous => emit_dev_log(
            window,
            "warn",
            "warning",
            "Ambiguous speaker label",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "candidates": speaker.candidates.clone()
            })),
        ),
        SpeakerResolutionStatus::Created => emit_dev_log(
            window,
            "success",
            "context",
            "Entity created",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::Unknown => emit_dev_log(
            window,
            "warn",
            "warning",
            "Unknown speaker label",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref()
            })),
        ),
    }
}

fn emit_relationship_delta_logs(window: &Window, conversation_id: &str, patch: &EnginePatch) {
    let Some(soul_patch) = patch.soul_patch.as_ref() else {
        return;
    };
    let mut deltas = Vec::new();
    if let Some(delta) = soul_patch.relationship_delta.as_ref() {
        deltas.push(delta);
    }
    deltas.extend(soul_patch.relationship_deltas.iter());
    for delta in deltas {
        emit_dev_log(
            window,
            "info",
            "state_updater",
            "Relationship delta applied",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "from": delta.from.as_deref().unwrap_or("active_soul"),
                "target": delta.target.as_deref().unwrap_or("user"),
                "trust": delta.trust,
                "affection": delta.affection,
                "fear": delta.fear,
                "desire": delta.desire,
                "conflict": delta.conflict,
                "curiosity": delta.curiosity,
                "comfort": delta.comfort,
                "dependency": delta.dependency
            })),
        );
    }
}

fn emit_memory_apply_logs(
    window: &Window,
    conversation_id: &str,
    events: &[state_engine::patch::MemoryApplyEvent],
) {
    for event in events {
        let (level, category, message) = match event.action {
            MemoryApplyAction::Added => ("info", "state_updater", "memory_added"),
            MemoryApplyAction::RejectedDuplicate => {
                ("warn", "warning", "memory_rejected_duplicate")
            }
            MemoryApplyAction::RejectedGeneric => ("warn", "warning", "memory_rejected_generic"),
            MemoryApplyAction::Merged => ("info", "state_updater", "memory_merged"),
            MemoryApplyAction::Deprioritized => ("debug", "state_updater", "memory_deprioritized"),
        };
        emit_dev_log(
            window,
            level,
            category,
            message,
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "source_type": event.source_type.as_label(),
                "content_preview": excerpt_for_log(&event.content, 160),
                "reason": event.reason.as_deref()
            })),
        );
        if event.source_type == MemorySourceType::ImportedLog {
            emit_dev_log(
                window,
                "info",
                "context",
                "imported_log_detected",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "content_preview": excerpt_for_log(&event.content, 160)
                })),
            );
        }
        if matches!(
            event.source_type,
            MemorySourceType::CrossSessionBleed | MemorySourceType::PreviousSession
        ) {
            emit_dev_log(
                window,
                "info",
                "context",
                "cross_session_memory_tagged",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "source_type": event.source_type.as_label(),
                    "content_preview": excerpt_for_log(&event.content, 160)
                })),
            );
        }
    }
}

fn excerpt_for_log(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            chars
                .into_iter()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
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

fn recent_assistant_replay_sources(messages: &[ChatMessage], limit: usize) -> Vec<ReplaySource> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == "assistant")
        .take(limit)
        .map(|message| ReplaySource {
            message_id: message.id,
            content: message.content.clone(),
        })
        .collect()
}

fn guard_narrator_visible_response(
    raw_visible_response: &str,
    user_text: &str,
    replay_sources: &[ReplaySource],
) -> (String, ReplayGuardResult, Option<String>) {
    let output = apply_output_contract_guard(raw_visible_response, user_text);
    let replay = detect_replay(&output.text, replay_sources);
    (output.text, replay, output.warning)
}

fn apply_output_contract_guard(content: &str, user_text: &str) -> OutputContractResult {
    let mut warnings = Vec::new();
    let without_hidden = strip_hidden_state_blocks(content);
    if without_hidden.trim_end() != content.trim_end() {
        warnings.push("hidden state stripped");
    }
    let (without_engine_patch, engine_patch_stripped) =
        strip_engine_patch_payloads(&without_hidden);
    if engine_patch_stripped {
        warnings.push("EnginePatch JSON stripped");
    }

    let (body, status_blocks) = remove_status_blocks(&without_engine_patch);
    if status_blocks.len() > 1 {
        warnings.push("multiple status blocks normalized");
    }
    let mut normalized = body.trim().to_string();
    let gm_reply = is_gm_facing_user_message(user_text) || is_plain_gm_reply(&normalized);

    if let Some(status) = status_blocks.last() {
        let status = normalize_status_block(status);
        if !normalized.is_empty() {
            normalized.push_str("\n\n");
        }
        normalized.push_str(&status);
    } else if !gm_reply && !normalized.is_empty() {
        normalized.push_str(
            "\n\n```status\nScene | Focus: Unknown | Physical state: Not specified | Atmosphere: Not specified\n```",
        );
        warnings.push("fallback status block appended");
    }

    OutputContractResult {
        text: normalized.trim_end().to_string(),
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    }
}

fn remove_status_blocks(content: &str) -> (String, Vec<String>) {
    let mut body = String::new();
    let mut status_blocks = Vec::new();
    let mut current_status = String::new();
    let mut in_status = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !in_status && trimmed.eq_ignore_ascii_case("```status") {
            in_status = true;
            current_status.clear();
            current_status.push_str("```status\n");
            continue;
        }

        if in_status {
            if trimmed == "```" {
                current_status.push_str("```");
                status_blocks.push(current_status.trim_end().to_string());
                current_status.clear();
                in_status = false;
            } else {
                current_status.push_str(line);
                current_status.push('\n');
            }
            continue;
        }

        body.push_str(line);
        body.push('\n');
    }

    if in_status && !current_status.trim().is_empty() {
        status_blocks.push(current_status.trim_end().to_string());
    }

    (body.trim_end().to_string(), status_blocks)
}

fn normalize_status_block(status_block: &str) -> String {
    let mut lines = status_block.lines();
    let first_line = lines.next().unwrap_or_default().trim();
    let body_lines = if first_line.eq_ignore_ascii_case("```status") {
        lines
            .filter(|line| line.trim() != "```")
            .map(str::trim_end)
            .collect::<Vec<_>>()
    } else {
        status_block
            .lines()
            .filter(|line| line.trim() != "```")
            .map(str::trim_end)
            .collect::<Vec<_>>()
    };
    let body = body_lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let body = if body.trim().is_empty() {
        "Scene | Focus: Unknown | Physical state: Not specified | Atmosphere: Not specified"
            .to_string()
    } else {
        body
    };
    format!("```status\n{}\n```", body.trim())
}

fn strip_engine_patch_payloads(content: &str) -> (String, bool) {
    let (without_fenced, stripped_fenced) = strip_engine_patch_fenced_blocks(content);
    let (without_raw, stripped_raw) = strip_engine_patch_raw_json_objects(&without_fenced);
    (
        without_raw.trim_end().to_string(),
        stripped_fenced || stripped_raw,
    )
}

fn strip_engine_patch_fenced_blocks(content: &str) -> (String, bool) {
    let mut output = String::new();
    let mut fenced_block = String::new();
    let mut in_fence = false;
    let mut stripped = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if !in_fence && trimmed.starts_with("```") {
            in_fence = true;
            fenced_block.clear();
            fenced_block.push_str(line);
            fenced_block.push('\n');
            continue;
        }

        if in_fence {
            fenced_block.push_str(line);
            fenced_block.push('\n');
            if line.trim() == "```" {
                if looks_like_engine_patch_text(&fenced_block) {
                    stripped = true;
                } else {
                    output.push_str(&fenced_block);
                }
                fenced_block.clear();
                in_fence = false;
            }
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    if in_fence && !fenced_block.is_empty() {
        if looks_like_engine_patch_text(&fenced_block) {
            stripped = true;
        } else {
            output.push_str(&fenced_block);
        }
    }

    (output.trim_end().to_string(), stripped)
}

fn strip_engine_patch_raw_json_objects(content: &str) -> (String, bool) {
    let char_indices = content.char_indices().collect::<Vec<_>>();
    let mut output = String::new();
    let mut last_emit = 0usize;
    let mut cursor = 0usize;
    let mut stripped = false;

    while cursor < char_indices.len() {
        let (byte_index, character) = char_indices[cursor];
        if character == '{' {
            if let Some(end_byte) = matching_json_object_end(content, byte_index) {
                let candidate = &content[byte_index..end_byte];
                if looks_like_engine_patch_text(candidate)
                    && serde_json::from_str::<serde_json::Value>(candidate).is_ok()
                {
                    output.push_str(&content[last_emit..byte_index]);
                    last_emit = end_byte;
                    stripped = true;
                    while cursor < char_indices.len() && char_indices[cursor].0 < end_byte {
                        cursor += 1;
                    }
                    continue;
                }
            }
        }
        cursor += 1;
    }

    output.push_str(&content[last_emit..]);
    (output.trim_end().to_string(), stripped)
}

fn matching_json_object_end(content: &str, start_byte: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, character) in content[start_byte..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start_byte + offset + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn looks_like_engine_patch_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("\"schema_version\"")
        && (lower.contains("\"soul_patch\"")
            || lower.contains("\"world_patch\"")
            || lower.contains("\"body_patch\"")
            || lower.contains("\"relationship_deltas\""))
}

fn is_gm_facing_user_message(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    let trimmed = lower.trim_start();
    trimmed.starts_with("gm:")
        || trimmed.starts_with("narrator:")
        || trimmed.starts_with("ooc:")
        || trimmed.starts_with("[ooc]")
        || lower.contains("talking to the narrator")
        || lower.contains("talking to the gm")
        || lower.contains("addressing the narrator")
        || lower.contains("address the narrator")
}

fn is_plain_gm_reply(response: &str) -> bool {
    let lower = response.to_ascii_lowercase();
    let trimmed = lower.trim_start();
    trimmed.starts_with("gm:")
        || trimmed.starts_with("narrator:")
        || trimmed.starts_with("ooc:")
        || trimmed.starts_with("[ooc]")
}

fn detect_replay(new_response: &str, replay_sources: &[ReplaySource]) -> ReplayGuardResult {
    let mut best = ReplayGuardResult::default();
    for source in replay_sources {
        let candidate = compare_replay_against_source(new_response, source);
        if candidate.replay_score > best.replay_score {
            best = candidate;
        }
    }
    best
}

fn compare_replay_against_source(new_response: &str, source: &ReplaySource) -> ReplayGuardResult {
    let new_clean = normalize_for_replay(new_response);
    let previous_clean = normalize_for_replay(&source.content);
    if new_clean.is_empty() || previous_clean.is_empty() {
        return ReplayGuardResult {
            compared_against_message_id: Some(source.message_id),
            ..ReplayGuardResult::default()
        };
    }

    let paragraph_score = paragraph_replay_score(new_response, &source.content);
    let sentence_score = sentence_overlap_score(&new_clean, &previous_clean);
    let shingle_score = shingle_overlap_score(&new_clean, &previous_clean, 10);
    let (score, reason) = [
        (paragraph_score, "paragraph nearly identical"),
        (sentence_score, "sentence overlap exceeded threshold"),
        (
            shingle_score,
            "repeated wording shingles exceeded threshold",
        ),
    ]
    .into_iter()
    .max_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
    .unwrap_or((0.0, "no overlap"));
    let replay_detected = score > 0.35
        || (paragraph_score >= 0.90
            && repeated_long_paragraph_exists(new_response, &source.content));

    ReplayGuardResult {
        replay_detected,
        replay_score: score,
        replay_reason: replay_detected.then(|| reason.to_string()),
        compared_against_message_id: Some(source.message_id),
    }
}

fn normalize_for_replay(content: &str) -> String {
    let (without_status, _) = remove_status_blocks(&strip_hidden_state_blocks(content));
    let (without_patch, _) = strip_engine_patch_payloads(&without_status);
    without_patch
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentence_overlap_score(new_clean: &str, previous_clean: &str) -> f32 {
    let previous_sentences = split_replay_sentences(previous_clean)
        .into_iter()
        .filter(|sentence| sentence.chars().count() >= 60)
        .collect::<HashSet<_>>();
    if previous_sentences.is_empty() {
        return 0.0;
    }
    let new_sentences = split_replay_sentences(new_clean)
        .into_iter()
        .filter(|sentence| sentence.chars().count() >= 60)
        .collect::<Vec<_>>();
    let total_chars = new_sentences
        .iter()
        .map(|sentence| sentence.chars().count())
        .sum::<usize>();
    if total_chars == 0 {
        return 0.0;
    }
    let overlap_chars = new_sentences
        .iter()
        .filter(|sentence| previous_sentences.contains(*sentence))
        .map(|sentence| sentence.chars().count())
        .sum::<usize>();
    overlap_chars as f32 / total_chars as f32
}

fn split_replay_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .map(str::to_string)
        .collect()
}

fn shingle_overlap_score(new_clean: &str, previous_clean: &str, shingle_size: usize) -> f32 {
    let new_words = new_clean.split_whitespace().collect::<Vec<_>>();
    let previous_words = previous_clean.split_whitespace().collect::<Vec<_>>();
    if new_words.len() < shingle_size || previous_words.len() < shingle_size {
        return 0.0;
    }
    let new_shingles = word_shingles(&new_words, shingle_size);
    let previous_shingles = word_shingles(&previous_words, shingle_size);
    if new_shingles.is_empty() {
        return 0.0;
    }
    let overlap = new_shingles
        .iter()
        .filter(|shingle| previous_shingles.contains(*shingle))
        .count();
    overlap as f32 / new_shingles.len() as f32
}

fn word_shingles(words: &[&str], size: usize) -> HashSet<String> {
    words
        .windows(size)
        .map(|window| window.join(" "))
        .collect::<HashSet<_>>()
}

fn paragraph_replay_score(new_response: &str, previous_response: &str) -> f32 {
    let new_paragraphs = replay_paragraphs(new_response);
    let previous_paragraphs = replay_paragraphs(previous_response);
    let mut best = 0.0f32;
    for new_paragraph in &new_paragraphs {
        if new_paragraph.chars().count() < 250 {
            continue;
        }
        for previous_paragraph in &previous_paragraphs {
            if previous_paragraph.chars().count() < 250 {
                continue;
            }
            best = best.max(word_jaccard_similarity(new_paragraph, previous_paragraph));
        }
    }
    best
}

fn repeated_long_paragraph_exists(new_response: &str, previous_response: &str) -> bool {
    paragraph_replay_score(new_response, previous_response) >= 0.90
}

fn replay_paragraphs(content: &str) -> Vec<String> {
    let (without_status, _) = remove_status_blocks(&strip_hidden_state_blocks(content));
    without_status
        .split("\n\n")
        .map(normalize_for_replay)
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

fn word_jaccard_similarity(left: &str, right: &str) -> f32 {
    let left_words = left.split_whitespace().collect::<HashSet<_>>();
    let right_words = right.split_whitespace().collect::<HashSet<_>>();
    if left_words.is_empty() || right_words.is_empty() {
        return 0.0;
    }
    let intersection = left_words.intersection(&right_words).count();
    let union = left_words.union(&right_words).count();
    intersection as f32 / union as f32
}

fn messages_with_repair_instruction(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    let mut repaired = messages.to_vec();
    if let Some(last_user) = repaired
        .iter_mut()
        .rev()
        .find(|message| message.role == "user")
    {
        last_user.content = format!(
            "{}\n\n[REPAIR INSTRUCTION - HIGH PRIORITY]\nThe previous draft repeated earlier narration. Continue from the latest user input and do not reuse previous wording.",
            last_user.content.trim()
        );
    }
    repaired
}

fn last_user_message_content(messages: &[ApiMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.clone())
        .unwrap_or_default()
}

fn ensure_default_entities(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
) -> rusqlite::Result<()> {
    let default_player = EntityRecord {
        entity_id: "default_player".into(),
        conversation_id: conversation_id.into(),
        display_name: "User".into(),
        aliases: vec!["user".into(), "player".into(), "operator".into()],
        kind: "operator".into(),
        controlled_by: "user".into(),
        linked_soul_id: None,
        active_in_scene: true,
        created_at: 0,
        updated_at: 0,
    };
    db::upsert_entity(conn, &default_player)?;

    let soul_entity = EntityRecord {
        entity_id: normalize_entity_id(&soul.character_name),
        conversation_id: conversation_id.into(),
        display_name: soul.character_name.clone(),
        aliases: vec![soul.character_name.clone()],
        kind: "soul".into(),
        controlled_by: "narrator".into(),
        linked_soul_id: Some(soul.character_id.clone()),
        active_in_scene: true,
        created_at: 0,
        updated_at: 0,
    };
    db::upsert_entity(conn, &soul_entity)?;
    Ok(())
}

fn resolve_speaker_for_turn(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
    user_text: &str,
) -> rusqlite::Result<EntityTurnContext> {
    ensure_default_entities(conn, conversation_id, soul)?;
    let label = extract_latest_speaker_label(user_text);
    let mut entities = db::list_entities(conn, conversation_id)?;
    let speaker = match label {
        None => default_speaker_resolution(),
        Some(label) => resolve_speaker_label(conn, conversation_id, &mut entities, &label)?,
    };
    entities = db::list_entities(conn, conversation_id)?;
    Ok(EntityTurnContext { entities, speaker })
}

fn default_speaker_resolution() -> SpeakerResolution {
    SpeakerResolution {
        label: None,
        entity_id: "default_player".into(),
        display_name: "User".into(),
        status: SpeakerResolutionStatus::NoLabel,
        candidates: Vec::new(),
    }
}

fn resolve_speaker_label(
    conn: &Connection,
    conversation_id: &str,
    entities: &mut [EntityRecord],
    label: &str,
) -> rusqlite::Result<SpeakerResolution> {
    if let Some(entity) = exact_entity_match(entities, label) {
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: entity.entity_id.clone(),
            display_name: entity.display_name.clone(),
            status: SpeakerResolutionStatus::Exact,
            candidates: Vec::new(),
        });
    }

    if let Some(match_result) = best_fuzzy_entity_match(entities, label, true) {
        if match_result.ambiguous {
            return Ok(ambiguous_resolution(label, match_result.candidates));
        }
        let entity = match_result.entity;
        let updated = db::add_entity_alias(conn, conversation_id, &entity.entity_id, label)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: updated.entity_id,
            display_name: updated.display_name,
            status: SpeakerResolutionStatus::FuzzyTypo,
            candidates: Vec::new(),
        });
    }

    if let Some(match_result) = best_fuzzy_entity_match(entities, label, false) {
        if match_result.ambiguous {
            return Ok(ambiguous_resolution(label, match_result.candidates));
        }
        let entity = match_result.entity;
        let updated = db::add_entity_alias(conn, conversation_id, &entity.entity_id, label)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: updated.entity_id,
            display_name: updated.display_name,
            status: SpeakerResolutionStatus::FuzzyTypo,
            candidates: Vec::new(),
        });
    }

    if label_can_create_entity(label) {
        let entity = EntityRecord {
            entity_id: normalize_entity_id(label),
            conversation_id: conversation_id.into(),
            display_name: label.trim().to_string(),
            aliases: vec![label.trim().to_string()],
            kind: "user_controlled".into(),
            controlled_by: "user".into(),
            linked_soul_id: None,
            active_in_scene: true,
            created_at: 0,
            updated_at: 0,
        };
        let entity = db::upsert_entity(conn, &entity)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: entity.entity_id,
            display_name: entity.display_name,
            status: SpeakerResolutionStatus::Created,
            candidates: Vec::new(),
        });
    }

    Ok(SpeakerResolution {
        label: Some(label.to_string()),
        entity_id: "unknown_speaker".into(),
        display_name: "Unknown speaker".into(),
        status: SpeakerResolutionStatus::Unknown,
        candidates: Vec::new(),
    })
}

#[derive(Debug)]
struct FuzzyEntityMatch {
    entity: EntityRecord,
    ambiguous: bool,
    candidates: Vec<String>,
}

fn best_fuzzy_entity_match(
    entities: &[EntityRecord],
    label: &str,
    active_only: bool,
) -> Option<FuzzyEntityMatch> {
    let normalized_label = normalize_match_key(label);
    if normalized_label.len() < 2 {
        return None;
    }
    let mut scored = entities
        .iter()
        .filter(|entity| !active_only || entity.active_in_scene)
        .filter_map(|entity| {
            best_entity_score(entity, &normalized_label)
                .filter(|score| fuzzy_score_is_close(normalized_label.len(), *score))
                .map(|score| (entity.clone(), score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (entity, score) = scored.first()?.clone();
    let candidates = scored
        .iter()
        .take(3)
        .map(|(entity, _)| entity.display_name.clone())
        .collect::<Vec<_>>();
    let ambiguous = scored
        .get(1)
        .map(|(_, second_score)| (score - *second_score).abs() < 0.08)
        .unwrap_or(false);
    Some(FuzzyEntityMatch {
        entity,
        ambiguous,
        candidates,
    })
}

fn exact_entity_match<'a>(entities: &'a [EntityRecord], label: &str) -> Option<&'a EntityRecord> {
    let trimmed = label.trim();
    entities.iter().find(|entity| {
        entity.entity_id == trimmed
            || entity.display_name == trimmed
            || entity.aliases.iter().any(|alias| alias == trimmed)
            || entity.entity_id.eq_ignore_ascii_case(trimmed)
            || entity.display_name.eq_ignore_ascii_case(trimmed)
            || entity
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(trimmed))
    })
}

fn best_entity_score(entity: &EntityRecord, normalized_label: &str) -> Option<f32> {
    let mut keys = vec![
        normalize_match_key(&entity.entity_id),
        normalize_match_key(&entity.display_name),
    ];
    keys.extend(
        entity
            .aliases
            .iter()
            .map(|alias| normalize_match_key(alias)),
    );
    keys.into_iter()
        .filter(|key| !key.is_empty())
        .map(|key| similarity_score(normalized_label, &key))
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
}

fn fuzzy_score_is_close(label_len: usize, score: f32) -> bool {
    if label_len <= 4 {
        score >= 0.66
    } else {
        score >= 0.82
    }
}

fn ambiguous_resolution(label: &str, candidates: Vec<String>) -> SpeakerResolution {
    SpeakerResolution {
        label: Some(label.to_string()),
        entity_id: "unknown_speaker".into(),
        display_name: "Unknown speaker".into(),
        status: SpeakerResolutionStatus::Ambiguous,
        candidates,
    }
}

fn extract_latest_speaker_label(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .filter_map(extract_speaker_label_from_line)
        .next()
}

fn extract_speaker_label_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let colon_index = trimmed.find(':')?;
    if colon_index == 0 || colon_index > 48 {
        return None;
    }
    let label = trimmed[..colon_index].trim();
    if label_can_be_speaker(label) {
        Some(label.to_string())
    } else {
        None
    }
}

fn label_can_be_speaker(label: &str) -> bool {
    let label = label.trim();
    !label.is_empty()
        && label.chars().any(char::is_alphabetic)
        && label.chars().all(|character| {
            character.is_alphanumeric()
                || character.is_whitespace()
                || matches!(character, '_' | '-' | '\'' | '.')
        })
}

fn label_can_create_entity(label: &str) -> bool {
    let normalized = normalize_match_key(label);
    normalized.len() >= 2
        && normalized.len() <= 40
        && !matches!(
            normalized.as_str(),
            "i" | "me" | "we" | "he" | "she" | "they" | "system" | "assistant" | "narrator"
        )
}

fn normalize_entity_id(label: &str) -> String {
    let normalized = normalize_match_key(label);
    if normalized == "user" || normalized == "player" || normalized == "operator" {
        "default_player".into()
    } else if normalized.is_empty() {
        "unknown_speaker".into()
    } else {
        normalized
    }
}

fn normalize_match_key(label: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for character in label.trim().chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn similarity_score(left: &str, right: &str) -> f32 {
    if left == right {
        return 1.0;
    }
    let max_len = left.chars().count().max(right.chars().count());
    if max_len == 0 {
        return 0.0;
    }
    let distance = levenshtein(left, right);
    (1.0 - (distance as f32 / max_len as f32)).clamp(0.0, 1.0)
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];
    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution = if left_char == *right_char { 0 } else { 1 };
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        previous.clone_from(&current);
    }
    previous[right_chars.len()]
}

fn build_entity_updater_context(soul: &Soul, context: &EntityTurnContext) -> String {
    let active_entities = context
        .entities
        .iter()
        .filter(|entity| entity.active_in_scene)
        .map(|entity| {
            format!(
                "- {} ({}) kind={}, controlled_by={}",
                entity.entity_id, entity.display_name, entity.kind, entity.controlled_by
            )
        })
        .collect::<Vec<_>>();
    let relationship_lines = context
        .entities
        .iter()
        .filter(|entity| entity.kind != "soul")
        .filter_map(|entity| {
            relationship_for_entity(soul, &entity.entity_id).map(|relationship| {
                format!(
                    "{} -> {}: trust {:.0}, affection {:.0}, fear {:.0}, desire {:.0}, conflict {:.0}, curiosity {:.0}, comfort {:.0}, dependency {:.0}",
                    soul.character_name,
                    entity.entity_id,
                    relationship.trust,
                    relationship.affection,
                    relationship.fear,
                    relationship.desire,
                    relationship.conflict,
                    relationship.curiosity,
                    relationship.comfort,
                    relationship.dependency
                )
            })
        })
        .collect::<Vec<_>>();

    format!(
        "[ACTIVE ENTITIES]\n{}\n\n[LATEST SPEAKER ENTITY]\n{}\n\n[RELEVANT RELATIONSHIPS]\n{}",
        if active_entities.is_empty() {
            "None".into()
        } else {
            active_entities.join("\n")
        },
        context.speaker.summary_line(),
        if relationship_lines.is_empty() {
            "No directed relationship records for active non-soul entities yet.".into()
        } else {
            relationship_lines.join("\n")
        }
    )
}

fn relationship_for_entity<'a>(
    soul: &'a Soul,
    entity_id: &str,
) -> Option<&'a state_engine::soul::Relationship> {
    soul.relationships.get(entity_id).or_else(|| {
        if entity_id.eq_ignore_ascii_case("default_player") {
            soul.relationships.get("user")
        } else {
            None
        }
    })
}

impl SpeakerResolution {
    fn summary_line(&self) -> String {
        match self.status {
            SpeakerResolutionStatus::NoLabel => {
                "No explicit speaker label; defaulting latest speaker to default_player (User)."
                    .into()
            }
            SpeakerResolutionStatus::Exact => format!(
                "Label {:?} resolved to {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::FuzzyTypo => format!(
                "Label {:?} resolved as likely typo/alias for {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::Ambiguous => format!(
                "Label {:?} is ambiguous; use unknown_speaker. Candidates: {}.",
                self.label.as_deref().unwrap_or(""),
                self.candidates.join(", ")
            ),
            SpeakerResolutionStatus::Created => format!(
                "Label {:?} created entity {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::Unknown => format!(
                "Label {:?} could not be resolved; use unknown_speaker.",
                self.label.as_deref().unwrap_or("")
            ),
        }
    }
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

fn build_state_updater_user_message(
    user_text: &str,
    narrator_response: &str,
    entity_context: Option<&str>,
) -> String {
    let entity_context = entity_context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map(|context| format!("{context}\n\n"))
        .unwrap_or_default();
    let compact_user = compact_user_message_for_updater(user_text);
    let compact_narrator = compact_narrator_response_for_updater(narrator_response);
    format!(
        "{}[LATEST USER MESSAGE]\n{}\n\n[NARRATOR RESPONSE]\n{}",
        entity_context, compact_user, compact_narrator
    )
}

fn compact_user_message_for_updater(user_text: &str) -> String {
    let trimmed = user_text.trim();
    if looks_like_imported_log_text(&trimmed.to_ascii_lowercase()) {
        return format!(
            "[IMPORTED LOG DETECTED]\nThe user pasted a Mnemosyne/exported chat log. Treat it as imported context, not current lived experience. Create at most 1-3 imported_log memories if durable facts matter.\nExcerpt:\n{}",
            head_tail_excerpt_chars(trimmed, 700, 500, 1_300)
        );
    }
    head_tail_excerpt_chars(trimmed, 900, 700, 1_700)
}

fn compact_narrator_response_for_updater(narrator_response: &str) -> String {
    let visible = strip_status_blocks_for_export(&strip_hidden_state_blocks(narrator_response));
    head_tail_excerpt_chars(visible.trim(), 500, 1_300, 1_900)
}

fn head_tail_excerpt_chars(
    text: &str,
    head_chars: usize,
    tail_chars: usize,
    max_chars: usize,
) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let head = chars.iter().take(head_chars).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(tail_chars)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}\n...[truncated for updater budget]...\n{tail}")
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
        build_state_updater_user_message(user_text, narrator_response, None)
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
    let relationship = patch.soul_patch.as_ref().and_then(|patch| {
        patch
            .relationship_delta
            .as_ref()
            .or_else(|| patch.relationship_deltas.first())
    });
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
    tag_memory_sources_from_turn(&mut patch, user_text, &turn_text);
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
        cleanup_stale_active_plots(soul, world_patch, &turn_text);
        if world_patch.is_empty_for_commands() {
            patch.world_patch = None;
        }
    }

    patch
}

fn tag_memory_sources_from_turn(patch: &mut EnginePatch, user_text: &str, turn_text: &str) {
    let Some(soul_patch) = patch.soul_patch.as_mut() else {
        return;
    };
    let turn_source = infer_turn_memory_source(user_text, turn_text);
    for memory in &mut soul_patch.new_memories {
        let memory_source = memory
            .source_type
            .or_else(|| infer_turn_memory_source(&memory.content, &memory.content))
            .or(turn_source);
        if let Some(source_type) = memory_source {
            memory.source_type = Some(source_type);
            if source_type.imported_or_cross_session() {
                memory.is_lived_experience.get_or_insert(false);
                memory.is_imported_context.get_or_insert(true);
            }
            if source_type == MemorySourceType::UserClaimed {
                memory.is_lived_experience.get_or_insert(false);
            }
            if source_type == MemorySourceType::Unknown {
                memory.confidence.get_or_insert(0.45);
            }
        } else {
            memory
                .source_type
                .get_or_insert(MemorySourceType::CurrentSession);
            memory.is_lived_experience.get_or_insert(true);
            memory.is_imported_context.get_or_insert(false);
        }
    }
}

fn infer_turn_memory_source(user_text: &str, turn_text: &str) -> Option<MemorySourceType> {
    let user_lower = user_text.to_ascii_lowercase();
    let turn_lower = turn_text.to_ascii_lowercase();
    if looks_like_imported_log_text(&user_lower) {
        Some(MemorySourceType::ImportedLog)
    } else if contains_any_text(
        &turn_lower,
        &[
            "cross-session bleed",
            "cross session bleed",
            "memory bleed",
            "another version",
            "parallel timeline",
            "imported memory",
        ],
    ) {
        Some(MemorySourceType::CrossSessionBleed)
    } else if contains_any_text(
        &turn_lower,
        &[
            "previous session",
            "prior session",
            "archived chat",
            "old chat log",
        ],
    ) {
        Some(MemorySourceType::PreviousSession)
    } else if contains_any_text(
        &user_lower,
        &[
            "i explain",
            "i tell her that",
            "i tell aurora that",
            "the truth is",
            "it means",
        ],
    ) {
        Some(MemorySourceType::UserClaimed)
    } else {
        None
    }
}

fn looks_like_imported_log_text(lower_text: &str) -> bool {
    lower_text.contains("# mnemosyne chat log")
        || (lower_text.contains("## user")
            && lower_text.contains("## narrator")
            && lower_text.contains("created:"))
        || lower_text.contains("# mnemosyne llm payload history")
}

fn contains_any_text(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn cleanup_stale_active_plots(
    soul: &Soul,
    world_patch: &mut state_engine::patch::WorldPatch,
    turn_text: &str,
) {
    let has_new_plot = world_patch
        .active_plot_add
        .iter()
        .any(|plot| !plot.trim().is_empty());
    if has_new_plot {
        for plot in &soul.world.active_plots {
            let lower = plot.to_ascii_lowercase();
            if lower.contains("establish the first scene")
                || (lower.contains("interact with rhy")
                    && !turn_text.to_ascii_lowercase().contains("rhy"))
            {
                world_patch.active_plot_resolve.push(plot.clone());
            }
        }
    }
    dedupe_strings(&mut world_patch.active_plot_resolve);
    dedupe_strings(&mut world_patch.active_plot_add);
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.trim().to_ascii_lowercase()));
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

fn normalize_image_source(source: Option<&str>) -> Result<String, String> {
    match source.unwrap_or("uploaded").trim() {
        "uploaded" => Ok("uploaded".into()),
        "generated" => Ok("generated".into()),
        "imported" => Ok("imported".into()),
        _ => Err("Unsupported image source".into()),
    }
}

fn uuid_like_id() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_millis(),
        DEV_LOG_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn inspect_image_bytes(bytes: &[u8]) -> Result<ImageFileInfo, String> {
    let header = &bytes[..bytes.len().min(32)];
    if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(ImageFileInfo {
            extension: "png",
            mime_type: "image/png",
            width: read_be_u32(header, 16),
            height: read_be_u32(header, 20),
        });
    }
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        let (width, height) = jpeg_dimensions_from_bytes(bytes).unwrap_or((None, None));
        return Ok(ImageFileInfo {
            extension: "jpg",
            mime_type: "image/jpeg",
            width,
            height,
        });
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return Ok(ImageFileInfo {
            extension: "webp",
            mime_type: "image/webp",
            width: None,
            height: None,
        });
    }
    if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        return Ok(ImageFileInfo {
            extension: "gif",
            mime_type: "image/gif",
            width: read_le_u16(header, 6),
            height: read_le_u16(header, 8),
        });
    }
    Err("Unsupported image type. Use PNG, JPG, WEBP, or GIF.".into())
}

fn read_be_u32(header: &[u8], offset: usize) -> Option<i64> {
    header
        .get(offset..offset + 4)
        .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64)
}

fn read_le_u16(header: &[u8], offset: usize) -> Option<i64> {
    header
        .get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as i64)
}

fn jpeg_dimensions_from_bytes(bytes: &[u8]) -> Result<(Option<i64>, Option<i64>), String> {
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        index += 2;
        if marker == 0xd9 || marker == 0xda || index + 2 > bytes.len() {
            break;
        }
        let length = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if length < 2 || index + length > bytes.len() {
            break;
        }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2 | 0xc3) && index + 7 < bytes.len() {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as i64;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as i64;
            return Ok((Some(width), Some(height)));
        }
        index += length;
    }
    Ok((None, None))
}

fn safe_image_log_name(file_name: &str) -> String {
    Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string()
}

fn resolve_export_path(
    app: &AppHandle,
    requested: &str,
    fallback_extension: &str,
) -> Result<PathBuf, String> {
    let requested_path = PathBuf::from(requested);
    if requested_path.is_absolute() {
        if let Some(parent) = requested_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        return Ok(requested_path);
    }
    let mut dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|err| err.to_string())?;
    dir.push("mnemosyne-exports");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let requested_name = requested_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback_extension);
    let mut file_name = safe_filename(requested_name);
    if !file_name.contains('.') {
        file_name.push('.');
        file_name.push_str(fallback_extension);
    }
    dir.push(file_name);
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
        replay_detected: false,
        replay_score: 0.0,
        replay_reason: None,
        replay_compared_against_message_id: None,
        output_contract_warning: None,
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
    fn opening_narrator_message_seeds_visible_assistant_without_payload_logs() {
        let conn = db::init_memory_connection().expect("db");
        let mut soul = new_default_soul("Aurora");
        soul.profile.opening_narrator_message = "Aurora waits by the door.".into();
        db::upsert_soul(&conn, &soul).expect("soul");
        db::ensure_conversation_with_title(
            &conn,
            "opening-session",
            &soul.character_id,
            Some("Opening test"),
        )
        .expect("conversation");

        let seeded = seed_opening_narrator_message(
            &conn,
            "opening-session",
            &soul.profile.opening_narrator_message,
        )
        .expect("seed");

        assert!(seeded.is_some());
        let messages = db::list_messages(&conn, "opening-session", 10).expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].content, "Aurora waits by the door.");
        assert!(db::list_llm_payload_logs(&conn, "opening-session")
            .expect("logs")
            .is_empty());
        assert!(
            seed_opening_narrator_message(&conn, "opening-session", "Another")
                .expect("second seed")
                .is_none()
        );
    }

    #[test]
    fn image_file_validation_accepts_png_and_rejects_unknown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("tiny.png");
        fs::write(
            &png_path,
            [
                0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D',
                b'R', 0, 0, 0, 2, 0, 0, 0, 3,
            ],
        )
        .expect("png");
        let info = inspect_image_bytes(&fs::read(&png_path).expect("png bytes")).expect("png info");
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.width, Some(2));
        assert_eq!(info.height, Some(3));

        let text_path = dir.path().join("not-image.txt");
        fs::write(&text_path, b"nope").expect("text");
        assert!(inspect_image_bytes(&fs::read(&text_path).expect("text bytes")).is_err());
    }

    #[test]
    fn speaker_label_creates_named_entity() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        db::upsert_soul(&conn, &soul).expect("upsert");
        db::ensure_conversation(&conn, "entities", &soul.character_id).expect("conversation");

        let context =
            resolve_speaker_for_turn(&conn, "entities", &soul, "Rhy: I keep my hands visible.")
                .expect("resolve");

        assert_eq!(context.speaker.entity_id, "rhy");
        assert_eq!(context.speaker.status, SpeakerResolutionStatus::Created);
        assert!(context
            .entities
            .iter()
            .any(|entity| entity.entity_id == "rhy" && entity.display_name == "Rhy"));
    }

    #[test]
    fn typo_speaker_label_resolves_to_active_entity() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        db::upsert_soul(&conn, &soul).expect("upsert");
        db::ensure_conversation(&conn, "typo", &soul.character_id).expect("conversation");
        resolve_speaker_for_turn(&conn, "typo", &soul, "Rhy: I answer first.").expect("seed");

        let context = resolve_speaker_for_turn(&conn, "typo", &soul, "Rjy: I correct myself.")
            .expect("resolve");

        assert_eq!(context.speaker.entity_id, "rhy");
        assert_eq!(context.speaker.status, SpeakerResolutionStatus::FuzzyTypo);
        let rhy = db::get_entity(&conn, "typo", "rhy").expect("rhy");
        assert!(rhy.aliases.iter().any(|alias| alias == "Rjy"));
    }

    #[test]
    fn ambiguous_speaker_typo_does_not_create_duplicate_entity() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        db::upsert_soul(&conn, &soul).expect("upsert");
        db::ensure_conversation(&conn, "ambiguous", &soul.character_id).expect("conversation");
        for name in ["Rhy", "Rey"] {
            db::upsert_entity(
                &conn,
                &EntityRecord {
                    entity_id: normalize_entity_id(name),
                    conversation_id: "ambiguous".into(),
                    display_name: name.into(),
                    aliases: vec![name.into()],
                    kind: "user_controlled".into(),
                    controlled_by: "user".into(),
                    linked_soul_id: None,
                    active_in_scene: true,
                    created_at: 0,
                    updated_at: 0,
                },
            )
            .expect("seed entity");
        }

        let context = resolve_speaker_for_turn(&conn, "ambiguous", &soul, "Ry: Maybe typo.")
            .expect("resolve");

        assert_eq!(context.speaker.entity_id, "unknown_speaker");
        assert_eq!(context.speaker.status, SpeakerResolutionStatus::Ambiguous);
        let entities = db::list_entities(&conn, "ambiguous").expect("entities");
        assert!(!entities.iter().any(|entity| entity.entity_id == "ry"));
    }

    #[test]
    fn state_updater_message_includes_entities_and_latest_speaker() {
        let conn = db::init_memory_connection().expect("db");
        let mut soul = new_default_soul("Aurora");
        db::upsert_soul(&conn, &soul).expect("upsert");
        db::ensure_conversation(&conn, "updater-entities", &soul.character_id)
            .expect("conversation");
        let context = resolve_speaker_for_turn(
            &conn,
            "updater-entities",
            &soul,
            "Junhwa: I refuse the warrant.",
        )
        .expect("resolve");
        soul.relationships.insert("junhwa".into(), {
            let mut relationship = soul.relationships["user"].clone();
            relationship.trust = 8.0;
            relationship.fear = 35.0;
            relationship.conflict = 60.0;
            relationship
        });
        let entity_context = build_entity_updater_context(&soul, &context);
        let message = build_state_updater_user_message(
            "Junhwa: I refuse the warrant.",
            "Aurora narrows her eyes.",
            Some(&entity_context),
        );

        assert!(message.contains("[ACTIVE ENTITIES]"));
        assert!(message.contains("[LATEST SPEAKER ENTITY]"));
        assert!(message.contains("junhwa"));
        assert!(message.contains("Aurora -> junhwa"));
        assert!(message.contains("[LATEST USER MESSAGE]"));
        assert!(message.contains("[NARRATOR RESPONSE]"));
    }

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
        assert!(!result
            .soul
            .memory
            .core
            .iter()
            .any(|memory| memory.contains("neutral exchange added texture")));
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
        assert!(preview
            .system_message
            .contains("You are Mnemosyne's scene narrator"));
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
    fn imported_chat_log_memory_is_tagged_before_apply() {
        let soul = new_default_soul("Aurora");
        let user = "# Mnemosyne Chat Log\n\n## User\nOld turn\n\n## Narrator\nPrevious Aurora argued.\nCreated: 100";
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"Imported log says previous Aurora argued about ownership.","tag":"identity_continuity"}]}}"#,
        )
        .expect("patch");

        let filtered =
            sanitize_state_updater_patch(patch, &soul, user, "Aurora studies the pasted log.");
        let memory = filtered
            .soul_patch
            .as_ref()
            .and_then(|patch| patch.new_memories.first())
            .expect("memory");

        assert_eq!(memory.source_type, Some(MemorySourceType::ImportedLog));
        assert_eq!(memory.is_lived_experience, Some(false));
        assert_eq!(memory.is_imported_context, Some(true));
    }

    #[test]
    fn previous_session_memory_is_tagged_before_apply() {
        let soul = new_default_soul("Aurora");
        let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"Aurora learned this may belong to a previous session version of herself.","tag":"identity_continuity"}]}}"#,
        )
        .expect("patch");

        let filtered = sanitize_state_updater_patch(
            patch,
            &soul,
            "I explain this was from a previous session.",
            "Aurora treats it as imported context, not direct memory.",
        );
        let memory = filtered
            .soul_patch
            .as_ref()
            .and_then(|patch| patch.new_memories.first())
            .expect("memory");

        assert!(matches!(
            memory.source_type,
            Some(MemorySourceType::PreviousSession | MemorySourceType::CrossSessionBleed)
        ));
        assert_eq!(memory.is_lived_experience, Some(false));
        assert_eq!(memory.is_imported_context, Some(true));
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
    fn updater_payload_compacts_long_imported_chat_log() {
        let soul = new_default_soul("Aurora");
        let long_log = format!(
            "# Mnemosyne Chat Log\n{}\n## User\nold\n## Narrator\nold\nCreated: 1",
            "very long imported line ".repeat(600)
        );
        let payload = build_compact_updater_payload_for_test(
            &soul,
            &long_log,
            "Aurora studies the imported log and does not treat it as lived experience.",
        );

        assert!(payload.contains("[IMPORTED LOG DETECTED]"));
        assert!(estimate_tokens(&payload) < STATE_UPDATER_TARGET_TOKENS);
        assert!(!payload.contains(&"very long imported line ".repeat(100)));
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
                attachments: Vec::new(),
            },
            ChatMessage {
                id: 2,
                conversation_id: "export".into(),
                role: "assistant".into(),
                content:
                    "Visible narrator text.\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]"
                        .into(),
                created_at: 11,
                attachments: Vec::new(),
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

    #[test]
    fn output_contract_keeps_only_last_status_block() {
        let raw = "```status\nScene | Focus: Old | Physical state: Old | Atmosphere: Old\n```\n\nAurora steps back from the doorway.\n\n```status\nScene | Focus: Aurora | Physical state: Guarded | Atmosphere: Tense\n```";

        let result = apply_output_contract_guard(raw, "I knock on the door.");

        assert_eq!(result.text.matches("```status").count(), 1);
        assert!(!result.text.contains("Focus: Old"));
        assert!(result.text.contains("Aurora steps back"));
        assert!(result.text.ends_with(
            "```status\nScene | Focus: Aurora | Physical state: Guarded | Atmosphere: Tense\n```"
        ));
        assert!(result
            .warning
            .as_deref()
            .unwrap_or_default()
            .contains("multiple status blocks"));
    }

    #[test]
    fn output_contract_appends_fallback_status_for_scene_narration() {
        let result =
            apply_output_contract_guard("Aurora watches the hallway in silence.", "I wait.");

        assert!(result.text.contains("Aurora watches the hallway"));
        assert!(result.text.contains("Scene | Focus: Unknown"));
        assert!(result
            .warning
            .as_deref()
            .unwrap_or_default()
            .contains("fallback status"));
    }

    #[test]
    fn output_contract_allows_gm_reply_without_status() {
        let result = apply_output_contract_guard(
            "GM: Yes, I understand the out-of-character correction.",
            "I am talking to the Narrator. The GM.",
        );

        assert!(!result.text.contains("```status"));
        assert!(result.text.starts_with("GM:"));
    }

    #[test]
    fn output_contract_strips_engine_patch_json() {
        let raw = "Aurora exhales.\n\n```json\n{\"schema_version\":1,\"world_patch\":{\"recent_event\":\"Should not be visible\"}}\n```\n\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]";

        let result = apply_output_contract_guard(raw, "I speak.");

        assert!(result.text.contains("Aurora exhales."));
        assert!(!result.text.contains("schema_version"));
        assert!(!result.text.contains("HIDDEN STATE"));
        assert!(result.text.contains("```status"));
        assert!(result
            .warning
            .as_deref()
            .unwrap_or_default()
            .contains("EnginePatch JSON stripped"));
    }

    #[test]
    fn anti_replay_detects_repeated_previous_paragraph() {
        let repeated = "Aurora braces one hand against the doorframe, listening to the alarm chew through the hallway while dust trembles down from the ceiling. She does not soften, does not step aside, and does not pretend the room is safe; every line of her body stays angled toward the threat as she demands the truth.";
        let source = ReplaySource {
            message_id: 42,
            content: format!("{repeated}\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```"),
        };

        let result = detect_replay(repeated, &[source]);

        assert!(result.replay_detected);
        assert_eq!(result.compared_against_message_id, Some(42));
        assert!(result.replay_score > 0.35);
    }

    #[test]
    fn anti_replay_ignores_matching_status_blocks() {
        let source = ReplaySource {
            message_id: 7,
            content: "A completely different prior scene.\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```"
                .into(),
        };
        let new_response = "Aurora answers the corrected premise instead of replaying the prior beat.\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```";

        let result = detect_replay(new_response, &[source]);

        assert!(!result.replay_detected);
    }

    #[test]
    fn anti_replay_passes_distinct_response() {
        let source = ReplaySource {
            message_id: 9,
            content:
                "Aurora talks about firewalls and system bleed-through in a prior explanation."
                    .into(),
        };
        let new_response =
            "The GM acknowledges the correction and resets the scene premise around the new system error.";

        let result = detect_replay(new_response, &[source]);

        assert!(!result.replay_detected);
        assert!(result.replay_score <= 0.35);
    }

    fn assert_order(text: &str, first: &str, second: &str) {
        let first_index = text.find(first).expect("first section");
        let second_index = text.find(second).expect("second section");
        assert!(first_index < second_index);
    }
}
