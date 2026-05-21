use std::{
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingSummary {
    pub setting_id: String,
    pub setting_name: String,
    pub last_updated: i64,
    pub turn_counter: u64,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
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
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
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
            setting_json TEXT NOT NULL
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
            title TEXT NOT NULL DEFAULT 'Untitled Session',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(character_id) ON DELETE CASCADE,
            FOREIGN KEY (world_id) REFERENCES session_worlds(world_id) ON DELETE SET NULL,
            FOREIGN KEY (source_setting_id) REFERENCES settings(setting_id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation_id_id
        ON messages(conversation_id, id);

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
    add_column_if_missing(conn, "messages", "branch_id", "TEXT")?;
    add_column_if_missing(conn, "messages", "is_active", "INTEGER NOT NULL DEFAULT 1")?;
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
    if added_soul_summary_columns {
        backfill_soul_summary_columns(conn)?;
    }
    if added_setting_summary_columns {
        backfill_setting_summary_columns(conn)?;
    }
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

    Ok(summarize_soul(soul))
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
        SELECT setting_id, setting_name, last_updated, turn_counter, location
        FROM settings
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

pub fn delete_setting(conn: &Connection, setting_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM settings WHERE setting_id = ?1", [setting_id])?;
    Ok(affected > 0)
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
               avatar_image_id, last_updated, recent_count, core_count
        FROM souls
        ORDER BY last_updated DESC, character_name ASC
        "
    } else {
        "
        SELECT character_id, character_name, soul_kind, source_soul_id, source_savepoint_id,
               avatar_image_id, last_updated, recent_count, core_count
        FROM souls
        WHERE soul_kind != 'session_clone'
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

pub fn delete_soul(conn: &Connection, soul_id: &str) -> rusqlite::Result<bool> {
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
        INSERT INTO conversations (id, soul_id, world_id, source_setting_id, title, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
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
            c.created_at,
            c.updated_at,
            (
                SELECT content
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0
                ORDER BY id DESC
                LIMIT 1
            ) AS last_message_preview,
            (
                SELECT COUNT(*)
                FROM messages
                WHERE conversation_id = c.id AND is_active != 0
            ) AS message_count
        FROM conversations c
        LEFT JOIN souls s ON s.character_id = c.soul_id
        ORDER BY c.updated_at DESC, c.created_at DESC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let preview: Option<String> = row.get(8)?;
        Ok(ConversationSummary {
            conversation_id: row.get(0)?,
            title: row.get(1)?,
            soul_id: row.get(2)?,
            source_savepoint_id: row.get(3)?,
            world_id: row.get(4)?,
            source_setting_id: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            last_message_preview: preview.map(compact_preview),
            message_count: row.get(9)?,
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
        SELECT c.id, c.title, c.soul_id, c.created_at, c.updated_at, COALESCE(s.source_savepoint_id, NULL), c.world_id, c.source_setting_id
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
                last_message_preview,
                message_count,
            })
        },
    )
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

fn last_message_preview(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "
        SELECT content FROM messages
        WHERE conversation_id = ?1 AND is_active != 0
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
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND is_active != 0",
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
    let now = now_ts();
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![conversation_id, role, content, now],
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
    let now = now_ts();
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![conversation_id, role, content, now],
    )?;
    let message_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )?;
    Ok(message_id)
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
        WHERE conversation_id = ?1 AND message_id = ?2
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
            (conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
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
        ],
    )?;
    Ok(conn.last_insert_rowid())
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
        SELECT id, conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id
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
        SELECT id, conversation_id, message_id, provider, mode, context_mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, truncated, created_at, branch_id, active_turn_id, parent_turn_id, state_patch_ids_applied_json, discarded_patch_ids_skipped_json, state_rebuild_generation, latest_assistant_variant_id
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
        SELECT id, conversation_id, role, content, created_at
        FROM (
            SELECT id, conversation_id, role, content, created_at
            FROM messages
            WHERE conversation_id = ?1 AND id < ?2 AND is_active != 0
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
                created_at: row.get(4)?,
                attachments: list_message_attachments(conn, message_id)?,
            })
        },
    )?;

    rows.collect()
}

