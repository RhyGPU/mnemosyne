use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::super::{active_commit_path_until, get_active_session_branch, now_ts, EvaluatorJob};

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
