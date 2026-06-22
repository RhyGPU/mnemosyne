use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use state_engine::{
    patch::EnginePatch,
    setting::{
        session_world_from_legacy_world, session_world_from_setting, SessionWorld, SettingSoul,
    },
    soul::Soul,
};
use tauri::{AppHandle, Manager};

static LEDGER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSummary {
    pub character_id: String,
    pub character_name: String,
    pub soul_kind: String,
    pub source_soul_id: Option<String>,
    pub source_savepoint_id: Option<String>,
    pub avatar_image_id: Option<String>,
    pub last_updated: i64,
    pub recent_count: usize,
    pub core_count: usize,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingSummary {
    pub setting_id: String,
    pub setting_name: String,
    pub last_updated: i64,
    pub turn_counter: u64,
    pub location: String,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    #[serde(default = "default_message_channel")]
    pub channel: String,
    #[serde(default = "default_message_status")]
    pub status: String,
    #[serde(default = "default_message_origin")]
    pub origin: String,
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
    pub hidden_at: Option<i64>,
}

fn default_message_status() -> String {
    "active".into()
}

fn default_message_channel() -> String {
    MESSAGE_CHANNEL_RP_SCENE.into()
}

fn default_message_origin() -> String {
    "active".into()
}

pub const MESSAGE_CHANNEL_RP_SCENE: &str = "rp_scene";
pub const MESSAGE_CHANNEL_COMMAND_OOC: &str = "command_ooc";
pub const MESSAGE_CHANNEL_COMMAND_SETUP: &str = "command_setup";
pub const MESSAGE_CHANNEL_COMMAND_STATE: &str = "command_state";
pub const MESSAGE_CHANNEL_COMMAND_PERSONA: &str = "command_persona";
pub const MESSAGE_CHANNEL_COMMAND_ASK: &str = "command_ask";
pub const MESSAGE_CHANNEL_COMMAND_HELP: &str = "command_help";
pub const MESSAGE_CHANNEL_SYSTEM_DEBUG: &str = "system_debug";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerPersona {
    pub persona_id: String,
    pub display_name: String,
    pub description: String,
    pub gender_code: String,
    pub pronouns: String,
    pub is_builtin: bool,
    pub is_archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub appearance: Option<String>,
    pub voice_style: Option<String>,
    pub boundaries: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageAsset {
    pub id: String,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub source: String,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub linked_soul_id: Option<String>,
    pub linked_conversation_id: Option<String>,
    pub linked_message_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageAttachment {
    pub id: i64,
    pub message_id: i64,
    pub image_asset_id: String,
    pub created_at: i64,
    pub image: ImageAsset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub title: String,
    pub soul_id: String,
    pub source_savepoint_id: Option<String>,
    pub world_id: Option<String>,
    pub source_setting_id: Option<String>,
    pub active_player_persona_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
    pub archived_at: Option<i64>,
    pub active_evaluator_profile_id: Option<String>,
    pub is_benchmark: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageVariant {
    pub id: Option<i64>,
    pub message_id: i64,
    pub conversation_id: String,
    pub content: String,
    pub created_at: i64,
    pub label: Option<String>,
    pub source: Option<String>,
    pub is_selected: bool,
    pub soul_snapshot_json: Option<String>,
    pub debug_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPayloadLog {
    pub id: i64,
    pub conversation_id: String,
    pub message_id: Option<i64>,
    pub provider: String,
    pub mode: String,
    pub context_mode: String,
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context_text: String,
    pub estimated_system_tokens: usize,
    pub estimated_user_tokens: usize,
    pub estimated_total_tokens: usize,
    pub truncated: bool,
    pub created_at: i64,
    #[serde(default)]
    pub branch_id: Option<String>,
    #[serde(default)]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub parent_turn_id: Option<String>,
    #[serde(default)]
    pub state_patch_ids_applied: Vec<String>,
    #[serde(default)]
    pub discarded_patch_ids_skipped: Vec<String>,
    #[serde(default)]
    pub state_rebuild_generation: Option<i64>,
    #[serde(default)]
    pub latest_assistant_variant_id: Option<i64>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub raw_provider_response: Option<String>,
    #[serde(default)]
    pub normalized_response: Option<String>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub provider_error: Option<String>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub provider_request_id: Option<String>,
    #[serde(default)]
    pub provider_response_id: Option<String>,
    #[serde(default)]
    pub pipeline_trace_json: Option<String>,
}

impl Default for LlmPayloadLog {
    fn default() -> Self {
        Self {
            id: 0,
            conversation_id: String::new(),
            message_id: None,
            provider: String::new(),
            mode: String::new(),
            context_mode: String::new(),
            model: String::new(),
            base_url: String::new(),
            system_message: String::new(),
            user_message: String::new(),
            context_text: String::new(),
            estimated_system_tokens: 0,
            estimated_user_tokens: 0,
            estimated_total_tokens: 0,
            truncated: false,
            created_at: 0,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            request_id: None,
            turn_id: None,
            raw_provider_response: None,
            normalized_response: None,
            finish_reason: None,
            provider_error: None,
            fallback_used: false,
            fallback_reason: None,
            provider_request_id: None,
            provider_response_id: None,
            pipeline_trace_json: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LlmPayloadResponseUpdate {
    pub request_id: Option<String>,
    pub turn_id: Option<String>,
    pub raw_provider_response: Option<String>,
    pub normalized_response: Option<String>,
    pub finish_reason: Option<String>,
    pub provider_error: Option<String>,
    pub fallback_used: Option<bool>,
    pub fallback_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    pub pipeline_trace_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityRecord {
    pub entity_id: String,
    pub conversation_id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub controlled_by: String,
    pub linked_soul_id: Option<String>,
    pub active_in_scene: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub narrator_timeout_ms: Option<u64>,
    pub evaluator_timeout_ms: Option<u64>,
    pub evaluator_timeout_mode: Option<String>,
    pub evaluator_mode: Option<String>,
    #[serde(default)]
    pub structured_evaluator_policy: Option<String>,
    pub wait_for_evaluator_before_next_turn: Option<bool>,
    pub allow_send_with_stale_state: Option<bool>,
    pub evaluator_background_enabled: Option<bool>,
    pub anti_replay_forced_retry_enabled: Option<bool>,
    pub archived_at: Option<i64>,
    pub narrator_compatibility_status: i32,
    pub evaluator_compatibility_status: i32,
    pub command_compatibility_status: i32,
    pub evaluator_contract_version: i32,
    pub evaluator_prompt_version: i32,
    pub evaluator_last_tested_at: Option<i64>,
    pub evaluator_last_failure_reason: Option<String>,
    /// Structured-output level the provider achieved during the last contract
    /// test probe: 0 untested/failed, 1 prompt-only, 2 json_object, 3 json_schema.
    #[serde(default)]
    pub structured_output_support: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorJob {
    pub evaluator_job_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub assistant_message_id: i64,
    pub status: String, // pending, running, completed, failed, canceled, timed_out
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub timeout_ms: Option<u64>,
    pub timeout_mode: String, // finite, no_app_timeout
    pub model: Option<String>,
    pub provider: Option<String>,
    pub error_message: Option<String>,
    pub patch_applied: bool,
}

#[derive(Debug, Clone)]
pub struct TurnSnapshot {
    pub conversation_id: String,
    pub assistant_message_id: i64,
    pub user_text: String,
    pub soul_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBranch {
    pub branch_id: String,
    pub conversation_id: String,
    pub base_soul_json: String,
    pub base_session_world_json: String,
    pub active_turn_id: Option<String>,
    pub rebuild_generation: i64,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCommit {
    pub turn_id: String,
    pub conversation_id: String,
    pub branch_id: String,
    pub parent_turn_id: Option<String>,
    pub user_message_id: Option<i64>,
    pub assistant_message_id: Option<i64>,
    pub state_patch_id: Option<String>,
    pub selected_variant_id: Option<i64>,
    pub created_at: i64,
    pub active_variant: bool,
    pub is_active: bool,
    pub is_discarded: bool,
    pub is_regenerated_variant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePatchRecord {
    pub patch_id: String,
    pub turn_id: String,
    pub parent_state_hash: Option<String>,
    pub patch_json: String,
    pub inverse_patch_json: Option<String>,
    pub applied_at: i64,
    pub applies_to: String,
    pub is_active: bool,
    pub invalidated_by_patch_id: Option<String>,
    pub supersedes_patch_id: Option<String>,
    pub patch_kind: String,
    pub parent_baseline_patch_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_assistant_message_id: Option<i64>,
    pub source_assistant_variant_id: Option<i64>,
    pub created_by_job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartupRecoveryReport {
    pub branches_rebuilt: usize,
    pub materialized_conversation_ids: Vec<String>,
    pub running_jobs_marked_retryable: usize,
    pub pending_job_ids: Vec<String>,
    pub failed_job_ids: Vec<String>,
    pub canceled_or_timed_out_job_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BranchPatchDebug {
    pub branch_id: String,
    pub active_turn_id: Option<String>,
    pub rebuild_generation: i64,
    pub applied_patches: Vec<String>,
    pub skipped_discarded_patches: Vec<String>,
    pub invalidated_patches: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerRebuild {
    pub soul: Soul,
    pub session_world: SessionWorld,
    pub debug: BranchPatchDebug,
}

pub fn connection_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    dir.push("mnemosyne.sqlite3");
    Ok(dir)
}

pub fn create_backup_file(
    db_path: &std::path::Path,
    backup_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    if !db_path.exists() {
        return Err(format!("Database file does not exist at {:?}", db_path));
    }
    std::fs::create_dir_all(backup_dir).map_err(|err| err.to_string())?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let backup_filename = format!("backup_{}.sqlite3", timestamp);
    let mut backup_path = backup_dir.to_path_buf();
    backup_path.push(backup_filename);

    std::fs::copy(db_path, &backup_path)
        .map_err(|err| format!("Failed to copy DB file: {}", err))?;
    Ok(backup_path)
}

pub fn init_connection(path: &PathBuf) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    run_migrations(&conn)?;
    Ok(conn)
}

pub fn init_memory_connection() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    run_migrations(&conn)?;
    Ok(conn)
}

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS souls (
            character_id TEXT PRIMARY KEY,
            character_name TEXT NOT NULL,
            soul_kind TEXT NOT NULL DEFAULT 'savepoint',
            source_soul_id TEXT,
            source_savepoint_id TEXT,
            avatar_image_id TEXT,
            recent_count INTEGER NOT NULL DEFAULT 0,
            core_count INTEGER NOT NULL DEFAULT 0,
            last_updated INTEGER NOT NULL,
            soul_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            setting_id TEXT PRIMARY KEY,
            setting_name TEXT NOT NULL,
            turn_counter INTEGER NOT NULL DEFAULT 0,
            location TEXT NOT NULL DEFAULT '',
            last_updated INTEGER NOT NULL,
            setting_json TEXT NOT NULL,
            archived_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS session_worlds (
            world_id TEXT PRIMARY KEY,
            source_setting_id TEXT,
            source_savepoint_id TEXT,
            world_kind TEXT NOT NULL DEFAULT 'session_world',
            setting_name TEXT NOT NULL,
            location TEXT NOT NULL DEFAULT '',
            last_updated INTEGER NOT NULL,
            world_json TEXT NOT NULL,
            FOREIGN KEY (source_setting_id) REFERENCES settings(setting_id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_souls_kind_updated
        ON souls(soul_kind, last_updated DESC);

        CREATE INDEX IF NOT EXISTS idx_settings_updated
        ON settings(last_updated DESC);

        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            soul_id TEXT NOT NULL,
            world_id TEXT,
            source_setting_id TEXT,
            active_player_persona_id TEXT NOT NULL DEFAULT 'preset_male',
            is_benchmark INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL DEFAULT 'Untitled Session',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(character_id) ON DELETE CASCADE,
            FOREIGN KEY (world_id) REFERENCES session_worlds(world_id) ON DELETE SET NULL,
            FOREIGN KEY (source_setting_id) REFERENCES settings(setting_id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS conversation_command_state (
            conversation_id TEXT PRIMARY KEY,
            pending_setup_text TEXT NOT NULL DEFAULT '',
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
            content TEXT NOT NULL,
            message_channel TEXT NOT NULL DEFAULT 'rp_scene',
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation_id_id
        ON messages(conversation_id, id);

        CREATE TABLE IF NOT EXISTS player_personas (
            persona_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            description TEXT NOT NULL,
            gender_code TEXT NOT NULL,
            pronouns TEXT NOT NULL,
            appearance TEXT,
            voice_style TEXT,
            boundaries TEXT,
            notes TEXT,
            is_archived INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS provider_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key TEXT NOT NULL,
            model TEXT NOT NULL,
            system_prompt TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS turn_snapshots (
            conversation_id TEXT NOT NULL,
            assistant_message_id INTEGER NOT NULL,
            user_text TEXT NOT NULL,
            soul_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (conversation_id, assistant_message_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (assistant_message_id) REFERENCES messages(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS assistant_message_variants (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            conversation_id TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            label TEXT,
            source TEXT,
            is_selected INTEGER NOT NULL DEFAULT 0,
            soul_snapshot_json TEXT,
            debug_json TEXT,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS llm_payload_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            message_id INTEGER,
            provider TEXT NOT NULL,
            mode TEXT NOT NULL,
            context_mode TEXT NOT NULL DEFAULT '',
            model TEXT NOT NULL,
            base_url TEXT NOT NULL,
            system_message TEXT NOT NULL,
            user_message TEXT NOT NULL,
            context_text TEXT NOT NULL,
            estimated_system_tokens INTEGER NOT NULL,
            estimated_user_tokens INTEGER NOT NULL,
            estimated_total_tokens INTEGER NOT NULL,
            truncated INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS conversation_entities (
            conversation_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            aliases_json TEXT NOT NULL DEFAULT '[]',
            kind TEXT NOT NULL DEFAULT 'unknown',
            controlled_by TEXT NOT NULL DEFAULT 'unknown',
            linked_soul_id TEXT,
            active_in_scene INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (conversation_id, entity_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_conversation_entities_active
        ON conversation_entities(conversation_id, active_in_scene);

        CREATE TABLE IF NOT EXISTS image_assets (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            thumbnail_path TEXT,
            source TEXT NOT NULL CHECK(source IN ('uploaded', 'generated', 'imported')),
            mime_type TEXT,
            width INTEGER,
            height INTEGER,
            prompt TEXT,
            provider TEXT,
            model TEXT,
            linked_soul_id TEXT,
            linked_conversation_id TEXT,
            linked_message_id INTEGER,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (linked_soul_id) REFERENCES souls(character_id) ON DELETE SET NULL,
            FOREIGN KEY (linked_conversation_id) REFERENCES conversations(id) ON DELETE SET NULL,
            FOREIGN KEY (linked_message_id) REFERENCES messages(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS message_attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_asset_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
            FOREIGN KEY (image_asset_id) REFERENCES image_assets(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_message_attachments_message_id
        ON message_attachments(message_id);

        CREATE TABLE IF NOT EXISTS session_branches (
            branch_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            base_soul_json TEXT NOT NULL,
            base_session_world_json TEXT NOT NULL,
            active_turn_id TEXT,
            rebuild_generation INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_session_branches_conversation_active
        ON session_branches(conversation_id, is_active);

        CREATE TABLE IF NOT EXISTS turn_commits (
            turn_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            parent_turn_id TEXT,
            user_message_id INTEGER,
            assistant_message_id INTEGER,
            state_patch_id TEXT,
            selected_variant_id INTEGER,
            created_at INTEGER NOT NULL,
            active_variant INTEGER NOT NULL DEFAULT 1,
            is_active INTEGER NOT NULL DEFAULT 1,
            is_discarded INTEGER NOT NULL DEFAULT 0,
            is_regenerated_variant INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE,
            FOREIGN KEY (parent_turn_id) REFERENCES turn_commits(turn_id) ON DELETE SET NULL,
            FOREIGN KEY (user_message_id) REFERENCES messages(id) ON DELETE SET NULL,
            FOREIGN KEY (assistant_message_id) REFERENCES messages(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_turn_commits_branch_active
        ON turn_commits(branch_id, is_active, created_at);

        CREATE TABLE IF NOT EXISTS state_patches (
            patch_id TEXT PRIMARY KEY,
            turn_id TEXT NOT NULL,
            parent_state_hash TEXT,
            patch_json TEXT NOT NULL,
            inverse_patch_json TEXT,
            applied_at INTEGER NOT NULL,
            applies_to TEXT NOT NULL DEFAULT 'session',
            is_active INTEGER NOT NULL DEFAULT 1,
            invalidated_by_patch_id TEXT,
            supersedes_patch_id TEXT,
            FOREIGN KEY (turn_id) REFERENCES turn_commits(turn_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_state_patches_turn_active
        ON state_patches(turn_id, is_active);
        ",
    )?;
    add_column_if_missing(
        conn,
        "state_patches",
        "patch_kind",
        "TEXT NOT NULL DEFAULT 'baseline'",
    )?;
    add_column_if_missing(conn, "state_patches", "parent_baseline_patch_id", "TEXT")?;
    add_column_if_missing(conn, "state_patches", "source_turn_id", "TEXT")?;
    add_column_if_missing(
        conn,
        "state_patches",
        "source_assistant_message_id",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "state_patches",
        "source_assistant_variant_id",
        "INTEGER",
    )?;
    add_column_if_missing(conn, "state_patches", "created_by_job_id", "TEXT")?;
    conn.execute(
        "
        CREATE INDEX IF NOT EXISTS idx_state_patches_source_turn_active
        ON state_patches(source_turn_id, is_active, patch_kind, applied_at)
        ",
        [],
    )?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "context_mode",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "truncated",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "souls",
        "soul_kind",
        "TEXT NOT NULL DEFAULT 'savepoint'",
    )?;
    add_column_if_missing(conn, "souls", "source_soul_id", "TEXT")?;
    add_column_if_missing(conn, "souls", "source_savepoint_id", "TEXT")?;
    let added_avatar_image_id = add_column_if_missing(conn, "souls", "avatar_image_id", "TEXT")?;
    let added_recent_count =
        add_column_if_missing(conn, "souls", "recent_count", "INTEGER NOT NULL DEFAULT 0")?;
    let added_core_count =
        add_column_if_missing(conn, "souls", "core_count", "INTEGER NOT NULL DEFAULT 0")?;
    let added_soul_summary_columns =
        added_avatar_image_id || added_recent_count || added_core_count;
    let added_setting_turn_counter = add_column_if_missing(
        conn,
        "settings",
        "turn_counter",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    let added_setting_location =
        add_column_if_missing(conn, "settings", "location", "TEXT NOT NULL DEFAULT ''")?;
    let added_setting_summary_columns = added_setting_turn_counter || added_setting_location;
    add_column_if_missing(
        conn,
        "conversations",
        "title",
        "TEXT NOT NULL DEFAULT 'Untitled Session'",
    )?;
    add_column_if_missing(conn, "conversations", "world_id", "TEXT")?;
    add_column_if_missing(conn, "conversations", "source_setting_id", "TEXT")?;
    add_column_if_missing(conn, "conversations", "archived_at", "INTEGER")?;
    add_column_if_missing(
        conn,
        "conversations",
        "active_player_persona_id",
        "TEXT NOT NULL DEFAULT 'preset_male'",
    )?;
    add_column_if_missing(
        conn,
        "conversations",
        "is_benchmark",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "messages", "branch_id", "TEXT")?;
    add_column_if_missing(
        conn,
        "messages",
        "message_channel",
        "TEXT NOT NULL DEFAULT 'rp_scene'",
    )?;
    add_column_if_missing(conn, "messages", "is_active", "INTEGER NOT NULL DEFAULT 1")?;
    add_column_if_missing(
        conn,
        "messages",
        "message_status",
        "TEXT NOT NULL DEFAULT 'active'",
    )?;
    add_column_if_missing(
        conn,
        "messages",
        "message_origin",
        "TEXT NOT NULL DEFAULT 'active'",
    )?;
    add_column_if_missing(conn, "messages", "hidden_at", "INTEGER")?;
    add_column_if_missing(conn, "souls", "archived_at", "INTEGER")?;
    add_column_if_missing(conn, "settings", "archived_at", "INTEGER")?;
    add_column_if_missing(conn, "assistant_message_variants", "turn_id", "TEXT")?;
    add_column_if_missing(conn, "assistant_message_variants", "state_patch_id", "TEXT")?;
    add_column_if_missing(
        conn,
        "assistant_message_variants",
        "is_discarded",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "llm_payload_logs", "branch_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "active_turn_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "parent_turn_id", "TEXT")?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "state_patch_ids_applied_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "discarded_patch_ids_skipped_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "state_rebuild_generation",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "latest_assistant_variant_id",
        "INTEGER",
    )?;
    add_column_if_missing(conn, "llm_payload_logs", "request_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "turn_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "raw_provider_response", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "normalized_response", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "finish_reason", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "provider_error", "TEXT")?;
    add_column_if_missing(
        conn,
        "llm_payload_logs",
        "fallback_used",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "llm_payload_logs", "fallback_reason", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "provider_request_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "provider_response_id", "TEXT")?;
    add_column_if_missing(conn, "llm_payload_logs", "pipeline_trace_json", "TEXT")?;

    // Migrate provider_profiles settings
    add_column_if_missing(conn, "provider_profiles", "narrator_timeout_ms", "INTEGER")?;
    add_column_if_missing(conn, "provider_profiles", "evaluator_timeout_ms", "INTEGER")?;
    add_column_if_missing(conn, "provider_profiles", "evaluator_timeout_mode", "TEXT")?;
    add_column_if_missing(conn, "provider_profiles", "evaluator_mode", "TEXT")?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "structured_evaluator_policy",
        "TEXT",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "wait_for_evaluator_before_next_turn",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "allow_send_with_stale_state",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_background_enabled",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "anti_replay_forced_retry_enabled",
        "INTEGER",
    )?;

    // Create evaluator_background_jobs table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evaluator_background_jobs (
            evaluator_job_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            assistant_message_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            completed_at INTEGER,
            elapsed_ms INTEGER,
            timeout_ms INTEGER,
            timeout_mode TEXT NOT NULL,
            model TEXT,
            provider TEXT,
            error_message TEXT,
            patch_applied INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_evaluator_background_jobs_conversation ON evaluator_background_jobs(conversation_id, status);"
    )?;

    if added_soul_summary_columns {
        backfill_soul_summary_columns(conn)?;
    }
    if added_setting_summary_columns {
        backfill_setting_summary_columns(conn)?;
    }

    add_column_if_missing(conn, "provider_profiles", "archived_at", "INTEGER")?;

    add_column_if_missing(
        conn,
        "provider_profiles",
        "narrator_compatibility_status",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_compatibility_status",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "command_compatibility_status",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_contract_version",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_prompt_version",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_last_tested_at",
        "INTEGER",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "evaluator_last_failure_reason",
        "TEXT",
    )?;
    add_column_if_missing(
        conn,
        "provider_profiles",
        "structured_output_support",
        "INTEGER NOT NULL DEFAULT 0",
    )?;

    add_column_if_missing(conn, "conversations", "active_evaluator_profile_id", "TEXT")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS conversation_evaluator_streaks (
            conversation_id TEXT NOT NULL,
            profile_id TEXT NOT NULL,
            empty_patch_streak INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (conversation_id, profile_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_evaluator_streaks ON conversation_evaluator_streaks(conversation_id, profile_id);"
    )?;

    // Dialogue-only exchanges skipped by the fast-mode evaluator gate; drained
    // into the next evaluator run as a catch-up block.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evaluator_catchup_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            user_message_id INTEGER,
            assistant_message_id INTEGER NOT NULL,
            user_text TEXT NOT NULL,
            assistant_text TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_evaluator_catchup_queue_conversation ON evaluator_catchup_queue(conversation_id);"
    )?;

    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|existing| existing == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
        return Ok(true);
    }
    Ok(false)
}

fn backfill_soul_summary_columns(conn: &Connection) -> rusqlite::Result<()> {
    let rows = {
        let mut stmt = conn.prepare("SELECT character_id, soul_json FROM souls")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (character_id, soul_json) in rows {
        let soul = decode_soul(&soul_json)?;
        conn.execute(
            "
            UPDATE souls
            SET avatar_image_id = ?1, recent_count = ?2, core_count = ?3
            WHERE character_id = ?4
            ",
            params![
                soul.profile.avatar_image_id.as_deref(),
                soul.memory.recent.len() as i64,
                soul.memory.core.len() as i64,
                character_id,
            ],
        )?;
    }
    Ok(())
}

fn backfill_setting_summary_columns(conn: &Connection) -> rusqlite::Result<()> {
    let rows = {
        let mut stmt = conn.prepare("SELECT setting_id, setting_json FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (setting_id, setting_json) in rows {
        let setting = decode_setting(&setting_json)?;
        conn.execute(
            "
            UPDATE settings
            SET turn_counter = ?1, location = ?2
            WHERE setting_id = ?3
            ",
            params![
                setting.turn_counter as i64,
                setting.world.location,
                setting_id,
            ],
        )?;
    }
    Ok(())
}

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn ledger_id(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        chrono::Utc::now().timestamp_millis(),
        LEDGER_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn normalized_soul_kind(kind: &str) -> String {
    match kind.trim() {
        "session_clone" => "session_clone".into(),
        "imported_package" => "imported_package".into(),
        "checkpoint" => "checkpoint".into(),
        _ => "savepoint".into(),
    }
}

fn summarize_soul(soul: &Soul) -> SoulSummary {
    SoulSummary {
        character_id: soul.character_id.clone(),
        character_name: soul.character_name.clone(),
        soul_kind: normalized_soul_kind(&soul.soul_kind),
        source_soul_id: soul.source_soul_id.clone(),
        source_savepoint_id: soul.source_savepoint_id.clone(),
        avatar_image_id: soul.profile.avatar_image_id.clone(),
        last_updated: soul.last_updated,
        recent_count: soul.memory.recent.len(),
        core_count: soul.memory.core.len(),
        archived_at: None,
    }
}

fn soul_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SoulSummary> {
    let recent_count: i64 = row.get(7)?;
    let core_count: i64 = row.get(8)?;
    Ok(SoulSummary {
        character_id: row.get(0)?,
        character_name: row.get(1)?,
        soul_kind: normalized_soul_kind(&row.get::<_, String>(2)?),
        source_soul_id: row.get(3)?,
        source_savepoint_id: row.get(4)?,
        avatar_image_id: row.get(5)?,
        last_updated: row.get(6)?,
        recent_count: recent_count.max(0) as usize,
        core_count: core_count.max(0) as usize,
        archived_at: row.get(9)?,
    })
}

pub fn upsert_soul(conn: &Connection, soul: &Soul) -> rusqlite::Result<SoulSummary> {
    let soul_json = serde_json::to_string(soul)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO souls
            (character_id, character_name, soul_kind, source_soul_id, source_savepoint_id, avatar_image_id, recent_count, core_count, last_updated, soul_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(character_id) DO UPDATE SET
            character_name = excluded.character_name,
            soul_kind = excluded.soul_kind,
            source_soul_id = excluded.source_soul_id,
            source_savepoint_id = excluded.source_savepoint_id,
            avatar_image_id = excluded.avatar_image_id,
            recent_count = excluded.recent_count,
            core_count = excluded.core_count,
            last_updated = excluded.last_updated,
            soul_json = excluded.soul_json
        ",
        params![
            soul.character_id,
            soul.character_name,
            normalized_soul_kind(&soul.soul_kind),
            soul.source_soul_id.as_deref(),
            soul.source_savepoint_id.as_deref(),
            soul.profile.avatar_image_id.as_deref(),
            soul.memory.recent.len() as i64,
            soul.memory.core.len() as i64,
            soul.last_updated,
            soul_json
        ],
    )?;

    let archived_at: Option<i64> = conn
        .query_row(
            "SELECT archived_at FROM souls WHERE character_id = ?1",
            [&soul.character_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    let mut summary = summarize_soul(soul);
    summary.archived_at = archived_at;
    Ok(summary)
}

pub fn upsert_setting(
    conn: &Connection,
    setting: &SettingSoul,
) -> rusqlite::Result<SettingSummary> {
    let setting_json = serde_json::to_string(setting)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO settings (setting_id, setting_name, turn_counter, location, last_updated, setting_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(setting_id) DO UPDATE SET
            setting_name = excluded.setting_name,
            turn_counter = excluded.turn_counter,
            location = excluded.location,
            last_updated = excluded.last_updated,
            setting_json = excluded.setting_json
        ",
        params![
            setting.setting_id,
            setting.setting_name,
            setting.turn_counter as i64,
            setting.world.location.as_str(),
            setting.last_updated,
            setting_json
        ],
    )?;

    Ok(summarize_setting(setting))
}

pub fn upsert_session_world(
    conn: &Connection,
    session_world: &SessionWorld,
) -> rusqlite::Result<SessionWorld> {
    let world_json = serde_json::to_string(session_world)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO session_worlds
            (world_id, source_setting_id, source_savepoint_id, world_kind, setting_name, location, last_updated, world_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(world_id) DO UPDATE SET
            source_setting_id = excluded.source_setting_id,
            source_savepoint_id = excluded.source_savepoint_id,
            world_kind = excluded.world_kind,
            setting_name = excluded.setting_name,
            location = excluded.location,
            last_updated = excluded.last_updated,
            world_json = excluded.world_json
        ",
        params![
            session_world.world_id,
            session_world.source_setting_id.as_deref(),
            session_world.source_savepoint_id.as_deref(),
            session_world.world_kind,
            session_world.setting_name,
            session_world.location,
            session_world.last_updated,
            world_json
        ],
    )?;
    get_session_world(conn, &session_world.world_id)
}

pub fn get_session_world(conn: &Connection, world_id: &str) -> rusqlite::Result<SessionWorld> {
    let world_json: String = conn.query_row(
        "SELECT world_json FROM session_worlds WHERE world_id = ?1",
        [world_id],
        |row| row.get(0),
    )?;
    decode_session_world(&world_json)
}

pub fn link_conversation_world(
    conn: &Connection,
    conversation_id: &str,
    world_id: &str,
    source_setting_id: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE conversations SET world_id = ?1, source_setting_id = ?2, updated_at = ?3 WHERE id = ?4",
        params![world_id, source_setting_id, now_ts(), conversation_id],
    )?;
    Ok(())
}

pub fn get_conversation_session_world(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Option<SessionWorld>> {
    let world_id: Option<String> = conn
        .query_row(
            "SELECT world_id FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    world_id
        .map(|world_id| get_session_world(conn, &world_id))
        .transpose()
}

pub fn create_session_world_from_setting(
    conn: &Connection,
    setting_id: &str,
) -> rusqlite::Result<SessionWorld> {
    let setting = get_setting(conn, setting_id)?;
    let session_world = session_world_from_setting(&setting);
    upsert_session_world(conn, &session_world)
}

pub fn create_legacy_session_world_from_soul(
    conn: &Connection,
    soul: &Soul,
) -> rusqlite::Result<SessionWorld> {
    let source = soul
        .source_savepoint_id
        .clone()
        .or_else(|| Some(soul.character_id.clone()));
    let session_world =
        session_world_from_legacy_world("Legacy Character World", source, &soul.world);
    upsert_session_world(conn, &session_world)
}

pub fn ensure_conversation_session_world(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
    setting_id: Option<&str>,
) -> rusqlite::Result<SessionWorld> {
    if let Some(existing) = get_conversation_session_world(conn, conversation_id)? {
        return Ok(existing);
    }
    let session_world = if let Some(setting_id) = setting_id
        .map(str::trim)
        .filter(|setting_id| !setting_id.is_empty())
    {
        create_session_world_from_setting(conn, setting_id)?
    } else {
        create_legacy_session_world_from_soul(conn, soul)?
    };
    link_conversation_world(
        conn,
        conversation_id,
        &session_world.world_id,
        session_world.source_setting_id.as_deref(),
    )?;
    Ok(session_world)
}

pub fn list_settings(conn: &Connection) -> rusqlite::Result<Vec<SettingSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT setting_id, setting_name, last_updated, turn_counter, location, archived_at
        FROM settings
        WHERE archived_at IS NULL
        ORDER BY last_updated DESC, setting_name ASC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let turn_counter: i64 = row.get(3)?;
        Ok(SettingSummary {
            setting_id: row.get(0)?,
            setting_name: row.get(1)?,
            last_updated: row.get(2)?,
            turn_counter: turn_counter.max(0) as u64,
            location: row.get(4)?,
            archived_at: row.get(5)?,
        })
    })?;

    rows.collect()
}

pub fn get_setting(conn: &Connection, setting_id: &str) -> rusqlite::Result<SettingSoul> {
    let setting_json: String = conn.query_row(
        "SELECT setting_json FROM settings WHERE setting_id = ?1",
        [setting_id],
        |row| row.get(0),
    )?;
    decode_setting(&setting_json)
}

pub fn delete_setting(_conn: &Connection, _setting_id: &str) -> rusqlite::Result<bool> {
    Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "delete_setting is deprecated; use archive_setting with active/default setting guard.",
        ),
    )))
}

#[allow(dead_code)]
pub(crate) fn delete_setting_internal(
    conn: &Connection,
    setting_id: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM settings WHERE setting_id = ?1", [setting_id])?;
    Ok(affected > 0)
}

pub fn archive_setting(
    conn: &Connection,
    setting_id: &str,
    active_or_default_ids: &[&str],
) -> Result<bool, String> {
    if active_or_default_ids.contains(&setting_id) {
        return Err(
            "Cannot archive the active/default setting. Switch settings first.".to_string(),
        );
    }

    let non_archived_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM settings WHERE archived_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;

    if non_archived_count <= 1 {
        let is_archived: Option<i64> = conn
            .query_row(
                "SELECT archived_at FROM settings WHERE setting_id = ?1",
                [setting_id],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        if is_archived.is_none() {
            return Err(
                "Cannot archive the active/default setting. Switch settings first.".to_string(),
            );
        }
    }

    let now = now_ts();
    let affected = conn
        .execute(
            "UPDATE settings SET archived_at = ?1 WHERE setting_id = ?2",
            params![Some(now), setting_id],
        )
        .map_err(|err| err.to_string())?;
    Ok(affected > 0)
}

pub fn restore_setting(conn: &Connection, setting_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE settings SET archived_at = NULL WHERE setting_id = ?1",
        [setting_id],
    )?;
    Ok(affected > 0)
}

pub fn list_archived_settings(conn: &Connection) -> rusqlite::Result<Vec<SettingSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT setting_id, setting_name, last_updated, turn_counter, location, archived_at
        FROM settings
        WHERE archived_at IS NOT NULL
        ORDER BY archived_at DESC, setting_name ASC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let turn_counter: i64 = row.get(3)?;
        Ok(SettingSummary {
            setting_id: row.get(0)?,
            setting_name: row.get(1)?,
            last_updated: row.get(2)?,
            turn_counter: turn_counter.max(0) as u64,
            location: row.get(4)?,
            archived_at: row.get(5)?,
        })
    })?;

    rows.collect()
}

pub fn list_souls(conn: &Connection) -> rusqlite::Result<Vec<SoulSummary>> {
    list_souls_with_filter(conn, false)
}

pub fn list_souls_including_session_clones(
    conn: &Connection,
) -> rusqlite::Result<Vec<SoulSummary>> {
    list_souls_with_filter(conn, true)
}

fn list_souls_with_filter(
    conn: &Connection,
    include_session_clones: bool,
) -> rusqlite::Result<Vec<SoulSummary>> {
    let sql = if include_session_clones {
        "
        SELECT character_id, character_name, soul_kind, source_soul_id, source_savepoint_id,
               avatar_image_id, last_updated, recent_count, core_count, archived_at
        FROM souls
        WHERE archived_at IS NULL
        ORDER BY last_updated DESC, character_name ASC
        "
    } else {
        "
        SELECT character_id, character_name, soul_kind, source_soul_id, source_savepoint_id,
               avatar_image_id, last_updated, recent_count, core_count, archived_at
        FROM souls
        WHERE soul_kind != 'session_clone' AND archived_at IS NULL
        ORDER BY last_updated DESC, character_name ASC
        "
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], soul_summary_from_row)?;

    rows.collect()
}

pub fn get_soul(conn: &Connection, soul_id: &str) -> rusqlite::Result<Soul> {
    let soul_json: String = conn.query_row(
        "SELECT soul_json FROM souls WHERE character_id = ?1",
        [soul_id],
        |row| row.get(0),
    )?;
    decode_soul(&soul_json)
}

pub fn archive_soul(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    let active_session_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE soul_id = ?1 AND archived_at IS NULL",
        [soul_id],
        |row| row.get(0),
    )?;
    if active_session_count > 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Cannot archive Soul while active sessions use it; archive the session first or save a copy.".into(),
        ));
    }
    let affected = conn.execute(
        "UPDATE souls SET archived_at = ?1 WHERE character_id = ?2",
        params![now_ts(), soul_id],
    )?;
    Ok(affected > 0)
}

pub fn restore_soul(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE souls SET archived_at = NULL WHERE character_id = ?1",
        params![soul_id],
    )?;
    Ok(affected > 0)
}

pub fn list_archived_souls(conn: &Connection) -> rusqlite::Result<Vec<SoulSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT character_id, character_name, soul_kind, source_soul_id, source_savepoint_id,
               avatar_image_id, last_updated, recent_count, core_count, archived_at
        FROM souls
        WHERE soul_kind NOT IN ('session_clone', 'checkpoint') AND archived_at IS NOT NULL
        ORDER BY archived_at DESC, character_name ASC
        ",
    )?;
    let rows = stmt.query_map([], soul_summary_from_row)?;
    rows.collect()
}

pub fn archive_savepoint(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    archive_soul(conn, soul_id)
}

pub fn restore_savepoint(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    restore_soul(conn, soul_id)
}

pub fn list_archived_savepoints(conn: &Connection) -> rusqlite::Result<Vec<SoulSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT character_id, character_name, soul_kind, source_soul_id, source_savepoint_id,
               avatar_image_id, last_updated, recent_count, core_count, archived_at
        FROM souls
        WHERE soul_kind = 'checkpoint' AND archived_at IS NOT NULL
        ORDER BY archived_at DESC, character_name ASC
        ",
    )?;
    let rows = stmt.query_map([], soul_summary_from_row)?;
    rows.collect()
}

pub fn delete_soul(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    let _ = (conn, soul_id);
    Err(rusqlite::Error::InvalidParameterName(
        "delete_soul is deprecated; use archive_soul with session safety guards.".into(),
    ))
}

pub fn hard_delete_soul_internal(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM souls WHERE character_id = ?1", [soul_id])?;
    Ok(affected > 0)
}

pub fn primary_soul(conn: &Connection) -> rusqlite::Result<Option<Soul>> {
    let soul_json: Option<String> = conn
        .query_row(
            "SELECT soul_json FROM souls WHERE soul_kind != 'session_clone' ORDER BY last_updated DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    soul_json.map(|json| decode_soul(&json)).transpose()
}

pub fn ensure_conversation(
    conn: &Connection,
    conversation_id: &str,
    soul_id: &str,
) -> rusqlite::Result<()> {
    ensure_conversation_with_title(conn, conversation_id, soul_id, None).map(|_| ())
}

pub fn ensure_conversation_with_title(
    conn: &Connection,
    conversation_id: &str,
    soul_id: &str,
    title: Option<&str>,
) -> rusqlite::Result<ConversationSummary> {
    ensure_conversation_with_title_and_world(conn, conversation_id, soul_id, None, None, title)
}

pub fn ensure_conversation_with_title_and_world(
    conn: &Connection,
    conversation_id: &str,
    soul_id: &str,
    world_id: Option<&str>,
    source_setting_id: Option<&str>,
    title: Option<&str>,
) -> rusqlite::Result<ConversationSummary> {
    let now = now_ts();
    let title = sanitize_conversation_title(title.unwrap_or("Untitled Session"));
    conn.execute(
        "
        INSERT INTO conversations (id, soul_id, world_id, source_setting_id, active_player_persona_id, title, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 'preset_male', ?5, ?6, ?6)
        ON CONFLICT(id) DO UPDATE SET
            soul_id = excluded.soul_id,
            world_id = COALESCE(excluded.world_id, conversations.world_id),
            source_setting_id = COALESCE(excluded.source_setting_id, conversations.source_setting_id),
            updated_at = excluded.updated_at
        ",
        params![conversation_id, soul_id, world_id, source_setting_id, title, now],
    )?;
    get_conversation_summary(conn, conversation_id)
}

pub fn get_pending_setup(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "
        SELECT pending_setup_text
        FROM conversation_command_state
        WHERE conversation_id = ?1
        ",
        [conversation_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.and_then(|text| (!text.trim().is_empty()).then_some(text)))
}

pub fn set_pending_setup(
    conn: &Connection,
    conversation_id: &str,
    pending_setup_text: &str,
) -> rusqlite::Result<()> {
    let text = pending_setup_text.trim();
    if text.is_empty() {
        clear_pending_setup(conn, conversation_id)?;
        return Ok(());
    }
    let now = now_ts();
    conn.execute(
        "
        INSERT INTO conversation_command_state (conversation_id, pending_setup_text, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(conversation_id) DO UPDATE SET
            pending_setup_text = excluded.pending_setup_text,
            updated_at = excluded.updated_at
        ",
        params![conversation_id, text, now],
    )?;
    Ok(())
}

pub fn clear_pending_setup(conn: &Connection, conversation_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM conversation_command_state WHERE conversation_id = ?1",
        [conversation_id],
    )?;
    Ok(())
}

pub fn rename_conversation(
    conn: &Connection,
    conversation_id: &str,
    title: &str,
) -> rusqlite::Result<ConversationSummary> {
    let title = sanitize_conversation_title(title);
    let updated = now_ts();
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, updated, conversation_id],
    )?;
    get_conversation_summary(conn, conversation_id)
}

pub fn list_conversations(conn: &Connection) -> rusqlite::Result<Vec<ConversationSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            c.id,
            c.title,
            c.soul_id,
            COALESCE(s.source_savepoint_id, NULL),
            c.world_id,
            c.source_setting_id,
            c.active_player_persona_id,
            c.created_at,
            c.updated_at,
            (
                SELECT content
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0 AND message_status = 'active'
                ORDER BY id DESC
                LIMIT 1
            ) AS last_message_preview,
            (
                SELECT COUNT(*)
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0 AND message_status = 'active'
            ) AS message_count,
            c.archived_at,
            c.active_evaluator_profile_id,
            c.is_benchmark
        FROM conversations c
        LEFT JOIN souls s ON s.character_id = c.soul_id
        WHERE c.archived_at IS NULL
        ORDER BY c.updated_at DESC, c.created_at DESC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let preview: Option<String> = row.get(9)?;
        Ok(ConversationSummary {
            conversation_id: row.get(0)?,
            title: row.get(1)?,
            soul_id: row.get(2)?,
            source_savepoint_id: row.get(3)?,
            world_id: row.get(4)?,
            source_setting_id: row.get(5)?,
            active_player_persona_id: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
            last_message_preview: preview.map(compact_preview),
            message_count: row.get(10)?,
            archived_at: row.get(11)?,
            active_evaluator_profile_id: row.get(12)?,
            is_benchmark: row.get::<_, i64>(13)? != 0,
        })
    })?;
    rows.collect()
}

pub fn get_conversation_summary(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<ConversationSummary> {
    conn.query_row(
        "
        SELECT c.id, c.title, c.soul_id, c.created_at, c.updated_at, COALESCE(s.source_savepoint_id, NULL), c.world_id, c.source_setting_id, c.active_player_persona_id, c.archived_at, c.active_evaluator_profile_id, c.is_benchmark
        FROM conversations c
        LEFT JOIN souls s ON s.character_id = c.soul_id
        WHERE c.id = ?1
        ",
        [conversation_id],
        |row| {
            let conversation_id: String = row.get(0)?;
            let last_message_preview = last_message_preview(conn, &conversation_id)?;
            let message_count = count_messages(conn, &conversation_id)?;
            Ok(ConversationSummary {
                conversation_id,
                title: row.get(1)?,
                soul_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                source_savepoint_id: row.get(5)?,
                world_id: row.get(6)?,
                source_setting_id: row.get(7)?,
                active_player_persona_id: row.get(8)?,
                last_message_preview,
                message_count,
                archived_at: row.get(9)?,
                active_evaluator_profile_id: row.get(10)?,
                is_benchmark: row.get::<_, i64>(11)? != 0,
            })
        },
    )
}

pub fn mark_conversation_benchmark(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE conversations SET is_benchmark = 1, updated_at = ?1 WHERE id = ?2",
        params![now_ts(), conversation_id],
    )?;
    Ok(affected > 0)
}

fn sanitize_conversation_title(title: &str) -> String {
    let trimmed = title.trim();
    let title = if trimmed.is_empty() {
        "Untitled Session"
    } else {
        trimmed
    };
    title.chars().take(120).collect()
}

fn compact_preview(content: String) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(140).collect()
}

pub fn built_in_player_personas() -> Vec<PlayerPersona> {
    vec![
        PlayerPersona {
            persona_id: "preset_male".into(),
            display_name: "Male Persona".into(),
            description: "User-controlled male RP persona. No additional traits specified.".into(),
            gender_code: "male".into(),
            pronouns: "he/him".into(),
            is_builtin: true,
            is_archived: false,
            created_at: 0,
            updated_at: 0,
            appearance: None,
            voice_style: None,
            boundaries: None,
            notes: None,
        },
        PlayerPersona {
            persona_id: "preset_female".into(),
            display_name: "Female Persona".into(),
            description: "User-controlled female RP persona. No additional traits specified."
                .into(),
            gender_code: "female".into(),
            pronouns: "she/her".into(),
            is_builtin: true,
            is_archived: false,
            created_at: 0,
            updated_at: 0,
            appearance: None,
            voice_style: None,
            boundaries: None,
            notes: None,
        },
    ]
}

pub fn list_player_personas(conn: &Connection) -> rusqlite::Result<Vec<PlayerPersona>> {
    let mut personas = built_in_player_personas();
    let mut stmt = conn.prepare(
        "
        SELECT persona_id, display_name, description, gender_code, pronouns, appearance, voice_style, boundaries, notes, is_archived, created_at, updated_at
        FROM player_personas
        WHERE is_archived = 0
        ORDER BY display_name COLLATE NOCASE ASC
        ",
    )?;
    let rows = stmt.query_map([], player_persona_from_row)?;
    for row in rows {
        personas.push(row?);
    }
    Ok(personas)
}

pub fn get_player_persona(
    conn: &Connection,
    persona_id: &str,
) -> rusqlite::Result<Option<PlayerPersona>> {
    if let Some(persona) = built_in_player_personas()
        .into_iter()
        .find(|persona| persona.persona_id == persona_id)
    {
        return Ok(Some(persona));
    }
    conn.query_row(
        "
        SELECT persona_id, display_name, description, gender_code, pronouns, appearance, voice_style, boundaries, notes, is_archived, created_at, updated_at
        FROM player_personas
        WHERE persona_id = ?1
        ",
        [persona_id],
        player_persona_from_row,
    )
    .optional()
}

pub fn find_player_persona(
    conn: &Connection,
    lookup: &str,
) -> rusqlite::Result<Option<PlayerPersona>> {
    let lookup = lookup.trim();
    if lookup.is_empty() {
        return Ok(None);
    }
    let normalized = normalize_lookup(lookup);
    if let Some(persona) = built_in_player_personas().into_iter().find(|persona| {
        normalize_lookup(&persona.persona_id) == normalized
            || normalize_lookup(&persona.display_name) == normalized
    }) {
        return Ok(Some(persona));
    }
    conn.query_row(
        "
        SELECT persona_id, display_name, description, gender_code, pronouns, appearance, voice_style, boundaries, notes, is_archived, created_at, updated_at
        FROM player_personas
        WHERE is_archived = 0
          AND (lower(persona_id) = lower(?1) OR lower(display_name) = lower(?1))
        LIMIT 1
        ",
        [lookup],
        player_persona_from_row,
    )
    .optional()
}

pub fn upsert_player_persona(
    conn: &Connection,
    persona: &PlayerPersona,
) -> rusqlite::Result<PlayerPersona> {
    if persona.is_builtin {
        return Ok(persona.clone());
    }
    let now = now_ts();
    let created_at = if persona.created_at > 0 {
        persona.created_at
    } else {
        now
    };
    conn.execute(
        "
        INSERT INTO player_personas
            (persona_id, display_name, description, gender_code, pronouns, appearance, voice_style, boundaries, notes, is_archived, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(persona_id) DO UPDATE SET
            display_name = excluded.display_name,
            description = excluded.description,
            gender_code = excluded.gender_code,
            pronouns = excluded.pronouns,
            appearance = excluded.appearance,
            voice_style = excluded.voice_style,
            boundaries = excluded.boundaries,
            notes = excluded.notes,
            is_archived = excluded.is_archived,
            updated_at = excluded.updated_at
        ",
        params![
            persona.persona_id.trim(),
            persona.display_name.trim(),
            persona.description.trim(),
            persona.gender_code.trim(),
            persona.pronouns.trim(),
            persona.appearance.as_deref(),
            persona.voice_style.as_deref(),
            persona.boundaries.as_deref(),
            persona.notes.as_deref(),
            if persona.is_archived { 1 } else { 0 },
            created_at,
            now,
        ],
    )?;
    get_player_persona(conn, persona.persona_id.trim())?
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
}

pub fn archive_player_persona(conn: &Connection, persona_id: &str) -> rusqlite::Result<bool> {
    if built_in_player_personas()
        .iter()
        .any(|persona| persona.persona_id == persona_id)
    {
        return Ok(false);
    }
    let affected = conn.execute(
        "UPDATE player_personas SET is_archived = 1, updated_at = ?1 WHERE persona_id = ?2",
        params![now_ts(), persona_id],
    )?;
    Ok(affected > 0)
}

pub fn restore_player_persona(conn: &Connection, persona_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE player_personas SET is_archived = 0, updated_at = ?1 WHERE persona_id = ?2",
        params![now_ts(), persona_id],
    )?;
    Ok(affected > 0)
}

pub fn get_active_player_persona_id(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT active_player_persona_id FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| {
        value
            .and_then(|id| (!id.trim().is_empty()).then_some(id))
            .unwrap_or_else(|| "preset_male".into())
    })
}

pub fn get_active_player_persona(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<PlayerPersona> {
    let persona_id = get_active_player_persona_id(conn, conversation_id)?;
    get_player_persona(conn, &persona_id).map(|persona| {
        persona.unwrap_or_else(|| {
            built_in_player_personas()
                .into_iter()
                .find(|persona| persona.persona_id == "preset_male")
                .expect("built-in male persona exists")
        })
    })
}

pub fn set_active_player_persona(
    conn: &Connection,
    conversation_id: &str,
    persona_id: &str,
) -> rusqlite::Result<PlayerPersona> {
    let persona = get_player_persona(conn, persona_id)?
        .filter(|persona| !persona.is_archived)
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
    conn.execute(
        "UPDATE conversations SET active_player_persona_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![persona.persona_id.as_str(), now_ts(), conversation_id],
    )?;
    Ok(persona)
}

fn player_persona_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlayerPersona> {
    Ok(PlayerPersona {
        persona_id: row.get(0)?,
        display_name: row.get(1)?,
        description: row.get(2)?,
        gender_code: row.get(3)?,
        pronouns: row.get(4)?,
        appearance: row.get(5)?,
        voice_style: row.get(6)?,
        boundaries: row.get(7)?,
        notes: row.get(8)?,
        is_archived: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        is_builtin: false,
    })
}

fn normalize_lookup(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn last_message_preview(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "
        SELECT content FROM messages
        WHERE conversation_id = ?1 AND is_active != 0 AND message_status = 'active'
        ORDER BY id DESC
        LIMIT 1
        ",
        [conversation_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.map(compact_preview))
}

fn count_messages(conn: &Connection, conversation_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND is_active != 0 AND message_status = 'active'",
        [conversation_id],
        |row| row.get(0),
    )
}

pub fn upsert_entity(conn: &Connection, entity: &EntityRecord) -> rusqlite::Result<EntityRecord> {
    let now = now_ts();
    let aliases_json = encode_aliases(&entity.aliases)?;
    conn.execute(
        "
        INSERT INTO conversation_entities
            (conversation_id, entity_id, display_name, aliases_json, kind, controlled_by, linked_soul_id, active_in_scene, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
        ON CONFLICT(conversation_id, entity_id) DO UPDATE SET
            display_name = excluded.display_name,
            aliases_json = excluded.aliases_json,
            kind = excluded.kind,
            controlled_by = excluded.controlled_by,
            linked_soul_id = excluded.linked_soul_id,
            active_in_scene = excluded.active_in_scene,
            updated_at = excluded.updated_at
        ",
        params![
            entity.conversation_id,
            entity.entity_id,
            entity.display_name,
            aliases_json,
            entity.kind,
            entity.controlled_by,
            entity.linked_soul_id,
            if entity.active_in_scene { 1 } else { 0 },
            now
        ],
    )?;
    get_entity(conn, &entity.conversation_id, &entity.entity_id)
}

pub fn get_entity(
    conn: &Connection,
    conversation_id: &str,
    entity_id: &str,
) -> rusqlite::Result<EntityRecord> {
    conn.query_row(
        "
        SELECT conversation_id, entity_id, display_name, aliases_json, kind, controlled_by, linked_soul_id, active_in_scene, created_at, updated_at
        FROM conversation_entities
        WHERE conversation_id = ?1 AND entity_id = ?2
        ",
        params![conversation_id, entity_id],
        entity_from_row,
    )
}

pub fn list_entities(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<EntityRecord>> {
    let mut stmt = conn.prepare(
        "
        SELECT conversation_id, entity_id, display_name, aliases_json, kind, controlled_by, linked_soul_id, active_in_scene, created_at, updated_at
        FROM conversation_entities
        WHERE conversation_id = ?1
        ORDER BY active_in_scene DESC, display_name COLLATE NOCASE ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], entity_from_row)?;
    rows.collect()
}

pub fn add_entity_alias(
    conn: &Connection,
    conversation_id: &str,
    entity_id: &str,
    alias: &str,
) -> rusqlite::Result<EntityRecord> {
    let mut entity = get_entity(conn, conversation_id, entity_id)?;
    let alias = alias.trim();
    if !alias.is_empty()
        && !entity
            .aliases
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(alias))
    {
        entity.aliases.push(alias.to_string());
    }
    upsert_entity(conn, &entity)
}

pub fn insert_message(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> rusqlite::Result<()> {
    insert_message_with_channel(
        conn,
        conversation_id,
        role,
        content,
        MESSAGE_CHANNEL_RP_SCENE,
    )
}

pub fn insert_message_with_channel(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    channel: &str,
) -> rusqlite::Result<()> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, message_channel, created_at, message_status, message_origin) VALUES (?1, ?2, ?3, ?4, ?5, 'active', 'active')",
        params![conversation_id, role, content, normalize_message_channel(channel), now],
    )?;
    let _message_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )?;
    Ok(())
}

pub fn insert_message_and_get_id(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> rusqlite::Result<i64> {
    insert_message_with_channel_and_get_id(
        conn,
        conversation_id,
        role,
        content,
        MESSAGE_CHANNEL_RP_SCENE,
    )
}

pub fn insert_message_with_channel_and_get_id(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    channel: &str,
) -> rusqlite::Result<i64> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, message_channel, created_at, message_status, message_origin) VALUES (?1, ?2, ?3, ?4, ?5, 'active', 'active')",
        params![conversation_id, role, content, normalize_message_channel(channel), now],
    )?;
    let message_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )?;
    Ok(message_id)
}

fn normalize_message_channel(channel: &str) -> &str {
    match channel {
        MESSAGE_CHANNEL_RP_SCENE
        | MESSAGE_CHANNEL_COMMAND_OOC
        | MESSAGE_CHANNEL_COMMAND_SETUP
        | MESSAGE_CHANNEL_COMMAND_STATE
        | MESSAGE_CHANNEL_COMMAND_PERSONA
        | MESSAGE_CHANNEL_COMMAND_ASK
        | MESSAGE_CHANNEL_COMMAND_HELP
        | MESSAGE_CHANNEL_SYSTEM_DEBUG => channel,
        _ => MESSAGE_CHANNEL_RP_SCENE,
    }
}

pub fn find_reusable_active_user_message(
    conn: &Connection,
    conversation_id: &str,
    content: &str,
) -> rusqlite::Result<Option<i64>> {
    let normalized = normalize_message_content(content);
    if normalized.is_empty() {
        return Ok(None);
    }
    let latest: Option<(i64, String, String)> = conn
        .query_row(
            "
            SELECT id, role, content
            FROM messages
            WHERE conversation_id = ?1 AND is_active != 0 AND message_status = 'active'
            ORDER BY id DESC
            LIMIT 1
            ",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((message_id, role, latest_content)) = latest else {
        return Ok(None);
    };
    if role != "user" || normalize_message_content(&latest_content) != normalized {
        return Ok(None);
    }
    let assistant_after: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM messages
        WHERE conversation_id = ?1
          AND id > ?2
          AND role = 'assistant'
          AND is_active != 0
          AND message_status = 'active'
        ",
        params![conversation_id, message_id],
        |row| row.get(0),
    )?;
    Ok((assistant_after == 0).then_some(message_id))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DedupeAdjacentUserMessagesResult {
    pub canonical_user_message_ids: Vec<i64>,
    pub hidden_duplicate_user_message_ids: Vec<i64>,
}

pub fn dedupe_active_adjacent_user_messages(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<DedupeAdjacentUserMessagesResult> {
    let messages = list_messages(conn, conversation_id, 100_000)?;
    let mut result = DedupeAdjacentUserMessagesResult::default();
    let mut last_user: Option<(i64, String)> = None;
    for message in messages {
        if message.role == "assistant" {
            last_user = None;
            continue;
        }
        if message.role != "user" {
            continue;
        }
        let normalized = normalize_message_content(&message.content);
        if normalized.is_empty() {
            last_user = Some((message.id, normalized));
            continue;
        }
        if let Some((canonical_id, previous_normalized)) = last_user.as_ref() {
            if *previous_normalized == normalized {
                conn.execute(
                    "
                    UPDATE messages
                    SET is_active = 0,
                        message_status = 'duplicate_hidden',
                        message_origin = 'duplicate_hidden'
                    WHERE conversation_id = ?1 AND id = ?2
                    ",
                    params![conversation_id, message.id],
                )?;
                conn.execute(
                    "
                    UPDATE turn_commits
                    SET user_message_id = ?1
                    WHERE conversation_id = ?2 AND user_message_id = ?3
                    ",
                    params![canonical_id, conversation_id, message.id],
                )?;
                result.canonical_user_message_ids.push(*canonical_id);
                result.hidden_duplicate_user_message_ids.push(message.id);
                continue;
            }
        }
        last_user = Some((message.id, normalized));
    }
    if !result.hidden_duplicate_user_message_ids.is_empty() {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }
    Ok(result)
}

fn normalize_message_content(content: &str) -> String {
    content
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn update_message_content(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    content: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE messages SET content = ?1 WHERE conversation_id = ?2 AND id = ?3 AND role = 'assistant'",
        params![content, conversation_id, message_id],
    )?;
    if affected > 0 {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }
    Ok(affected > 0)
}

pub fn update_user_message_content(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    content: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE messages SET content = ?1 WHERE conversation_id = ?2 AND id = ?3 AND role = 'user'",
        params![content, conversation_id, message_id],
    )?;
    if affected > 0 {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }
    Ok(affected > 0)
}

pub fn upsert_image_asset(conn: &Connection, asset: &ImageAsset) -> rusqlite::Result<ImageAsset> {
    conn.execute(
        "
        INSERT INTO image_assets
            (id, file_path, thumbnail_path, source, mime_type, width, height, prompt, provider, model, linked_soul_id, linked_conversation_id, linked_message_id, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(id) DO UPDATE SET
            file_path = excluded.file_path,
            thumbnail_path = excluded.thumbnail_path,
            source = excluded.source,
            mime_type = excluded.mime_type,
            width = excluded.width,
            height = excluded.height,
            prompt = excluded.prompt,
            provider = excluded.provider,
            model = excluded.model,
            linked_soul_id = excluded.linked_soul_id,
            linked_conversation_id = excluded.linked_conversation_id,
            linked_message_id = excluded.linked_message_id
        ",
        params![
            asset.id,
            asset.file_path,
            asset.thumbnail_path,
            asset.source,
            asset.mime_type,
            asset.width,
            asset.height,
            asset.prompt,
            asset.provider,
            asset.model,
            asset.linked_soul_id,
            asset.linked_conversation_id,
            asset.linked_message_id,
            asset.created_at,
        ],
    )?;
    get_image_asset(conn, &asset.id)
}

pub fn get_image_asset(conn: &Connection, image_asset_id: &str) -> rusqlite::Result<ImageAsset> {
    conn.query_row(
        "
        SELECT id, file_path, thumbnail_path, source, mime_type, width, height, prompt, provider, model, linked_soul_id, linked_conversation_id, linked_message_id, created_at
        FROM image_assets
        WHERE id = ?1
        ",
        [image_asset_id],
        image_asset_from_row,
    )
}

pub fn attach_image_to_message(
    conn: &Connection,
    message_id: i64,
    image_asset_id: &str,
) -> rusqlite::Result<MessageAttachment> {
    let now = now_ts();
    conn.execute(
        "
        INSERT INTO message_attachments (message_id, image_asset_id, created_at)
        VALUES (?1, ?2, ?3)
        ",
        params![message_id, image_asset_id, now],
    )?;
    let attachment_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE image_assets SET linked_message_id = ?1 WHERE id = ?2",
        params![message_id, image_asset_id],
    )?;
    get_message_attachment(conn, attachment_id)
}

pub fn list_message_attachments(
    conn: &Connection,
    message_id: i64,
) -> rusqlite::Result<Vec<MessageAttachment>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            ma.id, ma.message_id, ma.image_asset_id, ma.created_at,
            ia.id, ia.file_path, ia.thumbnail_path, ia.source, ia.mime_type, ia.width, ia.height,
            ia.prompt, ia.provider, ia.model, ia.linked_soul_id, ia.linked_conversation_id,
            ia.linked_message_id, ia.created_at
        FROM message_attachments ma
        JOIN image_assets ia ON ia.id = ma.image_asset_id
        WHERE ma.message_id = ?1
        ORDER BY ma.id ASC
        ",
    )?;
    let rows = stmt.query_map([message_id], message_attachment_from_row)?;
    rows.collect()
}

fn get_message_attachment(
    conn: &Connection,
    attachment_id: i64,
) -> rusqlite::Result<MessageAttachment> {
    conn.query_row(
        "
        SELECT
            ma.id, ma.message_id, ma.image_asset_id, ma.created_at,
            ia.id, ia.file_path, ia.thumbnail_path, ia.source, ia.mime_type, ia.width, ia.height,
            ia.prompt, ia.provider, ia.model, ia.linked_soul_id, ia.linked_conversation_id,
            ia.linked_message_id, ia.created_at
        FROM message_attachments ma
        JOIN image_assets ia ON ia.id = ma.image_asset_id
        WHERE ma.id = ?1
        ",
        [attachment_id],
        message_attachment_from_row,
    )
}

pub fn list_assistant_message_variants(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<Vec<AssistantMessageVariant>> {
    let message = get_message(conn, conversation_id, message_id)?;
    if message.role != "assistant" {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "
        SELECT id, message_id, conversation_id, content, created_at, label, source, is_selected, soul_snapshot_json, debug_json
        FROM assistant_message_variants
        WHERE conversation_id = ?1 AND message_id = ?2 AND COALESCE(is_discarded, 0) = 0
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map(params![conversation_id, message_id], variant_from_row)?;
    let variants = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    if variants.is_empty() {
        Ok(vec![AssistantMessageVariant {
            id: None,
            message_id: message.id,
            conversation_id: message.conversation_id,
            content: message.content,
            created_at: message.created_at,
            label: Some("Variant 1".into()),
            source: Some("legacy".into()),
            is_selected: true,
            soul_snapshot_json: None,
            debug_json: None,
        }])
    } else {
        Ok(variants)
    }
}

pub fn create_assistant_message_variant(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    content: &str,
    label: Option<&str>,
    source: Option<&str>,
    select: bool,
    soul_snapshot_json: Option<&str>,
    debug_json: Option<&str>,
) -> rusqlite::Result<AssistantMessageVariant> {
    let message = get_message(conn, conversation_id, message_id)?;
    if message.role != "assistant" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    ensure_base_assistant_variant(conn, &message)?;

    let now = now_ts();
    if select {
        conn.execute(
            "UPDATE assistant_message_variants SET is_selected = 0 WHERE message_id = ?1",
            [message_id],
        )?;
    }
    conn.execute(
        "
        INSERT INTO assistant_message_variants
            (message_id, conversation_id, content, created_at, label, source, is_selected, soul_snapshot_json, debug_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            message_id,
            conversation_id,
            content,
            now,
            label,
            source,
            if select { 1 } else { 0 },
            soul_snapshot_json,
            debug_json
        ],
    )?;
    let variant_id = conn.last_insert_rowid();
    if select {
        update_message_content(conn, conversation_id, message_id, content)?;
    }
    get_assistant_variant(conn, conversation_id, variant_id)
}

pub fn seed_initial_assistant_message_variant(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    content: &str,
    source: Option<&str>,
    soul_snapshot_json: Option<&str>,
    debug_json: Option<&str>,
) -> rusqlite::Result<AssistantMessageVariant> {
    let message = get_message(conn, conversation_id, message_id)?;
    if message.role != "assistant" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let existing_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM assistant_message_variants WHERE message_id = ?1",
        [message_id],
        |row| row.get(0),
    )?;
    if existing_count > 0 {
        let selected_id: i64 = conn.query_row(
            "
            SELECT id
            FROM assistant_message_variants
            WHERE message_id = ?1 AND is_selected = 1
            ORDER BY id ASC
            LIMIT 1
            ",
            [message_id],
            |row| row.get(0),
        )?;
        return get_assistant_variant(conn, conversation_id, selected_id);
    }

    conn.execute(
        "
        INSERT INTO assistant_message_variants
            (message_id, conversation_id, content, created_at, label, source, is_selected, soul_snapshot_json, debug_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)
        ",
        params![
            message_id,
            conversation_id,
            content,
            message.created_at,
            "Variant 1",
            source,
            soul_snapshot_json,
            debug_json
        ],
    )?;
    get_assistant_variant(conn, conversation_id, conn.last_insert_rowid())
}

pub fn insert_imported_assistant_message_variant(
    conn: &Connection,
    variant: &AssistantMessageVariant,
) -> rusqlite::Result<AssistantMessageVariant> {
    let message = get_message(conn, &variant.conversation_id, variant.message_id)?;
    if message.role != "assistant" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if variant.is_selected {
        conn.execute(
            "UPDATE assistant_message_variants SET is_selected = 0 WHERE message_id = ?1",
            [variant.message_id],
        )?;
    }
    conn.execute(
        "
        INSERT INTO assistant_message_variants
            (message_id, conversation_id, content, created_at, label, source, is_selected, soul_snapshot_json, debug_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            variant.message_id,
            variant.conversation_id,
            variant.content,
            variant.created_at,
            variant.label,
            variant.source,
            if variant.is_selected { 1 } else { 0 },
            variant.soul_snapshot_json,
            variant.debug_json
        ],
    )?;
    let imported = get_assistant_variant(conn, &variant.conversation_id, conn.last_insert_rowid())?;
    if variant.is_selected {
        update_message_content(
            conn,
            &variant.conversation_id,
            variant.message_id,
            &variant.content,
        )?;
    }
    Ok(imported)
}

pub fn select_assistant_message_variant(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    variant_id: i64,
) -> rusqlite::Result<AssistantMessageVariant> {
    let variant = get_assistant_variant(conn, conversation_id, variant_id)?;
    if variant.message_id != message_id {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    conn.execute(
        "UPDATE assistant_message_variants SET is_selected = 0 WHERE message_id = ?1",
        [message_id],
    )?;
    conn.execute(
        "UPDATE assistant_message_variants SET is_selected = 1 WHERE id = ?1",
        [variant_id],
    )?;
    update_message_content(conn, conversation_id, message_id, &variant.content)?;
    get_assistant_variant(conn, conversation_id, variant_id)
}

pub fn delete_assistant_message_variant(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
    variant_id: i64,
) -> rusqlite::Result<bool> {
    let variants = list_assistant_message_variants(conn, conversation_id, message_id)?;
    let real_count = variants
        .iter()
        .filter(|variant| variant.id.is_some())
        .count();
    if real_count <= 1 {
        return Ok(false);
    }
    let was_selected = variants
        .iter()
        .any(|variant| variant.id == Some(variant_id) && variant.is_selected);
    let affected = conn.execute(
        "DELETE FROM assistant_message_variants WHERE conversation_id = ?1 AND message_id = ?2 AND id = ?3",
        params![conversation_id, message_id, variant_id],
    )?;
    if affected > 0 && was_selected {
        if let Some(next) = list_assistant_message_variants(conn, conversation_id, message_id)?
            .into_iter()
            .find(|variant| variant.id.is_some())
        {
            if let Some(next_id) = next.id {
                select_assistant_message_variant(conn, conversation_id, message_id, next_id)?;
            }
        }
    }
    Ok(affected > 0)
}

pub fn insert_llm_payload_log(conn: &Connection, log: &LlmPayloadLog) -> rusqlite::Result<i64> {
    conn.execute(
        "
        INSERT INTO llm_payload_logs
            (conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id, request_id, turn_id, raw_provider_response, normalized_response, finish_reason, provider_error, fallback_used, fallback_reason, provider_request_id, provider_response_id, pipeline_trace_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)
        ",
        params![
            log.conversation_id,
            log.message_id,
            log.provider,
            log.mode,
            log.context_mode,
            log.model,
            log.base_url,
            log.system_message,
            log.user_message,
            log.context_text,
            log.estimated_system_tokens as i64,
            log.estimated_user_tokens as i64,
            log.estimated_total_tokens as i64,
            if log.truncated { 1 } else { 0 },
            log.created_at,
            log.branch_id,
            log.active_turn_id,
            log.parent_turn_id,
            serde_json::to_string(&log.state_patch_ids_applied).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&log.discarded_patch_ids_skipped).unwrap_or_else(|_| "[]".into()),
            log.state_rebuild_generation,
            log.latest_assistant_variant_id,
            log.request_id,
            log.turn_id,
            log.raw_provider_response,
            log.normalized_response,
            log.finish_reason,
            log.provider_error,
            if log.fallback_used { 1 } else { 0 },
            log.fallback_reason,
            log.provider_request_id,
            log.provider_response_id,
            log.pipeline_trace_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_llm_payload_log_response(
    conn: &Connection,
    log_id: i64,
    update: &LlmPayloadResponseUpdate,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "
        UPDATE llm_payload_logs
        SET request_id = COALESCE(?1, request_id),
            turn_id = COALESCE(?2, turn_id),
            raw_provider_response = COALESCE(?3, raw_provider_response),
            normalized_response = COALESCE(?4, normalized_response),
            finish_reason = COALESCE(?5, finish_reason),
            provider_error = COALESCE(?6, provider_error),
            fallback_used = COALESCE(?7, fallback_used),
            fallback_reason = COALESCE(?8, fallback_reason),
            provider_request_id = COALESCE(?9, provider_request_id),
            provider_response_id = COALESCE(?10, provider_response_id),
            pipeline_trace_json = COALESCE(?11, pipeline_trace_json)
        WHERE id = ?12
        ",
        params![
            update.request_id,
            update.turn_id,
            update.raw_provider_response,
            update.normalized_response,
            update.finish_reason,
            update.provider_error,
            update
                .fallback_used
                .map(|fallback_used| if fallback_used { 1 } else { 0 }),
            update.fallback_reason,
            update.provider_request_id,
            update.provider_response_id,
            update.pipeline_trace_json,
            log_id,
        ],
    )?;
    Ok(affected > 0)
}

pub fn update_assistant_variant_debug_json(
    conn: &Connection,
    variant_id: i64,
    debug_json: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE assistant_message_variants SET debug_json = ?1 WHERE id = ?2",
        params![debug_json, variant_id],
    )?;
    Ok(affected > 0)
}

pub fn set_llm_payload_log_message_id(
    conn: &Connection,
    log_id: i64,
    message_id: i64,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE llm_payload_logs SET message_id = ?1 WHERE id = ?2",
        params![message_id, log_id],
    )?;
    Ok(affected > 0)
}

pub fn set_llm_payload_log_ledger_metadata(
    conn: &Connection,
    log_id: i64,
    debug: &BranchPatchDebug,
    parent_turn_id: Option<&str>,
    latest_assistant_variant_id: Option<i64>,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "
        UPDATE llm_payload_logs
        SET branch_id = ?1,
            active_turn_id = ?2,
            parent_turn_id = ?3,
            state_patch_ids_applied_json = ?4,
            discarded_patch_ids_skipped_json = ?5,
            state_rebuild_generation = ?6,
            latest_assistant_variant_id = ?7
        WHERE id = ?8
        ",
        params![
            debug.branch_id,
            debug.active_turn_id,
            parent_turn_id,
            serde_json::to_string(&debug.applied_patches).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&debug.skipped_discarded_patches).unwrap_or_else(|_| "[]".into()),
            debug.rebuild_generation,
            latest_assistant_variant_id,
            log_id
        ],
    )?;
    Ok(affected > 0)
}