pub fn delete_conversation(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    let linked: Option<(String, Option<String>, String)> = conn
        .query_row(
            "
            SELECT c.soul_id, c.world_id, COALESCE(s.soul_kind, '')
            FROM conversations c
            LEFT JOIN souls s ON s.character_id = c.soul_id
            WHERE c.id = ?1
            ",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let affected = conn.execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])?;
    if affected > 0 {
        if let Some((soul_id, world_id, soul_kind)) = linked {
            if normalized_soul_kind(&soul_kind) == "session_clone" {
                let still_used: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM conversations WHERE soul_id = ?1",
                    [&soul_id],
                    |row| row.get(0),
                )?;
                if still_used == 0 {
                    let _ = conn.execute("DELETE FROM souls WHERE character_id = ?1", [&soul_id]);
                }
            }
            if let Some(world_id) = world_id {
                let still_used: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM conversations WHERE world_id = ?1",
                    [&world_id],
                    |row| row.get(0),
                )?;
                if still_used == 0 {
                    let _ = conn.execute(
                        "DELETE FROM session_worlds WHERE world_id = ?1",
                        [&world_id],
                    );
                }
            }
        }
    }
    Ok(affected > 0)
}

pub fn delete_message(
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

pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
    limit: usize,
) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, role, content, created_at
        FROM (
            SELECT id, conversation_id, role, content, created_at
            FROM messages
            WHERE conversation_id = ?1 AND is_active != 0
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
            created_at: row.get(4)?,
            attachments: list_message_attachments(conn, message_id)?,
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
        SELECT id, conversation_id, role, content, created_at
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
                created_at: row.get(4)?,
                attachments: list_message_attachments(conn, message_id)?,
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
    let now = now_ts();
    let turn_id = ledger_id("turn");
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
            (patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id)
        VALUES (?1, ?2, NULL, ?3, NULL, ?4, 'session', 1, NULL, NULL)
        ",
        params![patch_id, turn_id, patch_json, now],
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
        if let Some(patch_id) = commit.state_patch_id.as_deref() {
            conn.execute(
                "UPDATE state_patches SET is_active = 0 WHERE patch_id = ?1",
                [patch_id],
            )?;
        }
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
        "UPDATE messages SET is_active = 0 WHERE conversation_id = ?1 AND id >= ?2",
        params![conversation_id, message_id],
    )?;
    let commits = list_commits_at_or_after_message(conn, conversation_id, message_id)?;
    let mut count = 0;
    for commit in commits {
        if let Some(patch_id) = commit.state_patch_id.as_deref() {
            conn.execute(
                "UPDATE state_patches SET is_active = 0 WHERE patch_id = ?1",
                [patch_id],
            )?;
        }
        conn.execute(
            "UPDATE turn_commits SET is_active = 0, is_discarded = 1, active_variant = 0 WHERE turn_id = ?1",
            [commit.turn_id],
        )?;
        count += 1;
    }
    Ok(count)
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
    if let Some(patch_id) = commit.state_patch_id.as_deref() {
        conn.execute(
            "UPDATE state_patches SET is_active = 1 WHERE patch_id = ?1",
            [patch_id],
        )?;
    }
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
        let Some(patch_id) = commit.state_patch_id.as_deref() else {
            continue;
        };
        let patch_record = get_state_patch(conn, patch_id)?;
        if !patch_record.is_active || commit.is_discarded || !commit.is_active {
            debug.skipped_discarded_patches.push(patch_id.to_string());
            continue;
        }
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
        debug.applied_patches.push(patch_id.to_string());
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
    })
}

fn get_turn_commit(conn: &Connection, turn_id: &str) -> rusqlite::Result<TurnCommit> {
    conn.query_row(
        "SELECT turn_id, conversation_id, branch_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, created_at, active_variant, is_active, is_discarded, is_regenerated_variant FROM turn_commits WHERE turn_id = ?1",
        [turn_id],
        turn_commit_from_row,
    )
}

