use rusqlite::{params, Connection};

use super::{decode_setting, decode_soul};

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

        CREATE TABLE IF NOT EXISTS compiler_runs (
            run_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            source_hash TEXT NOT NULL,
            mode TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            compiler_version INTEGER NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            prompt_version TEXT NOT NULL,
            status TEXT NOT NULL,
            enforcement_level TEXT NOT NULL,
            raw_response_json TEXT,
            artifact_json TEXT,
            error_message TEXT,
            commit_allowed INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_compiler_runs_conversation_created
        ON compiler_runs(conversation_id, created_at, run_id);

        CREATE INDEX IF NOT EXISTS idx_compiler_runs_source_hash
        ON compiler_runs(source_hash);

        CREATE TABLE IF NOT EXISTS memory_v2_entries (
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            memory_id TEXT NOT NULL,
            layer TEXT NOT NULL,
            memory_kind TEXT NOT NULL,
            owner_entity_id TEXT,
            content TEXT NOT NULL,
            source_patch_id TEXT,
            source_turn_id TEXT,
            source_message_id INTEGER,
            source_entity_id TEXT,
            source_quote TEXT,
            source_memory_ids_json TEXT NOT NULL DEFAULT '[]',
            supporting_evidence_json TEXT NOT NULL DEFAULT '[]',
            contradicting_evidence_json TEXT NOT NULL DEFAULT '[]',
            confidence REAL NOT NULL,
            truth_status TEXT NOT NULL,
            validity TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            compiler_version INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL,
            PRIMARY KEY (conversation_id, branch_id, memory_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_memory_v2_recall
        ON memory_v2_entries(conversation_id, branch_id, validity, layer, memory_kind);

        CREATE INDEX IF NOT EXISTS idx_memory_v2_source_patch
        ON memory_v2_entries(source_patch_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_v2_fts USING fts5(
            memory_id UNINDEXED,
            conversation_id UNINDEXED,
            branch_id UNINDEXED,
            content,
            source_quote,
            tokenize = 'unicode61'
        );

        CREATE TABLE IF NOT EXISTS memory_v2_edges (
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            from_memory_id TEXT NOT NULL,
            to_memory_id TEXT NOT NULL,
            edge_kind TEXT NOT NULL,
            weight REAL NOT NULL DEFAULT 1.0,
            PRIMARY KEY (
                conversation_id, branch_id, from_memory_id, to_memory_id, edge_kind
            ),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_memory_v2_edges_from
        ON memory_v2_edges(conversation_id, branch_id, from_memory_id);

        CREATE TABLE IF NOT EXISTS memory_v2_projection_generations (
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            generation INTEGER NOT NULL,
            entry_count INTEGER NOT NULL,
            rebuilt_at INTEGER NOT NULL,
            PRIMARY KEY (conversation_id, branch_id),
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS memory_v2_consolidation_runs (
            run_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            projection_generation INTEGER NOT NULL,
            trigger_reason TEXT NOT NULL,
            raw_memory_count INTEGER NOT NULL,
            proposed_count INTEGER NOT NULL,
            stored_count INTEGER NOT NULL,
            rejected_count INTEGER NOT NULL,
            status TEXT NOT NULL,
            artifact_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_memory_v2_consolidation_runs
        ON memory_v2_consolidation_runs(conversation_id, branch_id, projection_generation);

        CREATE TABLE IF NOT EXISTS memory_correction_events (
            event_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            target_assistant_message_id INTEGER,
            instruction TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (branch_id) REFERENCES session_branches(branch_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_memory_correction_events_branch
        ON memory_correction_events(conversation_id, branch_id, created_at, event_id);

        CREATE TABLE IF NOT EXISTS compiler_candidates (
            run_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            kind TEXT NOT NULL,
            disposition TEXT NOT NULL DEFAULT 'shadow',
            candidate_json TEXT NOT NULL,
            diagnostics_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (run_id, candidate_id),
            FOREIGN KEY (run_id) REFERENCES compiler_runs(run_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_compiler_candidates_run_index
        ON compiler_candidates(run_id, candidate_index);
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

    raise_shadowed_evaluator_timeouts(conn)?;
    adopt_perception_compiler_for_untouched_profiles(conn)?;

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

/// Evaluator ceilings this project set for people, rather than ones they chose.
///
/// 25s came from when the field was never the value that applied: three timeout
/// fields fed one call and the front end filled all three, so the diagnostic
/// default shadowed whatever a profile stored. 120s came from the migration that
/// fixed that, sized against extractions measured at 49-70s.
///
/// Both are now too tight. A reasoning evaluator's cost grows with the
/// transcript it is reading, and the same model on the same session walked
/// 49s -> 69s -> 86s -> 103s -> 106s across five turns before crossing 120s and
/// losing a turn's state to a timeout. The ceiling has to sit above where that
/// curve is going, not where it started. A number someone typed is still theirs.
const PROJECT_SET_EVALUATOR_TIMEOUTS_MS: [i64; 2] = [25_000, 120_000];
const REPLACEMENT_EVALUATOR_TIMEOUT_MS: i64 = 180_000;

/// Evaluator modes the project shipped as a default, rather than ones a person
/// picked. Both predate the perception compiler being the path under test.
///
/// The compiler is the post-overhaul route: the model reports what it perceived
/// and Rust decides what persists, instead of the model proposing state changes
/// directly. Profiles still carrying an older shipped default move to it; a mode
/// someone selected stays selected.
const PROJECT_SET_EVALUATOR_MODES: [&str; 2] = ["evaluator_form_v1", "evaluator_structured_v1"];

fn adopt_perception_compiler_for_untouched_profiles(conn: &Connection) -> rusqlite::Result<()> {
    for superseded in PROJECT_SET_EVALUATOR_MODES {
        conn.execute(
            "
            UPDATE provider_profiles
            SET evaluator_mode = 'evaluator_perception_v2'
            WHERE evaluator_mode = ?1
            ",
            params![superseded],
        )?;
    }
    Ok(())
}

fn raise_shadowed_evaluator_timeouts(conn: &Connection) -> rusqlite::Result<()> {
    for superseded in PROJECT_SET_EVALUATOR_TIMEOUTS_MS {
        conn.execute(
            "
            UPDATE provider_profiles
            SET evaluator_timeout_ms = ?1
            WHERE evaluator_timeout_ms = ?2
            ",
            params![REPLACEMENT_EVALUATOR_TIMEOUT_MS, superseded],
        )?;
    }
    Ok(())
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