pub fn list_llm_payload_logs(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<LlmPayloadLog>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id, request_id, turn_id, raw_provider_response, normalized_response, finish_reason, provider_error, fallback_used, fallback_reason, provider_request_id, provider_response_id, pipeline_trace_json
        FROM llm_payload_logs
        WHERE conversation_id = ?1
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], llm_payload_log_from_row)?;
    rows.collect()
}

pub fn get_llm_payload_log(conn: &Connection, log_id: i64) -> rusqlite::Result<LlmPayloadLog> {
    conn.query_row(
        "
        SELECT id, conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id, request_id, turn_id, raw_provider_response, normalized_response, finish_reason, provider_error, fallback_used, fallback_reason, provider_request_id, provider_response_id, pipeline_trace_json
        FROM llm_payload_logs
        WHERE id = ?1
        ",
        [log_id],
        llm_payload_log_from_row,
    )
}

pub fn list_messages_before_id(
    conn: &Connection,
    conversation_id: &str,
    before_message_id: i64,
    limit: usize,
) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
        FROM (
            SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
            FROM messages
            WHERE conversation_id = ?1 AND id < ?2 AND is_active != 0 AND message_status = 'active'
            ORDER BY id DESC
            LIMIT ?3
        )
        ORDER BY id ASC
        ",
    )?;

    let rows = stmt.query_map(
        params![conversation_id, before_message_id, limit as i64],
        |row| {
            let message_id = row.get(0)?;
            Ok(ChatMessage {
                id: message_id,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                channel: row.get(4)?,
                created_at: row.get(5)?,
                status: row.get(6)?,
                origin: row.get(7)?,
                attachments: list_message_attachments(conn, message_id)?,
                hidden_at: row.get(8)?,
            })
        },
    )?;

    rows.collect()
}