fn get_state_patch(conn: &Connection, patch_id: &str) -> rusqlite::Result<StatePatchRecord> {
    conn.query_row(
        "SELECT patch_id, turn_id, parent_state_hash, patch_json, inverse_patch_json, applied_at, applies_to, is_active, invalidated_by_patch_id, supersedes_patch_id FROM state_patches WHERE patch_id = ?1",
        [patch_id],
        state_patch_from_row,
    )
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
    let mut commits = Vec::new();
    for row in rows {
        let commit = row?;
        let reached = until_turn_id.is_some_and(|turn_id| commit.turn_id == turn_id);
        commits.push(commit);
        if reached {
            break;
        }
    }
    Ok(commits)
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
        INSERT INTO provider_profiles (id, name, base_url, api_key, model, system_prompt, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            base_url = excluded.base_url,
            api_key = excluded.api_key,
            model = excluded.model,
            system_prompt = excluded.system_prompt,
            updated_at = excluded.updated_at
        ",
        params![
            updated.id,
            updated.name,
            updated.base_url,
            updated.api_key,
            updated.model,
            updated.system_prompt,
            updated.created_at,
            updated.updated_at
        ],
    )?;
    Ok(updated)
}

pub fn list_provider_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProviderProfile>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at
        FROM provider_profiles
        ORDER BY updated_at DESC, name ASC
        ",
    )?;
    let rows = stmt.query_map([], provider_profile_from_row)?;
    rows.collect()
}

pub fn get_provider_profile(conn: &Connection, id: &str) -> rusqlite::Result<ProviderProfile> {
    conn.query_row(
        "
        SELECT id, name, base_url, api_key, model, system_prompt, created_at, updated_at
        FROM provider_profiles
        WHERE id = ?1
        ",
        [id],
        provider_profile_from_row,
    )
}

pub fn delete_provider_profile(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM provider_profiles WHERE id = ?1", [id])?;
    Ok(affected > 0)
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state_engine::patch::{EnginePatch, WorldEventOperationPatch, WorldPatch};
    use state_engine::setting::new_default_setting;
    use state_engine::soul::{
        new_default_soul, session_soul_from_savepoint, soul_savepoint_from_session,
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

        assert!(delete_message(&conn, "variants", message_id).expect("delete message"));
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
    fn deletion_cascades_souls_and_conversations() {
        let conn = init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        upsert_soul(&conn, &soul).expect("upsert");
        ensure_conversation(&conn, "mock", &soul.character_id).expect("conversation");
        insert_message(&conn, "mock", "user", "Hello").expect("user");

        assert!(delete_conversation(&conn, "mock").expect("delete conversation"));
        assert_eq!(list_messages(&conn, "mock", 5).expect("messages").len(), 0);

        ensure_conversation(&conn, "mock", &soul.character_id).expect("conversation");
        insert_message(&conn, "mock", "assistant", "Hi").expect("assistant");
        assert!(delete_soul(&conn, &soul.character_id).expect("delete soul"));
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

        assert!(get_conversation_summary(&conn, "chat-a").is_err());
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

        assert!(get_conversation_summary(&conn, "session-a").is_err());
        assert!(get_soul(&conn, &session_a.character_id).is_err());
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
        assert!(delete_setting(&conn, &setting.setting_id).expect("delete"));
        assert!(list_settings(&conn).expect("list settings").is_empty());
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
        };

        let saved = upsert_provider_profile(&conn, &profile).expect("upsert");
        assert!(saved.created_at > 0);
        assert_eq!(list_provider_profiles(&conn).expect("list").len(), 1);
        assert_eq!(
            get_provider_profile(&conn, "openai").expect("get").model,
            "gpt"
        );
        assert!(delete_provider_profile(&conn, "openai").expect("delete"));
        assert!(list_provider_profiles(&conn).expect("list").is_empty());
    }
}
