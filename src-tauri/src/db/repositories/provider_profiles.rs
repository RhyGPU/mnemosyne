use rusqlite::{params, Connection, OptionalExtension};

use super::super::{now_ts, ProviderProfile};

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