pub fn archive_session(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    let title: Option<String> = conn
        .query_row(
            "SELECT title FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(title) = title else {
        return Ok(false);
    };
    let archived_title = if title.starts_with("[Archived] ") {
        title
    } else {
        format!("[Archived] {title}")
    };
    let affected = conn.execute(
        "UPDATE conversations SET title = ?1, archived_at = ?2, updated_at = ?3 WHERE id = ?4",
        params![archived_title, now_ts(), now_ts(), conversation_id],
    )?;
    Ok(affected > 0)
}

pub fn restore_session(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    let title: Option<String> = conn
        .query_row(
            "SELECT title FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(title) = title else {
        return Ok(false);
    };
    let restored_title = if title.starts_with("[Archived] ") {
        title.replacen("[Archived] ", "", 1)
    } else {
        title
    };
    let affected = conn.execute(
        "UPDATE conversations SET title = ?1, archived_at = NULL, updated_at = ?2 WHERE id = ?3",
        params![restored_title, now_ts(), conversation_id],
    )?;
    Ok(affected > 0)
}

pub fn list_archived_sessions(conn: &Connection) -> rusqlite::Result<Vec<ConversationSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            c.id,
            c.title,
            c.soul_id,
            COALESCE(s.source_savepoint_id, NULL),
            c.world_id,
            c.source_setting_id,
            c.active_player_persona_id,
            c.created_at,
            c.updated_at,
            (
                SELECT content
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0 AND message_status = 'active'
                ORDER BY id DESC
                LIMIT 1
            ) AS last_message_preview,
            (
                SELECT COUNT(*)
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0 AND message_status = 'active'
            ) AS message_count,
            c.archived_at,
            c.active_evaluator_profile_id,
            c.is_benchmark
        FROM conversations c
        LEFT JOIN souls s ON s.character_id = c.soul_id
        WHERE c.archived_at IS NOT NULL
        ORDER BY c.archived_at DESC, c.updated_at DESC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let preview: Option<String> = row.get(9)?;
        Ok(ConversationSummary {
            conversation_id: row.get(0)?,
            title: row.get(1)?,
            soul_id: row.get(2)?,
            source_savepoint_id: row.get(3)?,
            world_id: row.get(4)?,
            source_setting_id: row.get(5)?,
            active_player_persona_id: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
            last_message_preview: preview.map(compact_preview),
            message_count: row.get(10)?,
            archived_at: row.get(11)?,
            active_evaluator_profile_id: row.get(12)?,
            is_benchmark: row.get::<_, i64>(13)? != 0,
        })
    })?;
    rows.collect()
}

pub fn delete_conversation(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    archive_session(conn, conversation_id)
}

pub fn hide_turn_range(
    conn: &Connection,
    conversation_id: &str,
    start_message_id: i64,
    end_message_id: i64,
) -> rusqlite::Result<usize> {
    let affected = conn.execute(
        "UPDATE messages
         SET hidden_at = ?1, is_active = 0, message_status = 'hidden'
         WHERE conversation_id = ?2 AND id >= ?3 AND id <= ?4",
        params![now_ts(), conversation_id, start_message_id, end_message_id],
    )?;

    conn.execute(
        "UPDATE turn_commits SET is_active = 0, is_discarded = 1, active_variant = 0
         WHERE conversation_id = ?1
           AND (
             (user_message_id >= ?2 AND user_message_id <= ?3)
             OR (assistant_message_id >= ?2 AND assistant_message_id <= ?3)
           )",
        params![conversation_id, start_message_id, end_message_id],
    )?;

    conn.execute(
        "UPDATE state_patches SET is_active = 0
         WHERE turn_id IN (
             SELECT turn_id FROM turn_commits
             WHERE conversation_id = ?1
               AND (
                 (user_message_id >= ?2 AND user_message_id <= ?3)
                 OR (assistant_message_id >= ?2 AND assistant_message_id <= ?3)
               )
         )",
        params![conversation_id, start_message_id, end_message_id],
    )?;

    if affected > 0 {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }

    Ok(affected)
}

