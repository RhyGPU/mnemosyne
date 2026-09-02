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

mod models;
pub use models::*;
mod schema;
pub use schema::run_migrations;
mod repositories;
pub use repositories::*;

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

pub fn touch_conversation_access(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<ConversationSummary> {
    let affected = conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2 AND archived_at IS NULL",
        params![now_ts(), conversation_id],
    )?;
    if affected == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
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

pub fn list_archived_player_personas(conn: &Connection) -> rusqlite::Result<Vec<PlayerPersona>> {
    let mut stmt = conn.prepare(
        "
        SELECT persona_id, display_name, description, gender_code, pronouns, appearance, voice_style, boundaries, notes, is_archived, created_at, updated_at
        FROM player_personas
        WHERE is_archived = 1
        ORDER BY updated_at DESC, display_name COLLATE NOCASE ASC
        ",
    )?;
    let rows = stmt.query_map([], player_persona_from_row)?;
    rows.collect()
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
            // Replay stamps memories with the turn's recorded time, so two
            // rebuilds of the same ledger produce identical projections even if
            // they straddle a clock tick.
            patch
                .apply_to_session_at(
                    &mut soul,
                    Some(&mut session_world),
                    commit.created_at.max(0) as u64,
                )
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
    rebuild_memory_v2_projection(conn, conversation_id, branch_id, &soul)?;
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
#[path = "tests.rs"]
mod tests;
