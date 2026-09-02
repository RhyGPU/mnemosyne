use super::*;
use state_engine::patch::{
    EnginePatch, MemoryPatch, RelationshipDelta, SceneStatePatch, SoulPatch,
    WorldEventOperationPatch, WorldPatch,
};
use state_engine::setting::new_default_setting;
use state_engine::soul::{
    new_default_soul, session_soul_from_savepoint, soul_savepoint_from_session, MemorySourceType,
    ObjectState, TruthStatus,
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
fn compiler_shadow_runs_and_candidates_persist_without_commit_authority() {
    let conn = init_memory_connection().expect("db");
    let soul = new_default_soul("Compiler Shadow");
    upsert_soul(&conn, &soul).expect("soul");
    ensure_conversation(&conn, "compiler-shadow", &soul.character_id).expect("conversation");

    let run = CompilerRunRecord {
        run_id: "compiler-run-1".into(),
        conversation_id: "compiler-shadow".into(),
        branch_id: "branch-main".into(),
        turn_id: "turn-1".into(),
        source_hash: "fnv1a64:abc".into(),
        mode: "perception_v2_shadow".into(),
        schema_version: 2,
        compiler_version: 2,
        provider: "test".into(),
        model: "test-model".into(),
        prompt_version: "perception-v2.0".into(),
        status: "validated".into(),
        enforcement_level: "json_schema".into(),
        raw_response_json: Some(r#"{"schema_version":2}"#.into()),
        artifact_json: Some(r#"{"source_hash":"fnv1a64:abc"}"#.into()),
        error_message: None,
        commit_allowed: false,
        created_at: 100,
    };
    let candidate = CompilerCandidateRecord {
        run_id: run.run_id.clone(),
        candidate_id: "candidate-1".into(),
        candidate_index: 0,
        kind: "event".into(),
        disposition: "shadow".into(),
        candidate_json: r#"{"candidate_id":"candidate-1"}"#.into(),
        diagnostics_json: "[]".into(),
    };
    record_compiler_run(&conn, &run, &[candidate.clone()]).expect("record");

    let runs = list_compiler_runs(&conn, "compiler-shadow").expect("runs");
    assert_eq!(runs, vec![run]);
    assert!(!runs[0].commit_allowed);
    assert_eq!(
        list_compiler_candidates(&conn, "compiler-run-1").expect("candidates"),
        vec![candidate]
    );
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
    let user_id =
        insert_message_and_get_id(&conn, "edit-user", "user", "Original user text").expect("user");
    let assistant_id = insert_message_and_get_id(&conn, "edit-user", "assistant", "Assistant text")
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
    let user_id =
        insert_message_and_get_id(&conn, "retry-guard", "user", "Open the door.").expect("user");

    let reusable = find_reusable_active_user_message(&conn, "retry-guard", "  Open   the door.  ")
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
    let canonical = insert_message_and_get_id(&conn, "repair-dupes", "user", "The same prompt.")
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
    let archived = list_archived_player_personas(&conn).expect("list archived");
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].persona_id, "persona_jun");
    assert!(restore_player_persona(&conn, "persona_jun").expect("restore custom"));
    assert!(list_archived_player_personas(&conn)
        .expect("list archived after restore")
        .is_empty());
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

    let renamed = rename_conversation(&conn, "session-title", "Renamed Session").expect("rename");
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
    let message_id = insert_message_and_get_id(&conn, "payloads", "assistant", "Response").unwrap();

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
    let rebuilt = rebuild_session_state(&conn, "enrich-prior", &branch.branch_id).expect("rebuild");

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

    let rebuilt = rebuild_session_state(&conn, "enrich-order", &branch.branch_id).expect("rebuild");
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
    let user_id = insert_message_and_get_id(&conn, "ledger-cache", "user", "Move.").expect("user");
    let assistant_id =
        insert_message_and_get_id(&conn, "ledger-cache", "assistant", "Moved.").expect("assistant");
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

    let rebuilt = rebuild_session_state(&conn, "ledger-cache", &branch.branch_id).expect("rebuild");

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
fn memory_v2_projection_rebuild_is_equivalent_and_drops_inactive_branch_memory() {
    let conn = init_memory_connection().expect("db");
    let soul = new_default_soul("Projection");
    upsert_soul(&conn, &soul).expect("soul");
    ensure_conversation(&conn, "memory-v2", &soul.character_id).expect("conversation");
    let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
    create_session_branch(&conn, "memory-v2", &soul, &world).expect("branch");
    let branch = get_active_session_branch(&conn, "memory-v2").expect("branch");
    let user_id =
        insert_message_and_get_id(&conn, "memory-v2", "user", "The gate is locked.").expect("user");
    let assistant_id = insert_message_and_get_id(
        &conn,
        "memory-v2",
        "assistant",
        "Aurora hears the visitor say the gate is locked.",
    )
    .expect("assistant");
    let patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            new_memories: vec![MemoryPatch {
                memory_id: Some("memory-testimony-1".into()),
                content: "The visitor said the gate is locked.".into(),
                tag: Some("testimony".into()),
                source_type: Some(MemorySourceType::UserClaimed),
                source_message_id: Some(user_id),
                source_entity_id: Some("active_player".into()),
                source_quote: Some("The gate is locked.".into()),
                perceived_by_entity_id: Some(soul.character_id.clone()),
                confidence: Some(0.8),
                truth_status: Some(TruthStatus::CharacterBelief),
                owner_soul_id: Some(soul.character_id.clone()),
                knowledge_scope: Some("heard_about".into()),
                ..MemoryPatch::default()
            }],
            ..SoulPatch::default()
        }),
        ..EnginePatch::default()
    };
    record_turn_commit_with_patch(
        &conn,
        "memory-v2",
        &branch.branch_id,
        None,
        Some(user_id),
        assistant_id,
        None,
        &patch,
        false,
    )
    .expect("record");

    // Rebuild must be a function of the ledger, not of the wall clock. Backdating
    // the commit proves the projection takes its time from the recorded turn:
    // with `current_timestamp()` in the apply path this assertion fails outright,
    // and the equality check below only failed when two rebuilds happened to
    // straddle a clock tick.
    let backdated = 1_600_000_000_i64;
    conn.execute(
        "UPDATE turn_commits SET created_at = ?1 WHERE conversation_id = ?2",
        rusqlite::params![backdated, "memory-v2"],
    )
    .expect("backdate commit");

    rebuild_session_state(&conn, "memory-v2", &branch.branch_id).expect("first rebuild");
    let first =
        list_memory_v2_projection(&conn, "memory-v2", &branch.branch_id, true).expect("first list");
    assert_eq!(
        first[0].created_at_ms, backdated,
        "rebuilt memory must carry the ledger turn time, not the rebuild time"
    );
    rebuild_session_state(&conn, "memory-v2", &branch.branch_id).expect("second rebuild");
    let second = list_memory_v2_projection(&conn, "memory-v2", &branch.branch_id, true)
        .expect("second list");

    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].memory_kind, "testimony");
    assert_eq!(first[0].truth_status, "character_belief");
    assert!(first[0].source_patch_id.is_some());

    let derived = state_engine::memory_v2::MemoryV2Entry {
        schema_version: state_engine::memory_v2::MEMORY_V2_SCHEMA_VERSION,
        memory_id: "belief-gate-locked".into(),
        conversation_id: "memory-v2".into(),
        branch_id: branch.branch_id.clone(),
        owner_entity_id: Some(soul.character_id.clone()),
        layer: state_engine::memory_v2::MemoryLayerV2::Derived,
        episodic_kind: None,
        derived_kind: Some(state_engine::memory_v2::DerivedMemoryKind::Belief),
        content: "The gate may be locked.".into(),
        source_patch_id: None,
        source_turn_id: None,
        source_message_id: None,
        source_entity_id: None,
        source_quote: None,
        source_memory_ids: vec!["memory-testimony-1".into()],
        supporting_evidence: vec![state_engine::memory_v2::MemoryEvidenceRef {
            source_memory_id: "memory-testimony-1".into(),
            source_patch_id: first[0].source_patch_id.clone(),
            source_quote: first[0].source_quote.clone(),
            relation: "supports".into(),
        }],
        contradicting_evidence: Vec::new(),
        confidence: 0.6,
        truth_status: TruthStatus::CharacterBelief,
        validity: state_engine::memory_v2::MemoryValidity::Valid,
        compiler_version: state_engine::compiler::MEMORY_COMPILER_CONTRACT_VERSION,
        created_at_ms: 1,
    };
    store_derived_memory_v2(&conn, &derived).expect("store derived");

    discard_active_commits_for_assistant(&conn, "memory-v2", assistant_id).expect("discard");
    rebuild_session_state(&conn, "memory-v2", &branch.branch_id).expect("rebuild after discard");
    let all_after_discard = list_memory_v2_projection(&conn, "memory-v2", &branch.branch_id, true)
        .expect("after discard");
    assert_eq!(all_after_discard.len(), 1);
    assert_eq!(all_after_discard[0].layer, "derived");
    assert_eq!(all_after_discard[0].validity, "stale");
    assert!(
        list_memory_v2_projection(&conn, "memory-v2", &branch.branch_id, false)
            .expect("active after discard")
            .is_empty()
    );
}