pub fn restore_turn_range(
    conn: &Connection,
    conversation_id: &str,
    start_message_id: i64,
    end_message_id: i64,
) -> rusqlite::Result<usize> {
    let Some(branch) = get_active_session_branch(conn, conversation_id).optional()? else {
        return Ok(0);
    };

    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE branch_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([&branch.branch_id], turn_commit_from_row)?;
    let mut commits_by_id = HashMap::new();
    let mut latest_turn_id = None;
    for row in rows {
        let commit = row?;
        latest_turn_id = Some(commit.turn_id.clone());
        commits_by_id.insert(commit.turn_id.clone(), commit);
    }

    let Some(mut cursor) = branch.active_turn_id.clone().or(latest_turn_id) else {
        return Ok(0);
    };
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    while seen.insert(cursor.clone()) {
        let Some(commit) = commits_by_id.get(&cursor).cloned() else {
            break;
        };
        cursor = commit.parent_turn_id.clone().unwrap_or_default();
        path.push(commit);
        if cursor.is_empty() {
            break;
        }
    }
    path.reverse();

    let canonical_message_ids = path
        .iter()
        .flat_map(|commit| [commit.user_message_id, commit.assistant_message_id])
        .flatten()
        .collect::<HashSet<_>>();

    let mut stmt = conn.prepare(
        "
        SELECT id, role, content, message_status
        FROM messages
        WHERE conversation_id = ?1 AND is_active = 0 AND id >= ?2 AND id <= ?3
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map(
        params![conversation_id, start_message_id, end_message_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;

    let mut restored_message_ids = Vec::new();
    let mut canonical_user_by_group = HashMap::<String, i64>::new();
    for commit in &path {
        if let Some(message_id) = commit.user_message_id {
            canonical_user_by_group
                .entry(turn_request_group_key(commit))
                .or_insert(message_id);
        }
    }

    for row in rows {
        let (message_id, role, _content, status) = row?;
        match status.as_str() {
            "pending"
            | "failed"
            | "retry_attempt"
            | "regenerated_discarded"
            | "duplicate_hidden" => {
                // skip
            }
            _ if !canonical_message_ids.contains(&message_id) => {
                // skip if not in our branch's canonical path
            }
            _ => {
                let is_duplicate_user = role == "user"
                    && path.iter().any(|commit| {
                        commit.user_message_id == Some(message_id)
                            && canonical_user_by_group
                                .get(&turn_request_group_key(commit))
                                .is_some_and(|canonical_id| *canonical_id != message_id)
                    });
                if !is_duplicate_user {
                    restored_message_ids.push(message_id);
                }
            }
        }
    }

    for message_id in &restored_message_ids {
        conn.execute(
            "UPDATE messages SET is_active = 1, message_status = 'active', hidden_at = NULL WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
        )?;
    }

    for commit in &path {
        let has_restored_msg = commit
            .user_message_id
            .map(|id| restored_message_ids.contains(&id))
            .unwrap_or(false)
            || commit
                .assistant_message_id
                .map(|id| restored_message_ids.contains(&id))
                .unwrap_or(false);
        if has_restored_msg {
            conn.execute(
                "UPDATE turn_commits SET is_active = 1, is_discarded = 0, active_variant = 1 WHERE turn_id = ?1 AND conversation_id = ?2",
                params![commit.turn_id, conversation_id],
            )?;
            conn.execute(
                "UPDATE state_patches SET is_active = 1 WHERE turn_id = ?1",
                [&commit.turn_id],
            )?;
        }
    }

    if !restored_message_ids.is_empty() {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }

    Ok(restored_message_ids.len())
}

pub fn list_hidden_turns(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
        FROM messages
        WHERE conversation_id = ?1 AND (hidden_at IS NOT NULL OR message_status = 'hidden')
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        let message_id: i64 = row.get(0)?;
        Ok(ChatMessage {
            id: message_id,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            channel: row.get(4)?,
            created_at: row.get(5)?,
            status: row.get(6)?,
            origin: row.get(7)?,
            attachments: list_message_attachments(conn, message_id)?,
            hidden_at: row.get(8)?,
        })
    })?;
    rows.collect()
}

pub fn delete_message(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<bool> {
    let count = hide_turn_range(conn, conversation_id, message_id, message_id)?;
    Ok(count > 0)
}

pub fn hard_delete_message_internal(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1 AND id = ?2",
        params![conversation_id, message_id],
    )?;
    if affected > 0 {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }
    Ok(affected > 0)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RestoreInactiveMessagesResult {
    pub restored_message_ids: Vec<i64>,
    pub skipped_duplicate_ids: Vec<i64>,
    pub skipped_pending_ids: Vec<i64>,
    pub skipped_failed_ids: Vec<i64>,
    pub skipped_retry_attempt_ids: Vec<i64>,
    pub skipped_regenerated_discarded_ids: Vec<i64>,
}

pub fn restore_inactive_messages(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<RestoreInactiveMessagesResult> {
    let mut result = RestoreInactiveMessagesResult::default();
    let Some(branch) = get_active_session_branch(conn, conversation_id).optional()? else {
        let mut preview = restore_preview_for_inactive_messages(conn, conversation_id, &[])?;
        let restored = restore_inactive_messages_legacy_all(conn, conversation_id)?;
        preview.restored_message_ids = restored;
        return Ok(preview);
    };

    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE branch_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([&branch.branch_id], turn_commit_from_row)?;
    let mut commits_by_id = HashMap::new();
    let mut latest_turn_id = None;
    for row in rows {
        let commit = row?;
        latest_turn_id = Some(commit.turn_id.clone());
        commits_by_id.insert(commit.turn_id.clone(), commit);
    }

    let Some(mut cursor) = branch.active_turn_id.clone().or(latest_turn_id) else {
        return Ok(result);
    };
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    while seen.insert(cursor.clone()) {
        let Some(commit) = commits_by_id.get(&cursor).cloned() else {
            break;
        };
        cursor = commit.parent_turn_id.clone().unwrap_or_default();
        path.push(commit);
        if cursor.is_empty() {
            break;
        }
    }
    path.reverse();

    let canonical_message_ids = path
        .iter()
        .flat_map(|commit| [commit.user_message_id, commit.assistant_message_id])
        .flatten()
        .collect::<HashSet<_>>();

    let mut stmt = conn.prepare(
        "
        SELECT id, role, content, message_status
        FROM messages
        WHERE conversation_id = ?1 AND is_active = 0
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut restored_message_ids = Vec::new();
    let mut canonical_user_by_group = HashMap::<String, i64>::new();
    for commit in &path {
        if let Some(message_id) = commit.user_message_id {
            canonical_user_by_group
                .entry(turn_request_group_key(commit))
                .or_insert(message_id);
        }
    }

    for row in rows {
        let (message_id, role, _content, status) = row?;
        match status.as_str() {
            "pending" => result.skipped_pending_ids.push(message_id),
            "failed" => result.skipped_failed_ids.push(message_id),
            "retry_attempt" => result.skipped_retry_attempt_ids.push(message_id),
            "regenerated_discarded" => result.skipped_regenerated_discarded_ids.push(message_id),
            "duplicate_hidden" => result.skipped_duplicate_ids.push(message_id),
            _ if !canonical_message_ids.contains(&message_id) => {
                if role == "user" {
                    result.skipped_duplicate_ids.push(message_id);
                }
            }
            _ => {
                let is_duplicate_user = role == "user"
                    && path.iter().any(|commit| {
                        commit.user_message_id == Some(message_id)
                            && canonical_user_by_group
                                .get(&turn_request_group_key(commit))
                                .is_some_and(|canonical_id| *canonical_id != message_id)
                    });
                if is_duplicate_user {
                    result.skipped_duplicate_ids.push(message_id);
                } else {
                    restored_message_ids.push(message_id);
                }
            }
        }
    }

    for message_id in &restored_message_ids {
        conn.execute(
            "UPDATE messages SET is_active = 1, message_status = 'active', hidden_at = NULL WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
        )?;
        result.restored_message_ids.push(*message_id);
    }

    for commit in &path {
        let has_restored_msg = commit
            .user_message_id
            .map(|id| restored_message_ids.contains(&id))
            .unwrap_or(false)
            || commit
                .assistant_message_id
                .map(|id| restored_message_ids.contains(&id))
                .unwrap_or(false);
        if has_restored_msg {
            conn.execute(
                "UPDATE turn_commits SET is_active = 1, is_discarded = 0, active_variant = 1 WHERE turn_id = ?1 AND conversation_id = ?2",
                params![commit.turn_id, conversation_id],
            )?;
            conn.execute(
                "UPDATE state_patches SET is_active = 1 WHERE turn_id = ?1",
                [&commit.turn_id],
            )?;
        }
    }

    if !restored_message_ids.is_empty() {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }

    Ok(result)
}

fn restore_preview_for_inactive_messages(
    conn: &Connection,
    conversation_id: &str,
    canonical_commits: &[TurnCommit],
) -> rusqlite::Result<RestoreInactiveMessagesResult> {
    let mut result = RestoreInactiveMessagesResult::default();
    let mut canonical_by_message_id = HashSet::new();
    let mut canonical_user_by_group = HashMap::<String, i64>::new();
    for commit in canonical_commits {
        if let Some(message_id) = commit.user_message_id {
            canonical_by_message_id.insert(message_id);
            canonical_user_by_group
                .entry(turn_request_group_key(commit))
                .or_insert(message_id);
        }
        if let Some(message_id) = commit.assistant_message_id {
            canonical_by_message_id.insert(message_id);
        }
    }
    let mut stmt = conn.prepare(
        "
        SELECT id, role, content, message_status
        FROM messages
        WHERE conversation_id = ?1 AND is_active = 0
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (message_id, role, _content, status) = row?;
        match status.as_str() {
            "pending" => result.skipped_pending_ids.push(message_id),
            "failed" => result.skipped_failed_ids.push(message_id),
            "retry_attempt" => result.skipped_retry_attempt_ids.push(message_id),
            "regenerated_discarded" => result.skipped_regenerated_discarded_ids.push(message_id),
            "duplicate_hidden" => result.skipped_duplicate_ids.push(message_id),
            _ if !canonical_by_message_id.contains(&message_id) => {
                if role == "user" {
                    result.skipped_duplicate_ids.push(message_id);
                }
            }
            _ => {
                let is_duplicate_user = role == "user"
                    && canonical_commits.iter().any(|commit| {
                        commit.user_message_id == Some(message_id)
                            && canonical_user_by_group
                                .get(&turn_request_group_key(commit))
                                .is_some_and(|canonical_id| *canonical_id != message_id)
                    });
                if is_duplicate_user {
                    result.skipped_duplicate_ids.push(message_id);
                } else {
                    result.restored_message_ids.push(message_id);
                }
            }
        }
    }
    Ok(result)
}

fn turn_request_group_key(commit: &TurnCommit) -> String {
    commit
        .parent_turn_id
        .clone()
        .unwrap_or_else(|| "__root__".into())
}

pub fn restore_inactive_messages_legacy_all(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<i64>> {
    conn.execute(
        "
        UPDATE turn_commits
        SET is_active = 1, is_discarded = 0, active_variant = 1
        WHERE conversation_id = ?1
          AND (
            user_message_id IN (
                SELECT id FROM messages WHERE conversation_id = ?1 AND is_active = 0
            )
            OR assistant_message_id IN (
                SELECT id FROM messages WHERE conversation_id = ?1 AND is_active = 0
            )
          )
          AND (
            selected_variant_id IS NULL
            OR selected_variant_id IN (
                SELECT id
                FROM assistant_message_variants
                WHERE conversation_id = ?1 AND is_selected != 0
            )
          )
        ",
        [conversation_id],
    )?;
    conn.execute(
        "
        UPDATE state_patches
        SET is_active = 1
        WHERE patch_id IN (
            SELECT state_patch_id
            FROM turn_commits
            WHERE conversation_id = ?1 AND is_active != 0 AND state_patch_id IS NOT NULL
        )
        ",
        [conversation_id],
    )?;
    let ids = inactive_restorable_message_ids(conn, conversation_id)?;
    for message_id in &ids {
        conn.execute(
            "UPDATE messages SET is_active = 1, message_status = 'active', message_origin = 'restored' WHERE conversation_id = ?1 AND id = ?2 AND message_status NOT IN ('pending', 'failed', 'retry_attempt', 'regenerated_discarded', 'duplicate_hidden')",
            params![conversation_id, message_id],
        )?;
    }
    if !ids.is_empty() {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_ts(), conversation_id],
        )?;
    }
    Ok(ids)
}

pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
    limit: usize,
) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
        FROM (
            SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
            FROM messages
            WHERE conversation_id = ?1 AND is_active != 0 AND message_status = 'active'
            ORDER BY id DESC
            LIMIT ?2
        )
        ORDER BY id ASC
        ",
    )?;

    let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
        let message_id = row.get(0)?;
        Ok(ChatMessage {
            id: message_id,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            channel: row.get(4)?,
            created_at: row.get(5)?,
            status: row.get(6)?,
            origin: row.get(7)?,
            attachments: list_message_attachments(conn, message_id)?,
            hidden_at: row.get(8)?,
        })
    })?;

    rows.collect()
}

pub fn get_message(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<ChatMessage> {
    conn.query_row(
        "
        SELECT id, conversation_id, role, content, message_channel, created_at, message_status, message_origin, hidden_at
        FROM messages
        WHERE conversation_id = ?1 AND id = ?2
        ",
        params![conversation_id, message_id],
        |row| {
            let message_id = row.get(0)?;
            Ok(ChatMessage {
                id: message_id,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                channel: row.get(4)?,
                created_at: row.get(5)?,
                status: row.get(6)?,
                origin: row.get(7)?,
                attachments: list_message_attachments(conn, message_id)?,
                hidden_at: row.get(8)?,
            })
        },
    )
}

fn image_asset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImageAsset> {
    Ok(ImageAsset {
        id: row.get(0)?,
        file_path: row.get(1)?,
        thumbnail_path: row.get(2)?,
        source: row.get(3)?,
        mime_type: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        prompt: row.get(7)?,
        provider: row.get(8)?,
        model: row.get(9)?,
        linked_soul_id: row.get(10)?,
        linked_conversation_id: row.get(11)?,
        linked_message_id: row.get(12)?,
        created_at: row.get(13)?,
    })
}

fn message_attachment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageAttachment> {
    Ok(MessageAttachment {
        id: row.get(0)?,
        message_id: row.get(1)?,
        image_asset_id: row.get(2)?,
        created_at: row.get(3)?,
        image: ImageAsset {
            id: row.get(4)?,
            file_path: row.get(5)?,
            thumbnail_path: row.get(6)?,
            source: row.get(7)?,
            mime_type: row.get(8)?,
            width: row.get(9)?,
            height: row.get(10)?,
            prompt: row.get(11)?,
            provider: row.get(12)?,
            model: row.get(13)?,
            linked_soul_id: row.get(14)?,
            linked_conversation_id: row.get(15)?,
            linked_message_id: row.get(16)?,
            created_at: row.get(17)?,
        },
    })
}

fn ensure_base_assistant_variant(conn: &Connection, message: &ChatMessage) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM assistant_message_variants WHERE message_id = ?1",
        [message.id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Ok(());
    }
    conn.execute(
        "
        INSERT INTO assistant_message_variants
            (message_id, conversation_id, content, created_at, label, source, is_selected)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
        ",
        params![
            message.id,
            message.conversation_id,
            message.content,
            message.created_at,
            "Variant 1",
            "original"
        ],
    )?;
    Ok(())
}

fn get_assistant_variant(
    conn: &Connection,
    conversation_id: &str,
    variant_id: i64,
) -> rusqlite::Result<AssistantMessageVariant> {
    conn.query_row(
        "
        SELECT id, message_id, conversation_id, content, created_at, label, source, is_selected, soul_snapshot_json, debug_json
        FROM assistant_message_variants
        WHERE conversation_id = ?1 AND id = ?2
        ",
        params![conversation_id, variant_id],
        variant_from_row,
    )
}

fn variant_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssistantMessageVariant> {
    let is_selected: i64 = row.get(7)?;
    Ok(AssistantMessageVariant {
        id: Some(row.get(0)?),
        message_id: row.get(1)?,
        conversation_id: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        label: row.get(5)?,
        source: row.get(6)?,
        is_selected: is_selected != 0,
        soul_snapshot_json: row.get(8)?,
        debug_json: row.get(9)?,
    })
}

fn llm_payload_log_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LlmPayloadLog> {
    let estimated_system_tokens: i64 = row.get(11)?;
    let estimated_user_tokens: i64 = row.get(12)?;
    let estimated_total_tokens: i64 = row.get(13)?;
    let truncated: i64 = row.get(14)?;
    let applied_json: String = row.get(19).unwrap_or_else(|_| "[]".into());
    let skipped_json: String = row.get(20).unwrap_or_else(|_| "[]".into());
    Ok(LlmPayloadLog {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        message_id: row.get(2)?,
        provider: row.get(3)?,
        mode: row.get(4)?,
        context_mode: row.get(5)?,
        model: row.get(6)?,
        base_url: row.get(7)?,
        system_message: row.get(8)?,
        user_message: row.get(9)?,
        context_text: row.get(10)?,
        estimated_system_tokens: estimated_system_tokens.max(0) as usize,
        estimated_user_tokens: estimated_user_tokens.max(0) as usize,
        estimated_total_tokens: estimated_total_tokens.max(0) as usize,
        truncated: truncated != 0,
        created_at: row.get(15)?,
        branch_id: row.get(16).ok(),
        active_turn_id: row.get(17).ok(),
        parent_turn_id: row.get(18).ok(),
        state_patch_ids_applied: serde_json::from_str(&applied_json).unwrap_or_default(),
        discarded_patch_ids_skipped: serde_json::from_str(&skipped_json).unwrap_or_default(),
        state_rebuild_generation: row.get(21).ok(),
        latest_assistant_variant_id: row.get(22).ok(),
        request_id: row.get(23).ok(),
        turn_id: row.get(24).ok(),
        raw_provider_response: row.get(25).ok(),
        normalized_response: row.get(26).ok(),
        finish_reason: row.get(27).ok(),
        provider_error: row.get(28).ok(),
        fallback_used: row.get::<_, i64>(29).unwrap_or(0) != 0,
        fallback_reason: row.get(30).ok(),
        provider_request_id: row.get(31).ok(),
        provider_response_id: row.get(32).ok(),
        pipeline_trace_json: row.get(33).ok(),
    })
}

pub fn count_assistant_messages(conn: &Connection, conversation_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND role = 'assistant' AND is_active != 0",
        [conversation_id],
        |row| row.get(0),
    )
}

pub fn upsert_turn_snapshot(conn: &Connection, snapshot: &TurnSnapshot) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO turn_snapshots (conversation_id, assistant_message_id, user_text, soul_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(conversation_id, assistant_message_id) DO UPDATE SET
            user_text = excluded.user_text,
            soul_json = excluded.soul_json,
            created_at = excluded.created_at
        ",
        params![
            snapshot.conversation_id,
            snapshot.assistant_message_id,
            snapshot.user_text,
            snapshot.soul_json,
            now_ts()
        ],
    )?;
    Ok(())
}

pub fn get_turn_snapshot(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: i64,
) -> rusqlite::Result<Option<TurnSnapshot>> {
    conn.query_row(
        "
        SELECT conversation_id, assistant_message_id, user_text, soul_json
        FROM turn_snapshots
        WHERE conversation_id = ?1 AND assistant_message_id = ?2
        ",
        params![conversation_id, assistant_message_id],
        |row| {
            Ok(TurnSnapshot {
                conversation_id: row.get(0)?,
                assistant_message_id: row.get(1)?,
                user_text: row.get(2)?,
                soul_json: row.get(3)?,
            })
        },
    )
    .optional()
}

pub fn create_session_branch(
    conn: &Connection,
    conversation_id: &str,
    base_soul: &Soul,
    base_world: &SessionWorld,
) -> rusqlite::Result<SessionBranch> {
    let existing = get_active_session_branch(conn, conversation_id).optional()?;
    if let Some(existing) = existing {
        return Ok(existing);
    }
    let now = now_ts();
    let branch = SessionBranch {
        branch_id: ledger_id("branch"),
        conversation_id: conversation_id.to_string(),
        base_soul_json: serde_json::to_string(base_soul)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
        base_session_world_json: serde_json::to_string(base_world)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
        active_turn_id: None,
        rebuild_generation: 0,
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    conn.execute(
        "
        INSERT INTO session_branches
            (branch_id, conversation_id, base_soul_json, base_session_world_json, active_turn_id, rebuild_generation, is_active, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)
        ",
        params![
            branch.branch_id,
            branch.conversation_id,
            branch.base_soul_json,
            branch.base_session_world_json,
            branch.active_turn_id,
            branch.rebuild_generation,
            branch.created_at,
            branch.updated_at
        ],
    )?;
    get_active_session_branch(conn, conversation_id)
}

pub fn get_active_session_branch(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<SessionBranch> {
    conn.query_row(
        "
        SELECT branch_id, conversation_id, base_soul_json, base_session_world_json, active_turn_id, rebuild_generation, is_active, created_at, updated_at
        FROM session_branches
        WHERE conversation_id = ?1 AND is_active = 1
        ORDER BY created_at DESC
        LIMIT 1
        ",
        [conversation_id],
        session_branch_from_row,
    )
}

pub fn list_session_branches_for_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<SessionBranch>> {
    let mut stmt = conn.prepare(
        "
        SELECT branch_id, conversation_id, base_soul_json, base_session_world_json, active_turn_id, rebuild_generation, is_active, created_at, updated_at
        FROM session_branches
        WHERE conversation_id = ?1
        ORDER BY is_active DESC, created_at ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], session_branch_from_row)?;
    rows.collect()
}

pub fn has_session_branch(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    Ok(get_active_session_branch(conn, conversation_id)
        .optional()?
        .is_some())
}

pub fn get_active_turn_id(conn: &Connection, branch_id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT active_turn_id FROM session_branches WHERE branch_id = ?1",
        [branch_id],
        |row| row.get(0),
    )
}

pub fn get_turn_commit_by_assistant(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: i64,
) -> rusqlite::Result<Option<TurnCommit>> {
    conn.query_row(
        "
        SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant
        FROM turn_commits
        WHERE conversation_id = ?1 AND assistant_message_id = ?2 AND is_active = 1
        ORDER BY created_at DESC
        LIMIT 1
        ",
        params![conversation_id, assistant_message_id],
        turn_commit_from_row,
    )
    .optional()
}

pub fn list_turn_commits_for_branch(
    conn: &Connection,
    branch_id: &str,
) -> rusqlite::Result<Vec<TurnCommit>> {
    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE branch_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([branch_id], turn_commit_from_row)?;
    rows.collect()
}

pub fn list_state_patches_for_branch(
    conn: &Connection,
    branch_id: &str,
) -> rusqlite::Result<Vec<StatePatchRecord>> {
    let mut stmt = conn.prepare(
        "
        SELECT sp.patch_id, sp.turn_id, sp.parent_state_hash, sp.patch_json, sp.inverse_patch_json,
               sp.applied_at, sp.applies_to, sp.is_active, sp.invalidated_by_patch_id,
               sp.supersedes_patch_id, sp.patch_kind, sp.parent_baseline_patch_id,
               sp.source_turn_id, sp.source_assistant_message_id, sp.source_assistant_variant_id,
               sp.created_by_job_id
        FROM state_patches sp
        JOIN turn_commits tc ON tc.turn_id = sp.turn_id
        WHERE tc.branch_id = ?1
        ORDER BY tc.created_at ASC,
                 CASE sp.patch_kind WHEN 'baseline' THEN 0 WHEN 'enrichment' THEN 1 ELSE 2 END,
                 sp.applied_at ASC,
                 sp.patch_id ASC
        ",
    )?;
    let rows = stmt.query_map([branch_id], state_patch_from_row)?;
    rows.collect()
}

pub fn insert_imported_session_branch(
    conn: &Connection,
    branch: &SessionBranch,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE session_branches SET is_active = 0 WHERE conversation_id = ?1",
        [branch.conversation_id.as_str()],
    )?;
    conn.execute(
        "
        INSERT INTO session_branches
            (branch_id, conversation_id, base_soul_json, base_session_world_json, active_turn_id, rebuild_generation, is_active, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            branch.branch_id,
            branch.conversation_id,
            branch.base_soul_json,
            branch.base_session_world_json,
            branch.active_turn_id,
            branch.rebuild_generation,
            if branch.is_active { 1 } else { 0 },
            branch.created_at,
            branch.updated_at
        ],
    )?;
    Ok(())
}

pub fn insert_imported_turn_commit(conn: &Connection, commit: &TurnCommit) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO turn_commits
            (turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ",
        params![
            commit.turn_id,
            commit.conversation_id,
            commit.branch_id,
            commit.parent_turn_id,
            commit.user_message_id,
            commit.assistant_message_id,
            commit.state_patch_id,
            commit.selected_variant_id,
            commit.created_at,
            if commit.active_variant { 1 } else { 0 },
            if commit.is_active { 1 } else { 0 },
            if commit.is_discarded { 1 } else { 0 },
            if commit.is_regenerated_variant { 1 } else { 0 }
        ],
    )?;
    Ok(())
}

pub fn insert_imported_state_patch(
    conn: &Connection,
    patch: &StatePatchRecord,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO state_patches
            (patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id, patch_kind, parent_baseline_patch_id, source_turn_id, source_assistant_message_id, source_assistant_variant_id, created_by_job_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ",
        params![
            patch.patch_id,
            patch.turn_id,
            patch.parent_state_hash,
            patch.patch_json,
            patch.inverse_patch_json,
            patch.applied_at,
            patch.applies_to,
            if patch.is_active { 1 } else { 0 },
            patch.invalidated_by_patch_id,
            patch.supersedes_patch_id,
            patch.patch_kind,
            patch.parent_baseline_patch_id,
            patch.source_turn_id,
            patch.source_assistant_message_id,
            patch.source_assistant_variant_id,
            patch.created_by_job_id
        ],
    )?;
    Ok(())
}

pub fn record_turn_commit_with_patch(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    parent_turn_id: Option<&str>,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    selected_variant_id: Option<i64>,
    patch: &EnginePatch,
    is_regenerated_variant: bool,
) -> rusqlite::Result<(TurnCommit, StatePatchRecord)> {
    record_turn_commit_with_patch_for_turn_id(
        conn,
        &ledger_id("turn"),
        conversation_id,
        branch_id,
        parent_turn_id,
        user_message_id,
        assistant_message_id,
        selected_variant_id,
        patch,
        is_regenerated_variant,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn record_turn_commit_with_patch_for_turn_id(
    conn: &Connection,
    turn_id: &str,
    conversation_id: &str,
    branch_id: &str,
    parent_turn_id: Option<&str>,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    selected_variant_id: Option<i64>,
    patch: &EnginePatch,
    is_regenerated_variant: bool,
) -> rusqlite::Result<(TurnCommit, StatePatchRecord)> {
    let now = now_ts();
    let patch_id = ledger_id("patch");
    let patch_json = serde_json::to_string(patch)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO turn_commits
            (turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 1, 0, ?10)
        ",
        params![
            turn_id,
            conversation_id,
            branch_id,
            parent_turn_id,
            user_message_id,
            assistant_message_id,
            patch_id,
            selected_variant_id,
            now,
            if is_regenerated_variant { 1 } else { 0 }
        ],
    )?;
    conn.execute(
        "
        INSERT INTO state_patches
            (patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id, patch_kind, parent_baseline_patch_id, source_turn_id, source_assistant_message_id, source_assistant_variant_id, created_by_job_id)
        VALUES (?1, ?2, NULL, ?3, NULL, ?4, 'session', 1, NULL, NULL, 'baseline', NULL, ?2, ?5, ?6, NULL)
        ",
        params![patch_id, turn_id, patch_json, now, assistant_message_id, selected_variant_id],
    )?;
    conn.execute(
        "UPDATE session_branches SET active_turn_id = ?1, updated_at = ?2 WHERE branch_id = ?3",
        params![turn_id, now, branch_id],
    )?;
    if let Some(variant_id) = selected_variant_id {
        let _ = conn.execute(
            "UPDATE assistant_message_variants SET turn_id = ?1, state_patch_id = ?2, is_discarded = 0 WHERE id = ?3",
            params![turn_id, patch_id, variant_id],
        );
    }
    Ok((
        get_turn_commit(conn, &turn_id)?,
        get_state_patch(conn, &patch_id)?,
    ))
}

pub fn discard_active_commits_for_assistant(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: i64,
) -> rusqlite::Result<()> {
    let commits = list_commits_for_assistant(conn, conversation_id, assistant_message_id)?;
    for commit in commits {
        conn.execute(
            "UPDATE state_patches SET is_active = 0 WHERE turn_id = ?1",
            [&commit.turn_id],
        )?;
        conn.execute(
            "UPDATE turn_commits SET active_variant = 0, is_active = 0, is_discarded = 1 WHERE turn_id = ?1",
            [commit.turn_id],
        )?;
    }
    conn.execute(
        "UPDATE assistant_message_variants SET is_discarded = 1 WHERE conversation_id = ?1 AND message_id = ?2",
        params![conversation_id, assistant_message_id],
    )?;
    Ok(())
}

pub fn deactivate_downstream_from_message(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE messages SET is_active = 0, message_status = 'hidden', hidden_at = ?3 WHERE conversation_id = ?1 AND id >= ?2",
        params![conversation_id, message_id, now_ts()],
    )?;
    let commits = list_commits_at_or_after_message(conn, conversation_id, message_id)?;
    let mut count = 0;
    for commit in commits {
        conn.execute(
            "UPDATE state_patches SET is_active = 0 WHERE turn_id = ?1",
            [&commit.turn_id],
        )?;
        conn.execute(
            "UPDATE turn_commits SET is_active = 0, is_discarded = 1, active_variant = 0 WHERE turn_id = ?1",
            [commit.turn_id],
        )?;
        count += 1;
    }
    Ok(count)
}

pub fn hide_latest_matching_active_user_tail(
    conn: &Connection,
    conversation_id: &str,
    user_text: &str,
) -> rusqlite::Result<Option<i64>> {
    let expected = user_text.trim();
    if expected.is_empty() {
        return Ok(None);
    }
    let latest = list_messages(conn, conversation_id, 1)?.into_iter().next();
    let Some(message) = latest else {
        return Ok(None);
    };
    if message.role != "user" || message.status != "active" || message.content.trim() != expected {
        return Ok(None);
    }
    deactivate_downstream_from_message(conn, conversation_id, message.id)?;
    Ok(Some(message.id))
}

fn inactive_restorable_message_ids(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "
        SELECT id
        FROM messages
        WHERE conversation_id = ?1
          AND is_active = 0
          AND message_status NOT IN ('pending', 'failed', 'retry_attempt', 'regenerated_discarded', 'duplicate_hidden')
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map([conversation_id], |row| row.get(0))?;
    rows.collect()
}

pub fn activate_variant_commit(
    conn: &Connection,
    conversation_id: &str,
    variant_id: i64,
) -> rusqlite::Result<Option<String>> {
    let turn_id: Option<String> = conn
        .query_row(
            "SELECT turn_id FROM assistant_message_variants WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, variant_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let Some(turn_id) = turn_id else {
        return Ok(None);
    };
    let commit = get_turn_commit(conn, &turn_id)?;
    if let Some(assistant_id) = commit.assistant_message_id {
        discard_active_commits_for_assistant(conn, conversation_id, assistant_id)?;
    }
    conn.execute(
        "UPDATE state_patches SET is_active = 1 WHERE turn_id = ?1",
        [&turn_id],
    )?;
    conn.execute(
        "UPDATE turn_commits SET active_variant = 1, is_active = 1, is_discarded = 0 WHERE turn_id = ?1",
        [turn_id.as_str()],
    )?;
    conn.execute(
        "UPDATE assistant_message_variants SET is_discarded = 0 WHERE id = ?1",
        [variant_id],
    )?;
    conn.execute(
        "UPDATE session_branches SET active_turn_id = ?1, updated_at = ?2 WHERE branch_id = ?3",
        params![turn_id, now_ts(), commit.branch_id],
    )?;
    Ok(Some(commit.branch_id))
}

