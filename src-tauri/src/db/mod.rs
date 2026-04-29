use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use state_engine::soul::Soul;
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
pub struct ChatMessage {
    pub id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
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
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )?;
    Ok(())
}

pub fn delete_conversation(conn: &Connection, conversation_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])?;
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

pub fn count_assistant_messages(conn: &Connection, conversation_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND role = 'assistant'",
        [conversation_id],
        |row| row.get(0),
    )
}

fn decode_soul(json: &str) -> rusqlite::Result<Soul> {
    serde_json::from_str(json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
