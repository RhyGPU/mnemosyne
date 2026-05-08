use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use state_engine::{setting::SettingSoul, soul::Soul};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSummary {
    pub character_id: String,
    pub character_name: String,
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
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context_text: String,
    pub estimated_system_tokens: usize,
    pub estimated_user_tokens: usize,
    pub estimated_total_tokens: usize,
    pub created_at: i64,
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
            last_updated INTEGER NOT NULL,
            soul_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            setting_id TEXT PRIMARY KEY,
            setting_name TEXT NOT NULL,
            last_updated INTEGER NOT NULL,
            setting_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            soul_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (soul_id) REFERENCES souls(character_id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
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
            model TEXT NOT NULL,
            base_url TEXT NOT NULL,
            system_message TEXT NOT NULL,
            user_message TEXT NOT NULL,
            context_text TEXT NOT NULL,
            estimated_system_tokens INTEGER NOT NULL,
            estimated_user_tokens INTEGER NOT NULL,
            estimated_total_tokens INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
        );
        ",
    )
}

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn upsert_soul(conn: &Connection, soul: &Soul) -> rusqlite::Result<SoulSummary> {
    let soul_json = serde_json::to_string(soul)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO souls (character_id, character_name, last_updated, soul_json)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(character_id) DO UPDATE SET
            character_name = excluded.character_name,
            last_updated = excluded.last_updated,
            soul_json = excluded.soul_json
        ",
        params![
            soul.character_id,
            soul.character_name,
            soul.last_updated,
            soul_json
        ],
    )?;

    Ok(SoulSummary {
        character_id: soul.character_id.clone(),
        character_name: soul.character_name.clone(),
        last_updated: soul.last_updated,
        recent_count: soul.memory.recent.len(),
        core_count: soul.memory.core.len(),
    })
}

pub fn upsert_setting(
    conn: &Connection,
    setting: &SettingSoul,
) -> rusqlite::Result<SettingSummary> {
    let setting_json = serde_json::to_string(setting)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
    conn.execute(
        "
        INSERT INTO settings (setting_id, setting_name, last_updated, setting_json)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(setting_id) DO UPDATE SET
            setting_name = excluded.setting_name,
            last_updated = excluded.last_updated,
            setting_json = excluded.setting_json
        ",
        params![
            setting.setting_id,
            setting.setting_name,
            setting.last_updated,
            setting_json
        ],
    )?;

    Ok(summarize_setting(setting))
}

pub fn list_settings(conn: &Connection) -> rusqlite::Result<Vec<SettingSummary>> {
    let mut stmt = conn.prepare(
        "SELECT setting_json FROM settings ORDER BY last_updated DESC, setting_name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let setting_json: String = row.get(0)?;
        decode_setting(&setting_json).map(|setting| summarize_setting(&setting))
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
    let mut stmt =
        conn.prepare("SELECT soul_json FROM souls ORDER BY last_updated DESC, character_name ASC")?;
    let rows = stmt.query_map([], |row| {
        let soul_json: String = row.get(0)?;
        decode_soul(&soul_json).map(|soul| SoulSummary {
            character_id: soul.character_id,
            character_name: soul.character_name,
            last_updated: soul.last_updated,
            recent_count: soul.memory.recent.len(),
            core_count: soul.memory.core.len(),
        })
    })?;

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
            "SELECT soul_json FROM souls ORDER BY last_updated DESC LIMIT 1",
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
    let now = now_ts();
    conn.execute(
        "
        INSERT INTO conversations (id, soul_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(id) DO UPDATE SET soul_id = excluded.soul_id, updated_at = excluded.updated_at
        ",
        params![conversation_id, soul_id, now],
    )?;
    Ok(())
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
            (conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ",
        params![
            log.conversation_id,
            log.message_id,
            log.provider,
            log.mode,
            log.model,
            log.base_url,
            log.system_message,
            log.user_message,
            log.context_text,
            log.estimated_system_tokens as i64,
            log.estimated_user_tokens as i64,
            log.estimated_total_tokens as i64,
            log.created_at,
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

pub fn list_llm_payload_logs(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<LlmPayloadLog>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at
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
        SELECT id, conversation_id, message_id, provider, mode, model, base_url, system_message, user_message, context_text, estimated_system_tokens, estimated_user_tokens, estimated_total_tokens, created_at
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
            WHERE conversation_id = ?1 AND id < ?2
            ORDER BY id DESC
            LIMIT ?3
        )
        ORDER BY id ASC
        ",
    )?;

    let rows = stmt.query_map(
        params![conversation_id, before_message_id, limit as i64],
        |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    )?;

    rows.collect()
}

pub fn delete_conversation(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])?;
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
            WHERE conversation_id = ?1
            ORDER BY id DESC
            LIMIT ?2
        )
        ORDER BY id ASC
        ",
    )?;

    let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
        Ok(ChatMessage {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    rows.collect()
}

fn get_message(
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
            Ok(ChatMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    )
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
    let estimated_system_tokens: i64 = row.get(10)?;
    let estimated_user_tokens: i64 = row.get(11)?;
    let estimated_total_tokens: i64 = row.get(12)?;
    Ok(LlmPayloadLog {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        message_id: row.get(2)?,
        provider: row.get(3)?,
        mode: row.get(4)?,
        model: row.get(5)?,
        base_url: row.get(6)?,
        system_message: row.get(7)?,
        user_message: row.get(8)?,
        context_text: row.get(9)?,
        estimated_system_tokens: estimated_system_tokens.max(0) as usize,
        estimated_user_tokens: estimated_user_tokens.max(0) as usize,
        estimated_total_tokens: estimated_total_tokens.max(0) as usize,
        created_at: row.get(13)?,
    })
}

pub fn count_assistant_messages(conn: &Connection, conversation_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND role = 'assistant'",
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
    use state_engine::setting::new_default_setting;
    use state_engine::soul::new_default_soul;

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
                model: "debug-model".into(),
                base_url: "https://api.example/v1".into(),
                system_message: "System with context".into(),
                user_message: "User input".into(),
                context_text: "[LATEST EXCHANGE]".into(),
                estimated_system_tokens: 4,
                estimated_user_tokens: 2,
                estimated_total_tokens: 6,
                created_at: now_ts(),
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