pub fn rebuild_session_state(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
) -> rusqlite::Result<LedgerRebuild> {
    let active_turn_id = get_active_turn_id(conn, branch_id)?;
    rebuild_session_state_until(conn, conversation_id, branch_id, active_turn_id.as_deref())
}

pub fn rebuild_session_state_until(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    until_turn_id: Option<&str>,
) -> rusqlite::Result<LedgerRebuild> {
    let branch = get_active_session_branch(conn, conversation_id)?;
    let mut soul = decode_soul(&branch.base_soul_json)?;
    let mut session_world = decode_session_world(&branch.base_session_world_json)?;
    let commits = active_commit_path_until(conn, branch_id, until_turn_id)?;
    let mut debug = BranchPatchDebug {
        branch_id: branch_id.to_string(),
        active_turn_id: until_turn_id.map(str::to_string),
        rebuild_generation: branch.rebuild_generation + 1,
        ..BranchPatchDebug::default()
    };
    for commit in commits {
        let patches = list_active_patches_for_turn(conn, &commit.turn_id)?;
        if patches.is_empty() || commit.is_discarded || !commit.is_active {
            if let Some(ref patch_id) = commit.state_patch_id {
                debug.skipped_discarded_patches.push(patch_id.clone());
            }
            continue;
        }
        for patch_record in patches {
            let patch: EnginePatch = serde_json::from_str(&patch_record.patch_json)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
            patch
                .apply_to_session(&mut soul, Some(&mut session_world))
                .map_err(|err| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("{err:?}"),
                    )))
                })?;
            debug.applied_patches.push(patch_record.patch_id);
        }
        soul.turn_counter = soul.turn_counter.saturating_add(1);
        soul.turns_since_consolidation = soul.turns_since_consolidation.saturating_add(1);
    }
    let invalidated = list_inactive_patch_ids(conn, branch_id)?;
    debug.invalidated_patches = invalidated;
    conn.execute(
        "UPDATE session_branches SET rebuild_generation = rebuild_generation + 1, updated_at = ?1 WHERE branch_id = ?2",
        params![now_ts(), branch_id],
    )?;
    upsert_soul(conn, &soul)?;
    upsert_session_world(conn, &session_world)?;
    Ok(LedgerRebuild {
        soul,
        session_world,
        debug,
    })
}

pub fn recover_incomplete_sessions_on_startup(
    conn: &Connection,
) -> rusqlite::Result<StartupRecoveryReport> {
    let mut report = StartupRecoveryReport::default();
    let branches = {
        let mut stmt = conn.prepare(
            "
            SELECT branch_id, conversation_id, base_soul_json, base_session_world_json, active_turn_id, rebuild_generation, is_active, created_at, updated_at
            FROM session_branches
            WHERE is_active = 1
            ORDER BY updated_at ASC
            ",
        )?;
        let rows = stmt.query_map([], session_branch_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for branch in branches {
        if rebuild_session_state(conn, &branch.conversation_id, &branch.branch_id).is_ok() {
            report.branches_rebuilt += 1;
            report
                .materialized_conversation_ids
                .push(branch.conversation_id);
        }
    }

    let running_jobs = {
        let mut stmt = conn.prepare(
            "
            SELECT evaluator_job_id
            FROM evaluator_background_jobs
            WHERE status = 'running'
            ORDER BY started_at ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for job_id in &running_jobs {
        conn.execute(
            "
            UPDATE evaluator_background_jobs
            SET status = 'pending',
                error_message = COALESCE(error_message, 'Recovered after app restart; evaluator can be retried.'),
                completed_at = NULL,
                elapsed_ms = NULL,
                patch_applied = 0
            WHERE evaluator_job_id = ?1 AND status = 'running'
            ",
            [job_id],
        )?;
    }
    report.running_jobs_marked_retryable = running_jobs.len();

    report.pending_job_ids = evaluator_job_ids_by_status(conn, &["pending"])?;
    report.failed_job_ids = evaluator_job_ids_by_status(conn, &["failed"])?;
    report.canceled_or_timed_out_job_ids =
        evaluator_job_ids_by_status(conn, &["canceled", "timed_out"])?;
    Ok(report)
}

fn evaluator_job_ids_by_status(
    conn: &Connection,
    statuses: &[&str],
) -> rusqlite::Result<Vec<String>> {
    let mut ids = Vec::new();
    for status in statuses {
        let mut stmt = conn.prepare(
            "
            SELECT evaluator_job_id
            FROM evaluator_background_jobs
            WHERE status = ?1
            ORDER BY started_at ASC
            ",
        )?;
        let rows = stmt.query_map([*status], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.push(row?);
        }
    }
    Ok(ids)
}

fn session_branch_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionBranch> {
    Ok(SessionBranch {
        branch_id: row.get(0)?,
        conversation_id: row.get(1)?,
        base_soul_json: row.get(2)?,
        base_session_world_json: row.get(3)?,
        active_turn_id: row.get(4)?,
        rebuild_generation: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn turn_commit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnCommit> {
    Ok(TurnCommit {
        turn_id: row.get(0)?,
        conversation_id: row.get(1)?,
        branch_id: row.get(2)?,
        parent_turn_id: row.get(3)?,
        user_message_id: row.get(4)?,
        assistant_message_id: row.get(5)?,
        state_patch_id: row.get(6)?,
        selected_variant_id: row.get(7)?,
        created_at: row.get(8)?,
        active_variant: row.get::<_, i64>(9)? != 0,
        is_active: row.get::<_, i64>(10)? != 0,
        is_discarded: row.get::<_, i64>(11)? != 0,
        is_regenerated_variant: row.get::<_, i64>(12)? != 0,
    })
}

fn state_patch_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StatePatchRecord> {
    Ok(StatePatchRecord {
        patch_id: row.get(0)?,
        turn_id: row.get(1)?,
        parent_state_hash: row.get(2)?,
        patch_json: row.get(3)?,
        inverse_patch_json: row.get(4)?,
        applied_at: row.get(5)?,
        applies_to: row.get(6)?,
        is_active: row.get::<_, i64>(7)? != 0,
        invalidated_by_patch_id: row.get(8)?,
        supersedes_patch_id: row.get(9)?,
        patch_kind: row.get(10).unwrap_or_else(|_| "baseline".to_string()),
        parent_baseline_patch_id: row.get(11).ok().flatten(),
        source_turn_id: row.get(12).ok().flatten(),
        source_assistant_message_id: row.get(13).ok().flatten(),
        source_assistant_variant_id: row.get(14).ok().flatten(),
        created_by_job_id: row.get(15).ok().flatten(),
    })
}

fn get_turn_commit(conn: &Connection, turn_id: &str) -> rusqlite::Result<TurnCommit> {
    conn.query_row(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE turn_id = ?1",
        [turn_id],
        turn_commit_from_row,
    )
}

pub fn get_state_patch(conn: &Connection, patch_id: &str) -> rusqlite::Result<StatePatchRecord> {
    conn.query_row(
        "SELECT patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id, patch_kind, parent_baseline_patch_id, source_turn_id, source_assistant_message_id, source_assistant_variant_id, created_by_job_id FROM state_patches WHERE patch_id = ?1",
        [patch_id],
        state_patch_from_row,
    )
}

pub fn list_active_patches_for_turn(
    conn: &Connection,
    turn_id: &str,
) -> rusqlite::Result<Vec<StatePatchRecord>> {
    let mut stmt = conn.prepare(
        "SELECT patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id, patch_kind, parent_baseline_patch_id, source_turn_id, source_assistant_message_id, source_assistant_variant_id, created_by_job_id
         FROM state_patches 
         WHERE turn_id = ?1 AND is_active = 1
         ORDER BY CASE patch_kind WHEN 'baseline' THEN 0 WHEN 'enrichment' THEN 1 ELSE 2 END, applied_at ASC, patch_id ASC",
    )?;
    let rows = stmt.query_map([turn_id], state_patch_from_row)?;
    let mut patches = Vec::new();
    for row in rows {
        patches.push(row?);
    }
    Ok(patches)
}

pub fn record_enrichment_patch(
    conn: &Connection,
    turn_id: &str,
    patch: &EnginePatch,
) -> rusqlite::Result<StatePatchRecord> {
    record_enrichment_patch_with_metadata(conn, turn_id, patch, None, None, None, None)
}

pub fn record_enrichment_patch_with_metadata(
    conn: &Connection,
    turn_id: &str,
    patch: &EnginePatch,
    parent_baseline_patch_id: Option<&str>,
    source_assistant_message_id: Option<i64>,
    source_assistant_variant_id: Option<i64>,
    created_by_job_id: Option<&str>,
) -> rusqlite::Result<StatePatchRecord> {
    let now = now_ts();
    let patch_id = ledger_id("patch");
    let patch_json = serde_json::to_string(patch)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO state_patches
            (patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id, patch_kind, parent_baseline_patch_id, source_turn_id, source_assistant_message_id, source_assistant_variant_id, created_by_job_id)
        VALUES (?1, ?2, NULL, ?3, NULL, ?4, 'session', 1, NULL, NULL, 'enrichment', ?5, ?2, ?6, ?7, ?8)
        ",
        params![
            patch_id,
            turn_id,
            patch_json,
            now,
            parent_baseline_patch_id,
            source_assistant_message_id,
            source_assistant_variant_id,
            created_by_job_id
        ],
    )?;
    get_state_patch(conn, &patch_id)
}

fn list_commits_for_assistant(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: i64,
) -> rusqlite::Result<Vec<TurnCommit>> {
    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE conversation_id = ?1 AND assistant_message_id = ?2 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(
        params![conversation_id, assistant_message_id],
        turn_commit_from_row,
    )?;
    rows.collect()
}

fn list_commits_at_or_after_message(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<Vec<TurnCommit>> {
    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE conversation_id = ?1 AND is_active = 1 AND (COALESCE(user_message_id, 9223372036854775807) >= ?2 OR COALESCE(assistant_message_id, 9223372036854775807) >= ?2) ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![conversation_id, message_id], turn_commit_from_row)?;
    rows.collect()
}

fn active_commit_path_until(
    conn: &Connection,
    branch_id: &str,
    until_turn_id: Option<&str>,
) -> rusqlite::Result<Vec<TurnCommit>> {
    let mut stmt = conn.prepare(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE branch_id = ?1 AND is_active = 1 AND is_discarded = 0 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([branch_id], turn_commit_from_row)?;
    let mut commits_by_id = HashMap::new();
    let mut latest_turn_id = None;
    for row in rows {
        let commit = row?;
        latest_turn_id = Some(commit.turn_id.clone());
        commits_by_id.insert(commit.turn_id.clone(), commit);
    }
    let Some(mut cursor) = until_turn_id.map(str::to_string).or(latest_turn_id) else {
        return Ok(Vec::new());
    };
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    while seen.insert(cursor.clone()) {
        let Some(commit) = commits_by_id.get(&cursor).cloned() else {
            break;
        };
        cursor = commit.parent_turn_id.clone().unwrap_or_default();
        path.push(commit);
        if cursor.is_empty() {
            break;
        }
    }
    path.reverse();
    Ok(path)
}

pub fn active_branch_contains_turn(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    source_turn_id: &str,
) -> rusqlite::Result<bool> {
    let branch = get_active_session_branch(conn, conversation_id)?;
    if branch.branch_id != branch_id {
        return Ok(false);
    }
    let path = active_commit_path_until(conn, branch_id, branch.active_turn_id.as_deref())?;
    Ok(path.iter().any(|commit| commit.turn_id == source_turn_id))
}

fn list_inactive_patch_ids(conn: &Connection, branch_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT sp.patch_id FROM state_patches sp JOIN turn_commits tc ON tc.turn_id = sp.turn_id WHERE tc.branch_id = ?1 AND (sp.is_active = 0 OR tc.is_discarded = 1 OR tc.is_active = 0) ORDER BY sp.applied_at ASC",
    )?;
    let rows = stmt.query_map([branch_id], |row| row.get(0))?;
    rows.collect()
}

pub fn upsert_provider_profile(
    conn: &Connection,
    profile: &ProviderProfile,
) -> rusqlite::Result<ProviderProfile> {
    let now = now_ts();
    let created_at = if profile.created_at > 0 {
        profile.created_at
    } else {
        now
    };
    let updated = ProviderProfile {
        created_at,
        updated_at: now,
        ..profile.clone()
    };
    conn.execute(
        "
        INSERT INTO provider_profiles (
            id, name, base_url, api_key, model, system_prompt, created_at, updated_at,
            narrator_timeout_ms, evaluator_timeout_ms, evaluator_timeout_mode, evaluator_mode, structured_evaluator_policy,
            wait_for_evaluator_before_next_turn, allow_send_with_stale_state, evaluator_background_enabled,
            anti_replay_forced_retry_enabled, archived_at,
            narrator_compatibility_status, evaluator_compatibility_status, command_compatibility_status,
            evaluator_contract_version, evaluator_prompt_version, evaluator_last_tested_at,
            evaluator_last_failure_reason, structured_output_support
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            base_url = excluded.base_url,
            api_key = excluded.api_key,
            model = excluded.model,
            system_prompt = excluded.system_prompt,
            updated_at = excluded.updated_at,
            narrator_timeout_ms = excluded.narrator_timeout_ms,
            evaluator_timeout_ms = excluded.evaluator_timeout_ms,
            evaluator_timeout_mode = excluded.evaluator_timeout_mode,
            evaluator_mode = excluded.evaluator_mode,
            structured_evaluator_policy = excluded.structured_evaluator_policy,
            wait_for_evaluator_before_next_turn = excluded.wait_for_evaluator_before_next_turn,
            allow_send_with_stale_state = excluded.allow_send_with_stale_state,
            evaluator_background_enabled = excluded.evaluator_background_enabled,
            anti_replay_forced_retry_enabled = excluded.anti_replay_forced_retry_enabled,
            archived_at = excluded.archived_at,
            narrator_compatibility_status = excluded.narrator_compatibility_status,
            evaluator_compatibility_status = excluded.evaluator_compatibility_status,
            command_compatibility_status = excluded.command_compatibility_status,
            evaluator_contract_version = excluded.evaluator_contract_version,
            evaluator_prompt_version = excluded.evaluator_prompt_version,
            evaluator_last_tested_at = excluded.evaluator_last_tested_at,
            evaluator_last_failure_reason = excluded.evaluator_last_failure_reason,
            structured_output_support = excluded.structured_output_support
        ",
        params![
            updated.id,
            updated.name,
            updated.base_url,
            updated.api_key,
            updated.model,
            updated.system_prompt,
            updated.created_at,
            updated.updated_at,
            updated.narrator_timeout_ms,
            updated.evaluator_timeout_ms,
            updated.evaluator_timeout_mode,
            updated.evaluator_mode,
            updated.structured_evaluator_policy,
            updated.wait_for_evaluator_before_next_turn,
            updated.allow_send_with_stale_state,
            updated.evaluator_background_enabled,
            updated.anti_replay_forced_retry_enabled,
            updated.archived_at,
            updated.narrator_compatibility_status,
            updated.evaluator_compatibility_status,
            updated.command_compatibility_status,
            updated.evaluator_contract_version,
            updated.evaluator_prompt_version,
            updated.evaluator_last_tested_at,
            updated.evaluator_last_failure_reason,
            updated.structured_output_support
        ],
    )?;
    Ok(updated)
}

pub fn list_provider_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProviderProfile>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at,
               narrator_timeout_ms, evaluator_timeout_ms, evaluator_timeout_mode, evaluator_mode, structured_evaluator_policy,
               wait_for_evaluator_before_next_turn, allow_send_with_stale_state, evaluator_background_enabled,
               anti_replay_forced_retry_enabled, archived_at,
               narrator_compatibility_status, evaluator_compatibility_status, command_compatibility_status,
               evaluator_contract_version, evaluator_prompt_version, evaluator_last_tested_at,
               evaluator_last_failure_reason, structured_output_support
        FROM provider_profiles
        WHERE archived_at IS NULL
        ORDER BY updated_at DESC, name ASC
        ",
    )?;
    let rows = stmt.query_map([], provider_profile_from_row)?;
    rows.collect()
}

pub fn get_provider_profile(conn: &Connection, id: &str) -> rusqlite::Result<ProviderProfile> {
    conn.query_row(
        "
        SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at,
               narrator_timeout_ms, evaluator_timeout_ms, evaluator_timeout_mode, evaluator_mode, structured_evaluator_policy,
               wait_for_evaluator_before_next_turn, allow_send_with_stale_state, evaluator_background_enabled,
               anti_replay_forced_retry_enabled, archived_at,
               narrator_compatibility_status, evaluator_compatibility_status, command_compatibility_status,
               evaluator_contract_version, evaluator_prompt_version, evaluator_last_tested_at,
               evaluator_last_failure_reason, structured_output_support
        FROM provider_profiles
        WHERE id = ?1
        ",
        [id],
        provider_profile_from_row,
    )
}

pub fn delete_provider_profile(_conn: &Connection, _id: &str) -> rusqlite::Result<bool> {
    Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        "delete_provider_profile is deprecated; use archive_provider_profile with active profile guard.",
    ))))
}

#[allow(dead_code)]
pub(crate) fn delete_provider_profile_internal(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM provider_profiles WHERE id = ?1", [id])?;
    Ok(affected > 0)
}

pub fn archive_provider_profile(
    conn: &Connection,
    id: &str,
    active_ids: &[&str],
) -> Result<bool, String> {
    if active_ids.is_empty() {
        return Err("active_ids is required and cannot be empty.".to_string());
    }
    if active_ids.contains(&id) {
        return Err(
            "Cannot archive the active provider profile. Switch profiles first.".to_string(),
        );
    }
    let now = now_ts();
    let affected = conn
        .execute(
            "UPDATE provider_profiles SET archived_at = ?1 WHERE id = ?2",
            params![Some(now), id],
        )
        .map_err(|err| err.to_string())?;
    Ok(affected > 0)
}

pub fn restore_provider_profile(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute(
        "UPDATE provider_profiles SET archived_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(affected > 0)
}

pub fn list_archived_provider_profiles(
    conn: &Connection,
) -> rusqlite::Result<Vec<ProviderProfile>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at,
               narrator_timeout_ms, evaluator_timeout_ms, evaluator_timeout_mode, evaluator_mode, structured_evaluator_policy,
               wait_for_evaluator_before_next_turn, allow_send_with_stale_state, evaluator_background_enabled,
               anti_replay_forced_retry_enabled, archived_at,
               narrator_compatibility_status, evaluator_compatibility_status, command_compatibility_status,
               evaluator_contract_version, evaluator_prompt_version, evaluator_last_tested_at,
               evaluator_last_failure_reason, structured_output_support
        FROM provider_profiles
        WHERE archived_at IS NOT NULL
        ORDER BY archived_at DESC, name ASC
        ",
    )?;
    let rows = stmt.query_map([], provider_profile_from_row)?;
    rows.collect()
}

pub fn get_evaluator_empty_patch_streak(
    conn: &Connection,
    conversation_id: &str,
    profile_id: &str,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT empty_patch_streak FROM conversation_evaluator_streaks WHERE conversation_id = ?1 AND profile_id = ?2",
        params![conversation_id, profile_id],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.unwrap_or(0))
}

pub fn increment_evaluator_empty_patch_streak(
    conn: &Connection,
    conversation_id: &str,
    profile_id: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO conversation_evaluator_streaks (conversation_id, profile_id, empty_patch_streak)
         VALUES (?1, ?2, 1)
         ON CONFLICT(conversation_id, profile_id) DO UPDATE SET
             empty_patch_streak = empty_patch_streak + 1",
        params![conversation_id, profile_id],
    )?;
    get_evaluator_empty_patch_streak(conn, conversation_id, profile_id)
}

pub fn reset_evaluator_empty_patch_streak(
    conn: &Connection,
    conversation_id: &str,
    profile_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO conversation_evaluator_streaks (conversation_id, profile_id, empty_patch_streak)
         VALUES (?1, ?2, 0)
         ON CONFLICT(conversation_id, profile_id) DO UPDATE SET
             empty_patch_streak = 0",
        params![conversation_id, profile_id],
    )?;
    Ok(())
}

pub fn get_last_known_good_evaluator_profile(
    conn: &Connection,
) -> rusqlite::Result<Option<ProviderProfile>> {
    conn.query_row(
        "SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at,
                narrator_timeout_ms, evaluator_timeout_ms, evaluator_timeout_mode, evaluator_mode, structured_evaluator_policy,
                wait_for_evaluator_before_next_turn, allow_send_with_stale_state, evaluator_background_enabled,
                anti_replay_forced_retry_enabled, archived_at,
                narrator_compatibility_status, evaluator_compatibility_status, command_compatibility_status,
                evaluator_contract_version, evaluator_prompt_version, evaluator_last_tested_at,
                evaluator_last_failure_reason, structured_output_support
         FROM provider_profiles
         WHERE archived_at IS NULL AND evaluator_compatibility_status = 1
         ORDER BY evaluator_last_tested_at DESC, updated_at DESC
         LIMIT 1",
        [],
        provider_profile_from_row,
    )
    .optional()
}

pub fn set_active_evaluator_profile(
    conn: &Connection,
    conversation_id: &str,
    profile_id: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE conversations SET active_evaluator_profile_id = ?1 WHERE id = ?2",
        params![profile_id, conversation_id],
    )?;
    Ok(())
}

fn provider_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderProfile> {
    Ok(ProviderProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        api_key: row.get(3)?,
        model: row.get(4)?,
        system_prompt: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        narrator_timeout_ms: row.get(8)?,
        evaluator_timeout_ms: row.get(9)?,
        evaluator_timeout_mode: row.get(10)?,
        evaluator_mode: row.get(11)?,
        structured_evaluator_policy: row.get(12)?,
        wait_for_evaluator_before_next_turn: row.get(13)?,
        allow_send_with_stale_state: row.get(14)?,
        evaluator_background_enabled: row.get(15)?,
        anti_replay_forced_retry_enabled: row.get(16)?,
        archived_at: row.get(17)?,
        narrator_compatibility_status: row.get(18)?,
        evaluator_compatibility_status: row.get(19)?,
        command_compatibility_status: row.get(20)?,
        evaluator_contract_version: row.get(21)?,
        evaluator_prompt_version: row.get(22)?,
        evaluator_last_tested_at: row.get(23)?,
        evaluator_last_failure_reason: row.get(24)?,
        structured_output_support: row.get(25)?,
    })
}

// Background Evaluator Jobs Database Helpers
pub fn insert_evaluator_job(conn: &Connection, job: &EvaluatorJob) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO evaluator_background_jobs (
            evaluator_job_id, conversation_id, turn_id, assistant_message_id, status,
            started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode,
            model, provider, error_message, patch_applied
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            job.evaluator_job_id,
            job.conversation_id,
            job.turn_id,
            job.assistant_message_id,
            job.status,
            job.started_at,
            job.completed_at,
            job.elapsed_ms,
            job.timeout_ms.map(|v| v as i64),
            job.timeout_mode,
            job.model,
            job.provider,
            job.error_message,
            if job.patch_applied { 1 } else { 0 }
        ],
    )?;
    Ok(())
}

pub fn update_evaluator_job_status(
    conn: &Connection,
    job_id: &str,
    status: &str,
    error_message: Option<&str>,
    completed_at: Option<i64>,
    elapsed_ms: Option<i64>,
    patch_applied: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE evaluator_background_jobs
         SET status = ?1, error_message = ?2, completed_at = ?3, elapsed_ms = ?4, patch_applied = ?5
         WHERE evaluator_job_id = ?6",
        params![
            status,
            error_message,
            completed_at,
            elapsed_ms,
            if patch_applied { 1 } else { 0 },
            job_id
        ],
    )?;
    Ok(())
}