#[test]
fn memory_v2_consolidation_is_evidence_backed_and_idempotent() {
    let conn = init_memory_connection().expect("db");
    let soul = new_default_soul("Consolidation");
    upsert_soul(&conn, &soul).expect("soul");
    ensure_conversation(&conn, "memory-v2-consolidation", &soul.character_id)
        .expect("conversation");
    let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
    create_session_branch(&conn, "memory-v2-consolidation", &soul, &world).expect("branch");
    let branch =
        get_active_session_branch(&conn, "memory-v2-consolidation").expect("active branch");
    let assistant_id = insert_message_and_get_id(
        &conn,
        "memory-v2-consolidation",
        "assistant",
        "Aurora hears two reports.",
    )
    .expect("assistant");
    let memories = [
        (
            "first",
            "The visitor says the northern gate was locked at dawn.",
        ),
        (
            "second",
            "The guard reports the northern gate's iron chain remains fastened tonight.",
        ),
    ]
    .into_iter()
    .map(|(suffix, content)| MemoryPatch {
        memory_id: Some(format!("testimony-{suffix}")),
        content: content.into(),
        tag: Some("testimony".into()),
        source_type: Some(MemorySourceType::UserClaimed),
        source_quote: Some(format!("report {suffix}")),
        confidence: Some(0.8),
        truth_status: Some(TruthStatus::CharacterBelief),
        owner_soul_id: Some(soul.character_id.clone()),
        knowledge_scope: Some("heard_about".into()),
        ..MemoryPatch::default()
    })
    .collect();
    let patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            new_memories: memories,
            ..SoulPatch::default()
        }),
        ..EnginePatch::default()
    };
    record_turn_commit_with_patch(
        &conn,
        "memory-v2-consolidation",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        None,
        &patch,
        false,
    )
    .expect("record");

    rebuild_session_state(&conn, "memory-v2-consolidation", &branch.branch_id).expect("rebuild");
    let first =
        list_memory_v2_projection(&conn, "memory-v2-consolidation", &branch.branch_id, true)
            .expect("projection");
    let derived = first
        .iter()
        .find(|record| record.layer == "derived")
        .expect("derived belief");
    assert_eq!(derived.memory_kind, "belief");
    let source_ids: Vec<String> =
        serde_json::from_str(&derived.source_memory_ids_json).expect("source ids");
    assert_eq!(source_ids, vec!["testimony-first", "testimony-second"]);
    let evidence: Vec<state_engine::memory_v2::MemoryEvidenceRef> =
        serde_json::from_str(&derived.supporting_evidence_json).expect("evidence");
    assert_eq!(evidence.len(), 2);
    let recall = recall_memory_v2(
        &conn,
        "memory-v2-consolidation",
        &branch.branch_id,
        "northern gate",
        5,
    )
    .expect("hybrid recall");
    assert!(recall
        .iter()
        .any(|hit| hit.memory.memory_id == "testimony-first"
            && hit
                .selection_reasons
                .iter()
                .any(|reason| reason == "fts5_bm25")));
    assert!(recall.iter().any(|hit| {
        hit.selection_reasons
            .iter()
            .any(|reason| reason.starts_with("graph_"))
    }));
    assert!(recall.iter().all(|hit| hit.temporal_score > 0.0));
    assert!(recall.iter().all(|hit| hit.semantic_score == 0.0));
    let filtered = recall_memory_v2_filtered(
        &conn,
        "memory-v2-consolidation",
        &branch.branch_id,
        "northern gate",
        &MemoryV2RecallFilter {
            truth_statuses: vec!["character_belief".into()],
            memory_kinds: vec!["testimony".into()],
            owner_entity_id: Some(soul.character_id.clone()),
            created_after_ms: None,
            created_before_ms: None,
        },
        5,
    )
    .expect("filtered recall");
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|hit| hit.memory.layer == "raw"));
    assert!(filtered.iter().all(|hit| {
        hit.selection_reasons
            .iter()
            .any(|reason| reason == "filter:character")
    }));
    let excluded = recall_memory_v2_filtered(
        &conn,
        "memory-v2-consolidation",
        &branch.branch_id,
        "northern gate",
        &MemoryV2RecallFilter {
            truth_statuses: vec!["verified_engine".into()],
            ..MemoryV2RecallFilter::default()
        },
        5,
    )
    .expect("excluded recall");
    assert!(excluded.is_empty());
    struct ConstantSemantic;
    impl MemoryV2SemanticAdapter for ConstantSemantic {
        fn score(&self, _query: &str, _memory: &MemoryV2ProjectionRecord) -> Option<f32> {
            Some(0.4)
        }
    }
    let semantic = recall_memory_v2_filtered_with_semantic(
        &conn,
        "memory-v2-consolidation",
        &branch.branch_id,
        "northern gate",
        &MemoryV2RecallFilter::default(),
        &ConstantSemantic,
        5,
    )
    .expect("semantic recall");
    assert!(semantic.iter().all(|hit| hit.semantic_score == 0.4));
    assert!(semantic.iter().all(|hit| {
        hit.selection_reasons
            .iter()
            .any(|reason| reason == "local_embedding")
    }));

    rebuild_session_state(&conn, "memory-v2-consolidation", &branch.branch_id)
        .expect("replay rebuild");
    let replay =
        list_memory_v2_projection(&conn, "memory-v2-consolidation", &branch.branch_id, true)
            .expect("replay projection");
    assert_eq!(first, replay);
}

