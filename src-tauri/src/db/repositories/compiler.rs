use rusqlite::{params, Connection};

use super::super::{CompilerCandidateRecord, CompilerRunRecord};

pub fn record_compiler_run(
    conn: &Connection,
    run: &CompilerRunRecord,
    candidates: &[CompilerCandidateRecord],
) -> rusqlite::Result<()> {
    let transaction = conn.unchecked_transaction()?;
    transaction.execute(
        "
        INSERT INTO compiler_runs (
            run_id, conversation_id, branch_id, turn_id, source_hash, mode,
            schema_version, compiler_version, provider, model, prompt_version,
            status, enforcement_level, raw_response_json, artifact_json,
            error_message, commit_allowed, created_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15, ?16, ?17, ?18
        )
        ",
        params![
            run.run_id,
            run.conversation_id,
            run.branch_id,
            run.turn_id,
            run.source_hash,
            run.mode,
            run.schema_version,
            run.compiler_version,
            run.provider,
            run.model,
            run.prompt_version,
            run.status,
            run.enforcement_level,
            run.raw_response_json,
            run.artifact_json,
            run.error_message,
            run.commit_allowed,
            run.created_at,
        ],
    )?;
    for candidate in candidates {
        transaction.execute(
            "
            INSERT INTO compiler_candidates (
                run_id, candidate_id, candidate_index, kind, disposition,
                candidate_json, diagnostics_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                candidate.run_id,
                candidate.candidate_id,
                candidate.candidate_index as i64,
                candidate.kind,
                candidate.disposition,
                candidate.candidate_json,
                candidate.diagnostics_json,
            ],
        )?;
    }
    transaction.commit()
}

pub fn list_compiler_runs(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<CompilerRunRecord>> {
    let mut statement = conn.prepare(
        "
        SELECT run_id, conversation_id, branch_id, turn_id, source_hash, mode,
               schema_version, compiler_version, provider, model, prompt_version,
               status, enforcement_level, raw_response_json, artifact_json,
               error_message, commit_allowed, created_at
        FROM compiler_runs
        WHERE conversation_id = ?1
        ORDER BY created_at, run_id
        ",
    )?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(CompilerRunRecord {
                run_id: row.get(0)?,
                conversation_id: row.get(1)?,
                branch_id: row.get(2)?,
                turn_id: row.get(3)?,
                source_hash: row.get(4)?,
                mode: row.get(5)?,
                schema_version: row.get::<_, i64>(6)?.max(0) as u32,
                compiler_version: row.get::<_, i64>(7)?.max(0) as u32,
                provider: row.get(8)?,
                model: row.get(9)?,
                prompt_version: row.get(10)?,
                status: row.get(11)?,
                enforcement_level: row.get(12)?,
                raw_response_json: row.get(13)?,
                artifact_json: row.get(14)?,
                error_message: row.get(15)?,
                commit_allowed: row.get(16)?,
                created_at: row.get(17)?,
            })
        })?
        .collect();
    rows
}

pub fn list_compiler_candidates(
    conn: &Connection,
    run_id: &str,
) -> rusqlite::Result<Vec<CompilerCandidateRecord>> {
    let mut statement = conn.prepare(
        "
        SELECT run_id, candidate_id, candidate_index, kind, disposition,
               candidate_json, diagnostics_json
        FROM compiler_candidates
        WHERE run_id = ?1
        ORDER BY candidate_index, candidate_id
        ",
    )?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok(CompilerCandidateRecord {
                run_id: row.get(0)?,
                candidate_id: row.get(1)?,
                candidate_index: row.get::<_, i64>(2)?.max(0) as usize,
                kind: row.get(3)?,
                disposition: row.get(4)?,
                candidate_json: row.get(5)?,
                diagnostics_json: row.get(6)?,
            })
        })?
        .collect();
    rows
}