pub fn get_evaluator_job(
    conn: &Connection,
    job_id: &str,
) -> rusqlite::Result<Option<EvaluatorJob>> {
    let mut stmt = conn.prepare(
        "SELECT evaluator_job_id, conversation_id, turn_id, assistant_message_id, status,
                started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode,
                model, provider, error_message, patch_applied
         FROM evaluator_background_jobs
         WHERE evaluator_job_id = ?1",
    )?;
    let mut rows = stmt.query_map([job_id], evaluator_job_from_row)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn get_latest_evaluator_job(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Option<EvaluatorJob>> {
    let active_branch_opt = get_active_session_branch(conn, conversation_id).optional()?;
    if let Some(branch) = active_branch_opt {
        let path =
            active_commit_path_until(conn, &branch.branch_id, branch.active_turn_id.as_deref())?;
        let active_turn_ids: HashSet<String> =
            path.into_iter().map(|commit| commit.turn_id).collect();

        let mut stmt = conn.prepare(
            "SELECT evaluator_job_id, conversation_id, turn_id, assistant_message_id, status,
                    started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode,
                    model, provider, error_message, patch_applied
             FROM evaluator_background_jobs
             WHERE conversation_id = ?1
             ORDER BY started_at DESC",
        )?;

        let mut rows = stmt.query_map([conversation_id], evaluator_job_from_row)?;
        while let Some(row_res) = rows.next() {
            let job = row_res?;
            if active_turn_ids.contains(&job.turn_id) {
                return Ok(Some(job));
            }
        }
    }

    let mut stmt = conn.prepare(
        "SELECT evaluator_job_id, conversation_id, turn_id, assistant_message_id, status,
                started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode,
                model, provider, error_message, patch_applied
         FROM evaluator_background_jobs
         WHERE conversation_id = ?1
         ORDER BY started_at DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map([conversation_id], evaluator_job_from_row)?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn get_pending_evaluator_jobs_for_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<EvaluatorJob>> {
    let mut stmt = conn.prepare(
        "SELECT evaluator_job_id, conversation_id, turn_id, assistant_message_id, status,
                started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode,
                model, provider, error_message, patch_applied
         FROM evaluator_background_jobs
         WHERE conversation_id = ?1 AND status IN ('pending', 'running')
         ORDER BY started_at ASC",
    )?;
    let rows = stmt.query_map([conversation_id], evaluator_job_from_row)?;
    rows.collect()
}

/// One dialogue-only exchange the fast-mode gate skipped; held until the next
/// evaluator run folds it in as catch-up context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorCatchupEntry {
    pub id: i64,
    pub conversation_id: String,
    pub user_message_id: Option<i64>,
    pub assistant_message_id: i64,
    pub user_text: String,
    pub assistant_text: String,
    pub created_at: i64,
}

pub fn insert_evaluator_catchup_entry(
    conn: &Connection,
    conversation_id: &str,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    user_text: &str,
    assistant_text: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO evaluator_catchup_queue
            (conversation_id, user_message_id, assistant_message_id, user_text, assistant_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            conversation_id,
            user_message_id,
            assistant_message_id,
            user_text,
            assistant_text,
            now_ts()
        ],
    )?;
    Ok(())
}

pub fn list_evaluator_catchup_entries(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<EvaluatorCatchupEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, user_message_id, assistant_message_id,
                user_text, assistant_text, created_at
         FROM evaluator_catchup_queue
         WHERE conversation_id = ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([conversation_id], |row| {
        Ok(EvaluatorCatchupEntry {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            user_message_id: row.get(2)?,
            assistant_message_id: row.get(3)?,
            user_text: row.get(4)?,
            assistant_text: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn delete_evaluator_catchup_entries(
    conn: &Connection,
    conversation_id: &str,
    ids: &[i64],
) -> rusqlite::Result<()> {
    for id in ids {
        conn.execute(
            "DELETE FROM evaluator_catchup_queue WHERE conversation_id = ?1 AND id = ?2",
            rusqlite::params![conversation_id, id],
        )?;
    }
    Ok(())
}

fn evaluator_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvaluatorJob> {
    let timeout_ms_i64: Option<i64> = row.get(8)?;
    Ok(EvaluatorJob {
        evaluator_job_id: row.get(0)?,
        conversation_id: row.get(1)?,
        turn_id: row.get(2)?,
        assistant_message_id: row.get(3)?,
        status: row.get(4)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        elapsed_ms: row.get(7)?,
        timeout_ms: timeout_ms_i64.map(|v| v as u64),
        timeout_mode: row.get(9)?,
        model: row.get(10)?,
        provider: row.get(11)?,
        error_message: row.get(12)?,
        patch_applied: row.get::<_, i64>(13)? != 0,
    })
}

fn entity_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EntityRecord> {
    let aliases_json: String = row.get(3)?;
    let aliases = serde_json::from_str::<Vec<String>>(&aliases_json).unwrap_or_default();
    Ok(EntityRecord {
        conversation_id: row.get(0)?,
        entity_id: row.get(1)?,
        display_name: row.get(2)?,
        aliases,
        kind: row.get(4)?,
        controlled_by: row.get(5)?,
        linked_soul_id: row.get(6)?,
        active_in_scene: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn encode_aliases(aliases: &[String]) -> rusqlite::Result<String> {
    serde_json::to_string(aliases)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))
}

fn decode_soul(json: &str) -> rusqlite::Result<Soul> {
    serde_json::from_str(json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

fn decode_setting(json: &str) -> rusqlite::Result<SettingSoul> {
    serde_json::from_str(json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

fn decode_session_world(json: &str) -> rusqlite::Result<SessionWorld> {
    serde_json::from_str(json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

fn summarize_setting(setting: &SettingSoul) -> SettingSummary {
    SettingSummary {
        setting_id: setting.setting_id.clone(),
        setting_name: setting.setting_name.clone(),
        last_updated: setting.last_updated,
        turn_counter: setting.turn_counter,
        location: setting.world.location.clone(),
        archived_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state_engine::patch::{
        EnginePatch, MemoryPatch, RelationshipDelta, SceneStatePatch, SoulPatch,
        WorldEventOperationPatch, WorldPatch,
    };
    use state_engine::setting::new_default_setting;
    use state_engine::soul::{
        new_default_soul, session_soul_from_savepoint, soul_savepoint_from_session, ObjectState,
    };

    #[test]
    fn migrations_persist_souls_and_messages() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");

        let summaries = list_souls(&conn).expect("list");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].character_name, "Aurora");

        ensure_conversation(&conn, "mock", &soul.character_id).expect("conversation");
        insert_message(&conn, "mock", "user", "Hello").expect("user");
        insert_message(&conn, "mock", "assistant", "Hi").expect("assistant");

        let messages = list_messages(&conn, "mock", 5).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(count_assistant_messages(&conn, "mock").unwrap(), 1);

        assert!(update_message_content(&conn, "mock", messages[1].id, "Regenerated").unwrap());
        let messages = list_messages(&conn, "mock", 5).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "Regenerated");
    }

    #[test]
    fn mark_conversation_benchmark_sets_summary_flag() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "benchmark-session", &soul.character_id).expect("conversation");

        let before = get_conversation_summary(&conn, "benchmark-session").expect("summary");
        assert!(!before.is_benchmark);

        assert!(mark_conversation_benchmark(&conn, "benchmark-session").expect("mark"));
        let after = get_conversation_summary(&conn, "benchmark-session").expect("summary");
        assert!(after.is_benchmark);
        assert!(list_conversations(&conn)
            .expect("list")
            .iter()
            .any(
                |conversation| conversation.conversation_id == "benchmark-session"
                    && conversation.is_benchmark
            ));
    }

    #[test]
    fn image_asset_metadata_and_message_attachment_persist() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "image-chat", &soul.character_id).expect("conversation");
        let message_id =
            insert_message_and_get_id(&conn, "image-chat", "user", "[Image]").expect("message");
        let asset = ImageAsset {
            id: "asset-1".into(),
            file_path: "C:\\app-data\\images\\asset-1.png".into(),
            thumbnail_path: None,
            source: "uploaded".into(),
            mime_type: Some("image/png".into()),
            width: Some(2),
            height: Some(3),
            prompt: None,
            provider: None,
            model: None,
            linked_soul_id: None,
            linked_conversation_id: Some("image-chat".into()),
            linked_message_id: Some(message_id),
            created_at: now_ts(),
        };
        upsert_image_asset(&conn, &asset).expect("asset");
        attach_image_to_message(&conn, message_id, &asset.id).expect("attachment");

        let messages = list_messages(&conn, "image-chat", 10).expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].attachments.len(), 1);
        assert_eq!(
            messages[0].attachments[0].image.mime_type.as_deref(),
            Some("image/png")
        );
        assert_eq!(messages[0].attachments[0].image.width, Some(2));
        assert_eq!(get_image_asset(&conn, "asset-1").unwrap().height, Some(3));
    }

    #[test]
    fn user_message_update_edits_user_rows_only() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "edit-user", &soul.character_id).expect("conversation");
        let user_id = insert_message_and_get_id(&conn, "edit-user", "user", "Original user text")
            .expect("user");
        let assistant_id =
            insert_message_and_get_id(&conn, "edit-user", "assistant", "Assistant text")
                .expect("assistant");

        assert!(
            update_user_message_content(&conn, "edit-user", user_id, "Edited user text")
                .expect("edit user")
        );
        assert!(
            !update_user_message_content(&conn, "edit-user", assistant_id, "Wrong row")
                .expect("ignore assistant")
        );

        let messages = list_messages(&conn, "edit-user", 10).expect("messages");
        assert_eq!(messages[0].content, "Edited user text");
        assert_eq!(messages[1].content, "Assistant text");
    }

    #[test]
    fn duplicate_active_user_message_guard_reuses_canonical() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "retry-guard", &soul.character_id).expect("conversation");
        let user_id = insert_message_and_get_id(&conn, "retry-guard", "user", "Open the door.")
            .expect("user");

        let reusable =
            find_reusable_active_user_message(&conn, "retry-guard", "  Open   the door.  ")
                .expect("guard");

        assert_eq!(reusable, Some(user_id));
        insert_message_and_get_id(&conn, "retry-guard", "assistant", "The door opens.")
            .expect("assistant");
        assert_eq!(
            find_reusable_active_user_message(&conn, "retry-guard", "Open the door.")
                .expect("guard after assistant"),
            None
        );
    }

    #[test]
    fn dedupe_active_adjacent_user_messages_hides_duplicate() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "repair-dupes", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "repair-dupes", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "repair-dupes").expect("branch");
        let canonical =
            insert_message_and_get_id(&conn, "repair-dupes", "user", "The same prompt.")
                .expect("canonical");
        let duplicate =
            insert_message_and_get_id(&conn, "repair-dupes", "user", "  The same   prompt.  ")
                .expect("duplicate");
        let assistant = insert_message_and_get_id(&conn, "repair-dupes", "assistant", "Response.")
            .expect("assistant");
        let (commit, _) = record_turn_commit_with_patch(
            &conn,
            "repair-dupes",
            &branch.branch_id,
            None,
            Some(duplicate),
            assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("commit");

        let result = dedupe_active_adjacent_user_messages(&conn, "repair-dupes").expect("dedupe");

        assert_eq!(result.canonical_user_message_ids, vec![canonical]);
        assert_eq!(result.hidden_duplicate_user_message_ids, vec![duplicate]);
        let visible = list_messages(&conn, "repair-dupes", 10).expect("messages");
        assert_eq!(
            visible
                .iter()
                .filter(|message| message.role == "user")
                .count(),
            1
        );
        assert_eq!(visible[0].id, canonical);
        let duplicate_row = get_message(&conn, "repair-dupes", duplicate).expect("duplicate row");
        assert_eq!(duplicate_row.status, "duplicate_hidden");
        let commit_after = get_turn_commit(&conn, &commit.turn_id).expect("commit");
        assert_eq!(commit_after.user_message_id, Some(canonical));
        assert!(restore_inactive_messages(&conn, "repair-dupes")
            .expect("restore")
            .skipped_duplicate_ids
            .contains(&duplicate));
    }

    #[test]
    fn migration_creates_assistant_variants_table() {
        let conn = init_memory_connection().expect("db");
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'assistant_message_variants'",
                [],
                |row| row.get(0),
            )
            .expect("table query");

        assert_eq!(exists, 1);
    }

    #[test]
    fn migration_creates_llm_payload_logs_table() {
        let conn = init_memory_connection().expect("db");
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'llm_payload_logs'",
                [],
                |row| row.get(0),
            )
            .expect("table query");

        assert_eq!(exists, 1);
    }

    #[test]
    fn migration_creates_conversation_entities_table() {
        let conn = init_memory_connection().expect("db");
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'conversation_entities'",
                [],
                |row| row.get(0),
            )
            .expect("table query");

        assert_eq!(exists, 1);
    }

    #[test]
    fn player_personas_have_two_builtins_and_custom_lifecycle() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "persona-session", &soul.character_id).expect("conversation");

        let builtins = built_in_player_personas();
        assert_eq!(builtins.len(), 2);
        assert!(builtins
            .iter()
            .any(|persona| persona.persona_id == "preset_male"));
        assert!(builtins
            .iter()
            .any(|persona| persona.persona_id == "preset_female"));
        assert_eq!(
            get_active_player_persona_id(&conn, "persona-session").expect("active id"),
            "preset_male"
        );
        assert!(!archive_player_persona(&conn, "preset_male").expect("archive builtin"));

        let custom = PlayerPersona {
            persona_id: "persona_jun".into(),
            display_name: "Jun Persona".into(),
            description: "User-controlled custom RP persona.".into(),
            gender_code: "custom".into(),
            pronouns: "they/them".into(),
            is_builtin: false,
            is_archived: false,
            created_at: 0,
            updated_at: 0,
            appearance: Some("Black coat.".into()),
            voice_style: None,
            boundaries: None,
            notes: Some("Test persona.".into()),
        };
        let saved = upsert_player_persona(&conn, &custom).expect("save custom");
        assert_eq!(saved.persona_id, "persona_jun");
        assert!(saved.created_at > 0);

        let active =
            set_active_player_persona(&conn, "persona-session", "persona_jun").expect("set active");
        assert_eq!(active.display_name, "Jun Persona");
        assert_eq!(
            get_active_player_persona_id(&conn, "persona-session").expect("active id"),
            "persona_jun"
        );

        set_active_player_persona(&conn, "persona-session", "preset_male")
            .expect("switch away before archive");
        assert!(archive_player_persona(&conn, "persona_jun").expect("archive custom"));
        assert!(set_active_player_persona(&conn, "persona-session", "persona_jun").is_err());
        assert!(!list_player_personas(&conn)
            .expect("list")
            .iter()
            .any(|persona| persona.persona_id == "persona_jun"));
        assert!(restore_player_persona(&conn, "persona_jun").expect("restore custom"));
        assert!(list_player_personas(&conn)
            .expect("list restored")
            .iter()
            .any(|persona| persona.persona_id == "persona_jun"));
    }

    #[test]
    fn normal_soul_library_excludes_session_clones() {
        let conn = init_memory_connection().expect("db");
        let mut savepoint = new_default_soul("Aurora Start");
        savepoint.world.location = "Apartment door".into();
        upsert_soul(&conn, &savepoint).expect("savepoint");
        let session = session_soul_from_savepoint(&savepoint);
        upsert_soul(&conn, &session).expect("session");

        let library = list_souls(&conn).expect("library");
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].character_id, savepoint.character_id);
        assert_eq!(library[0].soul_kind, "savepoint");

        let debug = list_souls_including_session_clones(&conn).expect("debug");
        assert_eq!(debug.len(), 2);
        let session_summary = debug
            .iter()
            .find(|summary| summary.character_id == session.character_id)
            .expect("session summary");
        assert_eq!(session_summary.soul_kind, "session_clone");
        assert_eq!(
            session_summary.source_savepoint_id.as_deref(),
            Some(savepoint.character_id.as_str())
        );
    }

    #[test]
    fn session_checkpoint_savepoint_does_not_mutate_source() {
        let conn = init_memory_connection().expect("db");
        let mut savepoint = new_default_soul("Aurora After 10 Talks");
        savepoint.relationships.get_mut("user").unwrap().trust = 77.0;
        upsert_soul(&conn, &savepoint).expect("savepoint");
        let mut session = session_soul_from_savepoint(&savepoint);
        session.world.location = "New session room".into();
        session.relationships.get_mut("user").unwrap().trust = 12.0;
        upsert_soul(&conn, &session).expect("session");

        let checkpoint = soul_savepoint_from_session(&session, "Aurora Checkpoint", "checkpoint");
        upsert_soul(&conn, &checkpoint).expect("checkpoint");

        let source = get_soul(&conn, &savepoint.character_id).expect("source");
        assert_eq!(source.relationships["user"].trust, 77.0);
        assert_ne!(source.world.location, "New session room");
        let checkpoint = get_soul(&conn, &checkpoint.character_id).expect("checkpoint");
        assert_eq!(checkpoint.soul_kind, "checkpoint");
        assert_eq!(checkpoint.world.location, "New session room");
        assert_eq!(
            checkpoint.source_savepoint_id.as_deref(),
            Some(savepoint.character_id.as_str())
        );
    }

    #[test]
    fn session_world_clones_setting_without_mutating_source() {
        let conn = init_memory_connection().expect("db");
        let mut setting = new_default_setting("Aurora Testing Room World");
        setting.world.location = "Original lab".into();
        setting.world.recent_events = vec!["Aurora calibrated the lab.".into()];
        upsert_setting(&conn, &setting).expect("setting");

        let mut session_world =
            create_session_world_from_setting(&conn, &setting.setting_id).expect("session world");
        session_world.location = "Mutated session lab".into();
        session_world.recent_events.push("Echo-0 entered.".into());
        upsert_session_world(&conn, &session_world).expect("upsert session world");

        let source = get_setting(&conn, &setting.setting_id).expect("source setting");
        assert_eq!(source.world.location, "Original lab");
        assert_eq!(source.world.recent_events.len(), 1);
        assert_eq!(
            get_session_world(&conn, &session_world.world_id)
                .expect("session world")
                .location,
            "Mutated session lab"
        );
    }

    #[test]
    fn conversations_can_share_one_session_world_reference() {
        let conn = init_memory_connection().expect("db");
        let soul_a = new_default_soul("Aurora");
        let soul_b = new_default_soul("Echo-0");
        upsert_soul(&conn, &soul_a).expect("soul a");
        upsert_soul(&conn, &soul_b).expect("soul b");
        let world = create_legacy_session_world_from_soul(&conn, &soul_a).expect("world");

        ensure_conversation_with_title_and_world(
            &conn,
            "shared-a",
            &soul_a.character_id,
            Some(&world.world_id),
            None,
            Some("A"),
        )
        .expect("conversation a");
        ensure_conversation_with_title_and_world(
            &conn,
            "shared-b",
            &soul_b.character_id,
            Some(&world.world_id),
            None,
            Some("B"),
        )
        .expect("conversation b");

        let conversations = list_conversations(&conn).expect("conversations");
        assert_eq!(
            conversations
                .iter()
                .filter(|conversation| conversation.world_id.as_deref()
                    == Some(world.world_id.as_str()))
                .count(),
            2
        );
    }

    #[test]
    fn conversation_titles_can_be_renamed_independently_of_soul() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        let conversation = ensure_conversation_with_title(
            &conn,
            "session-title",
            &soul.character_id,
            Some("First title"),
        )
        .expect("conversation");
        assert_eq!(conversation.title, "First title");

        let renamed =
            rename_conversation(&conn, "session-title", "Renamed Session").expect("rename");
        assert_eq!(renamed.title, "Renamed Session");
        assert_eq!(
            get_soul(&conn, &soul.character_id).unwrap().character_name,
            "Aurora"
        );
        let conversations = list_conversations(&conn).expect("conversations");
        assert_eq!(conversations[0].title, "Renamed Session");
    }

    #[test]
    fn entity_registry_stores_aliases_and_active_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "entities", &soul.character_id).expect("conversation");

        let entity = upsert_entity(
            &conn,
            &EntityRecord {
                entity_id: "rhy".into(),
                conversation_id: "entities".into(),
                display_name: "Rhy".into(),
                aliases: vec!["Rhy".into()],
                kind: "user_controlled".into(),
                controlled_by: "user".into(),
                linked_soul_id: None,
                active_in_scene: true,
                created_at: 0,
                updated_at: 0,
            },
        )
        .expect("upsert entity");
        assert_eq!(entity.display_name, "Rhy");

        let entity = add_entity_alias(&conn, "entities", "rhy", "Rjy").expect("add alias");
        assert!(entity.aliases.iter().any(|alias| alias == "Rjy"));
        let entities = list_entities(&conn, "entities").expect("list");
        assert_eq!(entities.len(), 1);
        assert!(entities[0].active_in_scene);
    }

    #[test]
    fn llm_payload_log_stores_payload_without_api_key() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "payloads", &soul.character_id).expect("conversation");
        let message_id =
            insert_message_and_get_id(&conn, "payloads", "assistant", "Response").unwrap();

        let log_id = insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                id: 0,
                conversation_id: "payloads".into(),
                message_id: None,
                provider: "API".into(),
                mode: "Reader".into(),
                context_mode: "brief".into(),
                model: "debug-model".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System with context".into(),
                user_message: "User input".into(),
                context_text: "[LATEST EXCHANGE]".into(),
                estimated_system_tokens: 4,
                estimated_user_tokens: 2,
                estimated_total_tokens: 6,
                truncated: false,
                created_at: now_ts(),
                branch_id: None,
                active_turn_id: None,
                parent_turn_id: None,
                state_patch_ids_applied: Vec::new(),
                discarded_patch_ids_skipped: Vec::new(),
                state_rebuild_generation: None,
                latest_assistant_variant_id: None,
                ..Default::default()
            },
        )
        .expect("insert log");
        assert!(set_llm_payload_log_message_id(&conn, log_id, message_id).unwrap());

        let logs = list_llm_payload_logs(&conn, "payloads").expect("logs");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message_id, Some(message_id));
        assert_eq!(logs[0].model, "debug-model");
        let serialized = serde_json::to_string(&logs[0]).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn payload_response_update_preserves_fallback_used_when_omitted() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "fallback-flag", &soul.character_id).expect("conversation");

        let log_id = insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                conversation_id: "fallback-flag".into(),
                ..Default::default()
            },
        )
        .expect("insert log");

        update_llm_payload_log_response(
            &conn,
            log_id,
            &LlmPayloadResponseUpdate {
                fallback_used: Some(true),
                fallback_reason: Some("generic_mock_prose_detected".into()),
                ..Default::default()
            },
        )
        .expect("mark fallback");

        update_llm_payload_log_response(
            &conn,
            log_id,
            &LlmPayloadResponseUpdate {
                raw_provider_response: Some("raw narrator body".into()),
                normalized_response: Some("visible narrator body".into()),
                finish_reason: Some("stop".into()),
                provider_request_id: Some("req-retry".into()),
                provider_response_id: Some("resp-retry".into()),
                ..Default::default()
            },
        )
        .expect("response update without fallback flag");

        let log = get_llm_payload_log(&conn, log_id).expect("log");
        assert!(log.fallback_used);
        assert_eq!(
            log.fallback_reason.as_deref(),
            Some("generic_mock_prose_detected")
        );
        assert_eq!(
            log.raw_provider_response.as_deref(),
            Some("raw narrator body")
        );
        assert_eq!(log.provider_response_id.as_deref(), Some("resp-retry"));
    }

    fn test_world_event_patch(event: &str) -> EnginePatch {
        EnginePatch {
            world_patch: Some(WorldPatch {
                recent_event: Some(event.into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        }
    }

    #[test]
    fn background_enrichment_applies_to_prior_active_turn_after_branch_advances() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "enrich-prior", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        let branch = create_session_branch(&conn, "enrich-prior", &soul, &world).expect("branch");
        let first_assistant =
            insert_message_and_get_id(&conn, "enrich-prior", "assistant", "First").expect("a1");
        let (first_commit, first_patch) = record_turn_commit_with_patch(
            &conn,
            "enrich-prior",
            &branch.branch_id,
            None,
            None,
            first_assistant,
            None,
            &test_world_event_patch("baseline first turn"),
            false,
        )
        .expect("first");
        let second_assistant =
            insert_message_and_get_id(&conn, "enrich-prior", "assistant", "Second").expect("a2");
        let (_second_commit, second_patch) = record_turn_commit_with_patch(
            &conn,
            "enrich-prior",
            &branch.branch_id,
            Some(&first_commit.turn_id),
            None,
            second_assistant,
            None,
            &test_world_event_patch("second turn after branch advanced"),
            false,
        )
        .expect("second");

        assert!(active_branch_contains_turn(
            &conn,
            "enrich-prior",
            &branch.branch_id,
            &first_commit.turn_id
        )
        .expect("ancestry"));
        let enrichment = record_enrichment_patch_with_metadata(
            &conn,
            &first_commit.turn_id,
            &test_world_event_patch("enrichment for first turn"),
            Some(&first_patch.patch_id),
            Some(first_assistant),
            None,
            Some("job-prior"),
        )
        .expect("enrichment");
        let rebuilt =
            rebuild_session_state(&conn, "enrich-prior", &branch.branch_id).expect("rebuild");

        assert_eq!(
            rebuilt.debug.applied_patches,
            vec![
                first_patch.patch_id.clone(),
                enrichment.patch_id.clone(),
                second_patch.patch_id.clone()
            ]
        );
        assert!(rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|event| event.contains("enrichment for first turn")));
        assert_eq!(enrichment.patch_kind, "enrichment");
        assert_eq!(
            enrichment.parent_baseline_patch_id.as_deref(),
            Some(first_patch.patch_id.as_str())
        );
        assert_eq!(
            enrichment.source_turn_id.as_deref(),
            Some(first_commit.turn_id.as_str())
        );
        assert_eq!(enrichment.created_by_job_id.as_deref(), Some("job-prior"));
    }

    #[test]
    fn background_enrichment_skips_only_if_source_turn_not_on_active_branch() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "enrich-stale", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        let branch = create_session_branch(&conn, "enrich-stale", &soul, &world).expect("branch");
        let root_assistant =
            insert_message_and_get_id(&conn, "enrich-stale", "assistant", "Root").expect("root");
        let (root_commit, _) = record_turn_commit_with_patch(
            &conn,
            "enrich-stale",
            &branch.branch_id,
            None,
            None,
            root_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("root commit");
        let abandoned_assistant =
            insert_message_and_get_id(&conn, "enrich-stale", "assistant", "Old path").expect("old");
        let (abandoned_commit, _) = record_turn_commit_with_patch(
            &conn,
            "enrich-stale",
            &branch.branch_id,
            Some(&root_commit.turn_id),
            None,
            abandoned_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("abandoned");
        let selected_assistant =
            insert_message_and_get_id(&conn, "enrich-stale", "assistant", "New path").expect("new");
        record_turn_commit_with_patch(
            &conn,
            "enrich-stale",
            &branch.branch_id,
            Some(&root_commit.turn_id),
            None,
            selected_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("selected");

        assert!(active_branch_contains_turn(
            &conn,
            "enrich-stale",
            &branch.branch_id,
            &root_commit.turn_id
        )
        .expect("root active"));
        assert!(!active_branch_contains_turn(
            &conn,
            "enrich-stale",
            &branch.branch_id,
            &abandoned_commit.turn_id
        )
        .expect("abandoned inactive"));
    }

    #[test]
    fn rebuild_applies_baseline_then_enrichment_for_same_turn() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "enrich-order", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        let branch = create_session_branch(&conn, "enrich-order", &soul, &world).expect("branch");
        let assistant =
            insert_message_and_get_id(&conn, "enrich-order", "assistant", "Turn").expect("a");
        let (commit, baseline) = record_turn_commit_with_patch(
            &conn,
            "enrich-order",
            &branch.branch_id,
            None,
            None,
            assistant,
            None,
            &test_world_event_patch("baseline event"),
            false,
        )
        .expect("baseline");
        let enrichment = record_enrichment_patch_with_metadata(
            &conn,
            &commit.turn_id,
            &test_world_event_patch("enrichment event"),
            Some(&baseline.patch_id),
            Some(assistant),
            None,
            Some("job-order"),
        )
        .expect("enrichment");

        let patches = list_active_patches_for_turn(&conn, &commit.turn_id).expect("patches");
        assert_eq!(patches[0].patch_id, baseline.patch_id);
        assert_eq!(patches[1].patch_id, enrichment.patch_id);

        let rebuilt =
            rebuild_session_state(&conn, "enrich-order", &branch.branch_id).expect("rebuild");
        assert_eq!(
            rebuilt.debug.applied_patches,
            vec![baseline.patch_id, enrichment.patch_id]
        );
    }

    #[test]
    fn ledger_rebuild_overwrites_stale_materialized_cache() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Echo-0");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "ledger-cache", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "ledger-cache", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "ledger-cache").expect("branch");
        let user_id =
            insert_message_and_get_id(&conn, "ledger-cache", "user", "Move.").expect("user");
        let assistant_id = insert_message_and_get_id(&conn, "ledger-cache", "assistant", "Moved.")
            .expect("assistant");
        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                location: Some("Ledger room".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        record_turn_commit_with_patch(
            &conn,
            "ledger-cache",
            &branch.branch_id,
            None,
            Some(user_id),
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");
        let mut stale = soul.clone();
        stale.world.location = "Stale cache room".into();
        upsert_soul(&conn, &stale).expect("stale");

        let rebuilt =
            rebuild_session_state(&conn, "ledger-cache", &branch.branch_id).expect("rebuild");

        assert_eq!(rebuilt.session_world.location, "Ledger room");
        assert_eq!(
            get_soul(&conn, &soul.character_id)
                .expect("cache soul")
                .world
                .location,
            "Unspecified starting scene."
        );
    }

    #[test]
    fn evaluator_scene_turn_updates_session_world() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "eval-world", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "eval-world", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "eval-world").expect("branch");
        let user_id = insert_message_and_get_id(&conn, "eval-world", "user", "Someone knocks.")
            .expect("user");
        let assistant_id = insert_message_and_get_id(
            &conn,
            "eval-world",
            "assistant",
            "Aurora hears a knock at the apartment door.",
        )
        .expect("assistant");
        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                location: Some("Apartment entry".into()),
                recent_event: Some("A knock sounded at Aurora's apartment door.".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        record_turn_commit_with_patch(
            &conn,
            "eval-world",
            &branch.branch_id,
            None,
            Some(user_id),
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "eval-world", &branch.branch_id).expect("rebuild");

        assert_eq!(rebuilt.session_world.location, "Apartment entry");
        assert!(rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|event| event.contains("knock sounded")));
    }

    #[test]
    fn evaluator_scene_turn_increments_turn_counter_or_equivalent() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "eval-counter", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "eval-counter", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "eval-counter").expect("branch");
        let assistant_id =
            insert_message_and_get_id(&conn, "eval-counter", "assistant", "Scene advanced.")
                .expect("assistant");
        record_turn_commit_with_patch(
            &conn,
            "eval-counter",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &EnginePatch {
                world_patch: Some(WorldPatch {
                    recent_event: Some("The scene advanced.".into()),
                    ..WorldPatch::default()
                }),
                ..EnginePatch::default()
            },
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "eval-counter", &branch.branch_id).expect("rebuild");

        assert_eq!(rebuilt.soul.turn_counter, 1);
        assert_eq!(
            get_soul(&conn, &soul.character_id)
                .expect("soul")
                .turn_counter,
            1
        );
    }

    #[test]
    fn mne_export_uses_rebuilt_session_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "mne-rebuild", &soul.character_id).expect("conversation");
        let world =
            ensure_conversation_session_world(&conn, "mne-rebuild", &soul, None).expect("world");
        create_session_branch(&conn, "mne-rebuild", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "mne-rebuild").expect("branch");
        let assistant_id = insert_message_and_get_id(&conn, "mne-rebuild", "assistant", "Moved.")
            .expect("assistant");
        record_turn_commit_with_patch(
            &conn,
            "mne-rebuild",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &EnginePatch {
                world_patch: Some(WorldPatch {
                    location: Some("Exported rebuilt room".into()),
                    recent_event: Some("Aurora crossed into the rebuilt room.".into()),
                    ..WorldPatch::default()
                }),
                ..EnginePatch::default()
            },
            false,
        )
        .expect("record");
        let mut stale_world = world.clone();
        stale_world.location = "Stale export room".into();
        upsert_session_world(&conn, &stale_world).expect("stale world");

        let rebuilt =
            rebuild_session_state(&conn, "mne-rebuild", &branch.branch_id).expect("rebuild");
        let exported_world = get_conversation_session_world(&conn, "mne-rebuild")
            .expect("query world")
            .expect("world");

        assert_eq!(rebuilt.session_world.location, "Exported rebuilt room");
        assert_eq!(exported_world.location, "Exported rebuilt room");
        assert!(exported_world
            .recent_events
            .iter()
            .any(|event| event.contains("rebuilt room")));
    }

    #[test]
    fn ledger_regenerate_discards_old_patch_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "ledger-regen", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "ledger-regen", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "ledger-regen").expect("branch");
        let user_id =
            insert_message_and_get_id(&conn, "ledger-regen", "user", "Phone?").expect("user");
        let assistant_id = insert_message_and_get_id(&conn, "ledger-regen", "assistant", "Bad.")
            .expect("assistant");
        let bad = EnginePatch {
            world_patch: Some(WorldPatch {
                recent_event: Some("Aurora's phone buzzed.".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        record_turn_commit_with_patch(
            &conn,
            "ledger-regen",
            &branch.branch_id,
            None,
            Some(user_id),
            assistant_id,
            None,
            &bad,
            false,
        )
        .expect("bad");
        discard_active_commits_for_assistant(&conn, "ledger-regen", assistant_id).expect("discard");
        let good = EnginePatch {
            world_patch: Some(WorldPatch {
                recent_event: Some("Aurora keeps the silenced phone dark.".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        record_turn_commit_with_patch(
            &conn,
            "ledger-regen",
            &branch.branch_id,
            None,
            Some(user_id),
            assistant_id,
            None,
            &good,
            true,
        )
        .expect("good");

        let rebuilt =
            rebuild_session_state(&conn, "ledger-regen", &branch.branch_id).expect("rebuild");

        assert!(!rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|event| event.contains("buzzed")));
        assert!(rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|event| event.contains("silenced phone")));
    }

    #[test]
    fn world_event_operations_invalidate_by_stable_id() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "ledger-retcon", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "ledger-retcon", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "ledger-retcon").expect("branch");
        let assistant_id = insert_message_and_get_id(&conn, "ledger-retcon", "assistant", "Patch.")
            .expect("assistant");
        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                event_operations: vec![
                    WorldEventOperationPatch {
                        operation: "add_recent_event".into(),
                        recent_event_id: Some("phone_buzz".into()),
                        content: Some("Aurora's phone buzzed.".into()),
                        ..WorldEventOperationPatch::default()
                    },
                    WorldEventOperationPatch {
                        operation: "invalidate_recent_event".into(),
                        target_recent_event_id: Some("phone_buzz".into()),
                        ..WorldEventOperationPatch::default()
                    },
                ],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        record_turn_commit_with_patch(
            &conn,
            "ledger-retcon",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "ledger-retcon", &branch.branch_id).expect("rebuild");

        assert!(rebuilt.session_world.recent_events.is_empty());
        assert_eq!(
            rebuilt.session_world.recent_event_records[0].recent_event_id,
            "phone_buzz"
        );
        assert!(!rebuilt.session_world.recent_event_records[0].is_active);
    }

    #[test]
    fn assistant_variants_create_select_and_cascade() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "variants", &soul.character_id).expect("conversation");
        let message_id = insert_message_and_get_id(&conn, "variants", "assistant", "Response A")
            .expect("assistant");

        let legacy = list_assistant_message_variants(&conn, "variants", message_id)
            .expect("legacy variants");
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].id, None);
        assert_eq!(legacy[0].content, "Response A");

        let first = create_assistant_message_variant(
            &conn,
            "variants",
            message_id,
            "Response B",
            Some("Variant 2"),
            Some("regenerate"),
            true,
            Some("{}"),
            Some("{}"),
        )
        .expect("create variant");
        let variants =
            list_assistant_message_variants(&conn, "variants", message_id).expect("variants");
        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants
                .iter()
                .filter(|variant| variant.is_selected)
                .count(),
            1
        );
        assert_eq!(
            list_messages(&conn, "variants", 10).unwrap()[0].content,
            "Response B"
        );

        let base_id = variants
            .iter()
            .find(|variant| variant.content == "Response A")
            .and_then(|variant| variant.id)
            .expect("base variant id");
        select_assistant_message_variant(&conn, "variants", message_id, base_id)
            .expect("select base");
        let variants =
            list_assistant_message_variants(&conn, "variants", message_id).expect("variants");
        assert_eq!(
            variants
                .iter()
                .filter(|variant| variant.is_selected)
                .count(),
            1
        );
        assert!(variants
            .iter()
            .any(|variant| variant.id == Some(base_id) && variant.is_selected));
        assert!(variants
            .iter()
            .any(|variant| variant.id == first.id && !variant.is_selected));
        assert_eq!(
            list_messages(&conn, "variants", 10).unwrap()[0].content,
            "Response A"
        );

        assert!(
            hard_delete_message_internal(&conn, "variants", message_id).expect("delete message")
        );
        let variant_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM assistant_message_variants WHERE message_id = ?1",
                [message_id],
                |row| row.get(0),
            )
            .expect("variant count");
        assert_eq!(variant_count, 0);
    }

    #[test]
    fn conversation_delete_archives_without_destroying_chat_data() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "mock", &soul.character_id).expect("conversation");
        insert_message(&conn, "mock", "user", "Hello").expect("user");

        assert!(delete_conversation(&conn, "mock").expect("delete conversation"));
        let archived = get_conversation_summary(&conn, "mock").expect("archived conversation");
        assert!(archived.title.starts_with("[Archived] "));
        assert_eq!(list_messages(&conn, "mock", 5).expect("messages").len(), 1);

        assert!(hard_delete_soul_internal(&conn, &soul.character_id).expect("delete soul"));
        assert!(list_souls(&conn).expect("souls").is_empty());

        let message_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .expect("message count");
        assert_eq!(message_count, 0);
    }

    #[test]
    fn deleting_one_conversation_preserves_sibling_conversations() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "chat-a", &soul.character_id).expect("conversation a");
        ensure_conversation(&conn, "chat-b", &soul.character_id).expect("conversation b");
        insert_message(&conn, "chat-a", "user", "A").expect("message a");
        insert_message(&conn, "chat-b", "user", "B").expect("message b");

        assert!(delete_conversation(&conn, "chat-a").expect("delete chat a"));

        assert!(get_conversation_summary(&conn, "chat-a")
            .expect("chat a archived")
            .title
            .starts_with("[Archived] "));
        assert_eq!(
            get_conversation_summary(&conn, "chat-b")
                .expect("chat b remains")
                .conversation_id,
            "chat-b"
        );
        assert_eq!(
            list_messages(&conn, "chat-b", 5).expect("messages").len(),
            1
        );
        assert!(get_soul(&conn, &soul.character_id).is_ok());
    }

    #[test]
    fn deleting_one_session_clone_chat_preserves_other_sessions_from_same_source() {
        let conn = init_memory_connection().expect("db");
        let source = new_default_soul("Aurora");
        upsert_soul(&conn, &source).expect("source");
        let session_a = session_soul_from_savepoint(&source);
        let session_b = session_soul_from_savepoint(&source);
        upsert_soul(&conn, &session_a).expect("session a");
        upsert_soul(&conn, &session_b).expect("session b");
        ensure_conversation(&conn, "session-a", &session_a.character_id).expect("conversation a");
        ensure_conversation(&conn, "session-b", &session_b.character_id).expect("conversation b");
        insert_message(&conn, "session-a", "user", "A").expect("message a");
        insert_message(&conn, "session-b", "user", "B").expect("message b");

        assert!(delete_conversation(&conn, "session-a").expect("delete session a"));

        assert!(get_conversation_summary(&conn, "session-a")
            .expect("session a archived")
            .title
            .starts_with("[Archived] "));
        assert!(get_soul(&conn, &session_a.character_id).is_ok());
        assert!(get_soul(&conn, &source.character_id).is_ok());
        assert!(get_soul(&conn, &session_b.character_id).is_ok());
        assert_eq!(
            get_conversation_summary(&conn, "session-b")
                .expect("session b remains")
                .conversation_id,
            "session-b"
        );
        assert_eq!(
            list_messages(&conn, "session-b", 5)
                .expect("messages")
                .len(),
            1
        );
    }

    #[test]
    fn downstream_message_rewind_keeps_empty_conversation_visible() {
        let conn = init_memory_connection().expect("db");
        let source = new_default_soul("Aurora");
        let session = session_soul_from_savepoint(&source);
        upsert_soul(&conn, &source).expect("source");
        upsert_soul(&conn, &session).expect("session");
        ensure_conversation(&conn, "rewind-session", &session.character_id).expect("conversation");
        let first =
            insert_message_and_get_id(&conn, "rewind-session", "user", "First").expect("first");
        insert_message(&conn, "rewind-session", "assistant", "Second").expect("second");

        deactivate_downstream_from_message(&conn, "rewind-session", first).expect("rewind");

        assert!(list_messages(&conn, "rewind-session", 10)
            .expect("active messages")
            .is_empty());
        let conversation =
            get_conversation_summary(&conn, "rewind-session").expect("conversation remains");
        assert_eq!(conversation.conversation_id, "rewind-session");
        assert_eq!(conversation.message_count, 0);
        assert!(list_conversations(&conn)
            .expect("conversation list")
            .iter()
            .any(|conversation| conversation.conversation_id == "rewind-session"));
    }

    #[test]
    fn restore_inactive_messages_makes_rewound_chat_visible_again() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "restore-session", &soul.character_id).expect("conversation");
        let first =
            insert_message_and_get_id(&conn, "restore-session", "user", "First").expect("first");
        insert_message(&conn, "restore-session", "assistant", "Second").expect("second");

        deactivate_downstream_from_message(&conn, "restore-session", first).expect("rewind");
        assert!(list_messages(&conn, "restore-session", 10)
            .expect("active messages")
            .is_empty());

        let restored =
            restore_inactive_messages(&conn, "restore-session").expect("restore inactive");
        assert_eq!(restored.restored_message_ids.len(), 2);
        let messages = list_messages(&conn, "restore-session", 10).expect("restored messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "First");
        assert_eq!(messages[1].content, "Second");
    }

    #[test]
    fn restore_turns_does_not_restore_duplicate_user_messages() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "restore-dupes", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        let branch = create_session_branch(&conn, "restore-dupes", &soul, &world).expect("branch");
        let canonical_user =
            insert_message_and_get_id(&conn, "restore-dupes", "user", "I knock on the door.")
                .expect("user");
        let duplicate_user =
            insert_message_and_get_id(&conn, "restore-dupes", "user", "I knock on the door.")
                .expect("dup");
        let assistant =
            insert_message_and_get_id(&conn, "restore-dupes", "assistant", "Aurora answers.")
                .expect("assistant");
        record_turn_commit_with_patch(
            &conn,
            "restore-dupes",
            &branch.branch_id,
            None,
            Some(canonical_user),
            assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("commit");
        conn.execute(
            "UPDATE messages SET is_active = 0, message_status = 'hidden' WHERE id IN (?1, ?2, ?3)",
            params![canonical_user, duplicate_user, assistant],
        )
        .expect("hide");

        let restored = restore_inactive_messages(&conn, "restore-dupes").expect("restore");

        assert_eq!(
            restored.restored_message_ids,
            vec![canonical_user, assistant]
        );
        assert_eq!(restored.skipped_duplicate_ids, vec![duplicate_user]);
        let messages = list_messages(&conn, "restore-dupes", 10).expect("messages");
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.role == "user")
                .count(),
            1
        );
    }

    #[test]
    fn restore_turns_skips_pending_failed_retry_messages() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "restore-status", &soul.character_id).expect("conversation");
        let pending =
            insert_message_and_get_id(&conn, "restore-status", "user", "Pending").expect("pending");
        let failed =
            insert_message_and_get_id(&conn, "restore-status", "user", "Failed").expect("failed");
        let retry =
            insert_message_and_get_id(&conn, "restore-status", "user", "Retry").expect("retry");
        conn.execute(
            "
            UPDATE messages
            SET is_active = 0,
                message_status = CASE id
                    WHEN ?1 THEN 'pending'
                    WHEN ?2 THEN 'failed'
                    ELSE 'retry_attempt'
                END
            WHERE id IN (?1, ?2, ?3)
            ",
            params![pending, failed, retry],
        )
        .expect("mark statuses");

        let restored = restore_inactive_messages(&conn, "restore-status").expect("restore");

        assert!(restored.restored_message_ids.is_empty());
        assert_eq!(restored.skipped_pending_ids, vec![pending]);
        assert_eq!(restored.skipped_failed_ids, vec![failed]);
        assert_eq!(restored.skipped_retry_attempt_ids, vec![retry]);
        assert!(list_messages(&conn, "restore-status", 10)
            .expect("messages")
            .is_empty());
    }

    #[test]
    fn restore_turns_restores_only_active_branch_path() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "restore-path", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        let branch = create_session_branch(&conn, "restore-path", &soul, &world).expect("branch");
        let root_user =
            insert_message_and_get_id(&conn, "restore-path", "user", "First").expect("root user");
        let root_assistant = insert_message_and_get_id(&conn, "restore-path", "assistant", "Root")
            .expect("root assistant");
        let (root_commit, _) = record_turn_commit_with_patch(
            &conn,
            "restore-path",
            &branch.branch_id,
            None,
            Some(root_user),
            root_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("root");
        let abandoned_user =
            insert_message_and_get_id(&conn, "restore-path", "user", "Branch A").expect("a user");
        let abandoned_assistant =
            insert_message_and_get_id(&conn, "restore-path", "assistant", "Abandoned")
                .expect("a assistant");
        record_turn_commit_with_patch(
            &conn,
            "restore-path",
            &branch.branch_id,
            Some(&root_commit.turn_id),
            Some(abandoned_user),
            abandoned_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("abandoned");
        let selected_user =
            insert_message_and_get_id(&conn, "restore-path", "user", "Branch B").expect("b user");
        let selected_assistant =
            insert_message_and_get_id(&conn, "restore-path", "assistant", "Selected")
                .expect("b assistant");
        record_turn_commit_with_patch(
            &conn,
            "restore-path",
            &branch.branch_id,
            Some(&root_commit.turn_id),
            Some(selected_user),
            selected_assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("selected");
        conn.execute(
            "UPDATE messages SET is_active = 0, message_status = 'hidden' WHERE conversation_id = ?1",
            ["restore-path"],
        )
        .expect("hide all");

        let restored = restore_inactive_messages(&conn, "restore-path").expect("restore");

        assert_eq!(
            restored.restored_message_ids,
            vec![root_user, root_assistant, selected_user, selected_assistant]
        );
        assert!(!restored.restored_message_ids.contains(&abandoned_user));
        assert!(!restored.restored_message_ids.contains(&abandoned_assistant));
    }

    #[test]
    fn settings_persist_select_and_delete_independently() {
        let conn = init_memory_connection().expect("db");
        let mut setting = new_default_setting("Carver City");
        setting.world.location = "Underground cell".into();
        upsert_setting(&conn, &setting).expect("upsert");

        let summaries = list_settings(&conn).expect("list settings");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].setting_name, "Carver City");
        assert_eq!(summaries[0].location, "Underground cell");

        let loaded = get_setting(&conn, &setting.setting_id).expect("get setting");
        assert_eq!(loaded.world.location, "Underground cell");
        assert!(delete_setting(&conn, &setting.setting_id).is_err());
        assert!(delete_setting_internal(&conn, &setting.setting_id).expect("delete"));
        assert!(list_settings(&conn).expect("list settings").is_empty());
    }

    #[test]
    fn archive_setting_hides_from_active_list() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("ActiveSetting1");
        let setting2 = new_default_setting("ActiveSetting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        assert_eq!(list_settings(&conn).expect("list").len(), 2);

        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive");
        let active = list_settings(&conn).expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].setting_id, setting2.setting_id);
    }

    #[test]
    fn restore_setting_reappears_in_active_list() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("RestoreSetting1");
        let setting2 = new_default_setting("RestoreSetting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive");
        assert_eq!(list_settings(&conn).expect("list").len(), 1);

        restore_setting(&conn, &setting1.setting_id).expect("restore");
        assert_eq!(list_settings(&conn).expect("list").len(), 2);
    }

    #[test]
    fn archive_setting_does_not_delete_config() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("ConfigSetting1");
        let setting2 = new_default_setting("ConfigSetting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive");

        let retrieved = get_setting(&conn, &setting1.setting_id).expect("get");
        assert_eq!(retrieved.setting_name, "ConfigSetting1");
    }

    #[test]
    fn archive_one_setting_does_not_affect_sibling_settings() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("Setting1");
        let setting2 = new_default_setting("Setting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive");

        let active = list_settings(&conn).expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].setting_id, setting2.setting_id);
    }

    #[test]
    fn archive_active_or_default_setting_is_blocked() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("ActiveSetting1");
        let setting2 = new_default_setting("ActiveSetting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        // 1. Blocked because it is in active_ids list
        let active_ids = vec![setting1.setting_id.as_str()];
        let res = archive_setting(&conn, &setting1.setting_id, &active_ids);
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap(),
            "Cannot archive the active/default setting. Switch settings first."
        );

        // 2. Blocked because it is the last remaining setting
        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive first");
        let res2 = archive_setting(&conn, &setting2.setting_id, &[]);
        assert!(res2.is_err());
        assert_eq!(
            res2.err().unwrap(),
            "Cannot archive the active/default setting. Switch settings first."
        );
    }

    #[test]
    fn list_archived_settings_returns_only_archived() {
        let conn = init_memory_connection().expect("db");
        let setting1 = new_default_setting("Setting1");
        let setting2 = new_default_setting("Setting2");
        upsert_setting(&conn, &setting1).expect("upsert");
        upsert_setting(&conn, &setting2).expect("upsert");

        archive_setting(&conn, &setting1.setting_id, &[]).expect("archive");

        let archived = list_archived_settings(&conn).expect("list");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].setting_id, setting1.setting_id);
    }

    #[test]
    fn delete_setting_returns_deprecated_error_or_noops() {
        let conn = init_memory_connection().expect("db");
        let setting = new_default_setting("DeleteSetting");
        upsert_setting(&conn, &setting).expect("upsert");

        let res = delete_setting(&conn, &setting.setting_id);
        assert!(res.is_err());
    }

    #[test]
    fn setting_row_survives_legacy_delete_attempt() {
        let conn = init_memory_connection().expect("db");
        let setting = new_default_setting("SurvivingSetting");
        upsert_setting(&conn, &setting).expect("upsert");

        let _ = delete_setting(&conn, &setting.setting_id);

        let retrieved = get_setting(&conn, &setting.setting_id).expect("survives");
        assert_eq!(retrieved.setting_name, "SurvivingSetting");
    }

    #[test]
    fn provider_profiles_crud() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt".into(),
            system_prompt: String::new(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("allow_fallback".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };

        let saved = upsert_provider_profile(&conn, &profile).expect("upsert");
        assert!(saved.created_at > 0);
        assert_eq!(list_provider_profiles(&conn).expect("list").len(), 1);
        assert_eq!(
            get_provider_profile(&conn, "openai").expect("get").model,
            "gpt"
        );
        assert_eq!(
            get_provider_profile(&conn, "openai")
                .expect("get")
                .evaluator_mode
                .as_deref(),
            Some("evaluator_form_v1")
        );
        assert_eq!(
            get_provider_profile(&conn, "openai")
                .expect("get")
                .structured_evaluator_policy
                .as_deref(),
            Some("allow_fallback")
        );
        assert!(delete_provider_profile(&conn, "openai").is_err());
        assert!(delete_provider_profile_internal(&conn, "openai").expect("delete"));
        assert!(list_provider_profiles(&conn).expect("list").is_empty());
    }

    #[test]
    fn provider_profile_persists_structured_output_support() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "structured".into(),
            name: "Structured".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt".into(),
            system_prompt: String::new(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: None,
            evaluator_timeout_ms: None,
            evaluator_timeout_mode: None,
            evaluator_mode: Some("evaluator_structured_v1".into()),
            structured_evaluator_policy: Some("required".into()),
            wait_for_evaluator_before_next_turn: None,
            allow_send_with_stale_state: None,
            evaluator_background_enabled: None,
            anti_replay_forced_retry_enabled: None,
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 3,
        };

        upsert_provider_profile(&conn, &profile).expect("upsert");
        let retrieved = get_provider_profile(&conn, "structured").expect("get");
        assert_eq!(retrieved.structured_output_support, 3);
        assert_eq!(
            retrieved.structured_evaluator_policy.as_deref(),
            Some("required")
        );

        // Round-trips through a profile update without being reset.
        upsert_provider_profile(&conn, &retrieved).expect("re-upsert");
        assert_eq!(
            get_provider_profile(&conn, "structured")
                .expect("get")
                .structured_output_support,
            3
        );
    }

    #[test]
    fn provider_profiles_archive_restore_safety() {
        let conn = init_memory_connection().expect("db");
        let profile1 = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key-openai".into(),
            model: "gpt-4".into(),
            system_prompt: "Sys1".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };
        let profile2 = ProviderProfile {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_key: "key-anthropic".into(),
            model: "claude-3".into(),
            system_prompt: "Sys2".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(40_000),
            evaluator_timeout_ms: Some(35_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(false),
            allow_send_with_stale_state: Some(true),
            evaluator_background_enabled: Some(true),
            anti_replay_forced_retry_enabled: Some(true),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };

        upsert_provider_profile(&conn, &profile1).expect("upsert1");
        upsert_provider_profile(&conn, &profile2).expect("upsert2");

        // Verify initially both are in list_provider_profiles and list_archived is empty
        assert_eq!(list_provider_profiles(&conn).expect("list").len(), 2);
        assert_eq!(
            list_archived_provider_profiles(&conn)
                .expect("list archived")
                .len(),
            0
        );

        // 1. archive_active_provider_profile_is_blocked
        let active_ids = vec!["openai"];
        let block_res = archive_provider_profile(&conn, "openai", &active_ids);
        assert!(block_res.is_err());
        assert_eq!(
            block_res.err().unwrap(),
            "Cannot archive the active provider profile. Switch profiles first."
        );

        // 2. archive_provider_profile_hides_from_active_list
        let archive_res =
            archive_provider_profile(&conn, "openai", &["anthropic"]).expect("archive");
        assert!(archive_res);

        let active_list = list_provider_profiles(&conn).expect("list");
        assert_eq!(active_list.len(), 1);
        assert_eq!(active_list[0].id, "anthropic");

        let archived_list = list_archived_provider_profiles(&conn).expect("list archived");
        assert_eq!(archived_list.len(), 1);
        assert_eq!(archived_list[0].id, "openai");

        // 3. archive_one_profile_does_not_affect_sibling_profiles
        let anthropic_active = get_provider_profile(&conn, "anthropic").expect("get active");
        assert_eq!(anthropic_active.archived_at, None);

        // 4. archive_provider_profile_does_not_delete_config and does_not_delete_api_key_field
        let openai_archived = get_provider_profile(&conn, "openai").expect("get archived");
        assert!(openai_archived.archived_at.is_some());
        assert_eq!(openai_archived.base_url, "https://api.openai.com/v1");
        assert_eq!(openai_archived.api_key, "key-openai");
        assert_eq!(openai_archived.model, "gpt-4");
        assert_eq!(openai_archived.system_prompt, "Sys1");

        // 5. restore_provider_profile_reappears_in_active_list
        assert!(restore_provider_profile(&conn, "openai").expect("restore"));
        assert_eq!(list_provider_profiles(&conn).expect("list").len(), 2);
        assert_eq!(
            list_archived_provider_profiles(&conn)
                .expect("list archived")
                .len(),
            0
        );
    }

    #[test]
    fn delete_provider_profile_no_longer_hard_deletes() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt-4".into(),
            system_prompt: "Sys".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };
        upsert_provider_profile(&conn, &profile).expect("upsert");

        let delete_res = delete_provider_profile(&conn, "openai");
        assert!(delete_res.is_err());

        let record = get_provider_profile(&conn, "openai").expect("still exists");
        assert_eq!(record.id, "openai");
        assert_eq!(record.archived_at, None);
    }

    #[test]
    fn delete_provider_profile_cannot_archive_active_profile_or_returns_deprecated_error() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt-4".into(),
            system_prompt: "Sys".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };
        upsert_provider_profile(&conn, &profile).expect("upsert");

        let delete_res = delete_provider_profile(&conn, "openai");
        assert!(delete_res.is_err());
        let err_msg = format!("{:?}", delete_res.err().unwrap());
        assert!(
            err_msg.contains("deprecated")
                || err_msg.contains("delete_provider_profile is deprecated")
        );
    }

    #[test]
    fn archive_provider_profile_still_blocks_active_profile() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt-4".into(),
            system_prompt: "Sys".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };
        upsert_provider_profile(&conn, &profile).expect("upsert");

        let active_ids = vec!["openai"];
        let archive_res = archive_provider_profile(&conn, "openai", &active_ids);
        assert!(archive_res.is_err());
        assert_eq!(
            archive_res.err().unwrap(),
            "Cannot archive the active provider profile. Switch profiles first."
        );
    }

    #[test]
    fn archive_provider_profile_still_archives_inactive_profile() {
        let conn = init_memory_connection().expect("db");
        let profile = ProviderProfile {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "gpt-4".into(),
            system_prompt: "Sys".into(),
            created_at: 0,
            updated_at: 0,
            narrator_timeout_ms: Some(30_000),
            evaluator_timeout_ms: Some(25_000),
            evaluator_timeout_mode: Some("finite".into()),
            evaluator_mode: Some("evaluator_form_v1".into()),
            structured_evaluator_policy: Some("prefer".into()),
            wait_for_evaluator_before_next_turn: Some(true),
            allow_send_with_stale_state: Some(false),
            evaluator_background_enabled: Some(false),
            anti_replay_forced_retry_enabled: Some(false),
            archived_at: None,
            narrator_compatibility_status: 0,
            evaluator_compatibility_status: 0,
            command_compatibility_status: 0,
            evaluator_contract_version: 0,
            evaluator_prompt_version: 0,
            evaluator_last_tested_at: None,
            evaluator_last_failure_reason: None,
            structured_output_support: 0,
        };
        upsert_provider_profile(&conn, &profile).expect("upsert");

        let active_ids = vec!["some_other_active_profile"];
        let archive_res = archive_provider_profile(&conn, "openai", &active_ids).expect("archive");
        assert!(archive_res);

        let record = get_provider_profile(&conn, "openai").expect("exists");
        assert!(record.archived_at.is_some());
    }

    #[test]
    fn duplicate_repaired_user_message_remains_hidden_and_not_exported_active() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "hidden-dupes", &soul.character_id).expect("conversation");
        let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
        create_session_branch(&conn, "hidden-dupes", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "hidden-dupes").expect("branch");

        let canonical =
            insert_message_and_get_id(&conn, "hidden-dupes", "user", "Unique user message")
                .expect("canonical");
        let duplicate =
            insert_message_and_get_id(&conn, "hidden-dupes", "user", "Unique user message")
                .expect("duplicate");
        let assistant = insert_message_and_get_id(&conn, "hidden-dupes", "assistant", "Response.")
            .expect("assistant");

        record_turn_commit_with_patch(
            &conn,
            "hidden-dupes",
            &branch.branch_id,
            None,
            Some(duplicate),
            assistant,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("record");

        // Now dedupe to mark it duplicate_hidden
        let result = dedupe_active_adjacent_user_messages(&conn, "hidden-dupes").expect("dedupe");
        assert_eq!(result.hidden_duplicate_user_message_ids, vec![duplicate]);

        // When listing active messages for export:
        let active_messages = list_messages(&conn, "hidden-dupes", 100).expect("list");

        // Assert that the duplicate message is not present in active messages
        assert!(!active_messages.iter().any(|m| m.id == duplicate));
        assert!(active_messages.iter().any(|m| m.id == canonical));
    }

    #[test]
    fn scene_turn_export_contains_rebuilt_recent_event_or_scene_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "scene-turn-export", &soul.character_id).expect("conversation");
        let world = ensure_conversation_session_world(&conn, "scene-turn-export", &soul, None)
            .expect("world");
        create_session_branch(&conn, "scene-turn-export", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "scene-turn-export").expect("branch");
        let assistant_id = insert_message_and_get_id(
            &conn,
            "scene-turn-export",
            "assistant",
            "The plot thickens.",
        )
        .expect("assistant");

        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                recent_events: vec!["A scene event occurred.".to_string()],
                scene_state: Some(SceneStatePatch {
                    current_scene: Some("Active Investigation Scene".into()),
                    ..SceneStatePatch::default()
                }),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        record_turn_commit_with_patch(
            &conn,
            "scene-turn-export",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        // Rebuild state
        let rebuilt =
            rebuild_session_state(&conn, "scene-turn-export", &branch.branch_id).expect("rebuild");

        // Assert rebuilt state has the recent event and current scene
        assert!(rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|e| e.contains("scene event")));
        assert_eq!(
            rebuilt.session_world.scene_state.current_scene,
            "Active Investigation Scene"
        );
    }

    #[test]
    fn horror_knock_scene_creates_world_plot_current_scene_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "horror-knock", &soul.character_id).expect("conversation");
        let world =
            ensure_conversation_session_world(&conn, "horror-knock", &soul, None).expect("world");
        create_session_branch(&conn, "horror-knock", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "horror-knock").expect("branch");
        let assistant_id = insert_message_and_get_id(
            &conn,
            "horror-knock",
            "assistant",
            "A loud knock at the door.",
        )
        .expect("assistant");

        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                recent_events: vec!["A terrifying horror knock rattled the front door.".to_string()],
                corrected_object_states: vec![ObjectState {
                    object_id: "door".into(),
                    status: "rattling".into(),
                    ..ObjectState::default()
                }],
                scene_state: Some(SceneStatePatch {
                    current_scene: Some("Horror Knock Scene".into()),
                    resolved_active_plot: Some("Solve the Door Mystery".into()),
                    ..SceneStatePatch::default()
                }),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        record_turn_commit_with_patch(
            &conn,
            "horror-knock",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "horror-knock", &branch.branch_id).expect("rebuild");

        assert!(rebuilt
            .session_world
            .recent_events
            .iter()
            .any(|e| e.contains("horror knock")));
        assert_eq!(
            rebuilt.session_world.scene_state.current_scene,
            "Horror Knock Scene"
        );
        assert_eq!(
            rebuilt.session_world.scene_state.resolved_active_plot,
            "Solve the Door Mystery"
        );
        assert!(rebuilt
            .session_world
            .object_states
            .iter()
            .any(|obj| obj.object_id == "door" && obj.status == "rattling"));
    }

    #[test]
    fn reset_loop_scene_creates_current_plot_or_unresolved_tension_memory_candidate() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "reset-loop", &soul.character_id).expect("conversation");
        let world =
            ensure_conversation_session_world(&conn, "reset-loop", &soul, None).expect("world");
        create_session_branch(&conn, "reset-loop", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "reset-loop").expect("branch");
        let assistant_id =
            insert_message_and_get_id(&conn, "reset-loop", "assistant", "The loop resets again.")
                .expect("assistant");

        let patch = EnginePatch {
            soul_patch: Some(SoulPatch {
                new_memories: vec![
                    MemoryPatch {
                        content: "Stuck in a time loop.".into(),
                        tag: Some("unresolved_tension".into()),
                        ..MemoryPatch::default()
                    },
                    MemoryPatch {
                        content: "Break the reset-loop sequence.".into(),
                        tag: Some("current_plot".into()),
                        ..MemoryPatch::default()
                    },
                ],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        record_turn_commit_with_patch(
            &conn,
            "reset-loop",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "reset-loop", &branch.branch_id).expect("rebuild");

        let recent_memories = &rebuilt.soul.memory.recent;
        assert!(recent_memories
            .iter()
            .any(|m| m.content.contains("time loop") && m.tag == "unresolved_tension"));
        assert!(recent_memories
            .iter()
            .any(|m| m.content.contains("reset-loop") && m.tag == "current_plot"));
    }

    #[test]
    fn mne_export_includes_rebuilt_evaluator_applied_state() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "mne-evaluator", &soul.character_id).expect("conversation");
        let world =
            ensure_conversation_session_world(&conn, "mne-evaluator", &soul, None).expect("world");
        create_session_branch(&conn, "mne-evaluator", &soul, &world).expect("branch");
        let branch = get_active_session_branch(&conn, "mne-evaluator").expect("branch");
        let assistant_id = insert_message_and_get_id(
            &conn,
            "mne-evaluator",
            "assistant",
            "A shift in relationships.",
        )
        .expect("assistant");

        let patch = EnginePatch {
            soul_patch: Some(SoulPatch {
                relationship_deltas: vec![RelationshipDelta {
                    relationship_event_id: Some("rel-1".into()),
                    from: Some("Aurora".into()),
                    target: Some("Partner".into()),
                    trust: Some(15.0),
                    ..RelationshipDelta::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        record_turn_commit_with_patch(
            &conn,
            "mne-evaluator",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("record");

        let rebuilt =
            rebuild_session_state(&conn, "mne-evaluator", &branch.branch_id).expect("rebuild");

        let rel = rebuilt
            .soul
            .relationships
            .get("Partner")
            .expect("relationship exists");
        assert_eq!(rel.trust, 10.0);
    }

    #[test]
    fn stale_source_turn_not_on_active_branch_not_shown_as_current_failure() {
        let conn = init_memory_connection().unwrap();
        let conversation_id = "test-stale-convo";

        let mut soul = Soul::default();
        soul.character_id = "aurora_soul".into();
        soul.character_name = "Aurora".into();
        upsert_soul(&conn, &soul).unwrap();
        ensure_conversation(&conn, conversation_id, &soul.character_id).unwrap();

        let setting = SettingSoul::default_for_setting("starter_setting");
        upsert_setting(&conn, &setting).unwrap();

        let world = ensure_conversation_session_world(&conn, conversation_id, &soul, None).unwrap();
        let branch = create_session_branch(&conn, conversation_id, &soul, &world).unwrap();

        let msg1 =
            insert_message_and_get_id(&conn, conversation_id, "assistant", "Turn 1 msg").unwrap();
        let (commit1, _) = record_turn_commit_with_patch(
            &conn,
            conversation_id,
            &branch.branch_id,
            None,
            None,
            msg1,
            None,
            &EnginePatch::default(),
            false,
        )
        .unwrap();

        let msg2 =
            insert_message_and_get_id(&conn, conversation_id, "assistant", "Turn 2 msg").unwrap();
        let (commit2, _) = record_turn_commit_with_patch(
            &conn,
            conversation_id,
            &branch.branch_id,
            Some(&commit1.turn_id),
            None,
            msg2,
            None,
            &EnginePatch::default(),
            false,
        )
        .unwrap();

        conn.execute(
            "UPDATE session_branches SET active_turn_id = ?1 WHERE branch_id = ?2",
            [&commit1.turn_id, &branch.branch_id],
        )
        .unwrap();

        let job1 = EvaluatorJob {
            evaluator_job_id: "job1".into(),
            conversation_id: conversation_id.into(),
            turn_id: commit1.turn_id.clone(),
            assistant_message_id: msg1,
            status: "completed".into(),
            started_at: 1000,
            completed_at: Some(1500),
            elapsed_ms: Some(500),
            timeout_ms: None,
            timeout_mode: "finite".into(),
            model: Some("model".into()),
            provider: Some("provider".into()),
            error_message: None,
            patch_applied: true,
        };

        let job2 = EvaluatorJob {
            evaluator_job_id: "job2".into(),
            conversation_id: conversation_id.into(),
            turn_id: commit2.turn_id.clone(),
            assistant_message_id: msg2,
            status: "failed".into(),
            started_at: 2000,
            completed_at: Some(2500),
            elapsed_ms: Some(500),
            timeout_ms: None,
            timeout_mode: "finite".into(),
            model: Some("model".into()),
            provider: Some("provider".into()),
            error_message: Some("Failed".into()),
            patch_applied: false,
        };

        conn.execute(
            "INSERT INTO evaluator_background_jobs (evaluator_job_id, conversation_id, turn_id, assistant_message_id, status, started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode, model, provider, error_message, patch_applied)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job1.evaluator_job_id, job1.conversation_id, job1.turn_id, job1.assistant_message_id,
                job1.status, job1.started_at, job1.completed_at, job1.elapsed_ms, job1.timeout_ms,
                job1.timeout_mode, job1.model, job1.provider, job1.error_message, job1.patch_applied
            ]
        ).unwrap();

        conn.execute(
            "INSERT INTO evaluator_background_jobs (evaluator_job_id, conversation_id, turn_id, assistant_message_id, status, started_at, completed_at, elapsed_ms, timeout_ms, timeout_mode, model, provider, error_message, patch_applied)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job2.evaluator_job_id, job2.conversation_id, job2.turn_id, job2.assistant_message_id,
                job2.status, job2.started_at, job2.completed_at, job2.elapsed_ms, job2.timeout_ms,
                job2.timeout_mode, job2.model, job2.provider, job2.error_message, job2.patch_applied
            ]
        ).unwrap();

        let latest = get_latest_evaluator_job(&conn, conversation_id)
            .unwrap()
            .unwrap();
        assert_eq!(latest.evaluator_job_id, "job1");
    }

    #[test]
    fn startup_recovery_rebuilds_active_branch_and_marks_running_jobs_retryable() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Recovery");
        upsert_soul(&conn, &soul).expect("soul");
        ensure_conversation(&conn, "recovery-session", &soul.character_id).expect("conversation");
        let world = ensure_conversation_session_world(&conn, "recovery-session", &soul, None)
            .expect("world");
        let branch =
            create_session_branch(&conn, "recovery-session", &soul, &world).expect("branch");
        let assistant_id =
            insert_message_and_get_id(&conn, "recovery-session", "assistant", "Recovered.")
                .expect("assistant");
        let patch = EnginePatch {
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "Recovery memory survived restart.".into(),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            world_patch: Some(WorldPatch {
                corrected_object_states: vec![ObjectState {
                    object_id: "recovery_object_1".into(),
                    object_kind: "object".into(),
                    last_observed_state: "present after recovery".into(),
                    ..ObjectState::default()
                }],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };
        let (commit, _) = record_turn_commit_with_patch_for_turn_id(
            &conn,
            "turn_recovery",
            "recovery-session",
            &branch.branch_id,
            None,
            None,
            assistant_id,
            None,
            &patch,
            false,
        )
        .expect("commit");
        let mut stale_soul = soul.clone();
        stale_soul.memory.recent.clear();
        upsert_soul(&conn, &stale_soul).expect("stale soul");
        let mut stale_world = world.clone();
        stale_world.object_states.clear();
        upsert_session_world(&conn, &stale_world).expect("stale world");
        insert_evaluator_job(
            &conn,
            &EvaluatorJob {
                evaluator_job_id: "job-recovery-running".into(),
                conversation_id: "recovery-session".into(),
                turn_id: commit.turn_id,
                assistant_message_id: assistant_id,
                status: "running".into(),
                started_at: now_ts(),
                completed_at: None,
                elapsed_ms: None,
                timeout_ms: Some(25_000),
                timeout_mode: "finite".into(),
                model: Some("model".into()),
                provider: Some("provider".into()),
                error_message: None,
                patch_applied: false,
            },
        )
        .expect("job");

        let report = recover_incomplete_sessions_on_startup(&conn).expect("recover");

        assert_eq!(report.branches_rebuilt, 1);
        assert_eq!(report.running_jobs_marked_retryable, 1);
        assert!(report
            .pending_job_ids
            .contains(&"job-recovery-running".to_string()));
        let recovered_soul = get_soul(&conn, &soul.character_id).expect("recovered soul");
        assert_eq!(recovered_soul.memory.recent.len(), 1);
        let recovered_world = get_conversation_session_world(&conn, "recovery-session")
            .expect("world")
            .expect("linked world");
        assert_eq!(recovered_world.object_states.len(), 1);
        assert_eq!(
            get_evaluator_job(&conn, "job-recovery-running")
                .expect("job")
                .expect("job")
                .status,
            "pending"
        );
    }

    #[test]
    fn test_backups() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = temp_dir.path().join("mnemosyne.sqlite3");
        let conn = Connection::open(&db_path).expect("open");
        run_migrations(&conn).expect("migrations");

        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");

        let backup_dir = temp_dir.path().join("backups");
        let backup_path = create_backup_file(&db_path, &backup_dir).expect("backup");

        assert!(backup_path.exists(), "Backup file should exist");
        let metadata = std::fs::metadata(&backup_path).expect("metadata");
        assert!(metadata.len() > 0, "Backup file should have nonzero size");
    }

    #[test]
    fn test_session_archive_and_restore() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");

        // Create sibling sessions
        let sess_a = "session-a";
        let sess_b = "session-b";
        ensure_conversation(&conn, sess_a, &soul.character_id).expect("create session a");
        ensure_conversation(&conn, sess_b, &soul.character_id).expect("create session b");

        // Add a message and a payload log to session A
        let msg_id =
            insert_message_and_get_id(&conn, sess_a, "user", "Message in A").expect("insert msg");

        // Add an LLM payload log to session A
        conn.execute(
            "INSERT INTO llm_payload_logs (conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 10, 10, 20, 1000)",
            params![sess_a, msg_id, "openai", "chat", "gpt-4", "http://api", "sys", "usr", "ctx"]
        ).expect("insert log");

        // Verify both sessions are active
        let active = list_conversations(&conn).expect("list");
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|c| c.conversation_id == sess_a));
        assert!(active.iter().any(|c| c.conversation_id == sess_b));

        // Archive session A
        let ok = archive_session(&conn, sess_a).expect("archive");
        assert!(ok);

        // Verify session A is hidden from active list but sibling B is unaffected
        let active_after = list_conversations(&conn).expect("list");
        assert_eq!(active_after.len(), 1);
        assert!(!active_after.iter().any(|c| c.conversation_id == sess_a));
        assert!(active_after.iter().any(|c| c.conversation_id == sess_b));

        // Verify session A appears in archived list
        let archived = list_archived_sessions(&conn).expect("archived");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].conversation_id, sess_a);
        assert!(archived[0].title.starts_with("[Archived] "));

        // Verify archive did not delete the message
        let messages = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                [sess_a],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count");
        assert_eq!(messages, 1);

        // Verify archive did not delete the payload logs
        let logs = conn
            .query_row(
                "SELECT COUNT(*) FROM llm_payload_logs WHERE conversation_id = ?1",
                [sess_a],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count");
        assert_eq!(logs, 1);

        // Restore session A
        let ok = restore_session(&conn, sess_a).expect("restore");
        assert!(ok);

        // Verify A is back in active sessions and its title is stripped of prefix
        let active_restored = list_conversations(&conn).expect("list");
        assert_eq!(active_restored.len(), 2);
        let restored_a = active_restored
            .iter()
            .find(|c| c.conversation_id == sess_a)
            .unwrap();
        assert!(!restored_a.title.starts_with("[Archived] "));
        assert_eq!(restored_a.archived_at, None);
    }

    #[test]
    fn test_turn_hide_and_restore() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");

        let conversation_id = "hide-restore-conv";
        ensure_conversation(&conn, conversation_id, &soul.character_id).expect("create conv");

        // Insert messages
        let msg_u1 = insert_message_and_get_id(&conn, conversation_id, "user", "U1").unwrap();
        let msg_a1 = insert_message_and_get_id(&conn, conversation_id, "assistant", "A1").unwrap();
        let msg_u2 = insert_message_and_get_id(&conn, conversation_id, "user", "U2").unwrap();
        let msg_a2 = insert_message_and_get_id(&conn, conversation_id, "assistant", "A2").unwrap();

        // Setup session branch
        let branch_id = "branch-1";
        conn.execute(
            "INSERT INTO session_branches (branch_id, conversation_id, base_soul_json, base_session_world_json, rebuild_generation, is_active, created_at, updated_at)
             VALUES (?1, ?2, '{}', '{}', 1, 1, 1000, 1000)",
            params![branch_id, conversation_id]
        ).unwrap();

        // Setup commits
        conn.execute(
            "INSERT INTO turn_commits (turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, created_at, active_variant, is_active, is_discarded)
             VALUES ('turn-1', ?1, ?2, NULL, ?3, ?4, 1001, 1, 1, 0)",
            params![conversation_id, branch_id, msg_u1, msg_a1]
        ).unwrap();

        conn.execute(
            "INSERT INTO turn_commits (turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, created_at, active_variant, is_active, is_discarded)
             VALUES ('turn-2', ?1, ?2, 'turn-1', ?3, ?4, 1002, 1, 1, 0)",
            params![conversation_id, branch_id, msg_u2, msg_a2]
        ).unwrap();

        // Setup state patch for turn-2
        conn.execute(
            "INSERT INTO state_patches (patch_id, turn_id, patch_json, applied_at, applies_to, is_active)
             VALUES ('patch-2', 'turn-2', '{}', 1002, 'session', 1)",
            []
        ).unwrap();

        // Set active turn on branch
        conn.execute(
            "UPDATE session_branches SET active_turn_id = 'turn-2' WHERE branch_id = ?1",
            [branch_id],
        )
        .unwrap();

        // Insert some extra messages with various statuses to test skipping
        let msg_pending =
            insert_message_and_get_id(&conn, conversation_id, "assistant", "Pending response")
                .unwrap();
        conn.execute(
            "UPDATE messages SET message_status = 'pending', is_active = 0 WHERE id = ?1",
            [msg_pending],
        )
        .unwrap();

        let msg_failed =
            insert_message_and_get_id(&conn, conversation_id, "assistant", "Failed response")
                .unwrap();
        conn.execute(
            "UPDATE messages SET message_status = 'failed', is_active = 0 WHERE id = ?1",
            [msg_failed],
        )
        .unwrap();

        // Add an LLM payload log linked to turn 2
        conn.execute(
            "INSERT INTO llm_payload_logs (conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at)
             VALUES (?1, ?2, 'openai', 'chat', 'gpt-4', 'http://api', 'sys', 'usr', 'ctx', 10, 10, 20, 1000)",
            params![conversation_id, msg_a2]
        ).unwrap();

        // List messages, all 4 canonical messages are active/visible
        let active_msgs = list_messages(&conn, conversation_id, 10).unwrap();
        assert_eq!(active_msgs.len(), 4);

        // Hide turn 2 range (msg_u2 to msg_a2)
        let hidden_count = hide_turn_range(&conn, conversation_id, msg_u2, msg_a2).unwrap();
        assert_eq!(hidden_count, 2);

        // Verify only active turns (msg_u1 and msg_a1) remain in visible list
        let active_msgs_after = list_messages(&conn, conversation_id, 10).unwrap();
        assert_eq!(active_msgs_after.len(), 2);
        assert!(!active_msgs_after
            .iter()
            .any(|m| m.id == msg_u2 || m.id == msg_a2));

        // Verify payload logs remain unchanged
        let log_count = conn
            .query_row(
                "SELECT COUNT(*) FROM llm_payload_logs WHERE conversation_id = ?1",
                [conversation_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(log_count, 1);

        // Verify commits and patches are deactivated
        let commit_active: i64 = conn
            .query_row(
                "SELECT is_active FROM turn_commits WHERE turn_id = 'turn-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(commit_active, 0);

        let patch_active: i64 = conn
            .query_row(
                "SELECT is_active FROM state_patches WHERE patch_id = 'patch-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(patch_active, 0);

        // Verify they are listed in hidden turns (along with other hidden ones)
        let hidden = list_hidden_turns(&conn, conversation_id).unwrap();
        assert!(hidden
            .iter()
            .any(|m| m.id == msg_u2 && m.hidden_at.is_some()));
        assert!(hidden
            .iter()
            .any(|m| m.id == msg_a2 && m.hidden_at.is_some()));

        // Restore turn 2 range
        let restored_count =
            restore_turn_range(&conn, conversation_id, msg_u2, msg_failed).unwrap();
        // Should restore exactly msg_u2 and msg_a2 (2 messages), skipping msg_pending and msg_failed!
        assert_eq!(restored_count, 2);

        // Verify restored turns are active and visible
        let active_msgs_restored = list_messages(&conn, conversation_id, 10).unwrap();
        assert_eq!(active_msgs_restored.len(), 4);
        assert!(active_msgs_restored.iter().any(|m| m.id == msg_u2));
        assert!(active_msgs_restored.iter().any(|m| m.id == msg_a2));

        // Verify pending and failed messages remain inactive
        let pending_active: i64 = conn
            .query_row(
                "SELECT is_active FROM messages WHERE id = ?1",
                [msg_pending],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending_active, 0);

        // Verify turn commits and patches are reactivated
        let restored_commit_active: i64 = conn
            .query_row(
                "SELECT is_active FROM turn_commits WHERE turn_id = 'turn-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored_commit_active, 1);

        let restored_patch_active: i64 = conn
            .query_row(
                "SELECT is_active FROM state_patches WHERE patch_id = 'patch-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored_patch_active, 1);
    }

    #[test]
    fn test_soul_archive_and_restore() {
        let conn = init_memory_connection().expect("db");
        let soul_a = new_default_soul("Aurora");
        let soul_b = new_default_soul("Luna");
        upsert_soul(&conn, &soul_a).expect("upsert a");
        upsert_soul(&conn, &soul_b).expect("upsert b");

        // Verify sibling savepoints/checkpoints exist
        let mut savepoint_c = new_default_soul("Aurora Snapshot");
        savepoint_c.soul_kind = "checkpoint".into();
        savepoint_c.source_soul_id = Some(soul_a.character_id.clone());
        upsert_soul(&conn, &savepoint_c).expect("upsert c");

        // Create a conversation for Soul A
        let conv_id = "soul-a-conv";
        ensure_conversation(&conn, conv_id, &soul_a.character_id).expect("create conversation");

        // Insert message for Soul A
        let msg_id =
            insert_message_and_get_id(&conn, conv_id, "user", "Hello").expect("insert message");

        // Insert LLM payload log for Soul A
        conn.execute(
            "INSERT INTO llm_payload_logs (conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at)
             VALUES (?1, ?2, 'openai', 'chat', 'gpt-4', 'http://api', 'sys', 'usr', 'ctx', 10, 10, 20, 1000)",
            params![conv_id, msg_id]
        ).expect("insert log");

        // Verify active list
        let active = list_souls(&conn).expect("list");
        assert_eq!(active.len(), 3);
        assert!(active.iter().any(|s| s.character_id == soul_a.character_id));
        assert!(active.iter().any(|s| s.character_id == soul_b.character_id));
        assert!(active
            .iter()
            .any(|s| s.character_id == savepoint_c.character_id));

        // 1. Active sessions guard Soul archive until the session is archived first.
        let blocked = archive_soul(&conn, &soul_a.character_id).expect_err("active guard");
        assert!(blocked
            .to_string()
            .contains("Cannot archive Soul while active sessions use it"));
        assert!(archive_session(&conn, conv_id).expect("archive session first"));

        // 2. Archive Soul A
        let ok = archive_soul(&conn, &soul_a.character_id).expect("archive soul");
        assert!(ok);

        // 3. Verify archive_soul hides from active list
        let active_after = list_souls(&conn).expect("list");
        assert_eq!(active_after.len(), 2);
        assert!(!active_after
            .iter()
            .any(|s| s.character_id == soul_a.character_id));
        // Verify sibling Soul B and Savepoint C are unaffected
        assert!(active_after
            .iter()
            .any(|s| s.character_id == soul_b.character_id));
        assert!(active_after
            .iter()
            .any(|s| s.character_id == savepoint_c.character_id));

        // 4. Verify archived Soul appears in archived list
        let archived = list_archived_souls(&conn).expect("archived");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].character_id, soul_a.character_id);

        // 5. Verify archive does NOT delete conversations
        let conv_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                [conv_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count")
            > 0;
        assert!(conv_exists);

        // 6. Verify archive does NOT delete messages
        let msg_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE id = ?1",
                [msg_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count")
            > 0;
        assert!(msg_exists);

        // 7. Verify archive does NOT delete savepoints
        let savepoint_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM souls WHERE character_id = ?1",
                [&savepoint_c.character_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count")
            > 0;
        assert!(savepoint_exists);

        // 7. Verify archive does NOT delete payload logs
        let logs_exist = conn
            .query_row(
                "SELECT COUNT(*) FROM llm_payload_logs WHERE conversation_id = ?1",
                [conv_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count")
            > 0;
        assert!(logs_exist);

        // 8. Restore Soul A
        let ok = restore_soul(&conn, &soul_a.character_id).expect("restore");
        assert!(ok);

        // 9. Verify reappeared in active list
        let active_restored = list_souls(&conn).expect("list");
        assert_eq!(active_restored.len(), 3);
        assert!(active_restored
            .iter()
            .any(|s| s.character_id == soul_a.character_id));
    }

    #[test]
    fn test_savepoint_archive_and_restore() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");

        let mut savepoint_a = new_default_soul("Aurora Snapshot A");
        savepoint_a.soul_kind = "checkpoint".into();
        savepoint_a.source_soul_id = Some(soul.character_id.clone());
        upsert_soul(&conn, &savepoint_a).expect("upsert savepoint a");

        let mut savepoint_b = new_default_soul("Aurora Snapshot B");
        savepoint_b.soul_kind = "checkpoint".into();
        savepoint_b.source_soul_id = Some(soul.character_id.clone());
        upsert_soul(&conn, &savepoint_b).expect("upsert savepoint b");

        // Verify they are both active
        let active = list_souls(&conn).expect("list");
        assert_eq!(active.len(), 3);

        // 1. Archive Savepoint A
        let ok = archive_savepoint(&conn, &savepoint_a.character_id).expect("archive savepoint");
        assert!(ok);

        // 2. Verify archive hides from active list and does NOT affect sibling B
        let active_after = list_souls(&conn).expect("list");
        assert_eq!(active_after.len(), 2);
        assert!(!active_after
            .iter()
            .any(|s| s.character_id == savepoint_a.character_id));
        assert!(active_after
            .iter()
            .any(|s| s.character_id == savepoint_b.character_id));

        // 3. Verify archived savepoint appears in list_archived_savepoints
        let archived = list_archived_savepoints(&conn).expect("archived");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].character_id, savepoint_a.character_id);

        // 4. Verify archive did NOT delete savepoint data (row still exists in souls table)
        let row_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM souls WHERE character_id = ?1",
                [&savepoint_a.character_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("query count")
            > 0;
        assert!(row_exists);

        // 5. Restore Savepoint A
        let ok = restore_savepoint(&conn, &savepoint_a.character_id).expect("restore savepoint");
        assert!(ok);

        // 6. Verify reappeared in active list and preserved linked soul info
        let active_restored = list_souls(&conn).expect("list");
        assert_eq!(active_restored.len(), 3);
        let restored_a = active_restored
            .iter()
            .find(|s| s.character_id == savepoint_a.character_id)
            .unwrap();
        assert_eq!(restored_a.source_soul_id, Some(soul.character_id.clone()));
    }
}