#[test]
fn memory_v2_recall_benchmark_reduces_irrelevant_context() {
    let conn = init_memory_connection().expect("db");
    let soul = new_default_soul("Recall Benchmark");
    upsert_soul(&conn, &soul).expect("soul");
    ensure_conversation(&conn, "memory-v2-recall-benchmark", &soul.character_id)
        .expect("conversation");
    let world = create_legacy_session_world_from_soul(&conn, &soul).expect("world");
    create_session_branch(&conn, "memory-v2-recall-benchmark", &soul, &world).expect("branch");
    let branch =
        get_active_session_branch(&conn, "memory-v2-recall-benchmark").expect("active branch");
    let assistant_id = insert_message_and_get_id(
        &conn,
        "memory-v2-recall-benchmark",
        "assistant",
        "A long campaign accumulates many memories.",
    )
    .expect("assistant");
    let mut memories = (0..24)
        .map(|index| MemoryPatch {
            memory_id: Some(format!("filler-{index:02}")),
            content: format!(
                "Unrelated archive note {index:02} concerns a different room and ordinary supplies."
            ),
            tag: Some("testimony".into()),
            source_type: Some(MemorySourceType::UserClaimed),
            source_quote: Some(format!("archive note {index:02}")),
            confidence: Some(0.6),
            truth_status: Some(TruthStatus::CharacterBelief),
            owner_soul_id: Some(soul.character_id.clone()),
            knowledge_scope: Some("heard_about".into()),
            ..MemoryPatch::default()
        })
        .collect::<Vec<_>>();
    memories.extend([
        MemoryPatch {
            memory_id: Some("relevant-cobalt-1".into()),
            content: "The lighthouse password is cobalt according to Mira.".into(),
            tag: Some("testimony".into()),
            source_type: Some(MemorySourceType::UserClaimed),
            source_quote: Some("password is cobalt".into()),
            confidence: Some(0.85),
            truth_status: Some(TruthStatus::CharacterBelief),
            owner_soul_id: Some(soul.character_id.clone()),
            knowledge_scope: Some("heard_about".into()),
            ..MemoryPatch::default()
        },
        MemoryPatch {
            memory_id: Some("relevant-cobalt-2".into()),
            content: "A second lighthouse log repeats the cobalt password.".into(),
            tag: Some("testimony".into()),
            source_type: Some(MemorySourceType::UserClaimed),
            source_quote: Some("cobalt password".into()),
            confidence: Some(0.8),
            truth_status: Some(TruthStatus::CharacterBelief),
            owner_soul_id: Some(soul.character_id.clone()),
            knowledge_scope: Some("heard_about".into()),
            ..MemoryPatch::default()
        },
    ]);
    let patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            new_memories: memories,
            ..SoulPatch::default()
        }),
        ..EnginePatch::default()
    };
    record_turn_commit_with_patch(
        &conn,
        "memory-v2-recall-benchmark",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        None,
        &patch,
        false,
    )
    .expect("record");
    rebuild_session_state(&conn, "memory-v2-recall-benchmark", &branch.branch_id).expect("rebuild");
    let projection = list_memory_v2_projection(
        &conn,
        "memory-v2-recall-benchmark",
        &branch.branch_id,
        false,
    )
    .expect("projection");
    let raw_context_chars = projection
        .iter()
        .filter(|memory| memory.layer == "raw")
        .map(|memory| memory.content.chars().count())
        .sum::<usize>();
    let recall = recall_memory_v2(
        &conn,
        "memory-v2-recall-benchmark",
        &branch.branch_id,
        "lighthouse cobalt password",
        6,
    )
    .expect("recall");
    let selected_context_chars = recall
        .iter()
        .map(|hit| hit.memory.content.chars().count())
        .sum::<usize>();
    assert!(recall
        .iter()
        .any(|hit| hit.memory.memory_id == "relevant-cobalt-1"));
    assert!(recall
        .iter()
        .any(|hit| hit.memory.memory_id == "relevant-cobalt-2"));
    assert!(recall
        .iter()
        .all(|hit| hit.memory.content.to_lowercase().contains("cobalt")));
    assert!(
        selected_context_chars < raw_context_chars,
        "evidence bundle should be smaller than the raw memory context: selected={selected_context_chars}, raw={raw_context_chars}"
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
    let user_id =
        insert_message_and_get_id(&conn, "eval-world", "user", "Someone knocks.").expect("user");
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

    let rebuilt = rebuild_session_state(&conn, "eval-world", &branch.branch_id).expect("rebuild");

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

    let rebuilt = rebuild_session_state(&conn, "eval-counter", &branch.branch_id).expect("rebuild");

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
    let assistant_id =
        insert_message_and_get_id(&conn, "mne-rebuild", "assistant", "Moved.").expect("assistant");
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

    let rebuilt = rebuild_session_state(&conn, "mne-rebuild", &branch.branch_id).expect("rebuild");
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
    let user_id = insert_message_and_get_id(&conn, "ledger-regen", "user", "Phone?").expect("user");
    let assistant_id =
        insert_message_and_get_id(&conn, "ledger-regen", "assistant", "Bad.").expect("assistant");
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

    let rebuilt = rebuild_session_state(&conn, "ledger-regen", &branch.branch_id).expect("rebuild");

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
    let message_id =
        insert_message_and_get_id(&conn, "variants", "assistant", "Response A").expect("assistant");

    let legacy =
        list_assistant_message_variants(&conn, "variants", message_id).expect("legacy variants");
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
    select_assistant_message_variant(&conn, "variants", message_id, base_id).expect("select base");
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

    assert!(hard_delete_message_internal(&conn, "variants", message_id).expect("delete message"));
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
    let first = insert_message_and_get_id(&conn, "rewind-session", "user", "First").expect("first");
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

    let restored = restore_inactive_messages(&conn, "restore-session").expect("restore inactive");
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
    let retry = insert_message_and_get_id(&conn, "restore-status", "user", "Retry").expect("retry");
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
    let archive_res = archive_provider_profile(&conn, "openai", &["anthropic"]).expect("archive");
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
        err_msg.contains("deprecated") || err_msg.contains("delete_provider_profile is deprecated")
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

    let canonical = insert_message_and_get_id(&conn, "hidden-dupes", "user", "Unique user message")
        .expect("canonical");
    let duplicate = insert_message_and_get_id(&conn, "hidden-dupes", "user", "Unique user message")
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
    let world =
        ensure_conversation_session_world(&conn, "scene-turn-export", &soul, None).expect("world");
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

    let rebuilt = rebuild_session_state(&conn, "horror-knock", &branch.branch_id).expect("rebuild");

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
    let world = ensure_conversation_session_world(&conn, "reset-loop", &soul, None).expect("world");
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

    let rebuilt = rebuild_session_state(&conn, "reset-loop", &branch.branch_id).expect("rebuild");

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
    let world =
        ensure_conversation_session_world(&conn, "recovery-session", &soul, None).expect("world");
    let branch = create_session_branch(&conn, "recovery-session", &soul, &world).expect("branch");
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
        insert_message_and_get_id(&conn, conversation_id, "assistant", "Pending response").unwrap();
    conn.execute(
        "UPDATE messages SET message_status = 'pending', is_active = 0 WHERE id = ?1",
        [msg_pending],
    )
    .unwrap();

    let msg_failed =
        insert_message_and_get_id(&conn, conversation_id, "assistant", "Failed response").unwrap();
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
    let restored_count = restore_turn_range(&conn, conversation_id, msg_u2, msg_failed).unwrap();
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
